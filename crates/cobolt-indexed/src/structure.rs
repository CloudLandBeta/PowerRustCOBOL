// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Flatten / rebuild record trees from visual indentation depth.

use crate::IndexedDefinition;
use crate::IndexedField;

/// One row in the record-structure editor (preorder).
#[derive(Debug, Clone)]
pub struct FlatEntry {
    pub field: IndexedField,
    pub depth: usize,
}

/// Visual indent depth ↔ COBOL level number.
pub fn depth_from_level(level: u8) -> usize {
    if level <= 1 {
        0
    } else {
        level as usize / 5
    }
}

pub fn level_from_depth(depth: usize) -> u8 {
    if depth == 0 {
        1
    } else {
        (depth as u8) * 5
    }
}

/// Preorder flattening using nesting depth.
pub fn flatten_record(def: &IndexedDefinition) -> Vec<FlatEntry> {
    let mut out = Vec::new();
    for root in &def.fields {
        flatten_node(root, 0, &mut out);
    }
    out
}

fn flatten_node(node: &IndexedField, depth: usize, out: &mut Vec<FlatEntry>) {
    out.push(FlatEntry {
        field: node.clone(),
        depth,
    });
    for child in &node.children {
        flatten_node(child, depth + 1, out);
    }
}

/// Rebuild the field tree from a flat list; assigns COBOL levels from depth.
pub fn rebuild_record(flat: &[FlatEntry]) -> Result<Vec<IndexedField>, String> {
    validate_flat_indent(flat)?;
    let mut roots = Vec::new();
    let mut pos = 0usize;
    while pos < flat.len() {
        roots.push(build_subtree(flat, &mut pos)?);
    }
    Ok(roots)
}

fn build_subtree(flat: &[FlatEntry], pos: &mut usize) -> Result<IndexedField, String> {
    if *pos >= flat.len() {
        return Err("unexpected end of record structure".into());
    }
    let depth = flat[*pos].depth;
    let mut field = flat[*pos].field.clone();
    field.level = level_from_depth(depth);
    field.children.clear();
    *pos += 1;
    if field.is_group() {
        while *pos < flat.len() && flat[*pos].depth > depth {
            field.children.push(build_subtree(flat, pos)?);
        }
    } else if *pos < flat.len() && flat[*pos].depth > depth {
        return Err(format!(
            "data item '{}' cannot contain nested fields",
            field.name
        ));
    }
    Ok(field)
}

/// Apply changes from flat list back into a definition.
pub fn apply_flat(def: &mut IndexedDefinition, flat: &[FlatEntry]) -> Result<(), String> {
    def.fields = rebuild_record(flat)?;
    def.recompute_offsets();
    Ok(())
}

/// Whether `idx` may be outdented to `new_depth`.
pub fn outdent_allowed(flat: &[FlatEntry], idx: usize, new_depth: usize) -> bool {
    if idx == 0 || new_depth >= flat[idx].depth {
        return true;
    }
    if idx > 0
        && flat[idx - 1].field.is_group()
        && flat[idx].depth == flat[idx - 1].depth + 1
        && new_depth == flat[idx - 1].depth
    {
        return false;
    }
    true
}

/// Apply indent (+1) to entry `idx` and descendants until depth <= entry's original depth.
pub fn indent_entry(flat: &mut [FlatEntry], idx: usize) {
    let base = flat[idx].depth;
    flat[idx].depth += 1;
    for e in flat.iter_mut().skip(idx + 1) {
        if e.depth <= base {
            break;
        }
        e.depth += 1;
    }
}

/// Apply outdent (-1) to entry `idx` and descendants; returns false if illegal.
pub fn outdent_entry(flat: &mut [FlatEntry], idx: usize) -> bool {
    if flat[idx].depth == 0 {
        return false;
    }
    let new_depth = flat[idx].depth - 1;
    if !outdent_allowed(flat, idx, new_depth) {
        return false;
    }
    let base = flat[idx].depth;
    flat[idx].depth = new_depth;
    for e in flat.iter_mut().skip(idx + 1) {
        if e.depth <= base {
            break;
        }
        e.depth -= 1;
    }
    true
}

/// Validate flat indentation before save / rebuild.
pub fn validate_flat_indent(flat: &[FlatEntry]) -> Result<(), String> {
    for (i, entry) in flat.iter().enumerate() {
        if entry.depth == 0 {
            continue;
        }
        if !outdent_allowed(flat, i, entry.depth) {
            return Err(
                "illegal indentation: a data item cannot sit at the same level as an open group"
                    .into(),
            );
        }
    }
    Ok(())
}

/// Validate an on-disk definition (flatten + indent rules + rebuild).
pub fn validate_definition(def: &IndexedDefinition) -> Result<(), String> {
    let flat = flatten_record(def);
    validate_flat_indent(&flat)?;
    // REDEFINES must refer to a prior item (cannot point forward or to self).
    for (i, e) in flat.iter().enumerate() {
        if let Some(ref tgt) = e.field.redefines {
            if let Some(ti) = flat.iter().position(|ee| &ee.field.name == tgt) {
                if ti >= i {
                    return Err(format!(
                        "REDEFINES CLAUSE cannot refer to an item above the current item being redefined ({} redefines {})",
                        e.field.name, tgt
                    ));
                }
            } else {
                return Err(format!("REDEFINES target {} not found", tgt));
            }
        }
    }
    rebuild_record(&flat)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FieldUsage;

    fn leaf(name: &str, depth: usize) -> FlatEntry {
        FlatEntry {
            field: IndexedField {
                level: level_from_depth(depth),
                name: name.into(),
                pic: "X(1)".into(),
                usage: FieldUsage::Display,
                offset: Some(0),
                length: Some(1),
                comment: String::new(),
                grid_control: None,
                occurs: None,
                redefines: None,
                synchronized: false,
                children: Vec::new(),
            },
            depth,
        }
    }

    fn group(name: &str, depth: usize) -> FlatEntry {
        let mut f = leaf(name, depth).field;
        f.offset = None;
        f.length = None;
        f.pic.clear();
        FlatEntry { field: f, depth }
    }

    #[test]
    fn outdent_past_open_group_is_illegal() {
        let flat = vec![group("GRP", 0), group("SUB", 1), leaf("DATA", 2)];
        assert!(!outdent_allowed(&flat, 2, 1));
    }

    #[test]
    fn round_trip_flat_rebuild() {
        let flat = vec![group("ROOT", 0), leaf("A", 1), leaf("B", 1)];
        let roots = rebuild_record(&flat).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].children.len(), 2);
        assert_eq!(roots[0].children[0].level, 5);
    }
}
