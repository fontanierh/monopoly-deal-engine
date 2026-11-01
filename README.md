# Monopoly Deal Engine

A rules engine for the 2008 Hasbro Monopoly Deal card game, written in Rust. People can play it in a terminal or a local browser. AI agents can play it through JSON commands or an MCP tool server.

The project has three crates:

| Crate | What it provides |
| --- | --- |
| [`monopoly-deal-core`](crates/core) | The rules engine library. All game rules live here. |
| [`monopoly-deal-cli`](crates/cli) | The `mdeal` binary: terminal play, JSON commands, and an MCP stdio server. |
| [`monopoly-deal-web`](crates/web) | A browser table served on `127.0.0.1` only. |

This project is not affiliated with, endorsed by, or sponsored by Hasbro. "Monopoly Deal" is a trademark of its owner. This repository does not include official card art.

## Players

- 2 to 5 players use one 106-card pack.
- 6 to 42 players use two packs (212 cards).
- Other player counts are rejected.

## Install

You need a stable Rust toolchain. Install it with [rustup](https://rustup.rs). From the repository root:

```sh
cargo install --path crates/cli   # installs mdeal
cargo install --path crates/web   # installs monopoly-deal-web
```

To run from source without installing, use `cargo run -p monopoly-deal-cli --bin mdeal -- <args>` or `cargo run -p monopoly-deal-web -- <args>`.

## Play in a browser

```sh
monopoly-deal-web --players 3
```

| Option | Default |
| --- | --- |
| `--players N` | `2` |
| `--port PORT` | `8787` |
| `--seed SEED` | random |

The server prints one private link for each seat, a spectator link, and a host link. Give each player only their own seat link. Keep the host link private: it can start a new game with the same number of seats. The spectator link shows public information only.

The game exists only in server memory. Stopping the server ends the game.

## Play in a terminal

```sh
mdeal play --players 3
```

This is hot-seat play: one terminal, and players pass it between turns. The screen is cleared between players, but privacy depends on the other players looking away. To play in separate terminals, save a game and give each player a terminal:

```sh
mdeal new --players 2 --out game.json
mdeal play game.json --seats 0   # first terminal
mdeal play game.json --seats 1   # second terminal
```

At each decision, `mdeal play` lists the legal actions with numbers. Type a number to act, or `q` to quit.

Separate terminals hide hands from view only. Anyone who can read `game.json` or run `mdeal` against it can see every hand (see [Security](#security)).

## Play from a program

Each command reads or updates a saved game file and prints JSON:

| Command | Purpose |
| --- | --- |
| `mdeal new --players N [--seed S] --out game.json` | Create a saved game. |
| `mdeal status game.json` | Public status. |
| `mdeal observe game.json --seat P` | Seat `P`'s private view. |
| `mdeal legal game.json --seat P` | Seat `P`'s legal actions. |
| `mdeal act game.json --seat P 'ACTION_JSON'` | Validate and apply one action. |
| `mdeal mcp game.json --seat P` | MCP stdio server for seat `P`. |

```sh
mdeal new --players 2 --out game.json
mdeal legal game.json --seat 0
mdeal act game.json --seat 0 '{"type":"end_turn"}'
```

- Seats are numbered from 0 in the CLI. The browser labels them from 1.
- To get a valid action, copy one entry from `legal`.
- On success, a command prints JSON on stdout. On failure, it exits with status 1 and prints `{"error": {"code": ..., "message": ...}}` on stderr.
- Several processes can share one game file. Each access takes a lock on `game.json.lock`.

[crates/cli/README.md](crates/cli/README.md) describes observations, actions, error codes, and the MCP protocol in detail.

## MCP server

`mdeal mcp game.json --seat P` runs an MCP server over stdio (newline-delimited JSON-RPC 2.0). The server is bound to one seat. No tool takes a seat argument.

| Tool | Arguments | Result |
| --- | --- | --- |
| `get_game_status` | none | Phase, pending players, winner, and whether this seat must act. |
| `get_observation` | none | This seat's private view. |
| `get_legal_actions` | none | This seat's legal actions. |
| `take_action` | `{"action": {...}}` | Same result as `mdeal act`. |

An illegal action returns a tool result with `isError: true` and does not change the game.

Give each agent its own server and seat. A typical host configuration:

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

## Use the library

`monopoly-deal-core` is not published to crates.io. Add it as a path dependency:

```toml
[dependencies]
monopoly-deal-core = { path = "path/to/monopoly-deal-engine/crates/core" }
```

```rust
use monopoly_deal_core::{Game, GameConfig};

fn main() {
    let mut game = Game::new(GameConfig::new(3, 42)).expect("valid player count");

    for _ in 0..10_000 {
        if game.winner().is_some() {
            break;
        }
        let player = game.pending_actors()[0];
        let action = game
            .legal_actions(player)
            .pop()
            .expect("a pending player has a legal action");
        game.apply(player, action).expect("legal actions apply");
    }

    match game.winner() {
        Some(winner) => println!("player {} wins on turn {}", winner.0, game.turn()),
        None => println!("truncated after 10000 actions"),
    }
}
```

`Game::observation(Some(player))` returns one player's view without hidden cards. `Game`, `Action`, and `Observation` implement serde `Serialize` and `Deserialize`. The same seed and the same actions always produce the same game.

## Rules

The engine follows the 2008 Quick Start rules and card text. That text does not answer every rules question. [crates/core/RULES.md](crates/core/RULES.md) describes what the engine does. Each point is marked as **printed** or as an **engine default**. Engine defaults fill gaps in the 2008 text. They are not official rules.

The printed rules have no ending for a deadlocked game, so a game can run forever. Automated matches should limit the number of actions and report a truncated game separately from a win.

## Security

- **Seeds.** Anyone who knows a seed can rebuild the deck order. Without `--seed`, the seed comes from the operating system's secure random source and is never printed. Use fixed seeds only for tests and replays.
- **Game files.** `game.json` holds all hidden state: every hand and the draw pile order. It is created with mode `0600` on Unix. Treat it as referee data.
- **Seat binding is not a sandbox.** An agent that can read `game.json`, or run `mdeal observe` for another seat, can see other hands. For fair play between agents, run each MCP server where its agent has no file or shell access to the game. The agent should reach the game only through the MCP tools.
- **Web server.** The server listens on `127.0.0.1` only and uses plain HTTP without TLS. Do not expose it to a network, for example with a proxy or tunnel. Seat and host links carry a secret token after `#`. Treat a link like a password.

## License

MIT. See [LICENSE](LICENSE).
