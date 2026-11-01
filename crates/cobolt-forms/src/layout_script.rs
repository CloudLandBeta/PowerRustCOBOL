// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Layout scripts** (spec 109 S6, R32): `COBOL::Layout("Apply", container, script)`.
//!
//! A script is shorthand for properties the layout solver already reads. It adds
//! no solver input and no coordinate system: every line lowers to
//! `(control, property, value)` triples, and those are installed as
//! **breakpoint overrides** (spec 056 R59) — the mechanism a form already uses
//! for "this panel is a column when the window is narrow". The design is never
//! written (056 R10), a program's own write to a property still wins (R64), and
//! `Clear` takes the overrides away.
//!
//! The script is data in a string, one statement per line or separated by `;`
//! (`*>` starts a comment):
//!
//! ```text
//! MODE FLEX ROW|COLUMN [WRAP]      MODE GRID | FLOW | ABSOLUTE
//! GAP n [n]                        PADDING n | HORIZONTAL n | VERTICAL n
//! COLUMNS n | tracks               ROWS n | tracks
//! ORDER ctrl ctrl …                ITEM ctrl [SPAN n] [ROW-SPAN n] [GROW n]
//!                                       [MIN-WIDTH n] [MAX-WIDTH n] [MIN-HEIGHT n] [MAX-HEIGHT n]
//! WHEN breakpoint                  the lines below apply only while it is active
//! ```
//!
//! Lines before the first `WHEN` apply at every breakpoint; a `WHEN` section
//! runs to the next `WHEN` or the end.

use crate::layout::breakpoints::{Breakpoint, Override};
use crate::model::PropValue;
use crate::sequence::{entries_of, words};

/// A script that does not read: the line it stopped on and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptError {
    pub line: usize,
    pub text: String,
    pub message: String,
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {} (`{}`)", self.line, self.message, self.text)
    }
}

/// One property of one control.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Write {
    pub control: String,
    pub property: String,
    pub value: String,
}

/// The writes of a run of lines: for every breakpoint (`None`) or for one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub breakpoint: Option<String>,
    pub writes: Vec<Write>,
}

/// The text a breakpoint field holds for "every breakpoint".
pub const EVERY: &str = "*";

fn count(v: &str) -> Option<u32> {
    v.parse::<u32>().ok()
}

fn amount(v: &str) -> Option<String> {
    v.parse::<f32>().ok().filter(|n| *n >= 0.0).map(|_| v.to_owned())
}

/// Read a script. `container` is the control the lines configure.
pub fn parse(container: &str, script: &str) -> Result<Vec<Section>, ScriptError> {
    let mut sections = vec![Section { breakpoint: None, writes: Vec::new() }];
    for (n, raw) in script.lines().enumerate() {
        let line = n + 1;
        for part in entries_of(raw) {
            let w = words(&part);
            if w.is_empty() {
                continue;
            }
            let fail = |message: &str| ScriptError { line, text: part.trim().to_owned(), message: message.to_owned() };
            let verb = w[0].to_ascii_uppercase();
            let arg = |i: usize| w.get(i).map(String::as_str);
            let to = |property: &str, value: String| Write {
                control: container.to_owned(),
                property: property.to_owned(),
                value,
            };
            let mut out: Vec<Write> = Vec::new();
            match verb.as_str() {
                "WHEN" => {
                    let Some(name) = arg(1) else { return Err(fail("WHEN needs a breakpoint name")) };
                    if w.len() > 2 {
                        return Err(fail("WHEN takes one breakpoint name"));
                    }
                    sections.push(Section { breakpoint: Some(name.to_owned()), writes: Vec::new() });
                    continue;
                }
                "MODE" => {
                    let mode = arg(1).map(str::to_ascii_uppercase);
                    match mode.as_deref() {
                        Some("FLEX") => {
                            let mut dir = "Row";
                            let mut wrap = "NoWrap";
                            for extra in &w[2..] {
                                match extra.to_ascii_uppercase().as_str() {
                                    "ROW" => dir = "Row",
                                    "COLUMN" => dir = "Column",
                                    "WRAP" => wrap = "Wrap",
                                    _ => return Err(fail("after MODE FLEX expect ROW, COLUMN or WRAP")),
                                }
                            }
                            out.push(to("LayoutMode", "Flex".into()));
                            out.push(to("FlexDirection", dir.into()));
                            out.push(to("FlexWrap", wrap.into()));
                        }
                        Some(m @ ("GRID" | "FLOW" | "ABSOLUTE")) if w.len() == 2 => {
                            let value = match m {
                                "GRID" => "Grid",
                                "FLOW" => "Flow",
                                _ => "Absolute",
                            };
                            out.push(to("LayoutMode", value.into()));
                        }
                        _ => return Err(fail("MODE is FLEX [ROW|COLUMN] [WRAP], GRID, FLOW or ABSOLUTE")),
                    }
                }
                "GAP" => match (arg(1).and_then(amount), arg(2).map(amount), w.len()) {
                    (Some(g), None, 2) => out.push(to("Gap", g)),
                    (Some(r), Some(Some(c)), 3) => {
                        out.push(to("RowGap", r));
                        out.push(to("ColumnGap", c));
                    }
                    _ => return Err(fail("GAP takes one number, or a row gap and a column gap")),
                },
                "PADDING" => {
                    let bad = || fail("PADDING takes a number, or HORIZONTAL n, or VERTICAL n");
                    match (arg(1).map(str::to_ascii_uppercase).as_deref(), arg(2), w.len()) {
                        (Some("HORIZONTAL"), Some(v), 3) => {
                            let v = amount(v).ok_or_else(bad)?;
                            out.push(to("PaddingLeft", v.clone()));
                            out.push(to("PaddingRight", v));
                        }
                        (Some("VERTICAL"), Some(v), 3) => {
                            let v = amount(v).ok_or_else(bad)?;
                            out.push(to("PaddingTop", v.clone()));
                            out.push(to("PaddingBottom", v));
                        }
                        (Some(v), None, 2) => out.push(to("Padding", amount(v).ok_or_else(bad)?)),
                        _ => return Err(bad()),
                    }
                }
                "COLUMNS" | "ROWS" => {
                    if w.len() < 2 {
                        return Err(fail("COLUMNS and ROWS take a count or a track list"));
                    }
                    let tracks = w[1..].join(" ");
                    let value = match (w.len(), count(&w[1])) {
                        (2, Some(n)) if n > 0 => format!("Repeat({n}, 1fr)"),
                        _ => tracks,
                    };
                    out.push(to(if verb == "COLUMNS" { "GridColumns" } else { "GridRows" }, value));
                }
                "ORDER" => {
                    if w.len() < 2 {
                        return Err(fail("ORDER names the children in reading order"));
                    }
                    for (i, id) in w[1..].iter().enumerate() {
                        out.push(Write { control: id.clone(), property: "Order".into(), value: (i + 1).to_string() });
                    }
                }
                "ITEM" => {
                    let Some(id) = arg(1) else { return Err(fail("ITEM names a control")) };
                    let rest = &w[2..];
                    if rest.is_empty() || rest.len() % 2 != 0 {
                        return Err(fail("ITEM takes pairs: SPAN, ROW-SPAN, GROW, MIN-WIDTH, MAX-WIDTH, MIN-HEIGHT, MAX-HEIGHT"));
                    }
                    for pair in rest.chunks(2) {
                        let property = match pair[0].to_ascii_uppercase().as_str() {
                            "SPAN" => "ColumnSpan",
                            "ROW-SPAN" => "RowSpan",
                            "GROW" => "FlexGrow",
                            "MIN-WIDTH" => "MinWidth",
                            "MAX-WIDTH" => "MaxWidth",
                            "MIN-HEIGHT" => "MinHeight",
                            "MAX-HEIGHT" => "MaxHeight",
                            _ => return Err(fail("unknown ITEM word")),
                        };
                        let value = amount(&pair[1]).ok_or_else(|| fail("the value must be a number"))?;
                        out.push(Write { control: id.to_owned(), property: property.into(), value });
                    }
                }
                _ => return Err(fail("expected MODE, GAP, PADDING, COLUMNS, ROWS, ORDER, ITEM or WHEN")),
            }
            sections.last_mut().expect("never empty").writes.extend(out);
        }
    }
    sections.retain(|s| !s.writes.is_empty() || s.breakpoint.is_some());
    Ok(sections)
}

/// Every control a script writes, once each, in order of appearance.
pub fn controls_named(sections: &[Section]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in sections.iter().flat_map(|s| &s.writes) {
        if !out.iter().any(|c| c.eq_ignore_ascii_case(&w.control)) {
            out.push(w.control.clone());
        }
    }
    out
}

/// The text that carries a script to the host: one tab-separated
/// `breakpoint, control, property, value` line per write, `*` for "every
/// breakpoint". An empty text is `Clear`.
pub fn to_lines(sections: &[Section]) -> String {
    sections
        .iter()
        .flat_map(|s| {
            let bp = s.breakpoint.clone().unwrap_or_else(|| EVERY.to_owned());
            s.writes.iter().map(move |w| format!("{bp}\t{}\t{}\t{}", w.control, w.property, w.value))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Install the lines of [`to_lines`] on `table`, first taking away `previous` —
/// what the same owner installed before. Returns what was installed, for the
/// next call to take away. Within a breakpoint the "every breakpoint" writes
/// come first, so a `WHEN` section can say otherwise. A breakpoint the table
/// does not have is skipped.
pub fn install(table: &mut [Breakpoint], previous: &[(String, Override)], lines: &str) -> Vec<(String, Override)> {
    for (bp, ov) in previous {
        if let Some(b) = table.iter_mut().find(|b| b.name.eq_ignore_ascii_case(bp)) {
            if let Some(at) = b.overrides.iter().position(|o| o == ov) {
                b.overrides.remove(at);
            }
        }
    }
    let parsed: Vec<(&str, Override)> = lines
        .lines()
        .filter_map(|l| {
            let mut f = l.splitn(4, '\t');
            let (bp, control, property, value) = (f.next()?, f.next()?, f.next()?, f.next()?);
            Some((
                bp,
                Override { control: control.to_owned(), property: property.to_owned(), value: PropValue::String(value.to_owned()) },
            ))
        })
        .collect();
    let mut installed = Vec::new();
    for b in table.iter_mut() {
        let every = parsed.iter().filter(|(bp, _)| *bp == EVERY);
        let named = parsed.iter().filter(|(bp, _)| bp.eq_ignore_ascii_case(&b.name));
        for (_, ov) in every.chain(named) {
            b.overrides.push(ov.clone());
            installed.push((b.name.clone(), ov.clone()));
        }
    }
    installed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::defaults::default_breakpoints;

    fn writes(sections: &[Section], bp: Option<&str>) -> Vec<(String, String, String)> {
        sections
            .iter()
            .filter(|s| s.breakpoint.as_deref() == bp)
            .flat_map(|s| s.writes.iter().map(|w| (w.control.clone(), w.property.clone(), w.value.clone())))
            .collect()
    }

    fn t(c: &str, p: &str, v: &str) -> (String, String, String) {
        (c.into(), p.into(), v.into())
    }

    #[test]
    fn every_statement_lowers_to_the_properties_the_solver_reads() {
        let s = parse(
            "PNL",
            "MODE FLEX ROW WRAP\nGAP 14\nGAP 4 8\nPADDING HORIZONTAL 24; PADDING VERTICAL 6\nPADDING 2\n\
             COLUMNS 3\nROWS auto 1fr\nORDER A B\nITEM A SPAN 2 grow 1.5 MIN-WIDTH 160",
        )
        .unwrap();
        let base = writes(&s, None);
        assert_eq!(&base[..3], &[t("PNL", "LayoutMode", "Flex"), t("PNL", "FlexDirection", "Row"), t("PNL", "FlexWrap", "Wrap")]);
        assert!(base.contains(&t("PNL", "Gap", "14")));
        assert!(base.contains(&t("PNL", "RowGap", "4")) && base.contains(&t("PNL", "ColumnGap", "8")));
        assert!(base.contains(&t("PNL", "PaddingLeft", "24")) && base.contains(&t("PNL", "PaddingRight", "24")));
        assert!(base.contains(&t("PNL", "PaddingTop", "6")) && base.contains(&t("PNL", "Padding", "2")));
        assert!(base.contains(&t("PNL", "GridColumns", "Repeat(3, 1fr)")), "a bare count is that many equal columns");
        assert!(base.contains(&t("PNL", "GridRows", "auto 1fr")), "a track list is kept as written");
        assert!(base.contains(&t("A", "Order", "1")) && base.contains(&t("B", "Order", "2")));
        assert!(base.contains(&t("A", "ColumnSpan", "2")) && base.contains(&t("A", "FlexGrow", "1.5")));
        assert!(base.contains(&t("A", "MinWidth", "160")));
        assert_eq!(controls_named(&s), ["PNL", "A", "B"]);
    }

    #[test]
    fn a_when_section_runs_to_the_next_when() {
        let s = parse("PNL", "GAP 14\nWHEN COMPACT\nMODE FLEX COLUMN\nWHEN Medium\nGAP 10").unwrap();
        assert_eq!(writes(&s, None), [t("PNL", "Gap", "14")]);
        assert_eq!(writes(&s, Some("COMPACT")).len(), 3);
        assert_eq!(writes(&s, Some("Medium")), [t("PNL", "Gap", "10")]);
    }

    #[test]
    fn a_bad_line_is_named_with_its_number() {
        for (script, line) in [
            ("GAP 4\nSPIN 1", 2),
            ("MODE STACK", 1),
            ("GAP -3", 1),
            ("GAP", 1),
            ("PADDING HORIZONTAL", 1),
            ("ITEM A SPAN", 1),
            ("ITEM A SPAN x", 1),
            ("ITEM A FLY 2", 1),
            ("WHEN", 1),
            ("ORDER", 1),
            ("COLUMNS", 1),
            ("\n\nMODE FLEX SIDEWAYS", 3),
        ] {
            let e = parse("P", script).unwrap_err();
            assert_eq!(e.line, line, "{script:?}: {e}");
        }
    }

    #[test]
    fn installing_puts_the_every_writes_first_in_each_breakpoint_and_takes_them_away_again() {
        let s = parse("PNL", "GAP 14\nWHEN Compact\nGAP 8").unwrap();
        let mut table = default_breakpoints();
        let first = install(&mut table, &[], &to_lines(&s));
        let gaps = |b: &Breakpoint| b.overrides.iter().map(|o| o.value.to_xml_string()).collect::<Vec<_>>();
        assert_eq!(gaps(&table[0]), ["14", "8"], "Compact: the general gap, then its own, which wins");
        assert_eq!(gaps(&table[1]), ["14"]);
        assert_eq!(gaps(&table[2]), ["14"]);
        // A second Apply replaces the first; a Clear leaves the table as it was.
        let second = install(&mut table, &first, &to_lines(&parse("PNL", "GAP 20").unwrap()));
        assert_eq!(gaps(&table[0]), ["20"]);
        install(&mut table, &second, "");
        assert!(table.iter().all(|b| b.overrides.is_empty()));
        // A breakpoint the table lacks installs nothing.
        let none = install(&mut table, &[], "Phone\tPNL\tGap\t1");
        assert!(none.is_empty());
    }

    /// AC17: the solver itself, at the default breakpoints' edges. The panel is
    /// designed as a row; the script makes it a column in Compact only.
    #[test]
    fn the_solver_follows_the_script_at_the_breakpoint_edges() {
        use crate::layout::{solve, LayoutInput};
        use crate::model::{Control, ControlType, Rect};
        use std::collections::BTreeMap;

        let mut pnl = Control::new("PNL", ControlType::Panel, 0, 0);
        pnl.rect = Rect::new(0, 0, 400, 300);
        let kid = |id: &str| {
            let mut c = Control::new(id, ControlType::Button, 0, 0);
            c.rect = Rect::new(0, 0, 100, 40);
            c.parent = Some("PNL".into());
            c
        };
        let controls = vec![pnl, kid("A"), kid("B")];
        let sections = parse("PNL", "MODE FLEX ROW\nGAP 10\nWHEN Compact\nMODE FLEX COLUMN").unwrap();
        let mut table = default_breakpoints();
        install(&mut table, &[], &to_lines(&sections));
        let bag = BTreeMap::new();
        let mut cases = Vec::new();
        for (width, want_column, name) in
            [(599.0, true, "Compact"), (600.0, false, "Medium"), (1023.0, false, "Medium"), (1024.0, false, "Expanded")]
        {
            let input = LayoutInput::new(&controls, (1200.0, 700.0), (width, 700.0), &bag, &table);
            let out = solve(&input);
            let (a, b) = (out.rects["A"], out.rects["B"]);
            assert_eq!(out.breakpoint, name, "at {width}");
            if want_column {
                assert_eq!(a.x, b.x, "at {width}: one column");
                assert!(b.y >= a.y + a.h + 10.0 - 0.5, "at {width}: B under A with the gap: {a:?} {b:?}");
            } else {
                assert_eq!(a.y, b.y, "at {width}: one row");
                assert!(b.x >= a.x + a.w + 10.0 - 0.5, "at {width}: B beside A with the gap: {a:?} {b:?}");
            }
            cases.push(format!("{width}={name}:{}", if want_column { "column" } else { "row" }));
        }
        println!("layout script, solver geometry: {} widths — {}", cases.len(), cases.join(", "));
    }

    #[test]
    fn what_another_owner_installed_is_left_alone() {
        let mut table = default_breakpoints();
        let a = install(&mut table, &[], "*\tA\tGap\t1");
        let b = install(&mut table, &[], "*\tB\tGap\t2");
        install(&mut table, &a, "");
        assert_eq!(table[0].overrides.len(), 1);
        assert_eq!(table[0].overrides[0].control, "B");
        let _ = b;
    }
}
