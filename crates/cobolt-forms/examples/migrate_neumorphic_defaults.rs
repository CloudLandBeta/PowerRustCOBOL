// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Bring existing **Neumorphic Light and Neumorphic Dark** forms to the four
//! defaults the operator set on 2026-09-02: corner radius 10, no border, drop
//! shadow on everywhere except a Label, and a Label transparent with a
//! high-contrast ink.
//!
//! The registers differ in exactly one place — the ink a Label ends up with.
//! Light remaps the `#FFFFFF` "not chosen" sentinel to black, because white on
//! its pale ground is invisible. Dark *forces* white on every control and white
//! is right there (11.7:1 on `#36383E`), so the same value must be left alone.
//! Remapping by value rather than by register would blank every dark caption.
//!
//! Deliberately **surgical**, not a re-stamp. Calling
//! `Form::apply_neumorphic_defaults` would also rewrite every background,
//! gradient and shadow property on every control — correct for a style the
//! developer is switching *to*, destructive for forms that already exist and
//! carry colours somebody chose. Only the four properties that changed are
//! touched, and a Label's foreground only while it still holds the `#FFFFFF`
//! "not chosen" sentinel.
//!
//! Reads and writes through `load_form`/`save_form`, so a `.cfrm` is
//! round-tripped by the real serializer rather than edited as text.
//!
//! ```text
//! cargo run -p cobolt-forms --example migrate_neumorphic_defaults -- <dir> [--dry-run]
//!                                                                       [--verify]
//!                                                                       [--roundtrip-only]
//! ```
//!
//! **Run `--verify` before trusting it with files that are not under version
//! control.** The serializer omits properties whose value is the empty string,
//! so a no-op save changes the FILE (~2 KB smaller) even where it changes
//! nothing semantically — on PowerDemo3 that was 28 of 44 forms. `--verify`
//! proves the equivalence (load → serialize → load again, compared through the
//! derived `Debug`) instead of assuming it; `--roundtrip-only` writes the
//! no-op so the two trees can be diffed by hand.
//!
//! Idempotent: a second run reports nothing to do.

use cobolt_forms::model::{
    ControlType, GlassStyle, PropValue, NEUMORPHIC_CORNER_RADIUS, TRANSPARENT_COLOR,
};
use cobolt_forms::{form_to_string, load_form, load_form_from_str, save_form};
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let roundtrip_only = args.iter().any(|a| a == "--roundtrip-only");
    let verify = args.iter().any(|a| a == "--verify");
    let Some(root) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("usage: migrate_neumorphic_light <dir> [--dry-run|--verify|--roundtrip-only]");
        std::process::exit(2);
    };

    let mut forms = Vec::new();
    collect(Path::new(root), &mut forms);
    forms.sort();

    let (mut touched, mut skipped, mut controls_changed) = (0usize, 0usize, 0usize);
    for path in &forms {
        let mut form = match load_form(path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("SKIP (unreadable) {}: {e}", path.display());
                continue;
            }
        };

        if verify {
            let once = format!("{form:?}");
            let text = match form_to_string(&form) {
                Ok(t) => t,
                Err(e) => {
                    println!("SERIALIZE FAILED {}: {e}", path.display());
                    continue;
                }
            };
            match load_form_from_str(&text) {
                Ok(again) if format!("{again:?}") == once => {}
                Ok(_) => println!("NOT IDENTICAL after a round trip: {}", path.display()),
                Err(e) => println!("RELOAD FAILED {}: {e}", path.display()),
            }
            continue;
        }
        if roundtrip_only {
            if let Err(e) = save_form(&form, path) {
                eprintln!("FAILED to write {}: {e}", path.display());
                std::process::exit(1);
            }
            continue;
        }
        // Every form gets the corner-radius floor and the stale-stamp sweep;
        // only the neumorphic ones get the register's own four defaults.
        let dark = match form.glass_style {
            GlassStyle::Neumorphic => Some(false),
            GlassStyle::NeumorphicDark => Some(true),
            _ => None,
        };
        if dark.is_none() {
            skipped += 1;
        }

        let mut changed = 0usize;
        for ctrl in &mut form.controls {
            let is_label = matches!(ctrl.control_type, ControlType::Label);
            let mut did = Vec::new();

            // The corner-radius FLOOR, on every form whatever its style
            // (operator, 2026-09-02): anything under 10 becomes 10. A larger
            // radius somebody chose deliberately is left alone — this is a
            // floor, not an assignment.
            let radius = ctrl.get_prop("CornerRadius").map(|v| v.as_i64());
            if radius.unwrap_or(0) < NEUMORPHIC_CORNER_RADIUS {
                did.push(format!("CornerRadius {radius:?}->{NEUMORPHIC_CORNER_RADIUS}"));
                ctrl.set_prop("CornerRadius", PropValue::Int(NEUMORPHIC_CORNER_RADIUS));
            }

            // A STALE INK STAMP. `#000000` paired with the background sentinel
            // is what a previous Neumorphic style left behind: the theme
            // repaints the background (the sentinel is recognised) but not the
            // foreground (black is not the foreground sentinel), so black ink
            // lands on a dark themed field. Restoring the foreground sentinel
            // hands the decision back to the theme.
            //
            // Only that exact PAIR is touched. Black on a background the
            // developer actually chose is a decision, and stays.
            if dark.is_none() {
                let fg = ctrl
                    .get_prop("ForegroundColor")
                    .map(|v| v.as_str().trim().to_ascii_uppercase())
                    .unwrap_or_default();
                let bg = ctrl
                    .get_prop("BackgroundColor")
                    .map(|v| v.as_str().trim().to_ascii_uppercase())
                    .unwrap_or_default();
                let bg_is_sentinel = bg.trim_start_matches('#').starts_with("F0F0F0");
                if bg_is_sentinel && matches!(fg.as_str(), "#000000" | "#000000FF") {
                    did.push("ForegroundColor stale black -> theme".into());
                    ctrl.set_prop(
                        "ForegroundColor",
                        PropValue::String(cobolt_forms::model::DEFAULT_FOREGROUND_COLOR.into()),
                    );
                }
            }

            let Some(dark) = dark else {
                if !did.is_empty() {
                    changed += 1;
                    println!("    {} [{:?}] {}", ctrl.id, ctrl.control_type, did.join(", "));
                }
                continue;
            };

            let border = ctrl.get_prop("BorderStyle").map(|v| v.as_str().to_owned());
            if border.as_deref() != Some("None") {
                did.push(format!("BorderStyle {border:?}->None"));
                ctrl.set_prop("BorderStyle", PropValue::String("None".into()));
            }

            let shadow = ctrl.get_prop("ShadowEnabled").map(|v| v.as_bool());
            if shadow != Some(!is_label) {
                did.push(format!("ShadowEnabled {shadow:?}->{}", !is_label));
                ctrl.set_prop("ShadowEnabled", PropValue::Bool(!is_label));
            }

            if is_label {
                let bg = ctrl.get_prop("BackgroundColor").map(|v| v.as_str().to_owned());
                if bg.as_deref() != Some(TRANSPARENT_COLOR) {
                    did.push(format!("BackgroundColor {bg:?}->transparent"));
                    ctrl.set_prop(
                        "BackgroundColor",
                        PropValue::String(TRANSPARENT_COLOR.into()),
                    );
                }
                if ctrl
                    .get_prop("BackgroundGradientEnabled")
                    .map(|v| v.as_bool())
                    != Some(false)
                {
                    did.push("BackgroundGradientEnabled->false".into());
                    ctrl.set_prop("BackgroundGradientEnabled", PropValue::Bool(false));
                }
                // Light only: the `#FFFFFF` "not chosen" sentinel becomes black,
                // because white on the pale ground is invisible. A colour the
                // developer picked is theirs and survives either way.
                //
                // Under DARK the very same value is the correct, deliberate ink
                // — remapping it by value would blank every caption in the form.
                if !dark {
                    let fg = ctrl
                        .get_prop("ForegroundColor")
                        .map(|v| v.as_str().trim().trim_start_matches('#').to_owned())
                        .unwrap_or_default();
                    if fg.eq_ignore_ascii_case("FFFFFF") || fg.eq_ignore_ascii_case("FFFFFFFF") {
                        did.push("ForegroundColor sentinel->#000000".into());
                        ctrl.set_prop("ForegroundColor", PropValue::String("#000000".into()));
                    }
                }
            }

            if !did.is_empty() {
                changed += 1;
                println!("    {} [{:?}] {}", ctrl.id, ctrl.control_type, did.join(", "));
            }
        }

        if changed == 0 {
            continue;
        }
        println!("  {} — {changed} control(s)", path.display());
        touched += 1;
        controls_changed += changed;
        if !dry_run {
            if let Err(e) = save_form(&form, path) {
                eprintln!("FAILED to write {}: {e}", path.display());
                std::process::exit(1);
            }
        }
    }

    if !verify && !roundtrip_only {
        println!(
            "\n{} form(s) scanned · {touched} neumorphic form(s) updated \
             · {controls_changed} control(s) changed · {skipped} form(s) on another style{}",
            forms.len(),
            if dry_run { "  [DRY RUN — nothing written]" } else { "" }
        );
    }
}

/// Every `.cfrm` under `dir`, recursively. Uses `read_dir` rather than a shell
/// glob on purpose: `PowerDemo3/forms/Menus & Bars/` contains both a space and
/// an ampersand, and an unquoted shell expansion silently drops those forms.
fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "cfrm") {
            out.push(p);
        }
    }
}
