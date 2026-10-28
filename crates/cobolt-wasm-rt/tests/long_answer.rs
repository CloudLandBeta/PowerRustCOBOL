// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A property read keeps the whole answer the host gives it, up to the 64 KiB
//! answer region, and stops with run-time error 111 (1111 on the page) and a
//! text that says why past it — it used to keep the first 4,096 bytes without a
//! word (PowerAnalyticsWeb P-F1: `CheckedItems` of more than ~37 names).

use cobolt_wasm_rt::host::{take_log, Event, ANSWER};
use cobolt_wasm_rt::state::{err, ANSWER_SIZE};
use cobolt_wasm_rt::testkit::Kit;
use cobolt_wasm_rt::{cmd, func, last_err_text};

fn read(answer: Vec<u8>) -> std::thread::Result<(u32, u32)> {
    ANSWER.with(|a| *a.borrow_mut() = answer);
    cmd::path_begin(3);
    cmd::path_seg(0);
    std::panic::catch_unwind(|| {
        cmd::query(cmd::Q_GET as u32, 1, 1);
        func::ctemp_loc(1)
    })
}

#[test]
fn a_long_answer_is_read_whole_or_refused_with_its_reason() {
    let _k = Kit::new();
    for n in [4097usize, 24_000, ANSWER_SIZE as usize] {
        let text: Vec<u8> = (0..n).map(|i| b'A' + (i % 26) as u8).collect();
        let (at, len) = read(text.clone()).expect("an answer that fits");
        assert_eq!(len as usize, n);
        assert_eq!(cobolt_wasm_rt::mem::bytes(at, len), &text[..], "{n} bytes arrive as sent");
    }
    take_log();
    assert!(read(vec![b'x'; 70_000]).is_err(), "an answer over 64 KiB stops the statement");
    assert!(take_log().contains(&Event::Abort(err::TEXT_TOO_LARGE)));
    assert_eq!(
        String::from_utf8(last_err_text()).unwrap(),
        "a property or method answered 70000 bytes; a statement can read at most 65536 bytes of one answer"
    );
}
