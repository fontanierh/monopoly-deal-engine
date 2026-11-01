//! Local-only browser front end for monopoly-deal-engine.
//!
//! The server binds to loopback, holds the only copy of the game, validates
//! every action through the engine, and gives each seat a separate random
//! credential. Browsers only ever receive per-viewer observations.

pub mod auth;
pub mod core_engine;
pub mod engine;
pub mod server;
