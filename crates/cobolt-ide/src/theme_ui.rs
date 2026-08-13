// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Form-theme picker choices, shared between the project Settings form and the
//! per-form Appearance pane (spec 007).
//!
//! The app discovers the asset packs once and **publishes** the catalog
//! (`liquid-glass` + discovered packs, as `(id, display_name)` pairs) into egui's
//! per-frame temp storage. Both pickers read it from there, so the discovered
//! "special" themes surface automatically with no extra `show()` parameters.

/// One selectable theme, as the pickers need it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeChoice {
    pub id: String,
    pub display_name: String,
    /// 050 R17 — does this theme own the whole look? The Glass style row is
    /// disabled for one that does, because the setting would do nothing.
    pub self_contained: bool,
}

/// The catalogue for this frame, Liquid Glass first, plus the project default
/// so a per-form picker can say what an unset override actually resolves to.
#[derive(Clone, Default)]
pub struct FormThemeChoices {
    pub themes: Vec<ThemeChoice>,
    /// The project's `[forms] theme`, if it set one (050 R19).
    pub project_default: Option<String>,
}

fn id() -> egui::Id {
    egui::Id::new("cobolt-form-theme-choices")
}

/// Publish the available form themes — and the project default — for this frame.
pub fn publish(ctx: &egui::Context, themes: Vec<ThemeChoice>, project_default: Option<String>) {
    ctx.data_mut(|d| {
        d.insert_temp(
            id(),
            FormThemeChoices {
                themes,
                project_default,
            },
        )
    });
}

/// Read the published catalogue; falls back to just Liquid Glass if the app has
/// not published yet this frame.
pub fn choices(ctx: &egui::Context) -> Vec<ThemeChoice> {
    published(ctx).themes
}

/// The project's default theme id for this frame, if it set one.
pub fn project_default(ctx: &egui::Context) -> Option<String> {
    published(ctx).project_default
}

/// Does `theme_id` own the whole look? Unknown ids answer `false`, which is the
/// safe direction: the Glass style row stays usable.
pub fn is_self_contained(ctx: &egui::Context, theme_id: &str) -> bool {
    choices(ctx)
        .iter()
        .find(|c| c.id == theme_id)
        .is_some_and(|c| c.self_contained)
}

fn published(ctx: &egui::Context) -> FormThemeChoices {
    ctx.data(|d| d.get_temp::<FormThemeChoices>(id()))
        .filter(|c| !c.themes.is_empty())
        .unwrap_or_else(|| FormThemeChoices {
            themes: vec![ThemeChoice {
                id: cobolt_forms::theme::LIQUID_GLASS.to_owned(),
                display_name: "Liquid Glass".to_owned(),
                self_contained: false,
            }],
            project_default: None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_with(project_default: Option<&str>) -> egui::Context {
        let ctx = egui::Context::default();
        let themes = cobolt_forms::theme::ThemeCatalog::builtin()
            .themes()
            .iter()
            .map(|t| ThemeChoice {
                id: t.id.clone(),
                display_name: t.display_name.clone(),
                self_contained: t.self_contained,
            })
            .collect();
        publish(&ctx, themes, project_default.map(|s| s.to_owned()));
        ctx
    }

    /// 050 AC11/R19 — a form with no override of its own reports the theme it
    /// actually renders with.
    ///
    /// The per-form picker resolved against `None` instead of the project
    /// default, so a form inheriting a themed project displayed "Liquid Glass"
    /// while painting as something else entirely.
    #[test]
    fn a_form_shows_the_theme_it_inherits() {
        use cobolt_forms::theme::{resolve_theme_id, ELEGANCE, LIQUID_GLASS};
        let ctx = ctx_with(Some(ELEGANCE));
        let proj = project_default(&ctx);
        assert_eq!(proj.as_deref(), Some(ELEGANCE));

        let mut rows = Vec::new();
        for (form_override, want) in [
            (None, ELEGANCE),              // inherits the project
            (Some(""), ELEGANCE),          // an empty override IS "inherit"
            (Some(LIQUID_GLASS), LIQUID_GLASS), // an explicit override wins
            (Some(ELEGANCE), ELEGANCE),
        ] {
            let got = resolve_theme_id(form_override, proj.as_deref());
            assert_eq!(got, want, "override {form_override:?}");
            let inherited = form_override.unwrap_or("").trim().is_empty() && proj.is_some();
            rows.push((format!("{form_override:?}"), got, inherited));
        }

        // With no project default the old behaviour is unchanged.
        let bare = ctx_with(None);
        assert_eq!(project_default(&bare), None);
        assert_eq!(resolve_theme_id(None, None), LIQUID_GLASS);

        println!("\n  050 AC11 — project default = {ELEGANCE}");
        println!("  {:<18} {:<14} shown as inherited", "form override", "resolves to");
        for (o, r, inh) in &rows {
            println!("  {o:<18} {r:<14} {inh}");
        }
        println!();
    }

    /// 050 — every picker offers the THEME CATALOGUE, never the glass styles.
    ///
    /// The New Form dialog had a row labelled "Theme" that listed
    /// `Classic / Enhanced / Neumorphic Light / Neumorphic Dark` — the four
    /// GLASS STYLES. So a real theme could not be chosen when the form was
    /// created, and the two settings looked like one control. This is the same
    /// conflation the form inspector had, in a second place.
    #[test]
    fn every_picker_offers_the_theme_catalogue_not_the_glass_styles() {
        use cobolt_forms::theme::{ThemeCatalog, ELEGANCE, LIQUID_GLASS};
        let ctx = ctx_with(None);
        let offered = choices(&ctx);
        let ids: Vec<&str> = offered.iter().map(|c| c.id.as_str()).collect();

        // Every built-in catalogue entry is selectable.
        for t in ThemeCatalog::builtin().themes() {
            assert!(
                ids.contains(&t.id.as_str()),
                "the picker does not offer {:?} — a shipped theme that cannot \
                 be chosen is not shipped",
                t.id
            );
        }
        assert!(ids.contains(&LIQUID_GLASS) && ids.contains(&ELEGANCE));

        // And no glass style is masquerading as a theme.
        const GLASS_STYLES: [&str; 4] =
            ["Classic", "Enhanced", "Neumorphic Light", "Neumorphic Dark"];
        for c in &offered {
            for gs in GLASS_STYLES {
                assert_ne!(
                    c.display_name, gs,
                    "{gs:?} is a GLASS STYLE, not a theme — it belongs on its \
                     own row"
                );
            }
        }

        println!(
            "\n  050 — theme pickers offer {} catalogue entries {:?}; none of \
             the 4 glass styles appears among them\n",
            offered.len(),
            ids
        );
    }

    /// 050 §5 risk — the IDE must never install a theme's widget visuals.
    ///
    /// `install_widget_visuals` mutates egui's **global** style. The IDE drives
    /// every form window through `show_viewport_immediate`, so one Context
    /// serves the whole application: calling it here would restyle the IDE's own
    /// panels, toolbars and editor around the canvas. It is host-only, and
    /// making it a trait method makes it look callable from anywhere — hence
    /// this guard.
    #[test]
    fn the_ide_never_installs_widget_visuals() {
        const FILES: [(&str, &str); 4] = [
            ("theme_ui.rs", include_str!("theme_ui.rs")),
            ("app.rs", include_str!("app.rs")),
            ("panels/designer.rs", include_str!("panels/designer.rs")),
            ("panels/properties.rs", include_str!("panels/properties.rs")),
        ];
        let mut hits = Vec::new();
        for (name, src) in FILES {
            let lines: Vec<&str> = src.lines().collect();
            // The scanner's own source is data, not a call.
            let scanner_start = lines
                .iter()
                .position(|l| l.contains("fn the_ide_never_installs_widget_visuals"))
                .unwrap_or(lines.len());
            for (i, line) in lines.iter().enumerate() {
                if i >= scanner_start {
                    continue;
                }
                let t = line.trim();
                if t.contains("install_widget_visuals(")
                    && !t.starts_with("//")
                    && !t.starts_with("///")
                {
                    hits.push(format!("{name}:{}", i + 1));
                }
            }
        }
        assert!(
            hits.is_empty(),
            "the IDE shares one egui Context with every form window — installing \
             a theme's global widget style here restyles the IDE itself: {hits:?}"
        );
        println!(
            "\n  050 — {} IDE files scanned, 0 calls to install_widget_visuals \
             (host-only, by design)\n",
            FILES.len()
        );
    }

    /// 050 AC10/R17 — the Glass style row is disabled exactly when the resolved
    /// theme owns the whole look.
    #[test]
    fn the_glass_row_is_disabled_under_a_self_contained_theme() {
        use cobolt_forms::theme::{ELEGANCE, LIQUID_GLASS};
        let ctx = ctx_with(None);
        assert!(
            is_self_contained(&ctx, ELEGANCE),
            "Elegance owns the look ⇒ the row is disabled"
        );
        assert!(
            !is_self_contained(&ctx, LIQUID_GLASS),
            "Liquid Glass IS the glass configuration ⇒ the row stays usable"
        );
        assert!(
            !is_self_contained(&ctx, "no-such-theme"),
            "an unknown id must not disable the row"
        );
        println!(
            "\n  050 AC10 — glass row: {LIQUID_GLASS} enabled, {ELEGANCE} \
             disabled (+ hint), unknown id enabled\n"
        );
    }
}

/// The display name for a theme id (falls back to the id, then Liquid Glass).
pub fn display_name(ctx: &egui::Context, theme_id: &str) -> String {
    let all = choices(ctx);
    all.iter()
        .find(|c| c.id == theme_id)
        .map(|c| c.display_name.clone())
        .unwrap_or_else(|| {
            if theme_id.is_empty() {
                "Liquid Glass".to_owned()
            } else {
                theme_id.to_owned()
            }
        })
}
