// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

/// Current PowerRustCOBOL version string.
///
/// # Version convention  x.y.z
///
/// **x** — Increment when an entirely new platform component is added to the
///   solution (e.g. Web/WASM support, Android APK export, iOS target, cloud
///   deployment backend).  Reset y and z to 0.
///
/// **y** — Increment when new functionality is delivered within existing
///   components: new Form Designer widgets, new control properties, new IDE
///   panels or toolbar actions, new RustCOBOL language features, new rcrun
///   built-in CALLs, etc.  Reset z to 0.
///
/// **z** — Increment for bug fixes, visual polish, performance improvements,
///   and any change that does not add new user-visible functionality.
pub const VERSION: &str = "1.22.0";
