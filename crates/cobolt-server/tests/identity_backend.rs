// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-009 T16: the backend reads the identity programs the Build writes in `generated/identity/`
//! beside the developer's `exports.toml` and `src/`, keeps them out of the WebSocket's table and the
//! contract, and never lets them shadow — or be shadowed by — a program of the developer's. (The
//! WebSocket's own refusal is in `loopback.rs`, `the_identity_programs_are_not_on_the_websocket`.)

#![cfg(feature = "server")]

use std::path::{Path, PathBuf};

use cobolt_codegen::identity::{self as generated, PROGRAMS};
use cobolt_server::backend::{exported_names, Backend};
use cobolt_server::bundle::Bundle;
use cobolt_server::identity::ops;

const MY_EXPORTS: &str = "[contract]\nversion = \"1.0.0\"\n\n[[export]]\nprogram = \"HELLO\"\n";

const HELLO: &str = r#"       IDENTIFICATION DIVISION.
       PROGRAM-ID. HELLO.
       DATA DIVISION.
       LINKAGE SECTION.
       01 LK-A.
          05 MSG PIC X(10).
       PROCEDURE DIVISION USING LK-A.
       MAIN-PARA.
           MOVE "hello" TO MSG
           GOBACK.
"#;

fn backend(tag: &str, exports: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pr-identity-backend-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("exports.toml"), exports).unwrap();
    std::fs::write(dir.join("src").join("hello.cbl"), HELLO).unwrap();
    dir
}

/// Every file of the developer's (`exports.toml`, `src/`), by name, with its bytes.
fn developer_files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = vec![("exports.toml".to_string(), std::fs::read(dir.join("exports.toml")).unwrap())];
    let mut src: Vec<_> = std::fs::read_dir(dir.join("src")).unwrap().map(|e| e.unwrap().path()).collect();
    src.sort();
    out.extend(src.into_iter().map(|p| (p.display().to_string(), std::fs::read(&p).unwrap())));
    out
}

#[test]
fn the_identity_programs_join_the_backend_apart_from_its_exports() {
    let dir = backend("merge", MY_EXPORTS);
    let before = developer_files(&dir);
    generated::sync(&dir, true).expect("generates");
    let b = Backend::load(&dir).unwrap_or_else(|e| panic!("{e:#?}"));

    // The contract and the WebSocket's table are the developer's alone.
    let names: Vec<&str> = b.interface.programs.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["HELLO"]);
    assert_eq!(exported_names(&dir), vec!["HELLO".to_string()]);
    let hello = b.program("HELLO").expect("HELLO");
    assert!(
        hello.program.nested_programs.iter().all(|p| !p.identification.program_id.starts_with("AUTH-")),
        "a developer program does not carry the identity programs"
    );

    for (name, _) in PROGRAMS {
        assert!(b.program(name).is_none(), "{name} is not on the WebSocket's table");
        let p = b.identity_program(name).unwrap_or_else(|| panic!("{name} is an identity program"));
        assert!(p.sig.public, "{name}: a person signing in has no token yet");
        assert_eq!(p.params, vec!["AUTH-REQUEST".to_string()]);
        assert_eq!(p.returning.as_deref(), Some("AUTH-RESULT"));
        assert!(p.program.nested_programs.is_empty(), "{name} carries no other program");
    }

    // The packed backend (the built binary) reads the same.
    let packed = Bundle::from_bytes(&Bundle::from_dir(&dir).unwrap().to_bytes().unwrap()).unwrap().into_backend().unwrap();
    assert!(packed.program("AUTH-SIGN-IN").is_none());
    assert!(packed.identity_program("AUTH-SIGN-IN").is_some());
    assert_eq!(packed.interface.programs.len(), 1);

    // Removing `[identity]` removes the programs; the developer's files never changed.
    generated::sync(&dir, false).unwrap();
    let b = Backend::load(&dir).unwrap();
    assert!(b.identity_program("AUTH-SIGN-IN").is_none());
    assert_eq!(developer_files(&dir), before, "the developer's exports.toml and src/ are byte-identical");
    println!("merged: 13 identity programs, contract [HELLO]; the developer's {} files byte-identical", before.len());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_developer_export_of_an_identity_program_is_refused_naming_both_files() {
    let dir = backend("clash", &format!("{MY_EXPORTS}\n[[export]]\nprogram = \"auth-sign-in\"\npublic = true\n"));
    generated::sync(&dir, true).expect("generates");
    let errs = Backend::load(&dir).err().expect("refused");
    println!("{errs:#?}");
    assert!(errs.iter().any(|e| e.contains("AUTH-SIGN-IN") && e.contains("exports.toml") && e.contains("generated/identity/exports.toml")), "{errs:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_the_build_marks_an_export_identity() {
    let dir = backend("mark", "[contract]\nversion = \"1.0.0\"\n\n[[export]]\nprogram = \"HELLO\"\nidentity = true\n");
    let errs = Backend::load(&dir).err().expect("refused");
    assert!(errs.iter().any(|e| e.contains("HELLO sets identity")), "{errs:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_developer_program_with_an_identity_program_id_is_refused() {
    let dir = backend("shadow", MY_EXPORTS);
    std::fs::write(dir.join("src").join("mine.cbl"), HELLO.replace("PROGRAM-ID. HELLO.", "PROGRAM-ID. AUTH-MFA.")).unwrap();
    generated::sync(&dir, true).expect("generates");
    let errs = Backend::load(&dir).err().expect("refused");
    assert!(errs.iter().any(|e| e.contains("AUTH-MFA") && e.contains("generated/identity/")), "{errs:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `[identity]` with no generated programs (the project was not built since the section was added)
/// stops the start with what to do, rather than failing the first sign-in.
#[test]
fn an_identity_section_without_its_programs_stops_the_start() {
    std::env::set_var("PRC_IB_COOKIE", "AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA=");
    let dir = backend("unbuilt", MY_EXPORTS);
    let toml = "[server]\nlisten = \"127.0.0.1:0\"\nallowed_origins = [\"https://app.example.test\"]\n[auth]\nissuer = [\"https://cognito-idp.eu-west-1.amazonaws.com/eu-west-1_Pool\"]\njwks_uri = \"https://cognito-idp.eu-west-1.amazonaws.com/eu-west-1_Pool/.well-known/jwks.json\"\naudiences = [\"app\"]\n[identity]\nprovider = \"cognito\"\ncookie_key = \"env:PRC_IB_COOKIE\"\n[identity.settings]\nregion = \"eu-west-1\"\nuser_pool_id = \"eu-west-1_Pool\"\nclient_id = \"app\"\n";
    let resolved = cobolt_server::config::ServerConfig::from_toml(toml).unwrap().resolve(cobolt_server::config::RunMode::Release, &dir).unwrap_or_else(|e| panic!("{e:?}"));
    let backend = Backend::load(&dir).unwrap();
    let e = cobolt_server::start(resolved, backend, cobolt_server::Options::default()).err().expect("refused");
    println!("{e}");
    assert!(e.contains("[identity]") && e.contains("AUTH-SIGN-IN") && e.contains("build the project"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The generator and the identity service name the same operations and the same members: a member
/// one of them adds and the other lacks would be dropped on the way in or out without a word.
#[test]
fn the_generated_groups_match_the_service() {
    let ops_generated: Vec<&str> = PROGRAMS.iter().map(|(_, op)| *op).collect();
    assert_eq!(ops_generated, ops::OPERATIONS);
    for (name, op) in PROGRAMS {
        assert_eq!(name, format!("AUTH-{op}"));
    }
    let request: Vec<&str> = generated::REQUEST.iter().map(|(n, _)| *n).collect();
    assert_eq!(request, ops::REQUEST_MEMBERS);
    let result: Vec<&str> = generated::RESULT.iter().map(|(n, _)| *n).collect();
    assert_eq!(result, ops::RESULT_MEMBERS);
    assert!(generated::groups().contains("EXPIRES-IN       PIC 9(9)."), "the one numeric member");
}

/// The members as the backend PARSED them, not as the generator spelled them: a member named with a
/// reserved word (`RETURN`) parses clean and is silently not a member, so every request carrying it
/// was refused as an unknown key.
#[test]
fn every_member_survives_the_parse() {
    use cobolt_wire::layout::Kind;
    let dir = backend("layout", MY_EXPORTS);
    generated::sync(&dir, true).expect("generates");
    let b = Backend::load(&dir).unwrap_or_else(|e| panic!("{e:#?}"));
    let p = b.identity_program("AUTH-SIGN-IN").expect("AUTH-SIGN-IN");
    let names = |item: &cobolt_wire::layout::Item| -> Vec<String> {
        match &item.kind {
            Kind::Group { children } => children.iter().map(|c| c.name.to_ascii_uppercase()).collect(),
            other => panic!("{} is not a group: {other:?}", item.name),
        }
    };
    assert_eq!(names(&p.sig.params[0].layout), ops::REQUEST_MEMBERS);
    let mut result: Vec<String> = ops::RESULT_MEMBERS.iter().map(|s| s.to_string()).collect();
    let at = result.iter().position(|m| m == "REFRESH-TOKEN").unwrap() + 1;
    result.insert(at, "EXPIRES-IN".into());
    assert_eq!(names(&p.sig.returning.as_ref().expect("RETURNING").layout), result);
    println!("AUTH-REQUEST: {} members, AUTH-RESULT: {} members, as parsed", ops::REQUEST_MEMBERS.len(), result.len());
    let _ = std::fs::remove_dir_all(&dir);
}
