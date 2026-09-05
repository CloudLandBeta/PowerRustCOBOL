// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! "Hide empty blocks" — a **view** filter over the debugger's source pane.
//!
//! A generated `.cbl` is full of divisions, sections and paragraphs that exist
//! only because COBOL's structure requires them: an `ENVIRONMENT DIVISION` with
//! nothing in it, a paragraph that is a label and a period. Reading a program
//! through them costs the developer a screen of scrolling for no information.
//!
//! Three properties make this safe, and all three are tested below:
//!
//! 1. **It changes nothing but the display.** No file is touched, and the
//!    debugger's stepping, breakpoints and line mapping never consult it — a
//!    hidden line is still a line the program can stop on. (Operator ruling,
//!    2026-09-02: "visually only".)
//! 2. **Line numbers stay the file's own.** Hiding lines 251–254 leaves the
//!    next visible line numbered 255, so what the pane shows and what a
//!    breakpoint means are the same number.
//! 3. **Nothing is ever destroyed.** The developer's code is sacred; a run of
//!    hidden lines collapses to one expandable marker that gives them back.
//!
//! # What counts as empty
//!
//! A block holds **no executable statement**. Blank lines, comments, and a body
//! whose only content is `CONTINUE` or `EXIT` are all empty — those are
//! placeholders, not work. A paragraph containing anything else is not.

/// Why a run of lines is folded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldKind {
    /// A division, section or paragraph with no executable statement.
    Empty,
    /// A region codegen marked `*> <NAME>` … `*> </NAME>`.
    ///
    /// Hidden by default and for a different reason from an empty block: it is
    /// not that there is nothing there, it is that it is not the developer's
    /// code and is assumed to work (operator, 2026-09-02).
    Generated,
}

/// One run of consecutive source lines the pane will hide, and the marker row
/// that replaces it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenRun {
    /// First hidden line, 1-based inclusive.
    pub start: u32,
    /// Last hidden line, 1-based inclusive.
    pub end: u32,
    /// How many blocks were folded into this run — what the marker counts.
    pub blocks: usize,
    pub kind: FoldKind,
    /// The region's name, for a [`FoldKind::Generated`] run.
    pub label: Option<String>,
}

impl HiddenRun {
    pub fn contains(&self, line: u32) -> bool {
        line >= self.start && line <= self.end
    }

    pub fn lines(&self) -> u32 {
        self.end - self.start + 1
    }
}

/// Is this line a COBOL comment or blank — i.e. carries no code either way?
fn is_blank_or_comment(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return true;
    }
    // Free-form `*>` anywhere, and fixed-form `*` or `/` in the indicator
    // column (column 7, index 6).
    if trimmed.starts_with("*>") || trimmed.starts_with('*') || trimmed.starts_with('/') {
        return true;
    }
    matches!(line.as_bytes().get(6), Some(b'*') | Some(b'/'))
}

/// The code on a line, uppercased, with any fixed-form sequence area and
/// trailing identification area removed.
fn code_of(line: &str) -> String {
    // Columns 73+ are the identification area in fixed form and are not code.
    // Anything shorter is free form and is taken whole.
    //
    // A column is a CHARACTER, not a byte. `&line[..72]` panics the moment a
    // line carries an accented literal long enough to put a multi-byte
    // character across byte 72 — `VALUE 'Solicitações'` does exactly that — so
    // the cut goes through the lexer's own column helper, which is where the
    // rule is defined for the fixed-format reader.
    let cut = cobolt_lexer::source::char_boundary_at_col(line, 72);
    line[..cut].trim().to_ascii_uppercase()
}

/// Does this line open a block — a DIVISION, a SECTION, or a paragraph name?
///
/// A paragraph header is a word in Area A (columns 8–11) ending in a period
/// with nothing after it. Testing the *column* is what separates a paragraph
/// name from a statement: both are words followed by a period, and only the
/// margin tells them apart.
fn block_header(line: &str) -> Option<Rank> {
    if is_blank_or_comment(line) {
        return None;
    }
    let code = code_of(line);
    if code.ends_with("DIVISION.") {
        return Some(Rank::Division);
    }
    if code.ends_with("SECTION.") {
        return Some(Rank::Section);
    }
    // Area A is columns 8–11, i.e. indices 7–10; Area B begins at column 12.
    // An indent of 11 is therefore a STATEMENT, not a paragraph name — and the
    // margin is the only thing that distinguishes `GOBACK.` from a paragraph
    // called `GOBACK`. A free-form file with no indentation puts the name at
    // index 0, which counts as Area A here.
    let indent = line.len() - line.trim_start().len();
    if indent > 10 {
        return None;
    }
    let word = code.trim_end_matches('.');
    if code.ends_with('.')
        && !word.is_empty()
        && !word.contains(' ')
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && word.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
    {
        return Some(Rank::Paragraph);
    }
    None
}

/// How much a header encloses. A DIVISION contains SECTIONs, which contain
/// paragraphs — so a block runs until the next header of **equal or higher**
/// rank, and its emptiness is judged over everything nested inside it.
///
/// Without this, `PROCEDURE DIVISION.` followed by working paragraphs looks
/// empty: its own next line is another header, so a flat sibling model finds no
/// statements between the two and folds away the entire procedure division.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Rank {
    Division,
    Section,
    Paragraph,
}

/// Statements that occupy a block without doing anything — a placeholder body
/// is still an empty block.
fn is_placeholder_statement(code: &str) -> bool {
    let bare = code.trim_end_matches('.').trim();
    matches!(bare, "CONTINUE" | "EXIT" | "EXIT PARAGRAPH" | "EXIT SECTION")
}

/// Find every run of lines that "hide empty blocks" should fold away.
///
/// Returns runs in ascending order, never overlapping. Adjacent empty blocks
/// merge into one run so four empty divisions collapse to a single marker
/// rather than four.
pub fn hidden_runs(lines: &[String]) -> Vec<HiddenRun> {
    let headers: Vec<(usize, Rank)> = (0..lines.len())
        .filter_map(|i| block_header(&lines[i]).map(|r| (i, r)))
        .collect();
    if headers.is_empty() {
        return Vec::new();
    }

    let mut runs: Vec<HiddenRun> = Vec::new();
    let mut n = 0usize;
    while n < headers.len() {
        let (start, rank) = headers[n];
        // The block ends where the next header of equal or higher rank begins.
        let next_peer = headers[n + 1..]
            .iter()
            .position(|(_, r)| *r <= rank)
            .map(|off| headers[n + 1 + off].0)
            .unwrap_or(lines.len());

        let has_work = (start..next_peer).any(|i| {
            if i == start || is_blank_or_comment(&lines[i]) {
                return false;
            }
            // A nested header is structure, not work — its own emptiness is
            // what decides, and it is inside this extent either way.
            if block_header(&lines[i]).is_some() {
                return false;
            }
            !is_placeholder_statement(&code_of(&lines[i]))
        });

        if has_work {
            n += 1; // descend into it; nested blocks are judged on their own
            continue;
        }

        // Empty, with everything nested inside it. Its extent stops at the last
        // non-blank line: trailing blanks before the next block belong to
        // neither, and swallowing them would move the following code up.
        let last = (start..next_peer)
            .rev()
            .find(|&i| !lines[i].trim().is_empty())
            .unwrap_or(start);
        let blocks = headers[n..]
            .iter()
            .take_while(|(i, _)| *i <= last)
            .count()
            .max(1);
        let (s, e) = (start as u32 + 1, last as u32 + 1);
        match runs.last_mut() {
            // Merge with the previous run when only blank lines separate them,
            // so four empty divisions collapse to one marker, not four.
            Some(prev)
                if lines[prev.end as usize..s as usize - 1]
                    .iter()
                    .all(|l| l.trim().is_empty()) =>
            {
                prev.end = e;
                prev.blocks += blocks;
            }
            _ => runs.push(HiddenRun {
                start: s,
                end: e,
                blocks,
                kind: FoldKind::Empty,
                label: None,
            }),
        }
        // Skip every header this run swallowed.
        n += headers[n..].iter().take_while(|(i, _)| *i <= last).count().max(1);
    }
    runs
}

/// The `*> <NAME>` / `*> </NAME>` regions codegen marks.
///
/// Matched by NAME, not by nesting depth: an unclosed region would otherwise
/// swallow the rest of the file, and a generated block that fails to close is
/// exactly the kind of thing that happens once and is never noticed.
pub fn generated_runs(lines: &[String]) -> Vec<HiddenRun> {
    let mut open: Vec<(String, usize)> = Vec::new();
    let mut runs = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        let Some(rest) = t.strip_prefix("*>") else {
            continue;
        };
        let rest = rest.trim();
        if let Some(name) = rest.strip_prefix("</").and_then(|n| n.strip_suffix('>')) {
            if let Some(pos) = open.iter().rposition(|(n, _)| n == name) {
                let (name, start) = open.remove(pos);
                runs.push(HiddenRun {
                    start: start as u32 + 1,
                    end: i as u32 + 1,
                    blocks: 1,
                    kind: FoldKind::Generated,
                    label: Some(name),
                });
            }
        } else if let Some(name) = rest.strip_prefix('<').and_then(|n| n.strip_suffix('>')) {
            if !name.starts_with('/') {
                open.push((name.to_owned(), i));
            }
        }
    }
    runs.sort_by_key(|r| r.start);
    // Drop a region nested inside another: the outer fold already hides it, and
    // two markers for one hidden run reads as a bug.
    let mut out: Vec<HiddenRun> = Vec::new();
    for r in runs {
        if out.last().is_some_and(|p| r.end <= p.end) {
            continue;
        }
        out.push(r);
    }
    out
}

/// Every run the pane should fold, given which filters are on.
///
/// Generated regions win where the two overlap: an empty paragraph INSIDE the
/// event loop is hidden because it is generated, and saying "1 empty block
/// hidden" about it would be describing the wrong reason.
pub fn folds(lines: &[String], hide_empty: bool, hide_generated: bool) -> Vec<HiddenRun> {
    let mut runs: Vec<HiddenRun> = Vec::new();
    if hide_generated {
        runs.extend(generated_runs(lines));
    }
    if hide_empty {
        for r in hidden_runs(lines) {
            if !runs.iter().any(|g| r.start >= g.start && r.end <= g.end) {
                runs.push(r);
            }
        }
    }
    runs.sort_by_key(|r| r.start);
    runs
}

/// The marker text for a run, in the developer's language.
///
/// `template` is the `dbg_empty_blocks_hidden` string, which carries a `{n}`
/// placeholder rather than a fixed word order — "4 empty blocks hidden" and
/// "已隐藏 4 个空块" do not put the count in the same place.
pub fn marker_text(template: &str, run: &HiddenRun) -> String {
    template.replace("{n}", &run.blocks.to_string())
}

#[cfg(test)]
mod empty_block_tests {
    use super::*;

    fn src(text: &str) -> Vec<String> {
        text.lines().map(|l| l.to_owned()).collect()
    }

    #[test]
    fn an_empty_division_is_hidden_and_a_full_one_is_not() {
        let lines = src(
            "       IDENTIFICATION DIVISION.\n\
             \x20      PROGRAM-ID. DEMO.\n\
             \x20      ENVIRONMENT DIVISION.\n\
             \x20      PROCEDURE DIVISION.\n\
             \x20          DISPLAY \"HELLO\".",
        );
        let runs = hidden_runs(&lines);
        // ENVIRONMENT DIVISION (line 3) is empty; the other two are not.
        assert_eq!(runs.len(), 1, "{runs:?}");
        assert_eq!((runs[0].start, runs[0].end, runs[0].blocks), (3, 3, 1));
    }

    /// The mockup's case: four consecutive empty blocks collapse to ONE marker
    /// reading "4 empty blocks hidden", not four markers.
    #[test]
    fn adjacent_empty_blocks_merge_into_one_run() {
        let lines = src(
            "       PROGRAM-ID. DEMO.\n\
             \x20      ENVIRONMENT DIVISION.\n\
             \x20      CONFIGURATION SECTION.\n\
             \x20      INPUT-OUTPUT SECTION.\n\
             \x20      DATA DIVISION.\n\
             \x20      PROCEDURE DIVISION.\n\
             \x20          DISPLAY \"X\".",
        );
        let runs = hidden_runs(&lines);
        assert_eq!(runs.len(), 1, "{runs:?}");
        assert_eq!(runs[0].blocks, 4, "four empty blocks, one marker");
        assert_eq!((runs[0].start, runs[0].end), (2, 5));
        assert_eq!(marker_text("{n} empty blocks hidden", &runs[0]), "4 empty blocks hidden");
    }

    /// The operator's ruling: a body whose only content is CONTINUE or EXIT is
    /// a placeholder, and the block still counts as empty.
    #[test]
    fn a_continue_only_paragraph_is_empty() {
        let lines = src(
            "       PROCEDURE DIVISION.\n\
             \x20      MAIN-PARA.\n\
             \x20          DISPLAY \"WORK\".\n\
             \x20      DONE-PARA.\n\
             \x20          CONTINUE.\n\
             \x20      EXIT-PARA.\n\
             \x20          EXIT.",
        );
        let runs = hidden_runs(&lines);
        assert_eq!(runs.len(), 1, "{runs:?}");
        assert_eq!(runs[0].blocks, 2, "DONE-PARA and EXIT-PARA");
        assert_eq!((runs[0].start, runs[0].end), (4, 7));
    }

    /// Property 2, and the one a developer would notice instantly: a hidden run
    /// must not renumber what follows it.
    #[test]
    fn line_numbers_after_a_hidden_run_are_unchanged() {
        let lines = src(
            "       PROGRAM-ID. DEMO.\n\
             \x20      ENVIRONMENT DIVISION.\n\
             \x20      DATA DIVISION.\n\
             \x20      PROCEDURE DIVISION.\n\
             \x20          DISPLAY \"HELLO\".",
        );
        let runs = hidden_runs(&lines);
        let hidden: Vec<u32> = (1..=lines.len() as u32)
            .filter(|&l| runs.iter().any(|r| r.contains(l)))
            .collect();
        assert_eq!(hidden, vec![2, 3]);
        // The DISPLAY is line 5 whether or not anything is hidden.
        assert!(!runs.iter().any(|r| r.contains(5)));
        assert_eq!(lines[4].trim(), "DISPLAY \"HELLO\".");
    }

    /// Comments are not work. A paragraph documented but not implemented is
    /// still empty — and the comment is hidden with it, not orphaned.
    #[test]
    fn comments_alone_do_not_make_a_block_full() {
        let lines = src(
            "       PROCEDURE DIVISION.\n\
             \x20          DISPLAY \"X\".\n\
             \x20      TODO-PARA.\n\
             \x20     * to be written\n\
             \x20          *> also a comment",
        );
        let runs = hidden_runs(&lines);
        assert_eq!(runs.len(), 1, "{runs:?}");
        assert_eq!((runs[0].start, runs[0].end), (3, 5));
    }

    /// A COBOL column is a character, not a byte. A generated line carrying an
    /// accented literal put an 'o' with a tilde across byte 72, and cutting the
    /// identification area by byte index sliced it in half — panicking the IDE
    /// on a file it was only trying to *display*.
    #[test]
    fn a_line_whose_column_72_falls_inside_a_character_does_not_panic() {
        // The line from the report, verbatim.
        let line =
            "          05 WS-lblSolicitacoes-TEXT       PIC X(256) VALUE 'Solicita\u{e7}\u{f5}es'.";
        assert_eq!(line.len(), 77, "77 bytes");
        assert_eq!(line.chars().count(), 75, "but only 75 columns");

        let code = code_of(line);
        assert!(code.starts_with("05 WS-LBLSOLICITACOES-TEXT"), "{code}");
        // Cut at character 72, then the ten leading spaces trimmed away.
        assert_eq!(code.chars().count(), 62, "{code}");

        // A line that fits is taken whole, accents and all. Only ASCII is
        // uppercased, so the accented characters come back unchanged.
        assert_eq!(
            code_of("       MOVE 'Solicita\u{e7}\u{f5}es' TO WS-X."),
            "MOVE 'SOLICITA\u{e7}\u{f5}ES' TO WS-X."
        );

        // And every caller that reads a line through code_of survives it.
        assert!(block_header(line).is_none());
        assert!(!is_placeholder_statement(&code_of(line)));
        let _ = hidden_runs(&[line.to_string()]);
    }

    /// A statement is not a paragraph header just because it ends in a period.
    /// Only the Area A margin tells them apart, and getting this wrong would
    /// hide real code.
    #[test]
    fn an_indented_statement_is_not_mistaken_for_a_paragraph_header() {
        assert!(block_header("       MAIN-PARA.").is_some());
        // Indent 11 is column 12 — Area B, so it is a statement.
        assert!(block_header("           GOBACK.").is_none());
        assert!(block_header("           CONTINUE.").is_none());
        assert!(block_header("       PROCEDURE DIVISION.").is_some());
        assert!(block_header("      * a comment.").is_none());
        assert!(block_header("").is_none());
    }

    /// Nothing is hidden in a program that is all work — the filter must be
    /// invisible when there is nothing to fold.
    #[test]
    fn a_program_with_no_empty_blocks_hides_nothing() {
        let lines = src(
            "       PROCEDURE DIVISION.\n\
             \x20      MAIN-PARA.\n\
             \x20          DISPLAY \"A\".\n\
             \x20      NEXT-PARA.\n\
             \x20          DISPLAY \"B\".",
        );
        assert!(hidden_runs(&lines).is_empty());
    }

    /// The regions codegen marks fold whole, so the developer's own handler is
    /// not buried under the event loop.
    #[test]
    fn a_marked_region_folds_as_one_run() {
        let lines = src(
            "       PROCEDURE DIVISION.\n\
             \x20          DISPLAY \"MINE\".\n\
             \x20     *> <EVENT-LOOP>\n\
             \x20      COBOL-EVENT-LOOP.\n\
             \x20          CALL \"COBOL-WAIT-EVENT\".\n\
             \x20     *> </EVENT-LOOP>\n\
             \x20          DISPLAY \"MINE AGAIN\".",
        );
        let runs = generated_runs(&lines);
        assert_eq!(runs.len(), 1, "{runs:?}");
        assert_eq!((runs[0].start, runs[0].end), (3, 6), "markers included");
        assert_eq!(runs[0].kind, FoldKind::Generated);
        assert_eq!(runs[0].label.as_deref(), Some("EVENT-LOOP"));
        // The developer's own lines are untouched on both sides.
        assert!(!runs[0].contains(2) && !runs[0].contains(7));
    }

    /// Regions are paired by NAME. An unclosed one must fold nothing rather
    /// than swallow the rest of the file.
    #[test]
    fn an_unclosed_region_folds_nothing() {
        let lines = src(
            "      *> <EVENT-LOOP>\n\
             \x20          DISPLAY \"X\".\n\
             \x20          DISPLAY \"Y\".",
        );
        assert!(generated_runs(&lines).is_empty(), "no close, no fold");
    }

    #[test]
    fn a_region_inside_a_region_is_folded_once() {
        let lines = src(
            "      *> <OUTER>\n\
             \x20     *> <INNER>\n\
             \x20          DISPLAY \"X\".\n\
             \x20     *> </INNER>\n\
             \x20     *> </OUTER>",
        );
        let runs = generated_runs(&lines);
        assert_eq!(runs.len(), 1, "one marker, not two: {runs:?}");
        assert_eq!(runs[0].label.as_deref(), Some("OUTER"));
    }

    /// Where the two filters overlap, GENERATED wins: an empty paragraph inside
    /// the event loop is hidden because it is generated, and calling it an
    /// empty block would name the wrong reason.
    #[test]
    fn generated_wins_over_empty_where_they_overlap() {
        let lines = src(
            "       PROCEDURE DIVISION.\n\
             \x20          DISPLAY \"MINE\".\n\
             \x20     *> <EVENT-LOOP>\n\
             \x20      EMPTY-PARA.\n\
             \x20     *> </EVENT-LOOP>",
        );
        let runs = folds(&lines, true, true);
        assert_eq!(runs.len(), 1, "{runs:?}");
        assert_eq!(runs[0].kind, FoldKind::Generated);

        // With generated folding OFF, the empty paragraph folds on its own.
        let only_empty = folds(&lines, true, false);
        assert!(only_empty.iter().all(|r| r.kind == FoldKind::Empty));
        // And with both off, nothing folds.
        assert!(folds(&lines, false, false).is_empty());
    }

    #[test]
    fn an_empty_source_is_handled_without_panicking() {
        assert!(hidden_runs(&[]).is_empty());
        assert!(hidden_runs(&[String::new()]).is_empty());
        assert!(hidden_runs(&["   ".into(), "".into()]).is_empty());
    }

    /// Trailing blank lines belong to nobody: swallowing them into a hidden run
    /// would pull the following code up the pane and make the fold look like it
    /// removed a gap the developer wrote.
    #[test]
    fn a_hidden_run_stops_at_its_last_real_line() {
        let lines = src(
            "       PROGRAM-ID. DEMO.\n\
             \x20      ENVIRONMENT DIVISION.\n\
             \n\
             \n\
             \x20      PROCEDURE DIVISION.\n\
             \x20          DISPLAY \"X\".",
        );
        let runs = hidden_runs(&lines);
        assert_eq!(runs.len(), 1);
        assert_eq!((runs[0].start, runs[0].end), (2, 2), "blank lines 3-4 stay");
    }
}
