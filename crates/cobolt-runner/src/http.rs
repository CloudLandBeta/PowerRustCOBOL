// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The HTTP built-ins' clients (DESKTOP-WASM-001 T42, ABI A45): `COBOL::"HTTP-GET"` and its six
//! companions. Each form instance keeps its own client, as each form's interpreter keeps its own, so
//! the headers `HTTP-SET-HEADER` sets belong to that window, shared by its handlers and the procedures
//! they call; a console program has one (`None`). The interpreter's own client runs each call
//! (`cobolt_runtime::HttpClient`), so the request, the headers sent, the redirects, the bounds of a
//! download and the text answered when no answer came back are the same on both paths.
//!
//! A call blocks the program's thread while it waits, as it blocks the form's interpreter; the other
//! windows' events wait with it, where each form's interpreter would go on (WASM-PARITY row 76).

use std::collections::HashMap;
use std::path::Path;

use cobolt_runtime::HttpClient;
use cobolt_wasm_abi::http;

#[derive(Default)]
pub(crate) struct Clients {
    /// The form instance the current call acts on (`None`: a console program, or a form still starting).
    calling: Option<u32>,
    /// A client was made during the current call.
    made_in_call: bool,
    clients: HashMap<Option<u32>, HttpClient>,
}

impl Clients {
    /// A call into the module begins; it acts on form instance `inst` when it names one.
    pub(crate) fn begin_call(&mut self, inst: Option<u32>) {
        self.calling = inst;
        self.made_in_call = false;
    }

    /// The form instance an `OpenForm` call made is `inst`: the client its program made while it
    /// started, before the instance was known, is its own.
    pub(crate) fn opened_form(&mut self, inst: u32) {
        if std::mem::take(&mut self.made_in_call) {
            if let Some(c) = self.clients.remove(&None) {
                self.clients.insert(Some(inst), c);
            }
        }
    }

    /// Form instance `inst`'s window closed: its client goes, as its interpreter's does.
    pub(crate) fn forget_instance(&mut self, inst: u32) {
        self.clients.remove(&Some(inst));
    }

    /// A copy of form instance `inst`'s client, for a request sent outside a call into the module (an
    /// AgentObject's next round, T47c); a window that never made one has a client as new.
    pub(crate) fn of(&self, inst: u32) -> HttpClient {
        self.clients.get(&Some(inst)).cloned().unwrap_or_default()
    }

    /// The run ends.
    pub(crate) fn end(&mut self) {
        self.clients.clear();
    }

    /// The client of the window the current call acts on.
    pub(crate) fn client(&mut self) -> &mut HttpClient {
        let key = self.calling;
        if !self.clients.contains_key(&key) {
            self.made_in_call = true;
        }
        self.clients.entry(key).or_default()
    }

    /// Operation `op` (`cobolt_wasm_abi::http`, `TAIL` ignored) with the texts `a` and `b`, as the
    /// interpreter's arms run it: the answer's number and text; `None` for an operation it does not know.
    pub(crate) fn call(&mut self, op: u32, a: &[u8], b: &[u8]) -> Option<(u16, String)> {
        let (a, b) = (String::from_utf8_lossy(a), String::from_utf8_lossy(b));
        let (a, b) = (a.trim(), b.trim());
        let c = self.client();
        let (text, number) = match op & !http::TAIL {
            http::GET => c.get(a),
            http::POST => c.post(a, b),
            http::PUT => c.put(a, b),
            http::DELETE => c.delete(a),
            http::SET_HEADER => {
                c.set_header(a, b);
                (String::new(), 0)
            }
            http::CLEAR_HEADERS => {
                c.clear_headers();
                (String::new(), 0)
            }
            http::DOWNLOAD => {
                let got = c.download(a, Path::new(&cobolt_runtime::interpreter::anchor_relative_path(b)));
                (got.status.to_owned(), got.http_status)
            }
            _ => return None,
        };
        Some((number, text))
    }
}

/// The bytes of `text` the module asks for in `cap` bytes: its first, or with `TAIL` the last of it
/// without its trailing spaces; and the whole length the answer reports.
pub(crate) fn part(op: u32, text: &str, cap: usize) -> (&[u8], usize) {
    let mut bytes = text.as_bytes();
    let tail = op & http::TAIL != 0;
    if tail {
        bytes = &bytes[..bytes.iter().rposition(|&c| c != b' ').map_or(0, |e| e + 1)];
    }
    let n = bytes.len().min(cap);
    let shown = if tail { &bytes[bytes.len() - n..] } else { &bytes[..n] };
    (shown, bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The part written: the first bytes, or with `TAIL` the last ones without the trailing spaces.
    #[test]
    fn the_part_written_is_the_head_or_the_trimmed_tail() {
        let cases: [(u32, &str, usize, &str, usize); 6] = [
            (http::GET, "abcdef", 3, "abc", 6),
            (http::GET, "ab  ", 10, "ab  ", 4),
            (http::GET | http::TAIL, "abcdef", 3, "def", 6),
            (http::GET | http::TAIL, "abcdef   ", 4, "cdef", 6),
            (http::GET | http::TAIL, "ab", 5, "ab", 2),
            (http::GET | http::TAIL, "   ", 5, "", 0),
        ];
        for (op, text, cap, shown, len) in cases {
            let (p, l) = part(op, text, cap);
            assert_eq!((p, l), (shown.as_bytes(), len), "{op:#x} {text:?} {cap}");
        }
        println!("http parts: {} of {} cases", cases.len(), cases.len());
    }

    /// Each window keeps its own client; one made while a form starts becomes that form's.
    #[test]
    fn each_window_keeps_its_own_client() {
        let mut c = Clients::default();
        c.begin_call(None);
        c.call(http::SET_HEADER, b"X-A", b"1").unwrap();
        c.opened_form(7);
        c.begin_call(Some(8));
        c.call(http::SET_HEADER, b"X-B", b"2").unwrap();
        assert_eq!(c.clients.len(), 2);
        assert!(c.clients.contains_key(&Some(7)) && c.clients.contains_key(&Some(8)));
        c.forget_instance(8);
        assert_eq!(c.clients.len(), 1);
        assert_eq!(c.call(99, b"", b""), None, "an operation it does not know");
        c.end();
        assert!(c.clients.is_empty());
        println!("http clients: 5 of 5 checks");
    }
}
