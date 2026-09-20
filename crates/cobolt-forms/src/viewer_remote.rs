// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A Viewer `Source` that is a **web address**.
//!
//! The Viewer's document pipeline reads a local path and nothing else, which is
//! the right shape: indexing a 2 GB log by byte range, seeking to a PDF's
//! cross-reference table and decoding one page at a time all want a file, not a
//! socket. So a URL is not a third kind of document — it is **fetched to a
//! local file once and then opened exactly like any other path**. Every format,
//! every layout, every page-at-a-time read works on a downloaded document
//! without knowing it came from the network.
//!
//! The fetch is asynchronous for the same reason the file dialog is: a
//! synchronous request on the UI thread freezes the IDE for as long as the
//! server takes, and a server can take forever. [`resolve`] therefore answers
//! immediately with whatever it has — the cached file, or "still fetching", or
//! why it failed — and the caller repaints until it settles.
//!
//! **Not a browser.** What comes back is bytes, put through the Viewer's own
//! `detect_format`: a Markdown file renders as Markdown, a PDF as a PDF, an
//! HTML page as the HTML *subset* the control supports. Nothing is executed,
//! no script runs, no sub-resource is followed, and a redirect chain is
//! followed by the HTTP client only.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Is this `Source` a web address rather than a path?
///
/// Deliberately narrow — only the two schemes a document can actually be
/// fetched over. A Windows path like `C:\docs\a.pdf` has a colon in it and a
/// looser test would call it a URL.
pub fn is_url(source: &str) -> bool {
    let s = source.trim();
    let lower = s.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Where a fetched document lands.
///
/// One directory per process-user under the OS temp dir, one file per URL named
/// by a hash of that URL plus whatever extension the URL's own path carried —
/// the extension matters, because `detect_format` falls back to it when the
/// content sniff is inconclusive.
fn cache_dir() -> PathBuf {
    std::env::temp_dir().join("powerrustcobol-viewer-web")
}

fn cache_file(url: &str) -> PathBuf {
    // FNV-1a: a stable, dependency-free name for a URL. This is a cache key,
    // not a security boundary — nothing is decided by it but a filename.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in url.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let ext = url
        .rsplit('/')
        .next()
        .and_then(|name| name.split(['?', '#']).next())
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, e)| e.to_ascii_lowercase())
        .filter(|e| !e.is_empty() && e.len() <= 8 && e.chars().all(|c| c.is_ascii_alphanumeric()));
    let stem = format!("{hash:016x}");
    match ext {
        Some(e) => cache_dir().join(format!("{stem}.{e}")),
        None => cache_dir().join(stem),
    }
}

/// What [`resolve`] can say about a URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteState {
    /// Downloaded, and here is the local file to open.
    Ready(PathBuf),
    /// A fetch is in flight. Repaint; ask again next frame.
    Fetching,
    /// The fetch failed, and this is what to show the developer. Held so the
    /// same URL is not retried every frame forever — [`forget`] clears it.
    Failed(String),
}

/// How long a failure is remembered before the URL is tried again.
///
/// Not forever: the commonest failure is a typo the developer corrects a second
/// later, and a URL that stays failed for the life of the process makes the
/// control look broken after it has been fixed. Not zero either: a genuinely
/// dead address must not be re-fetched sixty times a second.
const FAILURE_COOLDOWN: Duration = Duration::from_secs(10);

#[derive(Default)]
struct Registry {
    states: HashMap<String, RemoteState>,
    /// When each remembered failure was recorded.
    failed_at: HashMap<String, Instant>,
}

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(Registry::default()))
}

/// The local file for `url`, fetching it in the background if this is the first
/// time it has been asked for.
///
/// Never blocks. A cached file that already exists on disk is adopted without a
/// request at all, so reopening a form does not re-download every document in
/// it.
pub fn resolve(url: &str) -> RemoteState {
    let url = url.trim().to_owned();
    let mut reg = registry().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(state) = reg.states.get(&url) {
        let expired = matches!(state, RemoteState::Failed(_))
            && reg
                .failed_at
                .get(&url)
                .map(|t| t.elapsed() >= FAILURE_COOLDOWN)
                .unwrap_or(true);
        if !expired {
            return state.clone();
        }
        reg.states.remove(&url);
        reg.failed_at.remove(&url);
    }

    // Already on disk from an earlier run — adopt it rather than re-fetch.
    let dest = cache_file(&url);
    if dest.is_file() {
        let state = RemoteState::Ready(dest);
        reg.states.insert(url, state.clone());
        return state;
    }

    reg.states.insert(url.clone(), RemoteState::Fetching);
    drop(reg);

    std::thread::spawn(move || {
        let outcome = fetch(&url, &dest);
        record(&url, dest, outcome);
    });
    RemoteState::Fetching
}

fn record(url: &str, dest: PathBuf, outcome: Result<(), String>) -> RemoteState {
    let state = match outcome {
        Ok(()) => RemoteState::Ready(dest),
        Err(e) => RemoteState::Failed(e),
    };
    let mut reg = registry().lock().unwrap_or_else(|e| e.into_inner());
    if matches!(state, RemoteState::Failed(_)) {
        reg.failed_at.insert(url.to_owned(), Instant::now());
    }
    reg.states.insert(url.to_owned(), state.clone());
    state
}

/// What a Viewer `Source` points at on this machine.
///
/// The ONE funnel every Viewer read goes through, so "a URL is just a path
/// once it has arrived" is true by construction rather than by four call sites
/// agreeing. A source that is not a URL resolves as it always has — through
/// [`crate::assets::resolve`], project-relative against the open project —
/// and is returned even when the file does not exist, because whether it opens
/// is the reader's answer to give, not this one's.
#[derive(Clone, Debug)]
pub enum Local {
    /// Read this file.
    Path(PathBuf),
    /// A download is in flight. Repaint and ask again.
    Fetching,
    /// The download failed, and this is why.
    Failed(String),
}

/// Non-blocking: for a paint, which may not wait for a socket.
pub fn local_path(source: &str) -> Local {
    if !is_url(source) {
        return Local::Path(crate::assets::resolve(source));
    }
    match resolve(source) {
        RemoteState::Ready(p) => Local::Path(p),
        RemoteState::Fetching => Local::Fetching,
        RemoteState::Failed(e) => Local::Failed(e),
    }
}

/// Blocking: for a worker thread that is already off the UI thread and whose
/// whole job is to wait for slow I/O.
///
/// The document host opens documents on such a thread, and there a URL should
/// behave exactly like a slow disk — no `Fetching` state to poll, no extra
/// round trip through the paint. It shares the same cache as [`local_path`],
/// so whichever asks first pays and the other is served from disk.
pub fn local_path_blocking(source: &str) -> Result<PathBuf, String> {
    if !is_url(source) {
        return Ok(crate::assets::resolve(source));
    }
    let url = source.trim().to_owned();
    if let RemoteState::Ready(p) = resolve_cached_only(&url) {
        return Ok(p);
    }
    let dest = cache_file(&url);
    match record(&url, dest.clone(), fetch(&url, &dest)) {
        RemoteState::Ready(p) => Ok(p),
        RemoteState::Failed(e) => Err(e),
        RemoteState::Fetching => Err("download did not complete".into()),
    }
}

/// The cached answer for `url`, without starting a fetch.
fn resolve_cached_only(url: &str) -> RemoteState {
    let reg = registry().lock().unwrap_or_else(|e| e.into_inner());
    match reg.states.get(url) {
        Some(RemoteState::Ready(p)) if p.is_file() => RemoteState::Ready(p.clone()),
        _ => {
            let dest = cache_file(url);
            if dest.is_file() {
                RemoteState::Ready(dest)
            } else {
                RemoteState::Fetching
            }
        }
    }
}

/// Drop what is remembered about `url`, so the next [`resolve`] fetches again.
///
/// The IDE calls this when the developer edits the Source, so a typo that
/// 404'd is retried the moment it is corrected rather than staying failed for
/// the life of the process.
pub fn forget(url: &str) {
    let mut reg = registry().lock().unwrap_or_else(|e| e.into_inner());
    reg.states.remove(url.trim());
    reg.failed_at.remove(url.trim());
}

/// The most a Viewer will download. A document is a document; a multi-gigabyte
/// response to a mistyped URL is a hung IDE and a full disk.
const MAX_BYTES: u64 = 256 * 1024 * 1024;

fn fetch(url: &str, dest: &std::path::Path) -> Result<(), String> {
    use std::io::Read;

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let resp = crate::map_tiles::agent()
        .get(url)
        .call()
        .map_err(|e| e.to_string())?;

    let mut body = resp.into_reader().take(MAX_BYTES + 1);
    let mut bytes = Vec::new();
    body.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(format!(
            "document larger than the {} MB a Viewer will download",
            MAX_BYTES / (1024 * 1024)
        ));
    }
    // Written beside the destination and renamed, so a fetch interrupted
    // half-way never leaves a truncated file that the next run would adopt as
    // a complete download.
    let tmp = dest.with_extension("part");
    std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dest).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A path is a path. The narrow scheme test is the whole point: a Windows
    /// drive letter is not a URL, and neither is a file with a colon in its
    /// name.
    #[test]
    fn only_http_and_https_count_as_a_web_address() {
        let urls = ["http://example.com/a.md", "HTTPS://Example.com/b.pdf", "  https://x/y  "];
        let paths = [
            "C:\\docs\\a.pdf",
            "/Users/me/a.md",
            "assets/docs/sample.txt",
            "ftp://example.com/a.txt",
            "mailto:someone@example.com",
            "",
        ];
        println!("  {:<40} url?", "source");
        for u in urls {
            println!("  {u:<40} {}", is_url(u));
            assert!(is_url(u), "{u} is a web address");
        }
        for p in paths {
            println!("  {p:<40} {}", is_url(p));
            assert!(!is_url(p), "{p} is NOT a web address");
        }
    }

    /// Two URLs must not collide, the same URL must be stable across runs, and
    /// the extension must survive — `detect_format` falls back to it.
    #[test]
    fn a_urls_cache_file_is_stable_distinct_and_keeps_its_extension() {
        let a = cache_file("https://example.com/guide.md");
        let b = cache_file("https://example.com/guide.pdf");
        let a_again = cache_file("https://example.com/guide.md");
        let query = cache_file("https://example.com/report.pdf?v=3#top");
        let bare = cache_file("https://example.com/download");
        println!("  guide.md   → {}", a.file_name().unwrap().to_string_lossy());
        println!("  guide.pdf  → {}", b.file_name().unwrap().to_string_lossy());
        println!("  ?v=3#top   → {}", query.file_name().unwrap().to_string_lossy());
        println!("  no ext     → {}", bare.file_name().unwrap().to_string_lossy());
        assert_eq!(a, a_again, "the same URL must map to the same file every time");
        assert_ne!(a, b, "different URLs must not share a cache file");
        assert_eq!(a.extension().and_then(|e| e.to_str()), Some("md"));
        assert_eq!(b.extension().and_then(|e| e.to_str()), Some("pdf"));
        assert_eq!(
            query.extension().and_then(|e| e.to_str()),
            Some("pdf"),
            "a query string and a fragment are not part of the extension"
        );
        assert_eq!(bare.extension(), None, "no extension in the URL, none invented");
    }
}
