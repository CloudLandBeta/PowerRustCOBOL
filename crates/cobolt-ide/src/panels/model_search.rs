//! The search box at the top of a model combobox's list.
//!
//! A provider like OpenRouter offers hundreds of models; scrolling through
//! them one by one is not a way to find one. Shared by the Model Leaderboard's
//! "add a model" row and the Agents table's Model column, so both search alike.
//!
//! A combobox using it must be opened with
//! `.close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)` — a click
//! into the search box would otherwise close the list — and close it by hand
//! with `ui.close()` when a model is picked.

/// The models whose id contains every word of `query`, case-insensitively.
///
/// Words, not one substring: "gpt mini" finds `openai/gpt-5-mini` though the
/// two words are not adjacent in the id.
pub fn filter_models<'a>(models: &'a [String], query: &str) -> Vec<&'a String> {
    let words: Vec<String> = query
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .collect();
    models
        .iter()
        .filter(|m| {
            let id = m.to_lowercase();
            words.iter().all(|w| id.contains(w.as_str()))
        })
        .collect()
}

/// The search box and the "shown / total" count, drawn inside the open list.
/// Returns `true` when Enter was pressed in the box — the caller then takes
/// the first match, so a precise enough search needs no mouse at all.
pub fn search_field(
    ui: &mut egui::Ui,
    filter: &mut String,
    hint: &str,
    shown: usize,
    total: usize,
) -> bool {
    let search = ui.add(
        egui::TextEdit::singleline(filter)
            .hint_text(hint)
            .desired_width(f32::INFINITY),
    );
    // Typing starts at once — no click into the box first.
    if !search.has_focus() && !ui.memory(|m| m.focused().is_some()) {
        search.request_focus();
    }
    ui.weak(format!("{shown} / {total}"));
    ui.separator();
    search.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
}

/// The tallest the list of matches grows before it scrolls.
pub const LIST_MAX_H: f32 = 320.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_search_matches_every_word_in_any_order() {
        let models: Vec<String> = [
            "openai/gpt-5-mini",
            "openai/gpt-5.6-terra",
            "anthropic/claude-opus-5.5",
            "meta-llama/llama-4-maverick",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let ids = |q: &str| -> Vec<&str> {
            filter_models(&models, q).into_iter().map(|s| s.as_str()).collect()
        };
        assert_eq!(ids("").len(), 4);
        assert_eq!(ids("   ").len(), 4);
        assert_eq!(ids("mini gpt"), vec!["openai/gpt-5-mini"]);
        assert_eq!(ids("CLAUDE"), vec!["anthropic/claude-opus-5.5"]);
        assert_eq!(ids("openai"), vec!["openai/gpt-5-mini", "openai/gpt-5.6-terra"]);
        assert!(ids("nothing-like-this").is_empty());
    }
}
