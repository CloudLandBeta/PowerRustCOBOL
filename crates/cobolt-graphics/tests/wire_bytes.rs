// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-018 T11: the bytes every command of display-list version 1 encodes to
//! are fixed in `tests/golden/display-list-v1-commands.txt`, recorded BEFORE the
//! command table moved to `cobolt-graphics-wire`. A command that encoded to
//! other bytes after the move would fail here. `UPDATE_GOLDEN=1` rewrites the file.

use cobolt_graphics::display_list::{Command, Reader, COMMANDS};

/// Deterministic operand bytes for one command, from the kinds in the table.
fn sample(opcode: u8, operands: &[(&str, &str)], seed: u32) -> Vec<u8> {
    let mut b = vec![opcode];
    for (i, (_, kind)) in operands.iter().enumerate() {
        let n = seed + i as u32;
        match *kind {
            "u8" => b.push((n % 3) as u8),
            "u32" => b.extend_from_slice(&(n * 3 + 1).to_le_bytes()),
            "f32" => b.extend_from_slice(&(1.5f32 * (n + 1) as f32).to_bits().to_le_bytes()),
            "rgba" => b.extend_from_slice(&(0x1122_3344u32 + n).to_le_bytes()),
            "str" => {
                let s = "h\u{e9}llo \u{65e5}\u{672c}";
                b.extend_from_slice(&(s.len() as u32).to_le_bytes());
                b.extend_from_slice(s.as_bytes());
            }
            "points" => {
                b.extend_from_slice(&2u32.to_le_bytes());
                for v in [1.0f32, -2.5, 3.25, 4.0] {
                    b.extend_from_slice(&v.to_bits().to_le_bytes());
                }
            }
            "floats" => {
                b.extend_from_slice(&3u32.to_le_bytes());
                for v in [4.0f32, 2.0, 0.5] {
                    b.extend_from_slice(&v.to_bits().to_le_bytes());
                }
            }
            "stops" => {
                b.extend_from_slice(&2u32.to_le_bytes());
                for (o, c) in [(0.0f32, 0xff00_00ffu32), (1.0, 0x0000_ffff)] {
                    b.extend_from_slice(&o.to_bits().to_le_bytes());
                    b.extend_from_slice(&c.to_le_bytes());
                }
            }
            other => panic!("operand kind {other:?} has no sample"),
        }
    }
    b
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn every_command_encodes_to_the_recorded_bytes() {
    let mut lines = Vec::new();
    for (i, c) in COMMANDS.iter().enumerate() {
        let wire = sample(c.opcode, c.operands, i as u32);
        let mut r = Reader::new(&wire);
        let cmd = Command::decode(&mut r).unwrap_or_else(|e| panic!("{} does not decode: {e}", c.name));
        assert_eq!(r.remaining(), 0, "{} left bytes", c.name);
        let mut out = Vec::new();
        cmd.encode(&mut out);
        assert_eq!(out, wire, "{} does not re-encode to the bytes it was decoded from", c.name);
        lines.push(format!("0x{:02X} {} {}", c.opcode, c.name, hex(&out)));
    }
    let text = lines.join("\n") + "\n";
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/display-list-v1-commands.txt");
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(path, &text).unwrap();
    }
    let recorded = std::fs::read_to_string(path).expect("run once with UPDATE_GOLDEN=1");
    assert_eq!(text, recorded, "a command encodes to other bytes than the recorded ones");
    println!("wire_bytes: {} commands checked against the recorded bytes", COMMANDS.len());
}
