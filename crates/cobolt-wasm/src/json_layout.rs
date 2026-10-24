// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The descriptor of a group for the module's group ⇄ JSON mapping
//! (`cobolt_wasm_rt::json`, WASM-005 R5b, WASM-010 T15).
//!
//! What travels and what is refused is decided by `cobolt_wire::from_decl`, the
//! function the interpreter and the backend use: the generator walks the
//! `Item` it returns side by side with its own byte layout, and writes the
//! offsets, widths and field descriptors the routines need into one
//! position-independent blob (the format is documented in
//! `cobolt_wasm_rt::json`).

use crate::layout::Layout;
use cobolt_wasm_rt::fd::Fd;
use cobolt_wasm_rt::json::*;
use cobolt_wire::layout::{find_odo_object, Item, Kind};
use std::collections::HashMap;

/// The children of layout item `idx` that are data (no 88, no 66).
fn data_children(layout: &Layout, idx: usize) -> Vec<usize> {
    layout.items[idx]
        .children
        .iter()
        .copied()
        .filter(|&c| layout.items[c].level != 88 && layout.items[c].level != 66)
        .collect()
}

struct Blob {
    bytes: Vec<u8>,
    /// Layout item -> its node's offset.
    node_of: HashMap<usize, u32>,
}

impl Blob {
    fn put(&mut self, b: &[u8]) -> u32 {
        while self.bytes.len() % 4 != 0 {
            self.bytes.push(0);
        }
        let at = self.bytes.len() as u32;
        self.bytes.extend_from_slice(b);
        at
    }
    fn set_word(&mut self, at: u32, w: u32) {
        self.bytes[at as usize..at as usize + 4].copy_from_slice(&w.to_le_bytes());
    }
    fn word(&self, at: u32) -> u32 {
        u32::from_le_bytes(self.bytes[at as usize..at as usize + 4].try_into().unwrap())
    }
}

/// A group occurrence the walk is inside: its `Item` children, the child being
/// walked, and its layout item.
struct Frame<'a> {
    children: &'a [Item],
    at: usize,
    item: usize,
}

/// The descriptor of the group `item` whose storage is layout item `root`.
pub fn build(layout: &Layout, root: usize, item: &Item) -> Result<Vec<u8>, String> {
    let mut b = Blob {
        bytes: vec![0; 4],
        node_of: HashMap::new(),
    };
    let mut frames: Vec<Frame<'_>> = Vec::new();
    let r = node(layout, root, item, root, &mut b, &mut frames)?;
    b.set_word(0, r);
    Ok(b.bytes)
}

fn node<'a>(
    layout: &Layout,
    idx: usize,
    item: &'a Item,
    parent: usize,
    b: &mut Blob,
    frames: &mut Vec<Frame<'a>>,
) -> Result<u32, String> {
    let it = &layout.items[idx];
    let mut w = [0u32; NODE_WORDS as usize];
    let member = item.is_member();
    w[1] = if member { NF_MEMBER } else { 0 };
    w[2] = b.put(item.name.as_bytes());
    w[3] = item.name.len() as u32;
    w[4] = if idx == parent {
        0
    } else {
        it.off - layout.items[parent].off
    };
    w[5] = it.size;
    if let Some(o) = &item.occurs {
        w[1] |= NF_TABLE;
        w[6] = o.min as u32;
        w[7] = o.max as u32;
        if let Some(dep) = &o.depending_on {
            w[1] |= NF_ODO;
            // Innermost group first, then outward (`mapping::odo_value`).
            let mut found = None;
            for (up, f) in frames.iter().rev().enumerate() {
                if let Some(path) = find_odo_object(f.children, f.at, dep) {
                    let mut cur = f.item;
                    for &k in &path {
                        cur = *data_children(layout, cur)
                            .get(k)
                            .ok_or("the layouts disagree")?;
                    }
                    found = Some((
                        up as u32,
                        cur,
                        layout.items[cur].off - layout.items[f.item].off,
                    ));
                    break;
                }
            }
            let (up, count_item, rel) = found.ok_or_else(|| {
                format!("OCCURS DEPENDING ON {dep}: the count item is not in the group")
            })?;
            let count_node = *b
                .node_of
                .get(&count_item)
                .ok_or("the count item follows its table")?;
            let flags = b.word(count_node + 4);
            b.set_word(count_node + 4, flags | NF_ODO_OBJECT);
            w[8] = up;
            w[9] = rel;
            w[10] = count_node;
        }
    }
    let fd: Fd = it.fd;
    w[0] = match &item.kind {
        Kind::Group { .. } => K_GROUP,
        Kind::Alphanumeric { len, justified } => {
            w[13] = *len as u32;
            if *justified {
                w[1] |= NF_JUSTIFIED;
            }
            K_ALNUM
        }
        Kind::National { len } => {
            w[13] = *len as u32;
            K_NATIONAL
        }
        Kind::Utf8 { len, bytes } => {
            w[13] = *len as u32;
            w[14] = bytes.unwrap_or(0) as u32;
            K_UTF8
        }
        Kind::Numeric {
            digits,
            scale,
            signed,
        } => {
            w[13] = *digits as u32 | (*scale as u32) << 8 | (*signed as u32) << 16;
            K_NUMERIC
        }
        Kind::Edited { len } => {
            w[13] = *len as u32;
            K_EDITED
        }
    };
    w[15] = b.put(&fd.to_bytes());
    let words: Vec<u8> = w.iter().flat_map(|x| x.to_le_bytes()).collect();
    let at = b.put(&words);
    b.node_of.insert(idx, at);
    // Only members travel: an overlay or a FILLER is never walked inside.
    if let (true, Kind::Group { children }) = (member, &item.kind) {
        let lc = data_children(layout, idx);
        if lc.len() != children.len() {
            return Err(format!(
                "{}: the generator's layout has {} items where the wire has {}",
                item.name,
                lc.len(),
                children.len()
            ));
        }
        frames.push(Frame {
            children,
            at: 0,
            item: idx,
        });
        let mut offs = Vec::with_capacity(children.len());
        for (k, (c, &ci)) in children.iter().zip(&lc).enumerate() {
            frames.last_mut().unwrap().at = k;
            offs.push(node(layout, ci, c, idx, b, frames)?);
        }
        frames.pop();
        let list: Vec<u8> = offs.iter().flat_map(|x| x.to_le_bytes()).collect();
        let list_at = b.put(&list);
        b.set_word(at + 44, offs.len() as u32);
        b.set_word(at + 48, list_at);
    }
    Ok(at)
}

/// One elementary item of a group, as the golden vectors name it.
#[derive(Debug, Clone)]
pub struct Leaf {
    /// `REC.LINES(2).QTY`, `REC.FILLER#3` (`cobolt_wire::node::dump`).
    pub path: String,
    /// Offset from the group's start.
    pub off: u32,
    pub fd: Fd,
    pub kind: Kind,
}

/// Every elementary item of the group in declaration order, tables expanded to
/// their maximum, `REDEFINES` overlays left out (the vectors' state).
pub fn leaves(layout: &Layout, root: usize, item: &Item) -> Result<Vec<Leaf>, String> {
    let mut out = Vec::new();
    leaf_item(
        layout,
        root,
        item,
        layout.items[root].off,
        0,
        &item.name,
        &mut out,
    )?;
    Ok(out)
}

fn leaf_item(
    layout: &Layout,
    idx: usize,
    item: &Item,
    root_off: u32,
    at: u32,
    path: &str,
    out: &mut Vec<Leaf>,
) -> Result<(), String> {
    match &item.occurs {
        Some(o) => {
            for j in 0..o.max {
                leaf_base(
                    layout,
                    idx,
                    item,
                    root_off,
                    at + j as u32 * layout.items[idx].size,
                    &format!("{path}({})", j + 1),
                    out,
                )?;
            }
            Ok(())
        }
        None => leaf_base(layout, idx, item, root_off, at, path, out),
    }
}

fn leaf_base(
    layout: &Layout,
    idx: usize,
    item: &Item,
    root_off: u32,
    at: u32,
    path: &str,
    out: &mut Vec<Leaf>,
) -> Result<(), String> {
    match &item.kind {
        Kind::Group { children } => {
            let lc = data_children(layout, idx);
            if lc.len() != children.len() {
                return Err(format!("{}: the layouts disagree", item.name));
            }
            for (k, (c, &ci)) in children.iter().zip(&lc).enumerate() {
                if c.redefines {
                    continue;
                }
                let name = if c.filler {
                    format!("FILLER#{}", k + 1)
                } else {
                    c.name.clone()
                };
                let rel = layout.items[ci].off - layout.items[idx].off;
                leaf_item(
                    layout,
                    ci,
                    c,
                    root_off,
                    at + rel,
                    &format!("{path}.{name}"),
                    out,
                )?;
            }
            Ok(())
        }
        kind => {
            out.push(Leaf {
                path: path.to_string(),
                off: at,
                fd: layout.items[idx].fd,
                kind: kind.clone(),
            });
            Ok(())
        }
    }
}
