// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Configure Claude Code** (spec 084): the plugin the IDE hands the
//! developer's Claude Code ([`bundle`]), and the `claude` commands that install
//! it once for every project ([`configure`]).

pub mod bundle;
pub mod configure;
