//! The only module that talks to the core crate. Every surface (CLI commands, hot-seat
//! play, MCP) goes through these functions, so rule validation stays in core and every
//! view is produced by core's per-viewer `observation`.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use std::collections::{BTreeMap, BTreeSet};

use monopoly_deal_core::{
    card_def, Action, CardDef, CardId, Game, GameConfig, Observation, PlayerId,
};
use serde_json::{json, Value};

use crate::mcp::{Seat, ToolError};

/// A seed from the operating system's cryptographic random source. Anyone who knows a
/// game's seed can reconstruct its deck order, so random seeds are never printed.
pub fn random_seed() -> Result<u64, ToolError> {
    getrandom::u64().map_err(|e| ToolError::new("random", e.to_string()))
}

pub fn new_game(num_players: usize, seed: u64) -> Result<Game, ToolError> {
    Game::new(GameConfig { num_players, seed })
        .map_err(|e| ToolError::new("invalid_config", e.to_string()))
}

pub fn num_players(game: &Game) -> usize {
    game.num_players()
}

pub fn check_seat(game: &Game, seat: u8) -> Result<PlayerId, ToolError> {
    let n = num_players(game);
    if usize::from(seat) < n {
        Ok(PlayerId(seat))
    } else {
        Err(ToolError::new(
            "invalid_seat",
            format!("seat {seat} does not exist; seats are 0..{}", n - 1),
        ))
    }
}

/// Public status that every seat may see.
pub fn status(game: &Game, seat: Option<PlayerId>) -> Value {
    let pending = game.pending_actors();
    let mut status = json!({
        "phase": game.phase(),
        "num_players": num_players(game),
        "pending_actors": pending,
        "winner": game.winner(),
        "game_over": game.winner().is_some(),
    });
    if let Some(seat) = seat {
        status["you"] = json!(seat);
        status["your_decision"] = json!(pending.contains(&seat));
    }
    status
}

/// Core's observation for `seat`, plus a `cards` map from every card id it mentions to
/// that card's public definition (name, value, kind).
pub fn observation(game: &Game, seat: Option<PlayerId>) -> Value {
    let obs = game.observation(seat);
    let cards: BTreeMap<String, &CardDef> = visible_cards(&obs)
        .into_iter()
        .map(|id| (id.0.to_string(), card_def(id)))
        .collect();
    let mut value = json!(obs);
    value["cards"] = json!(cards);
    value
}

/// Every card on the table, in the viewer's hand, or named by a public log event (a played
/// card may since have been reshuffled into the draw pile). Legal actions only mention
/// cards in the hand or on the table.
fn visible_cards(obs: &Observation) -> BTreeSet<CardId> {
    let mut ids: BTreeSet<CardId> = obs.hand.iter().chain(&obs.discard).copied().collect();
    for player in &obs.players {
        ids.extend(&player.bank);
        for set in &player.properties {
            ids.extend(set.cards.iter().chain(&set.house).chain(&set.hotel));
        }
    }
    for event in &obs.log {
        log_cards(&json!(event), &mut ids);
    }
    ids
}

fn log_cards(value: &Value, ids: &mut BTreeSet<CardId>) {
    let Value::Object(fields) = value else { return };
    for (key, v) in fields {
        match (key.as_str(), v) {
            ("card" | "take" | "give", Value::Number(n)) => {
                ids.extend(n.as_u64().map(|n| CardId(n as u16)))
            }
            ("doubles", Value::Array(items)) => ids.extend(
                items
                    .iter()
                    .filter_map(Value::as_u64)
                    .map(|n| CardId(n as u16)),
            ),
            (_, Value::Object(_)) => log_cards(v, ids),
            _ => {}
        }
    }
}

pub fn legal_actions(game: &Game, seat: PlayerId) -> Vec<Action> {
    game.legal_actions(seat)
}

/// Parses untrusted JSON into an action and lets core validate and apply it.
pub fn apply(game: &mut Game, seat: PlayerId, action: Value) -> Result<Value, ToolError> {
    let action: Action = serde_json::from_value(action)
        .map_err(|e| ToolError::new("invalid_action", format!("not a valid action: {e}")))?;
    game.apply(seat, action)
        .map_err(|e| ToolError::new("rejected", e.to_string()))?;
    Ok(
        json!({"accepted": true, "status": status(game, Some(seat)), "observation": observation(game, Some(seat))}),
    )
}

/// A game saved as JSON. The file holds all hidden state, so it is trusted referee data.
/// Each read or update holds an advisory lock on `<path>.lock`, and updates replace the
/// file atomically, so several `mdeal` processes can share one game.
pub struct GameFile {
    path: PathBuf,
}

impl GameFile {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn create(&self, game: &Game) -> Result<(), ToolError> {
        let _lock = self.lock()?;
        if fs::symlink_metadata(&self.path).is_ok() {
            return Err(ToolError::new(
                "file_exists",
                format!("{} already exists", self.path.display()),
            ));
        }
        self.write(game)
    }

    pub fn read(&self) -> Result<Game, ToolError> {
        let lock = self.lock()?;
        let game = self.load();
        drop(lock);
        game
    }

    /// Runs `f` on the stored game and saves the result only if `f` succeeds.
    pub fn update<T>(
        &self,
        f: impl FnOnce(&mut Game) -> Result<T, ToolError>,
    ) -> Result<T, ToolError> {
        let _lock = self.lock()?;
        let mut game = self.load()?;
        let out = f(&mut game)?;
        self.write(&game)?;
        Ok(out)
    }

    fn sibling(&self, suffix: &str) -> PathBuf {
        let mut path = self.path.clone().into_os_string();
        path.push(suffix);
        path.into()
    }

    fn lock(&self) -> Result<File, ToolError> {
        let lock_path = self.sibling(".lock");
        refuse_symlink(&self.path)?;
        refuse_symlink(&lock_path)?;
        let mut options = private_options();
        options.create(true).truncate(false);
        let file = options.open(lock_path).map_err(io_error)?;
        file.lock().map_err(io_error)?;
        Ok(file)
    }

    fn load(&self) -> Result<Game, ToolError> {
        let text = fs::read_to_string(&self.path).map_err(io_error)?;
        // serde messages can quote file contents, so report only the position.
        serde_json::from_str(&text).map_err(|e| {
            ToolError::new(
                "corrupt_game_file",
                format!(
                    "{} is not a valid saved game (line {}, column {})",
                    self.path.display(),
                    e.line(),
                    e.column()
                ),
            )
        })
    }

    /// Writes a new private temp file next to the game and renames it over the game.
    /// `create_new` never reuses or follows an existing path, so stale or planted temp
    /// files cannot change the result's permissions or redirect the write.
    fn write(&self, game: &Game) -> Result<(), ToolError> {
        let text =
            serde_json::to_string(game).map_err(|e| ToolError::new("serialize", e.to_string()))?;
        let (tmp, mut file) = self.create_temp()?;
        let result = file
            .write_all(text.as_bytes())
            .and_then(|()| file.sync_all())
            .and_then(|()| fs::rename(&tmp, &self.path));
        if let Err(e) = result {
            let _ = fs::remove_file(&tmp);
            return Err(io_error(e));
        }
        sync_parent_dir(&self.path);
        Ok(())
    }

    fn create_temp(&self) -> Result<(PathBuf, File), ToolError> {
        let mut options = private_options();
        options.create_new(true);
        for n in 0..100 {
            let tmp = self.sibling(&format!(".{}-{n}.tmp", std::process::id()));
            match options.open(&tmp) {
                Ok(file) => return Ok((tmp, file)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(io_error(e)),
            }
        }
        Err(ToolError::new(
            "io",
            "could not create a temporary file next to the game file",
        ))
    }
}

fn refuse_symlink(path: &Path) -> Result<(), ToolError> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(ToolError::new(
            "symlink",
            format!("refusing to use symlink {}", path.display()),
        )),
        _ => Ok(()),
    }
}

/// Makes the rename durable. Best effort: not every platform can open a directory.
fn sync_parent_dir(path: &Path) {
    #[cfg(unix)]
    {
        let dir = match path.parent() {
            Some(dir) if !dir.as_os_str().is_empty() => dir,
            _ => Path::new("."),
        };
        if let Ok(dir) = File::open(dir) {
            let _ = dir.sync_all();
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// Write-only options for files that only the owner may read (mode 0600 on Unix, applied
/// when the file is created).
fn private_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.write(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options
}

fn io_error(e: io::Error) -> ToolError {
    ToolError::new("io", e.to_string())
}

/// One seat of a shared game file, as exposed over MCP.
pub struct FileSeat {
    file: GameFile,
    seat: PlayerId,
}

impl FileSeat {
    pub fn open(file: GameFile, seat: u8) -> Result<Self, ToolError> {
        let seat = check_seat(&file.read()?, seat)?;
        Ok(Self { file, seat })
    }
}

impl Seat for FileSeat {
    fn observation(&mut self) -> Result<Value, ToolError> {
        Ok(observation(&self.file.read()?, Some(self.seat)))
    }

    fn legal_actions(&mut self) -> Result<Value, ToolError> {
        Ok(json!({"actions": legal_actions(&self.file.read()?, self.seat)}))
    }

    fn take_action(&mut self, action: Value) -> Result<Value, ToolError> {
        let seat = self.seat;
        self.file.update(|game| apply(game, seat, action))
    }

    fn status(&mut self) -> Result<Value, ToolError> {
        Ok(status(&self.file.read()?, Some(self.seat)))
    }
}
