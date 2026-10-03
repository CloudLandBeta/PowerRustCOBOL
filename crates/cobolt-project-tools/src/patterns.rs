// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The patterns pack** (spec 084 R33): working forms from the PowerDemo3
//! and PowerChat examples, each with a note on what it shows, served by both
//! tool servers as MCP resources. A coding agent reads a real, checked form
//! before writing its own, instead of guessing how the pieces fit.
//!
//! The forms are compiled into the binary from `examples/`, so the pack is
//! always the examples as they ship; a test proves each one loads and passes
//! `check` in a project of its own.

/// URI prefix of the patterns (`powerrustcobol://patterns/<name>`).
pub const PATTERN_PREFIX: &str = "powerrustcobol://patterns/";
/// The pack's index.
pub const INDEX: &str = "README.md";

/// One pattern: what it shows and the files it is made of.
pub struct Pattern {
    /// The resource name: `powerrustcobol://patterns/<name>`.
    pub name: &'static str,
    pub title: &'static str,
    /// What it demonstrates, and what to look at in it.
    pub note: &'static str,
    /// `(project-relative path, content)`, the form first.
    pub files: &'static [(&'static str, &'static str)],
}

macro_rules! example {
    ($path:literal) => {
        include_str!(concat!("../../../examples/", $path))
    };
}

/// The pack, in reading order.
pub const PATTERNS: &[Pattern] = &[
    Pattern {
        name: "application-shell",
        title: "The application shell: a main form with a SideMenu",
        note: "PowerDemo3's main form. A SideMenu on the main form is what turns the shell on: one \
               window with the menu rail, a breadcrumb, and a ContentPane where other forms load. The \
               menu lives in the sidecar `<SideMenu id>.menu.yaml` beside the form, not in the .cfrm: \
               each item's `action` is `open-form:<form>` (load into the ContentPane), `home` (back to \
               the main form's own content), or a standalone open. `preserve_previous_form: true` keeps \
               the form it replaces resident. What is drawn on the main form beside the rail is the \
               shell's own Home content. Look at the SideMenu control's properties (FullHeight, \
               Collapsed, BreadcrumbHeight, the profile card) and at the menu file's tree.",
        files: &[
            ("forms/sidebar-form.cfrm", example!("PowerDemo3/forms/sidebar-form.cfrm")),
            ("forms/SideMenu-1.menu.yaml", example!("PowerDemo3/forms/SideMenu-1.menu.yaml")),
        ],
    },
    Pattern {
        name: "contentpane-form",
        title: "A form loaded into the ContentPane",
        note: "PowerDemo3's Customers screen. `form-format=\"Embedded\"` means it may only load into a \
               shell's ContentPane (`Both` allows a window too; `Standalone` only a window). It keeps \
               its designed size: design it no larger than the pane — main form width minus the \
               SideMenu width, main form height minus BreadcrumbHeight — or the surplus scrolls. It \
               runs as its own program with its own WORKING-STORAGE; it reaches the main form only \
               through `super::` (properties, and `INVOKE super::\"<procedure>\"()`). onDeactivate \
               fires when it leaves the pane but stays resident; onDestroy when it is released.",
        files: &[("forms/inner-form1.cfrm", example!("PowerDemo3/forms/General/inner-form1.cfrm"))],
    },
    Pattern {
        name: "indexed-maintenance",
        title: "Maintaining records in indexed files",
        note: "PowerChat's provider list: add, edit, delete and list records kept in indexed files. The \
               files are declared in the form's own `<file-control>` and `<file-section>` blocks \
               (`FD … IS GLOBAL`, a `GLOBAL` file status), opened `I-O` with a fall-back to \
               `OUTPUT` to create them the first time, browsed with `START … KEY IS >=` and \
               `READ … NEXT RECORD`, and changed with `WRITE` / `REWRITE` / `DELETE`, the status \
               tested after each. Shared logic sits in the form's own procedures (the `PC-…` \
               events), `CALL`ed from the control handlers.",
        files: &[("forms/providers-form.cfrm", example!("PowerChat/forms/providers-form.cfrm"))],
    },
    Pattern {
        name: "grid-bound-to-indexed-file",
        title: "A DataGrid bound to an indexed file",
        note: "PowerDemo3's DataGrid demo: the grid's `DataSource` names an indexed-file definition and \
               its record (`indexed/actors.cidx / CUSTOMER-RECORD`), so the grid shows the file's \
               records with no read loop of your own. The `.cidx` describes the file — its path, keys \
               and fields — and `regenerate` writes its facade and copybooks. The handlers react to \
               the grid's selection and edits.",
        files: &[
            ("forms/datagrid-form.cfrm", example!("PowerDemo3/forms/Data/datagrid-form.cfrm")),
            ("indexed/actors.cidx", example!("PowerDemo3/indexed/actors.cidx")),
        ],
    },
    Pattern {
        name: "rest-call",
        title: "Calling a web service with RestClient",
        note: "PowerDemo3's REST demo, narrated step by step on the form: a RestClient control holds \
               the base URL, headers and method; the handlers set the request, call it, and read the \
               status and the body back through the control's properties and events. Look at how a \
               failure (status, timeout) is told apart from a success before the body is used.",
        files: &[("forms/restapi-form.cfrm", example!("PowerDemo3/forms/Non-Visual/restapi-form.cfrm"))],
    },
    Pattern {
        name: "agent-chat",
        title: "Asking a model through an AgentObject",
        note: "PowerDemo3's AgentObject demo: a non-visual AgentObject carries the endpoint \
               (`AgentURL`, `AgentAPI`, `AgentModel`), a `SystemPrompt`, `Temperature`, \
               `MaximumTokens`, `Stream` and `TimeoutSeconds`, or names a saved `Configuration`. A \
               handler asks with `Agent-Helper::Ask(<question>)`, and the answer is read back from \
               `Agent-Helper::LastReply`. Look at how the form checks the key before asking, shows \
               that a request is running, and reports a failure. Never write an API key into a form.",
        files: &[("forms/agent-form.cfrm", example!("PowerDemo3/forms/Non-Visual/agent-form.cfrm"))],
    },
    Pattern {
        name: "modal-dialog",
        title: "A small modal dialog that answers its caller",
        note: "PowerChat's confirmation dialog: a child form opened with `OpenFormSync`, which is \
               implicitly modal — the caller's whole face waits, drawn with the form's \
               `modal-overlay-style`. It reads what to ask from its caller with \
               `INVOKE super::\"GetProperty\"(\"ConfirmText\")`, and hands the answer back with \
               `INVOKE super::\"SetProperty\"(\"ConfirmAnswer\", \"Y\")` before it closes; the \
               caller (`INVOKE ME::\"OpenFormSync\"(\"CONFIRM-FORM\")`) reads it when the call returns.",
        files: &[("forms/confirm-form.cfrm", example!("PowerChat/forms/confirm-form.cfrm"))],
    },
];

/// The pack's index: every pattern, its URI and what it shows.
pub fn index() -> String {
    let mut out = String::from(
        "# PowerRustCOBOL patterns\n\nWorking forms from the PowerDemo3 and PowerChat examples. Read the one \
         closest to what you are building before you write your own; copy its structure, not its \
         names nor its theme — an application you build is Spatial.\n\n",
    );
    for p in PATTERNS {
        out.push_str(&format!("- `{PATTERN_PREFIX}{}` — **{}**\n", p.name, p.title));
    }
    out
}

/// One pattern as a document: the note, then each file in full.
pub fn document(p: &Pattern) -> String {
    let mut out = format!("# {}\n\n{}\n", p.title, p.note);
    for (path, text) in p.files {
        let lang = if path.ends_with(".yaml") { "yaml" } else { "xml" };
        out.push_str(&format!("\n## `{path}`\n\n```{lang}\n{}\n```\n", text.trim_end()));
    }
    out
}

/// `(name, title)` of every resource the pack serves, the index first.
pub fn resources() -> Vec<(String, String)> {
    let mut out = vec![(INDEX.to_owned(), "PowerRustCOBOL patterns — the index".to_owned())];
    out.extend(PATTERNS.iter().map(|p| (p.name.to_owned(), p.title.to_owned())));
    out
}

/// The text of the resource `name` (`README.md` or a pattern's name).
pub fn read(name: &str) -> Option<String> {
    if name == INDEX {
        return Some(index());
    }
    PATTERNS.iter().find(|p| p.name == name).map(document)
}
