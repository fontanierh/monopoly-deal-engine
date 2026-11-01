//! Adapter from `monopoly_deal_core::Game` to the server's `Engine` trait.

use monopoly_deal_core::{card_def, Action, Game, GameConfig, PlayerId};
use serde_json::{Map, Value};

use crate::engine::Engine;

pub struct CoreGame(Game);

impl CoreGame {
    pub fn new(num_players: usize, seed: u64) -> Result<Self, String> {
        Game::new(GameConfig::new(num_players, seed))
            .map(Self)
            .map_err(|e| e.to_string())
    }
}

/// Seats are always below the player count, which core keeps within `u8`.
fn player(seat: usize) -> PlayerId {
    PlayerId(seat as u8)
}

impl Engine for CoreGame {
    type Action = Action;

    fn num_seats(&self) -> usize {
        self.0.num_players()
    }

    fn card_catalog(&self) -> Value {
        let cards: Map<String, Value> = self
            .0
            .card_ids()
            .map(|id| {
                (
                    id.0.to_string(),
                    serde_json::to_value(card_def(id)).expect("card defs serialize"),
                )
            })
            .collect();
        Value::Object(cards)
    }

    fn observation(&self, viewer: Option<usize>) -> Value {
        serde_json::to_value(self.0.observation(viewer.map(player)))
            .expect("observations serialize")
    }

    fn legal_actions(&self, seat: usize) -> Vec<Action> {
        self.0.legal_actions(player(seat))
    }

    fn apply(&mut self, seat: usize, action: Action) -> Result<(), String> {
        self.0
            .apply(player(seat), action)
            .map(drop)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_hide_other_hands() {
        let g = CoreGame::new(3, 7).unwrap();
        let hand = |v: Option<usize>| g.observation(v)["hand"].as_array().unwrap().len();
        assert_eq!(hand(None), 0);
        assert_eq!(hand(Some(1)), g.0.observation(Some(PlayerId(1))).hand.len());
        assert!(hand(Some(1)) > 0);
        assert_eq!(
            g.card_catalog().as_object().unwrap().len(),
            g.0.card_ids().count()
        );
    }

    #[test]
    fn bad_player_count_is_an_error() {
        assert!(CoreGame::new(1, 0).is_err());
    }
}
