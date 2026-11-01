//! Rules engine for the 2008 Monopoly Deal card game. See `RULES.md` in this crate for edge-case defaults.

mod cards;
mod game;

pub use cards::{
    card_def, ActionKind, CardDef, CardId, CardKind, Color, ALL_COLORS, PACK, PACK_SIZE,
};
pub use game::{
    Action, ActionError, Effect, Event, Game, GameConfig, GameError, Observation, Phase, PlayerId,
    PropertySet, PublicPlayer, PublicSet, SetId, HAND_DEAL, HAND_LIMIT, MAX_PLAYERS, MIN_PLAYERS,
    PLAYS_PER_TURN, SETS_TO_WIN,
};
