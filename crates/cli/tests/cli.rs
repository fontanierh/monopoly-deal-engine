//! End-to-end tests of the `mdeal` binary against the real core. They check the CLI's own
//! guarantees (privacy, validation, determinism, file handling) without assuming rule
//! details, which core tests own.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use serde_json::{json, Value};

fn mdeal(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mdeal"))
        .args(args)
        .output()
        .unwrap()
}

fn ok_json(args: &[&str]) -> Value {
    let out = mdeal(args);
    assert!(
        out.status.success(),
        "mdeal {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

fn err_code(args: &[&str]) -> String {
    let out = mdeal(args);
    assert!(
        !out.status.success(),
        "mdeal {args:?} unexpectedly succeeded"
    );
    let err: Value = serde_json::from_slice(&out.stderr).unwrap();
    err["error"]["code"].as_str().unwrap().to_string()
}

/// A fresh game file path in a per-test directory.
fn game_path(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mdeal-test-{}-{test}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir.join("game.json")
}

fn new_game(test: &str, players: &str, seed: &str) -> String {
    let path = game_path(test).to_string_lossy().into_owned();
    ok_json(&["new", "--players", players, "--seed", seed, "--out", &path]);
    path
}

fn pending(path: &str) -> Vec<u64> {
    ok_json(&["status", path])["pending_actors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_u64().unwrap())
        .collect()
}

fn idle_seat(path: &str, players: u64) -> u64 {
    let pending = pending(path);
    (0..players).find(|s| !pending.contains(s)).unwrap()
}

#[test]
fn same_seed_gives_same_game() {
    let a = new_game("seed-a", "3", "42");
    let b = new_game("seed-b", "3", "42");
    for seat in ["0", "1", "2"] {
        assert_eq!(
            ok_json(&["observe", &a, "--seat", seat]),
            ok_json(&["observe", &b, "--seat", seat])
        );
    }
}

#[test]
fn observation_only_shows_own_hand() {
    let path = new_game("privacy", "3", "7");
    let obs0 = ok_json(&["observe", &path, "--seat", "0"]);
    let obs1 = ok_json(&["observe", &path, "--seat", "1"]);
    assert_ne!(obs0["hand"], obs1["hand"]);
    for players in [&obs0["players"], &obs1["players"]] {
        for player in players.as_array().unwrap() {
            assert!(
                player.get("hand").is_none(),
                "other hands must only appear as counts: {player}"
            );
        }
    }
    let hand1: Vec<&Value> = obs1["hand"].as_array().unwrap().iter().collect();
    assert!(!obs0["hand"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| hand1.contains(&c)));
    for card in hand1 {
        assert!(
            obs0["cards"].get(card.to_string()).is_none(),
            "card names must not reveal other hands"
        );
    }
}

#[test]
fn legal_actions_only_for_pending_seat() {
    let path = new_game("legal", "4", "3");
    let actor = pending(&path)[0].to_string();
    let idle = idle_seat(&path, 4).to_string();
    assert!(!ok_json(&["legal", &path, "--seat", &actor])["actions"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(ok_json(&["legal", &path, "--seat", &idle])["actions"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn act_applies_legal_action_and_rejects_others_without_changes() {
    let path = new_game("act", "2", "11");
    let actor = pending(&path)[0].to_string();
    let idle = idle_seat(&path, 2).to_string();
    let action = ok_json(&["legal", &path, "--seat", &actor])["actions"][0].to_string();

    let before = fs::read(&path).unwrap();
    assert_eq!(
        err_code(&["act", &path, "--seat", &idle, &action]),
        "rejected"
    );
    assert_eq!(
        err_code(&[
            "act",
            &path,
            "--seat",
            &actor,
            r#"{"type":"no_such_action"}"#
        ]),
        "invalid_action"
    );
    assert_eq!(
        err_code(&["act", &path, "--seat", &actor, "not json"]),
        "invalid_action"
    );
    assert_eq!(
        err_code(&["act", &path, "--seat", "99", &action]),
        "invalid_seat"
    );
    assert_eq!(
        fs::read(&path).unwrap(),
        before,
        "rejected actions must not change the game"
    );

    let result = ok_json(&["act", &path, "--seat", &actor, &action]);
    assert_eq!(result["accepted"], true);
    assert_eq!(
        result["observation"]["viewer"],
        json!(actor.parse::<u64>().unwrap())
    );
    assert_ne!(fs::read(&path).unwrap(), before);
}

#[test]
fn new_refuses_to_overwrite_and_validates_player_count() {
    let path = new_game("overwrite", "2", "1");
    assert_eq!(
        err_code(&["new", "--players", "2", "--out", &path]),
        "file_exists"
    );
    let other = game_path("one-player").to_string_lossy().into_owned();
    assert_eq!(
        err_code(&["new", "--players", "1", "--out", &other]),
        "invalid_config"
    );
    assert_eq!(err_code(&["new", "--players", "2"]), "usage");
}

#[test]
fn more_than_five_players_are_supported() {
    let path = new_game("six", "6", "5");
    assert_eq!(ok_json(&["status", &path])["num_players"], 6);
    ok_json(&["observe", &path, "--seat", "5"]);
}

#[cfg(unix)]
#[test]
fn game_file_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let path = new_game("mode", "2", "1");
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn mcp_server_plays_its_own_seat() {
    let path = new_game("mcp", "2", "9");
    let actor = pending(&path)[0];
    let idle = idle_seat(&path, 2);
    let action = ok_json(&["legal", &path, "--seat", &actor.to_string()])["actions"][0].clone();

    let session = |seat: u64, requests: Vec<Value>| -> Vec<Value> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_mdeal"))
            .args(["mcp", &path, "--seat", &seat.to_string()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        for request in requests {
            writeln!(stdin, "{request}").unwrap();
        }
        drop(stdin);
        let out = child.wait_with_output().unwrap();
        String::from_utf8(out.stdout)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    };
    let init = json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}});
    let call = |id: u64, name: &str, arguments: Value| json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": arguments}});

    let idle_out = session(
        idle,
        vec![
            init.clone(),
            call(1, "get_game_status", json!({})),
            call(2, "take_action", json!({"action": action})),
            call(3, "get_observation", json!({})),
        ],
    );
    assert_eq!(
        idle_out[1]["result"]["structuredContent"]["your_decision"],
        false
    );
    assert_eq!(idle_out[2]["result"]["isError"], true);
    assert_eq!(
        idle_out[3]["result"]["structuredContent"],
        ok_json(&["observe", &path, "--seat", &idle.to_string()])
    );

    let actor_out = session(
        actor,
        vec![init, call(1, "take_action", json!({"action": action}))],
    );
    assert_eq!(actor_out[1]["result"]["isError"], false, "{}", actor_out[1]);
    assert_eq!(
        actor_out[1]["result"]["structuredContent"]["accepted"],
        true
    );
}

#[test]
fn hot_seat_shows_the_acting_player_and_quits() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mdeal"))
        .args(["play", "--players", "2", "--seed", "1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"\nq\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("look away"));
    assert!(text.contains("Your legal actions:"));
}
