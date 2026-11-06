// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Every platform code the player emits is the wire's** (WASM-014 T7: the codes-equality check).
//!
//! The player keeps a small mirror of `cobolt_wire::error::codes` (`net::code`) for the codes it writes
//! itself, and names the wire's own table (`codes::RATE_LIMITED`) where it only compares. This test reads
//! the two tables and the player's source as text and holds them together without a hand-kept list:
//!
//! * every constant in the mirror is in `cobolt_wire::error::codes` with the same number;
//! * every `code::NAME` and `codes::NAME` the player's non-test source writes exists in the wire's table,
//!   and the test prints each one, its value, which table the player reads it from and where;
//! * no code is written as a bare number, and no mirror constant is unused.
//!
//! It is an integration test on purpose: the freshness fingerprint covers `src/`, so a test written
//! inside `src/` would make the shipped player stale; this one does not.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// The arguments of the call that opens right after `from` (a one-line call), split at the top-level commas.
fn call_args(from: &str) -> Vec<String> {
    let (mut depth, mut args, mut cur) = (0i32, Vec::new(), String::new());
    for c in from.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth == 0 => {
                args.push(cur.trim().to_owned());
                return args;
            }
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                args.push(cur.trim().to_owned());
                cur.clear();
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    Vec::new() // the call goes on to the next line: not judged here
}

/// `pub const NAME: u32 = 123;` lines of the block that opens with `opener`, as (name, value) in order.
fn constants(text: &str, opener: &str) -> Vec<(String, u32)> {
    let from = text.find(opener).unwrap_or_else(|| panic!("`{opener}` not found"));
    let block = &text[from..];
    let block = &block[..block.find("\n}").expect("the block ends")];
    block
        .lines()
        .filter_map(|l| l.trim().strip_prefix("pub const "))
        .filter_map(|l| {
            let (name, rest) = l.split_once(':')?;
            let value = rest.split_once('=')?.1.trim().trim_end_matches(';').trim();
            Some((name.trim().to_owned(), value.parse().ok()?))
        })
        .collect()
}

#[test]
fn every_code_the_player_emits_is_the_wires() {
    let src = root().join("src");
    let net = std::fs::read_to_string(src.join("net.rs")).unwrap();
    let wire_text = std::fs::read_to_string(root().join("../cobolt-wire/src/error.rs")).unwrap();
    let mirror = constants(&net, "pub mod code {");
    let wire: BTreeMap<String, u32> = constants(&wire_text, "pub mod codes {").into_iter().collect();
    assert!(mirror.len() >= 9 && wire.len() >= 20, "the tables were read: mirror {}, wire {}", mirror.len(), wire.len());

    for (name, value) in &mirror {
        let w = wire.get(name).unwrap_or_else(|| panic!("net::code::{name} is not in cobolt_wire::error::codes"));
        assert_eq!(value, w, "{name}: the player's mirror says {value}, the wire says {w}");
    }

    // Every platform code the player's own source names, and every bare number written as a code.
    let mut used: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut literals = Vec::new();
    let mut files = 0;
    let mut walk = vec![src.clone()];
    while let Some(dir) = walk.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk.push(path);
                continue;
            }
            let Some(file) = path.file_name().and_then(|n| n.to_str()).filter(|n| n.ends_with(".rs") && !n.ends_with("_tests.rs")) else { continue };
            files += 1;
            let text = std::fs::read_to_string(&path).unwrap();
            let production = text.split("#[cfg(test)]").next().unwrap();
            for (n, line) in production.lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                for marker in ["code::", "codes::"] {
                    for (at, _) in line.match_indices(marker) {
                        let word: String = line[at + marker.len()..].chars().take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_').collect();
                        if word.len() > 1 && word.starts_with(|c: char| c.is_ascii_uppercase()) {
                            used.entry(word).or_default().push(format!("{file}:{}", n + 1));
                        }
                    }
                }
                // A code is named, never a number: in a struct field (`code: 1000`, `"code": 1000`) ...
                let field = ["\"code\": ", "code: "].iter().any(|k| line.find(k).is_some_and(|i| line[i + k.len()..].starts_with(|c: char| c.is_ascii_digit())));
                // ... or as the code argument of the calls that make an error (`(call, index of the code argument)`).
                let call = [("error_frame(", 2usize), (".fail(", 2), (".fail_all(", 1), ("refuse(", 2)].iter().any(|(open, idx)| {
                    line.match_indices(open).any(|(at, _)| call_args(&line[at + open.len()..]).get(*idx).is_some_and(|a| a.starts_with(|c: char| c.is_ascii_digit())))
                });
                if field || call {
                    literals.push(format!("{file}:{}", n + 1));
                }
            }
        }
    }

    println!("{:<18} {:<6} {:<8} used at", "code", "value", "read via");
    let mut through_mirror = 0;
    for (name, places) in &used {
        let value = wire.get(name).unwrap_or_else(|| panic!("{name} ({}) is not a code of cobolt_wire::error::codes", places[0]));
        let mirrored = mirror.iter().any(|(m, _)| m == name);
        through_mirror += usize::from(mirrored);
        println!("{name:<18} {value:<6} {:<8} {} place(s), first {}", if mirrored { "mirror" } else { "wire" }, places.len(), places[0]);
    }
    println!(
        "RESULT wire_codes: {} source files read, {} platform codes named ({through_mirror} through the mirror of {}, {} from the wire's table), {} of {} wire codes unused by the player, bare numeric codes: {}",
        files,
        used.len(),
        mirror.len(),
        used.len() - through_mirror,
        wire.len() - used.len(),
        wire.len(),
        literals.len()
    );
    assert!(literals.is_empty(), "a code must be named, not written as a number: {literals:?}");
    for (name, _) in &mirror {
        assert!(used.contains_key(name), "the mirror holds {name} but no source of the player uses it: remove it");
    }
}
