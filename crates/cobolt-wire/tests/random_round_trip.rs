// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The mapping against layouts and values nobody wrote by hand (WASM-005 R6,
//! WASM-013 R15 — the JSON-to-group fuzz target, run here with a fixed seed so
//! it is reproducible and cheap enough for every run).
//!
//! Two properties, over thousands of random layouts:
//!
//! 1. **Round trip is the identity.** Any state the generator can build is
//!    encoded and read back into a blank group; what travelled is the same.
//! 2. **Garbage never panics and never corrupts.** The encoded text is mutated
//!    byte by byte; decoding either refuses it (with a named error) or accepts a
//!    state that still encodes. A refused read leaves the group as it was
//!    (decoding borrows it).

use cobolt_wire::layout::{validate, Item, Kind, Occurs};
use cobolt_wire::node::{dump, Node};
use cobolt_wire::number::from_mantissa;
use cobolt_wire::{decode, encode};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, pct: u64) -> bool {
        self.below(100) < pct
    }
}

fn leaf(rng: &mut Rng, name: String) -> Item {
    let kind = match rng.below(6) {
        0 => Kind::Alphanumeric { len: 1 + rng.below(12) as usize, justified: rng.chance(20) },
        1 => Kind::Numeric { digits: 1 + rng.below(8) as u16, scale: rng.below(4) as u16, signed: rng.chance(50) },
        2 => Kind::Numeric { digits: 1 + rng.below(30) as u16, scale: rng.below(8) as u16, signed: true },
        3 => Kind::National { len: 1 + rng.below(6) as usize },
        4 => Kind::Utf8 { len: 1 + rng.below(6) as usize, bytes: None },
        _ => Kind::Edited { len: 1 + rng.below(9) as usize },
    };
    Item { name, filler: false, redefines: false, occurs: None, kind, value: None }
}

fn group(rng: &mut Rng, name: String, depth: u32, counter: &mut u32) -> Item {
    let n = 1 + rng.below(5);
    let mut children: Vec<Item> = Vec::new();
    for _ in 0..n {
        *counter += 1;
        let name = format!("M{}", *counter);
        let mut item = if depth < 3 && rng.chance(30) {
            group(rng, name, depth + 1, counter)
        } else {
            leaf(rng, name)
        };
        if rng.chance(8) {
            // an unnamed filler, never a member
            item.filler = true;
            item.name = "FILLER".into();
        } else if rng.chance(6) && !children.is_empty() {
            item.redefines = true;
        } else if rng.chance(25) && depth < 3 {
            if rng.chance(50) {
                item.occurs = Some(Occurs { min: 1 + rng.below(3) as usize, max: 4 + rng.below(3) as usize, depending_on: None });
                if let Some(o) = item.occurs.as_mut() {
                    o.min = o.max;
                }
            } else {
                // a variable table needs a count item before it
                *counter += 1;
                let count = Item {
                    name: format!("N{}", *counter),
                    filler: false,
                    redefines: false,
                    occurs: None,
                    kind: Kind::Numeric { digits: 1, scale: 0, signed: false },
                    value: None,
                };
                let dep = count.name.clone();
                children.push(count);
                item.occurs = Some(Occurs { min: 0, max: 1 + rng.below(5) as usize, depending_on: Some(dep) });
            }
        }
        children.push(item);
    }
    Item { name, filler: false, redefines: false, occurs: None, kind: Kind::Group { children }, value: None }
}

fn random_text(rng: &mut Rng, max_chars: usize) -> String {
    const ALPHABET: &[char] = &['a', 'b', 'Z', '0', '9', ' ', '"', '\\', '/', '\n', '\t', '\u{1}', 'ç', 'ã', '日', '😀', '-', '.'];
    let n = rng.below(max_chars as u64 + 1) as usize;
    (0..n).map(|_| ALPHABET[rng.below(ALPHABET.len() as u64) as usize]).collect()
}

/// Fill a node with random but legal content.
fn fill(rng: &mut Rng, item: &Item, node: &mut Node) {
    match (&item.kind, node) {
        (Kind::Group { children }, Node::Group(nodes)) => {
            for (c, n) in children.iter().zip(nodes.iter_mut()) {
                fill_item(rng, c, n);
            }
        }
        _ => unreachable!("fill_base is called through fill_item"),
    }
}

fn fill_item(rng: &mut Rng, item: &Item, node: &mut Node) {
    if item.filler || item.redefines {
        return;
    }
    match node {
        Node::Table(elems) => {
            for e in elems.iter_mut() {
                fill_base(rng, item, e);
            }
        }
        other => fill_base(rng, item, other),
    }
}

fn fill_base(rng: &mut Rng, item: &Item, node: &mut Node) {
    match (&item.kind, node) {
        (Kind::Group { .. }, n @ Node::Group(_)) => fill(rng, item, n),
        (Kind::Numeric { digits, scale, signed }, Node::Leaf(b)) => {
            let total = (*digits + *scale).min(36) as u32;
            let mut m = (rng.next() as u128 % 10u128.pow(total.min(30))) as i128;
            if item.name.starts_with('N') {
                m = rng.below(2) as i128; // counts stay small; the tables below clamp
            }
            if *signed && rng.chance(40) {
                m = -m;
            }
            *b = from_mantissa(m, *scale).into_bytes();
        }
        (kind, Node::Leaf(b)) => {
            let max = match kind {
                Kind::Alphanumeric { len, .. } | Kind::Edited { len } => *len,
                Kind::National { len } | Kind::Utf8 { len, .. } => *len,
                _ => 0,
            };
            // Keep the text within its limit in the unit that kind counts.
            let mut s = random_text(rng, max);
            loop {
                let fits = match kind {
                    Kind::Alphanumeric { len, .. } | Kind::Edited { len } => s.len() <= *len,
                    Kind::National { len } => s.encode_utf16().count() <= *len,
                    Kind::Utf8 { len, .. } => s.chars().count() <= *len,
                    _ => true,
                };
                if fits {
                    break;
                }
                s.pop();
            }
            // A JUSTIFIED RIGHT item never ends in a space unless it is all spaces
            // (a MOVE drops the sender's trailing spaces before it aligns), so the
            // generator does not build the state that no MOVE can.
            if matches!(kind, Kind::Alphanumeric { justified: true, .. }) {
                s = s.trim_end_matches(' ').to_string();
            }
            *b = cobolt_wire::node::pad_text(kind, s.as_bytes());
        }
        _ => {}
    }
}

/// Set every count item to a value inside its table's bounds, so the state is encodable.
fn settle_counts(item: &Item, node: &mut Node, rng: &mut Rng) {
    if let (Kind::Group { children }, Node::Group(nodes)) = (&item.kind, &mut *node) {
        for i in 0..children.len() {
            if let Some(Occurs { depending_on: Some(dep), min, max }) = &children[i].occurs {
                if let Some(j) = children.iter().position(|c| c.name == *dep) {
                    let n = *min + rng.below((*max - *min) as u64 + 1) as usize;
                    nodes[j] = Node::Leaf(n.to_string().into_bytes());
                }
            }
        }
        for (c, n) in children.iter().zip(nodes.iter_mut()) {
            match n {
                Node::Table(elems) => elems.iter_mut().for_each(|e| settle_counts(c, e, rng)),
                other => settle_counts(c, other, rng),
            }
        }
    }
}

#[test]
fn random_layouts_round_trip_and_garbage_is_refused_without_panic() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let (mut layouts, mut skipped, mut states, mut json_bytes, mut mutations, mut refused, mut accepted) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let started = std::time::Instant::now();

    for _ in 0..1500 {
        let mut counter = 0;
        let root = group(&mut rng, "ROOT".into(), 0, &mut counter);
        if validate(&root).is_err() {
            skipped += 1;
            continue;
        }
        layouts += 1;
        for _ in 0..3 {
            let mut state = Node::initial(&root);
            fill(&mut rng, &root, &mut state);
            settle_counts(&root, &mut state, &mut rng);
            let text = match encode(&root, &state) {
                Ok(t) => t,
                Err(e) => panic!("a legal random state must encode: {e}\n{root:#?}"),
            };
            states += 1;
            json_bytes += text.len() as u64;

            // 1. Identity at the wire: read it into a blank group and write it again.
            let blank = Node::initial(&root);
            let back = decode(&root, &blank, &text, 1 << 22).unwrap_or_else(|e| panic!("what was written must be read: {e}\n{text}"));
            let again = encode(&root, &back).expect("re-encode");
            if again != text {
                let layout = serde_json::to_string(&root).unwrap();
                panic!("write -> read -> write is not the identity\n{text}\n{again}\n{layout}");
            }

            // 2. Mutations: flip, drop or double one byte at a time (a sample of positions).
            let bytes = text.as_bytes();
            let step = (bytes.len() / 24).max(1);
            for pos in (0..bytes.len()).step_by(step) {
                for kind in 0..3 {
                    let mut m = bytes.to_vec();
                    match kind {
                        0 => m[pos] = m[pos].wrapping_add(1 + (rng.below(200) as u8)),
                        1 => {
                            m.remove(pos);
                        }
                        _ => m.insert(pos, bytes[pos]),
                    }
                    let Ok(mutated) = String::from_utf8(m) else { continue };
                    mutations += 1;
                    let before = dump(&root, &blank);
                    match decode(&root, &blank, &mutated, 1 << 22) {
                        Err(_) => refused += 1,
                        Ok(n) => {
                            accepted += 1;
                            // An accepted read is a whole, writable state.
                            if let Err(e) = encode(&root, &n) {
                                assert_eq!(e.kind.code(), "not-utf8", "{e}");
                            }
                        }
                    }
                    assert_eq!(dump(&root, &blank), before, "the group read from is never changed");
                }
            }
        }
    }

    println!(
        "\n  random layouts: {layouts} layouts ({skipped} refused by validate), {states} states, {json_bytes} bytes of JSON written and read back identically; \
         {mutations} mutated texts: {refused} refused with a named error, {accepted} accepted as whole states, 0 panics; {:.0} ms",
        started.elapsed().as_secs_f64() * 1000.0
    );
    assert!(layouts > 1000 && states > 3000 && mutations > 100_000, "the run was big enough to mean something");
}
