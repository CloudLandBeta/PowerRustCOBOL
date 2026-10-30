// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The **canonical COBOL text** of a record description — what an indexed
//! file carries inside itself so that any program can read the layout of its
//! records without the `FD` that wrote them.
//!
//! The text is ordinary free-form COBOL: an `FD` paragraph followed by its
//! `01` records, one data description per line, nesting shown by four spaces
//! of indentation. Nothing in it is specific to PowerRustCOBOL — a COBOL
//! reader, or a person, takes it as the `FD` it is. Every clause the record's
//! layout and meaning depend on is written, in a fixed order, so that the same
//! `FD` always produces the same bytes:
//!
//! ```text
//! level  name | FILLER  [REDEFINES x]  [PIC p]  [usage]  [SIGN IS …]
//!        [OCCURS …]  [JUSTIFIED RIGHT]  [BLANK WHEN ZERO]  [VALUE v]
//!        [BYTE-LENGTH n] .
//! ```
//!
//! Level numbers are two digits (`01`, `05`, `88`). Condition names (`88`) and
//! regroupings (`66 … RENAMES`) are written too. `GLOBAL` and `EXTERNAL` are
//! not: they say where a *program* may see the item, not what the *record*
//! is.
//!
//! **Where the bytes are.** A description alone says what an item *is*; where
//! it sits in the record is a second question, and one a reader should not have
//! to answer by re-deriving a vendor's storage rules (a `COMP-3` item, for one,
//! does not take the bytes a packed field would elsewhere). So a file that knows
//! the layout it uses writes each item's extent beside it, in a comment —
//! `*> @6+30`, from byte 6 for 30 bytes, counting from 0. For an item with
//! `OCCURS` the extent covers every occurrence; a `REDEFINES` item shows the
//! bytes it shares. The comment is the authority on position; the clauses are
//! the authority on meaning.
//!
//! The text parses back, through the ordinary parser, to the descriptions it
//! was made from — the tests of `cobolt-runtime` hold that to every clause.

use crate::data::{
    ConditionValue, DataDecl, FileDescription, OccursClause, PicKind, RecordSizing, Usage,
};
use crate::expr::{FigurativeConstant, Literal};

/// The spaces one level of nesting is indented by.
const INDENT: &str = "    ";

/// Where each item sits in its record: `(offset, length)` in bytes, one entry
/// per data description, in the order the text lists them (a record, then
/// everything under it, depth first). `None` for an item that holds no bytes of
/// its own — a condition name, a regrouping.
pub type Extents = Vec<Option<(usize, usize)>>;

/// The canonical text of one file description: its `FD` paragraph and every
/// `01` record description, ending with a newline.
pub fn render_file_description(fd: &FileDescription) -> String {
    render_file_description_with(fd, &[])
}

/// [`render_file_description`], with each item's extent written beside it.
/// `extents[n]` belongs to record `n` of the file; a record with no entry is
/// written without extents.
pub fn render_file_description_with(fd: &FileDescription, extents: &[Extents]) -> String {
    let mut out = String::new();
    out.push_str("FD ");
    out.push_str(&fd.name);
    if let Some(clause) = fd.record_sizing.as_ref().and_then(record_clause) {
        out.push('\n');
        out.push_str(INDENT);
        out.push_str(&clause);
    }
    out.push_str(".\n");
    for (n, record) in fd.records.iter().enumerate() {
        let mut cursor = extents.get(n).map(|list| Cursor { list, next: 0 });
        render_node(record, 0, &mut out, &mut cursor);
    }
    out
}

/// Walks an [`Extents`] list in step with the tree being written.
struct Cursor<'a> {
    list: &'a [Option<(usize, usize)>],
    next: usize,
}

impl Cursor<'_> {
    fn take(&mut self) -> Option<(usize, usize)> {
        let at = self.next;
        self.next += 1;
        self.list.get(at).copied().flatten()
    }
}

/// The `RECORD` clause of an FD, in the spelling the parser reads back.
fn record_clause(rs: &RecordSizing) -> Option<String> {
    if !rs.varying {
        let n = rs.max.or(rs.min)?;
        return Some(format!("RECORD CONTAINS {n} CHARACTERS"));
    }
    if rs.depending_on.is_none() {
        if let (Some(min), Some(max)) = (rs.min, rs.max) {
            return Some(format!("RECORD CONTAINS {min} TO {max} CHARACTERS"));
        }
    }
    let mut s = String::from("RECORD VARYING");
    if let Some(min) = rs.min {
        s.push_str(&format!(" FROM {min}"));
    }
    if let Some(max) = rs.max {
        s.push_str(&format!(" TO {max}"));
    }
    s.push_str(" CHARACTERS");
    if let Some(dep) = &rs.depending_on {
        s.push_str(&format!(" DEPENDING ON {dep}"));
    }
    Some(s)
}

/// Append the description of `decl` and, below it and indented one more step,
/// every item it groups.
pub fn render_data_decl(decl: &DataDecl, depth: usize, out: &mut String) {
    render_node(decl, depth, out, &mut None);
}

fn render_node(decl: &DataDecl, depth: usize, out: &mut String, extents: &mut Option<Cursor<'_>>) {
    let extent = extents.as_mut().and_then(Cursor::take);
    for _ in 0..depth {
        out.push_str(INDENT);
    }
    out.push_str(&format!("{:02} ", decl.level));
    match &decl.name {
        Some(n) => out.push_str(n),
        None => out.push_str("FILLER"),
    }

    if let Some(r) = &decl.renames {
        out.push_str(&format!(" RENAMES {}", r.from));
        if let Some(t) = &r.thru {
            out.push_str(&format!(" THRU {t}"));
        }
        end_line(out, extent);
        return;
    }
    if decl.level == 88 {
        out.push_str(" VALUE");
        for v in &decl.condition_values {
            out.push(' ');
            match v {
                ConditionValue::Single(l) => out.push_str(&render_literal(l)),
                ConditionValue::Range(a, b) => {
                    out.push_str(&render_literal(a));
                    out.push_str(" THRU ");
                    out.push_str(&render_literal(b));
                }
            }
        }
        end_line(out, extent);
        return;
    }

    if let Some(target) = &decl.redefines {
        out.push_str(&format!(" REDEFINES {target}"));
    }
    if let Some(pic) = &decl.picture {
        out.push_str(&format!(" PIC {}", pic.template));
    }
    if let Some(u) = usage_clause(decl) {
        out.push(' ');
        out.push_str(&u);
    }
    if let Some(sign) = &decl.sign {
        out.push_str(if sign.leading { " SIGN IS LEADING" } else { " SIGN IS TRAILING" });
        if sign.separate {
            out.push_str(" SEPARATE CHARACTER");
        }
    }
    if let Some(occurs) = &decl.occurs {
        out.push(' ');
        out.push_str(&occurs_clause(occurs));
    }
    if decl.justified {
        out.push_str(" JUSTIFIED RIGHT");
    }
    if decl.blank_when_zero {
        out.push_str(" BLANK WHEN ZERO");
    }
    if let Some(v) = &decl.value {
        out.push_str(&format!(" VALUE {}", render_literal(v)));
    }
    if let Some(n) = decl.byte_length {
        out.push_str(&format!(" BYTE-LENGTH {n}"));
    }
    end_line(out, extent);

    for child in &decl.children {
        render_node(child, depth + 1, out, extents);
    }
}

/// The period that ends a description, then its extent as a comment, then the
/// line break.
fn end_line(out: &mut String, extent: Option<(usize, usize)>) {
    out.push('.');
    if let Some((offset, len)) = extent {
        out.push_str(&format!(" *> @{offset}+{len}"));
    }
    out.push('\n');
}

/// The USAGE of an item, or `None` when it is the default — or when its
/// PICTURE already says it (`PIC N` is national, `PIC U` UTF-8).
fn usage_clause(decl: &DataDecl) -> Option<String> {
    let implied = matches!(
        (decl.usage, decl.picture.as_ref().map(|p| p.kind)),
        (Usage::National, Some(PicKind::National)) | (Usage::Utf8, Some(PicKind::Utf8))
    );
    if implied {
        return None;
    }
    Some(
        match decl.usage {
            Usage::Display => return None,
            Usage::Binary => "BINARY".to_owned(),
            Usage::Comp => "COMP".to_owned(),
            Usage::Comp1 => "COMP-1".to_owned(),
            Usage::Comp2 => "COMP-2".to_owned(),
            Usage::Comp3 => "COMP-3".to_owned(),
            Usage::Comp5 => "COMP-5".to_owned(),
            Usage::PackedDecimal => "PACKED-DECIMAL".to_owned(),
            Usage::Index => "INDEX".to_owned(),
            Usage::Pointer => "POINTER".to_owned(),
            Usage::National => "NATIONAL".to_owned(),
            Usage::Utf8 => "UTF-8".to_owned(),
            Usage::ObjectReference => match &decl.object_class {
                Some(c) => format!("OBJECT REFERENCE {c}"),
                None => "OBJECT REFERENCE".to_owned(),
            },
        },
    )
}

/// `OCCURS n TIMES` or `OCCURS min TO max TIMES`, then its `DEPENDING ON`,
/// `ASCENDING`/`DESCENDING KEY` and `INDEXED BY` phrases.
fn occurs_clause(o: &OccursClause) -> String {
    let mut s = String::from("OCCURS ");
    if o.min > 0 || o.depending_on.is_some() {
        s.push_str(&format!("{} TO {} TIMES", o.min, o.max));
    } else {
        s.push_str(&format!("{} TIMES", o.max));
    }
    if let Some(d) = &o.depending_on {
        s.push_str(&format!(" DEPENDING ON {d}"));
    }
    // Keys in their declared order; a run of one direction shares one phrase.
    let mut i = 0;
    while i < o.keys.len() {
        let ascending = o.keys[i].1;
        s.push_str(if ascending { " ASCENDING KEY IS" } else { " DESCENDING KEY IS" });
        while i < o.keys.len() && o.keys[i].1 == ascending {
            s.push(' ');
            s.push_str(&o.keys[i].0);
            i += 1;
        }
    }
    if !o.indexed_by.is_empty() {
        s.push_str(" INDEXED BY");
        for n in &o.indexed_by {
            s.push(' ');
            s.push_str(n);
        }
    }
    s
}

/// A literal as COBOL source: a quoted string (a quote inside it doubled), a
/// number with the digits it was written with, or a figurative constant.
pub fn render_literal(l: &Literal) -> String {
    match l {
        Literal::String(s) => quoted("", s),
        Literal::National(s) => quoted("N", s),
        Literal::Utf8(s) => quoted("U", s),
        Literal::Integer(n) => n.to_string(),
        Literal::IntegerDigits(n, digits) => {
            let sign = if *n < 0 { "-" } else { "" };
            format!("{sign}{:0>width$}", n.unsigned_abs(), width = *digits as usize)
        }
        Literal::Float(f) => format!("{f}"),
        Literal::Decimal(mantissa, scale) => {
            let sign = if *mantissa < 0 { "-" } else { "" };
            let digits = mantissa.unsigned_abs().to_string();
            let scale = *scale as usize;
            if scale == 0 {
                return format!("{sign}{digits}");
            }
            let padded = format!("{digits:0>width$}", width = scale + 1);
            let (int, frac) = padded.split_at(padded.len() - scale);
            format!("{sign}{int}.{frac}")
        }
        Literal::Figurative(f) => match f {
            FigurativeConstant::Zero => "ZERO".to_owned(),
            FigurativeConstant::Space => "SPACE".to_owned(),
            FigurativeConstant::HighValue => "HIGH-VALUE".to_owned(),
            FigurativeConstant::LowValue => "LOW-VALUE".to_owned(),
            FigurativeConstant::Quote => "QUOTE".to_owned(),
            FigurativeConstant::Null => "NULL".to_owned(),
            FigurativeConstant::All(inner) => format!("ALL {}", render_literal(inner)),
        },
    }
}

fn quoted(prefix: &str, s: &str) -> String {
    format!("{prefix}\"{}\"", s.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{PicClause, RenamesClause, SignClause};
    use cobolt_lexer::Span;

    fn decl(level: u8, name: Option<&str>) -> DataDecl {
        DataDecl {
            level,
            name: name.map(str::to_owned),
            picture: None,
            value: None,
            usage: Usage::Display,
            object_class: None,
            occurs: None,
            redefines: None,
            renames: None,
            condition_values: Vec::new(),
            is_global: false,
            is_external: false,
            blank_when_zero: false,
            children: Vec::new(),
            span: Span::new(0, 0, 1, 1),
            justified: false,
            sign: None,
            byte_length: None,
            format: None,
        }
    }

    fn pic(template: &str, kind: PicKind, digits: u16, decimals: u16) -> PicClause {
        PicClause { template: template.to_owned(), kind, digits, decimals, span: Span::new(0, 0, 1, 1) }
    }

    fn text(d: &DataDecl) -> String {
        let mut s = String::new();
        render_data_decl(d, 0, &mut s);
        s
    }

    #[test]
    fn an_elementary_item_names_its_picture_and_its_usage() {
        let mut d = decl(5, Some("BALANCE"));
        d.picture = Some(pic("S9(7)V99", PicKind::Numeric, 7, 2));
        d.usage = Usage::Comp3;
        assert_eq!(text(&d), "05 BALANCE PIC S9(7)V99 COMP-3.\n");
    }

    #[test]
    fn display_is_the_default_and_is_not_written() {
        let mut d = decl(5, Some("NAME"));
        d.picture = Some(pic("X(30)", PicKind::Alphanumeric, 30, 0));
        assert_eq!(text(&d), "05 NAME PIC X(30).\n");
    }

    #[test]
    fn a_filler_has_no_name_and_a_redefines_names_its_target() {
        let mut f = decl(5, None);
        f.picture = Some(pic("X(3)", PicKind::Alphanumeric, 3, 0));
        assert_eq!(text(&f), "05 FILLER PIC X(3).\n");
        let mut r = decl(5, Some("BAL-TEXT"));
        r.redefines = Some("BALANCE".into());
        r.picture = Some(pic("X(5)", PicKind::Alphanumeric, 5, 0));
        assert_eq!(text(&r), "05 BAL-TEXT REDEFINES BALANCE PIC X(5).\n");
    }

    #[test]
    fn a_group_is_followed_by_its_members_one_step_in() {
        let mut leaf = decl(5, Some("ID"));
        leaf.picture = Some(pic("9(4)", PicKind::Numeric, 4, 0));
        let mut sub = decl(5, Some("DETAIL"));
        let mut inner = decl(10, Some("QTY"));
        inner.picture = Some(pic("9(3)", PicKind::Numeric, 3, 0));
        sub.children.push(inner);
        let mut rec = decl(1, Some("REC"));
        rec.children = vec![leaf, sub];
        assert_eq!(
            text(&rec),
            "01 REC.\n    05 ID PIC 9(4).\n    05 DETAIL.\n        10 QTY PIC 9(3).\n"
        );
    }

    #[test]
    fn occurs_depending_sign_justified_blank_and_value_are_written_in_one_order() {
        let mut d = decl(5, Some("LINE-AMT"));
        d.picture = Some(pic("S9(5)", PicKind::Numeric, 5, 0));
        d.sign = Some(SignClause { leading: true, separate: true });
        d.occurs = Some(OccursClause {
            min: 1,
            max: 20,
            depending_on: Some("LINE-COUNT".into()),
            indexed_by: vec!["LX".into()],
            keys: vec![("A".into(), true), ("B".into(), true), ("C".into(), false)],
            span: Span::new(0, 0, 1, 1),
        });
        d.justified = true;
        d.blank_when_zero = true;
        d.value = Some(Literal::Integer(0));
        assert_eq!(
            text(&d),
            "05 LINE-AMT PIC S9(5) SIGN IS LEADING SEPARATE CHARACTER \
             OCCURS 1 TO 20 TIMES DEPENDING ON LINE-COUNT ASCENDING KEY IS A B \
             DESCENDING KEY IS C INDEXED BY LX JUSTIFIED RIGHT BLANK WHEN ZERO VALUE 0.\n"
        );
    }

    #[test]
    fn a_fixed_occurs_is_written_the_short_way() {
        let mut d = decl(5, Some("MONTH"));
        d.picture = Some(pic("9(4)", PicKind::Numeric, 4, 0));
        d.occurs = Some(OccursClause {
            min: 0,
            max: 12,
            depending_on: None,
            indexed_by: Vec::new(),
            keys: Vec::new(),
            span: Span::new(0, 0, 1, 1),
        });
        assert_eq!(text(&d), "05 MONTH PIC 9(4) OCCURS 12 TIMES.\n");
    }

    #[test]
    fn condition_names_and_regroupings_are_written_with_their_values() {
        let mut c = decl(88, Some("ACTIVE"));
        c.condition_values = vec![
            ConditionValue::Single(Literal::String("A".into())),
            ConditionValue::Range(Literal::Integer(1), Literal::Integer(9)),
        ];
        assert_eq!(text(&c), "88 ACTIVE VALUE \"A\" 1 THRU 9.\n");
        let mut r = decl(66, Some("WHOLE"));
        r.renames = Some(RenamesClause { from: "A".into(), thru: Some("C".into()) });
        assert_eq!(text(&r), "66 WHOLE RENAMES A THRU C.\n");
    }

    #[test]
    fn national_and_utf8_pictures_imply_their_usage() {
        let mut n = decl(5, Some("WORD"));
        n.picture = Some(pic("N(5)", PicKind::National, 5, 0));
        n.usage = Usage::National;
        assert_eq!(text(&n), "05 WORD PIC N(5).\n");
        let mut u = decl(5, Some("TAG"));
        u.picture = Some(pic("U(4)", PicKind::Utf8, 4, 0));
        u.usage = Usage::Utf8;
        u.byte_length = Some(9);
        assert_eq!(text(&u), "05 TAG PIC U(4) BYTE-LENGTH 9.\n");
    }

    #[test]
    fn literals_keep_the_digits_they_were_written_with() {
        assert_eq!(render_literal(&Literal::String("say \"hi\"".into())), "\"say \"\"hi\"\"\"");
        assert_eq!(render_literal(&Literal::IntegerDigits(-7, 4)), "-0007");
        assert_eq!(render_literal(&Literal::Decimal(314, 2)), "3.14");
        assert_eq!(render_literal(&Literal::Decimal(-5, 3)), "-0.005");
        assert_eq!(render_literal(&Literal::Decimal(12, 0)), "12");
        assert_eq!(render_literal(&Literal::Figurative(FigurativeConstant::Space)), "SPACE");
        assert_eq!(
            render_literal(&Literal::Figurative(FigurativeConstant::All(Box::new(Literal::String("*".into()))))),
            "ALL \"*\""
        );
        assert_eq!(render_literal(&Literal::National("ação".into())), "N\"ação\"");
    }

    #[test]
    fn an_item_can_carry_where_its_bytes_are() {
        let mut a = decl(5, Some("ID"));
        a.picture = Some(pic("9(4)", PicKind::Numeric, 4, 0));
        let mut b = decl(5, Some("NAME"));
        b.picture = Some(pic("X(10)", PicKind::Alphanumeric, 10, 0));
        let mut c = decl(88, Some("ZERO-ID"));
        c.condition_values = vec![ConditionValue::Single(Literal::Integer(0))];
        a.children.push(c);
        let mut rec = decl(1, Some("REC"));
        rec.children = vec![a, b];
        let fd = FileDescription {
            name: "F".into(),
            is_global: false,
            records: vec![rec],
            linage: None,
            record_sizing: None,
            is_external: false,
            span: Span::new(0, 0, 1, 1),
        };
        // record, ID, ZERO-ID (no bytes), NAME — in listing order.
        let extents = vec![vec![Some((0, 14)), Some((0, 4)), None, Some((4, 10))]];
        assert_eq!(
            render_file_description_with(&fd, &extents),
            "FD F.\n01 REC. *> @0+14\n    05 ID PIC 9(4). *> @0+4\n        88 ZERO-ID VALUE 0.\n    05 NAME PIC X(10). *> @4+10\n"
        );
        // …and without extents the text is what it always was.
        assert_eq!(
            render_file_description(&fd),
            "FD F.\n01 REC.\n    05 ID PIC 9(4).\n        88 ZERO-ID VALUE 0.\n    05 NAME PIC X(10).\n"
        );
    }

    #[test]
    fn the_fd_paragraph_carries_the_record_clause() {
        let mut rec = decl(1, Some("R"));
        rec.picture = Some(pic("X(80)", PicKind::Alphanumeric, 80, 0));
        let fd = |rs: Option<RecordSizing>| FileDescription {
            name: "CUST-FILE".into(),
            is_global: false,
            records: vec![rec.clone()],
            linage: None,
            record_sizing: rs,
            is_external: false,
            span: Span::new(0, 0, 1, 1),
        };
        assert_eq!(render_file_description(&fd(None)), "FD CUST-FILE.\n01 R PIC X(80).\n");
        let fixed = RecordSizing { min: Some(80), max: Some(80), varying: false, depending_on: None };
        assert_eq!(
            render_file_description(&fd(Some(fixed))),
            "FD CUST-FILE\n    RECORD CONTAINS 80 CHARACTERS.\n01 R PIC X(80).\n"
        );
        let range = RecordSizing { min: Some(10), max: Some(80), varying: true, depending_on: None };
        assert!(render_file_description(&fd(Some(range))).contains("RECORD CONTAINS 10 TO 80 CHARACTERS"));
        let dep = RecordSizing { min: Some(10), max: Some(80), varying: true, depending_on: Some("LEN".into()) };
        assert!(render_file_description(&fd(Some(dep)))
            .contains("RECORD VARYING FROM 10 TO 80 CHARACTERS DEPENDING ON LEN"));
    }
}
