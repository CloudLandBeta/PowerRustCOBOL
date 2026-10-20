// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Files in and out of the backend (WASM-014 R15).
//!
//! **The backend names every file; the browser names none.** An upload picks one
//! of the *slots* the deployment declares (`[files] slots`), each with its own
//! mime allow-list and roles; the browser's file name is kept only as a display
//! name, after being refused if it could mean a path. The stored file is
//! `<files.dir>/files/<random 128-bit id>`, written first as
//! `<files.dir>/incoming/<upload id>.part`, verified (size and SHA-256) and only
//! then moved into place.
//!
//! An upload that breaks off resumes: `put-begin` with the same `resume` id
//! answers how many bytes the backend already holds, and the next chunk starts
//! there. A chunk that does not start exactly there is refused.
//!
//! A download address is `/dl/<random 256-bit token>`: tied to the subject and
//! the source address that asked for it, short-lived, and answered with one and
//! the same `404` whether it never existed, expired or is someone else's.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use sha2::{Digest, Sha256};

use cobolt_wire::error::{codes, CallError, ErrorClass};
use cobolt_wire::proto::PutBegin;

use crate::config::{FilesResolved, UploadSlot};

/// How long a file a program published stays offerable.
const PUBLISH_TTL: Duration = Duration::from_secs(900);

struct Upload {
    subject: String,
    slot: String,
    name: String,
    size: u64,
    mime: String,
    sha256: String,
    part: PathBuf,
    have: u64,
    /// Last time a piece arrived: an upload nobody resumes within [`UPLOAD_IDLE`] is dropped.
    touched: Instant,
}

/// How long an unfinished upload is kept for a resume.
const UPLOAD_IDLE: Duration = Duration::from_secs(3600);

struct Offer {
    path: PathBuf,
    subject: String,
    ip: IpAddr,
    expires: Instant,
    pub size: u64,
    name: String,
}

struct Published {
    path: PathBuf,
    subject: String,
    expires: Instant,
    size: u64,
    sha256: String,
}

/// What a download address resolves to.
pub struct Download {
    pub path: PathBuf,
    pub size: u64,
    pub name: String,
}

/// What `offer` answers.
pub struct Offered {
    pub token: String,
    pub expires_in: u64,
    pub size: u64,
    pub sha256: String,
}

pub struct FileStore {
    dir: PathBuf,
    slots: Vec<UploadSlot>,
    max_upload: u64,
    download_ttl: Duration,
    uploads: Mutex<HashMap<String, Upload>>,
    offers: Mutex<HashMap<String, Offer>>,
    published: Mutex<HashMap<String, Published>>,
}

fn random_hex(bytes: usize) -> String {
    let mut b = vec![0u8; bytes];
    let _ = getrandom::fill(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn bad(message: impl Into<String>) -> CallError {
    CallError::new(ErrorClass::Validation, codes::INVALID_PAYLOAD, message)
}

fn missing(message: impl Into<String>) -> CallError {
    CallError::new(ErrorClass::NotFound, codes::UNKNOWN_PROGRAM, message)
}

fn internal(e: impl std::fmt::Display) -> CallError {
    tracing::error!(target: "files", "{e}");
    CallError::new(ErrorClass::Server, codes::INTERNAL, "the backend could not store the file")
}

/// An id the backend issued: exactly 32 lowercase hex characters, nothing else.
pub fn is_file_id(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A browser's file name, refused if it could mean a path.
pub fn display_name(name: &str) -> Result<String, CallError> {
    let n = name.trim();
    if n.is_empty() || n.len() > 255 {
        return Err(bad("the file name is empty or longer than 255 bytes"));
    }
    if n == "." || n.contains("..") || n.contains('/') || n.contains('\\') || n.chars().any(|c| c.is_control()) || n.contains(':') {
        return Err(bad("the file name must be a plain name: no path, no '..', no control characters"));
    }
    Ok(n.to_string())
}

/// Does the content start the way the declared type says it must? Only the
/// types whose first bytes are fixed are checked; the others pass on the
/// declared type alone.
pub fn sniff(mime: &str, head: &[u8]) -> bool {
    match mime {
        "image/png" => head.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]),
        "image/jpeg" => head.starts_with(&[0xff, 0xd8, 0xff]),
        "image/gif" => head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a"),
        "application/pdf" => head.starts_with(b"%PDF-"),
        "application/zip" => head.starts_with(b"PK\x03\x04") || head.starts_with(b"PK\x05\x06"),
        _ => true,
    }
}

fn mime_allowed(allowed: &[String], mime: &str) -> bool {
    allowed.iter().any(|a| a == mime || (a.ends_with("/*") && mime.starts_with(&a[..a.len() - 1])))
}

fn sha256_of(path: &Path) -> std::io::Result<(u64, String)> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut n_total = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        n_total += n as u64;
    }
    Ok((n_total, h.finalize().iter().map(|b| format!("{b:02x}")).collect()))
}

impl FileStore {
    pub fn new(cfg: &FilesResolved) -> Self {
        FileStore {
            dir: cfg.dir.clone(),
            slots: cfg.slots.clone(),
            max_upload: cfg.max_upload_bytes,
            download_ttl: Duration::from_secs(cfg.download_ttl),
            uploads: Mutex::new(HashMap::new()),
            offers: Mutex::new(HashMap::new()),
            published: Mutex::new(HashMap::new()),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn uploads_in_progress(&self) -> usize {
        self.uploads.lock().map(|u| u.len()).unwrap_or(0)
    }

    fn files_dir(&self) -> PathBuf {
        self.dir.join("files")
    }

    fn incoming_dir(&self) -> PathBuf {
        self.dir.join("incoming")
    }

    /// `put-begin`: start an upload, or resume one. Returns `(upload id, bytes held)`.
    pub fn begin(&self, subject: &str, roles: &std::collections::BTreeSet<String>, req: &PutBegin) -> Result<(String, u64), CallError> {
        let slot = self.slots.iter().find(|s| s.name == req.slot).ok_or_else(|| missing(format!("there is no upload slot called \"{}\"", req.slot)))?;
        if !slot.roles.is_empty() && !slot.roles.iter().any(|r| roles.contains(r)) {
            return Err(CallError::new(ErrorClass::Authorisation, codes::ROLE_REFUSED, "the caller's roles do not allow this upload slot"));
        }
        let name = display_name(&req.name)?;
        if req.size > self.max_upload {
            return Err(CallError::new(ErrorClass::Limit, codes::FILE_TOO_LARGE, format!("the file is {} bytes; this backend accepts {} at most", req.size, self.max_upload)));
        }
        if !mime_allowed(&slot.mime, &req.mime) {
            return Err(bad(format!("the type \"{}\" is not allowed in the slot \"{}\"", req.mime, slot.name)));
        }
        if req.sha256.len() != 64 || !req.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(bad("sha256 must be 64 hexadecimal digits"));
        }
        let mut uploads = self.uploads.lock().map_err(internal)?;
        if let Some(id) = &req.resume {
            if !is_file_id(id) {
                return Err(bad("not an upload id"));
            }
            let u = uploads.get(id).ok_or_else(|| missing("there is no such upload to resume (it may have expired); start it again"))?;
            // Anyone else's upload, or one with other particulars, does not exist for this caller.
            if u.subject != subject || u.slot != req.slot || u.size != req.size || u.sha256 != req.sha256.to_ascii_lowercase() {
                return Err(missing("there is no such upload to resume (it may have expired); start it again"));
            }
            return Ok((id.clone(), u.have));
        }
        std::fs::create_dir_all(self.incoming_dir()).map_err(internal)?;
        let id = random_hex(16);
        let part = self.incoming_dir().join(format!("{id}.part"));
        std::fs::File::create(&part).map_err(internal)?;
        // Uploads nobody came back for are dropped with their pieces.
        let now = Instant::now();
        uploads.retain(|_, u| {
            let keep = now.duration_since(u.touched) < UPLOAD_IDLE;
            if !keep {
                let _ = std::fs::remove_file(&u.part);
            }
            keep
        });
        uploads.insert(
            id.clone(),
            Upload { subject: subject.to_string(), slot: req.slot.clone(), name, size: req.size, mime: req.mime.clone(), sha256: req.sha256.to_ascii_lowercase(), part, have: 0, touched: now },
        );
        Ok((id, 0))
    }

    /// `put-chunk`: the next piece, which must start exactly where the last ended.
    pub fn chunk(&self, upload: &str, offset: u64, data_b64: &str) -> Result<u64, CallError> {
        let data = B64.decode(data_b64).map_err(|_| bad("the chunk is not valid base64"))?;
        let mut uploads = self.uploads.lock().map_err(internal)?;
        let u = uploads.get_mut(upload).ok_or_else(|| missing("there is no upload in progress"))?;
        if offset != u.have {
            return Err(bad(format!("the next chunk must start at byte {} (it starts at {offset}); begin again with resume to continue", u.have)));
        }
        if u.have + data.len() as u64 > u.size {
            return Err(bad("the chunk goes past the size declared for the file"));
        }
        if u.have == 0 && !sniff(&u.mime, &data) {
            let _ = std::fs::remove_file(&u.part);
            uploads.remove(upload);
            return Err(bad("the content is not of the declared type"));
        }
        let mut f = std::fs::OpenOptions::new().write(true).open(&u.part).map_err(internal)?;
        f.seek(SeekFrom::Start(u.have)).map_err(internal)?;
        f.write_all(&data).map_err(internal)?;
        u.have += data.len() as u64;
        u.touched = Instant::now();
        Ok(u.have)
    }

    /// `put-end`: verify and place the file. Returns `(file id, size)`.
    pub fn end(&self, upload: &str) -> Result<(String, u64), CallError> {
        let u = {
            let mut uploads = self.uploads.lock().map_err(internal)?;
            let u = uploads.get(upload).ok_or_else(|| missing("there is no upload in progress"))?;
            if u.have != u.size {
                return Err(bad(format!("the upload holds {} of {} bytes", u.have, u.size)));
            }
            uploads.remove(upload).expect("present")
        };
        let (n, sum) = sha256_of(&u.part).map_err(internal)?;
        if n != u.size || sum != u.sha256 {
            let _ = std::fs::remove_file(&u.part);
            return Err(bad("the file does not match the checksum that was declared"));
        }
        std::fs::create_dir_all(self.files_dir()).map_err(internal)?;
        let id = random_hex(16);
        let dest = self.files_dir().join(&id);
        std::fs::rename(&u.part, &dest).map_err(internal)?;
        let meta = serde_json::json!({"subject": u.subject, "slot": u.slot, "name": u.name, "mime": u.mime, "size": u.size, "sha256": u.sha256});
        std::fs::write(self.files_dir().join(format!("{id}.meta")), meta.to_string()).map_err(internal)?;
        Ok((id, u.size))
    }

    /// A file this subject uploaded, by the id `put-done` gave: where it is on disk.
    pub fn uploaded_path(&self, file: &str, subject: &str) -> Option<PathBuf> {
        if !is_file_id(file) {
            return None;
        }
        let meta = std::fs::read_to_string(self.files_dir().join(format!("{file}.meta"))).ok()?;
        let v: serde_json::Value = serde_json::from_str(&meta).ok()?;
        if v.get("subject").and_then(|s| s.as_str()) != Some(subject) {
            return None;
        }
        let p = self.files_dir().join(file);
        p.is_file().then_some(p)
    }

    /// A program offers a file it holds for download by the caller. Returns the id
    /// to hand to the client, who asks `get-file` for the address.
    pub fn publish(&self, path: &Path, subject: &str) -> Result<String, String> {
        let (size, sha256) = sha256_of(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let id = random_hex(16);
        let mut p = self.published.lock().map_err(|_| "busy".to_string())?;
        let now = Instant::now();
        p.retain(|_, v| v.expires > now);
        p.insert(id.clone(), Published { path: path.to_path_buf(), subject: subject.to_string(), expires: now + PUBLISH_TTL, size, sha256 });
        Ok(id)
    }

    /// `get-file`: issue a short-lived address for a file this subject uploaded or was offered.
    pub fn offer(&self, file: &str, subject: &str, ip: IpAddr) -> Result<Offered, CallError> {
        if !is_file_id(file) {
            return Err(missing("there is no such file"));
        }
        let (path, size, sha256, name) = if let Some(p) = self.uploaded_path(file, subject) {
            let meta = std::fs::read_to_string(self.files_dir().join(format!("{file}.meta"))).map_err(internal)?;
            let v: serde_json::Value = serde_json::from_str(&meta).map_err(internal)?;
            (p, v["size"].as_u64().unwrap_or(0), v["sha256"].as_str().unwrap_or("").to_string(), v["name"].as_str().unwrap_or("download").to_string())
        } else {
            let p = self.published.lock().map_err(internal)?;
            match p.get(file) {
                Some(x) if x.subject == subject && x.expires > Instant::now() => (x.path.clone(), x.size, x.sha256.clone(), x.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "download".into())),
                _ => return Err(missing("there is no such file")),
            }
        };
        let token = random_hex(32);
        let mut offers = self.offers.lock().map_err(internal)?;
        let now = Instant::now();
        offers.retain(|_, o| o.expires > now);
        offers.insert(token.clone(), Offer { path, subject: subject.to_string(), ip, expires: now + self.download_ttl, size, name });
        Ok(Offered { token, expires_in: self.download_ttl.as_secs(), size, sha256 })
    }

    /// `GET /dl/<token>`: the file, if the address is live and this is the source it was issued to.
    pub fn download(&self, token: &str, ip: IpAddr) -> Option<Download> {
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
            return None;
        }
        let mut offers = self.offers.lock().ok()?;
        let now = Instant::now();
        offers.retain(|_, o| o.expires > now);
        let o = offers.get(token)?;
        if o.ip != ip {
            return None;
        }
        let _ = &o.subject;
        Some(Download { path: o.path.clone(), size: o.size, name: o.name.clone() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_that_could_mean_a_path_is_refused() {
        for bad in ["", "../x", "a/b", "a\\b", "..", "x..y", "a\u{0}b", "c:evil", "tab\there"] {
            assert!(display_name(bad).is_err(), "{bad:?}");
        }
        assert_eq!(display_name(" photo 1.png ").unwrap(), "photo 1.png");
    }

    #[test]
    fn content_is_checked_against_the_declared_type_where_the_type_fixes_the_first_bytes() {
        assert!(sniff("image/png", &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0]));
        assert!(!sniff("image/png", b"GIF89a"));
        assert!(sniff("application/pdf", b"%PDF-1.7"));
        assert!(!sniff("application/pdf", b"MZ"));
        assert!(sniff("text/plain", b"anything"));
    }

    fn store(name: &str, ttl: u64) -> (FileStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!("prc-filestore-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cfg = FilesResolved {
            dir: dir.clone(),
            max_upload_bytes: 1000,
            slots: vec![UploadSlot { name: "docs".into(), mime: vec!["text/plain".into(), "image/*".into()], roles: vec![] }, UploadSlot { name: "private".into(), mime: vec!["text/plain".into()], roles: vec!["clerk".into()] }],
            download_ttl: ttl,
        };
        (FileStore::new(&cfg), dir)
    }

    fn begin_for(slot: &str, data: &[u8], mime: &str, resume: Option<String>) -> PutBegin {
        let sum: String = Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect();
        PutBegin { t: "put-begin".into(), id: 1, slot: slot.into(), name: "a.txt".into(), size: data.len() as u64, mime: mime.into(), sha256: sum, resume }
    }

    #[test]
    fn an_upload_arrives_in_pieces_resumes_where_it_stopped_and_is_verified_at_the_end() {
        let (s, dir) = store("flow", 60);
        let roles = std::collections::BTreeSet::new();
        let data = b"the quick brown fox jumps over the lazy dog".to_vec();
        let (up, have) = s.begin("ana", &roles, &begin_for("docs", &data, "text/plain", None)).unwrap();
        assert_eq!(have, 0);
        assert_eq!(s.chunk(&up, 0, &B64.encode(&data[..10])).unwrap(), 10);
        // The line breaks; a new begin with the resume id says where to go on.
        let (same, have) = s.begin("ana", &roles, &begin_for("docs", &data, "text/plain", Some(up.clone()))).unwrap();
        assert_eq!((same.as_str(), have), (up.as_str(), 10));
        // A piece that does not start there, or past the end, is refused.
        assert!(s.chunk(&up, 0, &B64.encode(&data[..10])).unwrap_err().message.contains("must start at byte 10"));
        assert!(s.chunk(&up, 10, &B64.encode(&[b'x'; 100])).unwrap_err().message.contains("past the size"));
        // Not finished yet.
        assert!(s.end(&up).unwrap_err().message.contains("holds 10 of"));
        assert_eq!(s.chunk(&up, 10, &B64.encode(&data[10..])).unwrap(), data.len() as u64);
        let (file, size) = s.end(&up).unwrap();
        assert_eq!(size, data.len() as u64);
        assert!(is_file_id(&file));
        // Only the uploader reaches it.
        assert_eq!(std::fs::read(s.uploaded_path(&file, "ana").unwrap()).unwrap(), data);
        assert!(s.uploaded_path(&file, "bob").is_none());
        assert!(s.uploaded_path("../../etc/passwd", "ana").is_none());
        // Somebody else cannot resume ana's upload either.
        let (up2, _) = s.begin("ana", &roles, &begin_for("docs", &data, "text/plain", None)).unwrap();
        assert!(s.begin("bob", &roles, &begin_for("docs", &data, "text/plain", Some(up2))).unwrap_err().message.contains("no such upload"));
        assert_eq!(s.uploads_in_progress(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_upload_is_refused_for_its_slot_role_size_type_name_checksum_and_content() {
        let (s, dir) = store("refuse", 60);
        let roles = std::collections::BTreeSet::new();
        let clerk: std::collections::BTreeSet<String> = ["clerk".to_string()].into_iter().collect();
        let data = b"hello".to_vec();
        assert_eq!(s.begin("a", &roles, &begin_for("nowhere", &data, "text/plain", None)).unwrap_err().code, codes::UNKNOWN_PROGRAM);
        assert_eq!(s.begin("a", &roles, &begin_for("private", &data, "text/plain", None)).unwrap_err().code, codes::ROLE_REFUSED);
        assert!(s.begin("a", &clerk, &begin_for("private", &data, "text/plain", None)).is_ok());
        assert_eq!(s.begin("a", &roles, &begin_for("docs", &data, "application/x-msdownload", None)).unwrap_err().code, codes::INVALID_PAYLOAD);
        let mut big = begin_for("docs", &data, "text/plain", None);
        big.size = 5000;
        assert_eq!(s.begin("a", &roles, &big).unwrap_err().code, codes::FILE_TOO_LARGE);
        let mut named = begin_for("docs", &data, "text/plain", None);
        named.name = "../../x".into();
        assert_eq!(s.begin("a", &roles, &named).unwrap_err().code, codes::INVALID_PAYLOAD);
        let mut sum = begin_for("docs", &data, "text/plain", None);
        sum.sha256 = "zz".into();
        assert_eq!(s.begin("a", &roles, &sum).unwrap_err().code, codes::INVALID_PAYLOAD);
        // The checksum declared is the checksum checked: other bytes of the same length fail at the end.
        let mut wrong = begin_for("docs", &data, "text/plain", None);
        wrong.sha256 = "0".repeat(64);
        let (up, _) = s.begin("a", &roles, &wrong).unwrap();
        s.chunk(&up, 0, &B64.encode(&data)).unwrap();
        assert!(s.end(&up).unwrap_err().message.contains("checksum"));
        assert!(std::fs::read_dir(dir.join("files")).map(|d| d.count()).unwrap_or(0) == 0, "nothing was placed");
        // An image that is not one.
        let (up, _) = s.begin("a", &roles, &begin_for("docs", &data, "image/png", None)).unwrap();
        assert!(s.chunk(&up, 0, &B64.encode(&data)).unwrap_err().message.contains("not of the declared type"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_download_address_is_tied_to_its_subject_and_source_and_dies_with_its_time() {
        let (s, dir) = store("offer", 1);
        let roles = std::collections::BTreeSet::new();
        let data = b"payload".to_vec();
        let (up, _) = s.begin("ana", &roles, &begin_for("docs", &data, "text/plain", None)).unwrap();
        s.chunk(&up, 0, &B64.encode(&data)).unwrap();
        let (file, _) = s.end(&up).unwrap();
        let here: IpAddr = "127.0.0.1".parse().unwrap();
        let elsewhere: IpAddr = "10.1.2.3".parse().unwrap();
        // Somebody else's file, or a made-up one, cannot be offered.
        assert!(s.offer(&file, "bob", here).is_err());
        assert!(s.offer("not-an-id", "ana", here).is_err());
        let o = s.offer(&file, "ana", here).unwrap();
        assert_eq!((o.size, o.expires_in, o.token.len()), (data.len() as u64, 1, 64));
        assert!(s.download(&o.token, elsewhere).is_none(), "another source gets nothing");
        assert!(s.download("short", here).is_none());
        let d = s.download(&o.token, here).expect("live for the source it was issued to");
        assert_eq!((d.size, d.name.as_str()), (data.len() as u64, "a.txt"));
        std::thread::sleep(Duration::from_millis(1200));
        assert!(s.download(&o.token, here).is_none(), "expired");
        // A file a program publishes is offered to the subject it was published for, not to others.
        let local = dir.join("report.txt");
        std::fs::write(&local, b"report").unwrap();
        let id = s.publish(&local, "ana").unwrap();
        assert!(s.offer(&id, "bob", here).is_err());
        assert!(s.offer(&id, "ana", here).is_ok());
        assert!(s.publish(&dir.join("missing"), "ana").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_the_backends_own_ids_are_ids() {
        assert!(is_file_id("0123456789abcdef0123456789abcdef"));
        assert!(!is_file_id("../0123456789abcdef0123456789abc"));
        assert!(!is_file_id("0123456789ABCDEF0123456789ABCDEF"));
        assert!(!is_file_id("short"));
    }
}
