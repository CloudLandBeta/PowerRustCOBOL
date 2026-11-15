// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The round-trip test packages a copy of its own executable. On macOS that inserts a
//! `__PRPAYLOAD` segment command into the Mach-O header, which needs room the linker leaves only
//! when asked: the runner (T15) is linked the same way.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg-tests=-Wl,-headerpad,0x1000");
    }
}
