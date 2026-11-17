// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The external switches a run starts with (`SPECIAL-NAMES. SWITCH-1 IS SW-1 ON STATUS IS …`).
//!
//! A switch lives outside the program: COBOL-85 leaves its state to the operating environment.
//! `rcrun run` takes it from `--switch NAME=ON|OFF` and from the `COBOL_SWITCHES` environment
//! variable, a comma-separated list of the same pairs; a built program takes it from
//! `COBOL_SWITCHES`, on the Cargo path through [`external_switches_from_env`] and on the
//! WebAssembly path through the module's own reading of the variable (WASM-PARITY #55). One rule
//! for a pair serves all three, so they cannot read a value differently.

/// One `NAME=ON|OFF` pair: the name, trimmed and in capitals, and whether the switch is on. `ON`,
/// `1`, `TRUE` and `YES` turn it on and `OFF`, `0`, `FALSE` and `NO` off, in any case and with
/// white space around them. `None` when `spec` has no `=`; `Some(Err(state))` when the state is
/// none of those words, so a caller can say which one it passed over.
pub fn external_switch(spec: &str) -> Option<Result<(String, bool), String>> {
    let (name, state) = spec.split_once('=')?;
    let on = match state.trim().to_ascii_uppercase().as_str() {
        "ON" | "1" | "TRUE" | "YES" => true,
        "OFF" | "0" | "FALSE" | "NO" => false,
        other => return Some(Err(other.to_string())),
    };
    Some(Ok((name.trim().to_ascii_uppercase(), on)))
}

/// The pairs `COBOL_SWITCHES` holds, in its order, for
/// [`crate::Interpreter::set_external_switches`] (the first pair that names a switch sets it); a
/// pair [`external_switch`] cannot read is passed over. Empty when the variable is not set.
pub fn external_switches_from_env() -> Vec<(String, bool)> {
    std::env::var("COBOL_SWITCHES").map(|v| v.split(',').filter_map(|s| external_switch(s)?.ok()).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::external_switch;

    #[test]
    fn a_pair_is_read_in_any_case_with_white_space_around_it() {
        let cases: &[(&str, Option<Result<(&str, bool), &str>>)] = &[
            ("SWITCH-1=ON", Some(Ok(("SWITCH-1", true)))),
            (" switch-2 = off ", Some(Ok(("SWITCH-2", false)))),
            ("SW-1=yes", Some(Ok(("SW-1", true)))),
            ("SW-1=True", Some(Ok(("SW-1", true)))),
            ("SW-1=1", Some(Ok(("SW-1", true)))),
            ("SW-1=no", Some(Ok(("SW-1", false)))),
            ("SW-1=FALSE", Some(Ok(("SW-1", false)))),
            ("SW-1=0", Some(Ok(("SW-1", false)))),
            ("SW-1=maybe", Some(Err("MAYBE"))),
            ("SW-1=", Some(Err(""))),
            ("=ON", Some(Ok(("", true)))),
            ("SW-1", None),
            ("", None),
        ];
        for (spec, want) in cases {
            let got = external_switch(spec);
            let want = want.map(|r| r.map(|(n, on)| (n.to_string(), on)).map_err(str::to_string));
            assert_eq!(got, want, "{spec:?}");
        }
        println!("── a_pair_is_read_in_any_case_with_white_space_around_it ──");
        println!("  {} pairs: the eight state words, an unknown and an empty state, an empty name, no `=`", cases.len());
        println!("  checks: {} of {} passed", cases.len(), cases.len());
    }
}
