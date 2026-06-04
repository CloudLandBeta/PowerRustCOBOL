// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `CobolEnvironment` — the runtime data store for one COBOL program.
//!
//! Holds the current value of every data item declared in the DATA DIVISION
//! and provides the API used by statement executors and `EXEC RUST` blocks.
//!
//! # Initialisation
//!
//! `CobolEnvironment::from_data_division` walks the parsed DATA DIVISION AST,
//! allocates a `CobolValue` for each named item, and applies any `VALUE`
//! clause initial values.

use indexmap::IndexMap;

use cobolt_ast::{
    data::{ConditionValue, DataDecl, PicKind, Usage},
    expr::Literal,
    program::{DataSection, DataDivision},
};

use crate::value::{CobolNumeric, CobolValue};

// ── CobolEnvironment ──────────────────────────────────────────────────────────

/// Hierarchy / occurrence metadata for one declared data item.
#[derive(Debug, Clone, Default)]
pub struct ItemSym {
    /// OCCURS counts of this item plus its ancestor groups, outermost first.
    /// Empty for a non-table item. A subscripted reference supplies one index
    /// per entry.
    pub dims: Vec<usize>,
    /// Immediate child item names (uppercased), for `CORRESPONDING`. Empty for
    /// an elementary item.
    pub children: Vec<String>,
    /// Canonical storage keys of the immediate children, parallel to
    /// [`children`]. Used by `CORRESPONDING` to address the right occurrence of
    /// a duplicated child name.
    pub child_keys: Vec<String>,
    /// Ancestor group names (uppercased), outermost first, for qualified-name
    /// (`A OF B`) disambiguation.
    pub quals: Vec<String>,
    /// True if this item is a group (has children).
    pub is_group: bool,
    /// `INDEXED BY` index-item names of this table's OCCURS (uppercased).
    pub index_names: Vec<String>,
    /// This table's own OCCURS count (its last dimension), 0 if not a table.
    pub occurs: usize,
}

/// The data store for a running COBOL program.
///
/// Data items are addressed by their COBOL name (uppercase, hyphens preserved).
/// Subscripted table elements are stored under synthesized keys `NAME(i[,j…])`
/// created lazily from the base item's default; the base `NAME` slot doubles as
/// the template. Items that have not been initialised hold `CobolValue::Unset`.
#[derive(Debug, Default)]
pub struct CobolEnvironment {
    /// `name → value` store.  Insertion order is preserved (declaration order).
    store: IndexMap<String, CobolValue>,
    /// `name → (integer-digit capacity, decimal places)` for numeric items,
    /// used to detect ON SIZE ERROR overflow at store time.
    field_caps: IndexMap<String, (u8, u8)>,
    /// `name → raw PIC template` for numeric-edited items. A numeric value stored
    /// into such a field is run through the edit engine and kept as a string.
    edited_templates: IndexMap<String, String>,
    /// Names of edited items declared `BLANK WHEN ZERO` — storing a zero value
    /// blanks the whole field.
    blank_when_zero: std::collections::HashSet<String>,
    /// `DECIMAL-POINT IS COMMA` — comma is the decimal point and period the
    /// grouping symbol in edited PICs.
    decimal_comma: bool,
    /// Hierarchy / OCCURS metadata, keyed by the item's canonical storage key.
    symbols: IndexMap<String, ItemSym>,
    /// Leaf names that occur more than once in the program (under different
    /// groups). Only these need qualified (disambiguated) storage keys; every
    /// other name keys directly by itself, preserving the flat-store fast path.
    dup_names: std::collections::HashSet<String>,
    /// Leaf name → the canonical storage keys that share it (for resolution of
    /// `A OF B` qualified references). Only populated for duplicated names.
    by_leaf: IndexMap<String, Vec<String>>,
    /// 88-level condition-names → their parent item key + VALUE set.
    cond_names: IndexMap<String, CondName>,
    /// Pointer address table: `addr_of(key)` returns `index + 1` (0 = NULL).
    addr_table: Vec<String>,
    /// `SET ADDRESS OF item TO ptr` aliases: alias key → target storage key.
    addr_aliases: IndexMap<String, String>,
}

/// An 88-level condition-name: the parent data item it qualifies and the set of
/// values (single or `THRU` ranges) for which the condition is true.
#[derive(Debug, Clone)]
pub struct CondName {
    /// Canonical storage key of the parent (host) item.
    pub parent: String,
    /// The `VALUE` entries that make the condition true.
    pub values: Vec<ConditionValue>,
}

/// Tally every named (non-FILLER) leaf in a declaration subtree, so the
/// environment knows which names are duplicated and need qualified keys.
fn count_names(decl: &DataDecl, counts: &mut std::collections::HashMap<String, usize>) {
    if let Some(n) = &decl.name {
        let u = n.to_ascii_uppercase();
        if u != "FILLER" {
            *counts.entry(u).or_insert(0) += 1;
        }
    }
    for child in &decl.children {
        count_names(child, counts);
    }
}

/// `true` if `needle` appears as an (order-preserving, not necessarily
/// contiguous) subsequence of `haystack`.
fn is_subsequence(needle: &[String], haystack: &[&String]) -> bool {
    let mut it = haystack.iter();
    needle.iter().all(|q| it.any(|h| h.eq_ignore_ascii_case(q)))
}

/// The base item name of a (possibly subscripted) storage key: `A(2)` → `A`.
fn base_name(key: &str) -> &str {
    match key.find('(') {
        Some(i) => &key[..i],
        None => key,
    }
}

/// Build the storage key for a subscripted reference: `("A", [2])` → `"A(2)"`.
pub fn subscript_key(base: &str, indices: &[i64]) -> String {
    if indices.is_empty() {
        return base.to_ascii_uppercase();
    }
    let parts: Vec<String> = indices.iter().map(|i| i.to_string()).collect();
    format!("{}({})", base.to_ascii_uppercase(), parts.join(","))
}

impl CobolEnvironment {
    /// Create an empty environment.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build an environment pre-populated from a DATA DIVISION AST node.
    ///
    /// Each named data item gets a default value appropriate for its PIC
    /// clause (zeros for numeric, spaces for alphanumeric), then any `VALUE`
    /// clause is applied on top.
    pub fn from_data_division(data: &DataDivision) -> Self {
        Self::from_data_division_with(data, false)
    }

    /// Like [`from_data_division`], but with the program's `DECIMAL-POINT IS COMMA`
    /// setting (affects how edited PICs are formatted).
    pub fn from_data_division_with(data: &DataDivision, decimal_comma: bool) -> Self {
        let mut env = Self::new();
        env.decimal_comma = decimal_comma;
        // Pass 1: count every leaf name so we know which need disambiguation.
        let mut counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for section in &data.sections {
            match section {
                DataSection::WorkingStorage(items)
                | DataSection::LocalStorage(items)
                | DataSection::Linkage(items) => {
                    for decl in items {
                        count_names(decl, &mut counts);
                    }
                }
                DataSection::FileSection(fds) => {
                    for fd in fds {
                        for rec in &fd.records {
                            count_names(rec, &mut counts);
                        }
                    }
                }
                DataSection::Screen(_) => {}
            }
        }
        env.dup_names = counts
            .into_iter()
            .filter(|(_, c)| *c > 1)
            .map(|(n, _)| n)
            .collect();
        // Pass 2: initialise values + hierarchy under canonical keys.
        for section in &data.sections {
            match section {
                DataSection::WorkingStorage(items)
                | DataSection::LocalStorage(items)
                | DataSection::Linkage(items) => {
                    for decl in items {
                        env.init_decl(decl);
                    }
                }
                DataSection::FileSection(fds) => {
                    for fd in fds {
                        for rec in &fd.records {
                            env.init_decl(rec);
                        }
                    }
                }
                DataSection::Screen(_) => {} // screen items handled by forms layer
            }
        }
        env
    }

    /// Canonical storage key for a leaf with the given ancestor path
    /// (outermost first). Unique names key by themselves (flat-store fast path);
    /// duplicated names get a path-qualified key that cannot collide.
    fn canon_key(&self, leaf: &str, path: &[String]) -> String {
        if self.dup_names.contains(leaf) {
            let mut k = String::from(leaf);
            for q in path {
                k.push('\u{1}');
                k.push_str(q);
            }
            k
        } else {
            leaf.to_string()
        }
    }

    /// Resolve a (possibly qualified) reference to its canonical storage key.
    /// `quals` are the `OF`/`IN` qualifiers, innermost first. A unique name
    /// resolves to itself; a duplicated name is matched against the candidates'
    /// ancestor paths (an ambiguous reference picks the first declaration).
    pub fn resolve_name(&self, leaf: &str, quals: &[String]) -> String {
        let key = self.resolve_canonical(leaf, quals);
        // A `SET ADDRESS OF item TO ptr` aliases `item` onto another item's
        // storage — redirect here so every interpreter reference follows it.
        if let Some(target) = self.addr_aliases.get(&key) {
            return target.clone();
        }
        key
    }

    /// The canonical storage key for a reference, **without** following an
    /// address alias (used when (re)defining the alias itself).
    pub fn canonical_name(&self, leaf: &str, quals: &[String]) -> String {
        self.resolve_canonical(leaf, quals)
    }

    fn resolve_canonical(&self, leaf: &str, quals: &[String]) -> String {
        let leaf = leaf.to_ascii_uppercase();
        if !self.dup_names.contains(&leaf) {
            return leaf;
        }
        let cands = match self.by_leaf.get(&leaf) {
            Some(c) => c,
            None => return leaf,
        };
        if cands.len() == 1 {
            return cands[0].clone();
        }
        if quals.is_empty() {
            return cands[0].clone();
        }
        let qs: Vec<String> = quals.iter().map(|q| q.to_ascii_uppercase()).collect();
        for k in cands {
            if let Some(sym) = self.symbols.get(k) {
                // Qualifiers are innermost-first; the ancestor path is
                // outermost-first, so match against the reversed path.
                let rev: Vec<&String> = sym.quals.iter().rev().collect();
                if is_subsequence(&qs, &rev) {
                    return k.clone();
                }
            }
        }
        cands[0].clone()
    }

    /// Recursively initialise a data declaration and its children.
    fn init_decl(&mut self, decl: &DataDecl) {
        self.init_decl_h(decl, &mut Vec::new(), &mut Vec::new());
    }

    /// Hierarchy-aware initialisation: `dims` accumulates the OCCURS counts of
    /// this item + its ancestors; `quals` the ancestor group names.
    fn init_decl_h(&mut self, decl: &DataDecl, dims: &mut Vec<usize>, quals: &mut Vec<String>) {
        let occ = decl.occurs.as_ref().map(|o| o.max.max(1) as usize);
        if let Some(n) = occ {
            dims.push(n);
        }

        let upper = decl.name.as_ref().map(|n| n.to_ascii_uppercase());
        let is_named = matches!(&upper, Some(n) if n != "FILLER");

        if is_named {
            let leaf = upper.clone().unwrap();
            // Canonical storage key: the leaf itself when unique, otherwise a
            // path-qualified key that disambiguates duplicated names.
            let key = self.canon_key(&leaf, quals);
            // Register any 88-level condition-names qualifying this item.
            for c in &decl.children {
                if c.level == 88 {
                    if let Some(cn) = &c.name {
                        self.cond_names.insert(
                            cn.to_ascii_uppercase(),
                            CondName { parent: key.clone(), values: c.condition_values.clone() },
                        );
                    }
                }
            }
            let children: Vec<String> = decl.children.iter()
                .filter(|c| c.level != 88)
                .filter_map(|c| c.name.as_ref())
                .map(|n| n.to_ascii_uppercase())
                .filter(|n| n != "FILLER")
                .collect();
            // Canonical keys of those children (their path = our path + leaf).
            let mut child_path = quals.clone();
            child_path.push(leaf.clone());
            let child_keys: Vec<String> =
                children.iter().map(|c| self.canon_key(c, &child_path)).collect();
            let index_names: Vec<String> = decl.occurs.as_ref()
                .map(|o| o.indexed_by.iter().map(|n| n.to_ascii_uppercase()).collect())
                .unwrap_or_default();
            self.symbols.insert(key.clone(), ItemSym {
                dims: dims.clone(),
                children,
                child_keys,
                quals: quals.clone(),
                is_group: !decl.children.is_empty(),
                index_names: index_names.clone(),
                occurs: occ.unwrap_or(0),
            });
            self.by_leaf.entry(leaf.clone()).or_default().push(key.clone());
            // Base/template slot + caps/edited (one value; subscript slots are
            // created lazily from this template on first write).
            self.insert_value(&key, decl);
            // Register INDEXED BY index registers as numeric items (default 1).
            for ix in &index_names {
                self.field_caps.insert(ix.clone(), (9, 0));
                self.store.entry(ix.clone()).or_insert_with(|| CobolValue::from_i64(1));
            }
            quals.push(leaf);
        }

        for child in &decl.children {
            if child.level == 88 {
                continue; // condition-names are not data items
            }
            self.init_decl_h(child, dims, quals);
        }

        if is_named {
            quals.pop();
        }
        if occ.is_some() {
            dims.pop();
        }
    }

    /// Insert one item's base value + caps / edited template.
    fn insert_value(&mut self, upper: &str, decl: &DataDecl) {
        if let Some(pic) = &decl.picture {
            if pic.kind == PicKind::NumericEdited {
                self.init_edited(upper, &pic.template, decl.value.as_ref(), decl.blank_when_zero);
                return;
            }
        }
        let default = default_value(decl);
        let value = if let Some(lit) = &decl.value {
            apply_literal(lit, &default)
        } else {
            default
        };
        if let Some(pic) = &decl.picture {
            if pic.kind == PicKind::Numeric {
                let int_digits = pic.digits.min(u8::MAX as u16) as u8;
                let decimals = pic.decimals.min(u8::MAX as u16) as u8;
                self.field_caps.insert(upper.to_string(), (int_digits, decimals));
            }
        }
        self.store.insert(upper.to_string(), value);
    }

    // ── Hierarchy / occurrence accessors ────────────────────────────────────

    /// OCCURS dimensions of a (table) item; empty for a non-table item.
    pub fn dims_of(&self, name: &str) -> Vec<usize> {
        self.symbols.get(&name.to_ascii_uppercase()).map(|s| s.dims.clone()).unwrap_or_default()
    }

    /// Immediate child item names of a group (for CORRESPONDING).
    pub fn children_of(&self, name: &str) -> Vec<String> {
        self.symbols.get(&name.to_ascii_uppercase()).map(|s| s.children.clone()).unwrap_or_default()
    }

    /// The symbol-table entry for an item, if declared.
    pub fn symbol(&self, name: &str) -> Option<&ItemSym> {
        self.symbols.get(&name.to_ascii_uppercase())
    }

    /// The 88-level condition-name metadata for `name`, if it is one.
    pub fn cond_name(&self, name: &str) -> Option<&CondName> {
        self.cond_names.get(&name.to_ascii_uppercase())
    }

    // ── Pointers (USAGE POINTER / SET ADDRESS OF) ───────────────────────────────

    /// A stable non-zero address id for the storage key `key` (0 is reserved
    /// for NULL). Idempotent — the same key always yields the same id.
    pub fn addr_of(&mut self, key: &str) -> i64 {
        let key = key.to_ascii_uppercase();
        if let Some(i) = self.addr_table.iter().position(|k| k == &key) {
            return i as i64 + 1;
        }
        self.addr_table.push(key);
        self.addr_table.len() as i64
    }

    /// The storage key an address id points at (`None` for NULL / unknown).
    pub fn addr_target(&self, id: i64) -> Option<String> {
        if id < 1 { return None; }
        self.addr_table.get((id - 1) as usize).cloned()
    }

    /// `SET ADDRESS OF alias TO …` — make `alias` read/write `target`'s storage.
    pub fn set_alias(&mut self, alias: &str, target: &str) {
        self.addr_aliases
            .insert(alias.to_ascii_uppercase(), target.to_ascii_uppercase());
    }

    /// Remove an address alias (`SET ADDRESS OF alias TO NULL`).
    pub fn clear_alias(&mut self, alias: &str) {
        self.addr_aliases.shift_remove(&alias.to_ascii_uppercase());
    }

    /// Initialise a numeric-edited item: remember its template and store the
    /// edited string form of any VALUE (or spaces when there is none).
    fn init_edited(&mut self, name: &str, template: &str, value: Option<&Literal>, blank_when_zero: bool) {
        let dc = self.decimal_comma;
        let width = crate::numedit::edited_width(template, dc);
        if blank_when_zero {
            self.blank_when_zero.insert(name.to_string());
        }
        let v = match value {
            Some(Literal::String(s)) => CobolValue::from_str(s, width),
            Some(Literal::Integer(n)) => CobolValue::from_str(
                &crate::numedit::format_edited(template, *n as i128, 0, dc),
                width,
            ),
            Some(Literal::Decimal(m, s)) => CobolValue::from_str(
                &crate::numedit::format_edited(template, *m, *s, dc),
                width,
            ),
            _ => CobolValue::spaces(width),
        };
        self.edited_templates.insert(name.to_string(), template.to_string());
        self.store.insert(name.to_string(), v);
    }

    // ── Data access ───────────────────────────────────────────────────────────

    /// Get an immutable reference to a data item's value. An un-written table
    /// occurrence falls back to the base item's (template) value.
    pub fn get(&self, name: &str) -> Option<&CobolValue> {
        let key = name.to_ascii_uppercase();
        if let Some(v) = self.store.get(&key) {
            return Some(v);
        }
        if key.contains('(') {
            return self.store.get(base_name(&key));
        }
        None
    }

    /// Integer-digit capacity of a numeric field, if known (for ON SIZE ERROR).
    pub fn integer_capacity(&self, name: &str) -> Option<u8> {
        let key = name.to_ascii_uppercase();
        self.field_caps.get(&key).or_else(|| self.field_caps.get(base_name(&key)))
            .map(|(d, _)| *d)
    }

    /// The de-edited character form of a plain numeric field for a MOVE to an
    /// alphanumeric receiver: absolute zero-padded digits, no sign, no point.
    /// `None` if the item isn't a plain numeric.
    pub fn deedited_digits(&self, name: &str) -> Option<String> {
        let key = name.to_ascii_uppercase();
        let (int_digits, _) = *self.field_caps.get(&key).or_else(|| self.field_caps.get(base_name(&key)))?;
        if let Some(CobolValue::Numeric(n)) = self.get(&key) {
            let total = int_digits as usize + n.decimals as usize;
            let digits = n.mantissa.unsigned_abs().to_string();
            let padded = if digits.len() < total {
                format!("{}{}", "0".repeat(total - digits.len()), digits)
            } else {
                digits
            };
            Some(padded)
        } else {
            None
        }
    }

    /// `true` if the named item is a plain alphanumeric field (not numeric-edited).
    pub fn is_alphanumeric_field(&self, name: &str) -> bool {
        let key = name.to_ascii_uppercase();
        !self.edited_templates.contains_key(base_name(&key))
            && matches!(self.get(&key), Some(CobolValue::String { .. }))
    }

    /// Store `s` left-justified (space-padded) into an alphanumeric field.
    pub fn set_str_left(&mut self, name: &str, s: &str) {
        let key = name.to_ascii_uppercase();
        let cap = match self.get(&key) {
            Some(CobolValue::String { capacity, .. }) => *capacity,
            _ => s.len(),
        };
        self.store.insert(key, CobolValue::from_str(s, cap));
    }

    /// Render a data item for `DISPLAY`. A USAGE-DISPLAY numeric item is shown as
    /// its full fixed-width digit string — leading zeros to the PIC width, the
    /// implied decimal point (`V`) not shown, and a leading `-` for negatives —
    /// i.e. the characters as they are stored. Non-numeric items render verbatim.
    pub fn display_string(&self, name: &str) -> Option<String> {
        let key = name.to_ascii_uppercase();
        let val = self.get(&key)?;
        if let CobolValue::Numeric(n) = val {
            if let Some(&(int_digits, _)) =
                self.field_caps.get(&key).or_else(|| self.field_caps.get(base_name(&key)))
            {
                return Some(format_display_numeric(n, int_digits));
            }
        }
        Some(val.as_display_string())
    }

    /// Get a mutable reference to a data item's value.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut CobolValue> {
        self.store.get_mut(&name.to_ascii_uppercase())
    }

    /// Set a data item to a new value.
    ///
    /// If the item exists the new value is assigned via `CobolValue::assign`
    /// so that type coercions (rescaling, padding) are applied.
    /// If the item does not exist it is inserted directly.
    pub fn set(&mut self, name: &str, value: CobolValue) {
        let key = name.to_ascii_uppercase();
        // Storing a numeric into a numeric-edited field runs the edit engine and
        // keeps the result as the edited string. (Edited template / BLANK WHEN
        // ZERO are keyed by the base item, shared by all occurrences.)
        if let Some(template) = self.edited_templates.get(base_name(&key)).cloned() {
            // Accept any numeric source (incl. COMP-1/COMP-2 floats) for editing.
            let num = match &value {
                CobolValue::Float(f) => {
                    Some(CobolNumeric::new((*f * 1e9_f64).round() as i128, 9))
                }
                other => other.as_exact(),
            };
            if let Some(num) = num {
                let dc = self.decimal_comma;
                let width = crate::numedit::edited_width(&template, dc);
                let edited = if self.blank_when_zero.contains(base_name(&key)) && num.mantissa == 0 {
                    " ".repeat(width)
                } else {
                    crate::numedit::format_edited(&template, num.mantissa, num.decimals, dc)
                };
                self.store.insert(key, CobolValue::from_str(&edited, width));
                return;
            }
        }
        // Lazily materialise an un-written table occurrence from its base template.
        if !self.store.contains_key(&key) && key.contains('(') {
            if let Some(base_val) = self.store.get(base_name(&key)).cloned() {
                self.store.insert(key.clone(), base_val);
            }
        }
        if let Some(existing) = self.store.get_mut(&key) {
            if matches!(existing, CobolValue::Unset) {
                // Replace an uninitialised slot outright so the value isn't lost.
                *existing = value;
            } else {
                existing.assign(&value);
            }
        } else {
            self.store.insert(key, value);
        }
    }

    /// Get the numeric value of a data item as `i64` (integer part only).
    pub fn get_i64(&self, name: &str) -> Option<i64> {
        self.get(name)?.as_i64()
    }

    /// Get the numeric value of a data item as `f64`.
    pub fn get_f64(&self, name: &str) -> Option<f64> {
        Some(self.get(name)?.as_f64())
    }

    /// Get the string representation of a data item.
    pub fn get_string(&self, name: &str) -> Option<String> {
        Some(self.get(name)?.as_display_string())
    }

    /// Set a data item from an `i64`.
    pub fn set_i64(&mut self, name: &str, n: i64) {
        self.set(name, CobolValue::from_i64(n));
    }

    /// Set a data item from a `f64`.
    pub fn set_f64(&mut self, name: &str, v: f64) {
        self.set(name, CobolValue::from_f64(v));
    }

    /// Set a data item from a `&str`, padding/truncating to the existing capacity.
    pub fn set_str(&mut self, name: &str, s: &str) {
        let cap = match self.get(name) {
            Some(CobolValue::String { capacity, .. }) => *capacity,
            _ => s.len(),
        };
        self.set(name, CobolValue::from_str(s, cap));
    }

    /// `true` if the named data item is declared.
    pub fn contains(&self, name: &str) -> bool {
        self.store.contains_key(&name.to_ascii_uppercase())
    }

    /// Iterate all data items in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &CobolValue)> {
        self.store.iter()
    }

    // ── Nested-program scope management ──────────────────────────────────────

    /// Push a set of local data items into this environment for the duration
    /// of a nested-program call.
    ///
    /// Items that do not yet exist are inserted; items that already exist
    /// (e.g. GLOBAL names that happen to collide) are *not* overwritten —
    /// the caller's value wins.
    ///
    /// Returns the list of keys that were *newly inserted* so that
    /// [`pop_local_scope`] can remove exactly those entries.
    pub fn push_local_scope(&mut self, items: &[(String, CobolValue)]) -> Vec<String> {
        let mut inserted = Vec::with_capacity(items.len());
        for (key, val) in items {
            let upper = key.to_ascii_uppercase();
            if !self.store.contains_key(&upper) {
                self.store.insert(upper.clone(), val.clone());
                inserted.push(upper);
            }
        }
        inserted
    }

    /// Remove the keys that were inserted by a matching [`push_local_scope`]
    /// call, restoring the environment to its pre-call state.
    pub fn pop_local_scope(&mut self, keys: &[String]) {
        for key in keys {
            self.store.shift_remove(key);
        }
    }

    /// Collect all GLOBAL-flagged items declared in a DATA DIVISION.
    ///
    /// Returns `(name, initial_value)` pairs, ready to be inserted into a
    /// parent or sibling program's environment so nested programs can read
    /// and write them without re-declaration.
    pub fn global_items_from_data_division(data: &DataDivision) -> Vec<(String, CobolValue)> {
        let mut out = Vec::new();
        for section in &data.sections {
            match section {
                DataSection::WorkingStorage(items)
                | DataSection::LocalStorage(items)
                | DataSection::Linkage(items) => {
                    for decl in items {
                        collect_global_items(decl, &mut out);
                    }
                }
                DataSection::FileSection(fds) => {
                    for fd in fds {
                        for rec in &fd.records {
                            collect_global_items(rec, &mut out);
                        }
                    }
                }
                DataSection::Screen(_) => {}
            }
        }
        out
    }
}

/// Recursively collect GLOBAL-flagged data items (and their children).
fn collect_global_items(decl: &DataDecl, out: &mut Vec<(String, CobolValue)>) {
    if decl.is_global {
        if let Some(name) = &decl.name {
            let upper = name.to_ascii_uppercase();
            let val = default_value(decl);
            let val = if let Some(lit) = &decl.value { apply_literal(lit, &val) } else { val };
            out.push((upper, val));
        }
        for child in &decl.children {
            collect_global_items(child, out);
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Format a numeric value as its fixed-width DISPLAY digit string: zero-padded to
/// `int_digits` + scale, no decimal point (the `V` is implied), leading `-` if
/// negative.
fn format_display_numeric(n: &CobolNumeric, int_digits: u8) -> String {
    let total = int_digits as usize + n.decimals as usize;
    let digits = n.mantissa.unsigned_abs().to_string();
    let padded = if digits.len() < total {
        format!("{}{}", "0".repeat(total - digits.len()), digits)
    } else {
        digits
    };
    if n.mantissa < 0 {
        format!("-{padded}")
    } else {
        padded
    }
}

/// Build the default (zero / spaces) value for a data declaration.
fn default_value(decl: &DataDecl) -> CobolValue {
    // COMP-1 / COMP-2 are PIC-less floating point — default to 0.0, not Unset.
    if decl.picture.is_none() {
        if matches!(decl.usage, Usage::Comp1 | Usage::Comp2) {
            return CobolValue::Float(0.0);
        }
        // Group items with no PIC → treat as uninitialised.
        return CobolValue::Unset;
    }
    let pic = decl.picture.as_ref().unwrap();

    match pic.kind {
        PicKind::Numeric | PicKind::NumericEdited => {
            // Decimal places never exceed COBOL's 18-digit limit, so the narrowing
            // to u8 (CobolNumeric's scale) is safe.
            CobolValue::Numeric(CobolNumeric::new(0, pic.decimals.min(u8::MAX as u16) as u8))
        }
        PicKind::Alphabetic | PicKind::Alphanumeric | PicKind::AlphanumericEdited => {
            let cap = pic.digits as usize + pic.decimals as usize;
            CobolValue::spaces(cap.max(1))
        }
    }
}

/// Apply a `VALUE` clause literal on top of a default value.
fn apply_literal(lit: &Literal, default: &CobolValue) -> CobolValue {
    match lit {
        Literal::Integer(n) => match default {
            CobolValue::Numeric(num) => {
                let mut v = CobolValue::Numeric(num.clone());
                v.assign(&CobolValue::from_i64(*n));
                v
            }
            CobolValue::String { capacity, .. } => {
                CobolValue::from_str(&n.to_string(), *capacity)
            }
            _ => CobolValue::from_i64(*n),
        },
        Literal::Float(f) => CobolValue::from_f64(*f),
        Literal::Decimal(m, s) => {
            // Exact decimal VALUE — rescale into the receiving field's PIC.
            let src = CobolValue::Numeric(CobolNumeric::new(*m, *s));
            match default {
                CobolValue::Numeric(num) => {
                    let mut v = CobolValue::Numeric(num.clone());
                    v.assign(&src);
                    v
                }
                CobolValue::String { capacity, .. } => {
                    CobolValue::from_str(&src.as_display_string(), *capacity)
                }
                _ => src,
            }
        }
        Literal::String(s) => match default {
            CobolValue::String { capacity, .. } => CobolValue::from_str(s, *capacity),
            _ => CobolValue::from_str(s, s.len()),
        },
        Literal::Figurative(fig) => {
            use cobolt_ast::expr::FigurativeConstant;
            let cap = match default {
                CobolValue::String { capacity, .. } => *capacity,
                _ => 1,
            };
            match fig {
                FigurativeConstant::Space     => CobolValue::spaces(cap),
                // ZERO must preserve the receiving field's PIC scale — a numeric
                // field keeps its decimal places (a scale-0 zero would wipe them).
                FigurativeConstant::Zero      => match default {
                    CobolValue::Numeric(n) => CobolValue::Numeric(CobolNumeric::new(0, n.decimals)),
                    CobolValue::String { capacity, .. } =>
                        CobolValue::String { bytes: vec![b'0'; *capacity], capacity: *capacity },
                    _ => CobolValue::zero(0),
                },
                FigurativeConstant::HighValue => CobolValue::figurative_high_values(cap),
                FigurativeConstant::LowValue  => CobolValue::figurative_low_values(cap),
                _                             => default.clone(),
            }
        }
    }
}
