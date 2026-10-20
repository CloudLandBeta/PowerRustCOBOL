// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The support routines of the WebAssembly line (spec WASM-010).
//!
//! The compiler (`cobolt-wasm`) turns a program's control flow into
//! WebAssembly and leaves everything that has *meaning* — how a `PIC S9(5)V99`
//! is read from its bytes, what `MOVE` does between two categories, how `ADD ...
//! ROUNDED` stores, what a comparison of a numeric and an alphanumeric item
//! says — to this crate. It is compiled to `wasm32` once, committed as a blob,
//! and linked statically into every generated module with the functions the
//! program does not reach removed. **There is no interpreter and no AST in the
//! module**: the generated code calls these functions with the addresses of
//! small static *operand records* it also generated.
//!
//! # Addresses, not pointers
//!
//! Every function takes `u32` addresses. On `wasm32` an address is an offset in
//! linear memory 0 and is turned into a pointer by adding nothing; in the
//! native unit tests memory is a `Vec<u8>` ([`mem::TestMem`]) and the same code
//! runs against it. One body of code, two memories.
//!
//! # Rules this crate keeps
//!
//! * `no_std`, no allocator, no function pointers, no formatting machinery
//!   (`core::fmt`), no panics on the happy path: a size that would not fit is a
//!   checked error, never an allocation.
//! * Behaviour is the interpreter's wherever the interpreter follows the
//!   standard, and the standard's where it does not (WASM-PLAN P5). The
//!   decimal arithmetic *is* the interpreter's: [`cobolt_decimal`].
//! * Memory is never grown; scratch space is static.
//!
//! The data layouts the generator and this crate agree on are documented in
//! [`fd`] (field descriptors) and [`opd`] (operand records).

#![cfg_attr(target_arch = "wasm32", no_std)]
#![allow(clippy::missing_safety_doc)]

pub mod accept;
pub mod arith;
pub mod cmp;
pub mod codec;
pub mod disp;
pub mod edited;
pub mod fd;
pub mod file;
pub mod float;
pub mod func;
pub mod host;
pub mod mem;
pub mod mv;
pub mod opd;
pub mod state;
pub mod strings;
#[cfg(not(target_arch = "wasm32"))]
pub mod testkit;
pub mod val;

use state::st;

/// Report run-time error `code` to the host and stop. The host's `abort`
/// traps; if a host ever returned, we trap here.
#[cold]
pub fn fail(code: u32) -> ! {
    // SAFETY: the glue function only reads the code.
    unsafe { host::abort(code) };
    #[cfg(target_arch = "wasm32")]
    core::arch::wasm32::unreachable();
    #[cfg(not(target_arch = "wasm32"))]
    panic!("run-time error {code}");
}

// ── The exported surface ───────────────────────────────────────────────────
//
// Every function below is `extern "C"` so that it is a plain WebAssembly
// function with `i32` parameters; the linker (`cobolt-wasm`) keeps the ones a
// program reaches. Arguments named `base` are the address of the current form
// instance's region; `o`, `a`, `b`, `from`, `to` are addresses of operand
// records (see `opd`).

/// Give the routines their workspace and reset their state.
#[no_mangle]
pub extern "C" fn rt_init(ws: u32) {
    let s = st();
    *s = state::State::new();
    s.ws = ws;
}

/// Set the form instance whose handler is about to run.
#[no_mangle]
pub extern "C" fn rt_enter(inst: u32) {
    st().inst = inst;
}

/// `MOVE from TO to`.
#[no_mangle]
pub extern "C" fn rt_move(base: u32, from: u32, to: u32) {
    let snd = mv::sender(base, &opd::parse(from));
    mv::move_to(base, &snd, &opd::parse(to));
}

/// First half of `MOVE from TO to1 to2 …`: read the sender once.
#[no_mangle]
pub extern "C" fn rt_move_begin(base: u32, from: u32) {
    st().snd = Some(mv::sender(base, &opd::parse(from)));
}

/// Second half: move the sender read by [`rt_move_begin`] into one receiver.
#[no_mangle]
pub extern "C" fn rt_move_to(base: u32, to: u32) {
    match st().snd {
        Some(snd) => mv::move_to(base, &snd, &opd::parse(to)),
        None => fail(state::err::BAD_OPERAND),
    }
}

/// `a op b` for a relation condition; `op` is one of `cmp::OP_*`.
#[no_mangle]
pub extern "C" fn rt_rel(base: u32, a: u32, b: u32, op: u32) -> i32 {
    cmp::relation(base, a, b, op) as i32
}

/// `o IS class` (`cmp::CLASS_*`).
#[no_mangle]
pub extern "C" fn rt_class(base: u32, o: u32, class: u32) -> i32 {
    cmp::class_test(base, o, class) as i32
}

/// `o IS POSITIVE | NEGATIVE | ZERO` (`cmp::SIGN_*`).
#[no_mangle]
pub extern "C" fn rt_sign(base: u32, o: u32, sign: u32) -> i32 {
    cmp::sign_test(base, o, sign) as i32
}

/// Start an arithmetic statement: empty the register stack, clear the size error.
#[no_mangle]
pub extern "C" fn rt_begin() {
    arith::begin_statement();
}

/// Push the numeric value of operand `o`.
#[no_mangle]
pub extern "C" fn rt_push(base: u32, o: u32) {
    arith::push_operand(base, o);
}

/// Apply `arith::OP_*` to the top registers.
#[no_mangle]
pub extern "C" fn rt_op(op: u32) {
    arith::apply(op);
}

/// Push a copy of the register `depth` below the top.
#[no_mangle]
pub extern "C" fn rt_pick(depth: u32) {
    arith::pick(depth);
}

/// Discard the top register.
#[no_mangle]
pub extern "C" fn rt_drop() {
    let _ = state::pop();
}

/// The integer part of operand `o` (a loop count, a subscript).
#[no_mangle]
pub extern "C" fn rt_int(base: u32, o: u32) -> i64 {
    match opd::value(base, &opd::parse(o)) {
        val::Val::Dec(n) => (n.mantissa / cobolt_decimal::pow10(n.decimals as u32).unwrap_or(1)) as i64,
        val::Val::F(f) => f as i64,
    }
}

/// The integer part of the top register (popped).
#[no_mangle]
pub extern "C" fn rt_reg_int() -> i64 {
    match state::pop() {
        val::Val::Dec(n) => (n.mantissa / cobolt_decimal::pow10(n.decimals as u32).unwrap_or(1)) as i64,
        val::Val::F(f) => f as i64,
    }
}

fn goto_index(v: i64, n: u32) -> i32 {
    if v >= 1 && v <= n as i64 {
        (v - 1) as i32
    } else {
        -1
    }
}

/// `GO TO … DEPENDING ON o`: the zero-based target index, or -1 when the
/// value is outside `1..=n` (control falls through).
#[no_mangle]
pub extern "C" fn rt_goto_index(base: u32, o: u32, n: u32) -> i32 {
    goto_index(rt_int(base, o), n)
}

/// As [`rt_goto_index`] for a computed value on the register stack.
#[no_mangle]
pub extern "C" fn rt_reg_goto_index(n: u32) -> i32 {
    goto_index(rt_reg_int(), n)
}

/// Compare the two topmost registers numerically (`a` below `b`), popping both.
#[no_mangle]
pub extern "C" fn rt_rel_reg(op: u32) -> i32 {
    let b = state::pop();
    let a = state::pop();
    cmp::numeric_relation(&a, &b, op) as i32
}

/// Is the top register zero? Pops it.
#[no_mangle]
pub extern "C" fn rt_reg_is_zero() -> i32 {
    state::pop().is_zero() as i32
}

/// Pop the top register into item `o` (`arith::ST_*`); 1 on a size error.
#[no_mangle]
pub extern "C" fn rt_store(base: u32, o: u32, flags: u32) -> i32 {
    arith::store(base, o, flags) as i32
}

/// Truncate the top register to the scale of item `o`.
#[no_mangle]
pub extern "C" fn rt_trunc(base: u32, o: u32) {
    arith::truncate_to_item(base, o);
}

/// Did the statement hit a size error?
#[no_mangle]
pub extern "C" fn rt_size_err() -> i32 {
    st().size_err as i32
}

/// `DISPLAY` one operand.
#[no_mangle]
pub extern "C" fn rt_display(base: u32, o: u32) {
    disp::display_operand(base, o);
}

/// `DISPLAY` the top register.
#[no_mangle]
pub extern "C" fn rt_display_reg() {
    let v = state::pop();
    disp::display_val(&v);
}

/// End a `DISPLAY` statement.
#[no_mangle]
pub extern "C" fn rt_display_end(no_advancing: u32) {
    disp::display_end(no_advancing != 0);
}

/// Hand the staged command batch to the host.
#[no_mangle]
pub extern "C" fn rt_flush() {
    disp::flush();
}

#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

/// `OPEN` (conformance builds): returns the 2-digit file status.
#[no_mangle]
pub extern "C" fn rt_file_open(base: u32, fd: u32, mode: u32) -> u32 {
    file::open(base, fd, mode)
}

#[no_mangle]
pub extern "C" fn rt_file_close(base: u32, fd: u32) -> u32 {
    file::close(base, fd)
}

#[no_mangle]
pub extern "C" fn rt_file_write(base: u32, fd: u32, rec: u32, adv: u32) -> u32 {
    file::write(base, fd, rec, adv)
}

/// `READ`: 0 read, 10 at end, otherwise a status.
#[no_mangle]
pub extern "C" fn rt_file_read(base: u32, fd: u32, rec: u32) -> u32 {
    file::read(base, fd, rec)
}

/// `INSPECT target <spec>`.
#[no_mangle]
pub extern "C" fn rt_inspect(base: u32, target: u32, spec: u32) {
    strings::inspect(base, target, spec);
}

/// `STRING … INTO …`: 1 when it overflowed.
#[no_mangle]
pub extern "C" fn rt_string(base: u32, spec: u32) -> u32 {
    strings::string(base, spec)
}

/// `UNSTRING …`: 1 when it overflowed.
#[no_mangle]
pub extern "C" fn rt_unstring(base: u32, spec: u32) -> u32 {
    strings::unstring(base, spec)
}

/// Pop the top register into temporary `k`.
#[no_mangle]
pub extern "C" fn rt_save_temp(k: u32) {
    let v = state::pop();
    state::st().temps[(k as usize) % state::NTEMP] = v;
}

/// Store the integer `n` into numeric item `o` (an index, a counter).
#[no_mangle]
pub extern "C" fn rt_set_int(base: u32, o: u32, n: i64) {
    let loc = opd::locate(base, &opd::parse(o));
    codec::store_value(&loc, val::Val::int(n), false, false);
}

/// Three-way comparison of two operands: -1, 0 or 1.
#[no_mangle]
pub extern "C" fn rt_cmp3(base: u32, a: u32, b: u32) -> i32 {
    if cmp::relation(base, a, b, cmp::OP_EQ) {
        0
    } else if cmp::relation(base, a, b, cmp::OP_LT) {
        -1
    } else {
        1
    }
}

/// `INITIALIZE`.
#[no_mangle]
pub extern "C" fn rt_initialize(base: u32, spec: u32) {
    strings::initialize(base, spec);
}

/// Empty the register stack without touching the statement's size-error flag
/// (a statement that does several steps, as `ADD CORRESPONDING` does).
#[no_mangle]
pub extern "C" fn rt_regs_reset() {
    let s = state::st();
    s.sp = 0;
    s.div0 = false;
}

/// `x IS [NOT] user-class`: `table` is a 32-byte character bitmap.
#[no_mangle]
pub extern "C" fn rt_class_user(base: u32, o: u32, table: u32) -> i32 {
    cmp::user_class(base, o, table) as i32
}

/// `ACCEPT target [FROM DATE | TIME | DAY | DAY-OF-WEEK]` (`accept::ACC_*`).
#[no_mangle]
pub extern "C" fn rt_accept(base: u32, target: u32, kind: u32) {
    accept::accept(base, target, kind);
}

/// Seed the program's external switches (conformance builds): `table` is
/// `[count u32]` then, per switch, `[ext_addr, ext_len, mnemonic_addr,
/// mnemonic_len, item_offset]` (all u32). The host is asked for the
/// implementor name, then for the mnemonic; the answer is written to the
/// mnemonic's one-character item as '1' or '0', which the switch's
/// `ON STATUS` / `OFF STATUS` condition-names read.
#[no_mangle]
pub extern "C" fn rt_switches(base: u32, table: u32) {
    let n = mem::rd_u32(table);
    for i in 0..n {
        let e = table + 4 + i * 20;
        let (ea, el, ma, ml, off) = (
            mem::rd_u32(e),
            mem::rd_u32(e + 4),
            mem::rd_u32(e + 8),
            mem::rd_u32(e + 12),
            mem::rd_u32(e + 16),
        );
        // SAFETY: the glue copies the name to the arena batch and calls the host.
        let mut st = unsafe { host::switch_state(ea, el) };
        if st < 0 {
            st = unsafe { host::switch_state(ma, ml) };
        }
        if st >= 0 {
            mem::wr_u8(base + off, b'0' + (st & 1) as u8);
        }
    }
}

/// Evaluate a numeric intrinsic function (call record `rec`) and push its result.
#[no_mangle]
pub extern "C" fn rt_func(base: u32, rec: u32) {
    func::eval_num(base, rec);
}

/// Evaluate a character intrinsic function into character temporary `k`.
#[no_mangle]
pub extern "C" fn rt_func_chars(base: u32, rec: u32, k: u32) {
    func::eval_chars(base, rec, k);
}

/// Install the program collating sequence: `table` is `[low][high][u16 × 256]`.
#[no_mangle]
pub extern "C" fn rt_set_collation(table: u32) {
    state::st().collate = table;
}
