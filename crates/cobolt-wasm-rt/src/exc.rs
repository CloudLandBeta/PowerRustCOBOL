// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `THROW` and `TRY … CATCH` (DESKTOP-WASM-001 T45): the message of the exception in flight, and the
//! run-time error a `THROW` nobody catches ends the run with.
//!
//! The generated code does the unwinding: a `TRY` pushes a frame marked [`TRY_MARK`] on the stack of
//! `PERFORM` and `CALL` frames, and a `THROW` drops frames down to the nearest one. These routines only
//! keep the message. No statement of the program runs between a `THROW` and the `CATCH` that takes it,
//! so the message is read where the `THROW` found it, without a copy.

use crate::fd::{Fd, K_ALNUM};
use crate::opd::{self, Loc};
use crate::state::{err, st};

/// The end word of a `TRY`'s frame: no block's table index (they are never negative).
pub const TRY_MARK: i32 = -16;
/// What a `TRY`'s landing block finds in its resume slot after a `THROW` (any other value is the block
/// an unwinding `STOP RUN`, `GOBACK` or jump goes on to once `FINALLY` has run).
pub const THROWN: i32 = -1;

/// `THROW operand`: keep where its text is (an alphanumeric literal, item or group).
#[no_mangle]
pub extern "C" fn rt_throw_set(base: u32, o: u32) {
    let loc = opd::locate(base, &opd::parse(o));
    let s = st();
    s.exc_addr = loc.addr;
    s.exc_len = loc.len;
}

/// A `THROW` that no `TRY` takes: the run ends with the run-time error that carries the interpreter's
/// words for it (`RuntimeError::UserException`, "unhandled exception: …").
#[no_mangle]
pub extern "C" fn rt_throw_uncaught() {
    let s = st();
    let text = crate::mem::bytes(s.exc_addr, s.exc_len);
    crate::fail_text(err::UNHANDLED_EXCEPTION, &[crate::Part::T(b"unhandled exception: "), crate::Part::T(text)]);
}

/// `CATCH item`: the message goes into the item as an alphanumeric `MOVE` puts it there, which is what
/// the interpreter's `set_str` does with it (cut or padded, `JUSTIFIED` honoured, a group's fields filled).
#[no_mangle]
pub extern "C" fn rt_catch_bind(base: u32, to: u32) {
    let s = st();
    let len = s.exc_len;
    let loc = Loc { addr: s.exc_addr, len, fd: Fd { kind: K_ALNUM, flags: 0, digits: 0, scale: 0, len, aux: 0 } };
    crate::mv::move_to(base, &crate::mv::Sender::Bytes(loc, false), &opd::parse(to));
}
