// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! COBOL Structure model helpers (spec 005).
//!
//! The form's property inspector lists the shared COBOL sections —
//! `SPECIAL-NAMES`, `REPOSITORY`, `FILE-CONTROL`, `FILE SECTION`,
//! `WORKING-STORAGE` — plus the user procedures. Selecting one opens a popup
//! that hosts the **same** COBOL [`EditorPanel`](super::editor::EditorPanel) used
//! everywhere else (IntelliSense, syntax colouring, find/replace) — see
//! `DesignerPanel::show_cobol_structure_window`. This module just describes which
//! block is being edited and reads/writes its text on the form.

use cobolt_forms::code_site::{CodeSite, StructureSection};
use cobolt_forms::Form;

/// Which COBOL Structure block the popup editor is editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsTarget {
    SpecialNames,
    Repository,
    FileControl,
    FileSection,
    WorkingStorage,
    /// A user procedure by index into [`Form::user_procedures`].
    Procedure(usize),
}

/// The five fixed structure sections, in division/section order — **derived
/// from the model's [`StructureSection::ALL`]** (spec 053 R3), so the editor
/// targets and the model's code sites are one list and cannot drift.
pub const SECTIONS: [CsTarget; 5] = [
    CsTarget::for_section(StructureSection::ALL[0]),
    CsTarget::for_section(StructureSection::ALL[1]),
    CsTarget::for_section(StructureSection::ALL[2]),
    CsTarget::for_section(StructureSection::ALL[3]),
    CsTarget::for_section(StructureSection::ALL[4]),
];

impl CsTarget {
    /// The model section this target edits, or `None` for a user procedure.
    /// Exhaustive both ways with [`CsTarget::for_section`], so adding a
    /// section to either enum is a compile error until both carry it.
    pub const fn section(self) -> Option<StructureSection> {
        match self {
            CsTarget::SpecialNames => Some(StructureSection::SpecialNames),
            CsTarget::Repository => Some(StructureSection::Repository),
            CsTarget::FileControl => Some(StructureSection::FileControl),
            CsTarget::FileSection => Some(StructureSection::FileSection),
            CsTarget::WorkingStorage => Some(StructureSection::WorkingStorage),
            CsTarget::Procedure(_) => None,
        }
    }

    /// The editor target for a model structure section.
    pub const fn for_section(section: StructureSection) -> CsTarget {
        match section {
            StructureSection::SpecialNames => CsTarget::SpecialNames,
            StructureSection::Repository => CsTarget::Repository,
            StructureSection::FileControl => CsTarget::FileControl,
            StructureSection::FileSection => CsTarget::FileSection,
            StructureSection::WorkingStorage => CsTarget::WorkingStorage,
        }
    }

    /// The code site this target edits (spec 053 R3). A procedure resolves its
    /// name against `form`; an out-of-range index yields `None`.
    pub fn to_code_site(self, form: &Form) -> Option<CodeSite> {
        match self {
            CsTarget::Procedure(i) => form.user_procedures.get(i).map(|p| CodeSite::Procedure {
                name: p.name.trim().to_string(),
            }),
            other => other.section().map(CodeSite::Section),
        }
    }

    /// The editor target that owns `site`, or `None` when the structure window
    /// is not that site's editing surface (handlers, Common Code).
    pub fn from_code_site(site: &CodeSite, form: &Form) -> Option<CsTarget> {
        match site {
            CodeSite::Section(s) => Some(CsTarget::for_section(*s)),
            CodeSite::Procedure { name } => form
                .user_procedures
                .iter()
                .position(|p| p.name.trim() == name.as_str())
                .map(CsTarget::Procedure),
            _ => None,
        }
    }

    /// The fixed COBOL section keyword, or `None` for a user procedure (whose
    /// title is its name). Delegates to the model's [`StructureSection`] so
    /// there is exactly one spelling of each keyword.
    pub fn section_keyword(self) -> Option<&'static str> {
        self.section().map(StructureSection::keyword)
    }

    /// A stable key for the synthetic editor buffer path of this block.
    pub fn buffer_key(self) -> String {
        match self {
            CsTarget::SpecialNames => "special-names".into(),
            CsTarget::Repository => "repository".into(),
            CsTarget::FileControl => "file-control".into(),
            CsTarget::FileSection => "file-section".into(),
            CsTarget::WorkingStorage => "working-storage".into(),
            CsTarget::Procedure(i) => format!("procedure-{i}"),
        }
    }
}

/// Current text of a fixed section block (not valid for a user procedure).
pub fn section_text(form: &Form, t: CsTarget) -> Option<&str> {
    Some(match t {
        CsTarget::SpecialNames => form.cobol_structure.special_names.as_str(),
        CsTarget::Repository => form.cobol_structure.repository.as_str(),
        CsTarget::FileControl => form.cobol_structure.file_control.as_str(),
        CsTarget::FileSection => form.cobol_structure.file_section.as_str(),
        CsTarget::WorkingStorage => form.user_ws_source.as_str(),
        CsTarget::Procedure(_) => return None,
    })
}

/// The editable code of a block (section text, or a procedure's body).
pub fn block_text(form: &Form, t: CsTarget) -> String {
    match t {
        CsTarget::Procedure(i) => form
            .user_procedures
            .get(i)
            .map(|p| p.code.clone())
            .unwrap_or_default(),
        other => section_text(form, other).unwrap_or_default().to_owned(),
    }
}

/// Whether this block holds the FORM's own data descriptions, and so must have
/// `GLOBAL` on every `01`.
///
/// The form is the outermost program of the generated nest and every event
/// handler and common procedure is contained in it. A form-level item without
/// `GLOBAL` is private to the form, so no handler can name it — and there is no
/// legal workaround, because declaring it locally in a handler makes a second,
/// unrelated copy. It is the single most common way a form's data ends up
/// unreachable, so the two blocks that can declare it get the clause applied
/// rather than merely recommended.
fn declares_form_data(t: CsTarget) -> bool {
    matches!(t, CsTarget::WorkingStorage | CsTarget::FileSection)
}

/// Put `GLOBAL` on every `01`-level entry in a form-level block that lacks it.
///
/// Conservative by construction — it only ever ADDS the clause, and skips an
/// entry when adding it would be wrong or pointless:
///
/// * one that already says `GLOBAL`;
/// * one marked `EXTERNAL`, which COBOL-85 does not allow to be `GLOBAL` too;
/// * `01 FILLER`, which has no name to reach it by.
///
/// Placement follows the clause rules: normally straight after the data-name,
/// but after the `REDEFINES` target when there is one, because `REDEFINES` must
/// immediately follow the name it redefines.
pub fn ensure_global_on_01_levels(block: &str) -> String {
    let lines: Vec<&str> = block.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        if !starts_01_entry(lines[i]) {
            out.push(lines[i].to_string());
            i += 1;
            continue;
        }
        // An entry runs until the line that ends it with a period; a data
        // description may be wrapped over several lines and the clause could
        // sit on any of them.
        let mut end = i;
        while end < lines.len() && !ends_entry(lines[end]) {
            end += 1;
        }
        let end = end.min(lines.len().saturating_sub(1));
        let already = (i..=end)
            .any(|k| has_word(lines[k], "GLOBAL") || has_word(lines[k], "EXTERNAL"));
        out.push(if already {
            lines[i].to_string()
        } else {
            insert_global(lines[i])
        });
        for k in (i + 1)..=end {
            out.push(lines[k].to_string());
        }
        i = end + 1;
    }
    let mut joined = out.join("\n");
    if block.ends_with('\n') {
        joined.push('\n');
    }
    joined
}

/// Byte ranges of the whitespace-separated words on a line.
fn words(line: &str) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    let mut start: Option<usize> = None;
    for (idx, ch) in line.char_indices() {
        if ch.is_whitespace() {
            if let Some(s) = start.take() {
                v.push((s, idx));
            }
        } else if start.is_none() {
            start = Some(idx);
        }
    }
    if let Some(s) = start {
        v.push((s, line.len()));
    }
    v
}

fn word_text(line: &str, (s, e): (usize, usize)) -> &str {
    line[s..e].trim_end_matches('.')
}

/// A line that opens an `01`-level data description: `01` followed by a name,
/// and not a `*>` comment.
fn starts_01_entry(line: &str) -> bool {
    if line.trim_start().starts_with("*>") {
        return false;
    }
    let w = words(line);
    w.len() >= 2 && word_text(line, w[0]) == "01" && !word_text(line, w[1]).is_empty()
}

fn ends_entry(line: &str) -> bool {
    line.trim_end().ends_with('.')
}

fn has_word(line: &str, needle: &str) -> bool {
    words(line)
        .into_iter()
        .any(|w| word_text(line, w).eq_ignore_ascii_case(needle))
}

/// Insert ` GLOBAL` into an `01` line, after the data-name (or after a
/// `REDEFINES` target). Returns the line unchanged for `01 FILLER`.
fn insert_global(line: &str) -> String {
    let w = words(line);
    if w.len() < 2 {
        return line.to_string();
    }
    let name = word_text(line, w[1]);
    if name.eq_ignore_ascii_case("FILLER") {
        return line.to_string();
    }
    // REDEFINES must sit immediately after the name it redefines, so step past
    // it and its target before inserting.
    let anchor = if w.len() >= 4 && word_text(line, w[2]).eq_ignore_ascii_case("REDEFINES") {
        w[3]
    } else {
        w[1]
    };
    // Land before the entry-terminating period when the anchor carries one.
    let at = if line[anchor.0..anchor.1].ends_with('.') {
        anchor.1 - 1
    } else {
        anchor.1
    };
    let mut s = String::with_capacity(line.len() + 7);
    s.push_str(&line[..at]);
    s.push_str(" GLOBAL");
    s.push_str(&line[at..]);
    s
}

/// Write a block's edited code back to the form. Returns whether it changed.
pub fn set_block_text(form: &mut Form, t: CsTarget, text: String) -> bool {
    let text = if declares_form_data(t) {
        ensure_global_on_01_levels(&text)
    } else {
        text
    };
    let slot: &mut String = match t {
        CsTarget::SpecialNames => &mut form.cobol_structure.special_names,
        CsTarget::Repository => &mut form.cobol_structure.repository,
        CsTarget::FileControl => &mut form.cobol_structure.file_control,
        CsTarget::FileSection => &mut form.cobol_structure.file_section,
        CsTarget::WorkingStorage => &mut form.user_ws_source,
        CsTarget::Procedure(i) => match form.user_procedures.get_mut(i) {
            Some(up) => &mut up.code,
            None => return false,
        },
    };
    if *slot != text {
        *slot = text;
        true
    } else {
        false
    }
}

/// The size of a national or UTF-8 item (spec 077, R23): what the COBOL
/// Structure editor shows for the declaration under the caret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSize {
    pub name: String,
    /// Character positions; `None` for `PIC U BYTE-LENGTH n`, whose count of
    /// characters depends on what it holds.
    pub chars: Option<usize>,
    /// Bytes of storage: 2 per `N`, 4 per `U`, or the `BYTE-LENGTH`.
    pub bytes: usize,
}

/// The size of the item declared on 1-based `line` of `block`, when that line
/// declares a national (`PIC N`) or UTF-8 (`PIC U`) item. `None` for any other
/// line — an alphanumeric item's characters are its bytes, so there is nothing
/// to tell apart.
pub fn item_size(block: &str, line: usize) -> Option<ItemSize> {
    let text = block.lines().nth(line.checked_sub(1)?)?;
    // A comment line declares nothing.
    if text.trim_start().starts_with("*>") || text.chars().nth(6).is_some_and(|c| c == '*' || c == '/') {
        return None;
    }
    let words: Vec<String> = text
        .split_whitespace()
        .map(|w| w.trim_end_matches('.').to_ascii_uppercase())
        .collect();
    let level = words.first()?;
    if !level.chars().all(|c| c.is_ascii_digit()) || level.parse::<u32>().ok()? >= 50 {
        return None;
    }
    let name = text.split_whitespace().nth(1)?.trim_end_matches('.').to_owned();
    let at = words.iter().position(|w| w == "PIC" || w == "PICTURE")?;
    let mut pic_at = at + 1;
    if words.get(pic_at).is_some_and(|w| w == "IS") {
        pic_at += 1;
    }
    let picture = words.get(pic_at)?;
    let (mut n, mut u) = (0usize, 0usize);
    let chars: Vec<char> = picture.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        let mut count = 1usize;
        if chars.get(i) == Some(&'(') {
            let close = chars[i..].iter().position(|&x| x == ')')? + i;
            count = chars[i + 1..close].iter().collect::<String>().trim().parse().ok()?;
            i = close + 1;
        }
        match c {
            'N' => n += count,
            'U' => u += count,
            _ => return None,
        }
    }
    let byte_length = words
        .iter()
        .position(|w| w == "BYTE-LENGTH")
        .and_then(|k| words.get(k + 1).filter(|w| *w == "IS").map(|_| k + 2).or(Some(k + 1)))
        .and_then(|k| words.get(k))
        .and_then(|w| w.parse::<usize>().ok());
    match (n, u, byte_length) {
        (n, 0, None) if n > 0 => Some(ItemSize { name, chars: Some(n), bytes: 2 * n }),
        (0, u, None) if u > 0 => Some(ItemSize { name, chars: Some(u), bytes: 4 * u }),
        (0, 1, Some(b)) => Some(ItemSize { name, chars: None, bytes: b }),
        _ => None,
    }
}

#[cfg(test)]
mod item_size_tests {
    use super::*;

    /// AC15: `PIC N(30)` is 30 characters and 60 bytes.
    #[test]
    fn national_and_utf8_sizes() {
        let block = "\
       01  WS-MSG     PIC N(30) VALUE N\"Olá\".
       01  WS-U       PICTURE IS U(5).
       01  WS-B       PIC U BYTE-LENGTH 8.
       01  WS-X       PIC X(10).
      *> 01 WS-C PIC N(3).
       01  WS-NN      PIC NNN USAGE NATIONAL.";
        let at = |l| item_size(block, l);
        assert_eq!(at(1), Some(ItemSize { name: "WS-MSG".into(), chars: Some(30), bytes: 60 }));
        assert_eq!(at(2), Some(ItemSize { name: "WS-U".into(), chars: Some(5), bytes: 20 }));
        assert_eq!(at(3), Some(ItemSize { name: "WS-B".into(), chars: None, bytes: 8 }));
        assert_eq!(at(4), None, "an alphanumeric item has nothing to tell apart");
        assert_eq!(at(5), None, "a comment declares nothing");
        assert_eq!(at(6), Some(ItemSize { name: "WS-NN".into(), chars: Some(3), bytes: 6 }));
        assert_eq!(at(0), None);
        assert_eq!(at(99), None);
    }
}

#[cfg(test)]
mod global_01_tests {
    use super::*;

    /// The case that keeps costing a whole form: three plain `01`s in the
    /// form's WORKING-STORAGE, none of them reachable from any handler.
    #[test]
    fn every_bare_01_gains_the_clause() {
        let src = "\
       01  WS-MENU-DATA.
           05  WS-HAMB-PRICE  PIC 9(2)V99.

       01  WS-CALCULATION-VARS.
           05  WS-TOTAL-AMOUNT  PIC 9(4)V99 VALUE 0.";
        let out = ensure_global_on_01_levels(src);
        assert!(out.contains("01  WS-MENU-DATA GLOBAL."), "{out}");
        assert!(out.contains("01  WS-CALCULATION-VARS GLOBAL."), "{out}");
        // Subordinates are untouched: GLOBAL goes on the 01 and carries the
        // whole subtree, and on a 05 it is a hard error.
        assert!(out.contains("05  WS-HAMB-PRICE  PIC 9(2)V99."), "{out}");
        assert!(!out.contains("05  WS-HAMB-PRICE GLOBAL"), "{out}");
    }

    /// An elementary `01` carries its clauses on the same line; the clause goes
    /// after the data-name, before the terminating period.
    #[test]
    fn an_elementary_01_keeps_its_picture_and_value() {
        let out = ensure_global_on_01_levels("       01  WS-TOTAL  PIC 9(4)V99 VALUE 0.");
        assert_eq!(out, "       01  WS-TOTAL GLOBAL  PIC 9(4)V99 VALUE 0.");
    }

    /// Applying it twice must not stack clauses — the editor saves on every
    /// keystroke-committed edit, and the agent may resend a change-set.
    #[test]
    fn it_is_idempotent() {
        let once = ensure_global_on_01_levels("       01  WS-DATA.");
        let twice = ensure_global_on_01_levels(&once);
        assert_eq!(once, twice);
        assert_eq!(once.matches("GLOBAL").count(), 1, "{once}");
    }

    /// COBOL-85 does not allow both on one item, and an EXTERNAL item is
    /// already shared across the run unit — adding GLOBAL would be an error.
    #[test]
    fn external_and_already_global_items_are_left_alone() {
        let src = "\
       01  WS-SHARED EXTERNAL.
       01  WS-ALREADY GLOBAL.
       01  WS-LOWERCASE global.";
        assert_eq!(ensure_global_on_01_levels(src), src);
    }

    /// `REDEFINES` must immediately follow the name it redefines, so the clause
    /// has to go after the redefined target, not between the two.
    #[test]
    fn redefines_keeps_its_place_next_to_the_name() {
        let out = ensure_global_on_01_levels("       01  WS-ALT REDEFINES WS-MAIN.");
        assert_eq!(out, "       01  WS-ALT REDEFINES WS-MAIN GLOBAL.");
    }

    /// A `FILLER` has no name to reach it by, so the clause would buy nothing.
    #[test]
    fn filler_is_skipped() {
        let src = "       01  FILLER  PIC X(8).";
        assert_eq!(ensure_global_on_01_levels(src), src);
    }

    /// Comments, blank lines, FD entries and deeper levels pass through
    /// untouched — this only ever adds a clause to an `01`.
    #[test]
    fn everything_that_is_not_an_01_passes_through() {
        let src = "\
       *> 01  WS-NOT-REAL. — a comment, not a declaration

       FD  CUSTOMER-FILE.
       01  CUST-REC.
           05  CUST-ID  PIC 9(6).
       77  WS-STANDALONE  PIC 9.";
        let out = ensure_global_on_01_levels(src);
        assert!(out.contains("*> 01  WS-NOT-REAL."), "comment changed: {out}");
        assert!(out.contains("FD  CUSTOMER-FILE."), "FD changed: {out}");
        assert!(out.contains("77  WS-STANDALONE  PIC 9."), "77 changed: {out}");
        // The record under the FD is an 01 and does get the clause.
        assert!(out.contains("01  CUST-REC GLOBAL."), "{out}");
    }

    /// A declaration wrapped over several lines is one entry: the clause must
    /// be recognised wherever it sits, and added only once.
    #[test]
    fn a_wrapped_declaration_is_treated_as_one_entry() {
        let already = "\
       01  WS-WRAPPED
               GLOBAL
               PIC X(10).";
        assert_eq!(ensure_global_on_01_levels(already), already);

        let bare = "\
       01  WS-WRAPPED
               PIC X(10).
       01  WS-NEXT.";
        let out = ensure_global_on_01_levels(bare);
        assert!(out.contains("01  WS-WRAPPED GLOBAL"), "{out}");
        assert!(out.contains("01  WS-NEXT GLOBAL."), "{out}");
        assert_eq!(out.matches("GLOBAL").count(), 2, "{out}");
    }

    /// Only the two blocks that declare the form's data are rewritten. A
    /// procedure body or SPECIAL-NAMES must never be touched.
    #[test]
    fn only_the_data_declaring_blocks_are_rewritten() {
        assert!(declares_form_data(CsTarget::WorkingStorage));
        assert!(declares_form_data(CsTarget::FileSection));
        for t in [
            CsTarget::SpecialNames,
            CsTarget::Repository,
            CsTarget::FileControl,
            CsTarget::Procedure(0),
        ] {
            assert!(!declares_form_data(t), "{t:?} must be left alone");
        }
    }

    /// Trailing newline is part of the text the editor round-trips; losing it
    /// would show as a spurious diff on every save.
    #[test]
    fn a_trailing_newline_survives() {
        assert_eq!(
            ensure_global_on_01_levels("       01  WS-A.\n"),
            "       01  WS-A GLOBAL.\n"
        );
        assert_eq!(ensure_global_on_01_levels(""), "");
    }

    /// Spec 053 T4: the editor targets and the model's code sites are one
    /// list — every section round-trips CsTarget → CodeSite → CsTarget, and
    /// SECTIONS follows the model's division order.
    #[test]
    fn cs_target_and_code_site_are_one_list() {
        let form = cobolt_forms::code_site::all_sites_fixture();
        for (i, section) in StructureSection::ALL.into_iter().enumerate() {
            let target = CsTarget::for_section(section);
            assert_eq!(SECTIONS[i], target, "SECTIONS follows StructureSection::ALL");
            assert_eq!(target.section(), Some(section));
            assert_eq!(target.section_keyword(), Some(section.keyword()));
            let site = target.to_code_site(&form).expect("section site");
            assert_eq!(site, CodeSite::Section(section));
            assert_eq!(CsTarget::from_code_site(&site, &form), Some(target));
        }
        // A procedure round-trips through its name.
        let target = CsTarget::Procedure(0);
        let site = target.to_code_site(&form).expect("procedure site");
        assert_eq!(
            site,
            CodeSite::Procedure {
                name: "VALIDATE-CUSTOMER".into()
            }
        );
        assert_eq!(CsTarget::from_code_site(&site, &form), Some(target));
        // An out-of-range procedure index addresses nothing.
        assert_eq!(CsTarget::Procedure(9).to_code_site(&form), None);
    }
}
