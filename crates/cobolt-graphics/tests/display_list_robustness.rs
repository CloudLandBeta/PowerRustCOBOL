// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Robustness of the display list (WASM-018 R6, R15, AC10): the decoder never
//! panics on any bytes and refuses past the limits; the executor never panics on
//! any *valid* list however extreme its numbers.
//!
//! Randomness is a fixed-seed xorshift generator, so a failure reproduces.
//! `GRAPHICS_FUZZ_ITERS=<n>` multiplies the work for a longer run.

use cobolt_graphics::canvas::{Surface, SurfaceConfig};
use cobolt_graphics::display_list::{DisplayList, DisplayListBuilder, COMMANDS, HEADER_LEN, MAGIC, VERSION};
use cobolt_graphics::resources::NoResources;
use cobolt_graphics::{Limits, Rgba};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn chance(&mut self, one_in: u64) -> bool {
        self.below(one_in) == 0
    }
    /// A number a drawing might meet: mostly ordinary, sometimes hostile.
    fn float(&mut self) -> f32 {
        match self.below(20) {
            0 => f32::NAN,
            1 => f32::INFINITY,
            2 => f32::NEG_INFINITY,
            3 => f32::MAX,
            4 => f32::MIN,
            5 => f32::MIN_POSITIVE,
            6 => 0.0,
            7 => -0.0,
            8 => 1.0e9,
            9 => -1.0e9,
            10 => 1.0e-9,
            11 => (self.below(2000) as f32 - 1000.0) * 100.0,
            _ => (self.below(4000) as f32) / 10.0 - 100.0,
        }
    }
    fn small(&mut self) -> f32 {
        (self.below(300) as f32) / 3.0 - 20.0
    }
}

fn iters() -> u64 {
    std::env::var("GRAPHICS_FUZZ_ITERS").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(1).max(1)
}

fn header(count: u32, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&MAGIC);
    v.extend_from_slice(&VERSION.to_le_bytes());
    v.extend_from_slice(&0u16.to_le_bytes());
    v.extend_from_slice(&count.to_le_bytes());
    v.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    v.extend_from_slice(payload);
    v
}

/// One random command written straight from the command table's operand kinds.
fn random_command(rng: &mut Rng, out: &mut Vec<u8>, hostile: bool) {
    let info = COMMANDS[rng.below(COMMANDS.len() as u64) as usize];
    out.push(info.opcode);
    for (_, kind) in info.operands {
        match *kind {
            "u8" => out.push(if rng.chance(6) { rng.next() as u8 } else { rng.below(6) as u8 }),
            "u32" => out.extend_from_slice(&(if rng.chance(8) { rng.next() as u32 } else { rng.below(4) as u32 }).to_le_bytes()),
            "f32" => {
                let v = if hostile { rng.float() } else { rng.small() };
                out.extend_from_slice(&v.to_bits().to_le_bytes());
            }
            "rgba" => out.extend_from_slice(&(rng.next() as u32).to_le_bytes()),
            "str" => {
                let n = rng.below(12) as usize;
                let s: String = (0..n).map(|_| ['a', 'Z', ' ', '日', 'א', '\n', '😀', '.'][rng.below(8) as usize]).collect();
                out.extend_from_slice(&(s.len() as u32).to_le_bytes());
                out.extend_from_slice(s.as_bytes());
            }
            "points" => {
                let n = rng.below(7) as u32;
                out.extend_from_slice(&n.to_le_bytes());
                for _ in 0..n * 2 {
                    let v = if hostile { rng.float() } else { rng.small() };
                    out.extend_from_slice(&v.to_bits().to_le_bytes());
                }
            }
            "floats" => {
                let n = rng.below(6) as u32;
                out.extend_from_slice(&n.to_le_bytes());
                for _ in 0..n {
                    let v = if hostile { rng.float() } else { rng.small().abs() };
                    out.extend_from_slice(&v.to_bits().to_le_bytes());
                }
            }
            "stops" => {
                let n = rng.below(5) as u32;
                out.extend_from_slice(&n.to_le_bytes());
                for _ in 0..n {
                    out.extend_from_slice(&((rng.below(130) as f32) / 100.0 - 0.1).to_bits().to_le_bytes());
                    out.extend_from_slice(&(rng.next() as u32).to_le_bytes());
                }
            }
            other => panic!("the command table has an operand kind this fuzzer does not know: {other}"),
        }
    }
}

fn random_list(rng: &mut Rng, commands: usize, hostile: bool) -> Vec<u8> {
    let mut payload = Vec::new();
    for _ in 0..commands {
        random_command(rng, &mut payload, hostile);
    }
    header(commands as u32, &payload)
}

#[test]
fn arbitrary_bytes_never_panic_the_decoder() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let lim = Limits::default();
    let mut parsed = 0u32;
    for _ in 0..200_000 * iters() {
        let n = rng.below(200) as usize;
        let mut bytes: Vec<u8> = (0..n).map(|_| rng.next() as u8).collect();
        // Half the time make the header plausible so the commands are reached.
        if rng.chance(2) && n >= HEADER_LEN {
            bytes[0..4].copy_from_slice(&MAGIC);
            bytes[4..6].copy_from_slice(&VERSION.to_le_bytes());
            bytes[6..8].copy_from_slice(&[0, 0]);
            let payload = (n - HEADER_LEN) as u32;
            bytes[12..16].copy_from_slice(&payload.to_le_bytes());
            let count = rng.below(payload as u64 + 2) as u32;
            bytes[8..12].copy_from_slice(&count.to_le_bytes());
        }
        if let Ok(l) = DisplayList::parse(&bytes, &lim) {
            parsed += 1;
            let _ = l.validate();
            for c in l.commands() {
                let _ = c;
            }
        }
    }
    assert!(parsed > 1000, "the fuzzer should reach the command decoder often; it did {parsed} times");
}

#[test]
fn mutated_valid_lists_never_panic_and_a_decodable_list_re_encodes_to_the_same_bytes() {
    let mut rng = Rng(0xDEAD_BEEF_1234_5678);
    let lim = Limits::default();
    let (mut decodable, mut refused) = (0u32, 0u32);
    for _ in 0..30_000 * iters() {
        let n_cmds = 1 + rng.below(12) as usize;
        let base = random_list(&mut rng, n_cmds, true);
        let mut bytes = base.clone();
        match rng.below(4) {
            0 => {
                // flip a few bytes
                for _ in 0..1 + rng.below(4) {
                    let i = rng.below(bytes.len() as u64) as usize;
                    bytes[i] ^= 1 << rng.below(8);
                }
            }
            1 => {
                // truncate
                let cut = rng.below(bytes.len() as u64) as usize;
                bytes.truncate(cut);
            }
            2 => {
                // append garbage
                for _ in 0..rng.below(8) {
                    bytes.push(rng.next() as u8);
                }
            }
            _ => {}
        }
        match DisplayList::parse(&bytes, &lim) {
            Ok(l) => match l.commands().collect::<Result<Vec<_>, _>>() {
                Ok(cmds) => {
                    decodable += 1;
                    // The encoding is canonical: decoding and encoding again gives the payload back.
                    let mut again = Vec::new();
                    for c in &cmds {
                        c.encode(&mut again);
                    }
                    assert_eq!(again.as_slice(), l.payload(), "re-encoding changed the bytes");
                }
                Err(_) => refused += 1,
            },
            Err(_) => refused += 1,
        }
    }
    assert!(decodable > 3000 && refused > 3000, "decodable {decodable}, refused {refused}");
}

#[test]
fn the_decoder_refuses_what_the_limits_forbid() {
    let mut b = DisplayListBuilder::new();
    for _ in 0..100 {
        b.fill_rect(0.0, 0.0, 1.0, 1.0);
    }
    let bytes = b.take().unwrap();
    let small = Limits { max_display_list_bytes: 500, ..Limits::default() };
    let e = DisplayList::parse(&bytes, &small).unwrap_err();
    assert_eq!(e.code(), "limit-exceeded", "{e}");
    assert!(e.to_string().contains("max_display_list_bytes"));
    let few = Limits { max_display_list_commands: 50, ..Limits::default() };
    let e = DisplayList::parse(&bytes, &few).unwrap_err();
    assert!(e.to_string().contains("max_display_list_commands"));
    // A header that promises more commands than bytes could hold is refused before anything is allocated.
    let mut lie = header(4_000_000_000, &[1, 2, 3]);
    assert!(DisplayList::parse(&lie, &Limits::default()).is_err());
    lie[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(DisplayList::parse(&lie, &Limits::default()).is_err());
    // The surface refuses an over-long list without drawing.
    let mut s = Surface::new(&SurfaceConfig::new(8, 8), &small).unwrap();
    assert!(s.execute(&bytes, &NoResources).is_err());
}

#[test]
fn the_executor_survives_any_valid_list() {
    let mut rng = Rng(0x0123_4567_89AB_CDEF);
    let lim = Limits { max_path_points: 2000, max_save_depth: 16, ..Limits::default() };
    let (mut ran, mut failed) = (0u32, 0u32);
    for i in 0..1500 * iters() {
        let hostile = i % 3 != 0;
        let n_cmds = 1 + rng.below(40) as usize;
        let list = random_list(&mut rng, n_cmds, hostile);
        let w = 8 + rng.below(90) as u32;
        let h = 8 + rng.below(90) as u32;
        let cfg = SurfaceConfig { pixel_ratio: [1.0, 1.0, 2.0, 0.5][rng.below(4) as usize], tile_rows: if rng.chance(2) { Some(8 + rng.below(30) as u32) } else { None }, ..SurfaceConfig::new(w, h) };
        let mut s = Surface::new(&cfg, &lim).unwrap();
        let started = std::time::Instant::now();
        if std::env::var("GRAPHICS_FUZZ_TRACE").is_ok() {
            let names: Vec<_> = DisplayList::parse(&list, &lim).unwrap().commands().map(|c| format!("{:?}", c.unwrap())).collect();
            eprintln!("list {i} ({w}x{h}, {cfg:?}):\n{}", names.join("\n"));
        }
        match s.execute(&list, &NoResources) {
            Ok(_) => ran += 1,
            Err(_) => failed += 1, // a limit (Save depth, path points): fine, as long as it is an Err
        }
        // No list may take long: report the commands of the first one that does.
        let took = started.elapsed();
        if took > std::time::Duration::from_secs(5) {
            let names: Vec<_> = DisplayList::parse(&list, &lim).unwrap().commands().map(|c| format!("{:?}", c.unwrap())).collect();
            panic!("list {i} ({w}x{h}, {cfg:?}) took {took:?}:\n{}", names.join("\n"));
        }
        // The picture can always be read back.
        let img = s.to_image();
        assert_eq!(img.width() as usize * img.height() as usize * 4, img.as_bytes().len());
    }
    assert!(ran > 1000, "ran {ran}, refused {failed}");
}

#[test]
fn extreme_numbers_in_shapes_do_not_hang_or_panic() {
    // Hand-picked nasties: huge radii, tiny dashes, enormous transforms, deep clips.
    let mut s = Surface::new(&SurfaceConfig::new(64, 64), &Limits::default()).unwrap();
    let mut b = DisplayListBuilder::new();
    b.set_fill_color(Rgba::BLACK).set_stroke_color(Rgba::rgb(255, 0, 0)).set_line_width(3.0);
    b.begin_path().arc(32.0, 32.0, 1.0e8, 0.0, 6.28, 0).stroke();
    b.begin_path().arc(32.0, 32.0, 1.0e-8, 0.0, 6.28, 0).fill(0);
    b.begin_path().ellipse(32.0, 32.0, 1.0e7, 1.0e-7, 1.0, 0.0, 6.28, 1).stroke();
    b.set_dash(&[1.0e-7, 1.0e-7], 0.0).draw_line(0.0, 0.0, 64.0, 64.0).draw_circle(32.0, 32.0, 30.0, 2);
    b.set_dash(&[], 0.0).scale(1.0e30, 1.0e30).fill_rect(0.0, 0.0, 1.0, 1.0).reset_transform();
    b.scale(1.0e-30, 1.0e-30).stroke_rect(0.0, 0.0, 1.0e30, 1.0e30).reset_transform();
    b.set_shadow(Rgba::BLACK, 1.0e6, 0.0, 0.0).fill_rect(10.0, 10.0, 20.0, 20.0);
    b.set_shadow(Rgba::BLACK, 3.0, 1.0e9, -1.0e9).fill_rect(10.0, 10.0, 20.0, 20.0);
    b.set_line_width(1.0e12).draw_line(0.0, 0.0, 64.0, 5.0);
    b.begin_path().move_to(0.0, 0.0).line_to(1.0e30, 1.0e30).line_to(-1.0e30, 5.0).fill(0).stroke();
    b.set_miter_limit(1.0e30).set_line_join(0).begin_path().move_to(0.0, 0.0).line_to(30.0, 31.0).line_to(0.0, 1.0).stroke();
    s.execute(&b.take().unwrap(), &NoResources).unwrap();
}
