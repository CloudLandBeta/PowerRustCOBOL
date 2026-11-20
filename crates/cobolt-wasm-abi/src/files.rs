// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The files of a desktop module** (ABI A34, DESKTOP-WASM-001 T38): the custom section
//! `prfiles`, which tells the native runner what each file the program declares is.
//!
//! A module names a file by its number in [`FilesSection::files`] when it calls a file import of
//! `prn` (`f_open`, `f_read`, …). The runner builds the file's engine from this description, as
//! the interpreter builds it from the program's `SELECT` and `FD`: the organisation, the access
//! mode, the record's length, the keys and the clauses that choose the container. The section is
//! JSON, versioned by `format`; a field is never removed or given a new meaning, and a field a
//! later release adds has a default, so every runner reads every earlier module.
//!
//! Only a desktop module carries the section (the generator's `native` build), and only when the
//! program declares a file the runner opens.

use serde::{Deserialize, Serialize};

/// The custom section's name.
pub const FILES_SECTION: &str = "prfiles";

/// The section's format. A runner reads every format up to the one it knows.
pub const FILES_FORMAT: u32 = 1;

/// The `prfiles` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesSection {
    pub format: u32,
    /// The files, by number: the `id` a file import names is an index here.
    pub files: Vec<FileDecl>,
}

/// How a file's records are organised (`ORGANIZATION IS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Org {
    Sequential,
    LineSequential,
    Relative,
    Indexed,
}

/// How the program reaches the records (`ACCESS MODE IS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Access {
    Sequential,
    Random,
    Dynamic,
}

/// One file the program declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDecl {
    /// The file's name, upper case (for messages).
    pub name: String,
    pub org: Org,
    pub access: Access,
    /// `SELECT OPTIONAL`.
    #[serde(default)]
    pub optional: bool,
    /// The first record description's length: what a fixed-length sequential `READ` reads, and an
    /// INDEXED record's length.
    pub record_len: u32,
    /// A RELATIVE file's slot: the widest record description, raised to the `RECORD` clause's
    /// largest length.
    #[serde(default)]
    pub slot_len: u32,
    /// The records vary in length: the `RECORD` clause says so, or the record descriptions differ in
    /// length.
    #[serde(default)]
    pub varying: bool,
    /// The `RECORD` clause's smallest and largest lengths, when it states them. A record written
    /// outside them is status 44.
    #[serde(default)]
    pub min_len: Option<u32>,
    #[serde(default)]
    pub max_len: Option<u32>,
    /// INDEXED: the RECORD KEY, then each ALTERNATE RECORD KEY whose field is in the record. With no
    /// RECORD KEY in the record the first key is the whole record.
    #[serde(default)]
    pub keys: Vec<KeyDecl>,
    /// INDEXED: the name of each key, as the container records it (the RECORD KEY's when the
    /// `SELECT` declares one, whether or not it is in the record).
    #[serde(default)]
    pub key_names: Vec<Option<String>>,
    /// INDEXED: the RECORD KEY is declared and is a field of the record, so a write in the
    /// sequential access mode is judged by its order (status 21).
    #[serde(default)]
    pub ordered_writes: bool,
    /// `STORAGE IS MEMORY`, `WITH PERSISTENCE`, `DATA COMPRESSING`.
    #[serde(default)]
    pub memory: bool,
    #[serde(default)]
    pub persist: bool,
    #[serde(default)]
    pub compress: bool,
    /// The container the `SELECT` chooses: `prcidxd1` or `redb`; `None` leaves it to the run.
    #[serde(default)]
    pub engine: Option<String>,
    /// The FD as text, which an INDEXED file the program creates carries inside itself.
    #[serde(default)]
    pub definition: Option<String>,
    /// The integer digits of a RELATIVE file's RELATIVE KEY item: a sequential READ that
    /// delivers a record number with more digits is status 14.
    #[serde(default)]
    pub rel_digits: Option<u32>,
    /// INDEXED: the keys as the `SELECT` names them, which `COBOL::"PARTIQL"` (A56) finds in the
    /// first record description by name, as the interpreter does; `None` when the `SELECT` names no
    /// RECORD KEY.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub named_keys: Option<NamedKeys>,
}

/// The keys of an INDEXED file as its `SELECT` names them (A56).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedKeys {
    /// The RECORD KEY.
    pub record: NamedKey,
    /// Each ALTERNATE RECORD KEY, in the `SELECT`'s order, whether or not its field is in the record.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternates: Vec<NamedKey>,
}

/// One key as the `SELECT` names it, spelled as the interpreter holds it: the RECORD KEY's name and
/// qualifiers in upper case, an ALTERNATE RECORD KEY's and every split key's parts as written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedKey {
    /// The key's data name (a split key's own name).
    pub name: String,
    /// Its `OF`/`IN` chain, innermost first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quals: Vec<String>,
    /// A split key's parts, each a data name and its `OF`/`IN` chain; empty for a key of one field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<(String, Vec<String>)>,
    /// `WITH DUPLICATES`.
    #[serde(default)]
    pub dup: bool,
}

/// One key of an INDEXED file: the `(offset, length)` of each part in the record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyDecl {
    pub parts: Vec<(u32, u32)>,
    /// `WITH DUPLICATES`.
    #[serde(default)]
    pub dup: bool,
    /// A split key (`KEY IS a, b`), even of one part.
    #[serde(default)]
    pub split: bool,
}

impl FilesSection {
    /// The section's bytes.
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("the files section serialises")
    }

    /// Read the section's bytes. A format newer than this release knows is refused: its meaning
    /// cannot be guessed.
    pub fn decode(bytes: &[u8]) -> Result<FilesSection, String> {
        let s: FilesSection = serde_json::from_slice(bytes).map_err(|e| format!("the `{FILES_SECTION}` section cannot be read: {e}"))?;
        if s.format == 0 || s.format > FILES_FORMAT {
            return Err(format!("the `{FILES_SECTION}` section has format {}, and this release reads 1 to {FILES_FORMAT}", s.format));
        }
        Ok(s)
    }
}

/// The meaning of the file imports' arguments (A34).
pub mod verb {
    /// `f_open`'s `mode`.
    pub const OPEN_INPUT: i32 = 0;
    pub const OPEN_OUTPUT: i32 = 1;
    pub const OPEN_IO: i32 = 2;
    pub const OPEN_EXTEND: i32 = 3;
    /// `f_open`'s `flags`: `WITH LOCK` or `SHARING WITH NO OTHER`.
    pub const OPEN_EXCLUSIVE: i32 = 1;
    /// `f_close`'s `how`.
    pub const CLOSE_PLAIN: i32 = 0;
    pub const CLOSE_LOCK: i32 = 1;
    pub const CLOSE_REEL: i32 = 2;
    /// `f_read`'s `how`: bits.
    pub const READ_KEYED: i32 = 1;
    pub const READ_PREVIOUS: i32 = 2;
    pub const READ_NO_LOCK: i32 = 4;
    /// `f_start`'s `op`.
    pub const START_EQ: i32 = 0;
    pub const START_GT: i32 = 1;
    pub const START_GE: i32 = 2;
    pub const START_LT: i32 = 3;
    pub const START_LE: i32 = 4;
    /// `f_tx`'s `op` (A35): `UNLOCK` the named file's record locks; `COMMIT` or `ROLLBACK` every
    /// INDEXED file the program has open (the file arguments are then [`NONE`]).
    pub const TX_UNLOCK: i32 = 0;
    pub const TX_COMMIT: i32 = 1;
    pub const TX_ROLLBACK: i32 = 2;
    /// No key bytes (`key_len`), no record number (`rel`).
    pub const NONE: i32 = -1;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A27/A34: the section reads back as written, and a field a later release adds is optional.
    #[test]
    fn files_section_round_trips_and_reads_a_lean_entry() {
        let s = FilesSection {
            format: FILES_FORMAT,
            files: vec![FileDecl {
                name: "CUSTOMERS".into(),
                org: Org::Indexed,
                access: Access::Dynamic,
                optional: true,
                record_len: 80,
                slot_len: 0,
                varying: false,
                min_len: None,
                max_len: None,
                keys: vec![KeyDecl { parts: vec![(0, 8)], dup: false, split: false }],
                key_names: vec![Some("CUST-ID".into())],
                ordered_writes: true,
                memory: false,
                persist: false,
                compress: false,
                engine: Some("redb".into()),
                definition: None,
                rel_digits: None,
                named_keys: Some(NamedKeys {
                    record: NamedKey { name: "CUST-ID".into(), quals: vec![], parts: vec![], dup: false },
                    alternates: vec![NamedKey { name: "CUST-KEY".into(), quals: vec!["CUST-REC".into()], parts: vec![("CUST-REGION".into(), vec![]), ("CUST-NAME".into(), vec![])], dup: true }],
                }),
            }],
        };
        let bytes = s.encode();
        assert_eq!(FilesSection::decode(&bytes).unwrap(), s);
        let lean = br#"{"format":1,"files":[{"name":"LOG","org":"line-sequential","access":"sequential","record_len":132}]}"#;
        let d = FilesSection::decode(lean).unwrap();
        assert_eq!(d.files[0].org, Org::LineSequential);
        assert!(!d.files[0].optional && d.files[0].keys.is_empty());
        assert!(FilesSection::decode(br#"{"format":2,"files":[]}"#).is_err());
        println!("── files_section_round_trips_and_reads_a_lean_entry ──");
        println!("  round trip: {} bytes, equal", bytes.len());
        println!("  lean entry read, format 2 refused · 3 of 3 checks passed");
    }
}
