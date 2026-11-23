// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! `prc-buildtools.exe`: the bundle's Build Tools step (spec 112, task T19).
//!
//! The bundle (`crates/cobolt-setup/wix/bundle.wxs`) chains this program before the `.msi`. It installs Microsoft's
//! C++ build tools when this computer has none, and does nothing when it has them. The decisions are
//! `cobolt_setup::buildtools`; the work is `cobolt_setup::buildtools::system`. The exit code is the one that
//! module decides: `0` (go on), `3010` (go on, restart at the end), `1602` or `1618` (end the bundle).

#[cfg(windows)]
fn main() {
    // `ExitCode::from` stops at 255, and Windows Installer's codes are larger.
    std::process::exit(cobolt_setup::buildtools::system::main() as i32);
}

#[cfg(not(windows))]
fn main() {
    eprintln!("prc-buildtools installs Microsoft's C++ build tools and runs on Windows only.");
    std::process::exit(2);
}
