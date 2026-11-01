use std::io::{self, BufRead, Write};
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

use monopoly_deal_cli::engine::{self, FileSeat, GameFile};
use monopoly_deal_cli::mcp::{Server, ToolError};
use monopoly_deal_cli::render;
use serde_json::{json, Value};

const USAGE: &str = "\
mdeal: play Monopoly Deal in a terminal or expose one seat to an AI agent over MCP

USAGE:
  mdeal new --players N [--seed S] --out GAME.json create a saved game
  mdeal status GAME.json                           public status
  mdeal observe GAME.json --seat P                 seat P's private view (JSON)
  mdeal legal GAME.json --seat P                   seat P's legal actions (JSON)
  mdeal act GAME.json --seat P 'ACTION_JSON'       validate and apply one action
  mdeal mcp GAME.json --seat P                     MCP stdio server bound to seat P
  mdeal play --players N [--seed S]                hot-seat game, all seats in this terminal
  mdeal play GAME.json --seats 0,2                 play some seats of a shared saved game

Without --seed the deal is random. Anyone who knows a seed can reconstruct the deck.
GAME.json holds all hidden state. Only trusted referee software should read it.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!(
                "{}",
                json!({"error": {"code": e.code, "message": e.message}})
            );
            ExitCode::FAILURE
        }
    }
}

/// Parsed command-line arguments: positionals plus `--flag value` pairs.
struct Args {
    positional: Vec<String>,
    flags: Vec<(String, String)>,
}

impl Args {
    fn parse(args: &[String]) -> Result<Self, ToolError> {
        let mut positional = Vec::new();
        let mut flags = Vec::new();
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            if let Some(name) = arg.strip_prefix("--") {
                let value = iter
                    .next()
                    .ok_or_else(|| usage_error(format!("--{name} needs a value")))?;
                flags.push((name.to_string(), value.clone()));
            } else {
                positional.push(arg.clone());
            }
        }
        Ok(Self { positional, flags })
    }

    fn flag(&self, name: &str) -> Option<&str> {
        self.flags
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn number<T: std::str::FromStr>(&self, name: &str) -> Result<T, ToolError> {
        let value = self
            .flag(name)
            .ok_or_else(|| usage_error(format!("missing --{name}")))?;
        value
            .parse()
            .map_err(|_| usage_error(format!("--{name} must be a number")))
    }

    fn game_file(&self) -> Result<GameFile, ToolError> {
        self.positional
            .first()
            .map(GameFile::new)
            .ok_or_else(|| usage_error("missing GAME.json"))
    }

    fn check_known(&self, allowed: &[&str], max_positional: usize) -> Result<(), ToolError> {
        if let Some((name, _)) = self
            .flags
            .iter()
            .find(|(n, _)| !allowed.contains(&n.as_str()))
        {
            return Err(usage_error(format!("unknown flag --{name}")));
        }
        if self.positional.len() > max_positional {
            return Err(usage_error(format!(
                "unexpected argument {}",
                self.positional[max_positional]
            )));
        }
        Ok(())
    }
}

fn usage_error(message: impl Into<String>) -> ToolError {
    ToolError::new("usage", format!("{}\n\n{USAGE}", message.into()))
}

fn run(args: &[String]) -> Result<(), ToolError> {
    let Some((command, rest)) = args.split_first() else {
        println!("{USAGE}");
        return Ok(());
    };
    let args = Args::parse(rest)?;
    match command.as_str() {
        "new" => {
            args.check_known(&["players", "seed", "out"], 0)?;
            let game = engine::new_game(args.number("players")?, seed(&args)?)?;
            let file = GameFile::new(
                args.flag("out")
                    .ok_or_else(|| usage_error("missing --out"))?,
            );
            file.create(&game)?;
            print_json(&engine::status(&game, None));
        }
        "status" => {
            args.check_known(&[], 1)?;
            print_json(&engine::status(&args.game_file()?.read()?, None));
        }
        "observe" | "legal" => {
            args.check_known(&["seat"], 1)?;
            let game = args.game_file()?.read()?;
            let seat = engine::check_seat(&game, args.number("seat")?)?;
            if command == "observe" {
                print_json(&engine::observation(&game, Some(seat)));
            } else {
                print_json(&json!({"actions": engine::legal_actions(&game, seat)}));
            }
        }
        "act" => {
            args.check_known(&["seat"], 2)?;
            let text = args
                .positional
                .get(1)
                .ok_or_else(|| usage_error("missing ACTION_JSON"))?;
            let action: Value = serde_json::from_str(text).map_err(|e| {
                ToolError::new("invalid_action", format!("ACTION_JSON is not JSON: {e}"))
            })?;
            let seat_number = args.number("seat")?;
            let result = args.game_file()?.update(|game| {
                let seat = engine::check_seat(game, seat_number)?;
                engine::apply(game, seat, action)
            })?;
            print_json(&result);
        }
        "mcp" => {
            args.check_known(&["seat"], 1)?;
            let seat = FileSeat::open(args.game_file()?, args.number("seat")?)?;
            Server::new(seat)
                .serve(io::stdin().lock(), io::stdout().lock())
                .map_err(|e| ToolError::new("io", e.to_string()))?;
        }
        "play" => {
            args.check_known(&["players", "seed", "seats"], 1)?;
            if args.positional.is_empty() {
                let game = engine::new_game(args.number("players")?, seed(&args)?)?;
                let seats: Vec<u8> = (0..engine::num_players(&game)).map(|s| s as u8).collect();
                play(Table::Memory(game), &seats)?;
            } else {
                let file = args.game_file()?;
                let game = file.read()?;
                let seats = match args.flag("seats") {
                    Some(list) => parse_seats(list)?,
                    None => (0..engine::num_players(&game)).map(|s| s as u8).collect(),
                };
                for &seat in &seats {
                    engine::check_seat(&game, seat)?;
                }
                play(Table::File(file), &seats)?;
            }
        }
        "help" | "-h" | "--help" => println!("{USAGE}"),
        other => return Err(usage_error(format!("unknown command {other}"))),
    }
    Ok(())
}

/// `--seed` gives a reproducible deal; without it the seed is random and never shown.
fn seed(args: &Args) -> Result<u64, ToolError> {
    if args.flag("seed").is_none() {
        return engine::random_seed();
    }
    eprintln!("warning: anyone who knows --seed can reconstruct the deck. Use seeds only for tests and replays.");
    args.number("seed")
}

fn parse_seats(list: &str) -> Result<Vec<u8>, ToolError> {
    list.split(',')
        .map(|s| {
            s.trim()
                .parse()
                .map_err(|_| usage_error(format!("bad seat list {list}")))
        })
        .collect()
}

fn print_json(value: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("JSON values serialize")
    );
}

enum Table {
    Memory(monopoly_deal_core::Game),
    File(GameFile),
}

impl Table {
    fn snapshot(&self) -> Result<monopoly_deal_core::Game, ToolError> {
        match self {
            Table::Memory(game) => Ok(game.clone()),
            Table::File(file) => file.read(),
        }
    }

    fn apply(&mut self, seat: u8, action: Value) -> Result<Value, ToolError> {
        match self {
            Table::Memory(game) => {
                let seat = engine::check_seat(game, seat)?;
                engine::apply(game, seat, action)
            }
            Table::File(file) => file.update(|game| {
                let seat = engine::check_seat(game, seat)?;
                engine::apply(game, seat, action)
            }),
        }
    }
}

/// Hot-seat play for the seats controlled by this terminal.
fn play(mut table: Table, seats: &[u8]) -> Result<(), ToolError> {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let mut shown: Option<u8> = None;
    let mut waiting = false;
    loop {
        let game = table.snapshot()?;
        if let Some(winner) = game.winner() {
            println!("\nGame over. Player {} wins.", winner.0);
            return Ok(());
        }
        let pending = game.pending_actors();
        let Some(seat) = seats
            .iter()
            .copied()
            .find(|s| pending.iter().any(|p| p.0 == *s))
        else {
            if !waiting {
                let others: Vec<u8> = pending.iter().map(|p| p.0).collect();
                println!("Waiting for player(s) {others:?}...");
                waiting = true;
            }
            thread::sleep(Duration::from_millis(500));
            continue;
        };
        waiting = false;
        if seats.len() > 1 && shown != Some(seat) {
            clear_screen();
            println!(
                "Pass the terminal to player {seat}. Everyone else: look away from the screen."
            );
            prompt(
                &format!("Player {seat}, press Enter when only you can see the screen."),
                &mut lines,
            )?;
        }
        shown = Some(seat);
        let player = engine::check_seat(&game, seat)?;
        let obs = engine::observation(&game, Some(player));
        let actions = obs["legal_actions"].as_array().cloned().unwrap_or_default();
        clear_screen();
        println!("=== Player {seat} ===");
        println!("{}", render::observation(&obs));
        let Some(line) = prompt("Choose an action number (q to quit):", &mut lines)? else {
            return Ok(());
        };
        let line = line.trim();
        if line == "q" {
            return Ok(());
        }
        let Some(action) = line.parse::<usize>().ok().and_then(|i| actions.get(i)) else {
            prompt(
                &format!("'{line}' is not a listed action number. Press Enter."),
                &mut lines,
            )?;
            continue;
        };
        if let Err(e) = table.apply(seat, action.clone()) {
            prompt(
                &format!("Rejected: {}. Press Enter.", e.message),
                &mut lines,
            )?;
        }
    }
}

/// Clears the screen and, in terminals that support it, the scrollback.
fn clear_screen() {
    print!("\x1b[2J\x1b[3J\x1b[H");
}

fn prompt(
    text: &str,
    lines: &mut impl Iterator<Item = io::Result<String>>,
) -> Result<Option<String>, ToolError> {
    print!("{text} ");
    io::stdout()
        .flush()
        .map_err(|e| ToolError::new("io", e.to_string()))?;
    lines
        .next()
        .transpose()
        .map_err(|e| ToolError::new("io", e.to_string()))
}
