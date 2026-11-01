# Contributing

Contributions are released under the MIT License (see [LICENSE](LICENSE)).

## Setup

You need a stable Rust toolchain with `rustfmt` and `clippy`. Install it with [rustup](https://rustup.rs).

```sh
cargo build --workspace
```

## Checks

CI runs these commands on every push and pull request (see [.github/workflows/ci.yml](.github/workflows/ci.yml)). Run them from the repository root before you open a pull request:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Run `cargo fmt --all` to fix formatting.

## Where code goes

| Path | Contents |
| --- | --- |
| `crates/core/src/cards.rs` | Card definitions, values, and rent ladders. |
| `crates/core/src/game.rs` | Game state, actions, observations, and rules. |
| `crates/core/src/game/tests.rs` | Rules tests. |
| `crates/core/RULES.md` | Engine rules: printed rules and engine defaults. |
| `crates/cli/src/` | `mdeal`: argument parsing, game files, terminal rendering, MCP server. |
| `crates/cli/tests/` | CLI, MCP protocol, and full-game tests. |
| `crates/web/src/` | Web server, seat tokens, and the adapter to core. |
| `crates/web/static/` | Browser UI. These files are compiled into the binary, so rebuild after you change them. |
| `crates/web/tests/` | Web server and core integration tests. |

All game rules belong in `monopoly-deal-core`. The CLI and web crates parse input, call core, and show the views that core returns. They must not decide whether an action is legal.

## Rules changes

- Base rule changes on the 2008 Quick Start rules or 2008 card text. Cite the source in the pull request.
- If the 2008 text does not answer the question, open an issue before you change behavior. Do not settle an open rule question in a pull request without discussion.
- Update [crates/core/RULES.md](crates/core/RULES.md) in the same pull request. Mark the point as **printed** or **engine default**.
- Add a test in `crates/core/src/game/tests.rs` for each rule you add or change.
- Keep games deterministic: the same seed and the same actions must produce the same game.
- Do not let a player's view include hidden information: other players' hands or the draw pile order.

## Pull requests

- Keep each pull request to one change.
- Add or update tests for every behavior change.
- Update the README or [crates/cli/README.md](crates/cli/README.md) when a command, option, MCP tool, or output format changes.
- Describe what changed and why.

## Content you must not add

- Official card art, card scans, or other copied Hasbro material.
- Secrets, tokens, local paths, or personal data.

## Reporting bugs

Open an issue with:

- the command or code you ran, including the seed if the bug depends on the deal,
- what you expected,
- what happened,
- the rule source, if the bug is about a card or a rule.

A failing test is the most useful reproduction.
