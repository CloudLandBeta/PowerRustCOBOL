// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `prc-backend-runner`: the prebuilt runner of a Backend built as a WebAssembly module. A Build copies it, puts the
//! Backend inside the copy and names the copy after the project; this is what then runs when the Backend starts.

fn main() {
    std::process::exit(cobolt_backend_runner::run_self(std::env::args().skip(1).collect()));
}
