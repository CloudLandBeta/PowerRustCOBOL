// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `ObjectRegistry` — PowerCOBOL form and control object store.
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

/// A PowerCOBOL form or control object.
#[derive(Debug, Default, Clone)]
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

    /// Read a property value.
    pub fn get_property(&self, name: &str) -> Option<&PropertyValue> {
        self.properties.get(name)
    }

    /// Write a property value.
    pub fn set_property(&mut self, name: impl Into<String>, value: impl Into<PropertyValue>) {
        self.properties.insert(name.into(), value.into());
    }

    /// Convenience: read a property as a `String`.
    pub fn get_str(&self, name: &str) -> Option<&str> {
        match self.properties.get(name)? {
            PropertyValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Convenience: read a property as a `bool`.
    pub fn get_bool(&self, name: &str) -> Option<bool> {
        match self.properties.get(name)? {
            PropertyValue::Bool(b) => Some(*b),
            PropertyValue::Integer(n) => Some(*n != 0),
            _ => None,
        }
    }

    /// Convenience: read a property as an `i64`.
    pub fn get_i64(&self, name: &str) -> Option<i64> {
        match self.properties.get(name)? {
            PropertyValue::Integer(n) => Some(*n),
            PropertyValue::Bool(b) => Some(*b as i64),
            _ => None,
        }
    }
}

// ── PropertyValue ─────────────────────────────────────────────────────────────

/// The value of an object property.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
}

impl From<&str> for PropertyValue {
    fn from(s: &str) -> Self { PropertyValue::String(s.to_owned()) }
}
impl From<String> for PropertyValue {
    fn from(s: String) -> Self { PropertyValue::String(s) }
}
impl From<i64> for PropertyValue {
    fn from(n: i64) -> Self { PropertyValue::Integer(n) }
}
impl From<i32> for PropertyValue {
    fn from(n: i32) -> Self { PropertyValue::Integer(n as i64) }
}
impl From<f64> for PropertyValue {
    fn from(f: f64) -> Self { PropertyValue::Float(f) }
}
impl From<bool> for PropertyValue {
    fn from(b: bool) -> Self { PropertyValue::Bool(b) }
}

impl std::fmt::Display for PropertyValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropertyValue::String(s)  => write!(f, "{s}"),
            PropertyValue::Integer(n) => write!(f, "{n}"),
            PropertyValue::Float(v)   => write!(f, "{v}"),
            PropertyValue::Bool(b)    => write!(f, "{b}"),
        }
    }
}

// ── ObjectRegistry ────────────────────────────────────────────────────────────

/// Registry of all PowerCOBOL objects in a running program.
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
}
