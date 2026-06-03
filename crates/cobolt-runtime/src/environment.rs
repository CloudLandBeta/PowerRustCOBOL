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
    data::{DataDecl, PicKind, Usage},
    expr::Literal,
    program::{DataSection, DataDivision},
};

use crate::value::{CobolNumeric, CobolValue};

// ── CobolEnvironment ──────────────────────────────────────────────────────────

/// The data store for a running COBOL program.
///
/// Data items are addressed by their COBOL name (uppercase, hyphens preserved).
/// Items that have not been initialised hold `CobolValue::Unset`.
#[derive(Debug, Default)]
pub struct CobolEnvironment {
    /// `name → value` store.  Insertion order is preserved (declaration order).
    store: IndexMap<String, CobolValue>,
    /// `name → (integer-digit capacity, decimal places)` for numeric items,
    /// used to detect ON SIZE ERROR overflow at store time.
    field_caps: IndexMap<String, (u8, u8)>,
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
        let mut env = Self::new();
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

    /// Recursively initialise a data declaration and its children.
    fn init_decl(&mut self, decl: &DataDecl) {
        if let Some(name) = &decl.name {
            // Skip FILLER (anonymous items)
            let upper = name.to_ascii_uppercase();
            if upper != "FILLER" {
                let default = default_value(decl);
                let value = if let Some(lit) = &decl.value {
                    apply_literal(lit, &default)
                } else {
                    default
                };
                // Record numeric integer-digit capacity for ON SIZE ERROR checks.
                if let Some(pic) = &decl.picture {
                    if matches!(pic.kind, PicKind::Numeric | PicKind::NumericEdited) {
                        let int_digits = pic.digits.min(u8::MAX as u16) as u8;
                        let decimals = pic.decimals.min(u8::MAX as u16) as u8;
                        self.field_caps.insert(upper.clone(), (int_digits, decimals));
                    }
                }
                self.store.insert(upper, value);
            }
        }
        for child in &decl.children {
            self.init_decl(child);
        }
    }

    // ── Data access ───────────────────────────────────────────────────────────

    /// Get an immutable reference to a data item's value.
    pub fn get(&self, name: &str) -> Option<&CobolValue> {
        self.store.get(&name.to_ascii_uppercase())
    }

    /// Integer-digit capacity of a numeric field, if known (for ON SIZE ERROR).
    pub fn integer_capacity(&self, name: &str) -> Option<u8> {
        self.field_caps.get(&name.to_ascii_uppercase()).map(|(d, _)| *d)
    }

    /// Render a data item for `DISPLAY`. A USAGE-DISPLAY numeric item is shown as
    /// its full fixed-width digit string — leading zeros to the PIC width, the
    /// implied decimal point (`V`) not shown, and a leading `-` for negatives —
    /// i.e. the characters as they are stored. Non-numeric items render verbatim.
    pub fn display_string(&self, name: &str) -> Option<String> {
        let key = name.to_ascii_uppercase();
        let val = self.store.get(&key)?;
        if let CobolValue::Numeric(n) = val {
            if let Some(&(int_digits, _)) = self.field_caps.get(&key) {
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
        if let Some(existing) = self.store.get_mut(&key) {
            existing.assign(&value);
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
    // Group items with no PIC → treat as spaces of total subordinate width
    if decl.picture.is_none() {
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
