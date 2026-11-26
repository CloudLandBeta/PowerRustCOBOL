// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 T19 (R3, AC9 groundwork): the Backend container and its payload kind.
//!
//! The container's format is permanent, so each version is held here as a literal byte string, written by hand from the
//! format's description in `backend.rs` and not made by the encoder: a later change that moved a byte of a version would
//! fail this test, which is its whole purpose. Version 2 appended the application's name; version 1 is still read.

use cobolt_program_package::{self as pp, BackendContainer, Kind, PackageError};
use std::time::Instant;

/// A version 1 container, byte for byte: the magic, the version, then six fields (state, interface, exports,
/// history, slots, module), each a little-endian length and its bytes.
fn v1_literal() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(b"PRBCNTR\0\x01\0\0\0");
    b.extend_from_slice(b"\x08\0\0\0per-call");
    b.extend_from_slice(b"\x07\0\0\0{\"a\":1}");
    b.extend_from_slice(b"\x02\0\0\0[]");
    b.extend_from_slice(b"\x02\0\0\0[]");
    b.extend_from_slice(b"\x02\0\0\0[]");
    b.extend_from_slice(b"\x08\0\0\0\0asm\x01\0\0\0");
    b
}

/// A version 2 container, byte for byte: version 1's fields, the version number 2, and the name after the module.
fn v2_literal() -> Vec<u8> {
    let mut b = v1_literal();
    b[8] = 2;
    b.extend_from_slice(b"\x11\0\0\0PowerAnalyticsWeb");
    b
}

fn v1_value() -> BackendContainer {
    BackendContainer {
        state: "per-call".into(),
        interface: br#"{"a":1}"#.to_vec(),
        exports: b"[]".to_vec(),
        history: b"[]".to_vec(),
        slots: b"[]".to_vec(),
        module: b"\0asm\x01\0\0\0".to_vec(),
        name: String::new(),
    }
}

fn v2_value() -> BackendContainer {
    BackendContainer { name: "PowerAnalyticsWeb".into(), ..v1_value() }
}

#[test]
fn each_version_of_the_container_reads_as_written_and_the_current_one_is_written_as_read() {
    assert_eq!(BackendContainer::decode(&v1_literal()).unwrap(), v1_value(), "version 1 still reads, with no name");
    let literal = v2_literal();
    assert_eq!(BackendContainer::decode(&literal).unwrap(), v2_value(), "the version 2 literal reads as its fields");
    assert_eq!(v2_value().encode(), literal, "the encoder writes version 2 as the literal");
    assert_eq!(BackendContainer::default().encode().len(), 12 + 7 * 4, "an empty container: the header and seven empty fields");
    assert_eq!(BackendContainer::decode(&BackendContainer::default().encode()).unwrap(), BackendContainer::default());
}

#[test]
fn a_backend_travels_in_a_payload_beside_its_machine_code_and_a_kind_nobody_knows() {
    let started = Instant::now();
    let c = v1_value();
    let container = c.encode();
    let code = vec![7u8; 5_000];
    let engine = [9u8; 32];
    let payload = pp::encode(&[(Kind::Backend, &container), (Kind::MachineCode, &code), (Kind::MachineCodeEngine, &engine), (Kind::Other(99), b"later")]);
    let read = pp::decode(&payload).unwrap();
    assert_eq!(read.backend().unwrap(), c);
    assert_eq!(read.part(Kind::MachineCode).unwrap().verified().unwrap(), &code[..]);
    assert_eq!(read.part(Kind::Other(99)).unwrap().bytes, b"later");
    assert!(read.module().is_err(), "a Backend payload is not a Desktop program: it has no MODULE part");
    // The code 4 is the Backend's, and a reader that predates it sees `Other(4)` and passes it by.
    assert_eq!((Kind::Backend.code(), Kind::from_code(4), Kind::from_code(5)), (4, Kind::Backend, Kind::Other(5)));
    assert_eq!(Kind::Backend.to_string(), "BACKEND");
    // A Desktop payload has no Backend.
    let desktop = pp::encode(&[(Kind::Module, b"\0asm\x01\0\0\0")]);
    assert!(matches!(pp::decode(&desktop).unwrap().backend(), Err(PackageError::Damaged(_))));
    // A damaged part is refused by its checksum before the container is read.
    let mut damaged = payload.clone();
    let at = damaged.windows(8).position(|w| w == b"PRBCNTR\0").unwrap() + 20;
    damaged[at] ^= 0xff;
    assert_eq!(pp::decode(&damaged).unwrap().backend(), Err(PackageError::PartDamaged(Kind::Backend)));
    println!("payload of {} bytes: Backend container {} bytes, machine code, engine hash and an unknown kind; Desktop payload has no Backend; a damaged part refused by its checksum ({} ms)", payload.len(), container.len(), started.elapsed().as_millis());
}

#[test]
fn a_container_that_does_not_hold_together_is_refused_in_words() {
    let started = Instant::now();
    let good = v2_literal();
    // Every cut of a version 1 container too.
    for n in 0..v1_literal().len() {
        assert!(BackendContainer::decode(&v1_literal()[..n]).is_err(), "a version 1 container cut at {n} is read");
    }
    // Every cut of a container is refused, never read as something else.
    let mut cuts = 0u32;
    for n in 0..good.len() {
        let e = BackendContainer::decode(&good[..n]).expect_err("a cut container is read");
        assert!(matches!(&e, PackageError::Damaged(w) if w.contains("Backend container")), "cut at {n}: {e:?}");
        cuts += 1;
    }
    // Bytes after the last field; a wrong magic; version 0; a newer version; a state that is not text.
    let mut more = good.clone();
    more.push(0);
    assert!(matches!(BackendContainer::decode(&more), Err(PackageError::Damaged(w)) if w.contains("after its last field")));
    let mut wrong = good.clone();
    wrong[0] = b'X';
    assert!(matches!(BackendContainer::decode(&wrong), Err(PackageError::Damaged(w)) if w.contains("no header")));
    let mut zero = good.clone();
    zero[8] = 0;
    assert!(matches!(BackendContainer::decode(&zero), Err(PackageError::Damaged(w)) if w.contains("version 0")));
    let mut newer = good.clone();
    newer[8] = 3;
    assert_eq!(BackendContainer::decode(&newer), Err(PackageError::NewerFormat(3)));
    // A name that is not text.
    let mut nameless = good.clone();
    let at = good.len() - 1;
    nameless[at] = 0xff;
    assert!(matches!(BackendContainer::decode(&nameless), Err(PackageError::Damaged(w)) if w.contains("name that is not text")));
    let mut binary = good.clone();
    binary[16] = 0xff;
    assert!(matches!(BackendContainer::decode(&binary), Err(PackageError::Damaged(w)) if w.contains("not text")));
    // A length that overflows a 32-bit offset is a damaged container, not a panic.
    let mut huge = good.clone();
    huge[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(BackendContainer::decode(&huge), Err(PackageError::Damaged(w)) if w.contains("inside its state")));
    println!("{cuts} cuts of a {}-byte container refused, plus trailing bytes, wrong magic, version 0, a newer version, a state that is not text and a length past the end ({} ms)", good.len(), started.elapsed().as_millis());
    println!("PASS {:03} FAIL 000", cuts + 7);
}
