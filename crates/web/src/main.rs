use std::{
    net::{Ipv4Addr, SocketAddr},
    process::ExitCode,
};

use monopoly_deal_web::{
    core_engine::CoreGame,
    server::{router, AppState},
};

const USAGE: &str = "usage: monopoly-deal-web [--players N] [--port PORT] [--seed SEED]

Serves a local Monopoly Deal table on http://127.0.0.1:PORT (loopback only).
Each seat gets a private link; share each link only with that player.";

struct Args {
    players: usize,
    port: u16,
    seed: Option<u64>,
}

/// `Ok(None)` means help was requested.
fn parse_args() -> Result<Option<Args>, String> {
    let mut args = Args {
        players: 2,
        port: 8787,
        seed: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        if flag == "-h" || flag == "--help" {
            return Ok(None);
        }
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        let bad = |_| format!("invalid value for {flag}: {value}");
        match flag.as_str() {
            "--players" => args.players = value.parse().map_err(bad)?,
            "--port" => args.port = value.parse().map_err(bad)?,
            "--seed" => args.seed = Some(value.parse().map_err(bad)?),
            _ => return Err(format!("unknown option {flag}")),
        }
    }
    Ok(Some(args))
}

fn random_seed() -> u64 {
    let mut b = [0u8; 8];
    getrandom::fill(&mut b).expect("OS random number generator unavailable");
    u64::from_le_bytes(b)
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(Some(a)) => a,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("error: {e}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    // Bind first so `--port 0` reports and allows the port actually chosen.
    let requested = SocketAddr::from((Ipv4Addr::LOCALHOST, args.port));
    let listener = match tokio::net::TcpListener::bind(requested).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: cannot listen on {requested}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let addr = match listener.local_addr() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let players = args.players;
    let first_seed = args.seed.unwrap_or_else(random_seed);
    let pending_seed = std::sync::Mutex::new(Some(first_seed));
    let new_game = Box::new(move || {
        // The first game uses --seed; restarts are freshly shuffled.
        let seed = pending_seed
            .lock()
            .unwrap()
            .take()
            .unwrap_or_else(random_seed);
        CoreGame::new(players, seed)
    });
    let state = match AppState::new(new_game, addr) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let creds = state.credentials();
    println!("Monopoly Deal on http://{addr} (this computer only, plain HTTP)");
    if args.seed.is_some() {
        println!("Warning: --seed was given, so anyone who knows it can reconstruct the deck.");
    }
    println!("Give each player only their own link:");
    for (i, token) in creds.seats.iter().enumerate() {
        println!("  Seat {}: http://{addr}/#t={token}", i + 1);
    }
    println!("  Spectator: http://{addr}/");
    println!("  Host (can restart): http://{addr}/#host={}", creds.host);

    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    if let Err(e) = axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown)
        .await
    {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
