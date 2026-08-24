// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Code sites (spec 053): one stable address for every place a developer can
//! write COBOL in a project.
//!
//! There are nine kinds of such places — a control's event handler, a form's
//! lifecycle handler, a user procedure, the five structure sections
//! (`SPECIAL-NAMES`, `REPOSITORY`, `FILE-CONTROL`, `FILE SECTION`,
//! `WORKING-STORAGE`) and a hand-written Common Code file. Codegen weaves the
//! first eight into one generated `.cbl` per form; a `CodeSite` is how a
//! diagnostic or a search result names the *developer's* place instead of a
//! line of that build artifact.
//!
//! [`code_sites`] is the single enumeration of a form's sites, used by both
//! the source map (cobolt-codegen) and the project-wide search (cobolt-ide),
//! so the two can never disagree about the taxonomy.

use crate::model::{Control, Form};

/// The five fixed raw-COBOL blocks a form owns besides handlers and
/// procedures, in division/section order.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum StructureSection {
    SpecialNames,
    Repository,
    FileControl,
    FileSection,
    WorkingStorage,
}

impl StructureSection {
    /// All five sections, in the order they appear in a COBOL program.
    pub const ALL: [StructureSection; 5] = [
        StructureSection::SpecialNames,
        StructureSection::Repository,
        StructureSection::FileControl,
        StructureSection::FileSection,
        StructureSection::WorkingStorage,
    ];

    /// The fixed COBOL section keyword. These are COBOL words and stay English
    /// in every UI language (CRITICAL constraint).
    pub fn keyword(self) -> &'static str {
        match self {
            StructureSection::SpecialNames => "SPECIAL-NAMES",
            StructureSection::Repository => "REPOSITORY",
            StructureSection::FileControl => "FILE-CONTROL",
            StructureSection::FileSection => "FILE SECTION",
            StructureSection::WorkingStorage => "WORKING-STORAGE",
        }
    }

    /// The section addressed by a keyword, if any.
    pub fn from_keyword(word: &str) -> Option<StructureSection> {
        StructureSection::ALL
            .into_iter()
            .find(|s| s.keyword() == word)
    }
}

/// A place a developer writes COBOL (spec 053 R1).
///
/// Everything except [`CodeSite::CommonCode`] is owned by a form; a Common
/// Code file is a project-level `.cbl` the developer edits directly, so it
/// carries its project-relative path instead.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CodeSite {
    /// A control's event handler: `BTN-OK` ▸ `onClick`.
    ControlEvent { control_id: String, event: String },
    /// A form lifecycle handler: `onLoad` / `onClose`.
    FormEvent { event: String },
    /// A developer-written named procedure.
    Procedure { name: String },
    /// One of the five fixed structure sections.
    Section(StructureSection),
    /// A hand-written Common Code file (project-relative path). Not owned by
    /// any form.
    CommonCode { rel_path: String },
}

/// The separator between display-path components.
pub const PATH_SEP: &str = " ▸ ";

impl CodeSite {
    /// Human-readable address, form-first and left-to-right (R2):
    /// `Main Menu ▸ Button-1 ▸ onClick`, `Main Menu ▸ WORKING-STORAGE`,
    /// `Main Menu ▸ VALIDATE-CUSTOMER`. A Common Code file is its own path.
    ///
    /// The components are COBOL identifiers, event names and section keywords
    /// — they stay English in every UI language (CRITICAL constraint).
    pub fn display_path(&self, form_name: &str) -> String {
        match self {
            CodeSite::ControlEvent { control_id, event } => {
                format!("{form_name}{PATH_SEP}{control_id}{PATH_SEP}{event}")
            }
            CodeSite::FormEvent { event } => format!("{form_name}{PATH_SEP}{event}"),
            CodeSite::Procedure { name } => format!("{form_name}{PATH_SEP}{name}"),
            CodeSite::Section(s) => format!("{form_name}{PATH_SEP}{}", s.keyword()),
            CodeSite::CommonCode { rel_path } => rel_path.clone(),
        }
    }

    /// Short kind label for test inventories and logs (not UI text).
    pub fn kind_name(&self) -> &'static str {
        match self {
            CodeSite::ControlEvent { .. } => "control event",
            CodeSite::FormEvent { .. } => "form event",
            CodeSite::Procedure { .. } => "user procedure",
            CodeSite::Section(_) => "structure section",
            CodeSite::CommonCode { .. } => "common code",
        }
    }
}

/// Resolve a display path back to the site it names, against the form that
/// owns it (AC1 round-trip). A path with no `▸` component is a Common Code
/// file. Returns `None` when the path does not address anything on `form`.
pub fn resolve_display_path(form: &Form, path: &str) -> Option<CodeSite> {
    let comps: Vec<&str> = path.split(PATH_SEP).collect();
    match comps.as_slice() {
        [only] => Some(CodeSite::CommonCode {
            rel_path: (*only).to_string(),
        }),
        [form_name, second] => {
            if *form_name != form.name {
                return None;
            }
            if let Some(section) = StructureSection::from_keyword(second) {
                return Some(CodeSite::Section(section));
            }
            if form.form_events.iter().any(|e| e.event == *second) {
                return Some(CodeSite::FormEvent {
                    event: (*second).to_string(),
                });
            }
            if form.user_procedures.iter().any(|p| p.name.trim() == *second) {
                return Some(CodeSite::Procedure {
                    name: (*second).to_string(),
                });
            }
            None
        }
        [form_name, control_id, event] => {
            if *form_name != form.name {
                return None;
            }
            Some(CodeSite::ControlEvent {
                control_id: (*control_id).to_string(),
                event: (*event).to_string(),
            })
        }
        _ => None,
    }
}

/// The text a site currently holds on `form`, or `None` for a site the form
/// does not own — a [`CodeSite::CommonCode`] (its text lives in the file, not
/// on a form) or an address that matches nothing.
pub fn site_text<'a>(form: &'a Form, site: &CodeSite) -> Option<&'a str> {
    match site {
        CodeSite::Section(s) => Some(match s {
            StructureSection::SpecialNames => form.cobol_structure.special_names.as_str(),
            StructureSection::Repository => form.cobol_structure.repository.as_str(),
            StructureSection::FileControl => form.cobol_structure.file_control.as_str(),
            StructureSection::FileSection => form.cobol_structure.file_section.as_str(),
            StructureSection::WorkingStorage => form.user_ws_source.as_str(),
        }),
        CodeSite::FormEvent { event } => form
            .form_events
            .iter()
            .find(|e| e.event == *event)
            .map(|e| e.code.as_str()),
        CodeSite::ControlEvent { control_id, event } => {
            find_control(&form.controls, control_id)?
                .events
                .iter()
                .find(|e| e.event == *event)
                .map(|e| e.code.as_str())
        }
        CodeSite::Procedure { name } => form
            .user_procedures
            .iter()
            .find(|p| p.name.trim() == name.as_str())
            .map(|p| p.code.as_str()),
        CodeSite::CommonCode { .. } => None,
    }
}

/// Every code site on `form` that holds developer text, with that text —
/// the single enumeration both the source map and the search read (spec 053).
///
/// Order (stable): the five structure sections in division order, then form
/// lifecycle handlers, then control event handlers in document order
/// (containers before their children), then user procedures. Sites whose text
/// is empty are not yielded — there is nothing to search or attribute in them;
/// codegen emits a *generated* stub for an unwritten handler, which is
/// deliberately not the developer's code.
///
/// `Form::deleted_code` (the recycle bin) is never yielded (R24), and a
/// generated `.cbl` is not a site at all (R4).
pub fn code_sites(form: &Form) -> Vec<(CodeSite, &str)> {
    let mut out: Vec<(CodeSite, &str)> = Vec::new();

    for section in StructureSection::ALL {
        let site = CodeSite::Section(section);
        if let Some(text) = site_text(form, &site) {
            if !text.trim().is_empty() {
                out.push((site, text));
            }
        }
    }
    for ev in &form.form_events {
        if !ev.code.trim().is_empty() {
            out.push((
                CodeSite::FormEvent {
                    event: ev.event.clone(),
                },
                ev.code.as_str(),
            ));
        }
    }
    walk_control_events(&form.controls, &mut out);
    for proc in &form.user_procedures {
        if !proc.name.trim().is_empty() && !proc.code.trim().is_empty() {
            out.push((
                CodeSite::Procedure {
                    name: proc.name.trim().to_string(),
                },
                proc.code.as_str(),
            ));
        }
    }
    out
}

fn walk_control_events<'a>(controls: &'a [Control], out: &mut Vec<(CodeSite, &'a str)>) {
    for ctrl in controls {
        for ev in &ctrl.events {
            if !ev.code.trim().is_empty() {
                out.push((
                    CodeSite::ControlEvent {
                        control_id: ctrl.id.clone(),
                        event: ev.event.clone(),
                    },
                    ev.code.as_str(),
                ));
            }
        }
        walk_control_events(&ctrl.children, out);
    }
}

fn find_control<'a>(controls: &'a [Control], id: &str) -> Option<&'a Control> {
    for ctrl in controls {
        if ctrl.id == id {
            return Some(ctrl);
        }
        if let Some(found) = find_control(&ctrl.children, id) {
            return Some(found);
        }
    }
    None
}

// ── Test fixture (spec 053 T1) ───────────────────────────────────────────────

/// The all-sites fixture form: developer code in **every** in-form site kind,
/// with deliberately untidy input — leading blank lines, trailing whitespace,
/// one *empty* handler (so its generated stub is exercised) and a unique
/// marker string per site.
///
/// A test fixture, exposed `pub` because the cobolt-codegen source-map and
/// golden-byte tests build the same form. Not part of the IDE's runtime paths.
///
/// Marker → site line (1-based, within the site's own text):
///
/// | Site | Marker | Site line |
/// |---|---|---|
/// | SPECIAL-NAMES | `MARK-SPECIAL-NAMES-053` | 2 |
/// | REPOSITORY | `MARK-REPOSITORY-053` | 1 |
/// | FILE-CONTROL (leading blank line) | `MARK-FILE-CONTROL-053` | 3 |
/// | FILE SECTION | `MARK-FILE-SECTION-053` | 2 |
/// | WORKING-STORAGE (two leading blank lines) | `MARK-WORKING-STORAGE-053` | 3 |
/// | form `onLoad` (interior trailing spaces) | `MARK-FORM-ONLOAD-053` | 4 |
/// | `BTN-GO` `onClick` (trailing whitespace) | `MARK-BTN-GO-ONCLICK-053` | 3 |
/// | procedure `VALIDATE-CUSTOMER` (leading blank line) | `MARK-PROCEDURE-053` | 4 |
///
/// `BTN-EMPTY`'s `onClick` and the form's `onClose` are bound but **empty** —
/// their generated stubs are codegen's, not the developer's.
pub fn all_sites_fixture() -> Form {
    use crate::model::{ControlType, EventBinding, UserProcedure};

    let mut form = Form::new("ALL-SITES", "Every code site", 800, 600);

    form.cobol_structure.special_names =
        "           DECIMAL-POINT IS COMMA\n      *> MARK-SPECIAL-NAMES-053".to_string();
    form.cobol_structure.repository =
        "           CLASS MARK-REPOSITORY-053 IS \"Mark.Repository\"".to_string();
    form.cobol_structure.file_control = "\n           SELECT MARK-FILE ASSIGN TO \"mark053.dat\"\n               ORGANIZATION IS LINE SEQUENTIAL. *> MARK-FILE-CONTROL-053"
        .to_string();
    form.cobol_structure.file_section =
        "       FD  MARK-FILE.\n       01  MARK-FILE-REC PIC X(80). *> MARK-FILE-SECTION-053"
            .to_string();
    // Two leading blank lines: the WORKING-STORAGE weaver skips them, so the
    // marker's generated line is NOT site-line-1 — exactly the off-by-N the
    // map's `site_line_at_start` exists to record.
    form.user_ws_source =
        "\n\n       01  WS-MARK-053 PIC X(24) VALUE \"MARK-WORKING-STORAGE-053\".".to_string();

    // Form lifecycle: onLoad written (with interior trailing spaces), onClose
    // left empty so its stub is generated.
    if let Some(on_load) = form.form_events.iter_mut().find(|e| e.event == "onLoad") {
        on_load.code = "       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n           DISPLAY \"MARK-FORM-ONLOAD-053\".   \n           CONTINUE.".to_string();
    }

    // A control with a written handler (trailing whitespace at the end)...
    let mut btn_go = Control::new("BTN-GO", ControlType::Button, 10, 10);
    let mut on_click = EventBinding::for_control("BTN-GO", "onClick");
    on_click.code = "       ENVIRONMENT DIVISION.\n       PROCEDURE DIVISION.\n           DISPLAY \"MARK-BTN-GO-ONCLICK-053\".\n           CONTINUE.   \n\n".to_string();
    btn_go.events.push(on_click);
    form.controls.push(btn_go);

    // ... and one whose handler is bound but EMPTY: its body in the generated
    // .cbl is the template stub, which is generated code, not the developer's.
    let mut btn_empty = Control::new("BTN-EMPTY", ControlType::Button, 10, 60);
    btn_empty
        .events
        .push(EventBinding::for_control("BTN-EMPTY", "onClick"));
    form.controls.push(btn_empty);

    // A user procedure, with a leading blank line (handlers keep theirs).
    form.user_procedures = vec![UserProcedure {
        name: "VALIDATE-CUSTOMER".to_string(),
        code: "\n       ENVIRONMENT DIVISION.\n       PROCEDURE DIVISION.\n           DISPLAY \"MARK-PROCEDURE-053\".".to_string(),
    }];

    form
}

/// The markers of [`all_sites_fixture`]'s eight in-form site kinds, with the
/// site that owns each and the 1-based line *within that site's text* where it
/// sits. One table, shared by this crate's tests and cobolt-codegen's
/// source-map tests, so the two cannot drift.
pub fn fixture_markers() -> Vec<(CodeSite, &'static str, u32)> {
    vec![
        (
            CodeSite::Section(StructureSection::SpecialNames),
            "MARK-SPECIAL-NAMES-053",
            2,
        ),
        (
            CodeSite::Section(StructureSection::Repository),
            "MARK-REPOSITORY-053",
            1,
        ),
        (
            CodeSite::Section(StructureSection::FileControl),
            "MARK-FILE-CONTROL-053",
            3,
        ),
        (
            CodeSite::Section(StructureSection::FileSection),
            "MARK-FILE-SECTION-053",
            2,
        ),
        (
            CodeSite::Section(StructureSection::WorkingStorage),
            "MARK-WORKING-STORAGE-053",
            3,
        ),
        (
            CodeSite::FormEvent {
                event: "onLoad".into(),
            },
            "MARK-FORM-ONLOAD-053",
            4,
        ),
        (
            CodeSite::ControlEvent {
                control_id: "BTN-GO".into(),
                event: "onClick".into(),
            },
            "MARK-BTN-GO-ONCLICK-053",
            3,
        ),
        (
            CodeSite::Procedure {
                name: "VALIDATE-CUSTOMER".into(),
            },
            "MARK-PROCEDURE-053",
            4,
        ),
    ]
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// T1: the fixture holds developer code in every in-form site kind, each
    /// with its unique marker at the documented site line. Prints the
    /// inventory so the fixture is reviewable by eye.
    #[test]
    fn fixture_covers_every_in_form_site_kind() {
        let form = all_sites_fixture();
        let markers = fixture_markers();

        println!("── all-sites fixture inventory ──────────────────────────");
        for (site, marker, line) in &markers {
            let text = site_text(&form, site).expect("fixture site has text");
            assert!(
                !text.trim().is_empty(),
                "{} should hold code",
                site.display_path(&form.name)
            );
            let found = text
                .lines()
                .position(|l| l.contains(marker))
                .map(|i| i as u32 + 1);
            assert_eq!(
                found,
                Some(*line),
                "marker {marker} must sit at site line {line}"
            );
            println!(
                "  {:<18} {:<45} marker {marker} @ site line {line} ({} lines)",
                site.kind_name(),
                site.display_path(&form.name),
                text.lines().count()
            );
        }

        // Markers are unique across sites.
        let mut seen: Vec<&str> = markers.iter().map(|(_, m, _)| *m).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), markers.len(), "markers must be unique");

        // The two deliberate empties: BTN-EMPTY onClick and form onClose.
        let empty_click = site_text(
            &form,
            &CodeSite::ControlEvent {
                control_id: "BTN-EMPTY".into(),
                event: "onClick".into(),
            },
        )
        .expect("BTN-EMPTY onClick is bound");
        assert!(empty_click.trim().is_empty(), "BTN-EMPTY onClick stays empty");
        let on_close = site_text(
            &form,
            &CodeSite::FormEvent {
                event: "onClose".into(),
            },
        )
        .expect("onClose is bound");
        assert!(on_close.trim().is_empty(), "onClose stays empty");
        println!("  (empty, stub-generating)  BTN-EMPTY ▸ onClick · ALL-SITES ▸ onClose");
    }

    /// AC1: site → display path → resolve → the same site, for all nine kinds.
    /// Prints the nine display paths so the format is reviewable by eye.
    #[test]
    fn display_path_round_trips_for_all_nine_kinds() {
        let form = all_sites_fixture();
        let mut sites: Vec<CodeSite> =
            fixture_markers().into_iter().map(|(s, _, _)| s).collect();
        // The ninth kind: a Common Code file (project-relative path).
        sites.push(CodeSite::CommonCode {
            rel_path: "common/billing-helpers.cbl".into(),
        });
        assert_eq!(sites.len(), 9, "all nine site kinds under test");

        println!("── display paths (9 kinds) ──────────────────────────────");
        for site in &sites {
            let path = site.display_path(&form.name);
            println!("  {:<18} {path}", site.kind_name());
            let resolved = resolve_display_path(&form, &path)
                .unwrap_or_else(|| panic!("path {path:?} must resolve"));
            assert_eq!(&resolved, site, "round trip must return the same site");
        }
    }

    /// The enumeration yields one entry per non-empty site — and nothing for
    /// the empty handler, the empty onClose, or the recycle bin.
    #[test]
    fn code_sites_enumerates_exactly_the_non_empty_sites() {
        let mut form = all_sites_fixture();
        form.deleted_code.push(crate::model::DeletedControlCode {
            control_id: "BTN-GONE".into(),
            deleted_at: String::new(),
            events: vec![crate::model::EventBinding {
                event: "onClick".into(),
                paragraph: "BTN-GONE--ONCLICK".into(),
                code: "           DISPLAY \"ONLY-IN-RECYCLE-BIN\".".into(),
            }],
        });

        let sites = code_sites(&form);
        // 5 sections + onLoad + BTN-GO onClick + VALIDATE-CUSTOMER = 8.
        assert_eq!(sites.len(), 8, "eight non-empty in-form sites");
        for (site, text) in &sites {
            assert!(
                !text.trim().is_empty(),
                "{} yielded empty text",
                site.display_path(&form.name)
            );
        }
        assert!(
            !sites.iter().any(|(s, _)| matches!(
                s,
                CodeSite::ControlEvent { control_id, .. } if control_id == "BTN-EMPTY"
            )),
            "an empty handler is not a searchable site"
        );
        assert!(
            !sites
                .iter()
                .any(|(_, t)| t.contains("ONLY-IN-RECYCLE-BIN")),
            "recycled deleted code is never enumerated (R24)"
        );
    }

    /// serde round-trip (spec Q6): the map must be shippable into a compiled
    /// binary later, so the address serialises today.
    #[test]
    fn code_site_serde_round_trips() {
        let sites = vec![
            CodeSite::ControlEvent {
                control_id: "BTN-GO".into(),
                event: "onClick".into(),
            },
            CodeSite::Section(StructureSection::WorkingStorage),
            CodeSite::CommonCode {
                rel_path: "common/util.cbl".into(),
            },
        ];
        for site in sites {
            let json = serde_json::to_string(&site).expect("serialize");
            let back: CodeSite = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, site);
        }
    }
}
