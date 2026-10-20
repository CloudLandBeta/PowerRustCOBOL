// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The COBOL group <-> JSON mapping (WASM-005 R5, R5b, R6).
//!
//! [`encode`] turns a group's [`Node`] tree into JSON text; [`decode`] reads
//! JSON text into a **new** tree built from the current one. The rules are
//! written down once, in `specs/WASM-005-websocket-call-and-remote-data/
//! mapping-rules.md`, and pinned by the golden vectors beside it, which the
//! browser module's generator runs as well.

use crate::json::{self, Json};
use crate::layout::{find_odo_object, Item, Kind, Step};
use crate::node::Node;
use crate::number::{self, NumberError};

/// The largest JSON payload the mapping reads unless told otherwise.
pub const DEFAULT_MAX_BYTES: usize = 16 * 1024 * 1024;

/// What went wrong, as a closed list. `code` is the stable word the golden
/// vectors use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// The text is not JSON.
    Syntax,
    /// An object repeats a key.
    DuplicateKey,
    /// A key names no member of the group (a `FILLER`, a `REDEFINES` overlay
    /// and an 88-level are not members).
    UnknownKey,
    /// A value of the wrong JSON type for its item.
    WrongType,
    /// A number that does not fit its PICTURE (too many digits, too many
    /// decimals, a negative for an unsigned item, absurdly large).
    Overflow,
    /// A string longer than its PICTURE.
    TooLong,
    /// A table longer than its `OCCURS`.
    TooManyElements,
    /// A variable table shorter than its `OCCURS` minimum.
    TooFewElements,
    /// Text that is not valid UTF-8 cannot be sent.
    NotUtf8,
    /// The count item and the array disagree.
    OdoMismatch,
    /// A count outside `OCCURS min TO max`.
    OdoOutOfRange,
    /// The value tree does not match its layout (a bug in the caller).
    Internal,
}

impl ErrorKind {
    pub fn code(self) -> &'static str {
        match self {
            ErrorKind::Syntax => "syntax",
            ErrorKind::DuplicateKey => "duplicate-key",
            ErrorKind::UnknownKey => "unknown-key",
            ErrorKind::WrongType => "wrong-type",
            ErrorKind::Overflow => "overflow",
            ErrorKind::TooLong => "too-long",
            ErrorKind::TooManyElements => "too-many-elements",
            ErrorKind::TooFewElements => "too-few-elements",
            ErrorKind::NotUtf8 => "not-utf8",
            ErrorKind::OdoMismatch => "odo-mismatch",
            ErrorKind::OdoOutOfRange => "odo-out-of-range",
            ErrorKind::Internal => "internal",
        }
    }
}

/// A failed encode or decode, naming the item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapError {
    pub kind: ErrorKind,
    /// Where: `ORDER.LINES(3).QTY` (1-based subscripts), or the root name.
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for MapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at {}: {}", self.kind.code(), self.path, self.message)
    }
}

impl std::error::Error for MapError {}

fn fail<T>(kind: ErrorKind, path: &str, message: impl Into<String>) -> Result<T, MapError> {
    Err(MapError { kind, path: path.to_string(), message: message.into() })
}

// ── encode ────────────────────────────────────────────────────────────────────

struct Frame<'a> {
    children: &'a [Item],
    nodes: &'a [Node],
    /// Index of the child being processed.
    at: usize,
}

/// The JSON text of a group (an elementary item is its bare scalar).
pub fn encode(item: &Item, node: &Node) -> Result<String, MapError> {
    let mut out = String::new();
    let mut frames: Vec<Frame<'_>> = Vec::new();
    let mut path = item.name.clone();
    enc_item(item, node, &mut out, &mut path, &mut frames)?;
    Ok(out)
}

fn enc_item<'a>(
    item: &'a Item,
    node: &'a Node,
    out: &mut String,
    path: &mut String,
    frames: &mut Vec<Frame<'a>>,
) -> Result<(), MapError> {
    let Some(occ) = &item.occurs else {
        return enc_base(item, node, out, path, frames);
    };
    let Node::Table(elems) = node else {
        return fail(ErrorKind::Internal, path, "a table item holds no table node");
    };
    if elems.len() != occ.max {
        return fail(ErrorKind::Internal, path, format!("the table node has {} elements, OCCURS says {}", elems.len(), occ.max));
    }
    let count = match &occ.depending_on {
        None => occ.max,
        Some(dep) => {
            let n = odo_value(frames, dep, path)?;
            if n < occ.min || n > occ.max {
                return fail(ErrorKind::OdoOutOfRange, path, format!("{dep} holds {n}, outside OCCURS {} TO {}", occ.min, occ.max));
            }
            n
        }
    };
    out.push('[');
    for (j, e) in elems.iter().take(count).enumerate() {
        if j > 0 {
            out.push(',');
        }
        let mark = path.len();
        path.push_str(&format!("({})", j + 1));
        enc_base(item, e, out, path, frames)?;
        path.truncate(mark);
    }
    out.push(']');
    Ok(())
}

fn enc_base<'a>(
    item: &'a Item,
    node: &'a Node,
    out: &mut String,
    path: &mut String,
    frames: &mut Vec<Frame<'a>>,
) -> Result<(), MapError> {
    match (&item.kind, node) {
        (Kind::Group { children }, Node::Group(nodes)) => {
            if nodes.len() != children.len() {
                return fail(ErrorKind::Internal, path, "the group node does not match its layout");
            }
            out.push('{');
            frames.push(Frame { children, nodes, at: 0 });
            let mut first = true;
            for (i, (c, n)) in children.iter().zip(nodes.iter()).enumerate() {
                if !c.is_member() {
                    continue;
                }
                if let Some(f) = frames.last_mut() {
                    f.at = i;
                }
                if !first {
                    out.push(',');
                }
                first = false;
                json::write_string(out, &c.name);
                out.push(':');
                let mark = path.len();
                path.push('.');
                path.push_str(&c.name);
                enc_item(c, n, out, path, frames)?;
                path.truncate(mark);
            }
            frames.pop();
            out.push('}');
            Ok(())
        }
        (Kind::Numeric { digits, scale, signed }, Node::Leaf(b)) => {
            let text = std::str::from_utf8(b).unwrap_or("");
            if !number::is_canonical(text, *digits, *scale, *signed) {
                return fail(ErrorKind::Internal, path, format!("the stored number {text:?} is not canonical for its PICTURE"));
            }
            out.push_str(text);
            Ok(())
        }
        (Kind::Group { .. }, _) | (Kind::Numeric { .. }, _) => fail(ErrorKind::Internal, path, "the node does not match its layout"),
        (_, Node::Leaf(b)) => {
            let end = b.iter().rposition(|c| *c != b' ').map_or(0, |p| p + 1);
            match std::str::from_utf8(&b[..end]) {
                Ok(s) => {
                    json::write_string(out, s);
                    Ok(())
                }
                Err(_) => fail(ErrorKind::NotUtf8, path, "the item holds bytes that are not valid UTF-8, which JSON cannot carry"),
            }
        }
        _ => fail(ErrorKind::Internal, path, "the node does not match its layout"),
    }
}

/// The current value of the `DEPENDING ON` item `dep` for the table being
/// processed in the innermost frame.
fn odo_value(frames: &[Frame<'_>], dep: &str, path: &str) -> Result<usize, MapError> {
    for f in frames.iter().rev() {
        if let Some(idxs) = find_odo_object(f.children, f.at, dep) {
            let mut nodes = f.nodes;
            for (k, i) in idxs.iter().enumerate() {
                let Some(n) = nodes.get(*i) else {
                    return fail(ErrorKind::Internal, path, "the count item is missing from the value tree");
                };
                if k + 1 == idxs.len() {
                    let Node::Leaf(b) = n else {
                        return fail(ErrorKind::Internal, path, "the count item is not elementary");
                    };
                    let text = std::str::from_utf8(b).unwrap_or("");
                    return match number::to_mantissa(text) {
                        Some(m) if m >= 0 => Ok(m as usize),
                        Some(m) => fail(ErrorKind::OdoOutOfRange, path, format!("{dep} holds {m}, a table cannot have a negative length")),
                        None => fail(ErrorKind::Internal, path, "the count item is not a number"),
                    };
                }
                match n {
                    Node::Group(inner) => nodes = inner,
                    _ => return fail(ErrorKind::Internal, path, "the count item's path does not follow the layout"),
                }
            }
        }
    }
    fail(ErrorKind::Internal, path, format!("OCCURS DEPENDING ON {dep}: no such item before the table"))
}

// ── projection ────────────────────────────────────────────────────────────────

/// Keep, in the JSON of an answer, only the members `layout` knows (WASM-014
/// R4): the backend writes its reply for the contract version the *caller* was
/// compiled against, so a member added since — which an old client's strict
/// reader would refuse — never reaches it. Members the layout does not know are
/// dropped at every depth, in groups inside tables included; everything else is
/// left exactly as it was written.
pub fn project_json(layout: &Item, json: &str, max_bytes: usize) -> Result<String, MapError> {
    let parsed = json::parse(json, max_bytes).map_err(|e| MapError { kind: ErrorKind::Syntax, path: layout.name.clone(), message: e.to_string() })?;
    Ok(project_value(layout, parsed).to_text())
}

fn project_value(item: &Item, value: Json) -> Json {
    // A table is an array of elements shaped like the item without its OCCURS.
    match value {
        Json::Array(elems) if item.occurs.is_some() => Json::Array(elems.into_iter().map(|e| project_base(item, e)).collect()),
        other => project_base(item, other),
    }
}

fn project_base(item: &Item, value: Json) -> Json {
    match (&item.kind, value) {
        (Kind::Group { .. }, Json::Object(members)) => {
            let known: Vec<&Item> = item.members().collect();
            Json::Object(
                members
                    .into_iter()
                    .filter_map(|(k, v)| known.iter().find(|m| m.name == k).map(|m| (k, project_value(m, v))))
                    .collect(),
            )
        }
        (_, other) => other,
    }
}

// ── decode ────────────────────────────────────────────────────────────────────

struct DFrame<'a> {
    children: &'a [Item],
    at: usize,
    /// Steps from the root to this frame's group node.
    steps: Vec<Step>,
}

struct OdoRec {
    table_path: String,
    object_steps: Vec<Step>,
    count: usize,
    dep: String,
}

struct Ctx {
    /// Leaves (as step paths) whose value came from the JSON text.
    supplied: Vec<Vec<Step>>,
    odo: Vec<OdoRec>,
}

/// Read `text` into a copy of `current`, by name. On any fault returns the
/// error and `current` is untouched (the caller owns it; nothing here writes
/// to it). A key absent from the text leaves its item as it was.
pub fn decode(item: &Item, current: &Node, text: &str, max_bytes: usize) -> Result<Node, MapError> {
    let parsed = json::parse(text, max_bytes).map_err(|e| {
        let kind = if e.duplicate_key.is_some() { ErrorKind::DuplicateKey } else { ErrorKind::Syntax };
        MapError { kind, path: item.name.clone(), message: e.to_string() }
    })?;
    decode_json(item, current, &parsed)
}

/// [`decode`] for text already parsed.
pub fn decode_json(item: &Item, current: &Node, parsed: &Json) -> Result<Node, MapError> {
    let mut ctx = Ctx { supplied: Vec::new(), odo: Vec::new() };
    let mut path = item.name.clone();
    let mut steps: Vec<Step> = Vec::new();
    let mut frames: Vec<DFrame<'_>> = Vec::new();
    let mut tree = dec_item(item, current, parsed, &mut path, &mut steps, &mut frames, &mut ctx)?;

    // The count of a variable-length table follows the array that was sent.
    for rec in &ctx.odo {
        let Some(obj_item) = crate::layout::item_at(item, &rec.object_steps) else {
            return fail(ErrorKind::Internal, &rec.table_path, "the count item is not in the layout");
        };
        let Kind::Numeric { digits, scale, signed } = obj_item.kind else {
            return fail(ErrorKind::Internal, &rec.table_path, "the count item is not numeric");
        };
        let Some(Node::Leaf(cur)) = tree.at(&rec.object_steps) else {
            return fail(ErrorKind::Internal, &rec.table_path, "the count item is not in the value tree");
        };
        if ctx.supplied.contains(&rec.object_steps) {
            let held = number::to_mantissa(std::str::from_utf8(cur).unwrap_or("")).unwrap_or(-1);
            if held != rec.count as i128 {
                return fail(
                    ErrorKind::OdoMismatch,
                    &rec.table_path,
                    format!("{} says {held} but the array has {} elements", rec.dep, rec.count),
                );
            }
        } else {
            let text = rec.count.to_string();
            match number::canonical_from_decimal(&text, digits, scale, signed) {
                Ok(c) => {
                    if let Some(Node::Leaf(slot)) = tree.at_mut(&rec.object_steps) {
                        *slot = c.into_bytes();
                    }
                }
                Err(_) => {
                    return fail(
                        ErrorKind::OdoOutOfRange,
                        &rec.table_path,
                        format!("{} cannot hold {} (its PICTURE is too small for the table's length)", rec.dep, rec.count),
                    )
                }
            }
        }
    }
    // Whatever count the text set directly must still be a legal length.
    check_counts(item, &tree)?;
    Ok(tree)
}

fn dec_item<'a>(
    item: &'a Item,
    cur: &Node,
    json: &Json,
    path: &mut String,
    steps: &mut Vec<Step>,
    frames: &mut Vec<DFrame<'a>>,
    ctx: &mut Ctx,
) -> Result<Node, MapError> {
    let Some(occ) = &item.occurs else {
        return dec_base(item, cur, json, path, steps, frames, ctx);
    };
    let Json::Array(arr) = json else {
        return fail(ErrorKind::WrongType, path, format!("expected an array (it is a table of {}), found {}", occ.max, kind_name(json)));
    };
    let Node::Table(cur_elems) = cur else {
        return fail(ErrorKind::Internal, path, "a table item holds no table node");
    };
    if arr.len() > occ.max {
        return fail(ErrorKind::TooManyElements, path, format!("{} elements, but OCCURS allows {}", arr.len(), occ.max));
    }
    // A fixed table may be given short (the elements not given are unchanged,
    // as an absent key leaves an item); a variable one has a floor.
    if occ.depending_on.is_some() && arr.len() < occ.min {
        return fail(ErrorKind::TooFewElements, path, format!("{} elements, but OCCURS needs at least {}", arr.len(), occ.min));
    }
    let mut elems = Vec::with_capacity(cur_elems.len());
    for (j, cur_e) in cur_elems.iter().enumerate() {
        if let Some(v) = arr.get(j) {
            let mark = path.len();
            path.push_str(&format!("({})", j + 1));
            steps.push(Step::Elem(j));
            let r = dec_base(item, cur_e, v, path, steps, frames, ctx);
            steps.pop();
            path.truncate(mark);
            elems.push(r?);
        } else {
            elems.push(cur_e.clone());
        }
    }
    if let Some(dep) = &occ.depending_on {
        // Where the count lives is a question for the layout alone.
        let mut located: Option<Vec<Step>> = None;
        for f in frames.iter().rev() {
            if let Some(idxs) = find_odo_object(f.children, f.at, dep) {
                let mut s = f.steps.clone();
                s.extend(idxs.into_iter().map(Step::Child));
                located = Some(s);
                break;
            }
        }
        let Some(object_steps) = located else {
            return fail(ErrorKind::Internal, path, format!("OCCURS DEPENDING ON {dep}: no such item before the table"));
        };
        ctx.odo.push(OdoRec { table_path: path.clone(), object_steps, count: arr.len(), dep: dep.clone() });
    }
    Ok(Node::Table(elems))
}

fn dec_base<'a>(
    item: &'a Item,
    cur: &Node,
    json: &Json,
    path: &mut String,
    steps: &mut Vec<Step>,
    frames: &mut Vec<DFrame<'a>>,
    ctx: &mut Ctx,
) -> Result<Node, MapError> {
    match (&item.kind, cur) {
        (Kind::Group { children }, Node::Group(cur_nodes)) => {
            let Json::Object(members) = json else {
                return fail(ErrorKind::WrongType, path, format!("expected an object (it is a group), found {}", kind_name(json)));
            };
            let mut nodes = cur_nodes.clone();
            frames.push(DFrame { children, at: 0, steps: steps.clone() });
            for (key, val) in members {
                let Some(idx) = children.iter().position(|c| c.is_member() && c.name == *key) else {
                    return fail(ErrorKind::UnknownKey, &format!("{path}.{key}"), format!("{} has no member named \"{key}\"", item.name));
                };
                if let Some(f) = frames.last_mut() {
                    f.at = idx;
                }
                let mark = path.len();
                path.push('.');
                path.push_str(key);
                steps.push(Step::Child(idx));
                let r = dec_item(&children[idx], &cur_nodes[idx], val, path, steps, frames, ctx);
                steps.pop();
                path.truncate(mark);
                nodes[idx] = r?;
            }
            frames.pop();
            Ok(Node::Group(nodes))
        }
        (Kind::Numeric { digits, scale, signed }, Node::Leaf(_)) => {
            let Json::Number(t) = json else {
                return fail(ErrorKind::WrongType, path, format!("expected a number, found {}", kind_name(json)));
            };
            match number::canonical_from_decimal(t, *digits, *scale, *signed) {
                Ok(c) => {
                    ctx.supplied.push(steps.clone());
                    Ok(Node::Leaf(c.into_bytes()))
                }
                Err(e) => {
                    let why = match e {
                        NumberError::TooManyDecimals => format!("{t} has more decimals than the {scale} its PICTURE allows"),
                        NumberError::TooManyDigits => format!("{t} has more integer digits than the {digits} its PICTURE allows"),
                        NumberError::NegativeUnsigned => format!("{t} is negative but the item has no sign"),
                        NumberError::OutOfRange => format!("{t} is far beyond any PICTURE"),
                        NumberError::Syntax => format!("{t} is not a decimal number"),
                    };
                    fail(ErrorKind::Overflow, path, why)
                }
            }
        }
        (kind @ (Kind::Alphanumeric { .. } | Kind::National { .. } | Kind::Utf8 { .. } | Kind::Edited { .. }), Node::Leaf(_)) => {
            let Json::String(s) = json else {
                return fail(ErrorKind::WrongType, path, format!("expected a string, found {}", kind_name(json)));
            };
            let bytes = match text_fit(kind, s) {
                Ok(b) => b,
                Err(why) => return fail(ErrorKind::TooLong, path, why),
            };
            ctx.supplied.push(steps.clone());
            Ok(Node::Leaf(bytes))
        }
        _ => fail(ErrorKind::Internal, path, "the node does not match its layout"),
    }
}

/// The stored bytes of `s` in an item of `kind`, or why it does not fit.
fn text_fit(kind: &Kind, s: &str) -> Result<Vec<u8>, String> {
    match kind {
        Kind::Alphanumeric { len, justified } => {
            if s.len() > *len {
                return Err(format!("{} bytes of UTF-8, but the item holds {len}", s.len()));
            }
            Ok(crate::node::pad_text(&Kind::Alphanumeric { len: *len, justified: *justified }, s.as_bytes()))
        }
        Kind::Edited { len } => {
            if s.len() > *len {
                return Err(format!("{} bytes, but the edited item is {len} wide", s.len()));
            }
            Ok(crate::node::pad_text(kind, s.as_bytes()))
        }
        Kind::National { len } => {
            let units = s.encode_utf16().count();
            if units > *len {
                return Err(format!("{units} UTF-16 code units, but the national item holds {len}"));
            }
            Ok(crate::node::pad_text(kind, s.as_bytes()))
        }
        Kind::Utf8 { len, bytes } => {
            let chars = s.chars().count();
            if chars > *len {
                return Err(format!("{chars} characters, but the item holds {len}"));
            }
            if let Some(b) = bytes {
                if s.len() > *b {
                    return Err(format!("{} bytes of UTF-8, but the item holds {b}", s.len()));
                }
            }
            Ok(crate::node::pad_text(kind, s.as_bytes()))
        }
        _ => Err("not a text item".into()),
    }
}

fn kind_name(j: &Json) -> &'static str {
    match j {
        Json::Null => "null",
        Json::Bool(_) => "a boolean",
        Json::Number(_) => "a number",
        Json::String(_) => "a string",
        Json::Array(_) => "an array",
        Json::Object(_) => "an object",
    }
}

/// Every variable table's count must lie in its bounds.
fn check_counts(root: &Item, tree: &Node) -> Result<(), MapError> {
    fn walk<'a>(item: &'a Item, node: &'a Node, path: &mut String, frames: &mut Vec<Frame<'a>>) -> Result<(), MapError> {
        if let (Some(occ), Node::Table(elems)) = (&item.occurs, node) {
            if let Some(dep) = &occ.depending_on {
                let n = odo_value(frames, dep, path)?;
                if n < occ.min || n > occ.max {
                    return fail(ErrorKind::OdoOutOfRange, path, format!("{dep} holds {n}, outside OCCURS {} TO {}", occ.min, occ.max));
                }
            }
            for (j, e) in elems.iter().enumerate() {
                let mark = path.len();
                path.push_str(&format!("({})", j + 1));
                walk_base(item, e, path, frames)?;
                path.truncate(mark);
            }
            return Ok(());
        }
        walk_base(item, node, path, frames)
    }
    fn walk_base<'a>(item: &'a Item, node: &'a Node, path: &mut String, frames: &mut Vec<Frame<'a>>) -> Result<(), MapError> {
        if let (Kind::Group { children }, Node::Group(nodes)) = (&item.kind, node) {
            frames.push(Frame { children, nodes, at: 0 });
            for (i, (c, n)) in children.iter().zip(nodes.iter()).enumerate() {
                if !c.is_member() {
                    continue;
                }
                if let Some(f) = frames.last_mut() {
                    f.at = i;
                }
                let mark = path.len();
                path.push('.');
                path.push_str(&c.name);
                walk(c, n, path, frames)?;
                path.truncate(mark);
            }
            frames.pop();
        }
        Ok(())
    }
    walk(root, tree, &mut root.name.clone(), &mut Vec::new())
}
