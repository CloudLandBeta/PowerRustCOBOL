// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A stand-in for a built backend, for the tests of the service options (spec 113): the same
//! `standalone::main` a built binary runs, with the packed backend read from the file named by
//! `PRC_PROBE_BUNDLE` instead of being compiled in.

fn main() {
    let path = std::env::var("PRC_PROBE_BUNDLE").expect("PRC_PROBE_BUNDLE names the packed backend");
    let bundle = std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    std::process::exit(cobolt_server::standalone::main(&bundle, std::env::args().skip(1).collect()));
}
