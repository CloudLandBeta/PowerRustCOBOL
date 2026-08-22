// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Per-control diagnostics dump.
//!
//! When any diagnostic setting is enabled, the running form writes a detailed,
//! human-readable record of the whole form — every control, its geometry, all of
//! its properties, its events, animations, data bindings, and (for a DataGrid)
//! its advanced column configuration — to
//! `/tmp/<project>_diagnostics_dump.log`. It is meant to be opened alongside the
//! frame / component overlays so a problem seen on screen can be traced back to
//! the exact control state that produced it.
//!
//! Deliberately egui-free: it works off the [`Form`] model alone, so the dump is
//! identical whether produced by the IDE preview or a standalone `rcrun run-form`.

use std::fmt::Write as _;

use crate::model::{DataGridAdvanced, DATAGRID_ADVANCED_PROP};
use crate::{Control, Form};

/// `COBOLT_EVENT_TRACE` — one line per event, at BOTH ends of the channel.
///
/// A handler that runs twice for one click is either an event SENT twice or one
/// event DISPATCHED twice, and from outside the process those look identical:
/// the program simply prints its `DISPLAY` twice (operator, 2026-08-21). The
/// host logs `send`, the interpreter logs `dispatch`, and pairing them tells the
/// two apart in a single run — two `send` lines means the host is duplicating;
/// one `send` with two `dispatch` lines means the interpreter is.
///
/// It lives here, in the crate the host and the runtime BOTH depend on, because
/// the runtime cannot see the host crate and a second copy would be free to
/// disagree with the first about when it is on.
pub fn event_trace_enabled() -> bool {
    std::env::var("COBOLT_EVENT_TRACE")
        .map(|v| {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on")
        })
        .unwrap_or(false)
}

/// Where the event trace is also written, so it survives the terminal.
///
/// stderr alone means the evidence lives in whichever console the developer
/// launched from — which is exactly where it is least reachable when the person
/// diagnosing the fault is not sitting at that machine. Overridable with
/// `COBOLT_EVENT_TRACE_FILE`.
pub fn event_trace_path() -> std::path::PathBuf {
    std::env::var_os("COBOLT_EVENT_TRACE_FILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("prc-event-trace.log"))
}

/// One event-trace line. `stage` is `send` (host) or `dispatch` (interpreter).
///
/// Goes to stderr AND to [`event_trace_path`]. Both ends of the channel call
/// this, and the host and the interpreter are different threads, so the file is
/// opened in append mode per line: ordering between the two stages is the whole
/// point of the trace, and a buffered writer per thread would reorder it.
pub fn trace_event(stage: &str, ctrl_id: &str, event_id: &str, instance: usize) {
    if !event_trace_enabled() {
        return;
    }
    let line =
        format!("[prc][event] {stage:<8} ctrl={ctrl_id:?} event={event_id:?} instance={instance}");
    // stderr, never the DISPLAY channel: instrumentation must not be mistakable
    // for the program's own output.
    eprintln!("{line}");
    append_trace_line(&line);
}

/// Append one line to the trace file, with a header on the first write of the
/// process so consecutive runs are told apart.
fn append_trace_line(line: &str) {
    static STARTED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    let path = event_trace_path();
    STARTED.get_or_init(|| {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        write_trace_line(
            &path,
            &format!(
                "\n[prc][event] ---- run start (pid {}, t={secs}) ----",
                std::process::id()
            ),
        );
    });
    write_trace_line(&path, line);
}

/// Append one line to `path`. Split from the gate so it is testable without
/// mutating process environment — which is `unsafe` in Rust 2024 and racy
/// across parallel tests either way.
fn write_trace_line(path: &std::path::Path, line: &str) {
    use std::io::Write;
    let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) else {
        // A diagnostic that cannot write must not take the program with it.
        return;
    };
    let _ = writeln!(file, "{line}");
}

/// A DISPLAY line, interleaved into the same trace.
///
/// The order of `send`/`dispatch` against the program's own output is what
/// distinguishes "the queue delivered twice" from "the handler body ran twice",
/// and two streams reconstructed after the fact cannot show it. Routing DISPLAY
/// through the same file puts them on one timeline.
pub fn trace_display(text: &str) {
    if !event_trace_enabled() {
        return;
    }
    append_trace_line(&format!("[prc][event] {:<8} {text}", "DISPLAY"));
}

/// Build the full diagnostics dump for `form`. `project` names the project (used
/// in the header); `enabled` lists each diagnostic flag and whether it is on, so
/// the reader knows which overlays were active for this run.
pub fn dump_form_diagnostics(form: &Form, project: &str, enabled: &[(&str, bool)]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "===== PowerRustCOBOL diagnostics dump =====");
    let _ = writeln!(out, "project    : {project}");
    let _ = writeln!(
        out,
        "form       : {:?}  title={:?}  {}x{}",
        form.name, form.title, form.width, form.height
    );
    let _ = writeln!(out, "generated  : {}", timestamp());
    let flags: Vec<String> = enabled
        .iter()
        .map(|(name, on)| format!("{name}={}", if *on { "on" } else { "off" }))
        .collect();
    let _ = writeln!(out, "diagnostics: {}", flags.join("  "));
    let _ = writeln!(
        out,
        "background : color={:?} gradient={} transparency={} image={:?}",
        form.background_color,
        form.background_gradient_enabled,
        form.transparency,
        form.background_image
    );
    if !form.data_bindings.is_empty() {
        let _ = writeln!(out, "form data bindings: {}", form.data_bindings.len());
    }
    let total = count_controls(&form.controls);
    let _ = writeln!(out, "controls   : {total} total\n");

    let mut index = 0usize;
    for ctrl in &form.controls {
        dump_control(&mut out, ctrl, 0, &mut index);
    }
    out
}

/// Total control count including nested children.
fn count_controls(controls: &[Control]) -> usize {
    controls
        .iter()
        .map(|c| 1 + count_controls(&c.children))
        .sum()
}

fn dump_control(out: &mut String, ctrl: &Control, depth: usize, index: &mut usize) {
    *index += 1;
    let pad = "  ".repeat(depth);
    let _ = writeln!(
        out,
        "{pad}[{index}] {} ({:?})",
        ctrl.id, ctrl.control_type
    );
    let _ = writeln!(
        out,
        "{pad}    rect   : x={} y={} w={} h={}",
        ctrl.rect.x, ctrl.rect.y, ctrl.rect.w, ctrl.rect.h
    );
    let _ = writeln!(
        out,
        "{pad}    order  : z={} tab={}   visible={} enabled={}",
        ctrl.z_order, ctrl.tab_order, ctrl.visible, ctrl.enabled
    );
    if let Some(parent) = &ctrl.parent {
        let _ = write!(out, "{pad}    parent : {parent}");
        if let Some(tab) = ctrl.tab {
            let _ = write!(out, "   tab-page={tab}");
        }
        let _ = writeln!(out);
    } else if let Some(tab) = ctrl.tab {
        let _ = writeln!(out, "{pad}    tab-page: {tab}");
    }

    // Properties, in the control's own declaration order.
    if ctrl.properties.is_empty() {
        let _ = writeln!(out, "{pad}    properties: (none)");
    } else {
        let _ = writeln!(out, "{pad}    properties ({}):", ctrl.properties.len());
        for (key, value) in &ctrl.properties {
            // Skip the packed DataGrid blob here; it is expanded below.
            if key == DATAGRID_ADVANCED_PROP {
                continue;
            }
            let _ = writeln!(out, "{pad}      {key} = {value}");
        }
    }

    // Events (handlers). Report presence + size, not the whole CDATA body.
    if !ctrl.events.is_empty() {
        let _ = writeln!(out, "{pad}    events ({}):", ctrl.events.len());
        for ev in &ctrl.events {
            let _ = writeln!(
                out,
                "{pad}      {} -> {} ({} bytes of code)",
                ev.event,
                ev.paragraph,
                ev.code.len()
            );
        }
    }

    if !ctrl.animations.is_empty() {
        let _ = writeln!(out, "{pad}    animations: {}", ctrl.animations.len());
    }

    // Expand the DataGrid's advanced configuration so its per-column setup is
    // legible next to the component-frame overlay.
    if ctrl.get_prop(DATAGRID_ADVANCED_PROP).is_some() {
        let adv = DataGridAdvanced::from_control(ctrl);
        let _ = writeln!(
            out,
            "{pad}    datagrid: {} column(s), grid-line={:?}, {} filter(s), {} row-height override(s)",
            adv.columns.len(),
            adv.grid_line_style,
            adv.filters.len(),
            adv.row_overrides.len()
        );
        for (i, col) in adv.columns.iter().enumerate() {
            let _ = writeln!(
                out,
                "{pad}      col[{i}] title={:?} width={} frozen={} bg={:?} rules={} gauge={}",
                col.title,
                col.width,
                col.frozen,
                col.background_color,
                col.value_style_rules.len(),
                col.gauge.is_some()
            );
        }
    }

    if !ctrl.children.is_empty() {
        let ids: Vec<&str> = ctrl.children.iter().map(|c| c.id.as_str()).collect();
        let _ = writeln!(
            out,
            "{pad}    children ({}): {}",
            ctrl.children.len(),
            ids.join(", ")
        );
    }
    let _ = writeln!(out);

    for child in &ctrl.children {
        dump_control(out, child, depth + 1, index);
    }
}

/// Best-effort wall-clock stamp without pulling in a date crate: ISO-ish UTC from
/// the Unix epoch. Enough to order dumps across runs.
fn timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => {
            let secs = d.as_secs();
            let (days, rem) = (secs / 86_400, secs % 86_400);
            let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
            // Civil date from days since 1970 (Howard Hinnant's algorithm).
            let z = days as i64 + 719_468;
            let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
            let doe = z - era * 146_097;
            let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
            let y = yoe + era * 400;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let day = doy - (153 * mp + 2) / 5 + 1;
            let month = if mp < 10 { mp + 3 } else { mp - 9 };
            let year = if month <= 2 { y + 1 } else { y };
            format!(
                "{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z (epoch {secs})"
            )
        }
        Err(_) => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ControlType, PropValue};

    #[test]
    fn dump_lists_every_control_with_geometry_and_props() {
        let mut form = Form::new("My App", "Main", 800, 600);
        let mut panel = Control::new("PNL", ControlType::Panel, 10, 20);
        panel.rect = crate::model::Rect::new(10, 20, 400, 300);
        let mut label = Control::new("LBL", ControlType::Label, 12, 22);
        label.set_prop("Text", PropValue::String("Hello".into()));
        label.parent = Some("PNL".into());
        panel.children.push(label);
        form.controls.push(panel);

        let dump = dump_form_diagnostics(
            &form,
            "My App",
            &[("frame_diagnostics", true), ("datagrid_diagnostics", false)],
        );

        assert!(dump.contains("project    : My App"));
        assert!(dump.contains("controls   : 2 total"));
        assert!(dump.contains("PNL (Panel)"));
        assert!(dump.contains("rect   : x=10 y=20 w=400 h=300"));
        assert!(dump.contains("LBL (Label)"));
        assert!(dump.contains("Text = Hello"));
        assert!(dump.contains("parent : PNL"));
        assert!(dump.contains("frame_diagnostics=on"));
        assert!(dump.contains("datagrid_diagnostics=off"));
    }
}

#[cfg(test)]
mod event_trace_tests {
    use super::*;

    /// The trace must reach a FILE, not just the console.
    ///
    /// stderr alone puts the evidence in whichever terminal the developer
    /// launched from — the one place it is unreachable when the person
    /// diagnosing the fault is not at that machine. A whole round trip was
    /// spent discovering that (2026-08-21).
    #[test]
    fn a_trace_line_is_appended_to_the_file() {
        let dir = std::env::temp_dir().join(format!("prc-trace-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("trace.log");
        let _ = std::fs::remove_file(&path);

        write_trace_line(&path, "[prc][event] send     ctrl=\"Switch-1\"");
        write_trace_line(&path, "[prc][event] dispatch ctrl=\"Switch-1\"");
        let body = std::fs::read_to_string(&path).expect("the trace file was written");
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines.len(), 2, "both lines appended, in order: {body:?}");
        assert!(lines[0].contains("send"), "{body:?}");
        assert!(lines[1].contains("dispatch"), "{body:?}");

        // Appending, never truncating: the second end of the channel writes
        // from a different thread and must not erase the first.
        write_trace_line(&path, "third");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap().lines().count(),
            3,
            "append, not truncate"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An unwritable path is a no-op, not a crash: instrumentation must never
    /// be the reason a developer's form dies.
    #[test]
    fn an_unwritable_trace_path_is_silent() {
        write_trace_line(std::path::Path::new("/definitely/not/a/dir/x.log"), "x");
    }

    /// The switch is off unless explicitly turned on, and accepts the same
    /// truthy spellings as every other diagnostic here.
    #[test]
    fn the_trace_is_off_by_default() {
        assert!(
            !event_trace_enabled(),
            "COBOLT_EVENT_TRACE must be opt-in; it writes to stderr and a file"
        );
    }
}
