// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Generate COBOL `SELECT` / `FD` fragments from `.cidx` definitions.

use cobolt_indexed::{
    AccessMode, FieldUsage, IndexedDefinition, IndexedField, RecordFormatDef, StorageMode,
};

/// Generate a COBOL source fragment for one indexed-file definition.
pub fn generate_indexed(def: &IndexedDefinition) -> String {
    let mut out = String::with_capacity(2048);
    write_indexed_header(&mut out);
    write_environment(&mut out, def);
    write_file_section(&mut out, def);
    out
}

fn write_indexed_header(out: &mut String) {
    out.push_str("      *> ───────────────────────────────────────────────────────────\n");
    out.push_str("      *>  This code was generated automatically by PowerRustCOBOL RAD.\n");
    out.push_str("      *>\n");
    out.push_str("      *>  DO NOT MODIFY IT DIRECTLY: it is regenerated the next time\n");
    out.push_str("      *>  you interact with the Indexed File Editor, so manual edits\n");
    out.push_str("      *>  are lost. Edit the `.cidx` definition instead.\n");
    out.push_str("      *> ───────────────────────────────────────────────────────────\n\n");
}

fn write_environment(out: &mut String, def: &IndexedDefinition) {
    out.push_str("       ENVIRONMENT DIVISION.\n");
    out.push_str("       INPUT-OUTPUT SECTION.\n");
    out.push_str("       FILE-CONTROL.\n");
    out.push_str(&format!(
        "           SELECT {} ASSIGN TO \"{}\"\n",
        def.name, def.assign_path
    ));
    out.push_str("               ORGANIZATION IS INDEXED\n");
    out.push_str(&format!(
        "               ACCESS MODE IS {}\n",
        access_mode_cobol(def.access_mode)
    ));
    if let Some(key_field) = def.keys.primary.parts.first() {
        out.push_str(&format!(
            "               RECORD KEY IS {}\n",
            key_field.field_name
        ));
    }
    for alt in &def.keys.alternates {
        let name = alt
            .name
            .as_deref()
            .or_else(|| alt.parts.first().map(|p| p.field_name.as_str()))
            .unwrap_or("ALT-KEY");
        if alt.duplicates_allowed {
            out.push_str(&format!(
                "               ALTERNATE RECORD KEY IS {} WITH DUPLICATES\n",
                name
            ));
        } else {
            out.push_str(&format!(
                "               ALTERNATE RECORD KEY IS {}\n",
                name
            ));
        }
    }
    let mut clauses = String::new();
    match def.storage {
        StorageMode::Memory => clauses.push_str("STORAGE MODE IS MEMORY"),
        StorageMode::Disk => clauses.push_str("STORAGE MODE IS DISK"),
    }
    if def.compression {
        clauses.push_str(" WITH DATA COMPRESSION");
    }
    if def.persistence && def.storage == StorageMode::Memory {
        clauses.push_str(" WITH PERSISTENCE");
    }
    out.push_str(&format!("               {clauses}.\n\n"));
}

fn access_mode_cobol(mode: AccessMode) -> &'static str {
    match mode {
        AccessMode::Sequential => "SEQUENTIAL",
        AccessMode::Random => "RANDOM",
        AccessMode::Dynamic => "DYNAMIC",
    }
}

fn write_file_section(out: &mut String, def: &IndexedDefinition) {
    out.push_str("       DATA DIVISION.\n");
    out.push_str("       FILE SECTION.\n");
    out.push_str(&format!("       FD  {}.\n", def.name));
    match def.record_format {
        RecordFormatDef::Fixed { length } => {
            out.push_str(&format!(
                "           RECORD CONTAINS {length} CHARACTERS.\n"
            ));
        }
        RecordFormatDef::Variable {
            min_length,
            max_length,
        } => {
            out.push_str(&format!(
                "           RECORD CONTAINS {min_length} TO {max_length} CHARACTERS.\n"
            ));
        }
    }
    for field in &def.fields {
        write_field(out, field, 11);
    }
    out.push('\n');
}

fn write_field(out: &mut String, field: &IndexedField, indent: usize) {
    let pad = " ".repeat(indent);
    if field.children.is_empty() {
        if field.pic.is_empty() {
            return;
        }
        let usage = usage_suffix(field.usage);
        // Strip any custom PIC marker so the emitted COBOL source code is clean.
        let pic = field.pic.trim_start_matches('\u{200B}');
        out.push_str(&format!(
            "{pad}{:02} {} PIC {}",
            field.level, field.name, pic
        ));
        if !usage.is_empty() {
            out.push_str(&format!(" {usage}"));
        }
        out.push_str(".\n");
    } else {
        out.push_str(&format!("{pad}{:02} {}.\n", field.level, field.name));
        for child in &field.children {
            write_field(out, child, indent + 4);
        }
    }
}

fn usage_suffix(usage: FieldUsage) -> &'static str {
    match usage {
        FieldUsage::Display => "",
        FieldUsage::Comp => "USAGE IS COMP",
        FieldUsage::Comp3 => "USAGE IS COMP-3",
        FieldUsage::Comp4 => "USAGE IS COMP-4",
        FieldUsage::Binary => "USAGE IS BINARY",
        FieldUsage::PackedDecimal => "USAGE IS PACKED-DECIMAL",
        FieldUsage::Index => "USAGE IS INDEX",
        FieldUsage::Pointer => "USAGE IS POINTER",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_indexed::{
        IndexedField, KeyDef, KeyEncodingDef, KeyOrderingDef, KeyPartDef, KeySchema,
    };

    fn sample() -> IndexedDefinition {
        let mut def = IndexedDefinition::new("CUSTOMER-FILE", "data/customers.idx");
        def.keys = KeySchema {
            primary: KeyDef {
                name: Some("CUST-ID".into()),
                parts: vec![KeyPartDef {
                    field_name: "CUST-ID".into(),
                    offset: 0,
                    length: 8,
                    encoding: KeyEncodingDef::Bytes,
                }],
                duplicates_allowed: false,
                ordering: KeyOrderingDef::Ascending,
            },
            alternates: Vec::new(),
        };
        def.fields = vec![IndexedField {
            level: 1,
            name: "CUSTOMER-RECORD".into(),
            pic: String::new(),
            usage: FieldUsage::Display,
            offset: None,
            length: None,
            comment: String::new(),
            grid_control: None,
            occurs: None,
            redefines: None,
            synchronized: false,
            children: vec![IndexedField {
                level: 5,
                name: "CUST-ID".into(),
                pic: "9(8)".into(),
                usage: FieldUsage::Display,
                offset: Some(0),
                length: Some(8),
                comment: String::new(),
                grid_control: None,
                occurs: None,
                redefines: None,
                synchronized: false,
                children: Vec::new(),
            }],
        }];
        def
    }

    #[test]
    fn banner_and_select() {
        let out = generate_indexed(&sample());
        assert!(out.contains("Indexed File Editor"));
        assert!(out.contains("SELECT CUSTOMER-FILE"));
        assert!(out.contains("RECORD KEY IS CUST-ID"));
        assert!(out.contains("FD  CUSTOMER-FILE"));
        assert!(out.contains("PIC 9(8)"));
    }

    #[test]
    fn variable_and_compression() {
        let mut def = sample();
        def.record_format = RecordFormatDef::Variable {
            min_length: 10,
            max_length: 200,
        };
        def.storage = StorageMode::Memory;
        def.compression = true;
        def.persistence = true;
        let out = generate_indexed(&def);
        assert!(out.contains("10 TO 200"));
        assert!(out.contains("STORAGE MODE IS MEMORY"));
        assert!(out.contains("WITH DATA COMPRESSION"));
        assert!(out.contains("WITH PERSISTENCE"));
    }
}
