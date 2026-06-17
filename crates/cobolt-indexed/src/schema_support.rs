// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Warn-only checks for schema features not fully supported at runtime (R9).

use std::fmt::Write;

use crate::model::{IndexedDefinition, KeyEncodingDef};

/// Non-blocking warnings shown at finalize.
pub fn finalize_warnings(def: &IndexedDefinition) -> Vec<String> {
    let mut w = Vec::new();
    check_key(&def.keys.primary, "primary", &mut w);
    for (i, alt) in def.keys.alternates.iter().enumerate() {
        check_key(alt, &format!("alternate #{}", i + 1), &mut w);
    }
    if let RecordFormatDef::Variable { .. } = def.record_format {
        w.push(
            "Variable-length records are representable in the definition; verify runtime \
             support for your target engine before relying on them in production."
                .into(),
        );
    }
    w
}

use crate::model::RecordFormatDef;

fn check_key(key: &crate::model::KeyDef, label: &str, out: &mut Vec<String>) {
    if key.parts.len() > 1 {
        out.push(format!(
            "The {label} key has {} parts (composite). The runtime may not fully exercise \
             composite keys from COBOL yet — OPEN/READ behaviour should be verified.",
            key.parts.len()
        ));
    }
    for part in &key.parts {
        if part.encoding != KeyEncodingDef::Bytes {
            out.push(format!(
                "Key part '{}' uses encoding '{}'. Non-bytes key encodings are recorded but \
                 may not be fully supported by the current runtime.",
                part.field_name,
                part.encoding.as_str()
            ));
        }
    }
}

/// Structural fingerprint for schema drift detection (offsets + key layout).
pub fn structural_fingerprint(def: &IndexedDefinition) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = write!(s, "rf:{:?}", def.record_format);
    let _ = write!(s, ";st:{:?}{}{}", def.storage, def.compression, def.persistence);
    fingerprint_key(&def.keys.primary, &mut s);
    for alt in &def.keys.alternates {
        fingerprint_key(alt, &mut s);
    }
    for leaf in def.fields.first().map(|r| r.all_leaves()).unwrap_or_default() {
        let _ = write!(
            s,
            ";f:{}:{}:{}:{:?}",
            leaf.name, leaf.pic, leaf.offset.unwrap_or(0), leaf.usage
        );
    }
    s
}

fn fingerprint_key(key: &crate::model::KeyDef, s: &mut String) {
    for p in &key.parts {
        let _ = write!(s, ";k:{}:{}:{}:{:?}", p.field_name, p.offset, p.length, p.encoding);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    #[test]
    fn composite_primary_warns() {
        let mut def = IndexedDefinition::new("F", "f.idx");
        def.keys.primary.parts.push(KeyPartDef {
            field_name: "A".into(),
            offset: 0,
            length: 4,
            encoding: KeyEncodingDef::Bytes,
        });
        def.keys.primary.parts.push(KeyPartDef {
            field_name: "B".into(),
            offset: 4,
            length: 4,
            encoding: KeyEncodingDef::Bytes,
        });
        let w = finalize_warnings(&def);
        assert!(w.iter().any(|x| x.contains("composite")));
    }
}