// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The kit's content, once, for every coding agent** (spec 080 R20, T4.1).
//!
//! [`KitContent`] is plain data: the brief's rules, the task skills, the
//! reviewer, the gap-report template, the reference pack, which servers to
//! connect to and what the permissions should allow. It never names a target
//! agent and never decides a file layout: the Claude Code plugin bundle (spec
//! 084) turns it into that agent's files. A second agent adds a writer, never
//! a line here.
//!
//! All text is English (R22); every COBOL example writes a built-in inline as
//! `COBOL::"NAME" ( … )`.

use serde::Serialize;

/// The MCP server the IDE serves (HTTP, `127.0.0.1`).
pub const IDE_SERVER: &str = "powerrustcobol-ide";
/// The MCP server `rcrun mcp` serves (stdio, IDE closed).
pub const STDIO_SERVER: &str = "powerrustcobol";
/// Where a gap report goes, relative to the project folder (R17).
pub const GAP_REPORT_DIR: &str = "docs/compiler-requests";
/// Where the reference pack goes, relative to the project folder (R2).
pub const REFERENCE_DIR: &str = "docs/powerrustcobol";

/// Everything an external coding agent is told, independent of which agent.
#[derive(Debug, Clone, Serialize)]
pub struct KitContent {
    /// The product that wrote the kit, and its version (R5, R18).
    pub product: String,
    pub ide_version: String,
    /// The project's name, scrubbed of personal details before it gets here.
    pub project_name: String,
    /// The opening paragraphs of the brief.
    pub brief_intro: Vec<String>,
    /// The project layout an agent works in: `(path, what lives there)`.
    pub layout: Vec<(String, String)>,
    /// The standing rules, as instructions (R8, R16, R19 and the tool rules).
    pub brief_rules: Vec<Rule>,
    /// The project tools, as the tool set describes them.
    pub tools: Vec<ToolInfo>,
    pub skills: Vec<Skill>,
    pub reviewer: Reviewer,
    pub gap_template: GapTemplate,
    pub reference: Vec<RefDoc>,
    pub servers: Servers,
    pub permissions: PermissionIntent,
}

/// One standing rule of the brief. `id` is stable so a test can find it.
#[derive(Debug, Clone, Serialize)]
pub struct Rule {
    pub id: String,
    pub text: String,
}

/// One project tool: its MCP name and what it does.
#[derive(Debug, Clone, Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
}

/// One task skill (R9).
#[derive(Debug, Clone, Serialize)]
pub struct Skill {
    /// Lower-case, hyphenated; unique within the kit.
    pub name: String,
    /// What it is for and when to use it — one or two sentences.
    pub summary: String,
    /// The steps, in order.
    pub steps: Vec<String>,
    /// The project tools the steps call.
    pub uses_tools: Vec<String>,
    /// An optional worked example (Markdown, usually a fenced block).
    pub example: Option<String>,
}

/// The reviewer subagent (R10).
#[derive(Debug, Clone, Serialize)]
pub struct Reviewer {
    pub name: String,
    pub summary: String,
    pub checks: Vec<String>,
    pub uses_tools: Vec<String>,
}

/// The gap-report template (R17, R18).
#[derive(Debug, Clone, Serialize)]
pub struct GapTemplate {
    /// `docs/compiler-requests/<YYYY-MM-DD>-<topic>.md`.
    pub path_rule: String,
    /// The R18 fields, in the order a report lists them.
    pub fields: Vec<GapField>,
}

/// One section of a gap report.
#[derive(Debug, Clone, Serialize)]
pub struct GapField {
    pub heading: String,
    pub guidance: String,
}

/// One file of the reference pack (R7).
#[derive(Debug, Clone, Serialize)]
pub struct RefDoc {
    /// File name inside the pack folder, e.g. `controls.md`.
    pub name: String,
    pub title: String,
    pub body: String,
}

/// Where `rcrun` is, said without the developer's home folder (R4, plan F3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum RcrunLocation {
    /// Under the home folder: the path below it, `/`-separated.
    UnderHome(String),
    /// Outside the home folder: the absolute path as is.
    Absolute(String),
    /// Not found beside the IDE: the command is looked up on `PATH`.
    OnPath,
}

/// Where `rcrun` is, relative to `home` when it is under it — so the kit never
/// carries the home folder (R4, plan F3).
pub fn locate_rcrun(rcrun: Option<&std::path::Path>, home: Option<&std::path::Path>) -> RcrunLocation {
    let Some(rcrun) = rcrun else {
        return RcrunLocation::OnPath;
    };
    if let Some(rest) = home.and_then(|h| rcrun.strip_prefix(h).ok()) {
        let parts: Vec<String> = rest.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        if !parts.is_empty() {
            return RcrunLocation::UnderHome(parts.join("/"));
        }
    }
    RcrunLocation::Absolute(rcrun.to_string_lossy().into_owned())
}

/// The two connections an agent makes (R11a).
#[derive(Debug, Clone, Serialize)]
pub struct Servers {
    pub ide_name: String,
    /// `http://127.0.0.1:<port>/mcp/<kit_id>`.
    pub ide_url: String,
    pub stdio_name: String,
    pub rcrun: RcrunLocation,
    /// The arguments before the project folder: `mcp --project`.
    pub stdio_args: Vec<String>,
}

/// What the permissions should allow and deny, independent of any agent's
/// rule syntax (R15).
#[derive(Debug, Clone, Serialize)]
pub struct PermissionIntent {
    pub edit_inside_project: bool,
    pub read_inside_project: bool,
    /// Allow every project tool on both servers.
    pub allow_project_tools: bool,
    /// Deny the shell outright (plan D10).
    pub deny_shell: bool,
}

/// What [`build`] needs that is not fixed text.
pub struct BuildInput {
    pub ide_version: String,
    /// Already scrubbed of personal details.
    pub project_name: String,
    pub ide_url: String,
    pub rcrun: RcrunLocation,
    pub tools: Vec<ToolInfo>,
    pub reference: Vec<RefDoc>,
}

/// The kit's content for one project.
pub fn build(input: BuildInput) -> KitContent {
    let v = &input.ide_version;
    let tool_names: Vec<String> = input.tools.iter().map(|t| t.name.clone()).collect();
    let has = |n: &str| tool_names.iter().any(|t| t == n);
    debug_assert!(
        ["list_files", "check", "regenerate", "add_to_project", "build", "validate", "kb_lookup"]
            .iter()
            .all(|n| has(n)),
        "the skills name every project tool"
    );
    KitContent {
        product: "PowerRustCOBOL AI".into(),
        ide_version: v.clone(),
        project_name: input.project_name.clone(),
        brief_intro: brief_intro(&input.project_name),
        layout: layout(),
        brief_rules: rules(),
        tools: input.tools,
        skills: skills(v),
        reviewer: reviewer(),
        gap_template: gap_template(),
        reference: input.reference,
        servers: Servers {
            ide_name: IDE_SERVER.into(),
            ide_url: input.ide_url,
            stdio_name: STDIO_SERVER.into(),
            rcrun: input.rcrun,
            stdio_args: vec!["mcp".into(), "--project".into()],
        },
        permissions: PermissionIntent {
            edit_inside_project: true,
            read_inside_project: true,
            allow_project_tools: true,
            deny_shell: true,
        },
    }
}

/// URI prefix of the reference documents the tool servers serve as MCP
/// resources (spec 084 R13).
pub const RESOURCE_PREFIX: &str = "powerrustcobol://reference/";

/// The instructions both tool servers send at `initialize` (spec 084 R11):
/// how to work, the standing rules, and where the reference lives. Built from
/// the same rules the exported brief carries, so the two cannot disagree.
pub fn server_instructions(version: &str) -> String {
    let mut out = format!(
        "PowerRustCOBOL AI {version} — tools for PowerRustCOBOL projects: desktop applications \
         written in RustCOBOL (COBOL-85 plus the PowerRustCOBOL extensions) with forms (`.cfrm`), \
         indexed-file definitions (`.cidx`), Common Code and assets. You do not have the compiler's \
         source and must not need it: what the product supports is in the reference resources \
         (`{RESOURCE_PREFIX}…`, listed by resources/list) and `kb_lookup` finds one name in them.\n\n\
         Work in this order: look the names up, make the change, `regenerate` what you changed, \
         `check` until it reports no error, then report. A change is not done while `check` \
         reports an error for it.\n\nStanding rules:\n"
    );
    for rule in rules() {
        out.push_str(&format!("- {}\n", rule.text));
    }
    out
}

fn s(text: &str) -> String {
    text.to_owned()
}

fn brief_intro(project: &str) -> Vec<String> {
    vec![
        format!(
            "This folder is the PowerRustCOBOL project **{project}**: a desktop application written \
             in RustCOBOL (COBOL-85 plus the PowerRustCOBOL extensions), with forms designed in \
             PowerRustCOBOL AI. You create and change its COBOL, its forms (`.cfrm`), its \
             indexed-file definitions (`.cidx`) and its assets. You do not have the compiler's \
             source and you must not need it: everything the product supports is in the reference \
             pack under `{REFERENCE_DIR}/`, and the project tools check your work against the real \
             compiler."
        ),
        s("Work in this order: look the names up (reference pack or `kb_lookup`), make the \
           change, `regenerate` what you changed, `check` until it reports no error, then report. \
           A change is not done while `check` reports an error for it."),
    ]
}

fn layout() -> Vec<(String, String)> {
    [
        ("*.project.toml", "The project manifest (`cobolt.toml` in older projects). Never edit it by hand — see the rules."),
        ("forms/", "Forms: one `.cfrm` (XML) per window — its controls, properties, events and handler code."),
        ("indexed/", "Indexed-file definitions: one `.cidx` (XML) per file."),
        ("src/", "Common Code: hand-written COBOL programs and copybooks, `CALL`ed from handlers or run directly."),
        ("generated/", "Generated COBOL, written from the forms and `.cidx` files. Read it; never edit it."),
        ("COPYBOOKS/", "Per indexed file, its generated `SELECT` (`<name>.SEL`) and `FD` (`<name>.FD`)."),
        ("Assets/", "Images, audio, fonts and data files bundled with the application."),
        ("data/", "The application's data files at run time (a relative `ASSIGN` starts at the project folder)."),
        ("docs/powerrustcobol/", "The reference pack this kit wrote: what exists in this version of the product."),
        ("docs/compiler-requests/", "Your gap reports: what the product would need for a request it cannot do today."),
    ]
    .into_iter()
    .map(|(p, d)| (p.to_owned(), d.to_owned()))
    .collect()
}

/// The ids of the seven R8 rules, in R8's order.
pub const R8_RULE_IDS: [&str; 7] = [
    "generated-is-never-edited",
    "builtins-inline",
    "english-identifiers",
    "bound-events-only",
    "one-main-form",
    "parent-not-geometry",
    "reference-only-properties",
];

/// The standing rules, in brief order (spec 080 R8, R16–R19, the tool rules).
pub fn rules() -> Vec<Rule> {
    let r = |id: &str, text: &str| Rule { id: id.to_owned(), text: text.to_owned() };
    vec![
        // ── R8 — the standing COBOL rules ─────────────────────────────────────
        r(
            "generated-is-never-edited",
            "Never edit generated COBOL. Everything under `generated/` and the indexed copybooks in \
             `COPYBOOKS/` are rewritten from the `.cfrm` and `.cidx` files. Change the form or the \
             definition, then call `regenerate`.",
        ),
        r(
            "builtins-inline",
            "Write every built-in inline, as a method of the `COBOL` object: \
             `COBOL::\"HTTP-GET\" ( WS-URL WS-RESPONSE WS-HTTP-STATUS )`. Never write \
             `CALL \"COBOL-HTTP-GET\" USING …`. The built-ins are listed in \
             `powerrustcobol://reference/builtins.md`. Your own common procedures are still reached with \
             `CALL \"PROCEDURE-NAME\"`.",
        ),
        r(
            "english-identifiers",
            "COBOL identifiers and source stay in English — data items, paragraphs, programs, \
             control ids, comments — whatever language the developer writes to you in. Only the \
             texts the application's user reads may be in another language.",
        ),
        r(
            "bound-events-only",
            "An event handler runs only when it is bound: an `<Event name=\"…\" paragraph=\"…\">` \
             element inside its control (or in `<form-events>`) in the `.cfrm`. Code for an event \
             nothing binds never runs, and a control with no bound handler raises no event.",
        ),
        r(
            "one-main-form",
            "A project has exactly one main form: one `.cfrm` whose `<Form>` element carries \
             `main-form=\"true\"`. Never add it to a second form and never move it without the \
             developer asking; `check` reports two main forms as an error.",
        ),
        r(
            "parent-not-geometry",
            "A container is defined by its `parent` field, not by geometry: a control is inside a \
             GroupBox, Panel, TabControl or Splitter only when its `parent` attribute names that \
             container (and `tab` the TabControl page). Overlapping rectangles mean nothing.",
        ),
        r(
            "reference-only-properties",
            "A property, method or event exists only if the reference lists it for that control. \
             Before you write `<control>::<Property>` or `<control>::<Method>(…)`, find it in \
             `powerrustcobol://reference/controls.md` or with `kb_lookup`. A misspelled or invented \
             property is silently ignored at run time — it is never an error, so nothing will \
             tell you.",
        ),
        // ── Names and data the compiler holds you to ─────────────────────────
        r(
            "cobol-words",
            "Control ids, data items, paragraphs and procedures are COBOL words: letters, digits \
             and hyphens only, never starting or ending with a hyphen — `TEXTBOX-1`, never \
             `TEXTBOX_1`.",
        ),
        r(
            "global-for-handlers",
            "Each event handler is a program nested in its form's program, so it sees form-level \
             data only when it is declared `GLOBAL` (`01 WS-TOTAL GLOBAL PIC 9(8).`) and a file's \
             record only when its `FD` says `IS GLOBAL`. An undeclared data item is an error.",
        ),
        // ── R16, R19 — the product and the gaps ──────────────────────────────
        r(
            "product-is-off-limits",
            "Never modify, or ask for access to, PowerRustCOBOL itself — its compiler, runtime, \
             IDE or installation. You work only inside this project folder. If the product has to \
             change, that is a gap report, not an edit.",
        ),
        r(
            "gap-report-not-invention",
            "Never invent syntax, a property, a method, an event or a built-in to get around a \
             missing capability. If the reference does not list it, it does not exist: do the part \
             that is supported, write a gap report in `docs/compiler-requests/`, and tell the \
             developer both.",
        ),
        // ── The tools ─────────────────────────────────────────────────────────
        r(
            "manifest-through-tools",
            "Never edit the project manifest by hand. It carries a seal over the main-form \
             designation, and a hand edit makes the application report itself corrupted. Put a \
             new form, indexed definition, source or asset in the project with `add_to_project`; \
             `regenerate` records generated files itself.",
        ),
        r(
            "which-server",
            "Use the `powerrustcobol-ide` tools whenever they answer (PowerRustCOBOL AI is running \
             with this project open). Use `powerrustcobol` only when the IDE is closed, and never \
             both on one project at the same time.",
        ),
        r(
            "unsaved-in-the-ide",
            "When a tool answers that a file has unsaved edits in the IDE, stop and ask the \
             developer to save or close that form. Do not work around it.",
        ),
    ]
}

fn skills(version: &str) -> Vec<Skill> {
    let tools = |t: &[&str]| t.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    vec![
        Skill {
            name: "powerrustcobol-create-form".into(),
            summary: "Create a new form (window) in this PowerRustCOBOL project: write its .cfrm, \
                      add it to the project, generate and check its COBOL."
                .into(),
            steps: vec![
                s("Choose a form name that is a COBOL word (`ORDER-FORM`) and a file name for it, \
                   `forms/<name>.cfrm`. Read `powerrustcobol://reference/cfrm-format.md` first."),
                s("Write the `.cfrm`: a `<Form name=\"…\" title=\"…\" width=\"…\" height=\"…\">` \
                   element, its `<working-storage>` (form-level items, `GLOBAL` when a handler \
                   uses them), its `<form-events>` (`onLoad`, `onClose`) and its `<Control>` \
                   elements. Leave `main-form` out unless the developer asked for a new main \
                   form — there is exactly one."),
                s("Call `validate` with the path: the file must load."),
                s("Call `add_to_project` with the path. Never add it to the manifest yourself."),
                s("Call `regenerate` with the path, then `check` with the path, and fix every error \
                   in the `.cfrm` (never in `generated/`) until `check` reports none."),
            ],
            uses_tools: tools(&["validate", "add_to_project", "regenerate", "check"]),
            example: None,
        },
        Skill {
            name: "powerrustcobol-add-control".into(),
            summary: "Add a control to a form and bind one of its events to a COBOL handler; use \
                      whenever a form gains a control or a control gains behaviour."
                .into(),
            steps: vec![
                s("Look the control up: its `## Control:` section in \
                   `powerrustcobol://reference/controls.md`, or `kb_lookup` with its type. Use only the \
                   properties, methods and events listed there."),
                s("Add a `<Control id=\"…\" type=\"…\" x=\"…\" y=\"…\" w=\"…\" h=\"…\">` element to \
                   the form. The id is a COBOL word, unique in the form. Inside a container, set \
                   `parent` to the container's id — position alone does not place it there."),
                s("Write only the properties you need as `<Property name=\"…\">value</Property>`; a \
                   property left out takes its default."),
                s("Bind the event: an `<Event name=\"onClick\" paragraph=\"<ID>--ONCLICK\">` \
                   element inside the control, holding the handler's source in CDATA — \
                   `ENVIRONMENT DIVISION.`, `DATA DIVISION.`, its own `WORKING-STORAGE SECTION.` if \
                   it needs one, and `PROCEDURE DIVISION.` with the statements. A handler takes no \
                   parameters except where the reference says so."),
                s("In the handler, read and write controls as `<id>::<Property>` \
                   (`MOVE TXT-NAME::Text TO WS-NAME`, `SET LBL-TOTAL::Caption TO WS-TOTAL`) and call \
                   their methods as statements. Form-level data is visible only when `GLOBAL`."),
                s("Call `regenerate` with the form, then `check` with it; fix the `.cfrm` until no \
                   error remains. A diagnostic names the control ▸ event and the line inside that \
                   handler."),
            ],
            uses_tools: tools(&["kb_lookup", "regenerate", "check"]),
            example: Some(
                "```xml\n\
                 <Control id=\"BTN-SAVE\" type=\"Button\" x=\"16\" y=\"200\" w=\"112\" h=\"32\">\n\
                 \x20 <Property name=\"Caption\">Save</Property>\n\
                 \x20 <Event name=\"onClick\" paragraph=\"BTN-SAVE--ONCLICK\"><![CDATA[       ENVIRONMENT DIVISION.\n\
                 \x20      DATA DIVISION.\n\
                 \x20      PROCEDURE DIVISION.\n\
                 \x20          MOVE TXT-NAME::Text TO WS-NAME\n\
                 \x20          SET LBL-STATUS::Caption TO \"Saved\"\n\
                 ]]></Event>\n\
                 </Control>\n\
                 ```"
                    .into(),
            ),
        },
        Skill {
            name: "powerrustcobol-define-indexed-file".into(),
            summary: "Define an indexed (ISAM) file with a .cidx, generate its facade, and use the \
                      file from a form's handlers."
                .into(),
            steps: vec![
                s("Read `powerrustcobol://reference/cidx-format.md`. Write `indexed/<name>.cidx`: the \
                   file's COBOL name, its `assign-path` (relative paths start at the project \
                   folder, e.g. `data/customers.idx`), access mode, keys and the record's fields \
                   with their PICTUREs, offsets and lengths. Every key part names a field of the \
                   record."),
                s("Call `validate` with the path and fix what it reports."),
                s("Call `add_to_project` with the path, then `regenerate` with it: that writes \
                   `generated/<name>-indexed.cbl` and `COPYBOOKS/<name>.SEL` / `.FD`. Never edit \
                   those."),
                s("To use the file in a form, declare it in the form's COBOL structure: the \
                   `SELECT` in the `<file-control>` block (it may be `COPY \"COPYBOOKS/<name>.SEL\".`) \
                   and the `FD` in `<file-section>` as `FD <file> IS GLOBAL.` with the record exactly \
                   as the `.cidx` describes it — handlers see the record only through `GLOBAL`, and \
                   the generated `.FD` copybook is not `GLOBAL`. Give the `SELECT` a \
                   `FILE STATUS IS` item declared `GLOBAL`."),
                s("In handlers use the standard verbs — `OPEN`, `READ`, `WRITE`, `REWRITE`, \
                   `DELETE`, `START`, `CLOSE` — and test the file status after each. A file on \
                   disk made for another layout answers `OPEN` with status 39."),
                s("Make sure the data file's folder exists (`OPEN OUTPUT` creates the file, not the \
                   folder), then `regenerate` and `check` the form until it reports no error."),
            ],
            uses_tools: tools(&["validate", "add_to_project", "regenerate", "check"]),
            example: Some(
                "```cobol\n\
                 \x20      *> <file-control> block of the form\n\
                 \x20          SELECT CUSTOMER-FILE ASSIGN TO \"data/customers.idx\"\n\
                 \x20              ORGANIZATION IS INDEXED\n\
                 \x20              ACCESS MODE IS DYNAMIC\n\
                 \x20              RECORD KEY IS CUST-ID\n\
                 \x20              FILE STATUS IS WS-CUST-FS.\n\
                 \x20      *> <file-section> block of the form\n\
                 \x20      FD  CUSTOMER-FILE IS GLOBAL.\n\
                 \x20      01  CUSTOMER-RECORD.\n\
                 \x20          05 CUST-ID     PIC 9(8).\n\
                 \x20          05 CUST-NAME   PIC X(40).\n\
                 ```"
                    .into(),
            ),
        },
        Skill {
            name: "powerrustcobol-add-assets".into(),
            summary: "Add an image, icon, font, sound or data file to the project so forms and the \
                      built application can use it."
                .into(),
            steps: vec![
                s("Put the file under `Assets/` (a sub-folder is fine). Refer to it from a form or \
                   from COBOL by its project-relative path: assets are resolved against the \
                   application's folder, so the same path works under Run Form and in a built \
                   application."),
                s("Call `add_to_project` with its path. The list is chosen from the extension \
                   (images, fonts, sounds and data go to `assets`; `.md`, `.txt`, `.pdf` and `.html` \
                   to `documentation`); pass `list` only to override it."),
                s("Where a property takes the asset (an image or icon path), look the property up \
                   first with `kb_lookup`; then `regenerate` and `check` the form."),
            ],
            uses_tools: tools(&["add_to_project", "kb_lookup", "regenerate", "check"]),
            example: None,
        },
        Skill {
            name: "powerrustcobol-write-common-procedure".into(),
            summary: "Write reusable COBOL in Common Code (src/) and call it from form handlers, \
                      instead of repeating logic in several handlers."
                .into(),
            steps: vec![
                s("Write `src/<name>.cbl` as an ordinary COBOL-85 program: `IDENTIFICATION \
                   DIVISION.`, `PROGRAM-ID. <NAME>.`, its data, a `LINKAGE SECTION` for what the \
                   caller passes, and `PROCEDURE DIVISION USING …` ending in `GOBACK`."),
                s("Check every verb and clause against `powerrustcobol://reference/cobol85-supported-syntax.md`; \
                   write built-ins inline (`COBOL::\"NAME\" ( … )`)."),
                s("Call `add_to_project` with the path, then `check` with it until it reports no \
                   error."),
                s("From a handler, `CALL \"<NAME>\" USING …` with the arguments in the order of the \
                   procedure's `USING`; then `check` the form."),
            ],
            uses_tools: tools(&["add_to_project", "check"]),
            example: Some(
                "```cobol\n\
                 \x20      IDENTIFICATION DIVISION.\n\
                 \x20      PROGRAM-ID. CALC-TAX.\n\
                 \x20      DATA DIVISION.\n\
                 \x20      LINKAGE SECTION.\n\
                 \x20      01 LK-AMOUNT   PIC 9(7)V99.\n\
                 \x20      01 LK-TAX      PIC 9(7)V99.\n\
                 \x20      PROCEDURE DIVISION USING LK-AMOUNT LK-TAX.\n\
                 \x20          COMPUTE LK-TAX ROUNDED = LK-AMOUNT * 0.2\n\
                 \x20          GOBACK.\n\
                 ```"
                    .into(),
            ),
        },
        Skill {
            name: "powerrustcobol-check-and-fix".into(),
            summary: "Check the project against the real compiler and fix what it reports; run it \
                      before calling any change done, and before building."
                .into(),
            steps: vec![
                s("Prefer the `powerrustcobol-ide` server; use `powerrustcobol` only when the IDE is \
                   closed. Every answer is JSON: read it, do not guess from it."),
                s("Call `regenerate` for every `.cfrm` or `.cidx` you changed (or with no path for \
                   all of them)."),
                s("Call `check` (no path = the whole project). For each error, open the file it \
                   names — for a form, the `.cfrm`, at the control ▸ event and the line inside that \
                   handler — and fix it there. A diagnostic marked as generated code points at how \
                   a property or handler is set, never at a line to edit in `generated/`."),
                s("Repeat until `check` reports no error. Warnings: fix the ones your change caused."),
                s("Only when asked for a binary, call `build`. It refuses while `check` has errors. \
                   If it answers `running`, call `build` again: it keeps waiting on the same build."),
                s("If an error comes from something the product does not support, stop changing \
                   code around it and use the gap-report skill."),
            ],
            uses_tools: tools(&["regenerate", "check", "build", "list_files"]),
            example: None,
        },
        // ── Spec 084 R32 — building an application ───────────────────────────
        Skill {
            name: "powerrustcobol-build-an-application".into(),
            summary: "Build a whole multi-form PowerRustCOBOL application, or a large part of one: \
                      how it is structured and the order to build it in. Use it before creating \
                      more than one form."
                .into(),
            steps: vec![
                s("Agree the shape with the developer first: the screens, the data each one keeps \
                   (indexed files, or a web service), and how the operator moves between them. \
                   Ask, do not guess, when the request leaves it open."),
                s("Read the patterns index, `powerrustcobol://patterns/README.md`, and the pattern \
                   closest to each kind of screen. Copy their structure, not their names."),
                s("Choose the frame. Several screens of one application → an application shell: \
                   the main form carries a SideMenu and the screens load into its ContentPane \
                   (skill powerrustcobol-shell-and-navigation). A few independent windows → each \
                   form opens as its own window. There is exactly one main form; with \
                   `create_project` the project starts with `src/main.cbl` and no form yet."),
                s("Build in this order: (1) the data — every `.cidx`, validated, added, regenerated; \
                   (2) the main form, empty but for its frame; (3) one screen at a time, each \
                   checked before the next; (4) the menu that opens them; (5) Common Code in \
                   `src/` for logic two screens share."),
                s("Each form runs as its own program with its own WORKING-STORAGE. Forms never \
                   read each other's data: they talk through published properties, `super::` and \
                   the files they share. Design the data flow with that in mind."),
                s("After each screen: `regenerate`, `check` until no error, `render_form` to look at \
                   it (with `in_shell` for a ContentPane screen), then `run_form` with a short \
                   script that types, clicks and reads back what the screen must do. Fix what the \
                   picture or the run shows before going on."),
                s("Finish with `check` on the whole project and a `run_form` of the main path. \
                   Report what was built, what you verified and how, and anything left undone."),
            ],
            uses_tools: tools(&["create_project", "add_to_project", "regenerate", "check", "render_form", "run_form"]),
            example: None,
        },
        Skill {
            name: "powerrustcobol-shell-and-navigation".into(),
            summary: "Make an application shell — a main form with a SideMenu, screens loaded into \
                      its ContentPane, a breadcrumb — and move between forms: menus, child windows, \
                      `super::`, Home."
                .into(),
            steps: vec![
                s("Read `powerrustcobol://patterns/application-shell` and \
                   `powerrustcobol://patterns/contentpane-form`, and chapter 22 of \
                   `powerrustcobol://reference/developers-guide.md` (the application shell and \
                   the `super` receiver)."),
                s("Turn the shell on with a SideMenu control on the MAIN form; nothing else does it. \
                   Its menu is the sidecar `forms/<SideMenu id>.menu.yaml`, beside the form — \
                   items with `label`, `icon`, `action` and nested `items`. Actions: \
                   `open-form:<form>` loads a form into the ContentPane, `home` shows the main \
                   form's own content again, and the standalone actions open a window."),
                s("A form that loads into the ContentPane has `form-format=\"Embedded\"` (or \
                   `Both`). Size it to the pane: main form width minus the SideMenu's width, main \
                   form height minus the SideMenu's BreadcrumbHeight — a larger form scrolls."),
                s("Navigation keeps forms resident: the breadcrumb is the chain. A menu switch \
                   destroys the form it replaces unless the item sets \
                   `preserve_previous_form: true`. Close files and release resources in \
                   `onDestroy`, never in `onDeactivate` (the form only left the pane)."),
                s("Between forms: `super::<Property>` reads and writes the form that loaded or \
                   opened this one; `INVOKE super::\"<procedure>\"()` runs one of its procedures; \
                   `INVOKE ME::\"OpenFormSync\"(\"<FORM>\")` opens a modal child window (target \
                   `Standalone` or `Both`), `OpenFormAsync` a modeless one. See \
                   `powerrustcobol://patterns/modal-dialog` for a dialog that answers its caller."),
                s("Check the frame: `render_form` on the main form, then `render_form` with \
                   `in_shell` on each screen; `run_form` on the main form with an \
                   `{\"open_form\": \"<FORM>\"}` step, then the screen's own steps, proves the \
                   menu path works."),
            ],
            uses_tools: tools(&["kb_lookup", "regenerate", "check", "render_form", "run_form"]),
            example: Some(
                "```yaml\n\
                 menu:\n\
                 - id: home\n\
                 \x20 label: Home\n\
                 \x20 type: action\n\
                 \x20 icon: home\n\
                 \x20 action: home\n\
                 \x20 enabled: true\n\
                 - id: customers\n\
                 \x20 label: Customers\n\
                 \x20 type: action\n\
                 \x20 icon: users\n\
                 \x20 action: open-form:customers-form\n\
                 \x20 enabled: true\n\
                 \x20 preserve_previous_form: true\n\
                 ```"
                    .into(),
            ),
        },
        Skill {
            name: "powerrustcobol-layout-and-themes".into(),
            summary: "Lay a form out so it holds together at any window size, and give it a \
                      consistent look with a theme; then look at it with render_form."
                .into(),
            steps: vec![
                s("Read `powerrustcobol://reference/form-layout-and-events.md` and \
                   `powerrustcobol://reference/form-themes.md`. Use only the properties they list."),
                s("Make the form responsive (`responsive=\"true\"` on the `<Form>`). Then give each \
                   control the behaviour it needs: `Anchor` (`Top,Left` stays put; \
                   `Top,Left,Right` stretches with the width; `Bottom,Right` follows that corner), \
                   or `Dock` (`Top`, `Bottom`, `Left`, `Right`, `Fill`) for bars and panes, or a \
                   container whose `LayoutMode` is `Flex`, `Grid` or `Flow` for rows, columns and \
                   cards. Inside a container, set `parent`; position alone does not place a \
                   control in it."),
                s("Leave room: controls that touch in the design stop the window there. Give \
                   stretching fields a `MinWidth`, and the form a `MinFormWidth` / \
                   `MinFormHeight`, so it stops at a size that still works."),
                s("Theme: a form takes the project's default theme unless its `<Form>` sets \
                   `theme` (`liquid-glass`, `elegance`, `spatial`, or an installed pack); \
                   `glass-style` (`Classic`, `Enhanced`, `Neumorphic`, `NeumorphicDark`) refines the \
                   surface. Keep one theme across an application's forms. The project default is \
                   set in the IDE's Settings by the developer — never in the project file."),
                s("Text must stay readable on its background: after a theme change, check every \
                   label and button colour against the new background."),
                s("Look at it with `render_form` (scale 1). It shows the designed size: for the \
                   behaviour at other sizes rely on the anchors, docks and containers you set, and \
                   ask the developer to try the window under Run Form. Fix overlaps, clipped text \
                   and controls past the edge in the `.cfrm`, then `regenerate` and `check`."),
            ],
            uses_tools: tools(&["kb_lookup", "render_form", "run_form", "regenerate", "check"]),
            example: Some(
                "```xml\n\
                 <Form name=\"CUSTOMERS-FORM\" title=\"Customers\" width=\"1288\" height=\"908\" \
                 form-format=\"Embedded\" responsive=\"true\" theme=\"elegance\">\n\
                 \x20 <Control id=\"TXT-SEARCH\" type=\"TextBox\" x=\"24\" y=\"24\" w=\"400\" h=\"32\">\n\
                 \x20   <Property name=\"Anchor\">Top,Left,Right</Property>\n\
                 \x20 </Control>\n\
                 \x20 <Control id=\"BTN-SAVE\" type=\"Button\" x=\"1160\" y=\"852\" w=\"104\" h=\"32\">\n\
                 \x20   <Property name=\"Anchor\">Bottom,Right</Property>\n\
                 \x20 </Control>\n\
                 </Form>\n\
                 ```"
                    .into(),
            ),
        },
        Skill {
            name: "powerrustcobol-gap-report".into(),
            summary: "Write a gap report when a request needs a verb, control, property, method, \
                      event, file feature or IDE capability the reference does not list."
                .into(),
            steps: vec![
                s("Confirm the gap: search the reference pack and call `kb_lookup` for the name. \
                   Only a name neither finds is a gap."),
                s("Do the part of the request that is supported, and check it."),
                format!(
                    "Write `{GAP_REPORT_DIR}/<YYYY-MM-DD>-<topic>.md` (today's date, a short \
                     hyphenated topic) with every section of the template below, in that order. \
                     The version line reads: kit and IDE {version}."
                ),
                s("Tell the developer what you did, what is missing, and where the report is."),
                s("Never invent the missing syntax, property, method, event or built-in instead."),
            ],
            uses_tools: tools(&["kb_lookup", "check"]),
            example: None,
        },
    ]
}

fn reviewer() -> Reviewer {
    Reviewer {
        name: "powerrustcobol-reviewer".into(),
        summary: "Reviews a change to this PowerRustCOBOL project before it is reported done: every \
                  name against the reference, every standing rule, and a clean check. Use it after \
                  any change to a .cfrm, .cidx or COBOL source."
            .into(),
        checks: vec![
            s("Every control type, property, method, event and built-in the change uses is in \
               the `powerrustcobol://reference/` resources or found by `kb_lookup` — for that control. Anything else is \
               invented: reject it."),
            s("Nothing under `generated/` or `COPYBOOKS/` was edited by hand, and the project \
               manifest was not edited (files were added with `add_to_project`)."),
            s("Built-ins are written inline (`COBOL::\"NAME\" ( … )`), never `CALL \"COBOL-…\"`."),
            s("Identifiers and source are English COBOL words (letters, digits, hyphens)."),
            s("Every new handler is bound by an `<Event>` element in its control; form-level data a \
               handler uses is `GLOBAL`, and so is the `FD` of a file it reads."),
            s("Exactly one form carries `main-form=\"true\"`; every control inside a container names \
               it in `parent`."),
            s("Nothing outside the project folder was changed, and nothing of PowerRustCOBOL itself."),
            s("`check` reports no error for the changed files. A missing capability has a gap report \
               in `docs/compiler-requests/` instead of a workaround that pretends."),
        ],
        uses_tools: vec!["check".into(), "kb_lookup".into(), "validate".into(), "list_files".into()],
    }
}

/// The headings of the seven R18 fields, in order.
pub const R18_FIELDS: [&str; 7] = [
    "Request",
    "Missing capability",
    "Why it is needed",
    "Minimal example",
    "Workaround used",
    "Versions",
    "Classification",
];

fn gap_template() -> GapTemplate {
    let guidance = [
        "The developer's request, in their words.",
        "The verb, control, property, method, event, file feature or IDE capability that is \
         missing, named precisely (e.g. `TextBox::SelectionStart`, `onDragOver` on `ListBox`).",
        "What cannot be done without it, and why the supported features do not cover it.",
        "A minimal COBOL or `.cfrm` example of the wanted behaviour, as it would be written if it \
         existed.",
        "What you did instead, if anything — or `None`.",
        "The kit and IDE version (the version this project's agent brief states).",
        "`Fix` — the behaviour is COBOL-85 standard and should already work; or `Feature` — a \
         capability beyond the standard or beyond what the IDE offers today. Say why.",
    ];
    GapTemplate {
        path_rule: format!("{GAP_REPORT_DIR}/<YYYY-MM-DD>-<topic>.md"),
        fields: R18_FIELDS
            .iter()
            .zip(guidance)
            .map(|(h, g)| GapField { heading: (*h).into(), guidance: g.into() })
            .collect(),
    }
}

/// The gap-report template as Markdown, the way every writer shows it.
pub fn gap_template_markdown(t: &GapTemplate) -> String {
    let mut out = String::from("```markdown\n# Gap report: <topic>\n\n");
    for f in &t.fields {
        out.push_str(&format!("## {}\n\n<{}>\n\n", f.heading, f.guidance));
    }
    out.push_str("```\n");
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn sample() -> KitContent {
        build(BuildInput {
            ide_version: "1.80.70".into(),
            project_name: "Demo".into(),
            ide_url: "http://127.0.0.1:5720/mcp/k-test".into(),
            rcrun: RcrunLocation::OnPath,
            tools: crate::ProjectTools::<crate::HeadlessHost>::tool_list()
                .into_iter()
                .map(|t| ToolInfo { name: t.name, description: t.description.unwrap_or_default() })
                .collect(),
            reference: Vec::new(),
        })
    }

    /// AC8 + R8 + R9: the template carries every R18 field, the brief every
    /// R8 rule (plus R16 and R19), and ten skills cover the R9 subjects and
    /// building an application (spec 084 R32).
    #[test]
    fn gap_template_has_every_r18_field() {
        let c = sample();
        let md = gap_template_markdown(&c.gap_template);
        for f in R18_FIELDS {
            assert!(md.contains(&format!("## {f}")), "R18 field {f} missing from the template");
        }
        assert_eq!(c.gap_template.path_rule, "docs/compiler-requests/<YYYY-MM-DD>-<topic>.md");

        for id in R8_RULE_IDS {
            assert!(c.brief_rules.iter().any(|r| r.id == id), "R8 rule {id} missing from the brief");
        }
        for id in ["product-is-off-limits", "gap-report-not-invention", "manifest-through-tools", "which-server", "unsaved-in-the-ide"] {
            assert!(c.brief_rules.iter().any(|r| r.id == id), "rule {id} missing from the brief");
        }
        let all_text = serde_json::to_string(&c).unwrap();
        assert!(!all_text.contains("CALL \\\"COBOL-HTTP-GET\\\" USING WS"), "no example writes the CALL form");

        // R9 subjects → the skill that covers each.
        let subjects = [
            ("creating a form", "powerrustcobol-create-form"),
            ("adding a control and binding its event", "powerrustcobol-add-control"),
            ("an indexed file and its facade", "powerrustcobol-define-indexed-file"),
            ("adding assets", "powerrustcobol-add-assets"),
            ("a common procedure", "powerrustcobol-write-common-procedure"),
            ("the check-and-fix loop", "powerrustcobol-check-and-fix"),
            ("a gap report", "powerrustcobol-gap-report"),
        ];
        assert_eq!(c.skills.len(), 10);
        let tool_names: Vec<&str> = c.tools.iter().map(|t| t.name.as_str()).collect();
        for (subject, name) in subjects {
            let skill = c.skills.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("no skill for {subject}"));
            assert!(!skill.steps.is_empty() && !skill.uses_tools.is_empty(), "{name} is empty");
            for t in &skill.uses_tools {
                assert!(tool_names.contains(&t.as_str()), "{name} names an unknown tool {t}");
            }
        }
        assert!(!c.reviewer.checks.is_empty());
        println!(
            "kit content: {} R18 fields in the template, {} rules in the brief ({} of them R8), \
             {} skills for the 7 R9 subjects, reviewer with {} checks",
            R18_FIELDS.len(),
            c.brief_rules.len(),
            R8_RULE_IDS.len(),
            c.skills.len(),
            c.reviewer.checks.len()
        );
    }
}
