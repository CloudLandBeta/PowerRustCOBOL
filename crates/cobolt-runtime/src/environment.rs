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
    program::{DataDivision, DataSection},
};

use crate::value::{CobolNumeric, CobolValue};

// ── ItemScope ────────────────────────────────────────────────────────────────

/// DATA DIVISION origin for a runtime data item, used by debugger snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemScope {
    WorkingStorage,
    FileDescription,
    LocalStorage,
    Linkage,
    Screen,
}

impl ItemScope {
    pub fn abbrev(self) -> &'static str {
        match self {
            Self::WorkingStorage => "WS",
            Self::FileDescription => "FD",
            Self::LocalStorage => "LS",
            Self::Linkage => "LK",
            Self::Screen => "SCREEN",
        }
    }
}

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
    /// Storage keys of **every** subordinate item in declaration order,
    /// including unnamed `FILLER`. This is the group's byte layout, as opposed
    /// to [`child_keys`], which is the `CORRESPONDING` list and excludes items
    /// that have no name to correspond by. Empty for an elementary item.
    pub layout_keys: Vec<String>,
    /// Ancestor group names (uppercased), outermost first, for qualified-name
    /// (`A OF B`) disambiguation.
    pub quals: Vec<String>,
    /// True if this item is a group (has children).
    pub is_group: bool,
    /// `INDEXED BY` index-item names of this table's OCCURS (uppercased).
    pub index_names: Vec<String>,
    /// This table's own OCCURS count (its last dimension), 0 if not a table.
    pub occurs: usize,
    /// `ASCENDING/DESCENDING KEY` fields (uppercased, major-to-minor), each with
    /// its ascending flag. Empty unless the OCCURS declared sort keys; drives the
    /// `SEARCH ALL` binary search.
    pub keys: Vec<(String, bool)>,
    /// DATA DIVISION section that declared this item.
    pub scope: Option<ItemScope>,
    /// True if this item belongs to a GLOBAL declaration.
    pub is_global: bool,
    /// Raw PIC template, or an empty string for group/index items.
    pub pic: String,
    /// Program/form/procedure that declared this item.
    pub origin: String,
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
    /// Elementary item keys in declaration order (for 66 RENAMES ranges).
    elem_order: Vec<String>,
    /// 66-level RENAMES items → the covered elementary keys (in order).
    renames: IndexMap<String, Vec<String>>,
    /// Canonical storage keys of `EXTERNAL` items (01/77-level and EXTERNAL FD
    /// records), including all of their subordinate keys. These are shared
    /// run-unit-wide via the [`ExternalStore`] (spec 005); GLOBAL items are not
    /// listed here — they are shared only within a program's nested units.
    external_names: std::collections::HashSet<String>,
}

/// Run-unit-wide `EXTERNAL` data store.
///
/// COBOL `EXTERNAL` items are shared *by name* across every program in a run
/// unit (the process): there is exactly one physical copy. This map is that
/// copy. It lives behind `Arc<Mutex<…>>` so a run unit spanning several
/// interpreter instances — e.g. multiple form modules, possibly on different
/// threads — can share it. Create one per run unit and clone the `Arc` to hand
/// the same store to another interpreter.
pub type ExternalStore = std::sync::Arc<std::sync::Mutex<IndexMap<String, CobolValue>>>;

/// Create a fresh, empty run-unit `EXTERNAL` store.
pub fn new_external_store() -> ExternalStore {
    std::sync::Arc::new(std::sync::Mutex::new(IndexMap::new()))
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

/// The storage key of the `idx`-th subordinate item of `parent` when that item
/// is unnamed (`FILLER`, or the level number alone).
///
/// FILLER holds bytes and may carry a VALUE, so it has to be stored somewhere —
/// but it has no name, and nothing in the COBOL program may reach it. `\u{2}`
/// cannot appear in a COBOL word, so a key built with it is addressable by the
/// group's layout and by nothing else.
fn filler_key(parent: &str, idx: usize) -> String {
    format!("{parent}\u{2}{idx}")
}

/// `true` if `key` is one of those synthetic FILLER slots — used to keep them
/// out of anything a developer reads, such as the debugger's variable list.
fn is_filler_key(key: &str) -> bool {
    key.contains('\u{2}')
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
        Self::from_data_division_with_origin(data, decimal_comma, "")
    }

    /// Like [`from_data_division_with`], and records the declaring program/form
    /// name for debugger variable details.
    pub fn from_data_division_with_origin(
        data: &DataDivision,
        decimal_comma: bool,
        origin: &str,
    ) -> Self {
        let mut env = Self::new();
        env.decimal_comma = decimal_comma;
        let origin = origin.to_owned();
        // Pass 1: count every leaf name so we know which need disambiguation.
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
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
        // Pass 2: initialise values + hierarchy under canonical keys, recording
        // EXTERNAL items (01/77 and EXTERNAL FD records) for run-unit sharing.
        for section in &data.sections {
            match section {
                DataSection::WorkingStorage(items)
                | DataSection::LocalStorage(items)
                | DataSection::Linkage(items) => {
                    for decl in items {
                        let external = decl.is_external && (decl.level == 1 || decl.level == 77);
                        let scope = match section {
                            DataSection::WorkingStorage(_) => ItemScope::WorkingStorage,
                            DataSection::LocalStorage(_) => ItemScope::LocalStorage,
                            DataSection::Linkage(_) => ItemScope::Linkage,
                            _ => unreachable!(),
                        };
                        env.init_decl_tracking_external(decl, external, scope, false, &origin);
                    }
                }
                DataSection::FileSection(fds) => {
                    for fd in fds {
                        for rec in &fd.records {
                            env.init_decl_tracking_external(
                                rec,
                                rec.is_external,
                                ItemScope::FileDescription,
                                fd.is_global,
                                &origin,
                            );
                        }
                    }
                }
                DataSection::Screen(_) => {} // screen items handled by forms layer
            }
        }
        sync_redefines(&mut env, data);
        env
    }

    /// Initialise `decl`; when `external` is set, record every storage key it
    /// creates (the item and all its subordinates) as a run-unit EXTERNAL key.
    fn init_decl_tracking_external(
        &mut self,
        decl: &DataDecl,
        external: bool,
        scope: ItemScope,
        inherited_global: bool,
        origin: &str,
    ) {
        if !external {
            self.init_decl(decl, scope, inherited_global, origin);
            return;
        }
        let before: std::collections::HashSet<String> = self.store.keys().cloned().collect();
        self.init_decl(decl, scope, inherited_global, origin);
        for k in self.store.keys() {
            if !before.contains(k) {
                self.external_names.insert(k.clone());
            }
        }
    }

    /// Canonical storage keys that are `EXTERNAL` (shared run-unit-wide).
    pub fn external_names(&self) -> &std::collections::HashSet<String> {
        &self.external_names
    }

    /// Direct read of a canonical storage key — no name resolution, no group
    /// cascade. Used by the run-unit EXTERNAL sync, which already holds
    /// canonical keys (from [`external_names`](Self::external_names)).
    pub fn raw_get(&self, key: &str) -> Option<&CobolValue> {
        self.store.get(key)
    }

    /// Direct write of a canonical storage key — no name resolution, no group
    /// cascade. Counterpart to [`raw_get`](Self::raw_get).
    pub fn raw_set(&mut self, key: &str, value: CobolValue) {
        self.store.insert(key.to_string(), value);
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
    fn init_decl(
        &mut self,
        decl: &DataDecl,
        scope: ItemScope,
        inherited_global: bool,
        origin: &str,
    ) {
        self.init_decl_h(
            decl,
            &mut Vec::new(),
            &mut Vec::new(),
            scope,
            inherited_global,
            origin,
        );
    }

    /// Hierarchy-aware initialisation: `dims` accumulates the OCCURS counts of
    /// this item + its ancestors; `quals` the ancestor group names.
    fn init_decl_h(
        &mut self,
        decl: &DataDecl,
        dims: &mut Vec<usize>,
        quals: &mut Vec<String>,
        scope: ItemScope,
        inherited_global: bool,
        origin: &str,
    ) {
        let occ = decl.occurs.as_ref().map(|o| o.max.max(1) as usize);
        if let Some(n) = occ {
            dims.push(n);
        }

        let upper = decl.name.as_ref().map(|n| n.to_ascii_uppercase());
        let is_named = matches!(&upper, Some(n) if n != "FILLER");
        // This item's own storage key once it is known, so the children loop
        // below can key its unnamed FILLERs under it.
        let mut owner_key: Option<String> = None;

        // 66-level RENAMES: register the regrouping over already-declared
        // elementary items; it has no storage of its own.
        if decl.level == 66 {
            if let (Some(name), Some(ren)) = (&upper, &decl.renames) {
                let from = self.canonical_name(&ren.from, &[]);
                let thru = ren.thru.as_ref().map(|t| self.canonical_name(t, &[]));
                if let Some(covered) = self.renames_range(&from, thru.as_deref()) {
                    self.renames.insert(name.clone(), covered);
                }
            }
            if occ.is_some() {
                dims.pop();
            }
            return;
        }

        if is_named {
            let leaf = upper.clone().unwrap();
            let is_global = inherited_global || decl.is_global;
            // Canonical storage key: the leaf itself when unique, otherwise a
            // path-qualified key that disambiguates duplicated names.
            let key = self.canon_key(&leaf, quals);
            // Register any 88-level condition-names qualifying this item.
            for c in &decl.children {
                if c.level == 88 {
                    if let Some(cn) = &c.name {
                        self.cond_names.insert(
                            cn.to_ascii_uppercase(),
                            CondName {
                                parent: key.clone(),
                                values: c.condition_values.clone(),
                            },
                        );
                    }
                }
            }
            let children: Vec<String> = decl
                .children
                .iter()
                .filter(|c| c.level != 88)
                .filter_map(|c| c.name.as_ref())
                .map(|n| n.to_ascii_uppercase())
                .filter(|n| n != "FILLER")
                .collect();
            // Canonical keys of those children (their path = our path + leaf).
            let mut child_path = quals.clone();
            child_path.push(leaf.clone());
            let child_keys: Vec<String> = children
                .iter()
                .map(|c| self.canon_key(c, &child_path))
                .collect();
            // The group's LAYOUT: every subordinate item in declaration order,
            // **including FILLER**. This is deliberately not `child_keys`, which
            // is the CORRESPONDING list and must leave unnamed items out — they
            // have no name to correspond by. A group's *bytes*, though, are all
            // of its items: drop the FILLERs and `01 T. 05 HH PIC 99. 05 PIC X
            // VALUE ":". 05 MM PIC 99.` reads back as "1234" instead of "12:34".
            let layout_keys: Vec<String> = decl
                .children
                .iter()
                .enumerate()
                .filter(|(_, c)| c.level != 88 && c.level != 66)
                .map(|(i, c)| match c.name.as_deref() {
                    Some(n) if !n.eq_ignore_ascii_case("FILLER") => {
                        self.canon_key(&n.to_ascii_uppercase(), &child_path)
                    }
                    _ => filler_key(&key, i),
                })
                .collect();
            let index_names: Vec<String> = decl
                .occurs
                .as_ref()
                .map(|o| {
                    o.indexed_by
                        .iter()
                        .map(|n| n.to_ascii_uppercase())
                        .collect()
                })
                .unwrap_or_default();
            let keys: Vec<(String, bool)> = decl
                .occurs
                .as_ref()
                .map(|o| {
                    o.keys
                        .iter()
                        .map(|(n, asc)| (n.to_ascii_uppercase(), *asc))
                        .collect()
                })
                .unwrap_or_default();
            self.symbols.insert(
                key.clone(),
                ItemSym {
                    dims: dims.clone(),
                    children,
                    child_keys,
                    layout_keys,
                    quals: quals.clone(),
                    is_group: !decl.children.is_empty(),
                    index_names: index_names.clone(),
                    occurs: occ.unwrap_or(0),
                    keys,
                    scope: Some(scope),
                    is_global,
                    pic: decl
                        .picture
                        .as_ref()
                        .map(|pic| pic.template.clone())
                        .unwrap_or_default(),
                    origin: origin.to_owned(),
                },
            );
            self.by_leaf
                .entry(leaf.clone())
                .or_default()
                .push(key.clone());
            // Record elementary (leaf) items in declaration order for 66 RENAMES.
            if decl.children.is_empty() {
                self.elem_order.push(key.clone());
            }
            // Base/template slot + caps/edited (one value; subscript slots are
            // created lazily from this template on first write).
            self.insert_value(&key, decl);
            // Register INDEXED BY index registers as numeric items (default 1).
            for ix in &index_names {
                self.field_caps.insert(ix.clone(), (9, 0));
                self.store
                    .entry(ix.clone())
                    .or_insert_with(|| CobolValue::from_i64(1));
            }
            owner_key = Some(key);
            quals.push(leaf);
        }

        for (i, child) in decl.children.iter().enumerate() {
            if child.level == 88 {
                continue; // condition-names are not data items
            }
            // An unnamed / FILLER item occupies bytes in its parent and may
            // carry a VALUE, but it has no name, so `is_named` below would give
            // it no storage at all and the group would read back with its
            // separators missing. Give it a synthetic key under its parent —
            // `\u{2}` cannot occur in a COBOL name, so nothing in the program
            // can reach it — and store its value there.
            let unnamed = !matches!(child.name.as_deref(), Some(n) if !n.eq_ignore_ascii_case("FILLER"));
            if unnamed && child.children.is_empty() {
                if let Some(parent) = owner_key.as_deref() {
                    let fk = filler_key(parent, i);
                    self.insert_value(&fk, child);
                    continue;
                }
            }
            self.init_decl_h(
                child,
                dims,
                quals,
                scope,
                inherited_global || decl.is_global,
                origin,
            );
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
                self.init_edited(
                    upper,
                    &pic.template,
                    decl.value.as_ref(),
                    decl.blank_when_zero,
                );
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
                self.field_caps
                    .insert(upper.to_string(), (int_digits, decimals));
            }
        }
        self.store.insert(upper.to_string(), value);
    }

    // ── Hierarchy / occurrence accessors ────────────────────────────────────

    /// OCCURS dimensions of a (table) item; empty for a non-table item.
    pub fn dims_of(&self, name: &str) -> Vec<usize> {
        self.symbols
            .get(&name.to_ascii_uppercase())
            .map(|s| s.dims.clone())
            .unwrap_or_default()
    }

    /// Immediate child item names of a group (for CORRESPONDING).
    pub fn children_of(&self, name: &str) -> Vec<String> {
        self.symbols
            .get(&name.to_ascii_uppercase())
            .map(|s| s.children.clone())
            .unwrap_or_default()
    }

    /// The symbol-table entry for an item, if declared.
    pub fn symbol(&self, name: &str) -> Option<&ItemSym> {
        self.symbols.get(&name.to_ascii_uppercase())
    }

    /// Snapshot all symbol metadata entries for a temporary local scope.
    pub fn symbol_entries(&self) -> Vec<(String, ItemSym)> {
        self.symbols
            .iter()
            .map(|(key, sym)| (key.clone(), sym.clone()))
            .collect()
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
        if id < 1 {
            return None;
        }
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

    // ── 66-level RENAMES ────────────────────────────────────────────────────────

    /// The covered elementary keys for `RENAMES from [THRU thru]` — the slice of
    /// `elem_order` spanning `from`'s first leaf to `thru`'s (or `from`'s) last.
    fn renames_range(&self, from: &str, thru: Option<&str>) -> Option<Vec<String>> {
        let (lo, _) = self.leaf_span(from)?;
        let (_, hi) = self.leaf_span(thru.unwrap_or(from))?;
        if hi >= lo {
            Some(self.elem_order[lo..=hi].to_vec())
        } else {
            None
        }
    }

    /// The `[first, last]` `elem_order` index span of an item: itself if
    /// elementary, otherwise the range over all its descendant leaves.
    fn leaf_span(&self, key: &str) -> Option<(usize, usize)> {
        if let Some(p) = self.elem_order.iter().position(|k| k == key) {
            return Some((p, p));
        }
        // A group: collect its descendant leaf positions via the child tree.
        let mut positions = Vec::new();
        self.collect_leaf_positions(key, &mut positions);
        let lo = *positions.iter().min()?;
        let hi = *positions.iter().max()?;
        Some((lo, hi))
    }

    fn collect_leaf_positions(&self, key: &str, out: &mut Vec<usize>) {
        if let Some(p) = self.elem_order.iter().position(|k| k == key) {
            out.push(p);
            return;
        }
        if let Some(sym) = self.symbols.get(key) {
            for ck in &sym.child_keys {
                self.collect_leaf_positions(ck, out);
            }
        }
    }

    /// `true` if `name` is a 66-level RENAMES item.
    pub fn is_renames(&self, name: &str) -> bool {
        self.renames.contains_key(&name.to_ascii_uppercase())
    }

    // ── Group items ───────────────────────────────────────────────────────────
    //
    // A group item is **not** a slot of its own that happens to sit above other
    // slots: in COBOL-85 it *is* its subordinate items, laid end to end, and it
    // is alphanumeric whatever its children are. Its size is the sum of theirs.
    //
    // Before this, a group had an independent slot that nothing kept in step
    // with the children, so `DISPLAY <group>` printed whatever had been moved
    // to the group itself — usually nothing — while the children held the real
    // data, and `MOVE … TO <group>` left every child untouched.
    //
    // Reads synthesize and writes distribute, exactly as 66 RENAMES already
    // did; the group's own slot is no longer consulted for either.

    /// `true` if `name` is a group item — one with subordinate **data** items.
    ///
    /// Not [`ItemSym::is_group`], which is `!decl.children.is_empty()` and so is
    /// also true for an elementary item carrying 88-level condition-names. Those
    /// are not data items and hold no bytes: `01 WS-GRADE PIC 9(3). 88 PASSING
    /// VALUE 60 THRU 100.` is an ordinary numeric field, and treating it as a
    /// group made it read back as the empty concatenation of no children.
    pub fn is_group(&self, name: &str) -> bool {
        self.symbols
            .get(&name.to_ascii_uppercase())
            .map(|s| s.is_group && !s.layout_keys.is_empty())
            .unwrap_or(false)
    }

    /// The synthesized value of a group: its subordinate items' display strings
    /// concatenated in declaration order. `None` when `name` is not a group.
    ///
    /// Nested groups fold in through [`display_string`], which routes a group
    /// back here — so a group of groups reads as the whole flattened record.
    pub fn group_value(&self, name: &str) -> Option<String> {
        let sym = self.symbols.get(&name.to_ascii_uppercase())?;
        // `layout_keys` empty ⇒ no subordinate data items ⇒ elementary, whatever
        // `is_group` says (88-level condition-names count as children there).
        if !sym.is_group || sym.layout_keys.is_empty() {
            return None;
        }
        let mut out = String::new();
        for ck in &sym.layout_keys {
            out.push_str(&self.display_string(ck).unwrap_or_default());
        }
        Some(out)
    }

    /// The stored width of one item in bytes — its rendered, fixed-width form.
    /// For a group that is the sum of its children, by the same recursion.
    fn item_width(&self, key: &str) -> usize {
        self.display_string(key).map(|d| d.len()).unwrap_or(0)
    }

    /// Distribute `s` across a group's subordinate items by each one's stored
    /// width, left to right — a group move is alphanumeric, so the bytes land
    /// where they fall and each child is padded to its own width.
    ///
    /// Recurses into subordinate groups so a nested record fills correctly.
    pub fn set_group(&mut self, name: &str, s: &str) {
        let key = name.to_ascii_uppercase();
        let Some(sym) = self.symbols.get(&key) else {
            return;
        };
        if !sym.is_group || sym.layout_keys.is_empty() {
            return; // elementary (see `is_group` on 88-level children)
        }
        let child_keys = sym.layout_keys.clone();
        let bytes = s.as_bytes();
        let mut pos = 0usize;
        for ck in child_keys {
            let width = self.item_width(&ck).max(1);
            let end = (pos + width).min(bytes.len());
            let chunk = if pos < bytes.len() {
                String::from_utf8_lossy(&bytes[pos..end]).into_owned()
            } else {
                String::new()
            };
            let padded = format!("{chunk:<width$}");
            if self.is_group(&ck) {
                self.set_group(&ck, &padded);
            } else if self.field_caps.contains_key(&ck) {
                // Numeric receiver: keep the digits the bytes spell out. A slice
                // that is not a number (spaces from a short sender) leaves the
                // child alone rather than zeroing it.
                if let Ok(n) = padded.trim().parse::<i128>() {
                    self.set(&ck, CobolValue::from_i64(n as i64));
                }
            } else {
                self.set_str(&ck, &padded);
            }
            pos += width;
        }
    }

    /// The synthesized value of a RENAMES item: the concatenated display strings
    /// of the items it covers.
    pub fn renames_value(&self, name: &str) -> Option<String> {
        let covered = self.renames.get(&name.to_ascii_uppercase())?;
        let mut s = String::new();
        for k in covered {
            s.push_str(&self.display_string(k).unwrap_or_default());
        }
        Some(s)
    }

    /// Distribute `s` across the covered items of a RENAMES item by each item's
    /// stored width (left-to-right).
    pub fn set_renames(&mut self, name: &str, s: &str) {
        let Some(covered) = self.renames.get(&name.to_ascii_uppercase()).cloned() else {
            return;
        };
        let bytes = s.as_bytes();
        let mut pos = 0usize;
        for k in covered {
            let width = self.display_string(&k).map(|d| d.len()).unwrap_or(0).max(1);
            let end = (pos + width).min(bytes.len());
            let chunk = if pos < bytes.len() {
                String::from_utf8_lossy(&bytes[pos..end]).into_owned()
            } else {
                String::new()
            };
            // Pad the chunk to the field width.
            let padded = format!("{chunk:<width$}");
            if self.field_caps.contains_key(&k) {
                // numeric receiver — store the digits
                if let Ok(n) = padded.trim().parse::<i128>() {
                    self.set(&k, CobolValue::from_i64(n as i64));
                }
            } else {
                self.set_str(&k, &padded);
            }
            pos += width;
        }
    }

    /// Initialise a numeric-edited item: remember its template and store the
    /// edited string form of any VALUE (or spaces when there is none).
    fn init_edited(
        &mut self,
        name: &str,
        template: &str,
        value: Option<&Literal>,
        blank_when_zero: bool,
    ) {
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
            Some(Literal::Decimal(m, s)) => {
                CobolValue::from_str(&crate::numedit::format_edited(template, *m, *s, dc), width)
            }
            _ => CobolValue::spaces(width),
        };
        self.edited_templates
            .insert(name.to_string(), template.to_string());
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
        self.field_caps
            .get(&key)
            .or_else(|| self.field_caps.get(base_name(&key)))
            .map(|(d, _)| *d)
    }

    /// The de-edited character form of a plain numeric field for a MOVE to an
    /// alphanumeric receiver: absolute zero-padded digits, no sign, no point.
    /// `None` if the item isn't a plain numeric.
    pub fn deedited_digits(&self, name: &str) -> Option<String> {
        let key = name.to_ascii_uppercase();
        let (int_digits, _) = *self
            .field_caps
            .get(&key)
            .or_else(|| self.field_caps.get(base_name(&key)))?;
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
        if self.renames.contains_key(&key) {
            return self.renames_value(&key);
        }
        // A group IS its children (see the group-item section above), so it is
        // read from them and never from its own slot.
        if let Some(g) = self.group_value(&key) {
            return Some(g);
        }
        let val = self.get(&key)?;
        if let CobolValue::Numeric(n) = val {
            if let Some(&(int_digits, _)) = self
                .field_caps
                .get(&key)
                .or_else(|| self.field_caps.get(base_name(&key)))
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
                CobolValue::Float(f) => Some(CobolNumeric::new((*f * 1e9_f64).round() as i128, 9)),
                other => other.as_exact(),
            };
            if let Some(num) = num {
                let dc = self.decimal_comma;
                let width = crate::numedit::edited_width(&template, dc);
                let edited = if self.blank_when_zero.contains(base_name(&key)) && num.mantissa == 0
                {
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
    /// Every addressable data item and its value.
    ///
    /// Unnamed `FILLER` slots are left out: they exist so a group can render its
    /// own bytes, they carry a synthetic key no COBOL statement can name, and a
    /// developer watching variables in the debugger has no use for them.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &CobolValue)> {
        self.store.iter().filter(|(k, _)| !is_filler_key(k))
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
    pub fn push_local_scope(
        &mut self,
        items: &[(String, CobolValue)],
        symbols: &[(String, ItemSym)],
    ) -> Vec<String> {
        let mut inserted = Vec::with_capacity(items.len());
        for (key, val) in items {
            let upper = key.to_ascii_uppercase();
            if !self.store.contains_key(&upper) {
                self.store.insert(upper.clone(), val.clone());
                if let Some((_, sym)) = symbols
                    .iter()
                    .find(|(sym_key, _)| sym_key.eq_ignore_ascii_case(&upper))
                {
                    self.symbols.insert(upper.clone(), sym.clone());
                }
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
            self.symbols.shift_remove(key);
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
            let val = if let Some(lit) = &decl.value {
                apply_literal(lit, &val)
            } else {
                val
            };
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
            CobolValue::String { capacity, .. } => CobolValue::from_str(&n.to_string(), *capacity),
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
                FigurativeConstant::Space => CobolValue::spaces(cap),
                // ZERO must preserve the receiving field's PIC scale — a numeric
                // field keeps its decimal places (a scale-0 zero would wipe them).
                FigurativeConstant::Zero => match default {
                    CobolValue::Numeric(n) => CobolValue::Numeric(CobolNumeric::new(0, n.decimals)),
                    CobolValue::String { capacity, .. } => CobolValue::String {
                        bytes: vec![b'0'; *capacity],
                        capacity: *capacity,
                    },
                    _ => CobolValue::zero(0),
                },
                FigurativeConstant::HighValue => CobolValue::figurative_high_values(cap),
                FigurativeConstant::LowValue => CobolValue::figurative_low_values(cap),
                _ => default.clone(),
            }
        }
    }
}

/// Copy initialized bytes from the redefined item to the redefining item.
fn sync_redefines(env: &mut CobolEnvironment, data: &DataDivision) {
    let mut redefines_list = Vec::new();

    fn find_redefines(decl: &DataDecl, list: &mut Vec<(DataDecl, String)>) {
        if let Some(ref tgt) = decl.redefines {
            list.push((decl.clone(), tgt.to_ascii_uppercase()));
        }
        for c in &decl.children {
            find_redefines(c, list);
        }
    }

    for section in &data.sections {
        match section {
            DataSection::WorkingStorage(items)
            | DataSection::LocalStorage(items)
            | DataSection::Linkage(items) => {
                for decl in items {
                    find_redefines(decl, &mut redefines_list);
                }
            }
            DataSection::FileSection(fds) => {
                for fd in fds {
                    for rec in &fd.records {
                        find_redefines(rec, &mut redefines_list);
                    }
                }
            }
            _ => {}
        }
    }

    for (redefining_decl, target_name) in redefines_list {
        if let Some(target_decl) = find_decl_by_name(data, &target_name) {
            let mut bytes = Vec::new();
            serialize_decl(
                env,
                &target_decl,
                &mut Vec::new(),
                &mut Vec::new(),
                &mut bytes,
            );

            let mut offset = 0;
            deserialize_decl(
                env,
                &redefining_decl,
                &mut Vec::new(),
                &mut Vec::new(),
                &bytes,
                &mut offset,
            );
        }
    }
}

fn find_decl_by_name(data: &DataDivision, name: &str) -> Option<DataDecl> {
    fn search(decl: &DataDecl, name: &str) -> Option<DataDecl> {
        if let Some(n) = &decl.name {
            if n.to_ascii_uppercase() == name {
                return Some(decl.clone());
            }
        }
        for c in &decl.children {
            if let Some(found) = search(c, name) {
                return Some(found);
            }
        }
        None
    }
    for section in &data.sections {
        match section {
            DataSection::WorkingStorage(items)
            | DataSection::LocalStorage(items)
            | DataSection::Linkage(items) => {
                for decl in items {
                    if let Some(found) = search(decl, name) {
                        return Some(found);
                    }
                }
            }
            DataSection::FileSection(fds) => {
                for fd in fds {
                    for rec in &fd.records {
                        if let Some(found) = search(rec, name) {
                            return Some(found);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn serialize_decl(
    env: &CobolEnvironment,
    decl: &DataDecl,
    quals: &mut Vec<String>,
    indices: &mut Vec<usize>,
    bytes: &mut Vec<u8>,
) {
    let times = decl
        .occurs
        .as_ref()
        .map(|o| o.max.max(1) as usize)
        .unwrap_or(1);
    let name_upper = decl
        .name
        .as_ref()
        .map(|n| n.to_ascii_uppercase())
        .unwrap_or_else(|| "FILLER".to_string());

    for i in 1..=times {
        let mut local_indices = indices.clone();
        if decl.occurs.is_some() {
            local_indices.push(i);
        }
        let mut local_quals = quals.clone();
        if name_upper != "FILLER" {
            local_quals.push(name_upper.clone());
        }

        if !decl.children.is_empty() {
            for c in &decl.children {
                if c.level == 88 {
                    continue;
                }
                serialize_decl(env, c, &mut local_quals, &mut local_indices, bytes);
            }
        } else if let Some(pic) = &decl.picture {
            let len = (pic.digits as usize + pic.decimals as usize).max(1);
            let numeric = matches!(pic.kind, PicKind::Numeric | PicKind::NumericEdited);

            let key = env.canon_key(&name_upper, quals);
            let key = if !local_indices.is_empty() {
                let idx_i64: Vec<i64> = local_indices.iter().map(|&x| x as i64).collect();
                subscript_key(&key, &idx_i64)
            } else {
                key
            };

            let val = env.store.get(&key);
            let mut f_bytes = vec![b' '; len];

            if let Some(v) = val {
                match v {
                    CobolValue::Numeric(n) => {
                        let digits = n.mantissa.unsigned_abs().to_string();
                        let mut s = if digits.len() < len {
                            format!("{}{}", "0".repeat(len - digits.len()), digits)
                        } else {
                            digits
                        };
                        if s.len() > len {
                            s = s[s.len() - len..].to_string();
                        }
                        f_bytes = s.into_bytes();
                    }
                    other => {
                        let mut b = other.as_display_string().into_bytes();
                        b.resize(len, b' ');
                        f_bytes = b;
                    }
                }
            } else {
                let mut fallback_v = if numeric {
                    CobolValue::Numeric(CobolNumeric::new(
                        0,
                        pic.decimals.min(u8::MAX as u16) as u8,
                    ))
                } else {
                    CobolValue::spaces(len)
                };
                if let Some(lit) = &decl.value {
                    fallback_v = apply_literal(lit, &fallback_v);
                }
                match fallback_v {
                    CobolValue::Numeric(n) => {
                        let digits = n.mantissa.unsigned_abs().to_string();
                        let mut s = if digits.len() < len {
                            format!("{}{}", "0".repeat(len - digits.len()), digits)
                        } else {
                            digits
                        };
                        if s.len() > len {
                            s = s[s.len() - len..].to_string();
                        }
                        f_bytes = s.into_bytes();
                    }
                    other => {
                        let mut b = other.as_display_string().into_bytes();
                        b.resize(len, b' ');
                        f_bytes = b;
                    }
                }
            }

            bytes.extend_from_slice(&f_bytes);
        }
    }
}

fn deserialize_decl(
    env: &mut CobolEnvironment,
    decl: &DataDecl,
    quals: &mut Vec<String>,
    indices: &mut Vec<usize>,
    bytes: &[u8],
    offset: &mut usize,
) {
    let times = decl
        .occurs
        .as_ref()
        .map(|o| o.max.max(1) as usize)
        .unwrap_or(1);
    let name_upper = decl
        .name
        .as_ref()
        .map(|n| n.to_ascii_uppercase())
        .unwrap_or_else(|| "FILLER".to_string());

    for i in 1..=times {
        let mut local_indices = indices.clone();
        if decl.occurs.is_some() {
            local_indices.push(i);
        }
        let mut local_quals = quals.clone();
        if name_upper != "FILLER" {
            local_quals.push(name_upper.clone());
        }

        if !decl.children.is_empty() {
            for c in &decl.children {
                if c.level == 88 {
                    continue;
                }
                deserialize_decl(env, c, &mut local_quals, &mut local_indices, bytes, offset);
            }
        } else if let Some(pic) = &decl.picture {
            let len = (pic.digits as usize + pic.decimals as usize).max(1);
            let numeric = matches!(pic.kind, PicKind::Numeric | PicKind::NumericEdited);

            let end = (*offset + len).min(bytes.len());
            let slice = &bytes[*offset..end];
            *offset += len;

            if name_upper != "FILLER" {
                let key = env.canon_key(&name_upper, quals);
                let key = if !local_indices.is_empty() {
                    let idx_i64: Vec<i64> = local_indices.iter().map(|&x| x as i64).collect();
                    subscript_key(&key, &idx_i64)
                } else {
                    key
                };

                if numeric {
                    let digits: String = slice
                        .iter()
                        .map(|&b| if b.is_ascii_digit() { b as char } else { '0' })
                        .collect();
                    let mantissa: i128 = digits.parse().unwrap_or(0);
                    let decimals = pic.decimals.min(u8::MAX as u16) as u8;
                    env.set(
                        &key,
                        CobolValue::Numeric(CobolNumeric::new(mantissa, decimals)),
                    );
                } else {
                    env.set_str(&key, &String::from_utf8_lossy(slice));
                }
            }
        }
    }
}
