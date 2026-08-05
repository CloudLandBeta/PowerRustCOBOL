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
}
