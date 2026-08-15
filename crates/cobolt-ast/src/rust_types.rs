// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The shipped Rust types a `REPOSITORY` may name (spec 005, spec 041).
//!
//! # Why this lives in `cobolt-ast`
//!
//! Two crates need the same answer to "is `Rust.HashMap` a real type?" and they
//! must never disagree:
//!
//! * `cobolt-forms` seeds a new form's `REPOSITORY` with these classes;
//! * `cobolt-semantic` checks that a `CLASS` names something real (spec 041
//!   R8), which is what stops `CLASS RUST-NOPE IS "Rust.Nope"` reaching codegen
//!   and failing later as a `rustc` error about generated code.
//!
//! `cobolt-forms` had the only copy and `cobolt-semantic` cannot see it, so the
//! table moved here — the crate both already sit above — rather than being
//! duplicated, which would drift the moment either side gained a type.
//!
//! # A floor, not a ceiling
//!
//! These are the types shipped ready to use. A developer adds their own by
//! declaring it in an item-level `EXEC RUST` block and naming it with a `CLASS`
//! (spec 041 R19/R22), so this list is where the set *starts*, not where it
//! ends. [`is_shipped_rust_type`] answers only for this list; the caller is
//! responsible for also accepting developer-defined types.

/// `(COBOL class name, Rust type path)` for every shipped type.
///
/// The path is the type's place in the Rust hierarchy, analogous to
/// `System.String` in .NET, so a data item can be
/// `USAGE OBJECT REFERENCE RUST-STRING`.
pub const SHIPPED_RUST_TYPES: &[(&str, &str)] = &[
    // ── Primitive (scalar) types ──────────────────────────────────────────
    ("RUST-BOOL", "Rust.bool"),
    ("RUST-CHAR", "Rust.char"),
    ("RUST-I8", "Rust.i8"),
    ("RUST-I16", "Rust.i16"),
    ("RUST-I32", "Rust.i32"),
    ("RUST-I64", "Rust.i64"),
    ("RUST-I128", "Rust.i128"),
    ("RUST-ISIZE", "Rust.isize"),
    ("RUST-U8", "Rust.u8"),
    ("RUST-U16", "Rust.u16"),
    ("RUST-U32", "Rust.u32"),
    ("RUST-U64", "Rust.u64"),
    ("RUST-U128", "Rust.u128"),
    ("RUST-USIZE", "Rust.usize"),
    ("RUST-F32", "Rust.f32"),
    ("RUST-F64", "Rust.f64"),
    ("RUST-STR", "Rust.str"),
    ("RUST-UNIT", "Rust.unit"),
    // ── Strings, text and paths ───────────────────────────────────────────
    ("RUST-STRING", "Rust.String"),
    ("RUST-OSSTRING", "Rust.OsString"),
    ("RUST-OSSTR", "Rust.OsStr"),
    ("RUST-CSTRING", "Rust.CString"),
    ("RUST-CSTR", "Rust.CStr"),
    ("RUST-PATH", "Rust.Path"),
    ("RUST-PATHBUF", "Rust.PathBuf"),
    // ── Collections ───────────────────────────────────────────────────────
    ("RUST-VEC", "Rust.Vec"),
    ("RUST-VECDEQUE", "Rust.VecDeque"),
    ("RUST-LINKEDLIST", "Rust.LinkedList"),
    ("RUST-HASHMAP", "Rust.HashMap"),
    ("RUST-BTREEMAP", "Rust.BTreeMap"),
    ("RUST-HASHSET", "Rust.HashSet"),
    ("RUST-BTREESET", "Rust.BTreeSet"),
    ("RUST-BINARYHEAP", "Rust.BinaryHeap"),
    // ── Core enums ────────────────────────────────────────────────────────
    ("RUST-OPTION", "Rust.Option"),
    ("RUST-RESULT", "Rust.Result"),
    // ── Smart pointers, cells and synchronisation ─────────────────────────
    ("RUST-BOX", "Rust.Box"),
    ("RUST-RC", "Rust.Rc"),
    ("RUST-ARC", "Rust.Arc"),
    ("RUST-WEAK", "Rust.Weak"),
    ("RUST-CELL", "Rust.Cell"),
    ("RUST-REFCELL", "Rust.RefCell"),
    ("RUST-MUTEX", "Rust.Mutex"),
    ("RUST-RWLOCK", "Rust.RwLock"),
    ("RUST-COW", "Rust.Cow"),
    // ── Time ──────────────────────────────────────────────────────────────
    ("RUST-DURATION", "Rust.Duration"),
    ("RUST-INSTANT", "Rust.Instant"),
    ("RUST-SYSTEMTIME", "Rust.SystemTime"),
    // ── Ranges ────────────────────────────────────────────────────────────
    ("RUST-RANGE", "Rust.Range"),
];

/// What a compiled `EXEC RUST` block binds a shipped class as: the Rust type it
/// appears as inside the block, and the expression that starts it off
/// (spec 041 R4, R7).
///
/// Both strings are **Rust source text**, emitted verbatim into the generated
/// crate — nothing here is compiled by `cobolt-ast` itself. They live beside
/// [`SHIPPED_RUST_TYPES`] so the two lists cannot drift; a test asserts every
/// shipped path has an entry.
///
/// # Why some classes bind wider than they read
///
/// Every integer width binds as `i64` and both floats as `f64`, because that is
/// how the curated bridge *stores* them: `INVOKE`/`::` dispatch on the same
/// object through `downcast_mut::<i64>()`. Binding `RUST-U8` as a real `u8`
/// would give the block a nicer type and make the very next `INVOKE` on that
/// item fail. One storage, one truth.
///
/// The unsized classes (`Rust.str`, `Rust.OsStr`, `Rust.CStr`, `Rust.Path`)
/// bind as their owned forms, since a data item owns its object and a `str`
/// cannot be owned.
const BLOCK_BINDINGS: &[(&str, &str, &str)] = &[
    // ── Primitive (scalar) types ──────────────────────────────────────────
    ("Rust.bool", "bool", "false"),
    ("Rust.char", "char", "'\\0'"),
    ("Rust.i8", "i64", "0"),
    ("Rust.i16", "i64", "0"),
    ("Rust.i32", "i64", "0"),
    ("Rust.i64", "i64", "0"),
    ("Rust.i128", "i64", "0"),
    ("Rust.isize", "i64", "0"),
    ("Rust.u8", "i64", "0"),
    ("Rust.u16", "i64", "0"),
    ("Rust.u32", "i64", "0"),
    ("Rust.u64", "i64", "0"),
    ("Rust.u128", "i64", "0"),
    ("Rust.usize", "i64", "0"),
    ("Rust.f32", "f64", "0.0"),
    ("Rust.f64", "f64", "0.0"),
    ("Rust.str", "String", "String::new()"),
    ("Rust.unit", "()", "()"),
    // ── Strings, text and paths ───────────────────────────────────────────
    ("Rust.String", "String", "String::new()"),
    ("Rust.OsString", "std::ffi::OsString", "std::ffi::OsString::new()"),
    ("Rust.OsStr", "std::ffi::OsString", "std::ffi::OsString::new()"),
    ("Rust.CString", "std::ffi::CString", "std::ffi::CString::default()"),
    ("Rust.CStr", "std::ffi::CString", "std::ffi::CString::default()"),
    ("Rust.Path", "std::path::PathBuf", "std::path::PathBuf::new()"),
    ("Rust.PathBuf", "std::path::PathBuf", "std::path::PathBuf::new()"),
    // ── Collections ───────────────────────────────────────────────────────
    // The element type is the bridge's own value enum, so a collection filled
    // by `INVOKE` and one filled inside a block hold the same things.
    (
        "Rust.Vec",
        "Vec<cobolt_runtime::rust_bridge::BridgeValue>",
        "Vec::new()",
    ),
    (
        "Rust.VecDeque",
        "std::collections::VecDeque<cobolt_runtime::rust_bridge::BridgeValue>",
        "std::collections::VecDeque::new()",
    ),
    (
        "Rust.LinkedList",
        "std::collections::LinkedList<cobolt_runtime::rust_bridge::BridgeValue>",
        "std::collections::LinkedList::new()",
    ),
    (
        "Rust.HashMap",
        "std::collections::HashMap<String, cobolt_runtime::rust_bridge::BridgeValue>",
        "std::collections::HashMap::new()",
    ),
    (
        "Rust.BTreeMap",
        "std::collections::BTreeMap<String, cobolt_runtime::rust_bridge::BridgeValue>",
        "std::collections::BTreeMap::new()",
    ),
    // A set needs `Hash`/`Ord` on its element, which the bridge value is not;
    // `String` is the useful set element in COBOL programs anyway.
    (
        "Rust.HashSet",
        "std::collections::HashSet<String>",
        "std::collections::HashSet::new()",
    ),
    (
        "Rust.BTreeSet",
        "std::collections::BTreeSet<String>",
        "std::collections::BTreeSet::new()",
    ),
    (
        "Rust.BinaryHeap",
        "std::collections::BinaryHeap<i64>",
        "std::collections::BinaryHeap::new()",
    ),
    // ── Core enums ────────────────────────────────────────────────────────
    (
        "Rust.Option",
        "Option<cobolt_runtime::rust_bridge::BridgeValue>",
        "None",
    ),
    (
        "Rust.Result",
        "Result<cobolt_runtime::rust_bridge::BridgeValue, String>",
        "Ok(cobolt_runtime::rust_bridge::BridgeValue::Null)",
    ),
    // ── Smart pointers, cells and synchronisation ─────────────────────────
    (
        "Rust.Box",
        "Box<cobolt_runtime::rust_bridge::BridgeValue>",
        "Box::new(cobolt_runtime::rust_bridge::BridgeValue::Null)",
    ),
    // 051 Q1 — `Rust.Rc`/`Rust.Weak` are ATOMICALLY counted underneath
    // (`Arc`/`sync::Weak`) since the object bridge became one-per-process:
    // a bridged value may be touched from any form's interpreter thread, so
    // non-atomic counts would be unsound (`Rc` is `!Send`). From COBOL the
    // classes behave exactly as before — shared ownership, counts, upgrade/
    // downgrade — the names stay, only the count got thread-safe.
    (
        "Rust.Rc",
        "std::sync::Arc<cobolt_runtime::rust_bridge::BridgeValue>",
        "std::sync::Arc::new(cobolt_runtime::rust_bridge::BridgeValue::Null)",
    ),
    (
        "Rust.Arc",
        "std::sync::Arc<cobolt_runtime::rust_bridge::BridgeValue>",
        "std::sync::Arc::new(cobolt_runtime::rust_bridge::BridgeValue::Null)",
    ),
    (
        "Rust.Weak",
        "std::sync::Weak<cobolt_runtime::rust_bridge::BridgeValue>",
        "std::sync::Weak::new()",
    ),
    // `Cell` needs `Copy` to be useful at all, so it holds the integer.
    ("Rust.Cell", "std::cell::Cell<i64>", "std::cell::Cell::new(0)"),
    (
        "Rust.RefCell",
        "std::cell::RefCell<cobolt_runtime::rust_bridge::BridgeValue>",
        "std::cell::RefCell::new(cobolt_runtime::rust_bridge::BridgeValue::Null)",
    ),
    (
        "Rust.Mutex",
        "std::sync::Mutex<cobolt_runtime::rust_bridge::BridgeValue>",
        "std::sync::Mutex::new(cobolt_runtime::rust_bridge::BridgeValue::Null)",
    ),
    (
        "Rust.RwLock",
        "std::sync::RwLock<cobolt_runtime::rust_bridge::BridgeValue>",
        "std::sync::RwLock::new(cobolt_runtime::rust_bridge::BridgeValue::Null)",
    ),
    (
        "Rust.Cow",
        "std::borrow::Cow<'static, str>",
        "std::borrow::Cow::Borrowed(\"\")",
    ),
    // ── Time ──────────────────────────────────────────────────────────────
    (
        "Rust.Duration",
        "std::time::Duration",
        "std::time::Duration::ZERO",
    ),
    (
        "Rust.Instant",
        "std::time::Instant",
        "std::time::Instant::now()",
    ),
    (
        "Rust.SystemTime",
        "std::time::SystemTime",
        "std::time::SystemTime::UNIX_EPOCH",
    ),
    // ── Ranges ────────────────────────────────────────────────────────────
    ("Rust.Range", "std::ops::Range<i64>", "0..0"),
];

/// `(Rust type, initial value)` a compiled block binds `path` as — see
/// [`BLOCK_BINDINGS`].
///
/// `None` for a developer-defined class: codegen binds that as the bare type
/// name, started from `Default::default()`.
pub fn block_binding(path: &str) -> Option<(&'static str, &'static str)> {
    BLOCK_BINDINGS
        .iter()
        .find(|(p, _, _)| *p == path)
        .map(|(_, ty, init)| (*ty, *init))
}

/// Whether `path` (e.g. `"Rust.String"`) is one of the shipped types.
///
/// Answers for [`SHIPPED_RUST_TYPES`] only — a developer-defined type declared
/// in an item-level block is *not* here and is still legitimate (spec 041 R22).
pub fn is_shipped_rust_type(path: &str) -> bool {
    SHIPPED_RUST_TYPES.iter().any(|(_, p)| *p == path)
}

/// The bare type name of a `Rust.*` path — `"Rust.HashMap"` → `"HashMap"`.
///
/// `None` when `path` is not `Rust.`-prefixed, which is how a `REPOSITORY`
/// entry for some other external hierarchy is left alone.
pub fn rust_type_name(path: &str) -> Option<&str> {
    path.strip_prefix("Rust.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_set_is_complete_and_unique() {
        assert_eq!(
            SHIPPED_RUST_TYPES.len(),
            48,
            "the shipped Rust-type count changed — update the guide and the KB"
        );
        let mut paths: Vec<_> = SHIPPED_RUST_TYPES.iter().map(|(_, p)| *p).collect();
        paths.sort_unstable();
        let before = paths.len();
        paths.dedup();
        assert_eq!(before, paths.len(), "a Rust type path is listed twice");

        let mut names: Vec<_> = SHIPPED_RUST_TYPES.iter().map(|(n, _)| *n).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "a CLASS name is listed twice");
    }

    #[test]
    fn every_entry_is_rust_prefixed_and_looked_up_by_path() {
        for (name, path) in SHIPPED_RUST_TYPES {
            assert!(name.starts_with("RUST-"), "{name} is not RUST- prefixed");
            assert!(rust_type_name(path).is_some(), "{path} is not Rust.-prefixed");
            assert!(is_shipped_rust_type(path), "{path} does not look itself up");
        }
        assert!(!is_shipped_rust_type("Rust.Nope"));
        assert_eq!(rust_type_name("System.String"), None);
    }

    /// Spec 041 T8 — the two lists must describe the same set of classes. A
    /// shipped class with no binding could not be used from a compiled block;
    /// a binding for a class that no longer ships is dead text nobody will
    /// notice going stale.
    #[test]
    fn every_shipped_type_has_a_block_binding() {
        for (name, path) in SHIPPED_RUST_TYPES {
            let (ty, init) = block_binding(path)
                .unwrap_or_else(|| panic!("{name} ({path}) has no EXEC RUST binding"));
            assert!(!ty.is_empty() && !init.is_empty(), "{path} binds to nothing");
        }
        assert_eq!(
            BLOCK_BINDINGS.len(),
            SHIPPED_RUST_TYPES.len(),
            "a block binding exists for a class that is not shipped"
        );
        assert_eq!(block_binding("Rust.Point"), None, "developer types are not here");
    }
}
