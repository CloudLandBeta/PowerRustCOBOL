// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 092 R15–R18 / AC6 — the sound built-ins, through the interpreter, on
//! the null output (`COBOLT_AUDIO=null`), so no sound card is needed here or
//! in CI.
//!
//! `tests/cobol/audio/test-audio.cbl` is the quantified program; this runs it
//! and checks its result block. The tests share the application's one player,
//! so they take turns.

#![cfg(feature = "audio")]

use std::path::PathBuf;
use std::sync::{mpsc, Mutex, MutexGuard};

use cobolt_lexer::{expand_copybooks, tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// One player per process: one test at a time drives it.
static PLAYER: Mutex<()> = Mutex::new(());

fn take_the_player() -> MutexGuard<'static, ()> {
    let guard = PLAYER.lock().unwrap_or_else(|p| p.into_inner());
    // Before the player's first use: it reads the variable when it starts.
    std::env::set_var("COBOLT_AUDIO", "null");
    guard
}

fn audio_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/audio")
}

/// The fixture's bytes: two seconds of silence, 8 kHz 8-bit mono PCM, with
/// the RIFF header written by hand.
fn silent_wav(ms: u32) -> Vec<u8> {
    let rate: u32 = 8000;
    let data_len = rate * ms / 1000;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes()); // bytes per second
    b.extend_from_slice(&1u16.to_le_bytes()); // block align
    b.extend_from_slice(&8u16.to_le_bytes()); // bits per sample
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    b.resize(44 + data_len as usize, 0x80);
    b
}

fn detect_format(src: &str) -> SourceFormat {
    let looks_fixed = src.lines().any(|line| {
        let b = line.as_bytes();
        b.len() > 6 && b[6] != b' ' && b[..6].iter().all(|&c| c == b' ' || c.is_ascii_digit())
    });
    if looks_fixed {
        SourceFormat::Fixed
    } else {
        SourceFormat::Free
    }
}

/// Run a program to its end; its DISPLAY lines. The interpreter is dropped
/// before this returns, as when a program ends.
fn run(src: &str) -> String {
    let dir = audio_dir();
    let expanded = expand_copybooks(src, &dir, detect_format(src));
    assert!(expanded.errors.is_empty(), "copybook errors: {:?}", expanded.errors);
    let result = parse(tokenize(&expanded.text, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(result.program.expect("no program"), event_rx, state_tx, display_tx);
    interp.run().expect("the program runs");
    drop(interp);
    display_rx.try_iter().collect::<Vec<String>>().join("\n")
}

/// The committed fixture is exactly what the generator writes, so where it
/// came from is never a mystery. `COBOLT_WRITE_GOLDEN=1` rewrites it.
#[test]
fn the_fixture_is_two_seconds_of_generated_silence() {
    let path = audio_dir().join("tone-2s.wav");
    let wanted = silent_wav(2000);
    if std::env::var("COBOLT_WRITE_GOLDEN").is_ok() {
        std::fs::write(&path, &wanted).expect("write the fixture");
    }
    let found = std::fs::read(&path).unwrap_or_else(|_| {
        panic!("{} missing — write it with COBOLT_WRITE_GOLDEN=1", path.display())
    });
    assert_eq!(found.len(), 16_044, "44-byte header + 2 s × 8000 bytes");
    assert!(found == wanted, "the fixture differs from the generator's output");
}

#[test]
fn the_audio_test_program_passes_on_the_null_output() {
    let _player = take_the_player();
    std::env::set_var("AUDIO_TEST_DIR", audio_dir());
    let src = std::fs::read_to_string(audio_dir().join("test-audio.cbl")).expect("read .cbl");
    let out = run(&src);
    println!("{out}");
    assert!(out.contains("CASES RUN           : 16"), "{out}");
    assert!(out.contains("CASES FAILED        : 00"), "{out}");
    assert!(out.contains("OVERALL RESULT      : PASS"), "{out}");
}

/// The trailing arguments are optional, the `CALL` spelling is the same call,
/// and a sound stops when the program that started it ends.
#[test]
fn optional_arguments_the_call_form_and_the_end_of_the_program() {
    let _player = take_the_player();
    let tone = audio_dir().join("tone-2s.wav");
    let out = run(&format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. AUDIO-FORMS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-STATE   PIC X(10).
       01 WS-STATUS  PIC X(10).
       01 WS-POS     PIC 9(7).
       01 WS-LEN     PIC 9(7).
       PROCEDURE DIVISION.
       MAIN.
           COBOL::"AUDIO-PLAY" ( "{tone}" ).
           COBOL::"AUDIO-STATE" ( WS-STATE ).
           DISPLAY "STATE1 " WS-STATE.
           CALL "COBOL-AUDIO-PAUSE".
           CALL "COBOL-AUDIO-STATE" USING WS-STATE WS-POS.
           DISPLAY "STATE2 " WS-STATE.
           CALL "COBOL-AUDIO-RESUME" USING WS-STATUS.
           COBOL::"AUDIO-STATE" ( WS-STATE WS-POS WS-LEN ).
           DISPLAY "STATE3 " WS-STATUS " " WS-STATE " " WS-LEN.
           STOP RUN.
"#,
        tone = tone.display()
    ));
    println!("{out}");
    assert!(out.contains("STATE1 PLAYING"), "{out}");
    assert!(out.contains("STATE2 PAUSED"), "{out}");
    assert!(out.contains("STATE3 OK         PLAYING    0002000"), "{out}");

    // That program has ended while its sound was playing: the sound went
    // with it. A later program reads the player as idle.
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. AUDIO-AFTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-STATE   PIC X(10).
       01 WS-POS     PIC 9(7).
       01 WS-LEN     PIC 9(7).
       01 WS-TRIES   PIC 9(7) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           COBOL::"AUDIO-STATE" ( WS-STATE WS-POS WS-LEN ).
           PERFORM UNTIL WS-STATE NOT = "PLAYING" OR WS-TRIES > 2000000
               ADD 1 TO WS-TRIES
               COBOL::"AUDIO-STATE" ( WS-STATE WS-POS WS-LEN )
           END-PERFORM.
           DISPLAY "AFTER " WS-STATE " " WS-POS " " WS-LEN.
           STOP RUN.
"#,
    );
    println!("{out}");
    assert!(out.contains("AFTER IDLE       0000000 0000000"), "{out}");
}
