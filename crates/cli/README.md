# monopoly-deal-cli

`mdeal` lets people play Monopoly Deal in a terminal, and lets any AI agent or harness
play through either plain JSON commands or an [MCP](https://modelcontextprotocol.io)
stdio tool server. No model provider, API key or network access is needed.

All rules live in `monopoly-deal-core`. This crate only parses input, passes it to
core, and prints the views core produces for one player.

```sh
cargo run -p monopoly-deal-cli --bin mdeal -- help
```

## Commands

| Command | Purpose |
| --- | --- |
| `mdeal new --players N [--seed S] --out game.json` | Create a saved game. |
| `mdeal status game.json` | Public status: phase, pending actors, winner. |
| `mdeal observe game.json --seat P` | Seat `P`'s private observation (see below). |
| `mdeal legal game.json --seat P` | Every action seat `P` may take now (`{"actions": [...]}`; empty if it is not their decision). |
| `mdeal act game.json --seat P 'ACTION'` | Validate and apply one action, then print the public status and seat `P`'s new observation. |
| `mdeal mcp game.json --seat P` | MCP stdio server that controls seat `P`. |
| `mdeal play --players N [--seed S]` | Hot-seat game in this terminal. |
| `mdeal play game.json [--seats 0,2]` | Play some seats of a saved game while other seats are played elsewhere (for example by MCP agents). |

Without `--seed`, the seed comes from the operating system's cryptographic random
source and is never printed. With `--seed`, the same seed always gives the same deal,
which helps with tests and replays. Anyone who knows a seed can reconstruct the deck
order, so `mdeal` prints a warning when one is given.

Seats are numbered from 0. The CLI has no player limit of its own: core decides which
player counts are valid.

Successful commands print JSON on stdout. Failures exit with status 1 and print
`{"error": {"code": "...", "message": "..."}}` on stderr. Error codes include
`usage`, `invalid_seat`, `invalid_action` (the JSON is not an action), `rejected`
(core refused the action) and `io`.

### Observations

`observe` prints core's observation for one seat: the seat's own `hand`, each player's
`hand_count`, bank and property sets, the `discard` pile, `phase`, `pending_actors`,
`winner`, the seat's `legal_actions`, and a `log` of recent public events. The CLI adds
`cards`, which maps every card ID mentioned in the observation to its public
definition, for example `"17": {"name": "5M", "value": 5, "kind": {"type": "money"}}`.
Cards in other players' hands and in the draw pile are never listed.

### Actions

An action is a JSON object tagged by `type`, for example `{"type": "end_turn"}`,
`{"type": "bank", "card": 17}` or `{"type": "pay_card", "card": 40}`. Card, player and
set IDs are plain numbers. The full list of action types is the `Action` enum in
`monopoly-deal-core`. Payments and discards are made one card at a time with
`pay_card` and `discard`, so `legal` always lists every possible action. The easiest
correct strategy for a program is to copy one entry from `legal` verbatim.

## MCP tool server

`mdeal mcp game.json --seat P` speaks MCP over stdio: newline-delimited JSON-RPC 2.0 on
stdin/stdout, with nothing else written to stdout. It supports `initialize`,
`notifications/initialized`, `ping`, `tools/list` and `tools/call`.

The server speaks protocol versions `2025-06-18`, `2025-03-26` and `2024-11-05`. It
uses the version the client asks for, or answers with `2025-06-18` if the client asks
for another one. JSON-RPC batches (one line holding an array of messages) are accepted
in `2025-03-26` and `2024-11-05` sessions and rejected in `2025-06-18` sessions, as each
version of the specification requires.

| Tool | Arguments | Result |
| --- | --- | --- |
| `get_game_status` | none | Phase, pending actors, winner, `you`, `your_decision`. |
| `get_observation` | none | This seat's private observation. |
| `get_legal_actions` | none | `{"actions": [...]}` for this seat. |
| `take_action` | `{"action": {...}}` | Same result as `mdeal act`. |

The three `get_*` tools are annotated `readOnlyHint: true`. `take_action` is annotated
as a write that is not destructive or idempotent. All tools are `openWorldHint: false`:
they never reach outside the game. Hosts that ask for approval before write tools will
therefore ask only for `take_action`.

Results are returned both as `structuredContent` and as JSON text in `content`. An
illegal or malformed action is a tool result with `isError: true` and an
`{"error": {...}}` body, so the model can read it and try again. The game state does
not change when an action is rejected.

The seat is fixed when the server starts, and no tool takes a seat argument. A client
of the server can therefore only see its own seat's view.

### Example host configuration

Most MCP hosts accept a server entry with a command and arguments. Give each agent its
own server and seat:

```json
{
  "mcpServers": {
    "monopoly-deal": {
      "command": "/path/to/mdeal",
      "args": ["mcp", "/path/to/game.json", "--seat", "1"]
    }
  }
}
```

### Example session

```text
-> {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"my-harness","version":"1"}}}
<- {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"monopoly-deal-engine","version":"0.1.0"},"instructions":"..."}}
-> {"jsonrpc":"2.0","method":"notifications/initialized"}
-> {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_legal_actions","arguments":{}}}
<- {"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"{\"actions\":[...]}"}],"structuredContent":{"actions":[...]},"isError":false}}
-> {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"take_action","arguments":{"action":{"type":"end_turn"}}}}
```

### Codex

In `~/.codex/config.toml`:

```toml
[mcp_servers.monopoly-deal]
command = "/path/to/mdeal"
args = ["mcp", "/path/to/game.json", "--seat", "1"]
# Ask before every move; the read-only tools run without asking:
default_tools_approval_mode = "writes"
# Or let the agent play unattended:
# [mcp_servers.monopoly-deal.tools.take_action]
# approval_mode = "approve"
```

A harness that controls several seats starts one server per seat. Each seat calls
`get_game_status` and acts when `your_decision` is true; seats do not need to act in
any fixed order.

## Sharing a game and trust

Every `mdeal` process that uses the same `game.json` sees the same game. Each read or
change takes an advisory lock on `game.json.lock`, and changes replace the file
atomically. Humans (`mdeal play game.json --seats 0`), shell scripts and MCP servers
can therefore play the same game at once.

On Unix, the game file and the lock file are created with mode 0600 (owner read/write
only). Every save writes a new private temporary file next to the game and renames it
over the game. `mdeal` refuses to use a game or lock path that is a symbolic link.
Errors about a damaged game file give only the line and column, never file contents.

`game.json` contains **all hidden state**: every hand and the order of the draw pile.
The file, the `mdeal` binary and every MCP server process are trusted referee
infrastructure, run by the operator rather than by an agent. Seat binding only protects players who reach the game through the CLI
commands or MCP tools. It is not a sandbox: an agent that can read `game.json`, or
run `mdeal observe` for another seat, can see other hands. For fair play between AI
agents, run the MCP servers where the agents have no filesystem or shell access to the
game file, and connect them only through the MCP tools.

## Hot-seat play

`mdeal play` shows one player's private hand at a time. Between players it clears the
screen and asks the next player to press Enter. It also sends the escape sequence that
clears scrollback, but not every terminal honours it, and terminal logs or
multiplexers may keep a copy. Privacy is not guaranteed: it depends on the other
players looking away from the screen. For real privacy, give each person their own
terminal with `mdeal play game.json --seats P`.

At each decision the CLI lists the legal actions with numbers. Type a number to take
that action, or `q` to quit. A saved game can be resumed later with
`mdeal play game.json`.
