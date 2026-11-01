//! Safety of the saved game file: private permissions, planted temp files and symlinks,
//! corrupt files, and concurrent writers.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use monopoly_deal_cli::engine::{self, GameFile};
use monopoly_deal_core::Game;

fn dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mdeal-file-{}-{test}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn new_file(dir: &std::path::Path) -> GameFile {
    let file = GameFile::new(dir.join("game.json"));
    file.create(&engine::new_game(3, 1).unwrap()).unwrap();
    file
}

/// Applies the first legal action of the first pending player.
fn step(game: &mut Game) {
    let seat = game.pending_actors()[0];
    let action = serde_json::to_value(&engine::legal_actions(game, seat)[0]).unwrap();
    engine::apply(game, seat, action).unwrap();
}

#[cfg(unix)]
#[test]
fn planted_temp_files_do_not_affect_writes() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let dir = dir("planted");
    let file = new_file(&dir);
    let victim = dir.join("victim.txt");
    fs::write(&victim, "keep me").unwrap();
    // Every temp name this process could pick first, plus the old fixed name.
    let pid = std::process::id();
    symlink(&victim, dir.join(format!("game.json.{pid}-0.tmp"))).unwrap();
    for name in [
        format!("game.json.{pid}-1.tmp"),
        "game.json.tmp".to_string(),
    ] {
        fs::write(dir.join(&name), "stale").unwrap();
        fs::set_permissions(dir.join(&name), fs::Permissions::from_mode(0o644)).unwrap();
    }

    file.update(|game| {
        step(game);
        Ok(())
    })
    .unwrap();

    assert_eq!(fs::read_to_string(&victim).unwrap(), "keep me");
    assert_eq!(
        fs::read_to_string(dir.join(format!("game.json.{pid}-1.tmp"))).unwrap(),
        "stale"
    );
    let mode = fs::metadata(dir.join("game.json"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    file.read().unwrap();
}

#[cfg(unix)]
#[test]
fn symlinked_game_or_lock_is_refused() {
    use std::os::unix::fs::symlink;
    let dir = dir("symlink");
    let real = new_file(&dir);
    symlink(real.path(), dir.join("link.json")).unwrap();
    let link = GameFile::new(dir.join("link.json"));
    assert_eq!(link.read().unwrap_err().code, "symlink");
    assert_eq!(link.update(|_| Ok(())).unwrap_err().code, "symlink");
    assert_eq!(
        link.create(&engine::new_game(2, 1).unwrap())
            .unwrap_err()
            .code,
        "symlink"
    );

    fs::write(dir.join("other.json"), fs::read(real.path()).unwrap()).unwrap();
    symlink(dir.join("victim.lock"), dir.join("other.json.lock")).unwrap();
    assert_eq!(
        GameFile::new(dir.join("other.json"))
            .read()
            .unwrap_err()
            .code,
        "symlink"
    );
    assert!(!dir.join("victim.lock").exists());
}

#[test]
fn corrupt_file_errors_do_not_quote_contents() {
    let dir = dir("corrupt");
    let path = dir.join("game.json");
    fs::write(&path, r#""secret-password-123""#).unwrap();
    let err = GameFile::new(&path).read().unwrap_err();
    assert_eq!(err.code, "corrupt_game_file");
    assert!(!err.message.contains("secret"), "{}", err.message);
}

#[test]
fn failed_update_leaves_file_and_no_temp_behind() {
    let dir = dir("failed");
    let file = new_file(&dir);
    let before = fs::read(file.path()).unwrap();
    let err = file
        .update(|game| {
            step(game);
            Err::<(), _>(monopoly_deal_cli::mcp::ToolError::new("test", "abort"))
        })
        .unwrap_err();
    assert_eq!(err.code, "test");
    assert_eq!(fs::read(file.path()).unwrap(), before);
    let names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().all(|n| !n.ends_with(".tmp")), "{names:?}");
}

#[test]
fn concurrent_updates_are_not_lost() {
    let dir = dir("concurrent");
    let file = Arc::new(new_file(&dir));
    let (threads, steps) = (4, 25);
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let file = Arc::clone(&file);
            thread::spawn(move || {
                for _ in 0..steps {
                    file.update(|game| {
                        step(game);
                        Ok(())
                    })
                    .unwrap();
                    file.read().unwrap();
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }

    let mut expected = engine::new_game(3, 1).unwrap();
    for _ in 0..threads * steps {
        step(&mut expected);
    }
    assert_eq!(file.read().unwrap(), expected);
}
