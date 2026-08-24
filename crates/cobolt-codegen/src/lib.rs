// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Code generation: converts a [`cobolt_forms::Form`] into a complete COBOL source file.
//!
//! # Architecture (v1.0 — COBOL-85 nested-program model)
//!
//! The `.cfrm` file is the **single source of truth**.  The generated `.cbl` is a
//! build artifact — it is never edited by hand.  Each event handler's full body
//! lives in [`cobolt_forms::EventBinding::code`] (from `ENVIRONMENT DIVISION`
//! through `PROCEDURE DIVISION`), stored in the form file and edited through the
//! modal code editor in the Form Designer.
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
//!                      WHEN "onClick"
//!                          CALL "BTN-OK--ONCLICK"
//!                  END-EVALUATE
//!          END-EVALUATE
//!      END-PERFORM.
//!
//!  *> ── Nested event-handler programs (COBOL-85) ────────────
//!       IDENTIFICATION DIVISION.
//!       PROGRAM-ID. BTN-OK--ONCLICK.
//!       *>    (full handler body from EventBinding.code goes here:
//!       *>     ENVIRONMENT / DATA / WORKING-STORAGE / LINKAGE /
//!       *>     PROCEDURE DIVISION + statements)
//!           GOBACK.
//!       END PROGRAM BTN-OK--ONCLICK.
//!
//!       END PROGRAM MAIN-FORM.
//! ```

use cobolt_forms::code_site::{CodeSite, StructureSection};
use cobolt_forms::model::PropValue;
use cobolt_forms::{Control, ControlType, Form};

pub mod data_binding;
pub mod indexed;
pub use indexed::{generate_indexed, generate_indexed_fd, generate_indexed_select};

// ── Public API ────────────────────────────────────────────────────────────────

/// Turn a control id into a valid COBOL user-defined word.
///
/// A COBOL word may only hold letters, digits and hyphens, and may neither
/// begin nor end with a hyphen. Control ids are free-form — the designer and
/// the assistant both happily produce `textbox_1` or `label_result` — and
/// injecting one verbatim used to emit `WS-TEXTBOX_1-TEXT`, which the lexer
/// reads as `WS-TEXTBOX`, an error token for `_`, then `1`. The whole data
/// item was skipped ("skipping unknown data clause"), so the control had no
/// storage at all. Every character that is not a letter or digit therefore
/// becomes a hyphen, runs of hyphens collapse, and the ends are trimmed.
///
/// The id's own case is kept — COBOL words are case-insensitive, so there is
/// nothing to gain from shouting, and the generated source keeps reading like
/// the form the developer designed. The `WS-` prefix the callers add
/// guarantees the leading alphabetic character a data-name needs.
pub fn cobol_word(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for ch in id.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_owned()
}

// ── Source map (spec 053) ─────────────────────────────────────────────────────

/// One contiguous run of generated lines that came verbatim from a developer
/// [`CodeSite`] (spec 053 R6). Line numbers are 1-based and inclusive, in the
/// generated `.cbl`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MappedSpan {
    /// The place the developer wrote these lines.
    pub site: CodeSite,
    /// First generated line of the span.
    pub gen_start: u32,
    /// Last generated line of the span (inclusive).
    pub gen_end: u32,
    /// Which line **of the site's own text** `gen_start` holds. Usually 1 —
    /// but the WORKING-STORAGE weaver skips leading blank lines, so generated
    /// line *N* is NOT always site line *N − gen_start + 1*; this field
    /// records the true offset once instead of every consumer re-deriving it.
    pub site_line_at_start: u32,
}

/// The map from generated `.cbl` lines back to the code sites that produced
/// them (spec 053 R6). A line no span covers was authored by codegen itself.
///
/// Derives serde (spec Q6) so a later runtime-locations spec can ship it into
/// a compiled binary without a retrofit.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceMap {
    pub spans: Vec<MappedSpan>,
}

impl SourceMap {
    /// The site that produced `gen_line` and the 1-based line **within that
    /// site's text**, or `None` for a line codegen authored (R12).
    pub fn resolve(&self, gen_line: u32) -> Option<(&CodeSite, u32)> {
        self.spans
            .iter()
            .find(|s| gen_line >= s.gen_start && gen_line <= s.gen_end)
            .map(|s| (&s.site, s.site_line_at_start + (gen_line - s.gen_start)))
    }

    fn record(&mut self, site: CodeSite, gen_start: u32, gen_end: u32, site_line_at_start: u32) {
        self.spans.push(MappedSpan {
            site,
            gen_start,
            gen_end,
            site_line_at_start,
        });
    }

    /// The handler/procedure **body** ranges — exactly what
    /// [`generate_with_user_lines`] has always returned; structure-section
    /// spans are not bodies and are excluded.
    pub fn user_body_ranges(&self) -> Vec<(u32, u32)> {
        self.spans
            .iter()
            .filter(|s| {
                matches!(
                    s.site,
                    CodeSite::ControlEvent { .. }
                        | CodeSite::FormEvent { .. }
                        | CodeSite::Procedure { .. }
                )
            })
            .map(|s| (s.gen_start, s.gen_end))
            .collect()
    }
}

/// Generate a complete COBOL source skeleton from `form`.
///
/// Returns a `String` containing fixed-format COBOL source code.
pub fn generate(form: &Form) -> String {
    generate_with_map(form).0
}

/// Like [`generate`], but also returns the **user-code line map**: the 1-based,
/// inclusive line ranges of the generated `.cbl` that hold code the developer
/// authored directly — each event handler's body and each user procedure's body
/// (the `EventBinding::code` / `UserProcedure::code` verbatim blocks). Everything
/// outside these ranges is IDE-generated scaffolding.
///
/// The debugger uses this to optionally hide/skip generated code and stop only in
/// the developer's own handlers. Empty (unwritten) handlers contribute no range —
/// their body is a generated template stub, not user code.
///
/// A thin wrapper over [`generate_with_map`] — one recording path, so the
/// debugger's view of "user code" and the diagnostics' view are the same fact.
pub fn generate_with_user_lines(form: &Form) -> (String, Vec<(u32, u32)>) {
    let (out, map) = generate_with_map(form);
    let user_lines = map.user_body_ranges();
    (out, user_lines)
}

/// The generation primitive (spec 053 R6/R9): the source **and** the
/// [`SourceMap`] from its lines back to the developer's code sites, produced
/// by the same call so the two cannot drift. Producing the map changes the
/// generated COBOL by nothing — the map is a return value, never text in the
/// file (R8, pinned by the `generated_bytes_golden` test).
pub fn generate_with_map(form: &Form) -> (String, SourceMap) {
    let mut out = String::with_capacity(4096);
    let mut map = SourceMap::default();

    write_header(&mut out);
    write_identification(&mut out, form);
    write_environment(&mut out, form, &mut map);
    write_data_division(&mut out, form, &mut map);
    write_procedure_division(&mut out, form, &mut map);

    (out, map)
}

/// 1-based number of the line that the NEXT character pushed to `out` will start.
fn next_line_number(out: &str) -> u32 {
    out.matches('\n').count() as u32 + 1
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

/// Banner comment addressed to the developer, emitted at the very top of every
/// generated source file (GOLDEN RULE). Uses `*>` floating comments so it is
/// ignored by the compiler.
fn write_header(out: &mut String) {
    out.push_str("      *> ───────────────────────────────────────────────────────────\n");
    out.push_str("      *>  This code was generated automatically by PowerRustCOBOL RAD.\n");
    out.push_str("      *>\n");
    out.push_str("      *>  DO NOT MODIFY IT DIRECTLY: it is regenerated the next time\n");
    out.push_str("      *>  you interact with the Form Designer, so manual edits are lost.\n");
    out.push_str("      *>  Edit the form and its event handlers in the Form Designer\n");
    out.push_str("      *>  instead.\n");
    out.push_str("      *>\n");
    out.push_str("      *>  PowerRustCOBOL may change the structure of this generated code\n");
    out.push_str("      *>  at any time — without breaking your code's functionality — for\n");
    out.push_str("      *>  reasons such as performance improvements, new observability\n");
    out.push_str("      *>  features, and bug fixes.\n");
    out.push_str("      *>\n");
    out.push_str("      *>  PowerRustCOBOL and its components are distributed under the\n");
    out.push_str("      *>  Apache 2.0 License.\n");
    out.push_str("      *> ───────────────────────────────────────────────────────────\n");
    out.push('\n');
}

fn write_identification(out: &mut String, form: &Form) {
    out.push_str("       IDENTIFICATION DIVISION.\n");
    out.push_str(&format!("       PROGRAM-ID. {}.\n", form.name));
    out.push('\n');
}

/// Append a fixed section/paragraph header and the developer's verbatim block
/// body, only when the body is non-empty (spec 005 COBOL Structure).
///
/// The one chokepoint for the four woven structure sections, so it records the
/// span they occupy in the generated source (spec 053 R6/R7). Leading blank
/// lines are kept, so the span starts at site line 1.
fn weave_block(
    out: &mut String,
    header: &str,
    body: &str,
    section: StructureSection,
    map: &mut SourceMap,
) {
    let body = body.trim_end();
    if body.trim().is_empty() {
        return;
    }
    out.push_str(header);
    out.push('\n');
    let gen_start = next_line_number(out);
    out.push_str(body);
    // A COBOL paragraph must terminate with a period. If the author didn't write
    // one (e.g. a REPOSITORY whose last `CLASS … IS "…"` entry has no period),
    // supply one right after the last character so the generated code is valid.
    if !body.ends_with('.') {
        out.push('.');
    }
    out.push('\n');
    let gen_end = gen_start + body.lines().count().max(1) as u32 - 1;
    map.record(CodeSite::Section(section), gen_start, gen_end, 1);
}

fn write_environment(out: &mut String, form: &Form, map: &mut SourceMap) {
    out.push_str("       ENVIRONMENT DIVISION.\n");

    // ── COBOL Structure: CONFIGURATION / INPUT-OUTPUT (spec 005) ──────────────
    let cs = &form.cobol_structure;
    if !cs.special_names.trim().is_empty() || !cs.repository.trim().is_empty() {
        out.push_str("       CONFIGURATION SECTION.\n");
        weave_block(
            out,
            "       SPECIAL-NAMES.",
            &cs.special_names,
            StructureSection::SpecialNames,
            map,
        );
        weave_block(
            out,
            "       REPOSITORY.",
            &cs.repository,
            StructureSection::Repository,
            map,
        );
    }
    if !cs.file_control.trim().is_empty() {
        out.push_str("       INPUT-OUTPUT SECTION.\n");
        weave_block(
            out,
            "       FILE-CONTROL.",
            &cs.file_control,
            StructureSection::FileControl,
            map,
        );
    }
    out.push('\n');
}

fn write_data_division(out: &mut String, form: &Form, map: &mut SourceMap) {
    out.push_str("       DATA DIVISION.\n");
    // ── COBOL Structure: FILE SECTION (spec 005) — precedes WORKING-STORAGE ───
    weave_block(
        out,
        "       FILE SECTION.",
        &form.cobol_structure.file_section,
        StructureSection::FileSection,
        map,
    );
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

    // Declare the array index var in main WS if any member control has events.
    // The runtime will populate it from the incoming FormEvent's instance_index
    // before the EVALUATE/CALL in the event loop.
    if all_controls
        .iter()
        .any(|c| form.array_binding_context_for_member(&c.id).is_some() && !c.events.is_empty())
    {
        out.push_str("       01 CONTROL-ARRAY-INDEX     PIC S9(4) COMP-5 VALUE 0.\n");
    }

    // ── REST / HTTP infrastructure (emitted when any RestClient exists) ─────
    let has_rest = all_controls
        .iter()
        .any(|c| c.control_type == ControlType::RestClient);
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
    let has_anims =
        all_controls.iter().any(|c| !c.animations.is_empty()) || !form.animations.is_empty();
    if has_anims {
        out.push_str("      *>── Animation runtime fields ──────────────────────────────────\n");
        out.push_str("      *>   INVOKE ctrl-id 'PlayAnimation' USING BY VALUE WS-ANIM-NAME\n");
        out.push_str("       01 WS-ANIM-NAME          PIC X(128)  VALUE SPACES.\n");
        out.push_str("       01 WS-ANIM-ELAPSED-MS    PIC 9(8)    VALUE 0.\n");
        out.push('\n');
    }

    // ── Agent infrastructure ───────────────────────────────────────────────
    let has_agents = all_controls
        .iter()
        .any(|c| c.control_type == ControlType::AgentObject);
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
    for ctrl in all_controls
        .iter()
        .filter(|c| c.control_type == ControlType::RestClient)
    {
        let pfx = format!("WS-{}", cobol_word(&ctrl.id));
        let base = ctrl
            .get_prop("BaseURL")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();
        out.push_str(&format!(
            "      *>── REST client: {} ──────────────────────────────────\n",
            ctrl.id
        ));
        out.push_str(&format!(
            "       01 {}-BASE-URL      PIC X(2048) VALUE '{}'.\n",
            pfx, base
        ));
        let resp_item = ctrl
            .get_prop("ResponseDataItem")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();
        if !resp_item.is_empty() {
            out.push_str(&format!(
                "       01 {}             PIC X(32767) VALUE SPACES.\n",
                resp_item
            ));
        }
        let status_item = ctrl
            .get_prop("StatusDataItem")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();
        if !status_item.is_empty() {
            out.push_str(&format!(
                "       01 {}         PIC 9(4) VALUE 0.\n",
                status_item
            ));
        }
        out.push('\n');
    }

    // ── Per-WebSearch instance fields (spec 039 T14) ─────────────────────
    // Modelled on the RestClient block above. No key field here — the
    // resolved "google-custom-search" key (T7) is a runtime-only seed
    // property (R30/R31), never a design-time literal.
    for ctrl in all_controls
        .iter()
        .filter(|c| c.control_type == ControlType::WebSearch)
    {
        let pfx = format!("WS-{}", cobol_word(&ctrl.id));
        let engine_id = ctrl
            .get_prop("SearchEngineId")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();
        let num_results = ctrl.get_prop("NumResults").map(|v| v.as_i64()).unwrap_or(10);
        let safe_search = ctrl
            .get_prop("SafeSearch")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "Off".into());
        out.push_str(&format!(
            "      *>── Web Search: {} ──────────────────────────────────\n",
            ctrl.id
        ));
        out.push_str(&format!(
            "       01 {pfx}-SEARCH-ENGINE-ID PIC X(64)  VALUE '{engine_id}'.\n"
        ));
        out.push_str(&format!(
            "       01 {pfx}-QUERY            PIC X(512) VALUE SPACES.\n"
        ));
        out.push_str(&format!(
            "       01 {pfx}-NUM-RESULTS      PIC 9(2)   VALUE {num_results}.\n"
        ));
        out.push_str(&format!(
            "       01 {pfx}-SAFE-SEARCH      PIC X(6)   VALUE '{safe_search}'.\n"
        ));
        out.push('\n');
    }

    // ── DataGrid CSV fields ───────────────────────────────────────────────
    for ctrl in all_controls
        .iter()
        .filter(|c| c.control_type == ControlType::DataGrid)
    {
        if datagrid_csv_export_enabled(ctrl) {
            let pfx = format!("WS-{}", cobol_word(&ctrl.id));
            out.push_str(&format!(
                "      *>── DataGrid {} CSV export ──────────────────────────\n",
                ctrl.id
            ));
            out.push_str(&format!(
                "       01 {}-CSV-PATH    PIC X(512)  VALUE SPACES.\n",
                pfx
            ));
            out.push_str(&format!(
                "       01 {}-CSV-STATUS  PIC 9       VALUE 0.\n",
                pfx
            ));
            out.push('\n');
        }
    }

    // ── SQL Database infrastructure (Phase 8) ────────────────────────────
    let has_sql = all_controls
        .iter()
        .any(|c| c.control_type == ControlType::SqlDatabase);
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

    for ctrl in all_controls
        .iter()
        .filter(|c| c.control_type == ControlType::SqlDatabase)
    {
        let pfx = format!("WS-{}", cobol_word(&ctrl.id));
        let cs = ctrl
            .get_prop("ConnectionString")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| ":memory:".into());
        let drv = ctrl
            .get_prop("Driver")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "sqlite".into());
        out.push_str(&format!(
            "      *>── SQL instance: {} ({}) ─────────────────────────────────\n",
            ctrl.id, drv
        ));
        out.push_str(&format!(
            "       01 {pfx}-CONN-STRING   PIC X(512)  VALUE '{cs}'.\n"
        ));
        out.push_str(&format!(
            "       01 {pfx}-HANDLE        PIC 9(9)    VALUE 0.\n"
        ));
        out.push_str(&format!(
            "       01 {pfx}-STATUS        PIC X(512)  VALUE SPACES.\n"
        ));
        out.push('\n');
    }

    // ── Timer runtime fields ──────────────────────────────────────────────
    for ctrl in all_controls
        .iter()
        .filter(|c| c.control_type == ControlType::Timer)
    {
        let pfx = format!("WS-{}", cobol_word(&ctrl.id));
        let iv = ctrl
            .get_prop("Interval")
            .map(|v| v.as_i64())
            .unwrap_or(1000);
        let ena = ctrl
            .get_prop("Enabled")
            .map(|v| if v.as_bool() { 1 } else { 0 })
            .unwrap_or(1);
        out.push_str(&format!(
            "      *>── Timer: {} ──────────────────────────────────────────\n",
            ctrl.id
        ));
        out.push_str(&format!(
            "       01 {}-INTERVAL   PIC 9(8) VALUE {}.\n",
            pfx, iv
        ));
        out.push_str(&format!(
            "       01 {}-ENABLED    PIC 9    VALUE {}.\n",
            pfx, ena
        ));
        out.push_str(&format!("       01 {}-ELAPSED-MS PIC 9(8) VALUE 0.\n", pfx));
        out.push('\n');
    }

    // ── Chart working-storage items ───────────────────────────────────────
    let chart_types = [
        ControlType::BarChart,
        ControlType::LineChart,
        ControlType::PieChart,
        ControlType::AreaChart,
        ControlType::ScatterChart,
        ControlType::DonutChart,
    ];
    for ctrl in all_controls
        .iter()
        .filter(|c| chart_types.contains(&c.control_type))
    {
        let pfx = format!("WS-{}", cobol_word(&ctrl.id));
        let ds = ctrl
            .get_prop("DataSource")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();
        let cnt = ctrl
            .get_prop("DataCount")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();
        let kind = ctrl.control_type.as_str();
        out.push_str(&format!(
            "      *>── Chart: {} (type: {}) ─────────────────────────────────────\n",
            ctrl.id, kind
        ));
        out.push_str(&format!(
            "      *>   Data source : {}\n",
            if ds.is_empty() {
                "(none — use INVOKE SET-TABLE or ADD-POINT)"
            } else {
                &ds
            }
        ));
        out.push_str(&format!(
            "      *>   Row count   : {}\n",
            if cnt.is_empty() { "(not set)" } else { &cnt }
        ));
        out.push_str(&format!(
            "       01 {}-SELECTED-IDX PIC 9(6) VALUE 0.\n",
            pfx
        ));
        out.push_str(&format!(
            "       01 {}-SELECTED-LBL PIC X(64) VALUE SPACES.\n",
            pfx
        ));
        out.push_str(&format!(
            "       01 {}-SELECTED-VAL PIC 9(18)V9(6) VALUE ZEROES.\n",
            pfx
        ));
        out.push('\n');
    }

    // ── User Working Storage (raw COBOL from .cfrm) ──────────────────────
    // Preserve each line's own COBOL indentation: only drop leading/trailing
    // blank lines (a blanket `.trim()` would strip the first item's area-A/B
    // columns and break a fixed-format build).
    if !form.user_ws_source.trim().is_empty() {
        out.push_str("      *>── User Working Storage ────────────────────────────────────────\n");
        // This weaver skips the site's leading blank lines, so the first
        // generated line is NOT site line 1 — `site_line_at_start` records the
        // real offset (spec 053 R6) instead of every consumer re-deriving it.
        let gen_start = next_line_number(out);
        let mut skipped: u32 = 0;
        let mut started = false;
        for line in form.user_ws_source.trim_end().lines() {
            if !started && line.trim().is_empty() {
                skipped += 1;
                continue;
            }
            started = true;
            out.push_str(line);
            out.push('\n');
        }
        let gen_end = out.matches('\n').count() as u32; // last WS line written
        if gen_end >= gen_start {
            map.record(
                CodeSite::Section(StructureSection::WorkingStorage),
                gen_start,
                gen_end,
                1 + skipped,
            );
        }
        out.push('\n');
    }

    data_binding::write_data_binding_storage(out, form);

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
    let prefix = format!("WS-{}", cobol_word(&ctrl.id));
    out.push_str(&format!("       01 {}.\n", prefix));

    // Caption / Text property (if present)
    let caption_key = caption_prop_key(&ctrl.control_type);
    let caption_val = ctrl
        .get_prop(caption_key)
        .map(|v| match v {
            PropValue::String(s) => s.clone(),
            PropValue::Int(n) => n.to_string(),
            PropValue::Bool(b) => if *b { "1" } else { "0" }.to_string(),
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
        ControlType::TextBox | ControlType::CheckBox | ControlType::ComboBox | ControlType::ListBox
    ) {
        out.push_str(&format!(
            "          05 {}-VALUE      PIC X(512) VALUE SPACES.\n",
            prefix
        ));
    }

    // Slider: numeric value + min/max/step fields
    if matches!(ctrl.control_type, ControlType::Slider) {
        let val = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0);
        let min = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0);
        let max = ctrl.get_prop("Maximum").map(|v| v.as_i64()).unwrap_or(100);
        let step = ctrl.get_prop("Step").map(|v| v.as_i64()).unwrap_or(10);
        // Each 05 item must start at column 8+ in fixed-format COBOL.
        // Use separate push_str calls so Rust string continuation (`\n\`)
        // does not eat the leading spaces that position the level numbers.
        out.push_str(&format!(
            "          05 {prefix}-VALUE      PIC S9(9) VALUE {val}.\n"
        ));
        out.push_str(&format!(
            "          05 {prefix}-MINIMUM    PIC S9(9) VALUE {min}.\n"
        ));
        out.push_str(&format!(
            "          05 {prefix}-MAXIMUM    PIC S9(9) VALUE {max}.\n"
        ));
        out.push_str(&format!(
            "          05 {prefix}-STEP       PIC S9(9) VALUE {step}.\n"
        ));
    }

    // Knob (spec 039): same shape as Slider — a plain scalar value control,
    // no INVOKE-capable behaviour, so no separate WORKING-STORAGE block or
    // call-stub generator, only these per-instance fields.
    if matches!(ctrl.control_type, ControlType::Knob) {
        let val = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0);
        let min = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0);
        let max = ctrl.get_prop("Maximum").map(|v| v.as_i64()).unwrap_or(100);
        let step = ctrl.get_prop("Step").map(|v| v.as_i64()).unwrap_or(1);
        let default_v = ctrl
            .get_prop("DefaultValue")
            .map(|v| v.as_i64())
            .unwrap_or(0);
        out.push_str(&format!(
            "          05 {prefix}-VALUE      PIC S9(9) VALUE {val}.\n"
        ));
        out.push_str(&format!(
            "          05 {prefix}-MINIMUM    PIC S9(9) VALUE {min}.\n"
        ));
        out.push_str(&format!(
            "          05 {prefix}-MAXIMUM    PIC S9(9) VALUE {max}.\n"
        ));
        out.push_str(&format!(
            "          05 {prefix}-STEP       PIC S9(9) VALUE {step}.\n"
        ));
        out.push_str(&format!(
            "          05 {prefix}-DEFAULT    PIC S9(9) VALUE {default_v}.\n"
        ));
    }

    // Gauge (spec 039): read-only display (R10) — Value/Minimum/Maximum plus
    // the style that picked which egui-elegance widget renders it.
    if matches!(ctrl.control_type, ControlType::Gauge) {
        let val = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0);
        let min = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0);
        let max = ctrl.get_prop("Maximum").map(|v| v.as_i64()).unwrap_or(100);
        let style = ctrl
            .get_prop("GaugeStyle")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "Radial".into());
        out.push_str(&format!(
            "          05 {prefix}-VALUE      PIC S9(9) VALUE {val}.\n"
        ));
        out.push_str(&format!(
            "          05 {prefix}-MINIMUM    PIC S9(9) VALUE {min}.\n"
        ));
        out.push_str(&format!(
            "          05 {prefix}-MAXIMUM    PIC S9(9) VALUE {max}.\n"
        ));
        out.push_str(&format!(
            "          05 {prefix}-STYLE      PIC X(10)  VALUE '{style}'.\n"
        ));
    }

    // Switch (spec 039): a single boolean flag, same PIC 9 shape CheckBox's
    // own -VALUE field already uses for its checked state.
    if matches!(ctrl.control_type, ControlType::Switch) {
        let checked = ctrl
            .get_prop("Checked")
            .map(|v| v.as_bool())
            .unwrap_or(false);
        out.push_str(&format!(
            "          05 {}-CHECKED    PIC 9      VALUE {}.\n",
            prefix,
            if checked { 1 } else { 0 }
        ));
    }

    // FileDropZone (spec 039): DroppedFiles is a newline-joined path list at
    // the model layer (a plain property, like every other control's data),
    // but COBOL has no line-splitting primitive as ergonomic as an indexed
    // table — so this is the one new control in this batch that gets a
    // small dedicated block: a count plus a fixed-size OCCURS table, filled
    // by the runtime's SETFILES-style bridge (interpreter.rs) whenever
    // DroppedFiles changes, not by this static VALUE clause (it always
    // starts empty — DroppedFiles is runtime-only, never a design-time
    // default, per R13).
    if matches!(ctrl.control_type, ControlType::FileDropZone) {
        out.push_str(&format!(
            "          05 {prefix}-FILE-COUNT PIC S9(4) VALUE 0.\n"
        ));
        out.push_str(&format!(
            "          05 {prefix}-FILE-PATH  PIC X(1024) OCCURS 20 TIMES\n"
        ));
        out.push_str("                                      VALUE SPACES.\n");
    }

    // Maps (spec 039 T12): CenterLat/CenterLng/Zoom design-time defaults.
    // No API-key field here at all — R31/R33: the resolved google_maps key
    // never becomes literal generated-source text on the interpreted-Run
    // path (`form_runtime.rs` seeds it as a runtime-only property instead,
    // never written to any .cbl); a standalone compiled binary's own path
    // to the key is a known, documented gap (plan.md §5 risk), not solved
    // by this codegen block.
    if matches!(ctrl.control_type, ControlType::Maps) {
        let lat = ctrl
            .get_prop("CenterLat")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "0".into());
        let lng = ctrl
            .get_prop("CenterLng")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "0".into());
        let zoom = ctrl.get_prop("Zoom").map(|v| v.as_i64()).unwrap_or(2);
        out.push_str(&format!(
            "          05 {prefix}-CENTER-LAT PIC X(32)  VALUE '{lat}'.\n"
        ));
        out.push_str(&format!(
            "          05 {prefix}-CENTER-LNG PIC X(32)  VALUE '{lng}'.\n"
        ));
        out.push_str(&format!(
            "          05 {prefix}-ZOOM       PIC S9(4)  VALUE {zoom}.\n"
        ));
    }

    out.push('\n');
}

fn write_procedure_division(out: &mut String, form: &Form, map: &mut SourceMap) {
    out.push_str("       PROCEDURE DIVISION.\n");

    let all_controls = collect_all_controls(&form.controls);

    // ── COBOL-MAIN ──────────────────────────────────────────────────────
    out.push_str("       COBOL-MAIN.\n");
    out.push_str("           CALL \"COBOL-INIT-FORM\" USING FORM-NAME\n");
    if !form.data_bindings.is_empty() {
        out.push_str("           PERFORM COBOL-DATA-BINDINGS-LOAD\n");
    }

    // Kick off timer dispatcher if any timers exist
    let has_timers = all_controls
        .iter()
        .any(|c| c.control_type == ControlType::Timer);
    if has_timers {
        out.push_str("           PERFORM COBOL-START-TIMERS\n");
    }

    for ctrl in all_controls
        .iter()
        .filter(|c| c.control_type == ControlType::IndexedFile && prop_bool(c, "AutoOpen", false))
    {
        out.push_str(&format!("           PERFORM {}-OPEN\n", cobol_word(&ctrl.id)));
    }

    // Call OnLoad nested program
    if let Some(ev) = form.form_events.iter().find(|e| e.event == "onLoad") {
        out.push_str(&format!("           CALL \"{}\"\n", ev.paragraph));
    }

    if !form.data_bindings.is_empty() {
        out.push_str("           PERFORM COBOL-DATA-BINDINGS-POPULATE\n");
        out.push_str("           PERFORM COBOL-DATA-BINDINGS-MARK-CLEAN\n");
    }

    out.push_str("           PERFORM COBOL-EVENT-LOOP\n");

    // Call OnClose nested program
    if let Some(ev) = form.form_events.iter().find(|e| e.event == "onClose") {
        out.push_str(&format!("           CALL \"{}\"\n", ev.paragraph));
    }

    for ctrl in all_controls
        .iter()
        .filter(|c| c.control_type == ControlType::IndexedFile && prop_bool(c, "AutoOpen", false))
    {
        out.push_str(&format!("           PERFORM {}-CLOSE\n", cobol_word(&ctrl.id)));
    }

    out.push_str("           STOP RUN.\n");
    out.push('\n');

    // ── COBOL-EVENT-LOOP — dispatches via CALL to nested programs ─────────
    write_event_loop(out, form);

    // ── Infrastructure helper paragraphs (outer program scope) ────────────
    write_timer_stubs(out, &all_controls);
    write_csv_export_stubs(out, &all_controls);
    write_rest_client_stubs(out, &all_controls);
    write_web_search_stubs(out, &all_controls);
    write_sql_stubs(out, &all_controls);
    write_indexed_file_stubs(out, &all_controls);
    write_agent_stubs(out, &all_controls);
    write_animation_stubs(out, form, &all_controls);
    write_chart_stubs(out, &all_controls);
    data_binding::write_data_binding_paragraphs(out, form);

    // ── Nested COBOL-85 programs — one per event handler ─────────────────
    write_nested_programs(out, form, &all_controls, map);

    // ── Close the outer program ───────────────────────────────────────────
    out.push_str(&format!("       END PROGRAM {}.\n", form.name));
}

// ── Timer paragraph generator ─────────────────────────────────────────────────

fn write_timer_stubs(out: &mut String, all_controls: &[&Control]) {
    let timers: Vec<&&Control> = all_controls
        .iter()
        .filter(|c| c.control_type == ControlType::Timer)
        .collect();
    if timers.is_empty() {
        return;
    }

    // COBOL-START-TIMERS: enable each timer at startup
    out.push_str("       COBOL-START-TIMERS.\n");
    out.push_str("      *>    Called once from COBOL-MAIN to register timer intervals.\n");
    for ctrl in &timers {
        let iv = ctrl
            .get_prop("Interval")
            .map(|v| v.as_i64())
            .unwrap_or(1000);
        out.push_str(&format!(
            "           INVOKE {id} 'SetInterval' USING BY VALUE {iv}\n",
            id = ctrl.id,
            iv = iv
        ));
    }
    out.push_str("           CONTINUE.\n");
    out.push('\n');

    // Timer ticks are EVENTS in the nested-program model: the runtime fires
    // an `onTick` form event per interval, and the standard COBOL-EVENT-LOOP
    // dispatches it via CALL to the timer's `TIMER-ID--ONTICK` nested program
    // (bind the handler in the designer's Events list). No paragraph-based
    // tick dispatcher is generated.
}

// ── DataGrid CSV export paragraph generator ───────────────────────────────────

fn write_csv_export_stubs(out: &mut String, all_controls: &[&Control]) {
    for ctrl in all_controls
        .iter()
        .filter(|c| c.control_type == ControlType::DataGrid)
    {
        if !ctrl
            .get_prop("ShowCSVExportButton")
            .map(|v| v.as_bool())
            .unwrap_or_else(|| datagrid_csv_export_enabled(ctrl))
        {
            continue;
        }
        let pfx = format!("WS-{}", cobol_word(&ctrl.id));
        let para = ctrl
            .get_prop("CSVParagraph")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| format!("{}-EXPORT-CSV", ctrl.id));
        let delim = ctrl
            .get_prop("CSVDelimiter")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| ",".to_owned());
        let mode = ctrl
            .get_prop("CSVExportMode")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "Filtered".to_owned());

        out.push_str(&format!("       {}.\n", para));
        out.push_str(&format!(
            "      *>    Export {} data to CSV file.  Delimiter: \"{}\". Mode: {}.\n",
            ctrl.id, delim, mode
        ));
        out.push_str(
            "      *>    Column order and filtered/all rows follow the DataGrid settings.\n",
        );
        out.push_str(&format!(
            "      *>    Set {pfx}-CSV-PATH to the desired output file path before calling.\n"
        ));
        out.push_str(&format!(
            "           INVOKE {id} 'ExportCSV'\n",
            id = ctrl.id
        ));
        out.push_str(&format!(
            "               USING BY REFERENCE {pfx}-CSV-PATH\n"
        ));
        out.push_str(&format!("               RETURNING {pfx}-CSV-STATUS\n"));
        out.push_str(&format!("           IF {pfx}-CSV-STATUS NOT = 0\n"));
        out.push_str(&format!(
            "               DISPLAY \"CSV export error: \" {pfx}-CSV-STATUS\n"
        ));
        out.push_str("           END-IF.\n");
        out.push('\n');
    }
}

fn datagrid_csv_export_enabled(ctrl: &Control) -> bool {
    ctrl.get_prop("ExportCSV")
        .map(|v| v.as_bool())
        .unwrap_or(false)
        || ctrl
            .get_prop("ShowCSVExportButton")
            .map(|v| v.as_bool())
            .unwrap_or(false)
}

// ── RestClient call stub generator ───────────────────────────────────────────

fn write_rest_client_stubs(out: &mut String, all_controls: &[&Control]) {
    for ctrl in all_controls
        .iter()
        .filter(|c| c.control_type == ControlType::RestClient)
    {
        let para_get = format!("{}-GET", ctrl.id);
        let para_post = format!("{}-POST", ctrl.id);
        let para_put = format!("{}-PUT", ctrl.id);
        let resp_para = ctrl
            .get_prop("ResponseParagraph")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| format!("{}-ON-RESPONSE", ctrl.id));
        let err_para = ctrl
            .get_prop("ErrorParagraph")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| format!("{}-ON-ERROR", ctrl.id));
        let resp_item = ctrl
            .get_prop("ResponseDataItem")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();
        let status_item = ctrl
            .get_prop("StatusDataItem")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();

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
            out,
            &resp_para,
            &format!(
                "{} response handler — WS-HTTP-RESPONSE contains the body, WS-HTTP-STATUS the code",
                ctrl.id
            ),
        );
        write_stub_paragraph(
            out,
            &err_para,
            &format!(
                "{} error handler — WS-HTTP-STATUS contains the error code (0 = network failure)",
                ctrl.id
            ),
        );

        // ── Optional sync paragraph ───────────────────────────────────────────
        if !resp_item.is_empty() || !status_item.is_empty() {
            let sync_para = format!("{}-SYNC-ITEMS", ctrl.id);
            out.push_str(&format!("       {}.\n", sync_para));
            out.push_str("      *>    Copy response / status into your declared data items.\n");
            if !resp_item.is_empty() {
                out.push_str(&format!(
                    "           MOVE WS-HTTP-RESPONSE TO {}\n",
                    resp_item
                ));
            }
            if !status_item.is_empty() {
                out.push_str(&format!(
                    "           MOVE WS-HTTP-STATUS TO {}\n",
                    status_item
                ));
            }
            out.push_str("           CONTINUE.\n");
            out.push('\n');
        }
    }
}

// ── AgentObject Ask stub generator ───────────────────────────────────────────

fn write_agent_stubs(out: &mut String, all_controls: &[&Control]) {
    for ctrl in all_controls
        .iter()
        .filter(|c| c.control_type == ControlType::AgentObject)
    {
        let ask_para = format!("{}-ASK", ctrl.id);
        let resp_para = ctrl
            .get_prop("ResponseParagraph")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| format!("{}-ON-RESPONSE", ctrl.id));
        let err_para = ctrl
            .get_prop("ErrorParagraph")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| format!("{}-ON-ERROR", ctrl.id));
        let resp_item = ctrl
            .get_prop("ResponseDataItem")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();
        let model = ctrl
            .get_prop("AgentModel")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "llama3.2".into());
        let url = ctrl
            .get_prop("AgentURL")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "http://localhost:11434".into());

        out.push_str(&format!("       {}.\n", ask_para));
        out.push_str(&format!(
            "      *>    Ask the AI agent {} (model: {}, endpoint: {})\n",
            ctrl.id, model, url
        ));
        out.push_str("      *>    Set WS-AGENT-PROMPT before calling.\n");
        out.push_str(&format!(
            "           INVOKE {id} 'Ask'\n               USING BY VALUE WS-AGENT-PROMPT\n               RETURNING WS-AGENT-RESPONSE\n",
            id = ctrl.id
        ));
        if !resp_item.is_empty() {
            out.push_str(&format!(
                "           MOVE WS-AGENT-RESPONSE TO {}\n",
                resp_item
            ));
        }
        out.push_str("           EVALUATE TRUE\n");
        out.push_str("               WHEN WS-AGENT-ERROR = SPACES\n");
        out.push_str(&format!("                   PERFORM {}\n", resp_para));
        out.push_str("               WHEN OTHER\n");
        out.push_str(&format!("                   PERFORM {}\n", err_para));
        out.push_str("           END-EVALUATE.\n");
        out.push('\n');

        write_stub_paragraph(
            out,
            &resp_para,
            &format!(
                "{} response ready — WS-AGENT-RESPONSE contains the LLM reply",
                ctrl.id
            ),
        );
        write_stub_paragraph(
            out,
            &err_para,
            &format!(
                "{} error handler — WS-AGENT-ERROR contains the error message",
                ctrl.id
            ),
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
    if entries.is_empty() {
        return;
    }

    // ── COBOL-PLAY-ANIMATION ─────────────────────────────────────────────────
    // Dispatches to the correct INVOKE based on WS-ANIM-NAME.
    out.push_str("       COBOL-PLAY-ANIMATION.\n");
    out.push_str("      *> Set WS-ANIM-NAME before calling this paragraph.\n");
    if entries.len() == 1 {
        let (ctrl_id, anim_name) = &entries[0];
        if ctrl_id != "FORM" {
            out.push_str(&format!("           INVOKE {ctrl_id} 'PlayAnimation'\n"));
            out.push_str(&format!("               USING BY VALUE \"{anim_name}\".\n"));
        } else {
            out.push_str("           CONTINUE.\n");
        }
    } else {
        out.push_str("           EVALUATE WS-ANIM-NAME\n");
        for (ctrl_id, anim_name) in &entries {
            out.push_str(&format!("               WHEN \"{anim_name}\"\n"));
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
            out.push_str(&format!("           INVOKE {ctrl_id} 'StopAnimation'\n"));
            out.push_str(&format!("               USING BY VALUE \"{anim_name}\".\n"));
        } else {
            out.push_str("           CONTINUE.\n");
        }
    } else {
        out.push_str("           EVALUATE WS-ANIM-NAME\n");
        for (ctrl_id, anim_name) in &entries {
            out.push_str(&format!("               WHEN \"{anim_name}\"\n"));
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
            let para = format!(
                "{}-PLAY-{}",
                ctrl.id,
                anim.name
                    .to_ascii_uppercase()
                    .replace(' ', "-")
                    .replace('-', "-")
            );
            out.push_str(&format!("       {para}.\n"));
            out.push_str(&format!("           INVOKE {} 'PlayAnimation'\n", ctrl.id));
            out.push_str(&format!(
                "               USING BY VALUE \"{}\".\n\n",
                anim.name
            ));
        }
    }
}

// ── WebSearch stub generator (spec 039 T14) ──────────────────────────────────
//
// Generates a `<id>-SEARCH` paragraph that builds a Google Custom Search
// JSON API URL (cx/q/num/safe, deliberately no `key=` — the credential-aware
// search is `INVOKE <id> 'SEARCH'`, T15) and calls the same `COBOL-HTTP-GET`
// intrinsic `write_rest_client_stubs` above uses. Modelled directly on that
// function's `<id>-GET` paragraph.

fn write_web_search_stubs(out: &mut String, all_controls: &[&Control]) {
    for ctrl in all_controls
        .iter()
        .filter(|c| c.control_type == ControlType::WebSearch)
    {
        let pfx = format!("WS-{}", cobol_word(&ctrl.id));
        let para_search = format!("{}-SEARCH", ctrl.id);
        let resp_para = format!("{}-ON-RESULTS", ctrl.id);
        let err_para = format!("{}-ON-ERROR", ctrl.id);

        out.push_str(&format!("       {}.\n", para_search));
        out.push_str(&format!(
            "      *>    Google Custom Search via {} — MOVE your query text to {pfx}-QUERY\n",
            ctrl.id
        ));
        out.push_str(
            "      *>    before calling. This paragraph does a plain, unencoded STRING\n",
        );
        out.push_str(
            "      *>    concatenation (no key, no percent-encoding — a multi-word query\n",
        );
        out.push_str(&format!(
            "      *>    truncates at its first space). For a correct, credential-aware\n      *>    search use INVOKE {} 'SEARCH' instead.\n",
            ctrl.id
        ));
        out.push_str("           MOVE SPACES TO WS-REQUEST-URL\n");
        out.push_str("           STRING 'https://www.googleapis.com/customsearch/v1?cx='\n");
        out.push_str(&format!(
            "                  {pfx}-SEARCH-ENGINE-ID DELIMITED BY SPACE\n"
        ));
        out.push_str("                  '&q=' DELIMITED BY SIZE\n");
        out.push_str(&format!("                  {pfx}-QUERY DELIMITED BY SPACE\n"));
        out.push_str("                  '&num=' DELIMITED BY SIZE\n");
        out.push_str(&format!(
            "                  {pfx}-NUM-RESULTS DELIMITED BY SIZE\n"
        ));
        out.push_str("                  '&safe=' DELIMITED BY SIZE\n");
        out.push_str(&format!(
            "                  {pfx}-SAFE-SEARCH DELIMITED BY SPACE\n"
        ));
        out.push_str("               INTO WS-REQUEST-URL\n");
        out.push_str("           END-STRING\n");
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

        write_stub_paragraph(
            out,
            &resp_para,
            &format!(
                "{} results handler — WS-HTTP-RESPONSE contains the raw JSON body",
                ctrl.id
            ),
        );
        write_stub_paragraph(
            out,
            &err_para,
            &format!(
                "{} error handler — WS-HTTP-STATUS contains the error code (0 = network failure)",
                ctrl.id
            ),
        );
    }
}

// ── SqlDatabase stub generator (Phase 8) ─────────────────────────────────────
//
// Generates ready-to-run COBOL paragraphs that use the COBOL-OPEN-DB,
// COBOL-EXEC-SQL, COBOL-FETCH-ROW, COBOL-NEXT-ROW, and COBOL-CLOSE-DB
// built-in CALLs provided by the cobolt-runtime database engine.

fn write_sql_stubs(out: &mut String, all_controls: &[&Control]) {
    for ctrl in all_controls
        .iter()
        .filter(|c| c.control_type == ControlType::SqlDatabase)
    {
        // Paragraph and data names are COBOL words: an id like `sql_db` must
        // not reach the source verbatim (the `_` would break the whole item).
        let id = cobol_word(&ctrl.id);
        let id = id.as_str();
        let pfx = format!("WS-{}", cobol_word(id));
        // Backend label derived from the Driver property (sqlite / postgres /
        // mysql). The runtime actually routes on the connection-string scheme,
        // so this only affects the generated comments.
        let drv = ctrl
            .get_prop("Driver")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "sqlite".into());
        let drv_label = match drv.to_ascii_lowercase().as_str() {
            "postgres" | "postgresql" => "PostgreSQL",
            "mysql" => "MySQL",
            _ => "SQLite",
        };

        let conn_para = format!("{id}-CONNECT");
        let exec_para = format!("{id}-EXEC");
        let fetch_para = format!("{id}-FETCH-ALL");
        let close_para = format!("{id}-CLOSE");

        let connect_ok = ctrl
            .get_prop("ConnectParagraph")
            .map(|v| v.as_str().to_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("{id}-ON-CONNECT"));
        let query_done = ctrl
            .get_prop("QueryCompleteParagraph")
            .map(|v| v.as_str().to_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("{id}-ON-QUERY-DONE"));
        let error_para = ctrl
            .get_prop("ErrorParagraph")
            .map(|v| v.as_str().to_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("{id}-ON-ERROR"));

        // ── {id}-CONNECT ───────────────────────────────────────────────────
        // Opens a SQLite connection using the connection string in
        // {pfx}-CONN-STRING.  Stores the handle in {pfx}-HANDLE.
        out.push_str(&format!("       {conn_para}.\n"));
        out.push_str(&format!(
            "      *>  Open a {drv_label} connection for {id}.\n"
        ));
        out.push_str(&format!(
            "      *>  Connection string is in {pfx}-CONN-STRING.\n"
        ));
        out.push_str(&format!(
            "      *>  On success: {pfx}-HANDLE holds the connection handle.\n"
        ));
        out.push_str(&format!(
            "      *>  On error:   WS-SQL-ERROR contains the message.\n"
        ));
        out.push_str(&format!("           MOVE SPACES TO WS-SQL-ERROR\n"));
        out.push_str(&format!("           CALL \"COBOL-OPEN-DB\"\n"));
        out.push_str(&format!(
            "               USING BY REFERENCE {pfx}-CONN-STRING\n"
        ));
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
        out.push_str(&format!(
            "      *>  Stores row count in WS-SQL-ROW-COUNT.\n"
        ));
        out.push_str(&format!(
            "      *>  Resets WS-SQL-MORE to 'Y' if rows are present.\n"
        ));
        out.push_str(&format!("           MOVE SPACES TO WS-SQL-ERROR\n"));
        out.push_str(&format!("           CALL \"COBOL-EXEC-SQL\"\n"));
        out.push_str(&format!("               USING BY REFERENCE {pfx}-HANDLE\n"));
        out.push_str(&format!("                     BY REFERENCE WS-SQL-QUERY\n"));
        out.push_str(&format!(
            "                     BY REFERENCE WS-SQL-ROW-COUNT\n"
        ));
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
        out.push_str(&format!(
            "      *>  Iterate over all rows returned by {id}-EXEC.\n"
        ));
        out.push_str(&format!(
            "      *>  Copy this paragraph and add column reads inside the loop.\n"
        ));
        out.push_str(&format!("      *>  Example:\n"));
        out.push_str(&format!("      *>    MOVE 1 TO WS-SQL-COL-INDEX\n"));
        out.push_str(&format!(
            "      *>    CALL \"COBOL-FETCH-ROW\" USING {pfx}-HANDLE\n"
        ));
        out.push_str(&format!(
            "      *>                                   WS-SQL-COL-INDEX\n"
        ));
        out.push_str(&format!(
            "      *>                                   WS-SQL-CURRENT-VALUE\n"
        ));
        out.push_str(&format!(
            "      *>                                   WS-SQL-ERROR\n"
        ));
        out.push_str(&format!(
            "      *>    MOVE WS-SQL-CURRENT-VALUE TO WS-MY-NAME-FIELD\n"
        ));
        out.push_str(&format!("           PERFORM UNTIL WS-SQL-MORE = 'N'\n"));
        out.push_str(&format!("               MOVE 1 TO WS-SQL-COL-INDEX\n"));
        out.push_str(&format!("               CALL \"COBOL-FETCH-ROW\"\n"));
        out.push_str(&format!(
            "                   USING BY REFERENCE {pfx}-HANDLE\n"
        ));
        out.push_str(&format!(
            "                         BY REFERENCE WS-SQL-COL-INDEX\n"
        ));
        out.push_str(&format!(
            "                         BY REFERENCE WS-SQL-CURRENT-VALUE\n"
        ));
        out.push_str(&format!(
            "                         BY REFERENCE WS-SQL-ERROR\n"
        ));
        out.push_str(&format!(
            "      *>          MOVE WS-SQL-CURRENT-VALUE TO your-field-here\n"
        ));
        out.push_str(&format!("               CONTINUE\n"));
        out.push_str(&format!("               CALL \"COBOL-NEXT-ROW\"\n"));
        out.push_str(&format!(
            "                   USING BY REFERENCE {pfx}-HANDLE\n"
        ));
        out.push_str(&format!(
            "                         BY REFERENCE WS-SQL-MORE\n"
        ));
        out.push_str(&format!("           END-PERFORM.\n"));
        out.push('\n');

        // ── {id}-CLOSE ─────────────────────────────────────────────────────
        out.push_str(&format!("       {close_para}.\n"));
        out.push_str(&format!(
            "      *>  Close the {drv_label} connection for {id}.\n"
        ));
        out.push_str(&format!("           CALL \"COBOL-CLOSE-DB\"\n"));
        out.push_str(&format!(
            "               USING BY REFERENCE {pfx}-HANDLE.\n"
        ));
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

// ── IndexedFile control stub generator ───────────────────────────────────────

fn write_indexed_file_stubs(out: &mut String, all_controls: &[&Control]) {
    for ctrl in all_controls
        .iter()
        .filter(|c| c.control_type == ControlType::IndexedFile)
    {
        // Paragraph and data names are COBOL words: an id like `sql_db` must
        // not reach the source verbatim (the `_` would break the whole item).
        let id = cobol_word(&ctrl.id);
        let id = id.as_str();
        let file = indexed_control_file_name(ctrl);
        let record = prop_string(ctrl, "RecordName")
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("{file}-RECORD"));
        let key = prop_string(ctrl, "KeyName")
            .or_else(|| prop_string(ctrl, "CurrentKeyDataItem"))
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("{record}-KEY"));
        let status_item = prop_string(ctrl, "StatusDataItem")
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("WS-{}-STATUS", cobol_word(id)));
        let open_mode = prop_string(ctrl, "OpenMode").unwrap_or_else(|| "INPUT".into());
        let operator = prop_string(ctrl, "OperatorName").unwrap_or_default();
        let cobol_open_mode = if open_mode.eq_ignore_ascii_case("I-O") {
            "I-O"
        } else {
            "INPUT"
        };

        out.push_str(&format!("       {id}-OPEN.\n"));
        out.push_str(&format!(
            "      *>  Opens indexed file {file} for {cobol_open_mode}.\n"
        ));
        out.push_str(&format!("           IF WS-{id}-IS-OPEN = 0\n"));
        if operator.trim().is_empty() {
            out.push_str(&format!("               OPEN {cobol_open_mode} {file}\n"));
        } else {
            out.push_str(&format!(
                "               OPEN {cobol_open_mode} {file} REGISTERED USER {operator}\n"
            ));
        }
        out.push_str(&format!("               MOVE '00' TO {status_item}\n"));
        out.push_str(&format!("               MOVE 1 TO WS-{id}-IS-OPEN\n"));
        out.push_str(&format!("               MOVE 0 TO WS-{id}-AT-END\n"));
        out.push_str(&format!("               MOVE 0 TO WS-{id}-HAS-RECORD\n"));
        out.push_str("           END-IF.\n\n");

        out.push_str(&format!("       {id}-START.\n"));
        out.push_str(&format!(
            "      *>  Set {key}, then PERFORM {id}-START to position the current pointer.\n"
        ));
        out.push_str(&format!(
            "           START {file} KEY IS GREATER THAN OR EQUAL TO {key}\n"
        ));
        out.push_str(&format!("               INVALID KEY\n"));
        out.push_str(&format!("                   MOVE '23' TO {status_item}\n"));
        out.push_str(&format!(
            "                   MOVE 0 TO WS-{id}-HAS-RECORD\n"
        ));
        out.push_str("               NOT INVALID KEY\n");
        out.push_str(&format!("                   MOVE '00' TO {status_item}\n"));
        out.push_str(&format!("                   MOVE 0 TO WS-{id}-AT-END\n"));
        out.push_str("           END-START.\n\n");

        for (suffix, direction) in [
            ("READ-NEXT", "NEXT"),
            ("READ-PREVIOUS", "PREVIOUS"),
            ("READ-FIRST", "NEXT"),
            ("READ-LAST", "PREVIOUS"),
        ] {
            out.push_str(&format!("       {id}-{suffix}.\n"));
            if suffix == "READ-FIRST" {
                out.push_str(&format!(
                    "      *>  Set {key} to the lowest desired value, position, then read NEXT.\n"
                ));
                out.push_str(&format!(
                    "           START {file} KEY IS GREATER THAN OR EQUAL TO {key}\n"
                ));
                out.push_str("               INVALID KEY CONTINUE\n");
                out.push_str("           END-START\n");
            } else if suffix == "READ-LAST" {
                out.push_str(&format!(
                    "      *>  Set {key} to the highest desired value, position, then read PREVIOUS.\n"
                ));
                out.push_str(&format!(
                    "           START {file} KEY IS LESS THAN OR EQUAL TO {key}\n"
                ));
                out.push_str("               INVALID KEY CONTINUE\n");
                out.push_str("           END-START\n");
            }
            out.push_str(&format!("           READ {file} {direction}\n"));
            out.push_str("               AT END\n");
            out.push_str(&format!("                   MOVE '10' TO {status_item}\n"));
            out.push_str(&format!("                   MOVE 1 TO WS-{id}-AT-END\n"));
            out.push_str(&format!(
                "                   MOVE 0 TO WS-{id}-HAS-RECORD\n"
            ));
            out.push_str("               NOT AT END\n");
            out.push_str(&format!("                   MOVE '00' TO {status_item}\n"));
            out.push_str(&format!("                   MOVE 0 TO WS-{id}-AT-END\n"));
            out.push_str(&format!(
                "                   MOVE 1 TO WS-{id}-HAS-RECORD\n"
            ));
            out.push_str("           END-READ.\n\n");
        }

        out.push_str(&format!("       {id}-READ-INVALID.\n"));
        out.push_str(&format!(
            "      *>  Direct keyed read. Set {key} before calling this paragraph.\n"
        ));
        out.push_str(&format!("           READ {file}\n"));
        out.push_str("               INVALID KEY\n");
        out.push_str(&format!("                   MOVE '23' TO {status_item}\n"));
        out.push_str(&format!(
            "                   MOVE 0 TO WS-{id}-HAS-RECORD\n"
        ));
        out.push_str("               NOT INVALID KEY\n");
        out.push_str(&format!("                   MOVE '00' TO {status_item}\n"));
        out.push_str(&format!(
            "                   MOVE 1 TO WS-{id}-HAS-RECORD\n"
        ));
        out.push_str("           END-READ.\n\n");

        for (suffix, verb) in [
            ("WRITE", "WRITE"),
            ("REWRITE", "REWRITE"),
            ("DELETE", "DELETE"),
        ] {
            out.push_str(&format!("       {id}-{suffix}.\n"));
            out.push_str(&format!(
                "      *>  Requires {file} opened I-O. Data comes from bound/set record fields.\n"
            ));
            if verb == "DELETE" {
                out.push_str(&format!("           DELETE {file}\n"));
            } else {
                out.push_str(&format!("           {verb} {record}\n"));
            }
            out.push_str("               INVALID KEY\n");
            out.push_str(&format!("                   MOVE '23' TO {status_item}\n"));
            out.push_str("               NOT INVALID KEY\n");
            out.push_str(&format!("                   MOVE '00' TO {status_item}\n"));
            out.push_str(&format!("           END-{verb}.\n\n"));
        }

        out.push_str(&format!("       {id}-COMMIT.\n"));
        out.push_str(&format!(
            "      *>  Flushes pending indexed-file changes for {file}.\n"
        ));
        out.push_str(&format!("           CLOSE {file}\n"));
        out.push_str(&format!("           OPEN I-O {file}\n"));
        out.push_str(&format!("           MOVE '00' TO {status_item}.\n\n"));

        out.push_str(&format!("       {id}-ROLLBACK.\n"));
        out.push_str("      *>  Transaction rollback is storage-engine dependent; reopen to discard pending cursor state.\n");
        out.push_str(&format!("           CLOSE {file}\n"));
        out.push_str(&format!("           OPEN {cobol_open_mode} {file}\n"));
        out.push_str(&format!("           MOVE '00' TO {status_item}.\n\n"));

        out.push_str(&format!("       {id}-CLOSE.\n"));
        out.push_str("      *>  No-op when already closed. I-O close commits automatically.\n");
        out.push_str(&format!("           IF WS-{id}-IS-OPEN = 1\n"));
        if cobol_open_mode == "I-O" {
            out.push_str(&format!("               PERFORM {id}-COMMIT\n"));
        }
        out.push_str(&format!("               CLOSE {file}\n"));
        out.push_str(&format!("               MOVE 0 TO WS-{id}-IS-OPEN\n"));
        out.push_str(&format!("               MOVE '00' TO {status_item}\n"));
        out.push_str("           END-IF.\n\n");
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
    if charts.is_empty() {
        return;
    }

    out.push_str("      *> ── Chart INVOKE verb paragraphs ─────────────────────────────────\n");
    out.push('\n');

    for ctrl in charts {
        let id = &ctrl.id;
        let ws = format!("WS-{}", cobol_word(id));
        let ds = ctrl
            .get_prop("DataSource")
            .map(|v| v.as_str().to_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("WS-{}-TABLE", cobol_word(id)));
        let cnt = ctrl
            .get_prop("DataCount")
            .map(|v| v.as_str().to_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("WS-{}-COUNT", cobol_word(id)));

        // ── SET-TABLE ────────────────────────────────────────────────────────
        out.push_str(&format!("       {id}-SET-TABLE.\n"));
        out.push_str(&format!("      *>    Bind a COBOL table to {id}.\n"));
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
        out.push_str(&format!("      *>    Remove all data series from {id}.\n"));
        out.push_str(&format!("      *>    Usage: INVOKE {id} CLEAR\n"));
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
        out.push_str(&format!("      *>    Usage: INVOKE {id} REFRESH\n"));
        out.push_str(&format!(
            "           CALL \"COBOL-CHART-REFRESH\" USING \"{id}\"\n"
        ));
        out.push_str("           CONTINUE.\n");
        out.push('\n');
    }

    // ── Indexed File control fields ───────────────────────────────────────
    for ctrl in all_controls
        .iter()
        .filter(|c| c.control_type == ControlType::IndexedFile)
    {
        let pfx = format!("WS-{}", cobol_word(&ctrl.id));
        let selected = ctrl
            .get_prop("IndexedFile")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();
        let mode = ctrl
            .get_prop("OpenMode")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "INPUT".into());
        let strategy = ctrl
            .get_prop("LoadStrategy")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "Disk".into());
        let status_item = ctrl
            .get_prop("StatusDataItem")
            .map(|v| v.as_str().to_owned())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("{pfx}-STATUS"));

        out.push_str(&format!(
            "      *>── IndexedFile control: {} ─────────────────────────────\n",
            ctrl.id
        ));
        out.push_str(&format!(
            "      *>   Project indexed file: {}\n",
            if selected.trim().is_empty() {
                "<not selected>"
            } else {
                selected.as_str()
            }
        ));
        out.push_str(&format!(
            "       01 {pfx}-OPEN-MODE      PIC X(8)    VALUE '{}'.\n",
            cobol_lit(&mode)
        ));
        out.push_str(&format!(
            "       01 {pfx}-LOAD-STRATEGY  PIC X(8)    VALUE '{}'.\n",
            cobol_lit(&strategy)
        ));
        out.push_str(&format!(
            "       01 {pfx}-IS-OPEN        PIC 9       VALUE 0.\n"
        ));
        out.push_str(&format!(
            "       01 {pfx}-AT-END         PIC 9       VALUE 0.\n"
        ));
        out.push_str(&format!(
            "       01 {pfx}-HAS-RECORD     PIC 9       VALUE 0.\n"
        ));
        out.push_str(&format!(
            "       01 {pfx}-CURRENT-OP     PIC X(16)   VALUE SPACES.\n"
        ));
        out.push_str(&format!(
            "       01 {status_item:<24} PIC X(2)    VALUE '00'.\n"
        ));
        out.push('\n');
    }
}

// ── Nested COBOL-85 program generator ────────────────────────────────────────

/// Emit one nested `PROGRAM-ID ... END PROGRAM` block per event handler.
/// Form-level OnLoad / OnClose come first, then per-control events.
fn write_nested_programs(
    out: &mut String,
    form: &Form,
    all_controls: &[&Control],
    map: &mut SourceMap,
) {
    // A toolbar button's handler is emitted here too. It is not in any control's
    // `events` table — the toolbar owns its buttons — so the walk below would
    // never reach it, and a bound handler would have a `WHEN` calling a program
    // that was never written.
    let toolbar_handlers: Vec<(String, String, String, String, String)> =
        collect_toolbar_dispatch(all_controls)
            .into_iter()
            .filter(|d| toolbar_when_branches(d).is_ok())
            .flat_map(|d| {
                let origin = d.origin.clone();
                let control_id = d.control_id.clone();
                d.handlers.into_iter().map(move |(para, event, code)| {
                    (para, event, code, origin.clone(), control_id.clone())
                })
            })
            .collect();

    let has_any = !form.form_events.is_empty()
        || all_controls.iter().any(|c| !c.events.is_empty())
        || !form.user_procedures.is_empty()
        || !toolbar_handlers.is_empty();
    if !has_any {
        return;
    }

    out.push_str("\n      *> ── Nested event-handler programs (COBOL-85) ─────────────────────\n");
    out.push('\n');

    // Every woven procedure — event handlers included — is `IS COMMON` (spec 009
    // R4) so any procedure is callable from anywhere within the form module (e.g.
    // one handler CALLing another, or a user procedure CALLing a handler).
    for ev in &form.form_events {
        write_nested_program(
            out,
            &ev.paragraph,
            &ev.event,
            &ev.code,
            &format!("Form {} handler", ev.event),
            true,
            None,
            CodeSite::FormEvent {
                event: ev.event.clone(),
            },
            map,
        );
    }

    // Per-control events. A control that belongs to a repeating group (array)
    // gets the indexed handler stub — it receives the 1-based array index of the
    // item that fired.
    for ctrl in all_controls {
        let array_member = form
            .array_binding_context_for_member(&ctrl.id)
            .map(|_| ctrl.id.clone());
        for ev in &ctrl.events {
            write_nested_program(
                out,
                &ev.paragraph,
                &ev.event,
                &ev.code,
                &format!("{} {} handler", ctrl.id, ev.event),
                true,
                array_member.as_deref(),
                CodeSite::ControlEvent {
                    control_id: ctrl.id.clone(),
                    event: ev.event.clone(),
                },
                map,
            );
        }
    }

    // Toolbar button handlers. `IS COMMON` like every other woven procedure, so
    // a button's handler can CALL a user procedure and be CALLed in turn.
    for (paragraph, event, code, origin, control_id) in &toolbar_handlers {
        write_nested_program(
            out,
            paragraph,
            event,
            code,
            &format!("toolbar button {origin} {event} handler"),
            true,
            None,
            CodeSite::ControlEvent {
                control_id: control_id.clone(),
                event: event.clone(),
            },
            map,
        );
    }

    // User procedures (spec 005) — nested programs callable by name from the
    // event handlers. A handler is a *sibling* contained program, so the
    // procedure must be `IS COMMON` for that CALL to be valid COBOL-85.
    for up in &form.user_procedures {
        if up.name.trim().is_empty() {
            continue;
        }
        write_nested_program(
            out,
            up.name.trim(),
            "",
            &up.code,
            &format!("user procedure {}", up.name.trim()),
            true,
            None,
            CodeSite::Procedure {
                name: up.name.trim().to_string(),
            },
            map,
        );
    }
}

/// Emit a single COBOL-85 nested program for one event handler.
///
/// The developer owns the whole handler body (`ENVIRONMENT DIVISION` …
/// `PROCEDURE DIVISION` + statements), stored in `source`. The generator only
/// adds the `IDENTIFICATION DIVISION` / `PROGRAM-ID` header and the closing
/// `GOBACK` / `END PROGRAM`. An unwritten handler is emitted from the shared
/// [`event_handler_template`] so the generated file always compiles.
///
/// `common` emits `IS COMMON PROGRAM` so the nested program can be CALLed by its
/// siblings (used for user procedures, which handlers call).
#[allow(clippy::too_many_arguments)]
fn write_nested_program(
    out: &mut String,
    prog_id: &str,
    event: &str,
    source: &str,
    comment: &str,
    common: bool,
    array_member: Option<&str>,
    site: CodeSite,
    map: &mut SourceMap,
) {
    let attr = if common { " IS COMMON PROGRAM" } else { "" };
    out.push_str("       IDENTIFICATION DIVISION.\n");
    out.push_str(&format!("       PROGRAM-ID. {}{}.\n", prog_id, attr));
    out.push('\n');

    let trimmed = source.trim();
    let is_user_authored = !trimmed.is_empty();
    let body = if trimmed.is_empty() {
        out.push_str(&format!("      *>    TODO: {}\n", comment));
        match array_member {
            Some(control_id) => {
                cobolt_forms::model::event_handler_template_indexed(event, control_id)
            }
            None => cobolt_forms::model::event_handler_template(event),
        }
    } else {
        source.to_string()
    };
    // Record the inclusive 1-based line range of the developer-authored body so
    // the debugger can hide/skip the surrounding generated scaffolding and a
    // diagnostic can name the owning site (spec 053 R6). Only real user code
    // counts — an empty handler emits a generated template stub, which maps to
    // no site. Handler bodies keep their leading blank lines, so the span
    // starts at site line 1.
    let body_start = next_line_number(out);
    for line in body.trim_end().lines() {
        out.push_str(line);
        out.push('\n');
    }
    if is_user_authored {
        let body_end = out.matches('\n').count() as u32; // last line just written
        if body_end >= body_start {
            map.record(site, body_start, body_end, 1);
        }
    }
    out.push('\n');

    out.push_str("           GOBACK.\n");
    out.push('\n');
    out.push_str(&format!("       END PROGRAM {}.\n", prog_id));
    out.push('\n');
}

/// One toolbar button the event loop has to dispatch, and everything it runs.
struct ToolbarDispatch {
    /// The derived `<toolbar>-<group>-<button>` id the press arrives under.
    control_id: String,
    /// Where it came from, for the comment when something is wrong.
    origin: String,
    /// The action, when it is the form's own COBOL. `None` = `event` or a
    /// platform action, neither of which the loop runs.
    action: Option<cobolt_forms::toolbar::ToolbarAction>,
    /// The button's own handlers: `(nested-program name, event, source)`.
    handlers: Vec<(String, String, String)>,
}

/// Every toolbar button the generated event loop has something to do for: one
/// carrying the developer's own handler, and one whose action is the FORM's own
/// COBOL (`procedure:` / `open-modal:`).
///
/// A button is not a `Control`: the toolbar owns its layout, so it has no entry
/// for the per-control walk to find. Codegen reads each ToolBar's definition
/// instead and dispatches under the button's derived id. Without this pass a
/// button's handler and those two actions cannot reach anything at all, whatever
/// the designer offers.
///
/// The `event` action is not listed as an action here — the renderer already
/// fires the toolbar's own `onClick`, which is that action's whole meaning — and
/// neither are the platform actions, which the host carries out without COBOL.
/// A button can still have a handler alongside any of them.
fn collect_toolbar_dispatch(all_controls: &[&Control]) -> Vec<ToolbarDispatch> {
    use cobolt_forms::toolbar::{ToolbarAction, ToolbarDef};
    let mut out: Vec<ToolbarDispatch> = Vec::new();
    for ctrl in all_controls
        .iter()
        .filter(|c| c.control_type == ControlType::ToolBar)
    {
        let def = ToolbarDef::from_control(ctrl);
        for (group, button) in def.buttons() {
            let control_id =
                cobolt_forms::toolbar::button_control_id(&ctrl.id, &group.id, &button.id);
            let action = match button.action() {
                a @ (ToolbarAction::Procedure(_) | ToolbarAction::OpenModal(_)) => Some(a),
                _ => None,
            };
            let handlers: Vec<(String, String, String)> = button
                .events
                .iter()
                .filter(|e| e.has_code())
                .map(|e| {
                    (
                        cobolt_forms::model::derive_paragraph_name(&control_id, &e.event),
                        e.event.clone(),
                        e.code.clone(),
                    )
                })
                .collect();
            if action.is_none() && handlers.is_empty() {
                continue;
            }
            // A hand-edited `.cfrm` can repeat an id; a second WHEN on the same
            // literal would simply never be reached, so keep the first.
            if out.iter().any(|d| d.control_id == control_id) {
                continue;
            }
            out.push(ToolbarDispatch {
                control_id,
                origin: format!("{}/{}/{}", ctrl.id, group.id, button.id),
                action,
                handlers,
            });
        }
    }
    out
}

/// The inner `EVALUATE COBOL-EVENT-ID` for one toolbar button — one entry per
/// event, with the statements that event runs in order — or the reason the button
/// gets no `WHEN` at all.
///
/// Where a button has BOTH a handler and an action, the handler runs FIRST. That
/// is not arbitrary: an `open-modal:` button whose handler fills in the fields
/// the modal reads only works in that order, and the reverse order has no case
/// to make for it.
///
/// Nothing fails in silence: a button naming no procedure, naming no form, or
/// carrying an id too long for `COBOL-CONTROL-ID` yields an `Err` the generator
/// writes into the source as a comment — which the developer can read, since
/// Generated Code is a category in the project tree — instead of a `WHEN` that
/// could never fire.
fn toolbar_when_branches(d: &ToolbarDispatch) -> Result<Vec<(String, Vec<String>)>, String> {
    use cobolt_forms::toolbar::{ToolbarAction, MAX_BUTTON_CONTROL_ID};
    if d.control_id.len() > MAX_BUTTON_CONTROL_ID {
        return Err(format!(
            "cannot be dispatched: its id \"{}\" is {} characters and \
             COBOL-CONTROL-ID holds {}. Shorten the toolbar, group or button name.",
            d.control_id,
            d.control_id.len(),
            MAX_BUTTON_CONTROL_ID
        ));
    }
    let mut branches: Vec<(String, Vec<String>)> = Vec::new();
    for (paragraph, event, _) in &d.handlers {
        let slot = match branches.iter_mut().find(|(e, _)| e == event) {
            Some(slot) => slot,
            None => {
                branches.push((event.clone(), Vec::new()));
                branches.last_mut().expect("just pushed")
            }
        };
        slot.1.push(format!("CALL \"{paragraph}\""));
    }
    let action_statement = match &d.action {
        None => None,
        Some(ToolbarAction::Procedure(name)) if name.trim().is_empty() => {
            return Err("asks to run a procedure but names none.".to_owned())
        }
        Some(ToolbarAction::OpenModal(form_id)) if form_id.trim().is_empty() => {
            return Err("asks to open a modal form but names none.".to_owned())
        }
        // A user procedure is a nested program, `IS COMMON`, so the event loop
        // in the outer program can CALL it by name.
        Some(ToolbarAction::Procedure(name)) => Some(format!("CALL \"{}\"", name.trim())),
        // One argument means MODAL: `OpenFormSync`'s comma-form default. The
        // handle is already NULL by the time a modal returns, so nothing is
        // RETURNING-ed into.
        Some(ToolbarAction::OpenModal(form_id)) => Some(format!(
            "INVOKE ME::\"OpenFormSync\"(\"{}\")",
            form_id.trim()
        )),
        // `collect_toolbar_dispatch` stores nothing else as an action.
        Some(other) => {
            return Err(format!(
                "has an action the event loop cannot run: {other:?}"
            ))
        }
    };
    if let Some(statement) = action_statement {
        match branches.iter_mut().find(|(e, _)| e == "onClick") {
            Some((_, stmts)) => stmts.push(statement),
            None => branches.push(("onClick".to_owned(), vec![statement])),
        }
    }
    if branches.is_empty() {
        return Err("has nothing for the event loop to run.".to_owned());
    }
    Ok(branches)
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
    // Toolbar buttons split into the ones the loop can dispatch and the ones it
    // must explain. The refusals are written as comments OUTSIDE the EVALUATE:
    // a form whose only dispatchable thing was a broken button would otherwise
    // get an `EVALUATE` with no `WHEN` at all, which is not COBOL.
    let (dispatchable, refused): (Vec<_>, Vec<_>) = collect_toolbar_dispatch(&all_controls)
        .into_iter()
        .map(|d| {
            let verdict = toolbar_when_branches(&d);
            (d, verdict)
        })
        .partition(|(_, verdict)| verdict.is_ok());
    for (d, verdict) in &refused {
        if let Err(reason) = verdict {
            out.push_str(&format!(
                "      *> Toolbar button {} {reason}\n",
                d.origin
            ));
        }
    }

    // Form-level events dispatched through the loop. `onLoad` / `onClose` are
    // CALLed directly from COBOL-MAIN (not via the loop), so they're excluded
    // here; every other bound form event (onShow, onActivate, onResize, …) is
    // dispatched under a WHEN that matches the form's own id.
    let form_loop_events: Vec<_> = form
        .form_events
        .iter()
        .filter(|e| e.event != "onLoad" && e.event != "onClose")
        .collect();

    if controls_with_events.is_empty() && form_loop_events.is_empty() && dispatchable.is_empty() {
        // No events — nothing to dispatch.
        out.push_str("               *> No event handlers defined yet.\n");
        out.push_str("               CONTINUE\n");
    } else {
        out.push_str("               EVALUATE COBOL-CONTROL-ID\n");
        // Form-level events first (COBOL-CONTROL-ID = the form name).
        if !form_loop_events.is_empty() {
            out.push_str(&format!("                   WHEN \"{}\"\n", form.name));
            out.push_str("                       EVALUATE COBOL-EVENT-ID\n");
            for ev in &form_loop_events {
                out.push_str(&format!(
                    "                           WHEN \"{}\"\n",
                    ev.event
                ));
                if matches!(ev.event.as_str(), "onDeactivate" | "onDeactivated") {
                    for ctrl in all_controls.iter().filter(|c| {
                        c.control_type == ControlType::IndexedFile
                            && prop_bool(c, "AutoOpen", false)
                    }) {
                        out.push_str(&format!(
                            "                               PERFORM {}-CLOSE\n",
                            cobol_word(&ctrl.id)
                        ));
                    }
                }
                out.push_str(&format!(
                    "                               CALL \"{}\"\n",
                    ev.paragraph
                ));
            }
            out.push_str("                       END-EVALUATE\n");
        }
        for ctrl in &controls_with_events {
            out.push_str(&format!("                   WHEN \"{}\"\n", ctrl.id));
            out.push_str("                       EVALUATE COBOL-EVENT-ID\n");
            for ev in &ctrl.events {
                out.push_str(&format!(
                    "                           WHEN \"{}\"\n",
                    ev.event
                ));
                // Dispatch to nested program via CALL (not PERFORM).
                // For repeating-group (array) member controls, pass the index
                // so the handler's CONTROL-ARRAY-INDEX linkage item is populated.
                if form.array_binding_context_for_member(&ctrl.id).is_some() {
                    out.push_str(&format!(
                        "                               CALL \"{}\" USING CONTROL-ARRAY-INDEX\n",
                        ev.paragraph
                    ));
                } else {
                    out.push_str(&format!(
                        "                               CALL \"{}\"\n",
                        ev.paragraph
                    ));
                }
            }
            out.push_str("                       END-EVALUATE\n");
        }
        // Toolbar buttons last: they are dispatched under a derived id, which
        // cannot collide with a control's own, so the order is only cosmetic.
        for (d, verdict) in &dispatchable {
            let Ok(branches) = verdict else { continue };
            out.push_str(&format!("                   WHEN \"{}\"\n", d.control_id));
            out.push_str("                       EVALUATE COBOL-EVENT-ID\n");
            for (event, statements) in branches {
                out.push_str(&format!("                           WHEN \"{event}\"\n"));
                for statement in statements {
                    out.push_str(&format!("                               {statement}\n"));
                }
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

fn prop_string(ctrl: &Control, key: &str) -> Option<String> {
    ctrl.get_prop(key).map(|v| v.as_str().to_owned())
}

fn prop_bool(ctrl: &Control, key: &str, fallback: bool) -> bool {
    ctrl.get_prop(key).map(|v| v.as_bool()).unwrap_or(fallback)
}

fn cobol_lit(s: &str) -> String {
    s.replace('\'', "''")
}

fn indexed_control_file_name(ctrl: &Control) -> String {
    let selected = prop_string(ctrl, "IndexedFile").unwrap_or_default();
    let stem = selected
        .rsplit(|ch| ch == '/' || ch == '\\')
        .next()
        .unwrap_or(selected.as_str())
        .rsplit_once('.')
        .map(|(left, _)| left)
        .unwrap_or_else(|| selected.as_str());
    let source = if stem.trim().is_empty() {
        ctrl.id.as_str()
    } else {
        stem
    };
    let mut out = String::new();
    for ch in source.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_uppercase());
        } else if ch == '-' || ch == '_' || ch.is_whitespace() {
            if !out.ends_with('-') {
                out.push('-');
            }
        }
    }
    let out = out.trim_matches('-').to_owned();
    if out.is_empty() {
        ctrl.id.to_ascii_uppercase()
    } else {
        out
    }
}

/// Which property key holds the display text for a given control type.
fn caption_prop_key(ct: &ControlType) -> &'static str {
    match ct {
        ControlType::Label => "Caption",
        ControlType::Button => "Caption",
        ControlType::CheckBox => "Caption",
        ControlType::RadioButton => "Caption",
        ControlType::GroupBox => "Caption",
        ControlType::TextBox => "Text",
        ControlType::ComboBox => "Text",
        ControlType::ListBox => "Text",
        _ => "Caption",
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::{
        BindingDataType, BindingField, BindingSourceDescriptor, BindingTargetDescriptor,
        BindingTargetPath, Control, ControlType, DataBindingDef, EventBinding, FieldMapping, Form,
        PropValue,
    };

    fn make_form() -> Form {
        let mut form = Form::new("MAIN-FORM", "Test", 800, 600);

        let mut btn = Control::new("BTN-OK", ControlType::Button, 10, 10);
        btn.events
            .push(EventBinding::for_control("BTN-OK", "onClick"));
        form.controls.push(btn);

        form
    }

    /// 049 R26/T23 — bound `onDeactivate`/`onDestroy` form events generate
    /// their handler programs and dispatch through the event loop under the
    /// form's own WHEN, like every other bound form event.
    #[test]
    fn lifecycle_events_generate_handlers_and_dispatch_049() {
        let mut form = make_form();
        for ev in ["onDeactivate", "onDestroy"] {
            form.form_events.push(EventBinding {
                event: ev.into(),
                paragraph: cobolt_forms::model::derive_paragraph_name("MAIN-FORM", ev),
                code: "    CONTINUE".into(),
            });
        }
        let src = generate(&form);
        for (ev, para) in [
            ("onDeactivate", "MAIN-FORM--ONDEACTIVATE"),
            ("onDestroy", "MAIN-FORM--ONDESTROY"),
        ] {
            assert!(
                src.contains(&format!("WHEN \"{ev}\"")),
                "{ev} must dispatch through the loop"
            );
            assert!(
                src.contains(&format!("PROGRAM-ID. {para}")),
                "{para} handler program must be generated"
            );
        }
        // An UNBOUND form (no 049 events) generates neither — no dead WHENs.
        let plain = generate(&make_form());
        assert!(!plain.contains("WHEN \"onDeactivate\""));
        assert!(!plain.contains("WHEN \"onDestroy\""));
        println!(
            "049 T23 codegen — bound onDeactivate/onDestroy: 2 loop WHENs + 2 \
             handler programs generated; unbound form: none (no dead dispatch)"
        );
    }

    /// Operator report (2026-07-30): controls named `textbox_1` / `label_result`
    /// (the assistant names them that way) emitted `WS-textbox_1-TEXT`, which
    /// the lexer read as an identifier, an error token for `_`, then a number —
    /// so every one of those data items was skipped and the control had no
    /// storage. Ids must become valid COBOL words before they reach the source.
    #[test]
    fn control_ids_become_valid_cobol_words() {
        // The reported case: underscores are the killer.
        assert_eq!(cobol_word("textbox_1"), "textbox-1");
        assert_eq!(cobol_word("label_result"), "label-result");
        // Already-valid ids are untouched, case and all.
        assert_eq!(cobol_word("BTN-OK"), "BTN-OK");
        assert_eq!(cobol_word("CustomerFile"), "CustomerFile");
        // Anything else that is not a letter or digit also becomes a hyphen,
        // runs collapse, and a COBOL word may not start or end with one.
        assert_eq!(cobol_word("my box.value"), "my-box-value");
        assert_eq!(cobol_word("__a__b__"), "a-b");
        assert_eq!(cobol_word("_leading"), "leading");
        assert_eq!(cobol_word("trailing_"), "trailing");

        // End to end: a form whose controls carry underscores generates data
        // items and paragraph references that hold no invalid character.
        let mut form = Form::new("MAIN-FORM", "Test", 800, 600);
        form.controls
            .push(Control::new("textbox_1", ControlType::TextBox, 10, 10));
        form.controls
            .push(Control::new("label_result", ControlType::Label, 10, 60));
        let src = generate(&form);
        assert!(src.contains("01 WS-textbox-1."), "group name normalised");
        assert!(src.contains("WS-textbox-1-TEXT"), "field name normalised");
        assert!(src.contains("WS-label-result-TEXT"), "label field");
        // The only place a raw id may still show is inside a quoted literal
        // (a control's default caption) — never as a name the lexer must read.
        for line in src.lines() {
            let outside_quotes: String = line
                .split('\'')
                .step_by(2)
                .collect::<Vec<_>>()
                .join(" ");
            assert!(
                !outside_quotes.contains('_'),
                "underscore reached a COBOL name: {line}"
            );
        }
        println!("cobol_word: textbox_1 → {}", cobol_word("textbox_1"));
    }

    #[test]
    fn generate_contains_program_id() {
        let src = generate(&make_form());
        assert!(src.contains("PROGRAM-ID. MAIN-FORM."), "missing PROGRAM-ID");
    }

    // ── Spec 039 T5: Knob/Gauge/Switch/FileDropZone codegen ────────────────

    #[test]
    fn knob_gauge_switch_file_drop_zone_emit_working_storage_fields() {
        let mut form = Form::new("MAIN-FORM", "Test", 800, 600);

        let mut knob = Control::new("KNB-1", ControlType::Knob, 10, 10);
        knob.set_prop("Value", PropValue::Int(42));
        knob.set_prop("Minimum", PropValue::Int(0));
        knob.set_prop("Maximum", PropValue::Int(100));
        knob.set_prop("Step", PropValue::Int(1));
        knob.set_prop("DefaultValue", PropValue::Int(0));
        form.controls.push(knob);

        let mut gauge = Control::new("GAU-1", ControlType::Gauge, 10, 60);
        gauge.set_prop("Value", PropValue::Int(70));
        gauge.set_prop("GaugeStyle", PropValue::String("Donut".into()));
        form.controls.push(gauge);

        let mut switch = Control::new("SWT-1", ControlType::Switch, 10, 110);
        switch.set_prop("Checked", PropValue::Bool(true));
        form.controls.push(switch);

        form.controls
            .push(Control::new("FDZ-1", ControlType::FileDropZone, 10, 160));

        let src = generate(&form);
        assert!(src.contains("01 WS-KNB-1."), "Knob group missing");
        assert!(src.contains("WS-KNB-1-VALUE      PIC S9(9) VALUE 42."));
        assert!(src.contains("WS-KNB-1-STEP"));
        assert!(src.contains("WS-KNB-1-DEFAULT"));

        assert!(src.contains("01 WS-GAU-1."), "Gauge group missing");
        assert!(src.contains("WS-GAU-1-VALUE      PIC S9(9) VALUE 70."));
        assert!(src.contains("WS-GAU-1-STYLE      PIC X(10)  VALUE 'Donut'."));

        assert!(src.contains("01 WS-SWT-1."), "Switch group missing");
        assert!(src.contains("WS-SWT-1-CHECKED    PIC 9      VALUE 1."));

        assert!(src.contains("01 WS-FDZ-1."), "FileDropZone group missing");
        assert!(src.contains("WS-FDZ-1-FILE-COUNT PIC S9(4) VALUE 0."));
        assert!(src.contains("WS-FDZ-1-FILE-PATH  PIC X(1024) OCCURS 20 TIMES"));

        // The developer banner is preserved for every generated program,
        // batch of new controls or not (tech.md hard constraint).
        assert!(src.contains("PowerRustCOBOL"), "developer banner missing");
    }

    #[test]
    fn a_form_without_the_new_controls_carries_none_of_their_fields() {
        // Regression guard on the additive-only claim (plan.md §3): a form
        // with none of the six spec-039 controls must not gain any of their
        // WORKING-STORAGE shape just because the generator now knows how to
        // emit it.
        let src = generate(&make_form());
        for needle in [
            "-STEP",
            "-DEFAULT",
            "-STYLE",
            "-CHECKED",
            "-FILE-COUNT",
            "-FILE-PATH",
            "-CENTER-LAT",
            "-CENTER-LNG",
            "-SEARCH-ENGINE-ID",
        ] {
            assert!(!src.contains(needle), "unexpected field marker: {needle}");
        }
    }

    #[test]
    fn maps_emits_center_and_zoom_but_never_an_api_key_literal() {
        let mut form = Form::new("MAIN-FORM", "Test", 800, 600);
        let mut map = Control::new("MAP-1", ControlType::Maps, 10, 10);
        map.set_prop("CenterLat", PropValue::String("48.8566".into()));
        map.set_prop("CenterLng", PropValue::String("2.3522".into()));
        map.set_prop("Zoom", PropValue::Int(12));
        form.controls.push(map);

        let src = generate(&form);
        assert!(src.contains("01 WS-MAP-1."), "Maps group missing");
        assert!(src.contains("WS-MAP-1-CENTER-LAT PIC X(32)  VALUE '48.8566'."));
        assert!(src.contains("WS-MAP-1-CENTER-LNG PIC X(32)  VALUE '2.3522'."));
        assert!(src.contains("WS-MAP-1-ZOOM       PIC S9(4)  VALUE 12."));
        // R31/R33: the resolved google_maps key is never generated-source
        // text — the Run path seeds it as a runtime-only property instead.
        assert!(!src.to_lowercase().contains("apikey"));
        assert!(!src.to_lowercase().contains("api-key"));
    }

    #[test]
    fn web_search_emits_instance_fields_and_search_paragraph() {
        let mut form = Form::new("MAIN-FORM", "Test", 800, 600);
        let mut search = Control::new("SEARCH-1", ControlType::WebSearch, 10, 10);
        search.set_prop("SearchEngineId", PropValue::String("abc123".into()));
        search.set_prop("NumResults", PropValue::Int(5));
        search.set_prop("SafeSearch", PropValue::String("Medium".into()));
        form.controls.push(search);

        let src = generate(&form);

        assert!(src.contains(
            "01 WS-SEARCH-1-SEARCH-ENGINE-ID PIC X(64)  VALUE 'abc123'."
        ));
        assert!(src.contains("01 WS-SEARCH-1-QUERY            PIC X(512) VALUE SPACES."));
        assert!(src.contains("01 WS-SEARCH-1-NUM-RESULTS      PIC 9(2)   VALUE 5."));
        assert!(src.contains("01 WS-SEARCH-1-SAFE-SEARCH      PIC X(6)   VALUE 'Medium'."));

        assert!(src.contains("SEARCH-1-SEARCH."));
        assert!(src.contains("STRING 'https://www.googleapis.com/customsearch/v1?cx='"));
        assert!(src.contains("WS-SEARCH-1-SEARCH-ENGINE-ID DELIMITED BY SPACE"));
        assert!(src.contains("WS-SEARCH-1-QUERY DELIMITED BY SPACE"));
        assert!(src.contains("WS-SEARCH-1-NUM-RESULTS DELIMITED BY SIZE"));
        assert!(src.contains("WS-SEARCH-1-SAFE-SEARCH DELIMITED BY SPACE"));
        assert!(src.contains("INTO WS-REQUEST-URL"));
        assert!(src.contains("CALL \"COBOL-HTTP-GET\""));
        assert!(src.contains("SEARCH-1-ON-RESULTS."));
        assert!(src.contains("SEARCH-1-ON-ERROR."));
        // R30/R31: no API key anywhere in generated source — INVOKE 'SEARCH'
        // (T15) is the credential-aware path, not this static paragraph.
        assert!(!src.to_lowercase().contains("apikey"));
        assert!(!src.to_lowercase().contains("api-key"));
        assert!(!src.contains("&key="));
    }

    #[test]
    fn user_line_map_covers_authored_handler_body_only() {
        // A handler WITH real user code + one with none.
        let mut form = Form::new("MAIN-FORM", "Test", 800, 600);
        let mut btn = Control::new("BTN-OK", ControlType::Button, 10, 10);
        let mut ev = EventBinding::for_control("BTN-OK", "onClick");
        ev.code = "           DISPLAY \"HELLO\".\n           DISPLAY \"WORLD\".".to_string();
        btn.events.push(ev);
        form.controls.push(btn);
        // An empty handler contributes no range (generated stub, not user code).
        let mut btn2 = Control::new("BTN-NO", ControlType::Button, 10, 40);
        btn2.events
            .push(EventBinding::for_control("BTN-NO", "onClick"));
        form.controls.push(btn2);

        let (src, ranges) = generate_with_user_lines(&form);
        let lines: Vec<&str> = src.lines().collect();

        assert!(!ranges.is_empty(), "expected at least one user range");
        // Every line inside a reported range must be one of the authored lines,
        // and the authored DISPLAY lines must be inside some range.
        let in_range = |n: u32| ranges.iter().any(|(s, e)| n >= *s && n <= *e);
        let hello = lines
            .iter()
            .position(|l| l.contains("DISPLAY \"HELLO\""))
            .map(|i| i as u32 + 1)
            .expect("HELLO line present");
        let world = lines
            .iter()
            .position(|l| l.contains("DISPLAY \"WORLD\""))
            .map(|i| i as u32 + 1)
            .expect("WORLD line present");
        assert!(in_range(hello), "HELLO must be user code");
        assert!(in_range(world), "WORLD must be user code");
        // The generated PROGRAM-ID header line is never user code.
        let pid = lines
            .iter()
            .position(|l| l.contains("PROGRAM-ID. MAIN-FORM."))
            .map(|i| i as u32 + 1)
            .unwrap();
        assert!(!in_range(pid), "generated PROGRAM-ID must not be user code");
    }

    #[test]
    fn generate_indexed_file_control_facade() {
        let mut form = Form::new("CUSTOMER-FORM", "Customers", 800, 600);
        let mut idx = Control::new("CustomerFile", ControlType::IndexedFile, 0, 0);
        idx.set_prop(
            "IndexedFile",
            PropValue::String("indexed/customers.cidx".into()),
        );
        idx.set_prop("OpenMode", PropValue::String("I-O".into()));
        idx.set_prop("AutoOpen", PropValue::Bool(true));
        idx.set_prop("RecordName", PropValue::String("CUSTOMER-REC".into()));
        idx.set_prop("KeyName", PropValue::String("CUSTOMER-ID".into()));
        form.controls.push(idx);

        let src = generate(&form);

        assert!(src.contains("PERFORM CustomerFile-OPEN"));
        assert!(src.contains("PERFORM CustomerFile-CLOSE"));
        assert!(src.contains("OPEN I-O CUSTOMERS"));
        assert!(src.contains("CustomerFile-START."));
        assert!(src.contains("CustomerFile-READ-NEXT."));
        assert!(src.contains("CustomerFile-READ-PREVIOUS."));
        assert!(src.contains("CustomerFile-WRITE."));
        assert!(src.contains("WRITE CUSTOMER-REC"));
        assert!(src.contains("CustomerFile-COMMIT."));
    }

    #[test]
    fn generate_weaves_cobol_structure_005() {
        use cobolt_forms::model::UserProcedure;
        let mut form = make_form();
        form.cobol_structure.special_names = "       DECIMAL-POINT IS COMMA.".into();
        form.cobol_structure.repository = "       FUNCTION ALL INTRINSIC.".into();
        form.cobol_structure.file_control = "           SELECT F ASSIGN TO \"f.dat\".".into();
        form.cobol_structure.file_section = "       FD  F.\n       01 F-REC PIC X(80).".into();
        form.user_procedures = vec![UserProcedure {
            name: "RECALC-TOTAL".into(),
            code: "       ENVIRONMENT DIVISION.\n       PROCEDURE DIVISION.\n           CONTINUE."
                .into(),
        }];
        let src = generate(&form);

        assert!(src.contains("CONFIGURATION SECTION."));
        assert!(src.contains("SPECIAL-NAMES.") && src.contains("DECIMAL-POINT IS COMMA."));
        assert!(src.contains("REPOSITORY.") && src.contains("FUNCTION ALL INTRINSIC."));
        assert!(src.contains("INPUT-OUTPUT SECTION.") && src.contains("FILE-CONTROL."));
        assert!(src.contains("SELECT F ASSIGN"));
        // FILE SECTION must come before WORKING-STORAGE SECTION.
        let fs = src.find("FILE SECTION.").expect("no FILE SECTION");
        let ws = src
            .find("WORKING-STORAGE SECTION.")
            .expect("no WORKING-STORAGE");
        assert!(fs < ws, "FILE SECTION must precede WORKING-STORAGE");
        // User procedure emitted as a nested program, IS COMMON so the event
        // handlers (sibling contained programs) may CALL it (spec 005 T6).
        assert!(src.contains("PROGRAM-ID. RECALC-TOTAL IS COMMON PROGRAM."));
        assert!(src.contains("END PROGRAM RECALC-TOTAL."));
        // Banner still present.
        assert!(src.contains("generated automatically by PowerRustCOBOL"));
    }

    #[test]
    fn generate_all_procedures_are_common_009() {
        // Spec 009 R4: every woven procedure — event handlers AND user procedures
        // — is `IS COMMON` so any procedure is callable from anywhere in the form.
        use cobolt_forms::model::UserProcedure;
        let mut form = make_form(); // has BTN-OK onClick handler
        form.user_procedures = vec![UserProcedure {
            name: "RECALC-TOTAL".into(),
            code: "       ENVIRONMENT DIVISION.\n       PROCEDURE DIVISION.\n           CONTINUE."
                .into(),
        }];
        let src = generate(&form);

        // The control event handler is now IS COMMON.
        assert!(
            src.contains("PROGRAM-ID. BTN-OK--ONCLICK IS COMMON PROGRAM."),
            "event handler must be IS COMMON (009 R4):\n{src}"
        );
        // Form-level OnLoad/OnClose handlers too.
        assert!(
            src.contains("IS COMMON PROGRAM.") && src.matches("IS COMMON PROGRAM.").count() >= 3,
            "all woven procedures (form events + control event + user proc) must be IS COMMON"
        );
        // And the user procedure (already COMMON pre-009) stays COMMON.
        assert!(src.contains("PROGRAM-ID. RECALC-TOTAL IS COMMON PROGRAM."));
    }

    #[test]
    fn generate_starts_with_developer_header() {
        let src = generate(&make_form());
        // GOLDEN RULE: a developer banner precedes IDENTIFICATION DIVISION.
        assert!(src.starts_with("      *>"), "header must be first");
        assert!(
            src.contains("generated automatically by PowerRustCOBOL RAD"),
            "missing generated-by line"
        );
        assert!(
            src.contains("DO NOT MODIFY IT DIRECTLY"),
            "missing do-not-modify warning"
        );
        assert!(src.contains("Apache 2.0 License"), "missing license line");
        // The banner sits above the program proper.
        let hdr = src.find("*>").unwrap();
        let id = src.find("IDENTIFICATION DIVISION.").unwrap();
        assert!(hdr < id, "header must come before IDENTIFICATION DIVISION");
    }

    #[test]
    fn generate_contains_event_loop() {
        let src = generate(&make_form());
        assert!(
            src.contains("COBOL-EVENT-LOOP"),
            "missing event loop paragraph"
        );
        assert!(src.contains("WHEN \"BTN-OK\""), "missing control WHEN");
        assert!(src.contains("WHEN \"onClick\""), "missing event WHEN");
        // v1.0.0: dispatch via CALL to nested program (double-hyphen name)
        assert!(
            src.contains("CALL \"BTN-OK--ONCLICK\""),
            "missing nested CALL dispatch"
        );
    }

    #[test]
    fn generate_contains_data_group() {
        let src = generate(&make_form());
        assert!(src.contains("01 WS-BTN-OK."), "missing WS group for BTN-OK");
    }

    #[test]
    fn generate_contains_nested_program() {
        let src = generate(&make_form());
        // v1.0.0: event handlers are nested COBOL-85 programs.
        // Spec 009 R4: each is `IS COMMON PROGRAM` (callable from anywhere in the form).
        assert!(
            src.contains("PROGRAM-ID. BTN-OK--ONCLICK IS COMMON PROGRAM."),
            "missing nested program ID"
        );
        assert!(
            src.contains("END PROGRAM BTN-OK--ONCLICK."),
            "missing END PROGRAM for handler"
        );
        assert!(src.contains("GOBACK."), "missing GOBACK in nested program");
        assert!(
            src.contains("END PROGRAM MAIN-FORM."),
            "missing outer END PROGRAM"
        );
    }

    #[test]
    fn form_level_events_dispatch_through_the_loop() {
        // A bound form-level event (onResize) must be dispatched under a WHEN
        // matching the form's own id — not just generated as a nested program.
        let mut form = Form::new("MAIN-FORM", "Test", 800, 600);
        form.form_events
            .push(EventBinding::for_control("MAIN-FORM", "onResize"));
        let src = generate(&form);
        assert!(
            src.contains("WHEN \"MAIN-FORM\""),
            "event loop missing a WHEN for the form id"
        );
        assert!(
            src.contains("WHEN \"onResize\""),
            "missing onResize event WHEN"
        );
        assert!(
            src.contains("CALL \"MAIN-FORM--ONRESIZE\""),
            "onResize not dispatched to its nested program"
        );
    }

    #[test]
    fn generate_contains_form_events_nested() {
        let src = generate(&make_form());
        // Form events (OnLoad / OnClose) also become nested programs
        // Paragraph names come from `derive_paragraph_name`, which uppercases the
        // event name without inserting separators: "OnLoad" → "ONLOAD".
        // Spec 009 R4: form-event handlers are `IS COMMON PROGRAM` too.
        assert!(
            src.contains("PROGRAM-ID. MAIN-FORM--ONLOAD IS COMMON PROGRAM."),
            "missing OnLoad nested program"
        );
        assert!(
            src.contains("PROGRAM-ID. MAIN-FORM--ONCLOSE IS COMMON PROGRAM."),
            "missing OnClose nested program"
        );
    }

    #[test]
    fn generate_calls_on_load_nested() {
        let src = generate(&make_form());
        // Form::new() pre-populates form_events with OnLoad; COBOL-MAIN must CALL it.
        assert!(
            src.contains("CALL \"MAIN-FORM--ONLOAD\""),
            "missing OnLoad CALL in COBOL-MAIN"
        );
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

    /// Event-handler source stored in the model is emitted into the nested
    /// program body — including its own WORKING-STORAGE declarations.
    #[test]
    fn generate_emits_event_handler_code() {
        let mut form = Form::new("MAIN-FORM", "Test", 800, 600);
        let mut btn = Control::new("BTN-OK", ControlType::Button, 10, 10);
        let mut ev = EventBinding::for_control("BTN-OK", "onClick");
        ev.code = "\
       ENVIRONMENT DIVISION.\n\
       DATA DIVISION.\n\
       WORKING-STORAGE SECTION.\n\
       01 WS-LOCAL-FLAG  PIC 9 VALUE 0.\n\
       LINKAGE SECTION.\n\
\n\
       PROCEDURE DIVISION.\n\
           MOVE 1 TO COBOL-QUIT."
            .into();
        btn.events.push(ev);
        form.controls.push(btn);

        let src = generate(&form);
        assert!(
            src.contains("MOVE 1 TO COBOL-QUIT"),
            "handler statements must appear in the nested program body"
        );
        assert!(
            src.contains("WS-LOCAL-FLAG"),
            "handler-local WORKING-STORAGE must appear in the nested program"
        );
    }

    /// An unwritten handler is emitted from the shared template (it still
    /// compiles): LINKAGE SECTION present, PROCEDURE DIVISION + CONTINUE.
    #[test]
    fn generate_emits_template_stub_for_empty_handler() {
        let mut form = Form::new("MAIN-FORM", "Test", 800, 600);
        let mut btn = Control::new("BTN-OK", ControlType::Button, 10, 10);
        btn.events
            .push(EventBinding::for_control("BTN-OK", "onClick")); // no code
        form.controls.push(btn);

        let src = generate(&form);
        assert!(
            src.contains("LINKAGE SECTION."),
            "the stub handler must include a LINKAGE SECTION"
        );
        assert!(
            src.contains("PROCEDURE DIVISION."),
            "the stub handler must include a PROCEDURE DIVISION"
        );
    }

    /// A control that belongs to a repeating group (array) gets the indexed
    /// handler stub: it receives the fired item's 1-based array index.
    #[test]
    fn array_member_handler_stub_receives_array_index() {
        let mut form = Form::new("MAIN-FORM", "Test", 800, 600);
        let mut group = Control::new("CARD", ControlType::GroupBox, 0, 0);
        group.set_prop("IsRepeatingGroup", PropValue::Bool(true));
        let mut btn = Control::new("Button-1", ControlType::Button, 10, 10);
        btn.parent = Some("CARD".into());
        btn.events
            .push(EventBinding::for_control("Button-1", "onClick")); // no code
        form.controls.push(group);
        form.controls.push(btn);

        let src = generate(&form);
        assert!(
            src.contains("01 CONTROL-ARRAY-INDEX              PIC S9(4) COMP-5."),
            "array-member handler must declare CONTROL-ARRAY-INDEX:\n{src}"
        );
        assert!(
            src.contains("PROCEDURE DIVISION USING CONTROL-ARRAY-INDEX."),
            "array-member handler must receive the index via USING:\n{src}"
        );
        assert!(
            src.contains("Button-1(CONTROL-ARRAY-INDEX)::BackgroundColor"),
            "stub should hint indexed member access:\n{src}"
        );

        // A non-array control keeps the plain stub (no index parameter).
        let mut plain = Form::new("MAIN-FORM", "Test", 800, 600);
        let mut btn2 = Control::new("BTN-OK", ControlType::Button, 10, 10);
        btn2.events
            .push(EventBinding::for_control("BTN-OK", "onClick"));
        plain.controls.push(btn2);
        let plain_src = generate(&plain);
        assert!(!plain_src.contains("CONTROL-ARRAY-INDEX"));
    }

    fn binding_fields() -> Vec<BindingField> {
        vec![
            BindingField::new("ID", BindingDataType::Integer).key(),
            BindingField::new("NAME", BindingDataType::Text).required(),
            BindingField::new("AMOUNT", BindingDataType::Decimal).required(),
        ]
    }

    fn data_binding_fixture_form() -> Form {
        let mut form = Form::new("BIND-FORM", "Bindings", 800, 600);
        form.add_control(Control::new("GRID-1", ControlType::DataGrid, 0, 0));
        form.add_control(Control::new("CHART-1", ControlType::BarChart, 0, 120));
        form.add_control(Control::new("COMBO-1", ControlType::ComboBox, 0, 240));
        form.add_control(Control::new("LIST-1", ControlType::ListBox, 0, 300));
        let mut group = Control::new("ROWS", ControlType::GroupBox, 0, 360);
        group.set_prop("IsRepeatingGroup", PropValue::Bool(true));
        group.set_prop("ArrayName", PropValue::String("CUSTOMERS".into()));
        let mut name = Control::new("NAME", ControlType::TextBox, 10, 390);
        name.parent = Some("ROWS".into());
        form.add_control(group);
        form.add_control(name);

        let fields = binding_fields();
        form.data_bindings.push(
            DataBindingDef::new(
                "BIND-IDX-GRID",
                "Indexed Grid",
                BindingSourceDescriptor::IndexedFile {
                    definition_path: "data/customers.cidx".into(),
                    record_name: "CUSTOMER-REC".into(),
                    fields: fields.clone(),
                    key_field: Some("ID".into()),
                    writable: true,
                },
                BindingTargetDescriptor::DataGrid {
                    control_id: "GRID-1".into(),
                },
            )
            .with_mappings(vec![
                FieldMapping::new(
                    "ID",
                    BindingTargetPath::GridColumn {
                        control_id: "GRID-1".into(),
                        column_id: "ID".into(),
                    },
                ),
                FieldMapping::new(
                    "NAME",
                    BindingTargetPath::GridColumn {
                        control_id: "GRID-1".into(),
                        column_id: "NAME".into(),
                    },
                ),
            ]),
        );
        form.data_bindings.push(
            DataBindingDef::new(
                "BIND-SQL-ARRAY",
                "SQL Array",
                BindingSourceDescriptor::Sql {
                    source_control_id: "SQL-1".into(),
                    query_name: "CUSTOMERS".into(),
                    result_set_name: "CUSTOMER-ROWS".into(),
                    fields: fields.clone(),
                    key_fields: vec!["ID".into()],
                    writable: true,
                },
                BindingTargetDescriptor::ControlArray {
                    array_id: "CUSTOMERS".into(),
                    member_control_ids: vec!["NAME".into()],
                },
            )
            .with_mappings(vec![FieldMapping::new(
                "NAME",
                BindingTargetPath::ControlProperty {
                    array_id: "CUSTOMERS".into(),
                    control_id: "NAME".into(),
                    property_name: "Text".into(),
                },
            )]),
        );
        form.data_bindings.push(
            DataBindingDef::new(
                "BIND-TABLE-CHART",
                "Table Chart",
                BindingSourceDescriptor::CobolTable {
                    table_name: "CUSTOMER-TABLE".into(),
                    occurs_item: "CUSTOMER-ROW".into(),
                    fields: fields.clone(),
                    key_fields: vec!["ID".into()],
                    writable: true,
                },
                BindingTargetDescriptor::Chart {
                    control_id: "CHART-1".into(),
                    chart_kind: cobolt_forms::BindingChartKind::Bar,
                },
            )
            .with_mappings(vec![
                FieldMapping::new(
                    "NAME",
                    BindingTargetPath::ChartCategory {
                        control_id: "CHART-1".into(),
                    },
                ),
                FieldMapping::new(
                    "AMOUNT",
                    BindingTargetPath::ChartValueSeries {
                        control_id: "CHART-1".into(),
                        series_id: "AMOUNT".into(),
                    },
                ),
            ]),
        );
        form.data_bindings.push(DataBindingDef::new(
            "BIND-REST-LIST",
            "REST List",
            BindingSourceDescriptor::RestApi {
                source_control_id: "REST-1".into(),
                endpoint_name: "GET-CUSTOMERS".into(),
                response_data_item: "REST-RESPONSE".into(),
                fields: fields.clone(),
                update: None,
            },
            BindingTargetDescriptor::ListBox {
                control_id: "LIST-1".into(),
            },
        ));
        form.data_bindings.push(
            DataBindingDef::new(
                "BIND-AGENT-COMBO",
                "Agent Combo",
                BindingSourceDescriptor::AgentAi {
                    source_control_id: "AGENT-1".into(),
                    output_name: "CUSTOMER-CHOICES".into(),
                    fields,
                    update: None,
                },
                BindingTargetDescriptor::ComboBox {
                    control_id: "COMBO-1".into(),
                },
            )
            .with_mappings(vec![
                FieldMapping::new(
                    "NAME",
                    BindingTargetPath::ListDisplayItem {
                        control_id: "COMBO-1".into(),
                    },
                ),
                FieldMapping::new(
                    "ID",
                    BindingTargetPath::ListValue {
                        control_id: "COMBO-1".into(),
                    },
                ),
            ]),
        );
        form
    }

    #[test]
    fn data_binding_codegen_emits_deterministic_runtime_sections() {
        let src = generate(&data_binding_fixture_form());
        assert!(src.contains("COBOL-DATA-BINDINGS-LOAD."));
        assert!(src.contains("COBOL-DATA-BINDINGS-POPULATE."));
        assert!(src.contains("COBOL-DATA-BINDINGS-MARK-CLEAN."));
        assert!(src.contains("COBOL-DATA-BINDINGS-UPDATE."));
        assert!(src.contains("CALL \"COBOL-BINDING-LOAD\" USING \"BIND-IDX-GRID\""));
        assert!(src.contains("IndexedFile:CUSTOMER-REC"));
        assert!(src.contains("SQL:CUSTOMER-ROWS"));
        assert!(src.contains("COBOLTable:CUSTOMER-TABLE"));
        assert!(src.contains("REST:GET-CUSTOMERS"));
        assert!(src.contains("AgentAI:CUSTOMER-CHOICES"));
        assert!(src.contains("NAME -> GRID-1.NAME"));
        assert!(src.contains("NAME -> CHART-1.Category"));
        assert!(src.contains("NAME -> NAME.Text"));
        assert!(src.contains("NAME -> COMBO-1.Display"));
    }

    #[test]
    fn data_binding_codegen_is_stable_across_runs() {
        let form = data_binding_fixture_form();
        assert_eq!(generate(&form), generate(&form));
    }

    #[test]
    fn data_binding_codegen_seeds_datagrid_refresh_identity_for_cobol_tables() {
        let mut form = Form::new("BIND-FORM", "Bindings", 800, 600);
        form.add_control(Control::new("GRID-1", ControlType::DataGrid, 0, 0));
        form.data_bindings.push(DataBindingDef::new(
            "BIND-TABLE-GRID",
            "Table Grid",
            BindingSourceDescriptor::CobolTable {
                table_name: "CUSTOMER-TABLE".into(),
                occurs_item: "CUSTOMER-ROW".into(),
                fields: binding_fields(),
                key_fields: vec!["ID".into()],
                writable: true,
            },
            BindingTargetDescriptor::DataGrid {
                control_id: "GRID-1".into(),
            },
        ));

        let src = generate(&form);

        assert!(src.contains(
            "INVOKE GRID-1 'SetProperty' USING BY CONTENT \"_BindingKind\" BY CONTENT \"CobolTable\""
        ));
        assert!(src.contains(
            "INVOKE GRID-1 'SetProperty' USING BY CONTENT \"_BindingFields\" BY CONTENT \"ID,NAME,AMOUNT\""
        ));
    }

    #[test]
    fn data_binding_codegen_seeds_scalar_control_refresh_identity_for_cobol_tables() {
        // Spec 039 T13 (retroactively completing T6): a standalone Knob's
        // binding must reach a genuinely standalone `rcrun build` binary,
        // not just an interpreted `rcrun run-form` — this is the codegen
        // half of that (the Rust-side seeding in `cobolt-cli/src/
        // form_gui.rs` covers the interpreted path).
        let mut form = Form::new("BIND-FORM", "Bindings", 800, 600);
        form.add_control(Control::new("KNOB-1", ControlType::Knob, 0, 0));
        form.data_bindings.push(DataBindingDef::new(
            "BIND-KNOB",
            "Knob",
            BindingSourceDescriptor::CobolTable {
                table_name: "READING-TABLE".into(),
                occurs_item: "READING-ROW".into(),
                fields: vec![cobolt_forms::BindingField::new(
                    "READING-VALUE",
                    cobolt_forms::BindingDataType::Decimal,
                )
                .required()],
                key_fields: vec![],
                writable: false,
            },
            BindingTargetDescriptor::ScalarControl {
                control_id: "KNOB-1".into(),
            },
        ).with_mappings(vec![cobolt_forms::FieldMapping::new(
            "READING-VALUE",
            BindingTargetPath::ScalarValue {
                control_id: "KNOB-1".into(),
            },
        )]));

        let src = generate(&form);

        assert!(src.contains(
            "INVOKE KNOB-1 'SetProperty' USING BY CONTENT \"_BindingKind\" BY CONTENT \"CobolTable\""
        ));
        assert!(src.contains(
            "INVOKE KNOB-1 'SetProperty' USING BY CONTENT \"_BindingScalarField\" BY CONTENT \"READING-VALUE\""
        ));
        assert!(src.contains(
            "INVOKE KNOB-1 'SetProperty' USING BY CONTENT \"_BindingScalarProperty\" BY CONTENT \"Value\""
        ));
        assert!(src.contains("INVOKE KNOB-1 'RefreshBinding'"));
    }

    #[test]
    fn data_binding_codegen_seeds_switch_scalar_property_as_checked() {
        let mut form = Form::new("BIND-FORM", "Bindings", 800, 600);
        form.add_control(Control::new("SWITCH-1", ControlType::Switch, 0, 0));
        form.data_bindings.push(DataBindingDef::new(
            "BIND-SWITCH",
            "Switch",
            BindingSourceDescriptor::CobolTable {
                table_name: "ALARM-TABLE".into(),
                occurs_item: "ALARM-ROW".into(),
                fields: vec![cobolt_forms::BindingField::new(
                    "ALARM-ON",
                    cobolt_forms::BindingDataType::Boolean,
                )
                .required()],
                key_fields: vec![],
                writable: false,
            },
            BindingTargetDescriptor::ScalarControl {
                control_id: "SWITCH-1".into(),
            },
        ).with_mappings(vec![cobolt_forms::FieldMapping::new(
            "ALARM-ON",
            BindingTargetPath::ScalarValue {
                control_id: "SWITCH-1".into(),
            },
        )]));

        let src = generate(&form);

        assert!(src.contains(
            "INVOKE SWITCH-1 'SetProperty' USING BY CONTENT \"_BindingScalarProperty\" BY CONTENT \"Checked\""
        ));
    }

    #[test]
    fn data_binding_codegen_seeds_marker_collection_fields_for_cobol_tables() {
        let mut form = Form::new("BIND-FORM", "Bindings", 800, 600);
        form.add_control(Control::new("MAP-1", ControlType::Maps, 0, 0));
        form.data_bindings.push(DataBindingDef::new(
            "BIND-MARKERS",
            "Markers",
            BindingSourceDescriptor::CobolTable {
                table_name: "PLACE-TABLE".into(),
                occurs_item: "PLACE-ROW".into(),
                fields: vec![
                    cobolt_forms::BindingField::new(
                        "PLACE-LAT",
                        cobolt_forms::BindingDataType::Decimal,
                    )
                    .required(),
                    cobolt_forms::BindingField::new(
                        "PLACE-LNG",
                        cobolt_forms::BindingDataType::Decimal,
                    )
                    .required(),
                    cobolt_forms::BindingField::new(
                        "PLACE-NAME",
                        cobolt_forms::BindingDataType::Text,
                    )
                    .required(),
                ],
                key_fields: vec![],
                writable: false,
            },
            BindingTargetDescriptor::MarkerCollection {
                control_id: "MAP-1".into(),
            },
        ).with_mappings(vec![
            cobolt_forms::FieldMapping::new(
                "PLACE-LAT",
                BindingTargetPath::MarkerField {
                    control_id: "MAP-1".into(),
                    field: cobolt_forms::MapMarkerField::Lat,
                },
            ),
            cobolt_forms::FieldMapping::new(
                "PLACE-LNG",
                BindingTargetPath::MarkerField {
                    control_id: "MAP-1".into(),
                    field: cobolt_forms::MapMarkerField::Lng,
                },
            ),
            cobolt_forms::FieldMapping::new(
                "PLACE-NAME",
                BindingTargetPath::MarkerField {
                    control_id: "MAP-1".into(),
                    field: cobolt_forms::MapMarkerField::Label,
                },
            ),
        ]));

        let src = generate(&form);

        assert!(src.contains(
            "INVOKE MAP-1 'SetProperty' USING BY CONTENT \"_BindingMarkerFields\" BY CONTENT \"\tPLACE-LAT\tPLACE-LNG\tPLACE-NAME\t\""
        ));
        assert!(src.contains("INVOKE MAP-1 'RefreshBinding'"));
    }

    #[test]
    fn data_binding_codegen_omits_marker_seed_when_lat_or_lng_unmapped() {
        // The Guardian should already block this before Build/Run/Debug, but
        // codegen must not emit a half-formed seed either — no lat/lng means
        // no INVOKE at all, never a spec with an empty required slot.
        let mut form = Form::new("BIND-FORM", "Bindings", 800, 600);
        form.add_control(Control::new("MAP-1", ControlType::Maps, 0, 0));
        form.data_bindings.push(DataBindingDef::new(
            "BIND-MARKERS",
            "Markers",
            BindingSourceDescriptor::CobolTable {
                table_name: "PLACE-TABLE".into(),
                occurs_item: "PLACE-ROW".into(),
                fields: vec![cobolt_forms::BindingField::new(
                    "PLACE-LAT",
                    cobolt_forms::BindingDataType::Decimal,
                )
                .required()],
                key_fields: vec![],
                writable: false,
            },
            BindingTargetDescriptor::MarkerCollection {
                control_id: "MAP-1".into(),
            },
        ).with_mappings(vec![cobolt_forms::FieldMapping::new(
            "PLACE-LAT",
            BindingTargetPath::MarkerField {
                control_id: "MAP-1".into(),
                field: cobolt_forms::MapMarkerField::Lat,
            },
        )]));

        let src = generate(&form);

        assert!(!src.contains("_BindingMarkerFields"));
        assert!(!src.contains("INVOKE MAP-1 'RefreshBinding'"));
    }

    #[test]
    fn datagrid_csv_export_codegen_uses_button_mode_and_order_comment() {
        let mut form = Form::new("CSV-FORM", "CSV", 800, 600);
        let mut grid = Control::new("GRID-1", ControlType::DataGrid, 0, 0);
        grid.set_prop("ExportCSV", PropValue::Bool(false));
        grid.set_prop("ShowCSVExportButton", PropValue::Bool(true));
        grid.set_prop("CSVExportMode", PropValue::String("All".into()));
        grid.set_prop("CSVDelimiter", PropValue::String(";".into()));
        form.add_control(grid);

        let src = generate(&form);

        assert!(src.contains("DataGrid GRID-1 CSV export"));
        assert!(src.contains("Delimiter: \";\". Mode: All."));
        assert!(src.contains("Column order and filtered/all rows follow the DataGrid settings."));
        assert!(src.contains("INVOKE GRID-1 'ExportCSV'"));
    }

    /// A ToolBar whose buttons carry `procedure:` and `open-modal:`.
    fn toolbar_form(buttons: &[(&str, &str, &str)]) -> Form {
        use cobolt_forms::toolbar::{ToolbarButton, ToolbarDef, ToolbarGroup, TOOLBAR_DEF_PROP};
        let mut form = Form::new("MAIN-FORM", "Test", 800, 600);
        let mut group = ToolbarGroup::new("group-1", "File");
        for (id, label, action) in buttons {
            let mut b = ToolbarButton::new(*id, *label);
            b.action = (*action).to_owned();
            group.buttons.push(b);
        }
        let def = ToolbarDef {
            groups: vec![group],
            button_gap: 4,
        };
        let mut bar = Control::new("TOOLBAR-1", ControlType::ToolBar, 0, 0);
        bar.set_prop(TOOLBAR_DEF_PROP, PropValue::String(def.to_json().unwrap()));
        form.add_control(bar);
        form
    }

    /// `procedure:` and `open-modal:` were offered by the editor, documented in
    /// the guide — and reached nothing. The event loop can only dispatch what it
    /// has a `WHEN` for, and a toolbar button is not a `Control`, so the
    /// per-control walk never saw one. Codegen now reads each ToolBar's
    /// definition and dispatches under the button's derived id.
    #[test]
    fn a_toolbar_buttons_procedure_and_modal_are_dispatched() {
        let form = toolbar_form(&[
            ("button-1", "Save", "procedure:UPDATE-TOTAL"),
            ("button-2", "Find", "open-modal:CUST-LOOKUP"),
            // Neither of these belongs in the loop: `event` IS the toolbar's own
            // onClick, and the platform carries `print` out without COBOL.
            ("button-3", "Log", "event"),
            ("button-4", "Print", "print:/tmp/report.pdf"),
        ]);
        let src = generate(&form);

        assert!(
            src.contains("WHEN \"TOOLBAR-1-GROUP-1-BUTTON-1\""),
            "the procedure button has no WHEN\n{src}"
        );
        assert!(
            src.contains("CALL \"UPDATE-TOTAL\""),
            "the procedure is never called\n{src}"
        );
        assert!(
            src.contains("WHEN \"TOOLBAR-1-GROUP-1-BUTTON-2\""),
            "the open-modal button has no WHEN\n{src}"
        );
        assert!(
            src.contains("INVOKE ME::\"OpenFormSync\"(\"CUST-LOOKUP\")"),
            "the modal form is never opened\n{src}"
        );
        for dead in [
            "WHEN \"TOOLBAR-1-GROUP-1-BUTTON-3\"",
            "WHEN \"TOOLBAR-1-GROUP-1-BUTTON-4\"",
        ] {
            assert!(!src.contains(dead), "{dead} does not belong in the loop\n{src}");
        }
        // The derived id the renderer fires is the one the loop waits for.
        assert_eq!(
            cobolt_forms::toolbar::button_control_id("TOOLBAR-1", "group-1", "button-1"),
            "TOOLBAR-1-GROUP-1-BUTTON-1"
        );

        println!(
            "\n  Toolbar dispatch — 4 buttons: `procedure:UPDATE-TOTAL` ⇒ WHEN \
             \"TOOLBAR-1-GROUP-1-BUTTON-1\" CALL \"UPDATE-TOTAL\", \
             `open-modal:CUST-LOOKUP` ⇒ INVOKE ME::\"OpenFormSync\", and `event` + \
             `print:` get no WHEN (the toolbar's own onClick and the platform do those)\n"
        );
    }

    /// A button that cannot be dispatched says so in the generated source rather
    /// than emitting a `WHEN` that can never fire.
    #[test]
    fn a_toolbar_button_that_cannot_be_dispatched_is_reported() {
        // Nothing named.
        let src = generate(&toolbar_form(&[
            ("button-1", "A", "procedure:"),
            ("button-2", "B", "open-modal:"),
        ]));
        assert!(
            src.contains("asks to run a procedure but names none"),
            "an empty procedure target must be reported\n{src}"
        );
        assert!(
            src.contains("asks to open a modal form but names none"),
            "an empty modal target must be reported\n{src}"
        );
        assert!(
            !src.contains("WHEN \"TOOLBAR-1-GROUP-1-BUTTON-1\""),
            "…and must not get a WHEN\n{src}"
        );

        // An id longer than COBOL-CONTROL-ID could never match its own WHEN, so
        // it is reported instead of quietly generating a dead button.
        use cobolt_forms::toolbar::MAX_BUTTON_CONTROL_ID;
        let long_group = "g".repeat(MAX_BUTTON_CONTROL_ID);
        let id = cobolt_forms::toolbar::button_control_id("TOOLBAR-1", &long_group, "button-1");
        assert!(id.len() > MAX_BUTTON_CONTROL_ID);
        let mut form = toolbar_form(&[("button-1", "A", "procedure:UPDATE-TOTAL")]);
        {
            use cobolt_forms::toolbar::{ToolbarDef, TOOLBAR_DEF_PROP};
            let bar = &mut form.controls[0];
            let mut def = ToolbarDef::from_control(bar);
            def.groups[0].id = long_group;
            bar.set_prop(TOOLBAR_DEF_PROP, PropValue::String(def.to_json().unwrap()));
        }
        let src = generate(&form);
        assert!(
            src.contains("cannot be dispatched: its id"),
            "an over-long derived id must be reported\n{src}"
        );
        assert!(
            !src.contains("CALL \"UPDATE-TOTAL\""),
            "…and must not be dispatched\n{src}"
        );

        println!(
            "\n  Toolbar dispatch, refusals — `procedure:` and `open-modal:` with no \
             target are reported in the generated source and get no WHEN; a derived id \
             over {MAX_BUTTON_CONTROL_ID} characters (COBOL-CONTROL-ID's width) is \
             reported rather than generating a WHEN that could never fire\n"
        );
    }

    #[test]
    fn data_binding_e2e_generates_all_source_and_target_families() {
        let src = generate(&data_binding_fixture_form());
        for needle in [
            "This code was generated automatically by PowerRustCOBOL RAD.",
            "IndexedFile:CUSTOMER-REC",
            "SQL:CUSTOMER-ROWS",
            "COBOLTable:CUSTOMER-TABLE",
            "REST:GET-CUSTOMERS",
            "AgentAI:CUSTOMER-CHOICES",
            "DataGrid:GRID-1",
            "Chart:CHART-1",
            "ComboBox:COMBO-1",
            "ListBox:LIST-1",
            "ControlArray:CUSTOMERS",
            "COBOL-DATA-BINDINGS-UPDATE.",
        ] {
            assert!(src.contains(needle), "missing {needle}\n{src}");
        }
    }
}

// ── Source-map tests (spec 053 T9) ───────────────────────────────────────────

#[cfg(test)]
mod source_map_tests {
    use super::*;
    use cobolt_forms::code_site::{all_sites_fixture, fixture_markers, site_text};

    /// AC3: the marker planted at a known line of each of the eight in-form
    /// site kinds resolves to exactly that site and that site line (±0),
    /// untidy input included — the WORKING-STORAGE marker sits behind two
    /// skipped leading blank lines.
    #[test]
    fn source_map_finds_every_marker_at_its_site_line() {
        let form = all_sites_fixture();
        let (src, map) = generate_with_map(&form);
        let lines: Vec<&str> = src.lines().collect();

        println!("── per-kind marker resolution (AC3) ─────────────────────");
        for (site, marker, site_line) in fixture_markers() {
            let gen_line = lines
                .iter()
                .position(|l| l.contains(marker))
                .map(|i| i as u32 + 1)
                .unwrap_or_else(|| panic!("marker {marker} not in generated source"));
            let (got_site, got_line) = map
                .resolve(gen_line)
                .unwrap_or_else(|| panic!("marker {marker} (gen line {gen_line}) unmapped"));
            assert_eq!(got_site, &site, "marker {marker} owned by the wrong site");
            assert_eq!(
                got_line, site_line,
                "marker {marker} at the wrong site line"
            );
            println!(
                "  {marker:<26} gen {gen_line:>3} → {:<40} site line {got_line}",
                site.display_path(&form.name)
            );
        }
    }

    /// AC2: the map accounts for 100 % of the generated lines — every line a
    /// span covers is verbatim the site's own text at the resolved site line,
    /// and every developer marker line is covered by a span. Prints the span
    /// table and the attributed/total counts.
    #[test]
    fn source_map_attributes_every_span_line_verbatim() {
        let form = all_sites_fixture();
        let (src, map) = generate_with_map(&form);
        let lines: Vec<&str> = src.lines().collect();
        let total = lines.len() as u32;
        let mut attributed = 0u32;

        println!("── span table: site → generated range → site lines ─────");
        for span in &map.spans {
            println!(
                "  {:<44} gen {:>3}-{:<3} site lines {}-{}",
                span.site.display_path(&form.name),
                span.gen_start,
                span.gen_end,
                span.site_line_at_start,
                span.site_line_at_start + (span.gen_end - span.gen_start)
            );
        }

        for gen_line in 1..=total {
            let Some((site, site_line)) = map.resolve(gen_line) else {
                continue; // codegen-authored (R12) — the banner, scaffolding, stubs
            };
            attributed += 1;
            let text = site_text(&form, site).expect("mapped site has text");
            let expected = text
                .lines()
                .nth(site_line as usize - 1)
                .unwrap_or_else(|| {
                    panic!(
                        "gen line {gen_line} resolves past the end of {}",
                        site.display_path(&form.name)
                    )
                });
            let got = lines[gen_line as usize - 1];
            // The weave may trim trailing whitespace off a body's LAST line and
            // append the terminating period a section author omitted; nothing
            // else may differ.
            let matches = got == expected
                || got == expected.trim_end()
                || got == format!("{}.", expected.trim_end());
            assert!(
                matches,
                "gen line {gen_line} is not the site's text:\n  gen : {got:?}\n  site: {expected:?}"
            );
        }

        // Every marker line is inside a span (the developer's code is never
        // silently unattributed).
        for (_, marker, _) in fixture_markers() {
            let gen_line = lines
                .iter()
                .position(|l| l.contains(marker))
                .map(|i| i as u32 + 1)
                .unwrap();
            assert!(
                map.resolve(gen_line).is_some(),
                "marker {marker} must be attributed"
            );
        }
        println!("  {attributed}/{total} generated lines attributed to a site; the rest are codegen's own");
        assert!(attributed > 0, "the fixture must attribute lines");
    }

    /// The stub emitted for an EMPTY handler is codegen's code, not the
    /// developer's: every line of it resolves to no site (R12; the stub half
    /// of AC7).
    #[test]
    fn source_map_leaves_an_empty_handler_stub_unattributed() {
        let form = all_sites_fixture();
        let (src, map) = generate_with_map(&form);
        let lines: Vec<&str> = src.lines().collect();

        let start = lines
            .iter()
            .position(|l| l.contains("PROGRAM-ID. BTN-EMPTY--ONCLICK"))
            .expect("stub program present");
        let end = lines
            .iter()
            .position(|l| l.contains("END PROGRAM BTN-EMPTY--ONCLICK"))
            .expect("stub program closed");
        assert!(end > start);
        for (idx, _) in lines.iter().enumerate().take(end + 1).skip(start) {
            let gen_line = idx as u32 + 1;
            assert!(
                map.resolve(gen_line).is_none(),
                "stub line {gen_line} ({:?}) must resolve to no site",
                lines[idx]
            );
        }
        println!(
            "  stub BTN-EMPTY--ONCLICK: gen lines {}-{} all unattributed (generated code)",
            start + 1,
            end + 1
        );
    }

    /// Wrapper parity (R29): `generate_with_user_lines` derives exactly the
    /// handler/procedure body ranges from the map — section spans are not
    /// bodies and never leak into the debugger's view of user code.
    #[test]
    fn source_map_user_body_ranges_exclude_sections() {
        let form = all_sites_fixture();
        let (_, map) = generate_with_map(&form);
        let (_, ranges) = generate_with_user_lines(&form);
        assert_eq!(ranges, map.user_body_ranges());
        // The fixture has three bodies: onLoad, BTN-GO onClick, VALIDATE-CUSTOMER.
        assert_eq!(ranges.len(), 3, "three authored bodies");
        // And five section spans that must NOT be in the body ranges.
        let sections = map
            .spans
            .iter()
            .filter(|s| matches!(s.site, CodeSite::Section(_)))
            .count();
        assert_eq!(sections, 5, "five woven sections recorded");
    }
}
