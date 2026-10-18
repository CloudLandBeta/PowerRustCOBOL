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

// ── A secret never reaches a diagnostic ───────────────────────────────────────
//
// Operator, 2026-10-18: "keys should never be written to the log". The dump wrote every
// property as it was — a password box's `Text` and an `ApiKey` included — and the live trace
// printed every property a program wrote, value and all. Both now go through these.

/// A property name that holds a credential, by its ending, in any case: `ApiKey`,
/// `AgentAPIKey` and `EmbeddingAPIKey` do, and `ApiKeySource` does not; `AuthToken` and
/// `SessionToken` do, and `MaximumTokens` does not.
const SECRET_ENDINGS: &[&str] = &[
    "apikey",
    "password",
    "passphrase",
    "secret",
    "secretkey",
    "secretaccesskey",
    "accesskeyid",
    "token",
    "privatekey",
    "credentials",
];

/// Whether a property called `name` holds a credential.
pub fn is_secret_property(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase();
    SECRET_ENDINGS.iter().any(|e| n.ends_with(e))
}

/// Whether `prop` of a control holds a secret: a credential-named property, or the `Text`
/// of a box that hides what is typed in it (`password_character` set).
pub fn is_secret(prop: &str, password_character: Option<&str>) -> bool {
    is_secret_property(prop)
        || (prop.trim().eq_ignore_ascii_case("Text") && password_character.is_some_and(|c| !c.trim().is_empty()))
}

/// What a diagnostic shows in a secret's place: whether one is there, and how long — never
/// any of it.
pub fn hidden(value: &str) -> String {
    match value.trim().chars().count() {
        0 => "(empty)".to_owned(),
        n => format!("(hidden: {n} characters)"),
    }
}

/// Mask anything in `text` that looks like a credential, wherever it sits: access key ids
/// (`AKIA…`, `ASIA…`), secret keys and session tokens (long mixed-case base64 runs), and the
/// signature, token and credential of a pre-signed URL. For a value that is not itself a
/// secret property but may carry one — a header list, a URL, an error message.
pub fn mask_credentials(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let is_b64 = |c: char| c.is_ascii_alphanumeric() || c == '/' || c == '+' || c == '=';
    let mut i = 0;
    while i < chars.len() {
        // A pre-signed URL's secret-bearing query values.
        let rest: String = chars[i..chars.len().min(i + 24)].iter().collect();
        if let Some(key) = ["X-Amz-Signature=", "X-Amz-Security-Token=", "X-Amz-Credential="]
            .iter()
            .find(|k| rest.starts_with(**k))
        {
            out.push_str(key);
            i += key.chars().count();
            while i < chars.len() && !matches!(chars[i], '&' | ' ' | '"' | '\'' | '\n' | '\r' | '\t') {
                i += 1;
            }
            out.push_str("****");
            continue;
        }
        if is_b64(chars[i]) {
            let start = i;
            while i < chars.len() && is_b64(chars[i]) {
                i += 1;
            }
            let run: String = chars[start..i].iter().collect();
            let key_id = run.len() == 20
                && (run.starts_with("AKIA") || run.starts_with("ASIA"))
                && run.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
            let secret = run.len() >= 40
                && run.chars().any(|c| c.is_ascii_uppercase())
                && run.chars().any(|c| c.is_ascii_lowercase())
                && run.chars().any(|c| c.is_ascii_digit());
            if key_id {
                out.push_str(&run[..4]);
                out.push_str("****************");
            } else if secret {
                out.push_str("****");
            } else {
                out.push_str(&run);
            }
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// A property's value as a diagnostic may show it: [`hidden`] for a secret, else the value
/// with any credential inside it masked.
pub fn shown_value(prop: &str, value: &str, password_character: Option<&str>) -> String {
    if is_secret(prop, password_character) {
        hidden(value)
    } else {
        mask_credentials(value)
    }
}

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
        .map(|v| trace_level(&v).is_some())
        .unwrap_or(false)
}

/// How much the event trace records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceLevel {
    /// Everything except the per-frame pointer motion — the default, and what
    /// `1` / `true` / `on` select.
    Events,
    /// Every event, motion included. `all` / `full`.
    All,
}

/// The level named by `value`, or `None` when it does not enable the trace.
pub fn trace_level(value: &str) -> Option<TraceLevel> {
    let v = value.trim();
    if v.eq_ignore_ascii_case("all") || v.eq_ignore_ascii_case("full") {
        return Some(TraceLevel::All);
    }
    if v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on") {
        return Some(TraceLevel::Events);
    }
    None
}

/// Events that fire on EVERY frame the pointer is over the surface.
///
/// They are excluded from the trace unless it is set to `all`. A trace exists to
/// show the order of `send` against `dispatch` for the event being debugged, and
/// motion drowns that: a few seconds of an idle mouse buries the line that
/// matters under thousands of identical ones, and the operator's agent output
/// with it (operator, 2026-09-07). Nobody debugs delivery ordering by reading
/// pointer motion; anyone who needs it asks for `all`.
const MOTION_EVENTS: &[&str] = &["onmousemove", "onpointermove"];

/// Whether this event is recorded at the current level.
pub fn event_is_traced(event_id: &str) -> bool {
    match std::env::var("COBOLT_EVENT_TRACE")
        .ok()
        .and_then(|v| trace_level(&v))
    {
        None => false,
        Some(TraceLevel::All) => true,
        Some(TraceLevel::Events) => !is_motion_event(event_id),
    }
}

/// Whether `event_id` names a per-frame pointer-motion event.
pub fn is_motion_event(event_id: &str) -> bool {
    let id = event_id.trim().to_ascii_lowercase();
    MOTION_EVENTS.contains(&id.as_str())
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
    if !event_is_traced(event_id) {
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
        let password_character = ctrl.get_prop("PasswordCharacter").map(|v| v.to_string());
        for (key, value) in &ctrl.properties {
            // Skip the packed DataGrid blob here; it is expanded below.
            if key == DATAGRID_ADVANCED_PROP {
                continue;
            }
            let shown = shown_value(key, &value.to_string(), password_character.as_deref());
            let _ = writeln!(out, "{pad}      {key} = {shown}");
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

    /// Operator, 2026-10-18: "keys should never be written to the log". The dump wrote
    /// every property as it was. Now:
    /// - the text of a password box and every credential-named property show as present,
    ///   never as themselves;
    /// - a key id or a secret inside any other value is masked;
    /// - what is not a secret stays readable, `MaximumTokens` and `ApiKeySource` included.
    #[test]
    fn the_dump_never_writes_a_key() {
        const KEY_ID: &str = "AKIAIOSFODNN7EXAMPLE";
        const SECRET: &str = "fakeFAKE0000fakeFAKE1111fakeFAKE2222fake";
        let mut form = Form::new("PowerCloud", "Main", 800, 600);
        let mut secret_box = Control::new("Txt-Secret", ControlType::TextBox, 0, 0);
        secret_box.set_prop("PasswordCharacter", PropValue::String("*".into()));
        secret_box.set_prop("Text", PropValue::String(SECRET.into()));
        let mut key_box = Control::new("Txt-KeyId", ControlType::TextBox, 0, 30);
        key_box.set_prop("Text", PropValue::String(KEY_ID.into()));
        let mut agent = Control::new("Agent-1", ControlType::Label, 0, 60);
        agent.set_prop("AgentAPIKey", PropValue::String("sk-live-0123456789".into()));
        agent.set_prop("AuthToken", PropValue::String("tok-abc".into()));
        agent.set_prop("MaximumTokens", PropValue::Int(4096));
        agent.set_prop("ApiKeySource", PropValue::String("Settings".into()));
        agent.set_prop("Url", PropValue::String(format!("https://x/?X-Amz-Credential={KEY_ID}/2026&X-Amz-Signature=abc123")));
        form.controls.extend([secret_box, key_box, agent]);
        let dump = dump_form_diagnostics(&form, "PowerCloud", &[]);

        for secret in [KEY_ID, SECRET, "sk-live-0123456789", "tok-abc", "abc123"] {
            assert!(!dump.contains(secret), "{secret:?} is in the dump:\n{dump}");
        }
        assert!(dump.contains(&format!("Text = (hidden: {} characters)", SECRET.len())), "the password box's text is shown as present");
        assert!(dump.contains("AgentAPIKey = (hidden: 18 characters)"));
        assert!(dump.contains("AuthToken = (hidden: 7 characters)"));
        assert!(dump.contains("Text = AKIA****************"), "a key id typed into a plain box is masked where it sits");
        assert!(dump.contains("MaximumTokens = 4096") && dump.contains("ApiKeySource = Settings"), "not every name with Key or Token in it is a secret");

        assert!(is_secret_property("SecretAccessKey") && is_secret_property("embeddingapikey") && is_secret_property("SessionToken"));
        assert!(!is_secret_property("KeyName") && !is_secret_property("MaxKeys") && !is_secret_property("CurrentKeyDataItem"));
        assert_eq!(hidden("   "), "(empty)");
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

#[cfg(test)]
mod trace_level_tests {
    use super::*;

    /// Per-frame pointer motion is excluded unless the trace is set to `all`.
    ///
    /// An idle mouse over a form emits `onMouseMove` and `onPointerMove` every
    /// frame, and a few seconds of that buries whatever is being debugged —
    /// including an AgentObject's verbose output, which is what the operator
    /// was actually reading when the flood arrived (2026-09-07).
    #[test]
    fn motion_is_excluded_until_the_trace_is_set_to_all() {
        assert_eq!(trace_level("1"), Some(TraceLevel::Events));
        assert_eq!(trace_level("true"), Some(TraceLevel::Events));
        assert_eq!(trace_level("on"), Some(TraceLevel::Events));
        assert_eq!(trace_level("all"), Some(TraceLevel::All));
        assert_eq!(trace_level("FULL"), Some(TraceLevel::All));
        assert_eq!(trace_level("0"), None);
        assert_eq!(trace_level(""), None);
    }

    /// Exactly the per-frame pair, and nothing that fires on a real gesture:
    /// a click, an enter or a wheel tick is one event per action and belongs in
    /// the trace.
    #[test]
    fn only_the_per_frame_pair_counts_as_motion() {
        for id in ["onMouseMove", "onmousemove", "onPointerMove", " onPointerMove "] {
            assert!(is_motion_event(id), "{id} is per-frame motion");
        }
        for id in [
            "onClick",
            "onMouseEnter",
            "onMouseLeave",
            "onMouseDown",
            "onMouseWheel",
            "onResponse",
        ] {
            assert!(!is_motion_event(id), "{id} fires once per action, not per frame");
        }
    }
}
