// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! AC4 (R5): `GO TO … DEPENDING ON`, `ALTER` and `PERFORM … THRU`, with
//! `EXIT PERFORM` and `EXIT PERFORM CYCLE`, run identically on the module (both
//! memory variants) and on the interpreter.

mod common;

const SRC: &str =
    "IDENTIFICATION DIVISION.\nPROGRAM-ID. FLOW.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
01 K PIC 9 VALUE 0.\n01 I PIC 99 VALUE 0.\n01 N PIC 99 VALUE 0.\n\
PROCEDURE DIVISION.\n\
MAIN-PARA.\n\
    PERFORM P1 THRU P3\n\
    PERFORM P2 THRU P3 2 TIMES\n\
    MOVE 2 TO K\n\
    GO TO D1 D2 D3 DEPENDING ON K\n\
    DISPLAY \"NOT REACHED\".\n\
D1.\n    DISPLAY \"D1\"\n    GO TO NEXT-PART.\n\
D2.\n    DISPLAY \"D2\"\n    GO TO NEXT-PART.\n\
D3.\n    DISPLAY \"D3\"\n    IF K = 3\n        STOP RUN\n    END-IF\n    GO TO NEXT-PART.\n\
NEXT-PART.\n\
    PERFORM SW-PARA THRU ALT-END\n\
    ALTER SW-PARA TO PROCEED TO ALT-B\n\
    PERFORM SW-PARA THRU ALT-END\n\
    ALTER SW-PARA TO ALT-A\n\
    PERFORM SW-PARA THRU ALT-END\n\
    PERFORM VARYING I FROM 1 BY 1 UNTIL I > 10\n\
        IF I = 4\n            EXIT PERFORM CYCLE\n        END-IF\n\
        IF I = 7\n            EXIT PERFORM\n        END-IF\n\
        ADD I TO N\n        DISPLAY \"I \" I \" N \" N\n\
    END-PERFORM\n\
    DISPLAY \"AFTER LOOP \" I\n\
    MOVE 5 TO K\n\
    GO TO D1 D2 D3 DEPENDING ON K\n\
    DISPLAY \"OUT OF RANGE FALLS THROUGH\"\n\
    MOVE 3 TO K\n\
    GO TO D1 D2 D3 DEPENDING ON K.\n\
P1.\n    DISPLAY \"P1\".\n\
P2.\n    DISPLAY \"P2\".\n\
P3.\n    DISPLAY \"P3\".\n\
SW-PARA.\n    GO TO ALT-A.\n\
ALT-A.\n    DISPLAY \"ALT-A\"\n    GO TO ALT-END.\n\
ALT-B.\n    DISPLAY \"ALT-B\"\n    GO TO ALT-END.\n\
ALT-END.\n    EXIT.\n";

#[test]
fn go_to_depending_alter_and_perform_thru_match_the_interpreter() {
    let src = SRC.to_string();
    let out = common::same_output(&src);
    for want in [
        "P1",
        "D2",
        "ALT-A",
        "ALT-B",
        "I 06 N 17",
        "AFTER LOOP 07",
        "OUT OF RANGE FALLS THROUGH",
        "D3",
    ] {
        assert!(out.iter().any(|l| l == want), "{want} missing from {out:?}");
    }
    assert!(
        !out.iter()
            .any(|l| l.contains("NOT REACHED") || l.starts_with("I 04")),
        "{out:?}"
    );
    eprintln!(
        "control flow: {} lines equal on both engines and both memory variants",
        out.len()
    );
}
