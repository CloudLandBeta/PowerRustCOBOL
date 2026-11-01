// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Compact helper paragraphs** (spec 109 S5, R30, T5.2).
//!
//! Besides the control groups, the generator writes small paragraphs for the
//! developer to `PERFORM`: the animation dispatch (`COBOL-PLAY-ANIMATION`,
//! `COBOL-STOP-ANIMATION`), one `<ctrl>-PLAY-<name>` per triggered animation, a
//! chart's `-SET-TABLE` / `-ADD-POINT` / `-CLEAR` / `-REFRESH`, a database's
//! `-CONNECT` / `-EXEC` / `-FETCH-ALL` / `-CLOSE` and a grid's `-EXPORT-CSV`.
//! Nothing in the runtime, the form host or the browser generator looks one up by
//! name, so one that no other line of the program names is read by nothing.
//!
//! A form that opts into `compact-storage` has such a paragraph left out. The
//! rule is the same as for the groups, and as conservative: a name counts as
//! "named" if it appears as a whole word on any line outside the paragraph's own
//! span, comments included. Removing a paragraph can leave another unnamed (the
//! dispatch, once its triggers are gone), so the pass repeats until nothing more
//! goes. The source map is shifted, so a stop line or an error still resolves.

use std::collections::HashSet;

use cobolt_forms::{Control, Form};

use crate::{collect_all_controls, SourceMap};

/// The helper paragraphs the generator can write for `form`, upper-case.
pub(crate) fn candidates(form: &Form) -> HashSet<String> {
    let mut names: HashSet<String> = ["COBOL-PLAY-ANIMATION", "COBOL-STOP-ANIMATION"].iter().map(|s| (*s).to_owned()).collect();
    for c in collect_all_controls(&form.controls) {
        for suffix in ["SET-TABLE", "ADD-POINT", "CLEAR", "REFRESH", "CONNECT", "EXEC", "FETCH-ALL", "CLOSE", "EXPORT-CSV"] {
            names.insert(format!("{}-{suffix}", c.id).to_ascii_uppercase());
        }
        if let Some(p) = custom_csv_paragraph(c) {
            names.insert(p.to_ascii_uppercase());
        }
        for a in &c.animations {
            names.insert(format!("{}-PLAY-{}", c.id, a.name.to_ascii_uppercase().replace(' ', "-")).to_ascii_uppercase());
        }
    }
    names
}

fn custom_csv_paragraph(c: &Control) -> Option<String> {
    c.get_prop("CSVParagraph").map(|v| v.as_str().trim().to_owned()).filter(|s| !s.is_empty())
}

struct Paragraph {
    name: String,
    /// 0-based, `start..end` of the lines it takes, up to the next label.
    start: usize,
    end: usize,
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-'
}

/// The paragraphs of the outer program: from its first `PROCEDURE DIVISION`
/// to the first nested unit.
fn outer_paragraphs(lines: &[String]) -> Vec<Paragraph> {
    let up = |l: &str| l.trim_start().to_ascii_uppercase();
    let Some(from) = lines.iter().position(|l| up(l).starts_with("PROCEDURE DIVISION")) else { return Vec::new() };
    let to = lines[from..]
        .iter()
        .position(|l| up(l).starts_with("IDENTIFICATION DIVISION"))
        .map_or(lines.len(), |p| from + p);
    let labels: Vec<(usize, String)> = (from + 1..to)
        .filter_map(|i| {
            let name = lines[i].strip_prefix("       ")?.strip_suffix('.')?;
            let ok = !name.is_empty()
                && name.chars().all(is_word_char)
                && name.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
                && !name.to_ascii_uppercase().contains("DIVISION");
            ok.then(|| (i, name.to_ascii_uppercase()))
        })
        .collect();
    labels
        .iter()
        .enumerate()
        .map(|(k, (at, name))| Paragraph { name: name.clone(), start: *at, end: labels.get(k + 1).map_or(to, |(n, _)| *n) })
        .collect()
}

/// Does `name` appear as a whole word on any line outside `skip`?
fn named_elsewhere(lines: &[String], name: &str, skip: std::ops::Range<usize>) -> bool {
    lines.iter().enumerate().filter(|(i, _)| !skip.contains(i)).any(|(_, l)| {
        let up = l.to_ascii_uppercase();
        up.match_indices(name).any(|(at, _)| {
            let before = up[..at].chars().next_back();
            let after = up[at + name.len()..].chars().next();
            before.is_none_or(|c| !is_word_char(c)) && after.is_none_or(|c| !is_word_char(c))
        })
    })
}

impl SourceMap {
    /// Take generated lines `first..=last` (1-based) out of the map's frame:
    /// a span after them moves up, one inside them goes, one that straddles them
    /// loses the part they held.
    pub(crate) fn remove_lines(&mut self, first: u32, last: u32) {
        let n = last - first + 1;
        let adjust = |start: u32, end: u32| -> Option<(u32, u32)> {
            let before_start = if start > last { n } else if start >= first { start - first } else { 0 };
            let through_end = if end > last { n } else if end >= first { end - first + 1 } else { 0 };
            let (s, e_excl) = (start - before_start, end + 1 - through_end);
            (e_excl > s).then_some((s, e_excl - 1))
        };
        self.spans.retain_mut(|s| match adjust(s.gen_start, s.gen_end) {
            Some((a, b)) => {
                s.gen_start = a;
                s.gen_end = b;
                true
            }
            None => false,
        });
        self.owned.retain_mut(|s| match adjust(s.gen_start, s.gen_end) {
            Some((a, b)) => {
                s.gen_start = a;
                s.gen_end = b;
                true
            }
            None => false,
        });
    }
}

/// Leave out every helper paragraph of `candidates` that nothing names, and
/// return the program without them.
pub(crate) fn omit_unnamed(text: String, map: &mut SourceMap, candidates: &HashSet<String>) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    loop {
        let paras = outer_paragraphs(&lines);
        let Some(p) = paras
            .iter()
            .rev()
            .find(|p| candidates.contains(&p.name) && !named_elsewhere(&lines, &p.name, p.start..p.end))
        else {
            break;
        };
        let (start, end) = (p.start, p.end);
        map.remove_lines(start as u32 + 1, end as u32);
        lines.drain(start..end);
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}
