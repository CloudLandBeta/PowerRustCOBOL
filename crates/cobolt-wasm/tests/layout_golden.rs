// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! AC2 (R3): the memory image of a group after its `VALUE` clauses and after a
//! run of `MOVE`s, byte for byte, against golden images written by hand from
//! the encoding rules (`cobolt_wasm_rt::codec`: an embedded negative sign is
//! `}` / `J`–`R`, a positive one the plain digit; `COMP-3` ends in `C`
//! (signed positive), `D` (negative) or `F` (unsigned); `COMP` is big-endian;
//! a `SEPARATE` sign is `+`/`-`; the storage of an `OCCURS … DEPENDING ON`
//! table is its maximum; a `REDEFINES` shares its bytes).

mod common;

use cobolt_wasm::host::run_program_memory;
use cobolt_wasm::Options;

const DATA: &str =
    "IDENTIFICATION DIVISION.\nPROGRAM-ID. LAYOUT.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
01 MARK-A PIC X(16) VALUE \"<<<<GROUP-START>\".\n\
01 G.\n\
    05 G-NAME  PIC X(5) VALUE \"AB\".\n\
    05 G-NUM   PIC S9(3) VALUE -12.\n\
    05 G-LEAD  PIC S9(3) SIGN LEADING VALUE -12.\n\
    05 G-SEP   PIC S9(3) SIGN TRAILING SEPARATE VALUE -12.\n\
    05 G-SEPL  PIC S9(3) SIGN LEADING SEPARATE VALUE 7.\n\
    05 G-PACK  PIC S9(5) COMP-3 VALUE -123.\n\
    05 G-UPACK PIC 9(3) COMP-3 VALUE 45.\n\
    05 G-BIN   PIC S9(4) COMP VALUE -2.\n\
    05 G-COUNT PIC 9 VALUE 2.\n\
    05 G-TAB   PIC X(2) OCCURS 1 TO 3 DEPENDING ON G-COUNT.\n\
    05 G-RED   PIC X(4) VALUE \"WXYZ\".\n\
    05 G-RDN   REDEFINES G-RED PIC 9(4).\n\
01 MARK-B PIC X(16) VALUE \"<GROUP-END>>>>>>\".\n\
PROCEDURE DIVISION.\n";

/// The bytes of `G`: between the two markers of the live region (the last
/// copy in memory: the form's template, which also holds the markers, comes
/// before its instance regions).
fn group_bytes(memory: &[u8]) -> Vec<u8> {
    let a = b"<<<<GROUP-START>";
    let b = b"<GROUP-END>>>>>>";
    let start = memory
        .windows(a.len())
        .rposition(|w| w == a)
        .expect("MARK-A in memory")
        + a.len();
    let end = start
        + memory[start..]
            .windows(b.len())
            .position(|w| w == b)
            .expect("MARK-B in memory");
    memory[start..end].to_vec()
}

fn hex(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Golden images, field by field.
fn golden(fields: &[&[u8]]) -> Vec<u8> {
    fields.concat()
}

#[test]
fn group_images_after_value_and_after_moves() {
    let after_value = golden(&[
        b"AB   ",            // G-NAME: "AB" padded with spaces
        b"01K",              // G-NUM -12: the last digit carries the sign, 2 negative = K
        b"}12",              // G-LEAD -12: the first digit (0) carries it, 0 negative = }
        b"012-",             // G-SEP -12: a trailing separate sign
        b"+007",             // G-SEPL +7: a leading separate sign
        &[0x00, 0x12, 0x3D], // G-PACK -123: 00123 and D
        &[0x04, 0x5F],       // G-UPACK 45: 045 and F (unsigned)
        &[0xFF, 0xFE],       // G-BIN -2: two's complement, big-endian
        b"2",                // G-COUNT
        b"      ",           // G-TAB: three occurrences of storage, spaces
        b"WXYZ",             // G-RED (G-RDN shares it)
    ]);
    let after_moves = golden(&[
        b"HELLO",
        b"345",
        b"}07",
        b"009+",
        b"-045",
        &[0x99, 0x99, 0x9C],
        &[0x00, 0x7F],
        &[0x03, 0xE8],
        b"3",
        b"    AB",
        b"1234",
    ]);
    let moves = "    MOVE \"HELLO WORLD\" TO G-NAME\n    MOVE 345 TO G-NUM\n    MOVE -7 TO G-LEAD\n    MOVE 9 TO G-SEP\n    MOVE -45 TO G-SEPL\n\
                 \x20   MOVE 99999 TO G-PACK\n    MOVE 7 TO G-UPACK\n    MOVE 1000 TO G-BIN\n    MOVE 3 TO G-COUNT\n    MOVE \"AB\" TO G-TAB (3)\n    MOVE 1234 TO G-RDN\n";
    let mut compared = 0usize;
    for multi in [true, false] {
        for (what, procedure, want) in [
            ("after VALUE", "", &after_value),
            ("after MOVEs", moves, &after_moves),
        ] {
            let src = format!("{DATA}{procedure}    STOP RUN.\n");
            let c = common::compile(
                &src,
                &Options {
                    multi_memory: multi,
                    ..Options::default()
                },
            );
            let (o, memory) = run_program_memory(&c.wasm).unwrap();
            assert!(o.trapped.is_none() && o.errors.is_empty(), "{o:?}");
            let got = group_bytes(&memory);
            assert_eq!(hex(&got), hex(want), "{what}, multi={multi}");
            compared += got.len();
        }
    }
    eprintln!("layout golden: 2 images x 2 variants, {compared} bytes compared, all equal");
}
