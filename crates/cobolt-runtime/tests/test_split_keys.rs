// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Split keys — a RECORD KEY or ALTERNATE RECORD KEY made of several fields,
//! joined in order, in both spellings: Micro Focus's `KEY IS name = a b c`
//! and Fujitsu's `KEY IS a, b, c`. `tests/cobol/fileio/split-keys.cbl` runs
//! the same checks on every engine: the in-memory one (persisted, so the
//! reopen reads its container back), PRCIDXD1 on disk, and redb.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::indexed::IndexedEngine;
use cobolt_runtime::Interpreter;

const PROGRAM: &str = include_str!("../../../tests/cobol/fileio/split-keys.cbl");

fn run(tag: &str, storage: &str, engine: IndexedEngine) -> Vec<String> {
    let base = std::env::temp_dir().join(format!("prc-split-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let src = PROGRAM
        .replace("\"/tmp/", &format!("\"{}/", base.display()))
        .replace("*>STORAGE", storage);
    let parsed = parse(tokenize(&src, SourceFormat::Free));
    assert!(parsed.diagnostics.iter().all(|d| d.severity != Severity::Error), "{:?}", parsed.diagnostics);
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(parsed.program.unwrap(), event_rx, state_tx, display_tx);
    interp.set_indexed_engine(engine);
    interp.run().expect("runs");
    let _ = std::fs::remove_dir_all(&base);
    display_rx.try_iter().map(|l| l.trim_end().to_string()).collect()
}

#[test]
fn split_keys_work_on_every_engine() {
    let mut table = Vec::new();
    for (tag, storage, engine) in [
        ("memory", "STORAGE IS MEMORY WITH PERSISTENCE", IndexedEngine::Rust),
        ("disk", "", IndexedEngine::Rust),
        ("redb", "", IndexedEngine::Redb),
    ] {
        let out = run(tag, storage, engine);
        println!("\n  -- {tag} --\n    {}", out.join("\n    "));
        assert!(out.iter().all(|l| !l.starts_with("FAIL")), "{tag}: {out:#?}");
        assert!(out.iter().any(|l| l == "PASS 009 FAIL 000"), "{tag}: {out:#?}");
        table.push(format!("{tag}: PASS 009 FAIL 000"));
    }
    println!("\nsplit keys — {}", table.join("; "));
}

/// Both spellings parse to the same shape: the name a START names the key
/// by, and its fields in order.
#[test]
fn both_spellings_parse_to_the_fields_in_order() {
    let select = |keys: &str| {
        let src = format!(
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. P.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT F ASSIGN TO \"f.dat\" ORGANIZATION IS INDEXED {keys} FILE STATUS IS FS.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 FS PIC XX.\nPROCEDURE DIVISION.\n    STOP RUN.\n"
        );
        let p = parse(tokenize(&src, SourceFormat::Free)).program.unwrap();
        p.environment.unwrap().input_output.unwrap().file_controls.remove(0)
    };
    let names = |parts: &[cobolt_ast::program::KeyField]| parts.iter().map(|k| k.name.clone()).collect::<Vec<_>>();

    let f = select("RECORD KEY IS PK = A B ALTERNATE RECORD KEY IS C, D, E WITH DUPLICATES ALTERNATE RECORD KEY IS AK = D A");
    assert_eq!(f.record_key.as_deref(), Some("PK"));
    assert_eq!(names(&f.record_key_parts), ["A", "B"]);
    assert_eq!((f.alternate_keys[0].field.as_str(), names(&f.alternate_keys[0].parts)), ("C", vec!["C".to_string(), "D".into(), "E".into()]));
    assert!(f.alternate_keys[0].with_duplicates, "WITH DUPLICATES after a list is the list's");
    assert_eq!((f.alternate_keys[1].field.as_str(), names(&f.alternate_keys[1].parts)), ("AK", vec!["D".to_string(), "A".into()]));
    assert!(!f.alternate_keys[1].with_duplicates);

    // An ordinary key stays one field, with no parts.
    let f = select("RECORD KEY IS K1 ALTERNATE RECORD KEY IS K2 WITH DUPLICATES");
    assert_eq!((f.record_key.as_deref(), f.record_key_parts.len()), (Some("K1"), 0));
    assert_eq!((f.alternate_keys[0].field.as_str(), f.alternate_keys[0].parts.len(), f.alternate_keys[0].with_duplicates), ("K2", 0, true));
    println!("split keys: Micro Focus `name = a b` and Fujitsu `a, b, c` both parse to their fields in order; an ordinary key is unchanged");
}
