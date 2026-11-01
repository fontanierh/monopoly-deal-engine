//! Plain-text view of one player's observation for hot-seat play.

use serde_json::Value;

/// Renders an observation produced by `engine::observation`, with numbered legal actions.
pub fn observation(obs: &Value) -> String {
    let name = |id: &Value| -> String {
        let def = &obs["cards"][id.to_string()];
        format!(
            "{} ({}M) #{id}",
            def["name"].as_str().unwrap_or("?"),
            def["value"]
        )
    };
    let names = |ids: &Value| -> String {
        let list: Vec<String> = ids.as_array().into_iter().flatten().map(name).collect();
        if list.is_empty() {
            "-".to_string()
        } else {
            list.join(", ")
        }
    };
    let mut out = String::new();
    let viewer = &obs["viewer"];
    out += &format!(
        "Turn {}. You are player {viewer}. Draw pile: {} cards.\n",
        obs["turn"], obs["draw_count"]
    );
    out += &format!("Phase: {}\n\n", obs["phase"]);
    for player in obs["players"].as_array().into_iter().flatten() {
        let you = if &player["id"] == viewer {
            " (you)"
        } else {
            ""
        };
        out += &format!(
            "Player {}{you}: {} cards in hand, bank {}M, {} complete sets\n",
            player["id"], player["hand_count"], player["bank_value"], player["complete_colors"]
        );
        out += &format!("  bank: {}\n", names(&player["bank"]));
        for set in player["properties"].as_array().into_iter().flatten() {
            let mut line = format!(
                "  set #{} {}: {}",
                set["id"],
                set["color"].as_str().unwrap_or("?"),
                names(&set["cards"])
            );
            for building in ["house", "hotel"] {
                if !set[building].is_null() {
                    line += &format!(" + {building}");
                }
            }
            let complete = if set["complete"] == true {
                ", complete"
            } else {
                ""
            };
            out += &format!("{line} (rent {}M{complete})\n", set["rent"]);
        }
    }
    out += &format!("\nYour hand: {}\n", names(&obs["hand"]));
    let log = obs["log"].as_array().map(Vec::as_slice).unwrap_or_default();
    out += "\nRecent events:\n";
    for event in &log[log.len().saturating_sub(8)..] {
        out += &format!("  {}\n", event);
    }
    out += "\nYour legal actions:\n";
    for (i, action) in obs["legal_actions"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let cards: Vec<String> = ["card", "take", "give"]
            .iter()
            .filter(|k| !action[**k].is_null())
            .map(|k| format!("{k}: {}", name(&action[*k])))
            .collect();
        out += &format!("  {i:>3}: {}  {}\n", action, cards.join("; "));
    }
    out
}
