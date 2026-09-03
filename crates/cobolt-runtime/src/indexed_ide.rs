// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! IDE helpers: grid browser sessions and empty-file creation from `.cidx` defs.

use std::path::Path;

use cobolt_indexed::{IndexedDefinition, RecordFormatDef, StorageMode};

use crate::indexed::{
    status, IndexedFile, IndexedFileInfo, IndexedStore, KeySpec, OpenMode, ReadDir, StartOp,
};

/// Schema comparison result for drift detection (R26).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaDrift {
    Ok,
    Mismatch { detail: String },
    NoSchemaOnDisk,
}

/// Compare a `.cidx` definition against on-disk `IndexedFileInfo`.
pub fn compare_schema(def: &IndexedDefinition, info: &IndexedFileInfo) -> SchemaDrift {
    let fp = cobolt_indexed::structural_fingerprint(def);
    let mut disk = String::new();
    use std::fmt::Write;
    let _ = write!(disk, "rf:{:?}", info.record_format);
    for p in &info.primary.parts {
        let _ = write!(disk, ";k:{}:{}:{}", p.offset, p.length, p.encoding as u8);
    }
    for alt in &info.alternates {
        for p in &alt.parts {
            let _ = write!(disk, ";k:{}:{}:{}", p.offset, p.length, p.encoding as u8);
        }
    }
    let def_len = def.record_length();
    let disk_len = info.record_format.max_len();
    if def_len != disk_len {
        return SchemaDrift::Mismatch {
            detail: format!("record length {def_len} vs on-disk {disk_len}"),
        };
    }
    if !fp.contains(&format!(";k:")) && disk.contains(";k:") {
        // loose check — key offsets must match primary part
        if let (Some(dp), Some(kp)) = (info.primary.parts.first(), def.keys.primary.parts.first()) {
            if dp.offset != kp.offset || dp.length != kp.length {
                return SchemaDrift::Mismatch {
                    detail: format!(
                        "primary key offset/length mismatch ({}/{} vs {}/{})",
                        kp.offset, kp.length, dp.offset, dp.length
                    ),
                };
            }
        }
    }
    SchemaDrift::Ok
}

/// Which physical container a `STORAGE IS DISK` file actually is, read from its
/// own leading bytes rather than assumed from the `.cidx`.
///
/// `StorageMode::Disk` names a COBOL-level storage class, not a physical
/// format: which of the Rust (`PRCIDXD1`), RmCOBOL, Fujitsu or **redb** engines
/// actually wrote the bytes is chosen at OPEN time, by `rcrun --indexed-engine`
/// or `COBOL_INDEXED_ENGINE` — a run-time choice the `.cidx` definition has no
/// field to record. A file created once with `--indexed-engine redb` therefore
/// looks identical to any other on the definition alone, and opening it as the
/// default `DiskIndexedFile` fails outright: the bytes do not even parse as
/// `PRCIDXD1`, so the engine correctly (if confusingly) reports FILE STATUS 39
/// — "conflicting file attributes" is exactly what a wrong container is.
enum DiskFormat {
    /// The default Rust engine's own container, or no file yet (nothing to
    /// sniff — the definition's own choice governs what gets CREATED).
    Prcidxd1,
    /// A `redb::Database`.
    Redb,
}

fn sniff_disk_format(path: &Path) -> DiskFormat {
    let Ok(mut f) = std::fs::File::open(path) else {
        return DiskFormat::Prcidxd1;
    };
    use std::io::Read;
    let mut head = [0u8; 4];
    if f.read_exact(&mut head).is_ok() && &head == b"redb" {
        return DiskFormat::Redb;
    }
    DiskFormat::Prcidxd1
}

/// Build runtime key specs from a definition.
pub fn key_specs_from_def(def: &IndexedDefinition) -> (KeySpec, Vec<KeySpec>) {
    let primary = def
        .keys
        .primary
        .parts
        .first()
        .map(|p| KeySpec {
            offset: p.offset as usize,
            len: p.length as usize,
            duplicates: false,
        })
        .unwrap_or(KeySpec {
            offset: 0,
            len: 1,
            duplicates: false,
        });
    let alternates: Vec<KeySpec> = def
        .keys
        .alternates
        .iter()
        .map(|alt| {
            let p = alt.parts.first().unwrap_or(&def.keys.primary.parts[0]);
            KeySpec {
                offset: p.offset as usize,
                len: p.length as usize,
                duplicates: alt.duplicates_allowed,
            }
        })
        .collect();
    (primary, alternates)
}

/// Create/truncate an empty indexed data file matching `def` (finalize, R9).
pub fn create_empty_from_definition(def: &IndexedDefinition, path: &Path) -> std::io::Result<()> {
    let record_len = match def.record_format {
        RecordFormatDef::Fixed { length } => length as usize,
        RecordFormatDef::Variable { max_length, .. } => max_length as usize,
    };
    let (primary, alternates) = key_specs_from_def(def);

    let mut file: Box<dyn IndexedStore> = match def.storage {
        StorageMode::Disk => {
            let f =
                crate::indexed_disk::DiskIndexedFile::new(path, record_len, primary, alternates);
            Box::new(f)
        }
        StorageMode::Memory => {
            let mut f = IndexedFile::new(path, record_len, primary, alternates);
            f.set_compressing(def.compression);
            f.set_persist(def.persistence);
            let names: Vec<Option<String>> = std::iter::once(def.keys.primary.name.clone())
                .chain(def.keys.alternates.iter().map(|k| k.name.clone()))
                .collect();
            f.set_key_names(names);
            Box::new(f)
        }
    };

    let st = file.open(OpenMode::Output);
    if st != status::OK {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("OPEN OUTPUT failed: FILE STATUS {st}"),
        ));
    }
    let st = file.close();
    if st != status::OK {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("CLOSE failed: FILE STATUS {st}"),
        ));
    }
    Ok(())
}

/// In-memory grid session over an indexed file (IDE grid browser).
pub struct GridSession {
    file: Box<dyn IndexedStore>,
    rows: Vec<Vec<u8>>,
    primary_offset: usize,
    primary_len: usize,
}

impl GridSession {
    pub fn open(def: &IndexedDefinition, path: &Path) -> Result<Self, String> {
        let record_len = match def.record_format {
            RecordFormatDef::Fixed { length } => length as usize,
            RecordFormatDef::Variable { max_length, .. } => max_length as usize,
        };
        let (primary, alternates) = key_specs_from_def(def);
        let primary_offset = primary.offset;
        let primary_len = primary.len;

        let mut file: Box<dyn IndexedStore> = match def.storage {
            // The definition says DISK, but that names a COBOL storage class,
            // not a physical format — sniff the file's own bytes for which
            // engine actually wrote it (see `sniff_disk_format`). A file that
            // does not exist yet sniffs as `Prcidxd1`, matching what
            // `create_empty_from_definition` would create for this same
            // definition.
            StorageMode::Disk => match sniff_disk_format(path) {
                DiskFormat::Redb => {
                    let mut f = crate::indexed_redb::RedbIndexedFile::new(
                        path, record_len, primary, alternates,
                    );
                    // Lenient, like the in-memory arm below: the browser is
                    // for looking at rows, not for enforcing the schema a
                    // COBOL program's own OPEN would.
                    f.set_strict_metadata(false);
                    Box::new(f)
                }
                DiskFormat::Prcidxd1 => {
                    let f = crate::indexed_disk::DiskIndexedFile::new(
                        path, record_len, primary, alternates,
                    );
                    Box::new(f)
                }
            },
            StorageMode::Memory => {
                let mut f = IndexedFile::new(path, record_len, primary, alternates);
                f.set_strict_metadata(false);
                Box::new(f)
            }
        };

        let st = file.open(OpenMode::Io);
        if st != status::OK {
            return Err(format!("OPEN I-O failed: FILE STATUS {st}"));
        }
        let mut session = Self {
            file,
            rows: Vec::new(),
            primary_offset,
            primary_len,
        };
        session.reload_rows();
        Ok(session)
    }

    pub fn rows(&self) -> &[Vec<u8>] {
        &self.rows
    }

    /// Position the file for REWRITE/DELETE on the row at `index`.
    pub fn select_row(&mut self, index: usize) -> Result<(), String> {
        let row = self
            .rows
            .get(index)
            .ok_or_else(|| format!("row {index} out of range"))?;
        let end = (self.primary_offset + self.primary_len).min(row.len());
        let key = &row[self.primary_offset..end];
        let (_, st) = self.file.read_key(key);
        if st != status::OK {
            return Err(format!(
                "Could not position on row {index}: FILE STATUS {st}"
            ));
        }
        Ok(())
    }

    pub fn write_row(&mut self, record: &[u8]) -> Result<(), String> {
        let st = self.file.write(record);
        if st != status::OK {
            return Err(format!("WRITE failed: FILE STATUS {st}"));
        }
        self.reload_rows();
        Ok(())
    }

    pub fn rewrite_row(&mut self, record: &[u8]) -> Result<(), String> {
        let st = self.file.rewrite(record, None);
        if st != status::OK {
            return Err(format!("REWRITE failed: FILE STATUS {st}"));
        }
        self.reload_rows();
        Ok(())
    }

    pub fn delete_current(&mut self) -> Result<(), String> {
        let st = self.file.delete(None);
        if st != status::OK {
            return Err(format!("DELETE failed: FILE STATUS {st}"));
        }
        self.reload_rows();
        Ok(())
    }

    pub fn commit(&mut self) {
        self.file.commit();
    }

    pub fn rollback(&mut self) {
        self.file.rollback();
        self.reload_rows();
    }

    fn reload_rows(&mut self) {
        self.rows.clear();
        self.file.set_key_of_reference(0);
        let _ = self.file.start(StartOp::Ge, &[]);
        loop {
            let (rec, st) = self.file.read_seq(ReadDir::Next);
            if st == status::EOF {
                break;
            }
            if st == status::OK {
                if let Some(r) = rec {
                    self.rows.push(r.to_vec());
                }
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_indexed::{
        IndexedField, KeyDef, KeyEncodingDef, KeyOrderingDef, KeyPartDef, KeySchema,
    };
    use tempfile::tempdir;

    fn test_def() -> IndexedDefinition {
        let mut def = IndexedDefinition::new("T-FILE", "t.idx");
        def.record_format = RecordFormatDef::Fixed { length: 10 };
        def.keys.primary = KeyDef {
            name: Some("K".into()),
            parts: vec![KeyPartDef {
                field_name: "K".into(),
                offset: 0,
                length: 3,
                encoding: KeyEncodingDef::Bytes,
            }],
            duplicates_allowed: false,
            ordering: KeyOrderingDef::Ascending,
        };
        def.fields = vec![IndexedField {
            level: 1,
            name: "REC".into(),
            pic: String::new(),
            usage: cobolt_indexed::FieldUsage::Display,
            offset: None,
            length: None,
            comment: String::new(),
            grid_control: None,
            occurs: None,
            redefines: None,
            synchronized: false,
            children: vec![IndexedField {
                level: 5,
                name: "K".into(),
                pic: "X(3)".into(),
                usage: cobolt_indexed::FieldUsage::Display,
                offset: Some(0),
                length: Some(3),
                comment: String::new(),
                grid_control: None,
                occurs: None,
                redefines: None,
                synchronized: false,
                children: Vec::new(),
            }],
        }];
        def.finalized = true;
        def
    }

    #[test]
    fn create_empty_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("t.idx");
        let def = test_def();
        create_empty_from_definition(&def, &path).unwrap();
        let info = crate::indexed_import::inspect_any_path(&path).unwrap();
        assert!(info.is_some());
    }

    #[test]
    fn grid_write_rewrite_delete() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("t.idx");
        let def = test_def();
        create_empty_from_definition(&def, &path).unwrap();
        let mut grid = GridSession::open(&def, &path).unwrap();
        assert!(grid.rows().is_empty());
        let mut rec = vec![b' '; 10];
        rec[0..3].copy_from_slice(b"001");
        grid.write_row(&rec).unwrap();
        assert_eq!(grid.rows().len(), 1);
        rec[3..6].copy_from_slice(b"ABC");
        grid.select_row(0).unwrap();
        grid.rewrite_row(&rec).unwrap();
        grid.select_row(0).unwrap();
        grid.delete_current().unwrap();
        assert!(grid.rows().is_empty());
    }
}
