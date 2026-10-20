// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Edited pictures (numeric-edited and alphanumeric-edited receivers).
//!
//! Phase B of WASM-010 (R18): until the template interpreter of
//! `cobolt-forms/src/numedit.rs` is brought across, a program that moves into
//! an edited item is refused at compile time by the generator, so these entry
//! points are unreachable from generated code.

use crate::mv::Sender;
use crate::opd::Loc;
use crate::state::err;
use crate::val::Val;

pub fn move_to_edited(_snd: &Sender, _d: &Loc) {
    crate::fail(err::BAD_OPERAND)
}

pub fn deedit_value(_loc: &Loc) -> Val {
    crate::fail(err::BAD_OPERAND)
}
