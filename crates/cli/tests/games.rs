//! Plays whole games through the adapter the CLI and MCP server use, choosing actions by
//! their JSON form as an external agent would.

use monopoly_deal_cli::engine;
use monopoly_deal_core::PlayerId;
use serde_json::Value;

/// Every number stored under a card field anywhere in `value`.
fn mentioned_cards(value: &Value, out: &mut Vec<u64>) {
    match value {
        Value::Object(map) => {
            for (key, v) in map {
                let is_card_field = matches!(
                    key.as_str(),
                    "card"
                        | "take"
                        | "give"
                        | "doubles"
                        | "hand"
                        | "bank"
                        | "cards"
                        | "house"
                        | "hotel"
                        | "discard"
                );
                if is_card_field && key != "cards" || key == "cards" && v.is_array() {
                    match v {
                        Value::Number(n) => out.push(n.as_u64().unwrap()),
                        Value::Array(items) => out.extend(items.iter().filter_map(Value::as_u64)),
                        _ => {}
                    }
                }
                mentioned_cards(v, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|v| mentioned_cards(v, out)),
        _ => {}
    }
}

fn play(num_players: usize, seed: u64) {
    let mut game = engine::new_game(num_players, seed).unwrap();
    let mut rng = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    for step in 0..20_000 {
        if game.winner().is_some() {
            eprintln!("{num_players}p seed {seed}: turn {}", game.turn());
            return;
        }
        let seat: PlayerId = game.pending_actors()[0];
        let actions: Vec<Value> = engine::legal_actions(&game, seat)
            .iter()
            .map(|a| serde_json::to_value(a).unwrap())
            .collect();
        // Building observations is the slow part, so check every few steps.
        if step % 5 == 0 {
            let obs = engine::observation(&game, Some(seat));
            let mut mentioned = Vec::new();
            mentioned_cards(&obs, &mut mentioned);
            for id in mentioned {
                assert!(
                    obs["cards"].get(id.to_string()).is_some(),
                    "card {id} has no definition in {obs}"
                );
            }
            assert_eq!(obs["legal_actions"], Value::Array(actions.clone()));
        }
        assert!(
            !actions.is_empty(),
            "pending actor {seat:?} has no legal action"
        );
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let roll = (rng >> 33) as usize;
        // Mostly take actions that use up a card or a play, so games progress. Free
        // property moves and ending the turn early still happen now and then.
        let progressing: Vec<&Value> = actions
            .iter()
            .filter(|a| a["type"] != "move_property" && a["type"] != "end_turn")
            .collect();
        let pick = if progressing.is_empty() || roll.is_multiple_of(10) {
            &actions[roll / 10 % actions.len()]
        } else {
            progressing[roll / 10 % progressing.len()]
        };
        engine::apply(&mut game, seat, pick.clone())
            .unwrap_or_else(|e| panic!("legal action {pick} rejected: {e:?}"));
    }
    panic!("game with {num_players} players and seed {seed} did not finish");
}

#[test]
fn random_games_finish_through_the_json_interface() {
    for num_players in [2, 3, 6, 9] {
        for seed in 0..2 {
            play(num_players, seed);
        }
    }
}
