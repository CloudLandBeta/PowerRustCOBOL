// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Host diagnostics, shared by every form host (spec 042 R27/R28).
//!
//! Both diagnostic families live here so no host is poorer than the other:
//! the **file dump** written once at launch (historically run-form only) and
//! the **live trace** — the launch preamble plus a per-state-update line with
//! "NO SUCH CONTROL" reporting (historically compiled-application only; it is
//! what turned "the label never changed" into "that control id does not
//! exist" during the 1.60.32–1.60.34 investigation).

/// `true` when the named env var holds a truthy value (`1`/`true`/`on`,
/// case-insensitive) — the ONE truthiness rule for every diagnostic flag in
/// every host (R28). Presence alone is not enough: the IDE always sets these
/// vars (to `0` when the matching switch is off), so the value must be
/// inspected.
pub fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|v| {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on")
        })
        .unwrap_or(false)
}

/// The truthiness rule alone, for values already read from the environment —
/// unit-testable without mutating process env.
pub fn is_truthy(value: &str) -> bool {
    let v = value.trim();
    v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on")
}

/// `COBOLT_FRAME_DIAGNOSTICS` — the live per-frame trace switch.
pub fn frame_diagnostics_enabled() -> bool {
    env_flag("COBOLT_FRAME_DIAGNOSTICS")
}

/// `COBOLT_DATABIND_TRACE` — the render-side data-binding dump switch.
pub fn databind_trace_enabled() -> bool {
    env_flag("COBOLT_DATABIND_TRACE")
}

/// `COBOLT_EVENT_TRACE`, from the crate the runtime shares — see
/// [`cobolt_forms::diagnostics::trace_event`]. Re-exported so this module stays
/// the one place a host looks for a diagnostic switch.
pub use cobolt_forms::diagnostics::{event_trace_enabled, trace_event};

/// With diagnostics on, say what this window IS before anything runs: which
/// form, its controls and its background. The control roster is the useful
/// part — it turns "the label never changed" into "that control id does not
/// exist" at a glance.
///
/// (There was a transparency warning here once. It was wrong — a transparent
/// form renders its controls perfectly well; the real fault was a duplicated
/// property key, 1.60.33 — and it did nothing but send readers down the wrong
/// path. Dropped in 1.60.34; do not bring it back.)
pub fn launch_preamble(form: &cobolt_forms::Form, control_ids: &[&str]) {
    eprintln!(
        "[prc] form '{}' {}x{} background={:?} controls={:?}",
        form.name, form.width, form.height, form.background_color, control_ids
    );
}

/// The same preamble for a form loaded into the shell's **ContentPane** (049).
///
/// `launch_preamble` covers the ROOT form only, so a pane occupant — which is
/// what an `Embedded` form always is — went through the whole build with
/// nothing said about it. When an embedded form renders differently from the
/// way it looks in the designer, the first question is whether its controls
/// even reached the host, and that question had no answer short of a debugger
/// (operator, 2026-08-31: radios missing from an embedded form).
///
/// Per control: its id, type, designed rect and `visible` flag — enough to
/// tell "the control never arrived" from "it arrived and something hid it"
/// from "it arrived, is visible, and is being drawn somewhere unexpected".
pub fn embedded_preamble(
    handle: &str,
    form: &cobolt_forms::Form,
    controls: &[cobolt_forms::Control],
) {
    eprintln!(
        "[prc] EMBEDDED form '{}' handle={} {}x{} format={} background={:?}          bg-image={:?} bg-mode={} controls={}",
        form.name,
        handle,
        form.width,
        form.height,
        form.form_format.as_str(),
        form.background_color,
        form.background_image,
        form.bg_image_mode.as_str(),
        controls.len(),
    );
    for c in controls {
        eprintln!(
            "[prc]   {} {} rect=({},{},{}x{}) visible={} enabled={} parent={:?}",
            c.id,
            c.control_type.as_str(),
            c.rect.x,
            c.rect.y,
            c.rect.w,
            c.rect.h,
            c.visible,
            c.enabled,
            c.parent,
        );
    }
}

/// One live-trace line per state update: which designed control the write
/// landed on, or `NO SUCH CONTROL` with the ids that do exist. A miss means
/// the update is stored under a key nothing renders — the difference between
/// "the handler never ran" and "the handler wrote to a name that does not
/// exist", which is invisible without saying so.
pub fn trace_state_update(
    ctrl_id: &str,
    prop: &str,
    value: &str,
    matched: Option<&str>,
    known: &[&str],
) {
    match matched {
        Some(k) => eprintln!(
            "[prc] state update: {} :: {} = {:?} -> control '{}'",
            ctrl_id, prop, value, k
        ),
        None => eprintln!(
            "[prc] state update: {} :: {} = {:?} -> NO SUCH CONTROL \
             (known: {:?}) — this write will never be seen",
            ctrl_id, prop, value, known
        ),
    }
}

/// Where every control was ACTUALLY drawn, once, after the entrance effect.
///
/// `embedded_preamble` answers "did the control reach the host, and is it
/// visible" — and for a form whose controls arrive correct and still do not
/// appear, that is the wrong half of the question. Its own doc names three
/// cases; this is the third, which had no instrument: *it arrived, is visible,
/// and is being drawn somewhere unexpected* (operator, 2026-09-01: radios and a
/// Label's face missing from an embedded form whose controls all arrive with
/// `visible=true` at their designed rects).
///
/// Printed ONCE, after the entrance effect settles, because that is when the
/// operator sees the loss — during the effect the controls are not drawn at all
/// and every rect would be absent for a reason that is not the bug.
///
/// `surface` is the rect the occupant was given, so a control drawn outside it
/// is visible as such in the log rather than having to be inferred: a rect that
/// lies beyond the surface reads as "painted off-surface", which looks exactly
/// like "never painted" on screen.
pub fn drawn_rects(
    handle: &str,
    surface: egui::Rect,
    designed: &[cobolt_forms::Control],
    drawn: &std::collections::HashMap<String, egui::Rect>,
) {
    eprintln!(
        "[prc] DRAWN handle={} surface=({:.0},{:.0} {:.0}x{:.0}) — {} of {} controls placed",
        handle,
        surface.min.x,
        surface.min.y,
        surface.width(),
        surface.height(),
        drawn.len(),
        designed.len()
    );
    for c in designed {
        match drawn.get(&c.id) {
            Some(r) => {
                let inside = surface.contains_rect(*r);
                eprintln!(
                    "[prc]   {} {} drawn=({:.0},{:.0} {:.0}x{:.0}) inside_surface={}{}",
                    c.id,
                    c.control_type.as_str(),
                    r.min.x,
                    r.min.y,
                    r.width(),
                    r.height(),
                    inside,
                    if inside { "" } else { "   <<< OFF-SURFACE" }
                );
            }
            None => eprintln!(
                "[prc]   {} {} *** NOT DRAWN AT ALL *** (designed at {},{} {}x{})",
                c.id, c.control_type.as_str(), c.rect.x, c.rect.y, c.rect.w, c.rect.h
            ),
        }
    }
}

/// Sanitize a project name into a safe file stem (no path separators / oddities).
pub fn sanitize_stem(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = s.trim_matches('_');
    if trimmed.is_empty() {
        "project".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Write the per-control diagnostics dump to
/// `<diagnostics dir>/<project>_diagnostics_dump.log` — `/tmp` on Linux/macOS
/// (deliberately not the per-process /var/folders path `std::env::temp_dir`
/// gives on macOS), `%TEMP%` on Windows, which has no /tmp. Called once at
/// launch when any diagnostic is enabled. Best-effort: a failure to write is
/// reported on stderr but never blocks the form from running.
pub fn write_diagnostics_dump(project: &str, form: &cobolt_forms::Form) {
    let enabled = [
        ("frame_diagnostics", env_flag("COBOLT_FRAME_DIAGNOSTICS")),
        ("datagrid_diagnostics", env_flag("COBOLT_DATAGRID_DIAGNOSTICS")),
        ("databind_trace", env_flag("COBOLT_DATABIND_TRACE")),
    ];
    let body = cobolt_forms::diagnostics::dump_form_diagnostics(form, project, &enabled);
    let path = cobolt_runtime::diag_path::diagnostics_file(&format!(
        "{}_diagnostics_dump.log",
        sanitize_stem(project)
    ));
    match std::fs::write(&path, body) {
        Ok(()) => eprintln!("run-form: wrote diagnostics dump to {}", path.display()),
        Err(e) => eprintln!(
            "run-form: could not write diagnostics dump to {}: {e}",
            path.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R28 — one truthiness rule: `1`/`true`/`on` (any case, padded) are true;
    /// `0`, empty, `off`, `yes`, junk are false. Host B used to accept only
    /// `"1"` in one read and `1`/`true` in another.
    #[test]
    fn one_truthiness_rule_for_every_flag() {
        for yes in ["1", "true", "TRUE", "on", "On", " 1 ", "\ttrue\n"] {
            assert!(is_truthy(yes), "{yes:?} must be truthy");
        }
        for no in ["", "0", "false", "off", "yes", "2", "enabled"] {
            assert!(!is_truthy(no), "{no:?} must be falsy");
        }
    }

    /// Stems never contain path oddities and never come out empty.
    #[test]
    fn stems_are_safe_and_never_empty() {
        assert_eq!(sanitize_stem("My Project/2026"), "My_Project_2026");
        assert_eq!(sanitize_stem("///"), "project");
        assert_eq!(sanitize_stem("__x__"), "x");
    }
}
