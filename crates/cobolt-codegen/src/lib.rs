// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Code generation: converts a [`cobolt_forms::Form`] into a complete COBOL source file.
//!
//! # Architecture (v1.0 — PowerCOBOL nested-program model)
//!
//! The `.cfrm` file is the **single source of truth**.  The generated `.cbl` is a
//! build artifact — it is never edited by hand.  All event-handler code lives in
//! [`cobolt_forms::EventBinding::code`] and [`EventBinding::local_ws`], stored in
//! the form file and edited through the modal code editor in the Form Designer.
//!
//! Each event handler becomes a COBOL-85 **nested program** inside the outer program:
//!
//! ```cobol
//!  IDENTIFICATION DIVISION.
//!  PROGRAM-ID. MAIN-FORM.
//!
//!  ENVIRONMENT DIVISION.
//!
//!  DATA DIVISION.
//!  WORKING-STORAGE SECTION.
//!  *>── Form controls ──────────────────────────────────────────
//!  01 WS-BTN-OK.
//!     05 WS-BTN-OK-TEXT    PIC X(256) VALUE "OK".
//!     05 WS-BTN-OK-VISIBLE PIC 9      VALUE 1.
//!     05 WS-BTN-OK-ENABLED PIC 9      VALUE 1.
//!
//!  PROCEDURE DIVISION.
//!  COBOL-MAIN.
//!      CALL "MAIN-FORM--ONLOAD".
//!      PERFORM COBOL-EVENT-LOOP.
//!      CALL "MAIN-FORM--ONCLOSE".
//!      STOP RUN.
//!
//!  COBOL-EVENT-LOOP.
//!      PERFORM UNTIL COBOL-QUIT = 1
//!          CALL "COBOL-WAIT-EVENT"
//!              USING COBOL-EVENT-ID COBOL-CONTROL-ID
//!          EVALUATE COBOL-CONTROL-ID
//!              WHEN "BTN-OK"
//!                  EVALUATE COBOL-EVENT-ID
//!                      WHEN "Click"
//!                          CALL "BTN-OK--CLICK"
//!                  END-EVALUATE
//!          END-EVALUATE
//!      END-PERFORM.
//!
//!  *> ── Nested event-handler programs (COBOL-85) ────────────
//!       IDENTIFICATION DIVISION.
//!       PROGRAM-ID. BTN-OK--CLICK.
//!       DATA DIVISION.
//!       WORKING-STORAGE SECTION.
//!       *>    (local_ws from EventBinding goes here)
//!       PROCEDURE DIVISION.
//!           *>    (code from EventBinding goes here)
//!           GOBACK.
//!       END PROGRAM BTN-OK--CLICK.
//!
//!       END PROGRAM MAIN-FORM.
//! ```

use cobolt_forms::{Control, ControlType, Form};
use cobolt_forms::model::PropValue;

// ── Public API ────────────────────────────────────────────────────────────────

/// Generate a complete COBOL source skeleton from `form`.
///
/// Returns a `String` containing fixed-format COBOL source code.
pub fn generate(form: &Form) -> String {
    let mut out = String::with_capacity(4096);

    write_identification(&mut out, form);
    write_environment(&mut out);
    write_data_division(&mut out, form);
    write_procedure_division(&mut out, form);

    out
}

/// Regenerate the complete COBOL source from `form`.
///
/// In the v1.0 architecture the `.cbl` is a **build artifact** — all event-handler
/// code is stored in the `.cfrm` file inside [`cobolt_forms::EventBinding`] and
/// edited through the Form Designer's modal code editor.  There is therefore nothing
/// to "merge" from an existing source file; this function is a clean alias for
/// [`generate`].
///
/// The `_existing_source` parameter is accepted for API compatibility but is not
/// read.  Callers that formerly relied on paragraph-preservation behaviour should
/// migrate to storing code in the form model instead.
pub fn regenerate(form: &Form, _existing_source: &str) -> String {
    generate(form)
}

// ── Section writers ───────────────────────────────────────────────────────────

fn write_identification(out: &mut String, form: &Form) {
    out.push_str("       IDENTIFICATION DIVISION.\n");
    out.push_str(&format!("       PROGRAM-ID. {}.\n", form.name));
    out.push('\n');
}

fn write_environment(out: &mut String) {
    out.push_str("       ENVIRONMENT DIVISION.\n");
    out.push('\n');
}

fn write_data_division(out: &mut String, form: &Form) {
    out.push_str("       DATA DIVISION.\n");
    out.push_str("       WORKING-STORAGE SECTION.\n");
    out.push_str("      *>── Cobolt runtime fields ─────────────────────────────────────\n");
    out.push_str("       01 COBOL-QUIT             PIC 9        VALUE 0.\n");
    out.push_str("       01 COBOL-EVENT-ID         PIC X(64)   VALUE SPACES.\n");
    out.push_str("       01 COBOL-CONTROL-ID       PIC X(64)   VALUE SPACES.\n");
    out.push_str("       01 COBOL-LAST-STATUS       PIC X(256)  VALUE SPACES.\n");
    out.push_str("       01 FORM-NAME               PIC X(64)   VALUE ");
    out.push_str(&format!("'{}'.\n", form.name));
    out.push('\n');

    let all_controls = collect_all_controls(&form.controls);

    // ── REST / HTTP infrastructure (emitted when any RestClient exists) ─────
    let has_rest = all_controls.iter().any(|c| c.control_type == ControlType::RestClient);
    if has_rest {
        out.push_str("      *>── REST / HTTP runtime variables ──────────────────────────────\n");
        out.push_str("      *>   Usage:\n");
        out.push_str("      *>     MOVE 'https://api.example.com/resource' TO WS-REQUEST-URL\n");
        out.push_str("      *>     PERFORM RST1-GET\n");
        out.push_str("      *>     IF WS-HTTP-STATUS = 200\n");
        out.push_str("      *>         DISPLAY WS-HTTP-RESPONSE\n");
        out.push_str("      *>     END-IF\n");
        out.push_str("       01 WS-REQUEST-URL        PIC X(2048)  VALUE SPACES.\n");
        out.push_str("       01 WS-REQUEST-BODY       PIC X(32767) VALUE SPACES.\n");
        out.push_str("       01 WS-HTTP-RESPONSE      PIC X(32767) VALUE SPACES.\n");
        out.push_str("       01 WS-HTTP-STATUS        PIC 9(4)     VALUE 0.\n");
        out.push_str("       01 WS-HTTP-HEADER-NAME   PIC X(128)   VALUE SPACES.\n");
        out.push_str("       01 WS-HTTP-HEADER-VALUE  PIC X(512)   VALUE SPACES.\n");
        out.push_str("       01 WS-JSON-KEY           PIC X(256)   VALUE SPACES.\n");
        out.push_str("       01 WS-JSON-VALUE         PIC X(4096)  VALUE SPACES.\n");
        out.push('\n');
    }

    // ── Animation runtime fields ───────────────────────────────────────────
    let has_anims = all_controls.iter().any(|c| !c.animations.is_empty())
        || !form.animations.is_empty();
    if has_anims {
        out.push_str("      *>── Animation runtime fields ──────────────────────────────────\n");
        out.push_str("      *>   INVOKE ctrl-id 'PlayAnimation' USING BY VALUE WS-ANIM-NAME\n");
        out.push_str("       01 WS-ANIM-NAME          PIC X(128)  VALUE SPACES.\n");
        out.push_str("       01 WS-ANIM-ELAPSED-MS    PIC 9(8)    VALUE 0.\n");
        out.push('\n');
    }

    // ── Agent infrastructure ───────────────────────────────────────────────
    let has_agents = all_controls.iter().any(|c| c.control_type == ControlType::AgentObject);
    if has_agents {
        out.push_str("      *>── AI Agent infrastructure ────────────────────────────────────\n");
        out.push_str("      *>   INVOKE agent-id 'Ask'\n");
        out.push_str("      *>       USING BY VALUE WS-AGENT-PROMPT\n");
        out.push_str("      *>       RETURNING WS-AGENT-RESPONSE\n");
        out.push_str("       01 WS-AGENT-PROMPT        PIC X(4096)  VALUE SPACES.\n");
        out.push_str("       01 WS-AGENT-RESPONSE      PIC X(32767) VALUE SPACES.\n");
        out.push_str("       01 WS-AGENT-ERROR         PIC X(512)   VALUE SPACES.\n");
        out.push('\n');
    }

    // ── Per-RestClient instance fields ────────────────────────────────────
    for ctrl in all_controls.iter().filter(|c| c.control_type == ControlType::RestClient) {
        let pfx = format!("WS-{}", ctrl.id.replace('-', "-"));
        let base = ctrl.get_prop("BaseURL").map(|v| v.as_str().to_owned()).unwrap_or_default();
        out.push_str(&format!("      *>── REST client: {} ──────────────────────────────────\n", ctrl.id));
        out.push_str(&format!("       01 {}-BASE-URL      PIC X(2048) VALUE '{}'.\n", pfx, base));
        let resp_item = ctrl.get_prop("ResponseDataItem").map(|v| v.as_str().to_owned()).unwrap_or_default();
        if !resp_item.is_empty() {
            out.push_str(&format!("       01 {}             PIC X(32767) VALUE SPACES.\n", resp_item));
        }
        let status_item = ctrl.get_prop("StatusDataItem").map(|v| v.as_str().to_owned()).unwrap_or_default();
        if !status_item.is_empty() {
            out.push_str(&format!("       01 {}         PIC 9(4) VALUE 0.\n", status_item));
        }
        out.push('\n');
    }

    // ── DataGrid CSV fields ───────────────────────────────────────────────
    for ctrl in all_controls.iter().filter(|c| c.control_type == ControlType::DataGrid) {
        if ctrl.get_prop("ExportCSV").map(|v| v.as_bool()).unwrap_or(false) {
            let pfx = format!("WS-{}", ctrl.id.replace('-', "-"));
            out.push_str(&format!("      *>── DataGrid {} CSV export ──────────────────────────\n", ctrl.id));
            out.push_str(&format!("       01 {}-CSV-PATH    PIC X(512)  VALUE SPACES.\n", pfx));
            out.push_str(&format!("       01 {}-CSV-STATUS  PIC 9       VALUE 0.\n", pfx));
            out.push('\n');
        }
    }

    // ── SQL Database infrastructure (Phase 8) ────────────────────────────
    let has_sql = all_controls.iter().any(|c| c.control_type == ControlType::SqlDatabase);
    if has_sql {
        out.push_str("      *>── SQL Database runtime variables ──────────────────────────────\n");
        out.push_str("      *>   Usage:\n");
        out.push_str("      *>     MOVE 'SELECT * FROM t' TO WS-SQL-QUERY\n");
        out.push_str("      *>     PERFORM DB1-CONNECT\n");
        out.push_str("      *>     PERFORM DB1-EXEC\n");
        out.push_str("      *>     PERFORM UNTIL WS-SQL-MORE = 'N'\n");
        out.push_str("      *>         MOVE 1 TO WS-SQL-COL-INDEX\n");
        out.push_str("      *>         CALL \"COBOL-FETCH-ROW\" USING WS-DB1-HANDLE\n");
        out.push_str("      *>                                       WS-SQL-COL-INDEX\n");
        out.push_str("      *>                                       WS-SQL-CURRENT-VALUE\n");
        out.push_str("      *>                                       WS-SQL-ERROR\n");
        out.push_str("      *>         CALL \"COBOL-NEXT-ROW\" USING WS-DB1-HANDLE\n");
        out.push_str("      *>                                      WS-SQL-MORE\n");
        out.push_str("      *>     END-PERFORM\n");
        out.push_str("       01 WS-SQL-QUERY           PIC X(4096)  VALUE SPACES.\n");
        out.push_str("       01 WS-SQL-ERROR            PIC X(512)   VALUE SPACES.\n");
        out.push_str("       01 WS-SQL-ROW-COUNT        PIC 9(9)     VALUE 0.\n");
        out.push_str("       01 WS-SQL-COL-INDEX        PIC 9(4)     VALUE 1.\n");
        out.push_str("       01 WS-SQL-CURRENT-VALUE    PIC X(512)   VALUE SPACES.\n");
        out.push_str("       01 WS-SQL-MORE             PIC X(1)     VALUE 'N'.\n");
        out.push('\n');
    }

    for ctrl in all_controls.iter().filter(|c| c.control_type == ControlType::SqlDatabase) {
        let pfx  = format!("WS-{}", ctrl.id.to_ascii_uppercase());
        let cs   = ctrl.get_prop("ConnectionString").map(|v| v.as_str().to_owned())
                       .unwrap_or_else(|| ":memory:".into());
        let drv  = ctrl.get_prop("Driver").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "sqlite".into());
        out.push_str(&format!("      *>── SQL instance: {} ({}) ─────────────────────────────────\n", ctrl.id, drv));
        out.push_str(&format!("       01 {pfx}-CONN-STRING   PIC X(512)  VALUE '{cs}'.\n"));
        out.push_str(&format!("       01 {pfx}-HANDLE        PIC 9(9)    VALUE 0.\n"));
        out.push_str(&format!("       01 {pfx}-STATUS        PIC X(512)  VALUE SPACES.\n"));
        out.push('\n');
    }

    // ── Timer runtime fields ──────────────────────────────────────────────
    for ctrl in all_controls.iter().filter(|c| c.control_type == ControlType::Timer) {
        let pfx = format!("WS-{}", ctrl.id.replace('-', "-"));
        let iv  = ctrl.get_prop("Interval").map(|v| v.as_i64()).unwrap_or(1000);
        let ena = ctrl.get_prop("Enabled").map(|v| if v.as_bool() { 1 } else { 0 }).unwrap_or(1);
        out.push_str(&format!("      *>── Timer: {} ──────────────────────────────────────────\n", ctrl.id));
        out.push_str(&format!("       01 {}-INTERVAL   PIC 9(8) VALUE {}.\n", pfx, iv));
        out.push_str(&format!("       01 {}-ENABLED    PIC 9    VALUE {}.\n", pfx, ena));
        out.push_str(&format!("       01 {}-ELAPSED-MS PIC 9(8) VALUE 0.\n", pfx));
        out.push('\n');
    }

    // ── Chart working-storage items ───────────────────────────────────────
    let chart_types = [
        ControlType::BarChart, ControlType::LineChart, ControlType::PieChart,
        ControlType::AreaChart, ControlType::ScatterChart, ControlType::DonutChart,
    ];
    for ctrl in all_controls.iter().filter(|c| chart_types.contains(&c.control_type)) {
        let pfx  = format!("WS-{}", ctrl.id.to_ascii_uppercase().replace('-', "-"));
        let ds   = ctrl.get_prop("DataSource").map(|v| v.as_str().to_owned()).unwrap_or_default();
        let cnt  = ctrl.get_prop("DataCount").map(|v| v.as_str().to_owned()).unwrap_or_default();
        let kind = ctrl.control_type.as_str();
        out.push_str(&format!(
            "      *>── Chart: {} (type: {}) ─────────────────────────────────────\n",
            ctrl.id, kind));
        out.push_str(&format!(
            "      *>   Data source : {}\n",
            if ds.is_empty() { "(none — use INVOKE SET-TABLE or ADD-POINT)" } else { &ds }));
        out.push_str(&format!(
            "      *>   Row count   : {}\n",
            if cnt.is_empty() { "(not set)" } else { &cnt }));
        out.push_str(&format!("       01 {}-SELECTED-IDX PIC 9(6) VALUE 0.\n", pfx));
        out.push_str(&format!("       01 {}-SELECTED-LBL PIC X(64) VALUE SPACES.\n", pfx));
        out.push_str(&format!("       01 {}-SELECTED-VAL PIC 9(18)V9(6) VALUE ZEROES.\n", pfx));
        out.push('\n');
    }

    // ── Modal window data items ───────────────────────────────────────────
    for ctrl in all_controls.iter().filter(|c| c.control_type == ControlType::ModalWindow) {
        let shared = ctrl.get_prop("SharedDataItems")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();
        let prog = ctrl.get_prop("ProgramName")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| ctrl.id.clone());
        let pfx = format!("WS-{}", ctrl.id.replace('-', "-"));
        out.push_str(&format!("      *>── Modal window: {} (program: {}) ───────────────────\n", ctrl.id, prog));
        out.push_str(&format!("       01 {}-RESULT      PIC X(16) VALUE SPACES.\n", pfx));
        if !shared.is_empty() {
            out.push_str("      *>   Shared data items (passed to modal by reference):\n");
            for item in shared.split(',').map(|s| s.trim()) {
                if !item.is_empty() {
                    out.push_str(&format!("      *>     {}\n", item));
                }
            }
        }
        out.push('\n');
    }

    // ── User Working Storage (raw COBOL from .cfrm) ──────────────────────
    if !form.user_ws_source.trim().is_empty() {
        out.push_str("      *>── User Working Storage ────────────────────────────────────────\n");
        for line in form.user_ws_source.trim().lines() {
            out.push_str(line);
            out.push('\n');
        }
        out.push('\n');
    }

    // ── Per-control groups ────────────────────────────────────────────────
    if !all_controls.is_empty() {
        out.push_str("      *>── Form controls ───────────────────────────────────────────────\n");
        for ctrl in &all_controls {
            write_control_group(out, ctrl);
        }
    }
}

/// Write a `01 WS-<ID>.` group for one control.
fn write_control_group(out: &mut String, ctrl: &Control) {
    let prefix = format!("WS-{}", ctrl.id.replace('-', "-"));
    out.push_str(&format!("       01 {}.\n", prefix));

    // Caption / Text property (if present)
    let caption_key = caption_prop_key(&ctrl.control_type);
    let caption_val = ctrl
        .get_prop(caption_key)
        .map(|v| match v {
            PropValue::String(s) => s.clone(),
            PropValue::Int(n)    => n.to_string(),
            PropValue::Bool(b)   => if *b { "1" } else { "0" }.to_string(),
        })
        .unwrap_or_else(|| ctrl.id.clone());

    out.push_str(&format!(
        "          05 {}-TEXT       PIC X(256) VALUE '{}'.\n",
        prefix, caption_val
    ));
    out.push_str(&format!(
        "          05 {}-VISIBLE    PIC 9      VALUE {}.\n",
        prefix,
        if ctrl.visible { 1 } else { 0 }
    ));
    out.push_str(&format!(
        "          05 {}-ENABLED    PIC 9      VALUE {}.\n",
        prefix,
        if ctrl.enabled { 1 } else { 0 }
    ));

    // Extra numeric value field for editable controls
    if matches!(
        ctrl.control_type,
        ControlType::TextBox
            | ControlType::CheckBox
            | ControlType::ComboBox
            | ControlType::ListBox
    ) {
        out.push_str(&format!(
            "          05 {}-VALUE      PIC X(512) VALUE SPACES.\n",
            prefix
        ));
    }

    // Slider: numeric value + min/max/step fields
    if matches!(ctrl.control_type, ControlType::Slider) {
        let val  = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0);
        let min  = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0);
        let max  = ctrl.get_prop("Maximum").map(|v| v.as_i64()).unwrap_or(100);
        let step = ctrl.get_prop("Step").map(|v| v.as_i64()).unwrap_or(10);
        // Each 05 item must start at column 8+ in fixed-format COBOL.
        // Use separate push_str calls so Rust string continuation (`\n\`)
        // does not eat the leading spaces that position the level numbers.
        out.push_str(&format!("          05 {prefix}-VALUE      PIC S9(9) VALUE {val}.\n"));
        out.push_str(&format!("          05 {prefix}-MINIMUM    PIC S9(9) VALUE {min}.\n"));
        out.push_str(&format!("          05 {prefix}-MAXIMUM    PIC S9(9) VALUE {max}.\n"));
        out.push_str(&format!("          05 {prefix}-STEP       PIC S9(9) VALUE {step}.\n"));
    }

    out.push('\n');
}

fn write_procedure_division(out: &mut String, form: &Form) {
    out.push_str("       PROCEDURE DIVISION.\n");

    let all_controls = collect_all_controls(&form.controls);

    // ── COBOL-MAIN ──────────────────────────────────────────────────────
    out.push_str("       COBOL-MAIN.\n");
    out.push_str("           CALL \"COBOL-INIT-FORM\" USING FORM-NAME\n");

    // Kick off timer dispatcher if any timers exist
    let has_timers = all_controls.iter().any(|c| c.control_type == ControlType::Timer);
    if has_timers {
        out.push_str("           PERFORM COBOL-START-TIMERS\n");
    }

    // Call OnLoad nested program
    if let Some(ev) = form.form_events.iter().find(|e| e.event == "OnLoad") {
        out.push_str(&format!("           CALL \"{}\"\n", ev.paragraph));
    }

    out.push_str("           PERFORM COBOL-EVENT-LOOP\n");

    // Call OnClose nested program
    if let Some(ev) = form.form_events.iter().find(|e| e.event == "OnClose") {
        out.push_str(&format!("           CALL \"{}\"\n", ev.paragraph));
    }

    out.push_str("           STOP RUN.\n");
    out.push('\n');

    // ── COBOL-EVENT-LOOP — dispatches via CALL to nested programs ─────────
    write_event_loop(out, form);

    // ── Infrastructure helper paragraphs (outer program scope) ────────────
    write_timer_stubs(out, &all_controls);
    write_csv_export_stubs(out, &all_controls);
    write_rest_client_stubs(out, &all_controls);
    write_sql_stubs(out, &all_controls);
    write_agent_stubs(out, &all_controls);
    write_animation_stubs(out, form, &all_controls);
    write_modal_stubs(out, &all_controls);
    write_chart_stubs(out, &all_controls);

    // ── Nested COBOL-85 programs — one per event handler ─────────────────
    write_nested_programs(out, form, &all_controls);

    // ── Close the outer program ───────────────────────────────────────────
    out.push_str(&format!("       END PROGRAM {}.\n", form.name));
}

// ── Timer paragraph generator ─────────────────────────────────────────────────

fn write_timer_stubs(out: &mut String, all_controls: &[&Control]) {
    let timers: Vec<&&Control> = all_controls
        .iter()
        .filter(|c| c.control_type == ControlType::Timer)
        .collect();
    if timers.is_empty() { return; }

    // COBOL-START-TIMERS: enable each timer at startup
    out.push_str("       COBOL-START-TIMERS.\n");
    out.push_str("      *>    Called once from COBOL-MAIN to register timer intervals.\n");
    for ctrl in &timers {
        let iv = ctrl.get_prop("Interval").map(|v| v.as_i64()).unwrap_or(1000);
        out.push_str(&format!(
            "           INVOKE \"{id}\" 'SetInterval' USING BY VALUE {iv}\n",
            id = ctrl.id, iv = iv
        ));
    }
    out.push_str("           CONTINUE.\n");
    out.push('\n');

    // COBOL-TIMER-TICK: dispatches to each timer's paragraph
    out.push_str("       COBOL-TIMER-TICK.\n");
    out.push_str("      *>    The runtime calls this paragraph every time any timer fires.\n");
    out.push_str("      *>    COBOL-CONTROL-ID contains the timer's ID.\n");
    out.push_str("           EVALUATE COBOL-CONTROL-ID\n");
    for ctrl in &timers {
        let para = ctrl.get_prop("Paragraph")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| format!("{}-TICK", ctrl.id));
        out.push_str(&format!(
            "               WHEN \"{id}\"\n                   PERFORM {para}\n",
            id = ctrl.id
        ));
    }
    out.push_str("           END-EVALUATE.\n");
    out.push('\n');

    // Individual tick handler stubs
    for ctrl in &timers {
        let para = ctrl.get_prop("Paragraph")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| format!("{}-TICK", ctrl.id));
        write_stub_paragraph(
            out, &para,
            &format!("Timer {} fires every {} ms — add your logic here",
                ctrl.id,
                ctrl.get_prop("Interval").map(|v| v.as_i64()).unwrap_or(1000)
            ),
        );
    }
}

// ── DataGrid CSV export paragraph generator ───────────────────────────────────

fn write_csv_export_stubs(out: &mut String, all_controls: &[&Control]) {
    for ctrl in all_controls.iter().filter(|c| c.control_type == ControlType::DataGrid) {
        if !ctrl.get_prop("ExportCSV").map(|v| v.as_bool()).unwrap_or(false) {
            continue;
        }
        let pfx  = format!("WS-{}", ctrl.id.replace('-', "-"));
        let para = ctrl.get_prop("CSVParagraph")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| format!("{}-EXPORT-CSV", ctrl.id));
        let delim = ctrl.get_prop("CSVDelimiter")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| ",".to_owned());

        out.push_str(&format!("       {}.\n", para));
        out.push_str(&format!(
            "      *>    Export {} data to CSV file.  Delimiter: \"{}\"\n",
            ctrl.id, delim
        ));
        out.push_str(&format!(
            "      *>    Set {pfx}-CSV-PATH to the desired output file path before calling.\n"
        ));
        out.push_str(&format!(
            "           INVOKE \"{id}\" 'ExportCSV'\n",
            id = ctrl.id
        ));
        out.push_str(&format!(
            "               USING BY REFERENCE {pfx}-CSV-PATH\n"
        ));
        out.push_str(&format!(
            "               RETURNING {pfx}-CSV-STATUS\n"
        ));
        out.push_str(&format!(
            "           IF {pfx}-CSV-STATUS NOT = 0\n"
        ));
        out.push_str(&format!(
            "               DISPLAY \"CSV export error: \" {pfx}-CSV-STATUS\n"
        ));
        out.push_str("           END-IF.\n");
        out.push('\n');
    }
}

// ── RestClient call stub generator ───────────────────────────────────────────

fn write_rest_client_stubs(out: &mut String, all_controls: &[&Control]) {
    for ctrl in all_controls.iter().filter(|c| c.control_type == ControlType::RestClient) {
        let para_get  = format!("{}-GET",  ctrl.id);
        let para_post = format!("{}-POST", ctrl.id);
        let para_put  = format!("{}-PUT",  ctrl.id);
        let resp_para = ctrl.get_prop("ResponsePara")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| format!("{}-ON-RESPONSE", ctrl.id));
        let err_para = ctrl.get_prop("ErrorPara")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| format!("{}-ON-ERROR", ctrl.id));
        let resp_item   = ctrl.get_prop("ResponseDataItem").map(|v| v.as_str().to_owned()).unwrap_or_default();
        let status_item = ctrl.get_prop("StatusDataItem").map(|v| v.as_str().to_owned()).unwrap_or_default();

        // ── {ID}-GET ─────────────────────────────────────────────────────────
        out.push_str(&format!("       {}.\n", para_get));
        out.push_str(&format!(
            "      *>    HTTP GET via {} — set WS-REQUEST-URL before calling.\n",
            ctrl.id
        ));
        out.push_str("           CALL \"COBOL-HTTP-GET\"\n");
        out.push_str("               USING WS-REQUEST-URL\n");
        out.push_str("                     WS-HTTP-RESPONSE\n");
        out.push_str("                     WS-HTTP-STATUS\n");
        out.push_str("           END-CALL\n");
        out.push_str("           EVALUATE TRUE\n");
        out.push_str("               WHEN WS-HTTP-STATUS >= 200\n");
        out.push_str("                AND WS-HTTP-STATUS <= 299\n");
        out.push_str(&format!("                   PERFORM {}\n", resp_para));
        out.push_str("               WHEN OTHER\n");
        out.push_str(&format!("                   PERFORM {}\n", err_para));
        out.push_str("           END-EVALUATE.\n");
        out.push('\n');

        // ── {ID}-POST ────────────────────────────────────────────────────────
        out.push_str(&format!("       {}.\n", para_post));
        out.push_str(&format!(
            "      *>    HTTP POST via {} — set WS-REQUEST-URL and WS-REQUEST-BODY before calling.\n",
            ctrl.id
        ));
        out.push_str("           CALL \"COBOL-HTTP-POST\"\n");
        out.push_str("               USING WS-REQUEST-URL\n");
        out.push_str("                     WS-REQUEST-BODY\n");
        out.push_str("                     WS-HTTP-RESPONSE\n");
        out.push_str("                     WS-HTTP-STATUS\n");
        out.push_str("           END-CALL\n");
        out.push_str("           EVALUATE TRUE\n");
        out.push_str("               WHEN WS-HTTP-STATUS >= 200\n");
        out.push_str("                AND WS-HTTP-STATUS <= 299\n");
        out.push_str(&format!("                   PERFORM {}\n", resp_para));
        out.push_str("               WHEN OTHER\n");
        out.push_str(&format!("                   PERFORM {}\n", err_para));
        out.push_str("           END-EVALUATE.\n");
        out.push('\n');

        // ── {ID}-PUT ─────────────────────────────────────────────────────────
        out.push_str(&format!("       {}.\n", para_put));
        out.push_str(&format!(
            "      *>    HTTP PUT via {} — set WS-REQUEST-URL and WS-REQUEST-BODY before calling.\n",
            ctrl.id
        ));
        out.push_str("           CALL \"COBOL-HTTP-PUT\"\n");
        out.push_str("               USING WS-REQUEST-URL\n");
        out.push_str("                     WS-REQUEST-BODY\n");
        out.push_str("                     WS-HTTP-RESPONSE\n");
        out.push_str("                     WS-HTTP-STATUS\n");
        out.push_str("           END-CALL\n");
        out.push_str("           EVALUATE TRUE\n");
        out.push_str("               WHEN WS-HTTP-STATUS >= 200\n");
        out.push_str("                AND WS-HTTP-STATUS <= 299\n");
        out.push_str(&format!("                   PERFORM {}\n", resp_para));
        out.push_str("               WHEN OTHER\n");
        out.push_str(&format!("                   PERFORM {}\n", err_para));
        out.push_str("           END-EVALUATE.\n");
        out.push('\n');

        // ── Response / error handler stubs ───────────────────────────────────
        write_stub_paragraph(
            out, &resp_para,
            &format!("{} response handler — WS-HTTP-RESPONSE contains the body, WS-HTTP-STATUS the code", ctrl.id),
        );
        write_stub_paragraph(
            out, &err_para,
            &format!("{} error handler — WS-HTTP-STATUS contains the error code (0 = network failure)", ctrl.id),
        );

        // ── Optional sync paragraph ───────────────────────────────────────────
        if !resp_item.is_empty() || !status_item.is_empty() {
            let sync_para = format!("{}-SYNC-ITEMS", ctrl.id);
            out.push_str(&format!("       {}.\n", sync_para));
            out.push_str("      *>    Copy response / status into your declared data items.\n");
            if !resp_item.is_empty() {
                out.push_str(&format!("           MOVE WS-HTTP-RESPONSE TO {}\n", resp_item));
            }
            if !status_item.is_empty() {
                out.push_str(&format!("           MOVE WS-HTTP-STATUS TO {}\n", status_item));
            }
            out.push_str("           CONTINUE.\n");
            out.push('\n');
        }
    }
}

// ── AgentObject Ask stub generator ───────────────────────────────────────────

fn write_agent_stubs(out: &mut String, all_controls: &[&Control]) {
    for ctrl in all_controls.iter().filter(|c| c.control_type == ControlType::AgentObject) {
        let ask_para = format!("{}-ASK", ctrl.id);
        let resp_para = ctrl.get_prop("ResponsePara")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| format!("{}-ON-RESPONSE", ctrl.id));
        let err_para = ctrl.get_prop("ErrorPara")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| format!("{}-ON-ERROR", ctrl.id));
        let resp_item = ctrl.get_prop("ResponseDataItem").map(|v| v.as_str().to_owned()).unwrap_or_default();
        let model = ctrl.get_prop("AgentModel").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "llama3.2".into());
        let url   = ctrl.get_prop("AgentURL").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "http://localhost:11434".into());

        out.push_str(&format!("       {}.\n", ask_para));
        out.push_str(&format!(
            "      *>    Ask the AI agent {} (model: {}, endpoint: {})\n",
            ctrl.id, model, url
        ));
        out.push_str("      *>    Set WS-AGENT-PROMPT before calling.\n");
        out.push_str(&format!(
            "           INVOKE \"{id}\" 'Ask'\n               USING BY VALUE WS-AGENT-PROMPT\n               RETURNING WS-AGENT-RESPONSE\n",
            id = ctrl.id
        ));
        if !resp_item.is_empty() {
            out.push_str(&format!("           MOVE WS-AGENT-RESPONSE TO {}\n", resp_item));
        }
        out.push_str("           EVALUATE TRUE\n");
        out.push_str("               WHEN WS-AGENT-ERROR = SPACES\n");
        out.push_str(&format!("                   PERFORM {}\n", resp_para));
        out.push_str("               WHEN OTHER\n");
        out.push_str(&format!("                   PERFORM {}\n", err_para));
        out.push_str("           END-EVALUATE.\n");
        out.push('\n');

        write_stub_paragraph(
            out, &resp_para,
            &format!("{} response ready — WS-AGENT-RESPONSE contains the LLM reply", ctrl.id),
        );
        write_stub_paragraph(
            out, &err_para,
            &format!("{} error handler — WS-AGENT-ERROR contains the error message", ctrl.id),
        );
    }
}

// ── Animation play / stop stub generator ─────────────────────────────────────

fn write_animation_stubs(out: &mut String, form: &Form, all_controls: &[&Control]) {
    // Gather every named animation across all controls + form itself
    let mut entries: Vec<(String, String)> = Vec::new(); // (ctrl_id, anim_name)
    for anim in &form.animations {
        entries.push(("FORM".into(), anim.name.clone()));
    }
    for ctrl in all_controls {
        for anim in &ctrl.animations {
            entries.push((ctrl.id.clone(), anim.name.clone()));
        }
    }
    if entries.is_empty() { return; }

    // ── COBOL-PLAY-ANIMATION ─────────────────────────────────────────────────
    // Dispatches to the correct INVOKE based on WS-ANIM-NAME.
    out.push_str("       COBOL-PLAY-ANIMATION.\n");
    out.push_str("      *> Set WS-ANIM-NAME before calling this paragraph.\n");
    if entries.len() == 1 {
        let (ctrl_id, anim_name) = &entries[0];
        if ctrl_id != "FORM" {
            out.push_str(&format!(
                "           INVOKE {ctrl_id} 'PlayAnimation'\n"
            ));
            out.push_str(&format!(
                "               USING BY VALUE \"{anim_name}\".\n"
            ));
        } else {
            out.push_str("           CONTINUE.\n");
        }
    } else {
        out.push_str("           EVALUATE WS-ANIM-NAME\n");
        for (ctrl_id, anim_name) in &entries {
            out.push_str(&format!(
                "               WHEN \"{anim_name}\"\n"
            ));
            if ctrl_id != "FORM" {
                out.push_str(&format!(
                    "                   INVOKE {ctrl_id} 'PlayAnimation'\n"
                ));
                out.push_str(&format!(
                    "                       USING BY VALUE \"{anim_name}\"\n"
                ));
            } else {
                out.push_str("                   CONTINUE\n");
            }
        }
        out.push_str("               WHEN OTHER\n");
        out.push_str("                   CONTINUE\n");
        out.push_str("           END-EVALUATE.\n");
    }
    out.push('\n');

    // ── COBOL-STOP-ANIMATION ──────────────────────────────────────────────────
    out.push_str("       COBOL-STOP-ANIMATION.\n");
    out.push_str("      *> Set WS-ANIM-NAME before calling this paragraph.\n");
    if entries.len() == 1 {
        let (ctrl_id, anim_name) = &entries[0];
        if ctrl_id != "FORM" {
            out.push_str(&format!(
                "           INVOKE {ctrl_id} 'StopAnimation'\n"
            ));
            out.push_str(&format!(
                "               USING BY VALUE \"{anim_name}\".\n"
            ));
        } else {
            out.push_str("           CONTINUE.\n");
        }
    } else {
        out.push_str("           EVALUATE WS-ANIM-NAME\n");
        for (ctrl_id, anim_name) in &entries {
            out.push_str(&format!(
                "               WHEN \"{anim_name}\"\n"
            ));
            if ctrl_id != "FORM" {
                out.push_str(&format!(
                    "                   INVOKE {ctrl_id} 'StopAnimation'\n"
                ));
                out.push_str(&format!(
                    "                       USING BY VALUE \"{anim_name}\"\n"
                ));
            } else {
                out.push_str("                   CONTINUE\n");
            }
        }
        out.push_str("               WHEN OTHER\n");
        out.push_str("                   CONTINUE\n");
        out.push_str("           END-EVALUATE.\n");
    }
    out.push('\n');

    // ── Per-trigger auto-call paragraphs ──────────────────────────────────────
    // Emit OnLoad / OnClick / OnFocus trigger helpers for each control's anims.
    for ctrl in all_controls {
        for anim in &ctrl.animations {
            if anim.trigger.as_str() == "OnLoad" {
                // already called from COBOL-FORM-LOAD via timer dispatch
                continue;
            }
            let para = format!("{}-PLAY-{}", ctrl.id,
                anim.name.to_ascii_uppercase().replace(' ', "-").replace('-', "-"));
            out.push_str(&format!("       {para}.\n"));
            out.push_str(&format!(
                "           INVOKE {} 'PlayAnimation'\n", ctrl.id));
            out.push_str(&format!(
                "               USING BY VALUE \"{}\".\n\n", anim.name));
        }
    }
}

// ── SqlDatabase stub generator (Phase 8) ─────────────────────────────────────
//
// Generates ready-to-run COBOL paragraphs that use the COBOL-OPEN-DB,
// COBOL-EXEC-SQL, COBOL-FETCH-ROW, COBOL-NEXT-ROW, and COBOL-CLOSE-DB
// built-in CALLs provided by the cobolt-runtime database engine.

fn write_sql_stubs(out: &mut String, all_controls: &[&Control]) {
    for ctrl in all_controls.iter().filter(|c| c.control_type == ControlType::SqlDatabase) {
        let id   = ctrl.id.as_str();
        let pfx  = format!("WS-{}", id.to_ascii_uppercase());

        let conn_para  = format!("{id}-CONNECT");
        let exec_para  = format!("{id}-EXEC");
        let fetch_para = format!("{id}-FETCH-ALL");
        let close_para = format!("{id}-CLOSE");

        let connect_ok = ctrl.get_prop("ConnectPara")
            .map(|v| v.as_str().to_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("{id}-ON-CONNECT"));
        let query_done = ctrl.get_prop("QueryCompletePara")
            .map(|v| v.as_str().to_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("{id}-ON-QUERY-DONE"));
        let error_para = ctrl.get_prop("ErrorPara")
            .map(|v| v.as_str().to_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("{id}-ON-ERROR"));

        // ── {id}-CONNECT ───────────────────────────────────────────────────
        // Opens a SQLite connection using the connection string in
        // {pfx}-CONN-STRING.  Stores the handle in {pfx}-HANDLE.
        out.push_str(&format!("       {conn_para}.\n"));
        out.push_str(&format!("      *>  Open a SQLite connection for {id}.\n"));
        out.push_str(&format!("      *>  Connection string is in {pfx}-CONN-STRING.\n"));
        out.push_str(&format!("      *>  On success: {pfx}-HANDLE holds the connection handle.\n"));
        out.push_str(&format!("      *>  On error:   WS-SQL-ERROR contains the message.\n"));
        out.push_str(&format!("           MOVE SPACES TO WS-SQL-ERROR\n"));
        out.push_str(&format!("           CALL \"COBOL-OPEN-DB\"\n"));
        out.push_str(&format!("               USING BY REFERENCE {pfx}-CONN-STRING\n"));
        out.push_str(&format!("                     BY REFERENCE {pfx}-HANDLE\n"));
        out.push_str(&format!("                     BY REFERENCE WS-SQL-ERROR\n"));
        out.push_str(&format!("           IF WS-SQL-ERROR NOT = SPACES\n"));
        out.push_str(&format!("               PERFORM {error_para}\n"));
        out.push_str(&format!("           ELSE\n"));
        out.push_str(&format!("               PERFORM {connect_ok}\n"));
        out.push_str(&format!("           END-IF.\n"));
        out.push('\n');

        // ── {id}-EXEC ──────────────────────────────────────────────────────
        // Executes the SQL in WS-SQL-QUERY.
        // Row count / affected rows → WS-SQL-ROW-COUNT.
        out.push_str(&format!("       {exec_para}.\n"));
        out.push_str(&format!("      *>  Execute WS-SQL-QUERY via {id}.\n"));
        out.push_str(&format!("      *>  Stores row count in WS-SQL-ROW-COUNT.\n"));
        out.push_str(&format!("      *>  Resets WS-SQL-MORE to 'Y' if rows are present.\n"));
        out.push_str(&format!("           MOVE SPACES TO WS-SQL-ERROR\n"));
        out.push_str(&format!("           CALL \"COBOL-EXEC-SQL\"\n"));
        out.push_str(&format!("               USING BY REFERENCE {pfx}-HANDLE\n"));
        out.push_str(&format!("                     BY REFERENCE WS-SQL-QUERY\n"));
        out.push_str(&format!("                     BY REFERENCE WS-SQL-ROW-COUNT\n"));
        out.push_str(&format!("                     BY REFERENCE WS-SQL-ERROR\n"));
        out.push_str(&format!("           IF WS-SQL-ERROR NOT = SPACES\n"));
        out.push_str(&format!("               PERFORM {error_para}\n"));
        out.push_str(&format!("           ELSE\n"));
        out.push_str(&format!("               IF WS-SQL-ROW-COUNT > 0\n"));
        out.push_str(&format!("                   MOVE 'Y' TO WS-SQL-MORE\n"));
        out.push_str(&format!("               ELSE\n"));
        out.push_str(&format!("                   MOVE 'N' TO WS-SQL-MORE\n"));
        out.push_str(&format!("               END-IF\n"));
        out.push_str(&format!("               PERFORM {query_done}\n"));
        out.push_str(&format!("           END-IF.\n"));
        out.push('\n');

        // ── {id}-FETCH-ALL ─────────────────────────────────────────────────
        // Template loop — user copies this and adds their own MOVE/COMPUTE
        // statements to read column values from WS-SQL-CURRENT-VALUE.
        out.push_str(&format!("       {fetch_para}.\n"));
        out.push_str(&format!("      *>  Iterate over all rows returned by {id}-EXEC.\n"));
        out.push_str(&format!("      *>  Copy this paragraph and add column reads inside the loop.\n"));
        out.push_str(&format!("      *>  Example:\n"));
        out.push_str(&format!("      *>    MOVE 1 TO WS-SQL-COL-INDEX\n"));
        out.push_str(&format!("      *>    CALL \"COBOL-FETCH-ROW\" USING {pfx}-HANDLE\n"));
        out.push_str(&format!("      *>                                   WS-SQL-COL-INDEX\n"));
        out.push_str(&format!("      *>                                   WS-SQL-CURRENT-VALUE\n"));
        out.push_str(&format!("      *>                                   WS-SQL-ERROR\n"));
        out.push_str(&format!("      *>    MOVE WS-SQL-CURRENT-VALUE TO WS-MY-NAME-FIELD\n"));
        out.push_str(&format!("           PERFORM UNTIL WS-SQL-MORE = 'N'\n"));
        out.push_str(&format!("               MOVE 1 TO WS-SQL-COL-INDEX\n"));
        out.push_str(&format!("               CALL \"COBOL-FETCH-ROW\"\n"));
        out.push_str(&format!("                   USING BY REFERENCE {pfx}-HANDLE\n"));
        out.push_str(&format!("                         BY REFERENCE WS-SQL-COL-INDEX\n"));
        out.push_str(&format!("                         BY REFERENCE WS-SQL-CURRENT-VALUE\n"));
        out.push_str(&format!("                         BY REFERENCE WS-SQL-ERROR\n"));
        out.push_str(&format!("      *>          MOVE WS-SQL-CURRENT-VALUE TO your-field-here\n"));
        out.push_str(&format!("               CONTINUE\n"));
        out.push_str(&format!("               CALL \"COBOL-NEXT-ROW\"\n"));
        out.push_str(&format!("                   USING BY REFERENCE {pfx}-HANDLE\n"));
        out.push_str(&format!("                         BY REFERENCE WS-SQL-MORE\n"));
        out.push_str(&format!("           END-PERFORM.\n"));
        out.push('\n');

        // ── {id}-CLOSE ─────────────────────────────────────────────────────
        out.push_str(&format!("       {close_para}.\n"));
        out.push_str(&format!("      *>  Close the SQLite connection for {id}.\n"));
        out.push_str(&format!("           CALL \"COBOL-CLOSE-DB\"\n"));
        out.push_str(&format!("               USING BY REFERENCE {pfx}-HANDLE.\n"));
        out.push('\n');

        // ── user event handler stubs ───────────────────────────────────────
        for para in &[&connect_ok, &query_done, &error_para] {
            out.push_str(&format!("       {para}.\n"));
            out.push_str(&format!("      *>  TODO: add your {para} logic here.\n"));
            out.push_str("           CONTINUE.\n");
            out.push('\n');
        }
    }
}

// ── ModalWindow open / close stub generator ───────────────────────────────────

fn write_modal_stubs(out: &mut String, all_controls: &[&Control]) {
    for ctrl in all_controls.iter().filter(|c| c.control_type == ControlType::ModalWindow) {
        let pfx       = format!("WS-{}", ctrl.id.replace('-', "-"));
        let open_para = ctrl.get_prop("OpenPara")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| format!("{}-OPEN", ctrl.id));
        let closed_para = ctrl.get_prop("ClosedPara")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| format!("{}-CLOSED", ctrl.id));
        let confirmed_para = ctrl.get_prop("ConfirmedPara")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| format!("{}-CONFIRMED", ctrl.id));
        let cancelled_para = ctrl.get_prop("CancelledPara")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| format!("{}-CANCELLED", ctrl.id));
        let prog = ctrl.get_prop("ProgramName")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| ctrl.id.clone());
        let shared = ctrl.get_prop("SharedDataItems")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();

        // Open paragraph
        out.push_str(&format!("       {}.\n", open_para));
        out.push_str(&format!(
            "      *>    Open modal window {} (COBOL program: {}).\n",
            ctrl.id, prog
        ));
        if !shared.is_empty() {
            out.push_str("      *>    Shared data items passed BY REFERENCE:\n");
            for item in shared.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                out.push_str(&format!("      *>        {}\n", item));
            }
            // Build CALL USING clause
            let using_clause: String = shared
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| format!("\n                     BY REFERENCE {}", s))
                .collect::<Vec<_>>()
                .join("");
            out.push_str(&format!(
                "           CALL \"{prog}\" USING BY VALUE FORM-NAME{using}\n",
                prog = prog, using = using_clause
            ));
        } else {
            out.push_str(&format!(
                "           CALL \"{prog}\" USING BY VALUE FORM-NAME\n",
                prog = prog
            ));
        }
        out.push_str(&format!(
            "           MOVE COBOL-MODAL-RESULT TO {pfx}-RESULT\n"
        ));
        out.push_str("           EVALUATE TRUE\n");
        out.push_str(&format!("               WHEN {pfx}-RESULT = \"Confirmed\"\n"));
        out.push_str(&format!("                   PERFORM {}\n", confirmed_para));
        out.push_str(&format!("               WHEN {pfx}-RESULT = \"Cancelled\"\n"));
        out.push_str(&format!("                   PERFORM {}\n", cancelled_para));
        out.push_str("               WHEN OTHER\n");
        out.push_str(&format!("                   PERFORM {}\n", closed_para));
        out.push_str("           END-EVALUATE.\n");
        out.push('\n');

        write_stub_paragraph(
            out, &closed_para,
            &format!("{} closed — {}-RESULT contains the outcome", ctrl.id, pfx),
        );
        write_stub_paragraph(
            out, &confirmed_para,
            &format!("{} confirmed by user", ctrl.id),
        );
        write_stub_paragraph(
            out, &cancelled_para,
            &format!("{} cancelled by user", ctrl.id),
        );
    }
}

// ── Chart INVOKE verb paragraph generator ─────────────────────────────────────

fn write_chart_stubs(out: &mut String, all_controls: &[&Control]) {
    let chart_types = [
        ControlType::BarChart,
        ControlType::LineChart,
        ControlType::PieChart,
        ControlType::AreaChart,
        ControlType::ScatterChart,
        ControlType::DonutChart,
    ];
    let charts: Vec<&&Control> = all_controls
        .iter()
        .filter(|c| chart_types.contains(&c.control_type))
        .collect();
    if charts.is_empty() { return; }

    out.push_str("      *> ── Chart INVOKE verb paragraphs ─────────────────────────────────\n");
    out.push('\n');

    for ctrl in charts {
        let id  = &ctrl.id;
        let ws  = format!("WS-{}", id);
        let ds  = ctrl.get_prop("DataSource")
                      .map(|v| v.as_str().to_owned())
                      .filter(|s| !s.is_empty())
                      .unwrap_or_else(|| format!("WS-{}-TABLE", id));
        let cnt = ctrl.get_prop("DataCount")
                      .map(|v| v.as_str().to_owned())
                      .filter(|s| !s.is_empty())
                      .unwrap_or_else(|| format!("WS-{}-COUNT", id));

        // ── SET-TABLE ────────────────────────────────────────────────────────
        out.push_str(&format!("       {id}-SET-TABLE.\n"));
        out.push_str(&format!(
            "      *>    Bind a COBOL table to {id}.\n"
        ));
        out.push_str(&format!(
            "      *>    Usage: INVOKE {id} SET-TABLE USING {ds} {cnt}\n"
        ));
        out.push_str(&format!(
            "           MOVE {cnt}        TO {ws}-SELECTED-IDX\n"
        ));
        out.push_str(&format!(
            "           CALL \"COBOL-CHART-SET-TABLE\" USING \"{id}\" {ds} {cnt}\n"
        ));
        out.push_str("           CONTINUE.\n");
        out.push('\n');

        // ── ADD-POINT ────────────────────────────────────────────────────────
        out.push_str(&format!("       {id}-ADD-POINT.\n"));
        out.push_str(&format!(
            "      *>    Append a single data point to {id}.\n"
        ));
        out.push_str(&format!(
            "      *>    Usage: INVOKE {id} ADD-POINT USING WS-LABEL WS-VALUE\n"
        ));
        out.push_str(&format!(
            "           CALL \"COBOL-CHART-ADD-POINT\" USING \"{id}\" {ws}-SELECTED-LBL {ws}-SELECTED-VAL\n"
        ));
        out.push_str("           CONTINUE.\n");
        out.push('\n');

        // ── CLEAR ────────────────────────────────────────────────────────────
        out.push_str(&format!("       {id}-CLEAR.\n"));
        out.push_str(&format!(
            "      *>    Remove all data series from {id}.\n"
        ));
        out.push_str(&format!(
            "      *>    Usage: INVOKE {id} CLEAR\n"
        ));
        out.push_str(&format!(
            "           CALL \"COBOL-CHART-CLEAR\" USING \"{id}\"\n"
        ));
        out.push_str("           CONTINUE.\n");
        out.push('\n');

        // ── REFRESH ──────────────────────────────────────────────────────────
        out.push_str(&format!("       {id}-REFRESH.\n"));
        out.push_str(&format!(
            "      *>    Force {id} to redraw with current data.\n"
        ));
        out.push_str(&format!(
            "      *>    Usage: INVOKE {id} REFRESH\n"
        ));
        out.push_str(&format!(
            "           CALL \"COBOL-CHART-REFRESH\" USING \"{id}\"\n"
        ));
        out.push_str("           CONTINUE.\n");
        out.push('\n');
    }
}

// ── Nested COBOL-85 program generator ────────────────────────────────────────

/// Emit one nested `PROGRAM-ID ... END PROGRAM` block per event handler.
/// Form-level OnLoad / OnClose come first, then per-control events.
fn write_nested_programs(out: &mut String, form: &Form, all_controls: &[&Control]) {
    let has_any = !form.form_events.is_empty()
        || all_controls.iter().any(|c| !c.events.is_empty());
    if !has_any { return; }

    out.push_str("\n      *> ── Nested event-handler programs (COBOL-85) ─────────────────────\n");
    out.push('\n');

    // Form-level events: OnLoad, OnClose
    for ev in &form.form_events {
        write_nested_program(out, &ev.paragraph, &ev.code, &ev.local_ws,
            &format!("Form {} handler", ev.event));
    }

    // Per-control events
    for ctrl in all_controls {
        for ev in &ctrl.events {
            write_nested_program(out, &ev.paragraph, &ev.code, &ev.local_ws,
                &format!("{} {} handler", ctrl.id, ev.event));
        }
    }
}

/// Emit a single COBOL-85 nested program.
///
/// If `code` is non-empty it is emitted verbatim as the procedure body.
/// Otherwise a `CONTINUE` stub is written so the generated file compiles cleanly.
/// `local_ws` is emitted in the WORKING-STORAGE SECTION when non-empty.
fn write_nested_program(out: &mut String, prog_id: &str, code: &str, local_ws: &str, comment: &str) {
    out.push_str("       IDENTIFICATION DIVISION.\n");
    out.push_str(&format!("       PROGRAM-ID. {}.\n", prog_id));
    out.push('\n');
    out.push_str("       DATA DIVISION.\n");
    out.push_str("       WORKING-STORAGE SECTION.\n");
    let ws_trimmed = local_ws.trim();
    if ws_trimmed.is_empty() {
        out.push_str("      *>    Add LOCAL or EXTERNAL data items here.\n");
    } else {
        for line in ws_trimmed.lines() {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push('\n');
    out.push_str("       PROCEDURE DIVISION.\n");

    let trimmed = code.trim();
    if trimmed.is_empty() {
        out.push_str(&format!("      *>    TODO: {}\n", comment));
        out.push_str("           CONTINUE.\n");
    } else {
        // Emit user's COBOL verbatim.
        for line in trimmed.lines() {
            out.push_str(line);
            out.push('\n');
        }
    }

    out.push_str("           GOBACK.\n");
    out.push('\n');
    out.push_str(&format!("       END PROGRAM {}.\n", prog_id));
    out.push('\n');
}

fn write_event_loop(out: &mut String, form: &Form) {
    out.push_str("       COBOL-EVENT-LOOP.\n");
    out.push_str("           PERFORM UNTIL COBOL-QUIT = 1\n");
    out.push_str("               CALL \"COBOL-WAIT-EVENT\"\n");
    out.push_str("                   USING COBOL-EVENT-ID COBOL-CONTROL-ID\n");

    let all_controls = collect_all_controls(&form.controls);
    let controls_with_events: Vec<_> = all_controls
        .iter()
        .filter(|c| !c.events.is_empty())
        .collect();

    if controls_with_events.is_empty() {
        // No events — nothing to dispatch.
        out.push_str("               *> No event handlers defined yet.\n");
        out.push_str("               CONTINUE\n");
    } else {
        out.push_str("               EVALUATE COBOL-CONTROL-ID\n");
        for ctrl in &controls_with_events {
            out.push_str(&format!("                   WHEN \"{}\"\n", ctrl.id));
            out.push_str("                       EVALUATE COBOL-EVENT-ID\n");
            for ev in &ctrl.events {
                out.push_str(&format!(
                    "                           WHEN \"{}\"\n",
                    ev.event
                ));
                // Dispatch to nested program via CALL (not PERFORM)
                out.push_str(&format!(
                    "                               CALL \"{}\"\n",
                    ev.paragraph
                ));
            }
            out.push_str("                       END-EVALUATE\n");
        }
        out.push_str("               END-EVALUATE\n");
    }

    out.push_str("           END-PERFORM.\n");
    out.push('\n');
}

fn write_stub_paragraph(out: &mut String, name: &str, comment: &str) {
    out.push_str(&format!("       {}.\n", name));
    out.push_str(&format!("      *>    TODO: {}\n", comment));
    out.push_str("           CONTINUE.\n");
    out.push('\n');
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Flatten a nested control tree into a pre-order Vec.
fn collect_all_controls(controls: &[Control]) -> Vec<&Control> {
    let mut result = Vec::new();
    for ctrl in controls {
        collect_rec(ctrl, &mut result);
    }
    result
}

fn collect_rec<'a>(ctrl: &'a Control, out: &mut Vec<&'a Control>) {
    out.push(ctrl);
    for child in &ctrl.children {
        collect_rec(child, out);
    }
}

/// Which property key holds the display text for a given control type.
fn caption_prop_key(ct: &ControlType) -> &'static str {
    match ct {
        ControlType::Label       => "Caption",
        ControlType::Button      => "Caption",
        ControlType::CheckBox    => "Caption",
        ControlType::RadioButton => "Caption",
        ControlType::GroupBox    => "Caption",
        ControlType::TextBox     => "Text",
        ControlType::ComboBox    => "Text",
        ControlType::ListBox     => "Text",
        _                        => "Caption",
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::{Control, ControlType, EventBinding, Form};

    fn make_form() -> Form {
        let mut form = Form::new("MAIN-FORM", "Test", 800, 600);

        let mut btn = Control::new("BTN-OK", ControlType::Button, 10, 10);
        btn.events.push(EventBinding::for_control("BTN-OK", "Click"));
        form.controls.push(btn);

        form
    }

    #[test]
    fn generate_contains_program_id() {
        let src = generate(&make_form());
        assert!(src.contains("PROGRAM-ID. MAIN-FORM."), "missing PROGRAM-ID");
    }

    #[test]
    fn generate_contains_event_loop() {
        let src = generate(&make_form());
        assert!(src.contains("COBOL-EVENT-LOOP"), "missing event loop paragraph");
        assert!(src.contains("WHEN \"BTN-OK\""), "missing control WHEN");
        assert!(src.contains("WHEN \"Click\""), "missing event WHEN");
        // v1.0.0: dispatch via CALL to nested program (double-hyphen name)
        assert!(src.contains("CALL \"BTN-OK--CLICK\""), "missing nested CALL dispatch");
    }

    #[test]
    fn generate_contains_data_group() {
        let src = generate(&make_form());
        assert!(src.contains("01 WS-BTN-OK."), "missing WS group for BTN-OK");
    }

    #[test]
    fn generate_contains_nested_program() {
        let src = generate(&make_form());
        // v1.0.0: event handlers are nested COBOL-85 programs
        assert!(src.contains("PROGRAM-ID. BTN-OK--CLICK."), "missing nested program ID");
        assert!(src.contains("END PROGRAM BTN-OK--CLICK."), "missing END PROGRAM for handler");
        assert!(src.contains("GOBACK."), "missing GOBACK in nested program");
        assert!(src.contains("END PROGRAM MAIN-FORM."), "missing outer END PROGRAM");
    }

    #[test]
    fn generate_contains_form_events_nested() {
        let src = generate(&make_form());
        // Form events (OnLoad / OnClose) also become nested programs
        // Paragraph names come from `derive_paragraph_name`, which uppercases the
        // event name without inserting separators: "OnLoad" → "ONLOAD".
        assert!(src.contains("PROGRAM-ID. MAIN-FORM--ONLOAD."), "missing OnLoad nested program");
        assert!(src.contains("PROGRAM-ID. MAIN-FORM--ONCLOSE."), "missing OnClose nested program");
    }

    #[test]
    fn generate_calls_on_load_nested() {
        let src = generate(&make_form());
        // Form::new() pre-populates form_events with OnLoad; COBOL-MAIN must CALL it.
        assert!(src.contains("CALL \"MAIN-FORM--ONLOAD\""), "missing OnLoad CALL in COBOL-MAIN");
    }

    /// `regenerate` is now a clean alias for `generate` — all event code lives in the
    /// form model, so the existing .cbl content is irrelevant and ignored.
    #[test]
    fn regenerate_equals_generate() {
        let form = make_form();
        // Pass a non-empty "existing" source — it must be completely ignored.
        let existing = "       STALE-PARA.\n           CONTINUE.\n";
        assert_eq!(
            regenerate(&form, existing),
            generate(&form),
            "regenerate must return the same output as generate regardless of existing_source"
        );
    }

    /// Event-handler code stored in the model is emitted into the nested program body.
    #[test]
    fn generate_emits_event_handler_code() {
        let mut form = Form::new("MAIN-FORM", "Test", 800, 600);
        let mut btn = Control::new("BTN-OK", ControlType::Button, 10, 10);
        let mut ev = EventBinding::for_control("BTN-OK", "Click");
        ev.code = "           MOVE 1 TO COBOL-QUIT".into();
        btn.events.push(ev);
        form.controls.push(btn);

        let src = generate(&form);
        assert!(src.contains("MOVE 1 TO COBOL-QUIT"),
            "handler code from EventBinding must appear in nested program body");
    }

    /// local_ws declared in EventBinding appears in the handler's WS section.
    #[test]
    fn generate_emits_local_ws() {
        let mut form = Form::new("MAIN-FORM", "Test", 800, 600);
        let mut btn = Control::new("BTN-OK", ControlType::Button, 10, 10);
        let mut ev = EventBinding::for_control("BTN-OK", "Click");
        ev.local_ws = "       01 WS-LOCAL-FLAG  PIC 9 VALUE 0.".into();
        btn.events.push(ev);
        form.controls.push(btn);

        let src = generate(&form);
        assert!(src.contains("WS-LOCAL-FLAG"),
            "local_ws from EventBinding must appear in nested program WORKING-STORAGE");
    }
}
