// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Backend entry** (ABI A59, spec 114): the `backend` section of the manifest and the frames
//! of `pr_backend_call`.
//!
//! A Backend module holds every program a Backend exports. The host calls
//! `pr_backend_call(export) -> i32` with the **request frame** in memory 0 at [`BackendSection::request`]
//! and reads the **response frame** at [`BackendSection::response`]; both frames are little-endian words
//! and length-prefixed UTF-8 texts:
//!
//! ```text
//! request    count, then count × (len, len bytes of JSON)   one text per USING parameter
//! response   0 answered   count, count × (len, JSON), has_returning, [len, JSON]
//!            1 refused    code, path_len, path        (nothing ran)
//!            4 unwritable code, path_len, path        (the program ran; a record it holds is not JSON-able)
//! ```
//!
//! The section is **not a field of [`crate::Manifest`]**: a module without it has the manifest it always
//! had, byte for byte, and the existing readers (which ignore a key they do not know) still read a Backend
//! module's manifest. A Backend module's own readers use [`BackendSection::from_manifest`].

use serde::{Deserialize, Serialize};

/// The key the section has in the manifest.
pub const SECTION: &str = "backend";

/// The state mode A59 defines: every call starts from the program's initial image.
pub const STATE_PER_CALL: &str = "per-call";

/// The state mode A61 adds: a connection's calls to one exported program run on one instance of the module, which is
/// entered with [`CALL_EXPORT`] the first time and with [`RESUME_EXPORT`] after, so the program's storage, its open files
/// and its database connection are what the next call finds. A module of this mode exports both entries.
pub const STATE_PER_CONNECTION: &str = "per-connection";

/// The entry of a call that starts the program from its initial image (A59): the module's start-up, the program's image and
/// its parameters' initial state, then the request.
pub const CALL_EXPORT: &str = "pr_backend_call";

/// The entry of a call that finds the program as the last call left it (A61, state [`STATE_PER_CONNECTION`] only): the
/// request, as for [`CALL_EXPORT`], but no start-up, no image and no initial state of the parameters.
pub const RESUME_EXPORT: &str = "pr_backend_resume";

/// The result of `pr_backend_call` (ABI A59).
pub mod result {
    /// Answered: the response frame holds the parameters and the `RETURNING` record.
    pub const ANSWERED: i32 = 0;
    /// The request was refused before the program ran: the response frame holds the code and the path.
    pub const REFUSED: i32 = 1;
    /// The answer does not fit the response area.
    pub const TOO_LARGE: i32 = 2;
    /// The request frame is malformed, or the export is not in the manifest.
    pub const BAD_REQUEST: i32 = 3;
    /// The program ran, but a record it holds cannot be written as JSON (an `OCCURS DEPENDING ON` count out of
    /// its range, bytes that are not UTF-8, a number its picture cannot hold): the response frame is the code
    /// and the path of the member, as for [`REFUSED`]. The interpreter answers such a call with a validation
    /// error that names the same member.
    pub const UNWRITABLE: i32 = 4;
    // A run-time error is not a result: it traps (ABI §9 `-10`), the `ERROR` command of §5 having been flushed first.
}

/// A region of memory 0 the host and the module pass a frame through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Area {
    /// Offset in memory 0.
    pub at: u32,
    /// Bytes the frame may take.
    pub cap: u32,
}

/// One exported program, as `pr_backend_call` numbers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendExport {
    /// The argument of `pr_backend_call`.
    pub index: u32,
    /// The program's upper-case `PROGRAM-ID`.
    pub name: String,
    /// How many `USING` parameters it has.
    pub params: u32,
    /// Whether it has a `RETURNING` record.
    pub returning: bool,
}

/// The manifest's `backend` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendSection {
    pub request: Area,
    pub response: Area,
    /// [`STATE_PER_CALL`] or [`STATE_PER_CONNECTION`].
    pub state: String,
    pub exports: Vec<BackendExport>,
}

impl BackendSection {
    /// The export called `name` (upper case).
    pub fn export(&self, name: &str) -> Option<&BackendExport> {
        self.exports.iter().find(|e| e.name == name)
    }

    /// What is wrong with the section, in words for the person who built the Backend; `Ok` when it is usable.
    pub fn check(&self) -> Result<(), String> {
        if self.state != STATE_PER_CALL && self.state != STATE_PER_CONNECTION {
            return Err(format!("the Backend section names the state mode `{}`, which ABI A59 and A61 do not define", self.state));
        }
        if self.request.cap == 0 || self.response.cap == 0 {
            return Err("the Backend section has an empty request or response area".into());
        }
        if self.exports.is_empty() {
            return Err("the Backend section lists no exported program".into());
        }
        for (i, e) in self.exports.iter().enumerate() {
            if e.index as usize != i {
                return Err(format!("the Backend section numbers {} as {} (expected {i})", e.name, e.index));
            }
            if self.exports[..i].iter().any(|o| o.name == e.name) {
                return Err(format!("the Backend section lists {} twice", e.name));
            }
        }
        Ok(())
    }

    /// The section as the JSON value the manifest carries under [`SECTION`].
    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("a plain struct serialises")
    }

    /// The Backend section of a manifest's text: `Ok(None)` for a module that has none (every module but a
    /// Backend's), an error for one that is present but unreadable or unusable.
    pub fn from_manifest(text: &str) -> Result<Option<BackendSection>, String> {
        let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("the manifest is not valid JSON: {e}"))?;
        let Some(section) = v.get(SECTION) else {
            return Ok(None);
        };
        let s: BackendSection = serde_json::from_value(section.clone()).map_err(|e| format!("the manifest's `{SECTION}` section is unreadable: {e}"))?;
        s.check()?;
        Ok(Some(s))
    }
}

/// The request frame for `texts`, as the host writes it at the request area.
pub fn encode_request(texts: &[&str]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + texts.iter().map(|t| 4 + t.len()).sum::<usize>());
    out.extend_from_slice(&(texts.len() as u32).to_le_bytes());
    for t in texts {
        out.extend_from_slice(&(t.len() as u32).to_le_bytes());
        out.extend_from_slice(t.as_bytes());
    }
    out
}

/// What an answered response frame holds.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Answer {
    /// Every `USING` parameter, as the program left it.
    pub args: Vec<String>,
    /// The `RETURNING` record, when the program has one.
    pub returning: Option<String>,
}

/// What a refused request's response frame holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The rt's group⇄JSON error code (`E_*`, ABI A59).
    pub code: i32,
    /// The member the code names: `ORDER.LINES(2).QTY`.
    pub path: String,
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn word(&mut self) -> Result<u32, String> {
        let w = self.b.get(self.at..self.at + 4).ok_or("the frame ends inside a word")?;
        self.at += 4;
        Ok(u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
    }
    fn text(&mut self) -> Result<String, String> {
        let n = self.word()? as usize;
        let s = self.b.get(self.at..self.at.checked_add(n).ok_or("a length overflows")?).ok_or("the frame ends inside a text")?;
        self.at += n;
        String::from_utf8(s.to_vec()).map_err(|_| "a text of the frame is not UTF-8".to_owned())
    }
    /// A text that may have been cut in the middle of a character (the runtime keeps a path to 1,024 bytes).
    fn text_lossy(&mut self) -> Result<String, String> {
        let n = self.word()? as usize;
        let s = self.b.get(self.at..self.at.checked_add(n).ok_or("a length overflows")?).ok_or("the frame ends inside a text")?;
        self.at += n;
        Ok(String::from_utf8_lossy(s).into_owned())
    }
}

/// Read an answered response frame.
pub fn decode_answer(frame: &[u8]) -> Result<Answer, String> {
    let mut r = Reader { b: frame, at: 0 };
    let n = r.word()?;
    let mut args = Vec::new();
    for _ in 0..n {
        args.push(r.text()?);
    }
    let returning = match r.word()? {
        0 => None,
        1 => Some(r.text()?),
        other => return Err(format!("`has_returning` is {other}, not 0 or 1")),
    };
    Ok(Answer { args, returning })
}

/// Read a refused request's response frame.
pub fn decode_refusal(frame: &[u8]) -> Result<Refusal, String> {
    let mut r = Reader { b: frame, at: 0 };
    let code = r.word()? as i32;
    // The runtime keeps a member's path to 1,024 bytes, which can end inside a character: the path is read as
    // it is, with the broken character replaced, rather than the whole refusal being lost.
    let path = r.text_lossy()?;
    Ok(Refusal { code, path })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section() -> BackendSection {
        BackendSection {
            request: Area { at: 0x1000, cap: 4096 },
            response: Area { at: 0x2000, cap: 4096 },
            state: STATE_PER_CALL.into(),
            exports: vec![
                BackendExport { index: 0, name: "ORDER-LIST".into(), params: 2, returning: false },
                BackendExport { index: 1, name: "ORDER-TOTAL".into(), params: 1, returning: true },
            ],
        }
    }

    #[test]
    fn a_manifest_without_the_section_has_none_and_one_with_it_reads_back() {
        assert_eq!(BackendSection::from_manifest(r#"{"abi":1}"#).unwrap(), None, "no key, no section");
        let text = serde_json::json!({ "abi": 1, "backend": section().to_value() }).to_string();
        assert_eq!(BackendSection::from_manifest(&text).unwrap(), Some(section()));
        println!("── backend section: absent reads as none, present round-trips ({} exports) ──", section().exports.len());
    }

    #[test]
    fn a_section_that_names_something_a59_does_not_define_is_refused_in_words() {
        let mut s = section();
        s.state = STATE_PER_CONNECTION.into();
        assert!(s.check().is_ok(), "A61 defines the per-connection mode");
        s.state = "per-session".into();
        assert!(s.check().unwrap_err().contains("per-session"));
        let mut s = section();
        s.exports[1].index = 7;
        assert!(s.check().unwrap_err().contains("ORDER-TOTAL"));
        let mut s = section();
        s.exports[1].name = "ORDER-LIST".into();
        assert!(s.check().unwrap_err().contains("twice"));
        let mut s = section();
        s.response.cap = 0;
        assert!(s.check().is_err());
        let mut s = section();
        s.exports.clear();
        assert!(s.check().is_err());
        let text = serde_json::json!({ "backend": { "request": 3 } }).to_string();
        assert!(BackendSection::from_manifest(&text).unwrap_err().contains("unreadable"));
    }

    #[test]
    fn the_frames_round_trip_and_a_short_one_is_refused() {
        let req = encode_request(&["{\"A\":1}", "", "é"]);
        assert_eq!(&req[..4], &3u32.to_le_bytes());
        // An answer written by hand, as the module writes it.
        let mut resp = Vec::new();
        resp.extend_from_slice(&2u32.to_le_bytes());
        for t in ["{\"X\":\"1\"}", "{}"] {
            resp.extend_from_slice(&(t.len() as u32).to_le_bytes());
            resp.extend_from_slice(t.as_bytes());
        }
        resp.extend_from_slice(&1u32.to_le_bytes());
        resp.extend_from_slice(&2u32.to_le_bytes());
        resp.extend_from_slice(b"{}");
        let a = decode_answer(&resp).unwrap();
        assert_eq!(a.args, vec!["{\"X\":\"1\"}".to_string(), "{}".to_string()]);
        assert_eq!(a.returning.as_deref(), Some("{}"));
        assert!(decode_answer(&resp[..resp.len() - 1]).is_err(), "a frame cut short is refused");
        let mut refusal = Vec::new();
        refusal.extend_from_slice(&6u32.to_le_bytes());
        refusal.extend_from_slice(&9u32.to_le_bytes());
        refusal.extend_from_slice(b"REC.NAME1");
        assert_eq!(decode_refusal(&refusal).unwrap(), Refusal { code: 6, path: "REC.NAME1".into() });
    }
}
