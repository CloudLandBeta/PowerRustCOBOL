// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The value of a group, as a tree that mirrors its layout (WASM-005 R5).
//!
//! A [`Node`] tree is what the mapping encodes and what it decodes into. It is
//! deliberately **not** the interpreter's environment: decoding builds a whole
//! new tree and only then does the caller write it back, which is how "an
//! error writes nothing" (R6) is true by construction.

use crate::layout::{zero_text, Item, Kind, Step};

/// The value of one item, shaped like its [`Item`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// An elementary item's stored text, as bytes. A numeric item holds its
    /// canonical decimal text; a text item holds its full fixed width (padded
    /// with spaces); a national or UTF-8 item holds its characters, padded
    /// with spaces to its width.
    Leaf(Vec<u8>),
    /// A group: one node per child of the layout, **including** `FILLER` and
    /// `REDEFINES` children, in declaration order.
    Group(Vec<Node>),
    /// A table: exactly `occurs.max` elements. The current length of an
    /// `OCCURS DEPENDING ON` table is a property of the data (the count item),
    /// not of this vector.
    Table(Vec<Node>),
}

impl Node {
    /// The initial value of an item: its `VALUE`, else zeros / spaces.
    pub fn initial(item: &Item) -> Node {
        let base = Self::initial_base(item);
        match &item.occurs {
            Some(o) => Node::Table(vec![base; o.max]),
            None => base,
        }
    }

    fn initial_base(item: &Item) -> Node {
        match &item.kind {
            Kind::Group { children } => Node::Group(children.iter().map(Node::initial).collect()),
            Kind::Numeric { scale, .. } => {
                Node::Leaf(item.value.clone().unwrap_or_else(|| zero_text(*scale)).into_bytes())
            }
            kind => Node::Leaf(pad_text(kind, item.value.as_deref().unwrap_or("").as_bytes())),
        }
    }

    /// Follow `steps` from this node.
    pub fn at(&self, steps: &[Step]) -> Option<&Node> {
        let mut n = self;
        for s in steps {
            n = match (s, n) {
                (Step::Child(i), Node::Group(c)) => c.get(*i)?,
                (Step::Elem(i), Node::Table(e)) => e.get(*i)?,
                _ => return None,
            };
        }
        Some(n)
    }

    pub fn at_mut(&mut self, steps: &[Step]) -> Option<&mut Node> {
        let mut n = self;
        for s in steps {
            n = match (s, n) {
                (Step::Child(i), Node::Group(c)) => c.get_mut(*i)?,
                (Step::Elem(i), Node::Table(e)) => e.get_mut(*i)?,
                _ => return None,
            };
        }
        Some(n)
    }

    /// The text of a leaf, lossily as UTF-8 (for messages and test dumps).
    pub fn leaf_text(&self) -> Option<String> {
        match self {
            Node::Leaf(b) => Some(String::from_utf8_lossy(b).into_owned()),
            _ => None,
        }
    }
}

/// Pad (or, for an over-long value, keep) the text bytes of a non-numeric item
/// to its declared width. Used for initial values; the decoder checks length
/// itself and never relies on this to truncate.
pub fn pad_text(kind: &Kind, text: &[u8]) -> Vec<u8> {
    match kind {
        Kind::Alphanumeric { len, justified } => pad_bytes(text, *len, *justified),
        Kind::Edited { len } => pad_bytes(text, *len, false),
        Kind::National { len } => {
            let units = std::str::from_utf8(text).map(|s| s.encode_utf16().count()).unwrap_or(text.len());
            let mut v = text.to_vec();
            if units < *len {
                v.extend(std::iter::repeat_n(b' ', len - units));
            }
            v
        }
        Kind::Utf8 { len, .. } => {
            let chars = std::str::from_utf8(text).map(|s| s.chars().count()).unwrap_or(text.len());
            let mut v = text.to_vec();
            if chars < *len {
                v.extend(std::iter::repeat_n(b' ', len - chars));
            }
            v
        }
        _ => text.to_vec(),
    }
}

fn pad_bytes(text: &[u8], len: usize, right_justified: bool) -> Vec<u8> {
    let take = text.len().min(len);
    let mut v = Vec::with_capacity(len);
    if right_justified {
        v.extend(std::iter::repeat_n(b' ', len - take));
        v.extend_from_slice(&text[text.len() - take..]);
    } else {
        v.extend_from_slice(&text[..take]);
        v.extend(std::iter::repeat_n(b' ', len - take));
    }
    v
}

// ── state dumps (the vectors' "group read back") ──────────────────────────────

/// Every elementary value of a group in declaration order, as
/// `(path, text)`: `REC.NAME`, `REC.LINES(2).QTY`, and `REC.FILLER#3` for an
/// unnamed item (its 1-based position in its group). `REDEFINES` overlays are
/// left out — their bytes are the redefined item's. The text is the stored
/// text: canonical decimal for a number, the full padded width otherwise.
pub fn dump(item: &Item, node: &Node) -> Vec<(String, String)> {
    let mut out = Vec::new();
    dump_item(item, node, &item.name, &mut out);
    out
}

fn dump_item(item: &Item, node: &Node, path: &str, out: &mut Vec<(String, String)>) {
    match (&item.occurs, node) {
        (Some(_), Node::Table(elems)) => {
            for (i, e) in elems.iter().enumerate() {
                dump_base(item, e, &format!("{path}({})", i + 1), out);
            }
        }
        _ => dump_base(item, node, path, out),
    }
}

fn dump_base(item: &Item, node: &Node, path: &str, out: &mut Vec<(String, String)>) {
    match (&item.kind, node) {
        (Kind::Group { children }, Node::Group(nodes)) => {
            for (i, (c, n)) in children.iter().zip(nodes.iter()).enumerate() {
                if c.redefines {
                    continue;
                }
                let name = if c.filler { format!("FILLER#{}", i + 1) } else { c.name.clone() };
                dump_item(c, n, &format!("{path}.{name}"), out);
            }
        }
        (_, Node::Leaf(b)) => out.push((path.to_string(), String::from_utf8_lossy(b).into_owned())),
        _ => out.push((path.to_string(), "<shape mismatch>".to_string())),
    }
}

/// Set the leaf at a dumped `path` (the form [`dump`] writes) to `text`. The
/// text is stored as given (padded for a text item); it is the caller's job to
/// give a canonical number.
pub fn set_by_path(item: &Item, node: &mut Node, path: &str, text: &str) -> Result<(), String> {
    let segs = parse_path(path)?;
    if !segs.first().is_some_and(|(n, _)| *n == item.name) {
        return Err(format!("path {path} does not start at {}", item.name));
    }
    set_walk(item, node, &segs[1..], segs[0].1, text)
}

fn parse_path(path: &str) -> Result<Vec<(String, Option<usize>)>, String> {
    let mut segs = Vec::new();
    for part in path.split('.') {
        if let Some((name, rest)) = part.split_once('(') {
            let n: usize = rest.trim_end_matches(')').parse().map_err(|_| format!("bad subscript in {part}"))?;
            if n == 0 {
                return Err(format!("subscripts are 1-based: {part}"));
            }
            segs.push((name.to_string(), Some(n)));
        } else {
            segs.push((part.to_string(), None));
        }
    }
    Ok(segs)
}

fn set_walk(item: &Item, node: &mut Node, rest: &[(String, Option<usize>)], sub: Option<usize>, text: &str) -> Result<(), String> {
    let node = match (&item.occurs, sub) {
        (Some(_), Some(n)) => match node {
            Node::Table(e) => e.get_mut(n - 1).ok_or_else(|| format!("{}({n}) is past the table", item.name))?,
            _ => return Err("table node expected".into()),
        },
        (Some(_), None) => return Err(format!("{} is a table: give a subscript", item.name)),
        (None, Some(_)) => return Err(format!("{} is not a table", item.name)),
        (None, None) => node,
    };
    match (&item.kind, node) {
        (Kind::Group { children }, Node::Group(nodes)) => {
            let Some(((name, sub), tail)) = rest.split_first() else {
                return Err(format!("{} is a group: name a member", item.name));
            };
            let idx = if let Some(k) = name.strip_prefix("FILLER#") {
                k.parse::<usize>().ok().and_then(|k| k.checked_sub(1)).filter(|i| children.get(*i).is_some_and(|c| c.filler))
            } else {
                children.iter().position(|c| !c.redefines && !c.filler && c.name == *name)
            }
            .ok_or_else(|| format!("{} has no member {name}", item.name))?;
            set_walk(&children[idx], &mut nodes[idx], tail, *sub, text)
        }
        (kind, Node::Leaf(b)) => {
            if !rest.is_empty() {
                return Err(format!("{} is elementary", item.name));
            }
            *b = match kind {
                Kind::Numeric { .. } => text.as_bytes().to_vec(),
                other => pad_text(other, text.as_bytes()),
            };
            Ok(())
        }
        _ => Err("shape mismatch".into()),
    }
}
