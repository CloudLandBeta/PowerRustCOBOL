// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The shape of a COBOL group, as far as the wire cares (WASM-005 R5).
//!
//! A [`Item`] tree is the **layout**: names, nesting, tables, and for each
//! elementary item the category, width and scale the JSON mapping needs. It is
//! built from a parsed `DataDecl` ([`from_decl`]) by the compiler, the
//! analyser and the backend, and from the interpreter's symbol table by the
//! interpreter; it is also what the interface description carries, so it
//! serialises.
//!
//! What the layout leaves out is deliberate. `USAGE` (COMP, COMP-3, …) is not
//! here: on the wire a number is its value, never its bytes. `VALUE` is kept
//! only as the item's initial text. Level 66 (`RENAMES`) and 88-level
//! condition-names are not data and never appear.

use cobolt_ast::data::{DataDecl, PicKind, Usage};
use cobolt_ast::expr::{FigurativeConstant, Literal};
use serde::{Deserialize, Serialize};

/// The widest numeric item the wire carries: what an `i128` mantissa holds.
pub const MAX_DIGITS: u32 = 38;

/// One data item of a layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// The data-name as declared, spelling preserved; `FILLER` when unnamed.
    pub name: String,
    /// Declared without a name (or as `FILLER`): storage, never a member.
    #[serde(default, skip_serializing_if = "is_false")]
    pub filler: bool,
    /// Declared with `REDEFINES`: it overlays the item before it and is never a
    /// member — only the redefined (first) item travels.
    #[serde(default, skip_serializing_if = "is_false")]
    pub redefines: bool,
    /// `OCCURS`: this item is a table, a JSON array.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurs: Option<Occurs>,
    pub kind: Kind,
    /// The item's `VALUE`, as the text it initialises the item to (a string is
    /// padded by the consumer; a number is already canonical).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// A table's bounds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Occurs {
    /// The fewest occurrences (equal to `max` for a fixed table).
    pub min: usize,
    /// The most occurrences, which is the storage reserved.
    pub max: usize,
    /// `DEPENDING ON` data-name: the item that holds the current count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depending_on: Option<String>,
}

/// What an item is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Kind {
    /// A group: a JSON object of its members, in declaration order.
    Group { children: Vec<Item> },
    /// `PIC X` / `PIC A`: `len` **bytes**, a JSON string in UTF-8.
    Alphanumeric {
        len: usize,
        /// `JUSTIFIED RIGHT`: a short value is padded on the left.
        #[serde(default, skip_serializing_if = "is_false")]
        justified: bool,
    },
    /// `PIC N` / `USAGE NATIONAL`: `len` UTF-16 code units.
    National { len: usize },
    /// `PIC U` / `USAGE UTF-8`: `len` characters, held in `bytes` bytes when the
    /// item was declared `BYTE-LENGTH`.
    Utf8 {
        len: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bytes: Option<usize>,
    },
    /// `PIC 9` (any `USAGE`): `digits` integer digits, `scale` after the
    /// implied point, a sign when `signed`. A JSON number, written from its
    /// decimal text — never through a binary floating-point value.
    Numeric { digits: u16, scale: u16, signed: bool },
    /// A numeric-edited or alphanumeric-edited picture: the edited image, `len`
    /// bytes, travels as a JSON string and is not re-edited on the way in.
    Edited { len: usize },
}

impl Item {
    /// Is this item a member of its group's JSON object?
    pub fn is_member(&self) -> bool {
        !self.filler && !self.redefines
    }

    /// The members of a group, in declaration order; empty for anything else.
    pub fn members(&self) -> impl Iterator<Item = &Item> {
        let children: &[Item] = match &self.kind {
            Kind::Group { children } => children,
            _ => &[],
        };
        children.iter().filter(|c| c.is_member())
    }

    pub fn children(&self) -> &[Item] {
        match &self.kind {
            Kind::Group { children } => children,
            _ => &[],
        }
    }

    pub fn is_group(&self) -> bool {
        matches!(self.kind, Kind::Group { .. })
    }
}

/// Why a declaration cannot travel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutError {
    /// The dotted path of the item, e.g. `ORDER.LINES`.
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for LayoutError {}

// ── building a layout from a declaration ──────────────────────────────────────

/// The layout of a parsed `01`/`77` item (or any group or elementary item).
///
/// Fails, naming the item, on what has no wire form: `USAGE COMP-1/COMP-2`
/// (binary floating point has no decimal scale), `INDEX`, `POINTER`,
/// `OBJECT REFERENCE`, a picture wider than [`MAX_DIGITS`] digits, a `P`
/// scaling position, a duplicate member name, or a `DEPENDING ON` item the
/// group does not hold. A subtree that is only a `REDEFINES` overlay is not
/// inspected: it never travels.
pub fn from_decl(decl: &DataDecl) -> Result<Item, LayoutError> {
    let mut path = Vec::new();
    let item = build(decl, &mut path, false)?;
    validate(&item)?;
    Ok(item)
}

fn name_of(decl: &DataDecl) -> (String, bool) {
    match decl.name.as_deref() {
        Some(n) if !n.eq_ignore_ascii_case("FILLER") => (n.to_string(), false),
        _ => ("FILLER".to_string(), true),
    }
}

fn err(path: &[String], message: impl Into<String>) -> LayoutError {
    LayoutError { path: path.join("."), message: message.into() }
}

fn build(decl: &DataDecl, path: &mut Vec<String>, overlay: bool) -> Result<Item, LayoutError> {
    let (name, filler) = name_of(decl);
    path.push(name.clone());
    let redefines = decl.redefines.is_some();
    // Inside a REDEFINES overlay nothing is inspected: it is never sent.
    let overlay = overlay || redefines;
    let result = build_inner(decl, path, overlay, name, filler, redefines);
    path.pop();
    result
}

fn build_inner(
    decl: &DataDecl,
    path: &mut Vec<String>,
    overlay: bool,
    name: String,
    filler: bool,
    redefines: bool,
) -> Result<Item, LayoutError> {
    let occurs = decl.occurs.as_ref().map(|o| {
        let max = o.max as usize;
        let depending_on = o.depending_on.clone();
        let min = if depending_on.is_some() { o.min as usize } else { max };
        Occurs { min, max, depending_on }
    });
    let data_children: Vec<&DataDecl> = decl.children.iter().filter(|c| c.level != 88 && c.level != 66).collect();

    let kind = if !data_children.is_empty() {
        let mut children = Vec::with_capacity(data_children.len());
        for c in data_children {
            children.push(build(c, path, overlay)?);
        }
        Kind::Group { children }
    } else if let Some(pic) = &decl.picture {
        match decl.usage {
            Usage::Comp1 | Usage::Comp2 if !overlay => {
                return Err(err(path, "USAGE COMP-1/COMP-2 is binary floating point: it has no decimal scale, so it cannot travel as a JSON number without losing exactness"))
            }
            Usage::Index | Usage::Pointer | Usage::ObjectReference if !overlay => {
                return Err(err(path, "an INDEX, POINTER or OBJECT REFERENCE item has no wire form"))
            }
            _ => {}
        }
        match pic.kind {
            PicKind::Alphabetic | PicKind::Alphanumeric => {
                Kind::Alphanumeric { len: pic.digits as usize, justified: decl_justified(decl) }
            }
            PicKind::Numeric => {
                let upper = pic.template.to_ascii_uppercase();
                if upper.contains('P') && !overlay {
                    return Err(err(path, "a PICTURE with P scaling positions has no wire form"));
                }
                if pic.digits as u32 + pic.decimals as u32 > MAX_DIGITS && !overlay {
                    return Err(err(path, format!("a numeric item of {} digits is wider than the {MAX_DIGITS} the wire carries", pic.digits as u32 + pic.decimals as u32)));
                }
                Kind::Numeric { digits: pic.digits, scale: pic.decimals, signed: upper.contains('S') }
            }
            PicKind::NumericEdited | PicKind::AlphanumericEdited => Kind::Edited { len: edited_width(&pic.template) },
            PicKind::National => Kind::National { len: pic.digits as usize },
            PicKind::Utf8 => Kind::Utf8 { len: pic.digits as usize, bytes: decl.byte_length.map(|b| b as usize) },
        }
    } else if overlay {
        // A pictureless item inside an overlay: nothing to inspect.
        Kind::Group { children: Vec::new() }
    } else {
        match decl.usage {
            Usage::Index | Usage::Pointer | Usage::ObjectReference => {
                return Err(err(path, "an INDEX, POINTER or OBJECT REFERENCE item has no wire form"))
            }
            Usage::Comp1 | Usage::Comp2 => {
                return Err(err(path, "USAGE COMP-1/COMP-2 is binary floating point: it has no decimal scale, so it cannot travel as a JSON number without losing exactness"))
            }
            _ => return Err(err(path, "an elementary item without a PICTURE has no wire form")),
        }
    };

    let value = if overlay { None } else { initial_text(decl, &kind) };
    Ok(Item { name, filler, redefines, occurs, kind, value })
}

fn decl_justified(decl: &DataDecl) -> bool {
    decl.justified
}

/// The text a `VALUE` clause initialises an item to, in the form the mapping
/// keeps it: canonical decimal text for a number, the characters for a string.
fn initial_text(decl: &DataDecl, kind: &Kind) -> Option<String> {
    let lit = decl.value.as_ref()?;
    match (kind, lit) {
        (Kind::Numeric { digits, scale, signed }, Literal::Integer(n)) => {
            canonical_number(&n.to_string(), *digits, *scale, *signed)
        }
        (Kind::Numeric { digits, scale, signed }, Literal::IntegerDigits(n, _)) => {
            canonical_number(&n.to_string(), *digits, *scale, *signed)
        }
        (Kind::Numeric { digits, scale, signed }, Literal::Decimal(m, s)) => {
            let neg = *m < 0;
            let digits_txt = m.unsigned_abs().to_string();
            let s = *s as usize;
            let padded = if digits_txt.len() <= s { format!("{}{}", "0".repeat(s + 1 - digits_txt.len()), digits_txt) } else { digits_txt };
            let split = padded.len() - s;
            let text = if s == 0 { padded } else { format!("{}.{}", &padded[..split], &padded[split..]) };
            canonical_number(&format!("{}{}", if neg { "-" } else { "" }, text), *digits, *scale, *signed)
        }
        (Kind::Numeric { scale, .. }, Literal::Figurative(FigurativeConstant::Zero)) => Some(zero_text(*scale)),
        (Kind::Alphanumeric { .. } | Kind::Edited { .. } | Kind::National { .. } | Kind::Utf8 { .. }, Literal::String(s)) => Some(s.clone()),
        (Kind::National { .. }, Literal::National(s)) | (Kind::Utf8 { .. }, Literal::Utf8(s)) => Some(s.clone()),
        _ => None,
    }
}

fn canonical_number(text: &str, digits: u16, scale: u16, signed: bool) -> Option<String> {
    crate::number::canonical_from_decimal(text, digits, scale, signed).ok()
}

/// The canonical text of zero at `scale`.
pub fn zero_text(scale: u16) -> String {
    if scale == 0 {
        "0".to_string()
    } else {
        format!("0.{}", "0".repeat(scale as usize))
    }
}

/// The width in bytes of an edited picture: every symbol that prints a
/// position, a repeat count `(n)` expanded; `V`, `S` and `P` print nothing.
pub fn edited_width(template: &str) -> usize {
    let t: Vec<char> = template.to_ascii_uppercase().chars().collect();
    let mut width = 0usize;
    let mut i = 0usize;
    while i < t.len() {
        let c = t[i];
        let mut count = 1usize;
        if i + 1 < t.len() && t[i + 1] == '(' {
            let mut j = i + 2;
            let mut n = 0usize;
            while j < t.len() && t[j].is_ascii_digit() {
                n = n * 10 + t[j].to_digit(10).unwrap_or(0) as usize;
                j += 1;
            }
            if j < t.len() && t[j] == ')' {
                count = n;
                i = j;
            }
        }
        let printed = match c {
            'V' | 'S' | 'P' => 0,
            'C' if t.get(i + 1) == Some(&'R') => {
                i += 1;
                2
            }
            'D' if t.get(i + 1) == Some(&'B') => {
                i += 1;
                2
            }
            _ => 1,
        };
        width += printed * count;
        i += 1;
    }
    width
}

// ── validation ────────────────────────────────────────────────────────────────

/// Check a layout can be mapped unambiguously: member names are unique within
/// their group (a JSON object cannot repeat a key, and COBOL names ignore
/// case), tables are sound, and every `DEPENDING ON` item resolves.
pub fn validate(root: &Item) -> Result<(), LayoutError> {
    let mut path = vec![root.name.clone()];
    validate_item(root, &mut path)?;
    // Every ODO item must resolve exactly as the mapping will resolve it.
    check_odo(root, &mut vec![root.name.clone()])
}

fn validate_item(item: &Item, path: &mut Vec<String>) -> Result<(), LayoutError> {
    if let Some(o) = &item.occurs {
        if o.max == 0 {
            return Err(err(path, "OCCURS must be at least 1"));
        }
        if o.min > o.max {
            return Err(err(path, "OCCURS minimum is larger than its maximum"));
        }
    }
    if let Kind::Group { children } = &item.kind {
        let mut seen: Vec<String> = Vec::new();
        for c in children.iter().filter(|c| c.is_member()) {
            let key = c.name.to_ascii_uppercase();
            if seen.contains(&key) {
                path.push(c.name.clone());
                let e = err(path, "two members of one group share a name; a JSON object cannot repeat a key");
                path.pop();
                return Err(e);
            }
            seen.push(key);
        }
        for c in children {
            path.push(c.name.clone());
            validate_item(c, path)?;
            path.pop();
        }
    }
    Ok(())
}

/// One step from a group node to a node inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// The n-th child of a group (index into `Kind::Group::children`).
    Child(usize),
    /// The n-th element of a table (zero-based).
    Elem(usize),
}

/// Find the `DEPENDING ON` item `name` for the table at `children[at]` of a
/// scope: look in the items declared **before** it, in that group and then in
/// each enclosing group, going down through groups but never into a table.
/// Returns the child-index path from the scope's group.
pub fn find_odo_object(children: &[Item], at: usize, name: &str) -> Option<Vec<usize>> {
    fn dfs(items: &[Item], name: &str, out: &mut Vec<usize>) -> bool {
        for (i, c) in items.iter().enumerate() {
            if !c.is_member() || c.occurs.is_some() {
                continue;
            }
            out.push(i);
            match &c.kind {
                Kind::Group { children } => {
                    if dfs(children, name, out) {
                        return true;
                    }
                }
                Kind::Numeric { scale: 0, .. } if c.name.eq_ignore_ascii_case(name) => return true,
                _ => {}
            }
            out.pop();
        }
        false
    }
    let mut out = Vec::new();
    if dfs(&children[..at.min(children.len())], name, &mut out) {
        Some(out)
    } else {
        None
    }
}

/// The layout item a step path leads to (table elements share their item).
pub fn item_at<'a>(root: &'a Item, steps: &[Step]) -> Option<&'a Item> {
    let mut it = root;
    for s in steps {
        if let Step::Child(i) = s {
            it = it.children().get(*i)?;
        }
    }
    Some(it)
}

fn check_odo(root: &Item, path: &mut Vec<String>) -> Result<(), LayoutError> {
    fn walk<'a>(
        item: &'a Item,
        enclosing: &mut Vec<(&'a [Item], usize)>,
        path: &mut Vec<String>,
    ) -> Result<(), LayoutError> {
        if let Kind::Group { children } = &item.kind {
            for (i, c) in children.iter().enumerate() {
                if let Some(Occurs { depending_on: Some(dep), .. }) = &c.occurs {
                    // Innermost enclosing group first, then outward.
                    let here = (children.as_slice(), i);
                    let found = std::iter::once(&here)
                        .chain(enclosing.iter().rev())
                        .any(|(kids, at)| find_odo_object(kids, *at, dep).is_some());
                    if !found {
                        path.push(c.name.clone());
                        let e = err(
                            path,
                            format!("OCCURS DEPENDING ON {dep}: no integer item of that name is declared before the table inside this argument group"),
                        );
                        path.pop();
                        return Err(e);
                    }
                }
                enclosing.push((children.as_slice(), i));
                path.push(c.name.clone());
                let r = walk(c, enclosing, path);
                path.pop();
                enclosing.pop();
                r?;
            }
        }
        Ok(())
    }
    walk(root, &mut Vec::new(), path)
}
