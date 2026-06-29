// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `ObjectRegistry` — PowerRustCOBOL form and control object store.
//!
//! In PowerCOBOL 3.0 every form, button, text-box, list-box, etc. is an
//! *object* with named string properties.  This module provides the runtime
//! equivalent: a map of object name → property map.
//!
//! `EXEC RUST` blocks receive a `&mut ObjectRegistry` called
//! `cobolt_objects` so they can read and write form properties directly
//! from Rust code:
//!
//! ```text
//! EXEC RUST
//!     cobolt_objects.get_mut("FORM1")
//!         .unwrap()
//!         .set_property("Caption", "Hello!");
//! END-EXEC.
//! ```

use indexmap::IndexMap;

// ── CoboltObject ──────────────────────────────────────────────────────────────

/// A form or control object.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct CoboltObject {
    /// The object class name (`"Form"`, `"Button"`, `"TextBox"`, …).
    pub class: String,
    /// Named properties (`"Caption"`, `"Text"`, `"Visible"`, `"Enabled"`, …).
    properties: IndexMap<String, PropertyValue>,
}

impl CoboltObject {
    pub fn new(class: impl Into<String>) -> Self {
        Self {
            class: class.into(),
            properties: IndexMap::new(),
        }
    }

    /// Read a property value. Property names are **case-insensitive** (COBOL
    /// identifiers are; the inline `::Caption` arrives upper-cased while designer/
    /// explicit-method values may use mixed case), so keys are canonicalised to
    /// upper-case (spec 010).
    pub fn get_property(&self, name: &str) -> Option<&PropertyValue> {
        self.properties.get(&name.to_ascii_uppercase())
    }

    /// Write a property value (case-insensitive key — see [`get_property`]).
    pub fn set_property(&mut self, name: impl Into<String>, value: impl Into<PropertyValue>) {
        self.properties
            .insert(name.into().to_ascii_uppercase(), value.into());
    }

    /// Convenience: read a property as a `String`.
    pub fn get_str(&self, name: &str) -> Option<&str> {
        match self.properties.get(&name.to_ascii_uppercase())? {
            PropertyValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Convenience: read a property as a `bool`.
    pub fn get_bool(&self, name: &str) -> Option<bool> {
        match self.properties.get(&name.to_ascii_uppercase())? {
            PropertyValue::Bool(b) => Some(*b),
            PropertyValue::Integer(n) => Some(*n != 0),
            _ => None,
        }
    }

    /// Convenience: read a property as an `i64`.
    pub fn get_i64(&self, name: &str) -> Option<i64> {
        match self.properties.get(&name.to_ascii_uppercase())? {
            PropertyValue::Integer(n) => Some(*n),
            PropertyValue::Bool(b) => Some(*b as i64),
            _ => None,
        }
    }
}

// ── PropertyValue ─────────────────────────────────────────────────────────────

/// The value of an object property.
///
/// Beyond the scalar variants, a property may hold a **nested object** (a grid
/// row, a cell, …) or an indexable **list** (a control's `Rows`, `Columns`,
/// `Items`). This is what makes member-access chains such as
/// `Grid-1::Rows(I)::Columns(2)::Value` navigate real structure rather than
/// flattened keys (spec 011).
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    /// A nested object — its own named-property map.
    Object(Box<CoboltObject>),
    /// An ordered, indexable collection of values.
    List(Vec<PropertyValue>),
}

impl From<&str> for PropertyValue {
    fn from(s: &str) -> Self {
        PropertyValue::String(s.to_owned())
    }
}
impl From<String> for PropertyValue {
    fn from(s: String) -> Self {
        PropertyValue::String(s)
    }
}
impl From<i64> for PropertyValue {
    fn from(n: i64) -> Self {
        PropertyValue::Integer(n)
    }
}
impl From<i32> for PropertyValue {
    fn from(n: i32) -> Self {
        PropertyValue::Integer(n as i64)
    }
}
impl From<f64> for PropertyValue {
    fn from(f: f64) -> Self {
        PropertyValue::Float(f)
    }
}
impl From<bool> for PropertyValue {
    fn from(b: bool) -> Self {
        PropertyValue::Bool(b)
    }
}

impl std::fmt::Display for PropertyValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropertyValue::String(s) => write!(f, "{s}"),
            PropertyValue::Integer(n) => write!(f, "{n}"),
            PropertyValue::Float(v) => write!(f, "{v}"),
            PropertyValue::Bool(b) => write!(f, "{b}"),
            // A list renders as its elements joined by newlines (matching the
            // legacy newline-joined `Items` string form); a nested object as its
            // class name in angle brackets (it has no single scalar rendering).
            PropertyValue::List(items) => {
                let joined = items
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                write!(f, "{joined}")
            }
            PropertyValue::Object(o) => write!(f, "<{}>", o.class),
        }
    }
}

// ── ObjectRegistry ────────────────────────────────────────────────────────────

/// Registry of all objects in a running program.
///
/// Keyed by object name (case-insensitive, e.g. `"FORM1"`, `"BTN-OK"`).
#[derive(Debug, Default)]
pub struct ObjectRegistry {
    objects: IndexMap<String, CoboltObject>,
}

impl ObjectRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new object.
    pub fn register(&mut self, name: impl Into<String>, class: impl Into<String>) {
        let key = name.into().to_ascii_uppercase();
        self.objects.insert(key, CoboltObject::new(class));
    }

    /// Get an immutable reference to an object by name.
    pub fn get(&self, name: &str) -> Option<&CoboltObject> {
        self.objects.get(&name.to_ascii_uppercase())
    }

    /// Get a mutable reference to an object by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut CoboltObject> {
        self.objects.get_mut(&name.to_ascii_uppercase())
    }

    /// `true` if an object with the given name is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.objects.contains_key(&name.to_ascii_uppercase())
    }

    /// Set a property on a named object.  No-op if the object doesn't exist.
    pub fn set_property(&mut self, obj: &str, prop: &str, value: impl Into<PropertyValue>) {
        if let Some(o) = self.get_mut(obj) {
            o.set_property(prop, value);
        }
    }

    /// Get a property from a named object.
    pub fn get_property(&self, obj: &str, prop: &str) -> Option<&PropertyValue> {
        self.get(obj)?.get_property(prop)
    }

    /// Iterate all registered objects.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &CoboltObject)> {
        self.objects.iter()
    }

    // ── Member-access path navigation (spec 011) ──────────────────────────────

    /// Read the value at `path` under object `obj` (a clone). `None` if any
    /// segment is missing. A trailing `Index` into a `String` property reads the
    /// n-th newline-delimited line (interop with the legacy `Items` form).
    pub fn get_path(&self, obj: &str, path: &[PathSeg]) -> Option<PropertyValue> {
        get_in_obj(self.get(obj)?, path)
    }

    /// Write `val` at `path` under object `obj`, registering the object and
    /// creating intermediate objects/lists (and growing lists) as needed. A
    /// trailing `Index` into a `String` property replaces the n-th line.
    pub fn set_path(&mut self, obj: &str, path: &[PathSeg], val: PropertyValue) {
        if !self.contains(obj) {
            self.register(obj, "Control");
        }
        if let Some(o) = self.get_mut(obj) {
            set_in_obj(o, path, val);
        }
    }

    /// Remove the element addressed by `path` (which must end in an `Index`) from
    /// its parent list, returning it. `None` if the parent is not a list or the
    /// index is out of range.
    pub fn remove_path(&mut self, obj: &str, path: &[PathSeg]) -> Option<PropertyValue> {
        let o = self.get_mut(obj)?;
        let (last, parent) = path.split_last()?;
        let PathSeg::Index(i) = last else { return None };
        let place = place_in_obj_mut(o, parent, false)?;
        match place {
            PropertyValue::List(items) if *i < items.len() => Some(items.remove(*i)),
            // Legacy string list: drop the n-th line.
            PropertyValue::String(s) => {
                let mut lines: Vec<&str> = s.lines().collect();
                if *i < lines.len() {
                    let removed = lines.remove(*i).to_string();
                    *s = lines.join("\n");
                    Some(PropertyValue::String(removed))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Number of elements in the list (or lines in the legacy string) at `path`.
    pub fn path_len(&self, obj: &str, path: &[PathSeg]) -> Option<usize> {
        match get_in_obj(self.get(obj)?, path)? {
            PropertyValue::List(items) => Some(items.len()),
            PropertyValue::String(s) if s.trim().is_empty() => Some(0),
            PropertyValue::String(s) => Some(s.lines().count()),
            _ => None,
        }
    }
}

// ── Path segments + free navigation helpers ─────────────────────────────────

/// One step in a member-access path: a named property or a list index.
/// `Grid::Rows(I)::Columns(2)::Value` lowers to
/// `[Prop("Rows"), Index(I), Prop("Columns"), Index(2), Prop("Value")]`.
#[derive(Debug, Clone, PartialEq)]
pub enum PathSeg {
    Prop(String),
    Index(usize),
}

/// Read `path` starting from an object's property map.
fn get_in_obj(o: &CoboltObject, path: &[PathSeg]) -> Option<PropertyValue> {
    let (first, rest) = path.split_first()?;
    // A path under an object must begin with a property name.
    let PathSeg::Prop(name) = first else {
        return None;
    };
    let cur = o.get_property(name)?;
    get_in_value(cur, rest)
}

/// Read the remaining `path` from within a property value.
fn get_in_value(cur: &PropertyValue, path: &[PathSeg]) -> Option<PropertyValue> {
    let Some((seg, rest)) = path.split_first() else {
        return Some(cur.clone());
    };
    match seg {
        PathSeg::Index(i) => match cur {
            PropertyValue::List(items) => get_in_value(items.get(*i)?, rest),
            // Legacy string list: index a newline-delimited line (terminal).
            PropertyValue::String(s) if rest.is_empty() => s
                .lines()
                .nth(*i)
                .map(|l| PropertyValue::String(l.to_string())),
            _ => None,
        },
        PathSeg::Prop(_) => match cur {
            PropertyValue::Object(o) => get_in_obj(o, path),
            _ => None,
        },
    }
}

/// Write `val` at `path` from an object's property map, creating containers.
fn set_in_obj(o: &mut CoboltObject, path: &[PathSeg], val: PropertyValue) {
    if let Some(place) = place_in_obj_mut(o, path, true) {
        *place = val;
    }
}

/// Resolve `path` to a mutable property slot, optionally vivifying intermediate
/// objects/lists (and growing lists). The path must begin with a property name.
fn place_in_obj_mut<'a>(
    o: &'a mut CoboltObject,
    path: &[PathSeg],
    vivify: bool,
) -> Option<&'a mut PropertyValue> {
    let PathSeg::Prop(name) = path.first()? else {
        return None;
    };
    let key = name.to_ascii_uppercase();
    if vivify && o.get_property(&key).is_none() {
        let seed = match path.get(1) {
            Some(PathSeg::Index(_)) => PropertyValue::List(Vec::new()),
            Some(PathSeg::Prop(_)) => PropertyValue::Object(Box::new(CoboltObject::new("Object"))),
            None => PropertyValue::String(String::new()),
        };
        o.set_property(&key, seed);
    }
    let cur = o.properties.get_mut(&key)?;
    place_in_value_mut(cur, &path[1..], vivify)
}

/// Resolve the remaining `path` to a mutable slot inside a property value.
fn place_in_value_mut<'a>(
    cur: &'a mut PropertyValue,
    path: &[PathSeg],
    vivify: bool,
) -> Option<&'a mut PropertyValue> {
    let Some((seg, rest)) = path.split_first() else {
        return Some(cur);
    };
    match seg {
        PathSeg::Index(i) => {
            // Vivify a bare/empty slot into a list before indexing.
            if vivify && !matches!(cur, PropertyValue::List(_)) {
                if matches!(cur, PropertyValue::String(s) if s.is_empty()) {
                    *cur = PropertyValue::List(Vec::new());
                }
            }
            let PropertyValue::List(items) = cur else {
                return None;
            };
            if vivify {
                while items.len() <= *i {
                    items.push(PropertyValue::String(String::new()));
                }
            }
            place_in_value_mut(items.get_mut(*i)?, rest, vivify)
        }
        PathSeg::Prop(_) => {
            if vivify && !matches!(cur, PropertyValue::Object(_)) {
                if matches!(cur, PropertyValue::String(s) if s.is_empty()) {
                    *cur = PropertyValue::Object(Box::new(CoboltObject::new("Object")));
                }
            }
            let PropertyValue::Object(o) = cur else {
                return None;
            };
            place_in_obj_mut(o, path, vivify)
        }
    }
}
