// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! One spelling for the value methods of a data item, in the IDE.
//!
//! `Upper()`, `Lower()` and `Len()` are the spellings that are written;
//! `UpperCase()`, `ToUpperCase()`, `LowerCase()`, `ToLowerCase()` and
//! `Length()` still run (operator, 2026-10-10). When the developer saves a piece
//! of COBOL, the editor finds the older spellings in it and writes the single
//! one in their place — and says so in the Output panel, because a change to
//! the developer's code that leaves no trace is not acceptable.
//!
//! The rewriting itself is [`cobolt_ast::value_methods`]: token-aware,
//! receiver-aware, exact. This module is what the IDE adds to it:
//!
//! * **which names are data items** — asked of the analyser, on the program the
//!   text belongs to ([`cobolt_semantic::value_item_names`]), never guessed;
//! * **only code that parses cleanly** — a text with a syntax error is left as
//!   typed, because the scanner cannot know where a half-written statement
//!   ends;
//! * **proof that nothing else changed** — the rewritten text must tokenise to
//!   the same tokens as the original, bar the spellings themselves
//!   ([`cobolt_ast::value_methods::differs_only_by_spellings`]);
//! * **the report** — [`Notice`], formatted with the IDE's own language.

use std::path::Path;

use cobolt_ast::value_methods::{canonicalize_value_methods, differs_only_by_spellings, Rewritten};
use cobolt_forms::Form;
use cobolt_lexer::SourceFormat;

use crate::i18n::Tr;

/// What a save left behind in the developer's code, as one line for the Output
/// panel. Structured, not a finished sentence, so that whoever shows it can use
/// the language the IDE is set to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// How many spellings were changed.
    pub count: usize,
    /// Where: a file name, or `form ▸ control ▸ event`.
    pub place: String,
}

impl Notice {
    pub fn new(rewritten: &Rewritten, place: impl Into<String>) -> Self {
        Self { count: rewritten.edits.len(), place: place.into() }
    }

    /// The line, in the IDE's language.
    pub fn text(&self, tr: &Tr) -> String {
        let template = if self.count == 1 { tr.vm_rewritten_one } else { tr.vm_rewritten_many };
        template.replace("{n}", &self.count.to_string()).replace("{place}", &self.place)
    }
}

/// What an editor that hosts ONE handler of a form needs to rewrite its buffer:
/// the rewrite, closed over the form it belongs to, and the place to report it
/// as. See [`crate::panels::editor::EditorPanel::canon_host`].
#[derive(Clone)]
pub struct CanonHost {
    /// `form ▸ control ▸ event`, or `form ▸ procedure`.
    pub place: String,
    /// Maps the buffer's text to its rewrite, or `None` when there is nothing
    /// to change (or the text is not fit to be rewritten).
    pub rewrite: std::sync::Arc<dyn Fn(&str) -> Option<Rewritten> + Send + Sync>,
}

/// The COBOL sources the editor rewrites on save. Anything else — Markdown, a
/// Rust file, a data file — is not COBOL, and a `::Length` in it is not ours.
pub fn is_cobol_source(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ["cbl", "cob", "cobol", "cpy", "copy"].iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// A cheap look before any parsing: could `text` hold an older spelling at all?
/// Most saves cannot, and must not pay for a parse.
fn might_hold_an_alias(text: &str) -> bool {
    if !text.contains("::") {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    lower.contains("uppercase") || lower.contains("lowercase") || lower.contains("length")
}

fn is_clean(parsed: &cobolt_parser::ParseResult) -> bool {
    parsed.program.is_some() && !parsed.diagnostics.iter().any(|d| d.severity == cobolt_parser::Severity::Error)
}

/// The shared tail: rewrite, then prove the rewrite changed nothing else.
fn finish(text: &str, format: SourceFormat, items: &std::collections::HashSet<String>) -> Option<Rewritten> {
    let rewritten = canonicalize_value_methods(text, format, &|name| items.contains(&name.to_ascii_uppercase()));
    (rewritten.changed() && differs_only_by_spellings(text, &rewritten.text, format)).then_some(rewritten)
}

/// A whole COBOL program on disk — a Common Code file. `path` says where its
/// `COPY` books are looked for.
///
/// `None` when there is nothing to change, and also when the program does not
/// parse cleanly: the older spellings stay as typed, and still run.
pub fn rewrite_program(text: &str, path: &Path) -> Option<Rewritten> {
    if !might_hold_an_alias(text) {
        return None;
    }
    let format = cobolt_project_tools::validate_source::source_format(text);
    let tokens = match cobolt_lexer::preprocess_program(text, path, format) {
        Some(expansion) if !expansion.errors.is_empty() => return None,
        Some(expansion) => cobolt_lexer::tokenize_expansion(&expansion),
        None => cobolt_lexer::tokenize(text, format),
    };
    let parsed = cobolt_parser::parse(tokens);
    if !is_clean(&parsed) {
        return None;
    }
    let items = cobolt_semantic::value_item_names(parsed.program.as_ref()?, None);
    finish(text, format, &items)
}

/// One handler or procedure of a form (`text` is its whole body, the way the
/// form stores it). `probe` is the form WITH `text` in place — the program the
/// analyser would see — so the form's `GLOBAL` data, the handler's own and the
/// form's controls (a control wins over a data item of the same name, as in the
/// analyser) are all known.
pub fn rewrite_handler_text(probe: &Form, text: &str) -> Option<Rewritten> {
    if !might_hold_an_alias(text) {
        return None;
    }
    // The text must parse on its own, the way the save gate reads it.
    let alone = format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. VALUE-METHODS-CHECK.\n{text}\n       END PROGRAM VALUE-METHODS-CHECK.\n"
    );
    let alone = cobolt_parser::parse(cobolt_lexer::tokenize(&alone, SourceFormat::Free));
    if !is_clean(&alone) {
        return None;
    }
    // The names come from the whole form's program AND from the text itself: a
    // handler that is new to the form declares items the form does not hold yet.
    let whole = cobolt_parser::parse(cobolt_lexer::tokenize(&cobolt_codegen::generate(probe), SourceFormat::Free));
    let objects = cobolt_forms::toolbar::object_names(probe);
    let mut units = vec![alone.program.as_ref()?];
    units.extend(whole.program.as_ref());
    let items = cobolt_semantic::value_item_names_in(&units, Some(&objects));
    finish(text, SourceFormat::Free, &items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tr() -> Tr {
        crate::i18n::Language::English.tr()
    }

    #[test]
    fn a_common_code_program_is_rewritten_where_the_receiver_is_a_data_item() {
        let src = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DEMO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-NAME   PIC X(20) VALUE \"  ada  \".
       01  WS-OUT    PIC X(20).
       01  WS-N      PIC 9(4).
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE WS-NAME::Trim()::UpperCase() TO WS-OUT.
           COMPUTE WS-N = WS-NAME::Length.
           MOVE \"WS-NAME::UpperCase()\" TO WS-OUT. *> WS-NAME::Length
           STOP RUN.
";
        let r = rewrite_program(src, &PathBuf::from("demo.cbl")).expect("two spellings to change");
        assert_eq!(r.edits.len(), 2);
        assert!(r.text.contains("WS-NAME::Trim()::Upper() TO WS-OUT."));
        assert!(r.text.contains("COMPUTE WS-N = WS-NAME::Len."));
        assert!(r.text.contains("MOVE \"WS-NAME::UpperCase()\" TO WS-OUT. *> WS-NAME::Length"), "{}", r.text);
        // Idempotent: the saved text has nothing left to change.
        assert!(rewrite_program(&r.text, &PathBuf::from("demo.cbl")).is_none());
    }

    #[test]
    fn a_program_that_does_not_parse_is_left_exactly_as_typed() {
        let src = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. BROKEN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-NAME   PIC X(20).
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE WS-NAME::UpperCase() TO
           IF WS-NAME::Length > 3 THEN THEN ((
";
        assert!(rewrite_program(src, &PathBuf::from("broken.cbl")).is_none());
    }

    #[test]
    fn an_object_reference_and_a_control_receiver_are_not_data_items() {
        let src = "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. HANDLES.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       REPOSITORY.
           CLASS RUST-STRING IS \"Rust.String\"
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  S USAGE IS OBJECT REFERENCE RUST-STRING VALUE \"hello\".
       01  WS-N PIC 9(4).
       PROCEDURE DIVISION.
       MAIN-PARA.
           COMPUTE WS-N = S::Length.
           STOP RUN.
";
        assert!(rewrite_program(src, &PathBuf::from("handles.cbl")).is_none());
    }

    #[test]
    fn only_cobol_sources_are_eligible() {
        for (name, yes) in [
            ("a.cbl", true), ("A.CBL", true), ("a.cob", true), ("a.cpy", true), ("a.md", false),
            ("a.rs", false), ("a.txt", false), ("a", false), ("a.cfrm", false),
        ] {
            assert_eq!(is_cobol_source(&PathBuf::from(name)), yes, "{name}");
        }
    }

    #[test]
    fn the_notice_is_one_line_in_the_ides_language() {
        let r = Rewritten { text: String::new(), edits: vec![] };
        let mut n = Notice::new(&r, "Main ▸ BTN-GO ▸ onClick");
        n.count = 3;
        let line = n.text(&tr());
        assert!(line.contains('3') && line.contains("Upper()") && line.contains("Main ▸ BTN-GO ▸ onClick"), "{line}");
        n.count = 1;
        let one = n.text(&tr());
        assert_ne!(one, line);
        assert!(!one.contains("{n}") && !one.contains("{place}"), "{one}");
    }
}
