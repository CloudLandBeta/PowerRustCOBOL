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
pub mod call;
pub mod cmd;
pub mod cmp;
pub mod codec;
pub mod disp;
pub mod edited;
pub mod fd;
pub mod file;
pub mod float;
pub mod func;
pub mod host;
pub mod idx;
pub mod json;
pub mod mem;
pub mod mv;
pub mod opd;
pub mod remote;
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
    disp::flush_all();
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

/// Make character temporary `k` a receiving field of its full capacity: its
/// current text, then spaces (`STRING … INTO Ctrl::Prop`).
#[no_mangle]
pub extern "C" fn rt_ctemp_widen(k: u32) {
    func::ctemp_widen(k);
}

/// Drop the trailing spaces of character temporary `k`, as a property write
/// does on the interpreter.
#[no_mangle]
pub extern "C" fn rt_ctemp_rtrim(k: u32) {
    func::ctemp_rtrim(k);
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

/// `ACCEPT target FROM DATE | TIME | DAY | DAY-OF-WEEK` only: the clock, with no
/// file import behind it (what a browser module links).
#[no_mangle]
pub extern "C" fn rt_accept_clock(base: u32, target: u32, kind: u32) {
    accept::accept_clock(base, target, kind);
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

/// `CALL … BY REFERENCE`: point linkage slot `slot` at argument `src`.
#[no_mangle]
pub extern "C" fn rt_bind(base: u32, slot: u32, src: u32) {
    call::bind(base, slot, src);
}

/// `CALL … BY CONTENT / BY VALUE`: copy the argument for the call and bind it.
#[no_mangle]
pub extern "C" fn rt_bind_content(base: u32, slot: u32, src: u32, tmp: u32, dst: u32, tmp_len: u32) {
    call::bind_content(base, slot, src, tmp, dst, tmp_len);
}

/// The index of the program named by operand `name` in `table`, or -1.
#[no_mangle]
pub extern "C" fn rt_find_program(base: u32, name: u32, table: u32) -> i32 {
    call::find_program(base, name, table)
}

/// A `CALL` of a program that is not there, with nothing to catch it.
#[no_mangle]
pub extern "C" fn rt_program_not_found() {
    call::not_found();
}

/// Discard what a run that trapped left half-built (ABI A5, `pr_cancel(-1)`):
/// the staged batch, the command path and the registers, so the next run of
/// the instance starts clean.
#[no_mangle]
pub extern "C" fn rt_cancel_run() {
    let s = state::st();
    s.batch_len = 0;
    s.path_len = 0;
    s.sp = 0;
    s.snd = None;
}

/// `recv::Method(a, b)`, a value method of a data item, into character
/// temporary `k` (`strings::VM_*`; `a`, `b` 0 when absent).
#[no_mangle]
pub extern "C" fn rt_value_method(base: u32, method: u32, recv: u32, a: u32, b: u32, k: u32) {
    strings::value_method(base, method, recv, a, b, k);
}

/// The argument count of the next intrinsic-function call (an `ALL` over an
/// `OCCURS … DEPENDING ON` table, WASM-PARITY #7).
#[no_mangle]
pub extern "C" fn rt_func_argc(n: u32) {
    state::st().func_argc = n;
}

/// The form instance the module is running (for the paths of host commands).
#[no_mangle]
pub extern "C" fn rt_set_instance(inst: u32) {
    state::st().inst = inst;
}

/// Start a host command path on control `ctrl` of the running instance.
#[no_mangle]
pub extern "C" fn rt_path_begin(ctrl: u32) {
    cmd::path_begin(ctrl);
}

/// Add the segment named `name` to the path.
#[no_mangle]
pub extern "C" fn rt_path_seg(name: u32) {
    cmd::path_seg(name);
}

/// Add the text of operand `o` as an argument of the current segment.
#[no_mangle]
pub extern "C" fn rt_path_arg(base: u32, o: u32) {
    cmd::path_arg(base, o);
}

/// `SET path value`.
#[no_mangle]
pub extern "C" fn rt_cmd_set(base: u32, value: u32) {
    cmd::set(base, value);
}

/// `CALL path`.
#[no_mangle]
pub extern "C" fn rt_cmd_call() {
    cmd::call();
}

/// Query the path (`GET`, `CALLR`, `COUNT`); the answer text is left in
/// character temporary `k`.
#[no_mangle]
pub extern "C" fn rt_cmd_query(op: u32, k: u32, dynamic: u32) {
    cmd::query(op, k, dynamic);
}

/// `WAIT-EVENT`: remember where the next event's names are to be written.
#[no_mangle]
pub extern "C" fn rt_park(base: u32, slot: u32, a: u32, b: u32) {
    cmd::park(base, slot, a, b);
}

/// Deliver an event: write its control and event names to the parked items.
#[no_mangle]
pub extern "C" fn rt_deliver(base: u32, slot: u32, ctrl_text: u32, ctrl_len: u32, ev_text: u32, ev_len: u32) {
    cmd::deliver(base, slot, ctrl_text, ctrl_len, ev_text, ev_len);
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

// ── In-memory INDEXED files and SORT (WASM-010 R7, Q8; see `idx`) ──────────

/// Give the routines the file heap (`pr_init`, after `rt_init`).
#[no_mangle]
pub extern "C" fn rt_heap_init(base: u32, size: u32) {
    idx::heap_init(base, size);
}

/// Give back every heap block of instance `inst` (its form is being released).
#[no_mangle]
pub extern "C" fn rt_heap_release(inst: u32) {
    idx::release(inst);
}

/// Bytes of the file heap in use (`pr_dbg_region_used`).
#[no_mangle]
pub extern "C" fn rt_heap_used() -> u32 {
    idx::used()
}

/// `OPEN mode file` on an in-memory INDEXED file; the status.
#[no_mangle]
pub extern "C" fn rt_idx_open(base: u32, d: u32, mode: u32) -> u32 {
    idx::open(base, d, mode)
}

/// `CLOSE file` (`how`: 0 plain, 1 `WITH LOCK`, 2 `REEL`).
#[no_mangle]
pub extern "C" fn rt_idx_close(base: u32, d: u32, how: u32) -> u32 {
    idx::close(base, d, how)
}

/// `WRITE record` of `wlen` bytes.
#[no_mangle]
pub extern "C" fn rt_idx_write(base: u32, d: u32, wlen: u32) -> u32 {
    idx::write(base, d, wlen)
}

/// `READ file` (see [`idx::read`]).
#[no_mangle]
pub extern "C" fn rt_idx_read(base: u32, d: u32, how: u32, kor: u32, ks: u32) -> u32 {
    idx::read(base, d, how, kor, ks)
}

/// `REWRITE record`.
#[no_mangle]
pub extern "C" fn rt_idx_rewrite(base: u32, d: u32, random: u32) -> u32 {
    idx::rewrite(base, d, random)
}

/// `DELETE file`.
#[no_mangle]
pub extern "C" fn rt_idx_delete(base: u32, d: u32, random: u32) -> u32 {
    idx::delete(base, d, random)
}

/// `START file KEY rel …`.
#[no_mangle]
pub extern "C" fn rt_idx_start(base: u32, d: u32, rel: u32, kor: u32, ks: u32) -> u32 {
    idx::start(base, d, rel, kor, ks)
}

/// Start a `SORT`.
#[no_mangle]
pub extern "C" fn rt_sort_begin(base: u32, d: u32) {
    idx::sort_begin(base, d);
}

/// `RELEASE record`.
#[no_mangle]
pub extern "C" fn rt_sort_release(base: u32, d: u32) {
    idx::sort_release(base, d);
}

/// Order the released records.
#[no_mangle]
pub extern "C" fn rt_sort_sort(base: u32, d: u32) {
    idx::sort_sort(base, d);
}

/// `RETURN file`: 0, or 10 at the end.
#[no_mangle]
pub extern "C" fn rt_sort_return(base: u32, d: u32) -> u32 {
    idx::sort_return(base, d)
}

/// The end of a `SORT`.
#[no_mangle]
pub extern "C" fn rt_sort_end(base: u32, d: u32) {
    idx::sort_end(base, d);
}

// ── The group <-> JSON mapping (WASM-005 R5, R5b; see `json`) ──────────────

/// Write the JSON text of the group at `addr` (descriptor `desc`) at `out`,
/// at most `cap` bytes: the length, or a negated error code.
#[no_mangle]
pub extern "C" fn rt_json_encode(desc: u32, addr: u32, out: u32, cap: u32) -> i32 {
    json::encode(desc, addr, out, cap)
}

/// Read the JSON text at `text` (`len` bytes) into the group at `addr`: 0, or a
/// negated error code; the group is untouched on an error.
#[no_mangle]
pub extern "C" fn rt_json_decode(desc: u32, addr: u32, text: u32, len: u32) -> i32 {
    json::decode(desc, addr, text, len)
}

// ── CALL … WITH WEBSOCKET LINKAGE (WASM-005 R9; see `remote`) ──────────────

/// Build and send the request frame: the request id, or a negative code.
#[no_mangle]
pub extern "C" fn rt_remote_request(base: u32, name: u32, call: u32) -> i32 {
    remote::request(base, name, call)
}

/// Read the answer into the call's groups: 0, or why not (`remote::FAILED` …).
#[no_mangle]
pub extern "C" fn rt_remote_result(base: u32, call: u32) -> i32 {
    remote::result(base, call)
}
