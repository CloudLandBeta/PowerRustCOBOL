// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The phone shell's verbs (WASM-012 T15, ABI A26) as the host sees them: refused with `-6`
//! outside the shell, a command to the shell and a wait inside it, and the answer read back as
//! the application's `SecureStatus` and `SecureValue`.

use super::*;
use cobolt_wasm_abi::manifest::{Limits, VariantInfo};

fn host(shell: bool) -> Host {
    let manifest = Manifest {
        contract: None,
        remote: Vec::new(),
        abi: 1,
        generator: "test".into(),
        project: "demo".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: vec![],
        events: vec![],
        forms: vec![],
        builtins: vec!["SECURESTORE".into(), "SECURELOAD".into(), "SECUREDELETE".into()],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions::default());
    h.shell = shell;
    h
}

fn shell_calls(h: &mut Host) -> Vec<(u32, &'static str, Vec<(&'static str, String)>)> {
    h.effects.drain(..).filter_map(|e| if let Effect::Shell { id, command, args } = e { Some((id, command, args)) } else { None }).collect()
}

#[test]
fn outside_the_shell_each_secure_verb_answers_status_minus_6_and_says_why() {
    let mut h = host(false);
    h.opts.debug_level = 1;
    h.app.insert("securevalue".into(), "stale".into());
    for verb in ["SecureStore", "SecureLoad", "SecureDelete"] {
        assert!(matches!(h.verb(1, 2, verb, &["session", "token"]), Ok(None)), "{verb}: the handler goes on, as after a FILE STATUS");
        assert_eq!((h.app_get("SecureStatus"), h.app_get("SecureValue")), ("-6".to_owned(), String::new()), "{verb}");
        let said = h.effects.iter().any(|e| matches!(e, Effect::Log { text, .. } if text.contains(&format!("COBOL::{verb}")) && text.contains("mobile shell")));
        assert!(said, "{verb}: the console says why");
    }
    assert!(shell_calls(&mut h).is_empty(), "nothing is asked of a shell that is not there");
    assert!(h.inflight.is_none() || h.inflight.as_ref().is_some_and(|f| f.wait.is_none()), "no task waits");
    println!("outside the shell: 3 verbs answered SecureStatus -6 with SecureValue empty and a console line, no command sent, no wait");
}

#[test]
fn inside_the_shell_a_verb_becomes_a_command_and_its_answer_is_read_back() {
    let mut h = host(true);
    h.verb(1, 2, "SecureStore", &["  session  ", "token-123      "]).unwrap();
    let calls = shell_calls(&mut h);
    assert_eq!(calls.len(), 1);
    let (id, command, args) = calls[0].clone();
    assert_eq!((command, args), ("secure_store", vec![("key", "session".to_owned()), ("value", "token-123".to_owned())]), "the key trimmed, the value without its padding");
    h.shell_answer(id, 0, "");
    assert_eq!(h.app_get("SecureStatus"), "0");

    h.verb(1, 2, "SecureLoad", &["session"]).unwrap();
    let (id, command, args) = shell_calls(&mut h).remove(0);
    assert_eq!((command, args), ("secure_load", vec![("key", "session".to_owned())]));
    assert_eq!(h.app_get("SecureValue"), "", "cleared while the shell is asked");
    h.shell_answer(id, 0, "token-123");
    assert_eq!((h.app_get("SecureStatus"), h.app_get("SecureValue")), ("0".to_owned(), "token-123".to_owned()));

    h.verb(1, 2, "SecureDelete", &["absent"]).unwrap();
    let (id, command, _) = shell_calls(&mut h).remove(0);
    assert_eq!(command, "secure_delete");
    h.shell_answer(id, 1, "");
    assert_eq!(h.app_get("SecureStatus"), "1", "nothing to delete");

    h.verb(1, 2, "SecureLoad", &["k"]).unwrap();
    let (id, _, _) = shell_calls(&mut h).remove(0);
    h.shell_answer(id, -6, "");
    assert_eq!(h.app_get("SecureStatus"), "-6", "a refusal of the shell");
    println!("inside the shell: 4 commands (store, load, delete, load refused), statuses 0 0 1 -6, the loaded value read back");
}
