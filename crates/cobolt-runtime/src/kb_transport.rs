// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The network the application Knowledge Base uses (spec 068 R24, R23): the
//! runtime's one HTTP agent, with its TLS already set up, behind
//! `cobolt_kb::embed::Transport`. Without the `http` feature every request
//! fails with a message, and the Knowledge Base searches lexically.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use cobolt_kb::embed::Transport;

/// The runtime's HTTP, for the Knowledge Base.
#[derive(Debug, Default, Clone, Copy)]
pub struct RuntimeTransport;

#[cfg(feature = "http")]
impl Transport for RuntimeTransport {
    fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
        timeout_ms: u64,
    ) -> Result<(u16, String), String> {
        let mut req = crate::http_runtime::agent(timeout_ms).post(url);
        for (k, v) in headers {
            req = req.set(k, v);
        }
        match req.send_string(body) {
            Ok(resp) => {
                let status = resp.status();
                resp.into_string().map(|b| (status, b)).map_err(|e| e.to_string())
            }
            // A 4xx or 5xx is an answer, not a failure to reach the server.
            Err(ureq::Error::Status(status, resp)) => Ok((status, resp.into_string().unwrap_or_default())),
            Err(e) => Err(e.to_string()),
        }
    }

    fn get_to_file(
        &self,
        url: &str,
        dest: &Path,
        progress: &mut dyn FnMut(u64, Option<u64>),
        cancel: &AtomicBool,
    ) -> Result<(), String> {
        use std::io::{Read, Write};
        let resp = crate::http_runtime::agent(0).get(url).call().map_err(|e| e.to_string())?;
        let total = resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok());
        let mut reader = resp.into_reader();
        let mut file = std::fs::File::create(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        let mut buf = vec![0u8; 256 * 1024];
        let mut got = 0u64;
        loop {
            if cancel.load(Ordering::Relaxed) {
                drop(file);
                let _ = std::fs::remove_file(dest);
                return Err("cancelled".into());
            }
            let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            got += n as u64;
            progress(got, total);
        }
        file.sync_all().map_err(|e| e.to_string())
    }
}

#[cfg(not(feature = "http"))]
impl Transport for RuntimeTransport {
    fn post_json(&self, _: &str, _: &[(String, String)], _: &str, _: u64) -> Result<(u16, String), String> {
        Err("the HTTP bridge is not linked into this program".into())
    }

    fn get_to_file(
        &self,
        _: &str,
        _: &Path,
        _: &mut dyn FnMut(u64, Option<u64>),
        _: &AtomicBool,
    ) -> Result<(), String> {
        Err("the HTTP bridge is not linked into this program".into())
    }
}
