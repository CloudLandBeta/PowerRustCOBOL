// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `prc-runner`: the PowerRustCOBOL runner. A Build copies it, puts the program inside the copy and
//! names the copy after the project; this is what then runs when the program starts.

fn main() {
    std::process::exit(cobolt_runner::run_self());
}
