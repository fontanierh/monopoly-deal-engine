//! The narrow surface the web server needs from a game engine.
//!
//! The server never serializes full game state: it only asks the engine for a
//! per-viewer observation, so hidden-information redaction stays in core.

use serde::{de::DeserializeOwned, Serialize};

pub trait Engine: Send + 'static {
    type Action: Serialize + DeserializeOwned + Send;

    fn num_seats(&self) -> usize;

    /// What `viewer` may see. `None` is a spectator with public information only.
    fn observation(&self, viewer: Option<usize>) -> serde_json::Value;

    /// Public definitions of every card that can appear in this game, keyed by
    /// card id. Must not depend on the shuffle.
    fn card_catalog(&self) -> serde_json::Value;

    /// Every action `seat` may take now; empty when it has no decision.
    fn legal_actions(&self, seat: usize) -> Vec<Self::Action>;

    /// Validate and apply. On error the state must be unchanged.
    fn apply(&mut self, seat: usize, action: Self::Action) -> Result<(), String>;
}
