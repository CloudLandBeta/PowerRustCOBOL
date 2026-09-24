// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Indexed files a program registers **by path** while it runs (spec 075).
//!
//! A file compiled into the program is described by its `FD`. A registered
//! file has none, so its layout comes from its `.cidx` — and is **checked
//! against the schema the data file stores about itself** before a model sees
//! it (R3, R4). The file is the user's data: it is only ever read, never
//! written, converted or recovered (R14–R16), and nothing here keeps its path
//! after the program ends (R6a).
//!
//! What this module decides, in order, is where a file is ([`Location`]),
//! whether its `.cidx` describes it ([`validate`]), and whether it is held in
//! memory, read in place from disk, or refused ([`decide`]). Every refusal
//! carries a stable code a program can translate ([`Refusal`]).

use std::path::PathBuf;

use crate::indexed::{
    IndexedFileInfo, KeyDescriptor, KeyEncoding, KeyOrdering, KeyPart, KeySpec,
};
use crate::mcp_tool::{ColumnDescription, ColumnLayout, FileDescription};

/// Why a file was not registered. `code` is stable; `message` is English, with
/// any password masked; `size` and `bound` are the two numbers of a size
/// refusal (0 otherwise).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub code: &'static str,
    pub message: String,
    pub size: u64,
    pub bound: u64,
}

impl Refusal {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Refusal {
            code,
            message: message.into(),
            size: 0,
            bound: 0,
        }
    }
}

/// The refusal codes (spec 075 §4; plan §1), for a program's translation table.
pub mod code {
    pub const BAD_PATH: &str = "BAD-PATH";
    pub const NOT_FOUND: &str = "NOT-FOUND";
    pub const CIDX_NOT_FOUND: &str = "CIDX-NOT-FOUND";
    pub const ACCESS_DENIED: &str = "ACCESS-DENIED";
    pub const UNREACHABLE: &str = "UNREACHABLE";
    pub const CIDX_INVALID: &str = "CIDX-INVALID";
    pub const NO_PURPOSE: &str = "NO-PURPOSE";
    pub const NO_FIELDS: &str = "NO-FIELDS";
    pub const NO_FIELD_DESCRIPTIONS: &str = "NO-FIELD-DESCRIPTIONS";
    pub const NOT_INDEXED: &str = "NOT-INDEXED";
    pub const FORMAT_UNSUPPORTED: &str = "FORMAT-UNSUPPORTED";
    pub const RECORD_LENGTH_MISMATCH: &str = "RECORD-LENGTH-MISMATCH";
    pub const KEY_MISMATCH: &str = "KEY-MISMATCH";
    pub const CORRUPT: &str = "CORRUPT";
    pub const JOURNAL_PRESENT: &str = "JOURNAL-PRESENT";
    pub const NEEDS_UPGRADE: &str = "NEEDS-UPGRADE";
    pub const TOO_LARGE_FOR_LIMIT: &str = "TOO-LARGE-FOR-LIMIT";
    pub const TOO_LARGE_FOR_FREE_MEMORY: &str = "TOO-LARGE-FOR-FREE-MEMORY";
    pub const SMB_UNAVAILABLE: &str = "SMB-UNAVAILABLE";
}

// ── Where the file is ────────────────────────────────────────────────────────

/// A path as a program gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    /// A path the operating system opens: local, a UNC path on Windows, a
    /// mounted share on macOS or Linux (R7, R8).
    Fs(PathBuf),
    /// An `smb://` address, read without mounting the share (R9).
    Smb(SmbUrl),
}

impl Location {
    /// Classify `text`. A relative path is anchored like every other path a
    /// running form names (`cobolt_forms::assets::resolve`).
    pub fn parse(text: &str) -> Result<Location, Refusal> {
        let text = text.trim();
        if text.is_empty() {
            return Err(Refusal::new(code::BAD_PATH, "no path was given"));
        }
        if text.len() >= 6 && text[..6].eq_ignore_ascii_case("smb://") {
            return SmbUrl::parse(text).map(Location::Smb);
        }
        if cfg!(not(windows)) && (text.starts_with("\\\\") || text.starts_with("//")) {
            return Err(Refusal::new(
                code::BAD_PATH,
                format!(
                    "{text} is a Windows network path; on this system use the mounted \
                     share's path, or an smb://server/share/… address"
                ),
            ));
        }
        Ok(Location::Fs(cobolt_forms::assets::resolve(text)))
    }

    /// The same location with `suffix` added to its file name — the `.jrn`
    /// beside a data file.
    pub fn with_suffix(&self, suffix: &str) -> Location {
        match self {
            Location::Fs(p) => {
                let mut s = p.clone().into_os_string();
                s.push(suffix);
                Location::Fs(PathBuf::from(s))
            }
            Location::Smb(u) => Location::Smb(SmbUrl {
                path: format!("{}{suffix}", u.path),
                ..u.clone()
            }),
        }
    }

    /// How the location is shown in a message: a path as given, an `smb://`
    /// address with its password masked (R13).
    pub fn display(&self) -> String {
        match self {
            Location::Fs(p) => p.display().to_string(),
            Location::Smb(u) => u.to_string(),
        }
    }
}

/// `smb://[domain;][user[:password]@]server[:port]/share/path`.
///
/// `Display` and `Debug` never show the password (R13): it exists only to log
/// in, and the value itself is dropped with the `SmbUrl`.
#[derive(Clone, PartialEq, Eq)]
pub struct SmbUrl {
    pub domain: Option<String>,
    pub user: Option<String>,
    pub password: Option<String>,
    pub host: String,
    pub port: u16,
    pub share: String,
    /// The file within the share, `/`-separated, never empty.
    pub path: String,
}

impl SmbUrl {
    pub fn parse(text: &str) -> Result<SmbUrl, Refusal> {
        // The message never repeats `text`: it may hold a password.
        let bad = |why: &str| Refusal::new(code::BAD_PATH, format!("the smb:// address {why}"));
        let rest = &text.trim()[6..];
        let (authority, tail) = rest.split_once('/').unwrap_or((rest, ""));
        let (userinfo, hostport) = match authority.rsplit_once('@') {
            Some((u, h)) => (Some(u), h),
            None => (None, authority),
        };
        let (mut domain, mut user, mut password) = (None, None, None);
        if let Some(info) = userinfo {
            let (who, pass) = match info.split_once(':') {
                Some((w, p)) => (w, Some(p)),
                None => (info, None),
            };
            let (dom, name) = match who.split_once(';') {
                Some((d, n)) => (Some(d), n),
                None => (None, who),
            };
            domain = dom.filter(|d| !d.is_empty()).map(percent_decode);
            user = Some(percent_decode(name)).filter(|u| !u.is_empty());
            password = pass.map(percent_decode);
        }
        // An IPv6 address is bracketed, so its own colons are not a port.
        let port_at = match hostport.rfind(']') {
            Some(close) => hostport[close..].find(':').map(|i| close + i),
            None => hostport.rfind(':'),
        };
        let (host, port) = match port_at {
            Some(i) => {
                let port = hostport[i + 1..].parse::<u16>().map_err(|_| bad("has an invalid port"))?;
                (&hostport[..i], port)
            }
            None => (hostport, 445),
        };
        let host = host.trim_start_matches('[').trim_end_matches(']').to_string();
        if host.is_empty() {
            return Err(bad("names no server"));
        }
        let mut segments = tail.split('/').filter(|s| !s.is_empty());
        let share = segments.next().map(percent_decode).ok_or_else(|| bad("names no share"))?;
        let path = segments.map(percent_decode).collect::<Vec<_>>().join("/");
        if path.is_empty() || path.split('/').any(|s| s == "..") {
            return Err(bad("names no file within the share"));
        }
        Ok(SmbUrl {
            domain,
            user,
            password,
            host,
            port,
            share,
            path,
        })
    }

    /// `host:port`, for connecting.
    pub fn server(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

impl std::fmt::Display for SmbUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("smb://")?;
        if let Some(d) = &self.domain {
            write!(f, "{d};")?;
        }
        if let Some(u) = &self.user {
            f.write_str(u)?;
            if self.password.is_some() {
                f.write_str(":****")?;
            }
            f.write_str("@")?;
        }
        f.write_str(&self.host)?;
        if self.port != 445 {
            write!(f, ":{}", self.port)?;
        }
        write!(f, "/{}/{}", self.share, self.path)
    }
}

impl std::fmt::Debug for SmbUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SmbUrl({self})")
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(b) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ── Does the .cidx describe the file? ────────────────────────────────────────

/// Which engine a file's first bytes call for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    /// `PRCIDX1` — loaded whole.
    Memory,
    /// `PRCIDXD1` — paged, readable in place.
    Disk,
}

/// The container a file's first bytes name, or why it cannot be registered.
pub fn container(head: &[u8]) -> Result<Container, Refusal> {
    if head.starts_with(b"PRCIDX1\0") {
        Ok(Container::Memory)
    } else if head.starts_with(b"PRCIDXD1") {
        Ok(Container::Disk)
    } else if head.starts_with(b"redb") || head.starts_with(b"PRCISAM1") {
        Err(Refusal::new(
            code::FORMAT_UNSUPPORTED,
            "the file's format carries no schema to check its .cidx against; \
             rewrite it with STORAGE IS DISK or MEMORY",
        ))
    } else {
        Err(Refusal::new(code::NOT_INDEXED, "the file is not a PowerRustCOBOL indexed file"))
    }
}

/// What a checked `.cidx` gives: how to read the file, and how to describe it.
#[derive(Debug, Clone)]
pub struct Checked {
    pub description: FileDescription,
    pub record_len: usize,
    pub primary: KeySpec,
    pub alternates: Vec<KeySpec>,
    pub columns: Vec<ColumnLayout>,
}

/// Check a `.cidx` (as text) against the schema its data file stores
/// (`stored`), and against R5's rule that a model can only choose a file that
/// says what it is for and what its fields mean.
pub fn validate(cidx_text: &str, stored: &IndexedFileInfo) -> Result<Checked, Refusal> {
    let invalid = |why: String| Refusal::new(code::CIDX_INVALID, why);
    let def = cobolt_indexed::load_indexed_from_str(cidx_text)
        .map_err(|e| invalid(format!("the .cidx cannot be read: {e:?}")))?;
    let mut leaves = Vec::new();
    for field in &def.fields {
        collect_leaves(field, &mut leaves);
    }
    if leaves.is_empty() {
        return Err(Refusal::new(code::NO_FIELDS, "the .cidx describes no fields"));
    }
    cobolt_indexed::validate_definition(&def).map_err(|e| invalid(format!("the .cidx is not valid: {e}")))?;
    if let Some(bad) = leaves.iter().find(|l| l.offset.is_none() || l.length.is_none()) {
        return Err(invalid(format!("field {} has no position in the record", bad.name)));
    }
    if def.comment.trim().is_empty() {
        return Err(Refusal::new(code::NO_PURPOSE, "the .cidx does not say what the file is for"));
    }
    if leaves.iter().all(|l| l.comment.trim().is_empty()) {
        return Err(Refusal::new(
            code::NO_FIELD_DESCRIPTIONS,
            "no field in the .cidx says what it means",
        ));
    }

    let record_len = def.record_length();
    if record_len != stored.record_format.max_len() {
        return Err(Refusal::new(
            code::RECORD_LENGTH_MISMATCH,
            format!(
                "the .cidx declares {record_len}-byte records and the file holds {}-byte records",
                stored.record_format.max_len()
            ),
        ));
    }
    if let Some(field) = leaves
        .iter()
        .find(|l| l.offset.unwrap_or(0) + l.length.unwrap_or(0) > record_len)
    {
        return Err(invalid(format!("field {} runs past the end of the record", field.name)));
    }

    let single = |k: &cobolt_indexed::KeyDef| -> Option<KeySpec> {
        match k.parts.as_slice() {
            [p] => Some(KeySpec {
                offset: p.offset as usize,
                len: p.length as usize,
                duplicates: k.duplicates_allowed,
            }),
            _ => None,
        }
    };
    let key_mismatch = || {
        Refusal::new(
            code::KEY_MISMATCH,
            "the keys the .cidx declares are not the keys the file was written with",
        )
    };
    let primary = single(&def.keys.primary).ok_or_else(key_mismatch)?;
    let alternates = def
        .keys
        .alternates
        .iter()
        .map(single)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(key_mismatch)?;
    let descriptor = |n: u16, k: &KeySpec| KeyDescriptor {
        key_number: n,
        name: None,
        parts: vec![KeyPart {
            offset: k.offset as u32,
            length: k.len as u32,
            encoding: KeyEncoding::Bytes,
        }],
        duplicates_allowed: k.duplicates,
        ordering: KeyOrdering::Ascending,
    };
    let declared_alts: Vec<KeyDescriptor> = alternates
        .iter()
        .enumerate()
        .map(|(i, k)| descriptor(i as u16 + 2, k))
        .collect();
    let declared = IndexedFileInfo {
        record_format: stored.record_format.clone(),
        key_count: 1 + declared_alts.len() as u16,
        total_key_length: 0,
        primary: descriptor(1, &primary),
        alternates: declared_alts,
    };
    if !crate::indexed::schema_equivalent(&declared, stored) {
        return Err(key_mismatch());
    }

    Ok(Checked {
        description: FileDescription {
            name: def.name.clone(),
            purpose: def.comment.trim().to_string(),
            columns: leaves
                .iter()
                .map(|l| ColumnDescription {
                    name: l.name.clone(),
                    description: l.comment.trim().to_string(),
                })
                .collect(),
        },
        record_len: record_len as usize,
        primary,
        alternates,
        columns: leaves
            .iter()
            .map(|l| ColumnLayout {
                name: l.name.clone(),
                offset: l.offset.unwrap_or(0) as usize,
                len: l.length.unwrap_or(0) as usize,
            })
            .collect(),
    })
}

/// Every elementary field, with or without a position — `IndexedField::leaves`
/// skips the ones without, which is exactly what must be caught here.
fn collect_leaves<'a>(
    field: &'a cobolt_indexed::IndexedField,
    out: &mut Vec<&'a cobolt_indexed::IndexedField>,
) {
    if field.children.is_empty() {
        out.push(field);
    } else {
        for child in &field.children {
            collect_leaves(child, out);
        }
    }
}

// ── Memory, or disk ──────────────────────────────────────────────────────────

/// How a registered file is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// Held in memory (R17).
    Memory,
    /// Read in place from disk, a page at a time (R19).
    InPlace,
}

/// Where a file may be read from when it does not fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source {
    pub container: Container,
    /// On the file system (local or an OS network path), not `smb://`.
    pub on_file_system: bool,
    /// Reading it in place would need a temporary copy (an old directory
    /// format, an unfinished reclaim).
    pub needs_copy: bool,
}

/// R17–R20: a file fits when it is under the project's limit **and** at most
/// half of the free memory; otherwise a local `PRCIDXD1` that needs no copy is
/// read in place, and anything else is refused with both numbers.
pub fn decide(source: Source, size: u64, limit: u64, free: u64) -> Result<Fit, Refusal> {
    let half_free = free / 2;
    if size <= limit && size <= half_free {
        return Ok(Fit::Memory);
    }
    if source.container == Container::Disk && source.on_file_system {
        if source.needs_copy {
            return Err(Refusal::new(
                code::NEEDS_UPGRADE,
                "the file is too large to hold in memory and is in an older format that \
                 cannot be read in place without changing it; open it once with the \
                 application that owns it",
            ));
        }
        return Ok(Fit::InPlace);
    }
    let (code, bound, what) = if size > limit {
        (code::TOO_LARGE_FOR_LIMIT, limit, "this project's file-search memory limit")
    } else {
        (code::TOO_LARGE_FOR_FREE_MEMORY, half_free, "half of this machine's free memory")
    };
    let why = match (source.container, source.on_file_system) {
        (Container::Disk, false) => "a file on an smb:// share cannot be read in place",
        _ => "a STORAGE IS MEMORY file cannot be read in place; rewrite it with STORAGE IS DISK",
    };
    Err(Refusal {
        code,
        message: format!("the file is {size} bytes, over {what} of {bound} bytes, and {why}"),
        size,
        bound,
    })
}

/// The machine's free memory, as the fit test sees it.
pub trait FreeMemory: Send + Sync {
    fn available_bytes(&self) -> u64;
}

/// The operating system's available memory.
///
/// `sysinfo`'s *available* figure is unreliable on macOS — measured on this
/// project's own machine it read 0 bytes on one call and 815 MB on the next
/// while the system reported half its 18 GB free — so the larger of it and
/// total-minus-used is taken. Both count reclaimable cache as used or not
/// differently per OS; the guide says the figure is an estimate.
pub struct SystemMemory;

impl FreeMemory for SystemMemory {
    fn available_bytes(&self) -> u64 {
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        sys.available_memory()
            .max(sys.total_memory().saturating_sub(sys.used_memory()))
    }
}

/// A fixed figure — how a test says "this machine has 100 MB free".
pub struct FixedMemory(pub u64);

impl FreeMemory for FixedMemory {
    fn available_bytes(&self) -> u64 {
        self.0
    }
}

// ── Reading a location ───────────────────────────────────────────────────────

/// How a registration reads the bytes it needs — a file system path here, an
/// `smb://` share through [`crate::smb_source`]. Read only: there is no way to
/// write through it.
pub trait Fetch {
    /// The file's size, `None` when it does not exist.
    fn size(&self, at: &Location) -> Result<Option<u64>, Refusal>;
    /// The whole file, refused if it is longer than `max`.
    fn read(&self, at: &Location, max: u64) -> Result<Vec<u8>, Refusal>;
}

/// Local paths and the OS's own network paths; `smb://` through `smb`, which
/// is `None` in a build without the `smb` feature.
pub struct Fetcher<'a> {
    pub smb: Option<&'a dyn Fetch>,
}

impl Fetcher<'_> {
    fn smb(&self) -> Result<&dyn Fetch, Refusal> {
        self.smb.ok_or_else(|| {
            Refusal::new(
                code::SMB_UNAVAILABLE,
                "this application was built without smb:// support",
            )
        })
    }
}

fn io_refusal(at: &Location, e: &std::io::Error) -> Refusal {
    match e.kind() {
        std::io::ErrorKind::NotFound => Refusal::new(code::NOT_FOUND, format!("{} does not exist", at.display())),
        std::io::ErrorKind::PermissionDenied => {
            Refusal::new(code::ACCESS_DENIED, format!("{} cannot be read: access refused", at.display()))
        }
        _ => Refusal::new(code::UNREACHABLE, format!("{} cannot be read: {e}", at.display())),
    }
}

impl Fetch for Fetcher<'_> {
    fn size(&self, at: &Location) -> Result<Option<u64>, Refusal> {
        match at {
            Location::Fs(p) => match std::fs::metadata(p) {
                Ok(m) => Ok(Some(m.len())),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(io_refusal(at, &e)),
            },
            Location::Smb(_) => self.smb()?.size(at),
        }
    }

    fn read(&self, at: &Location, max: u64) -> Result<Vec<u8>, Refusal> {
        match at {
            Location::Fs(p) => {
                use std::io::Read;
                let f = std::fs::File::open(p).map_err(|e| io_refusal(at, &e))?;
                let mut out = Vec::new();
                f.take(max + 1).read_to_end(&mut out).map_err(|e| io_refusal(at, &e))?;
                if out.len() as u64 > max {
                    return Err(Refusal {
                        code: code::TOO_LARGE_FOR_LIMIT,
                        message: format!("{} is larger than {max} bytes", at.display()),
                        size: out.len() as u64,
                        bound: max,
                    });
                }
                Ok(out)
            }
            Location::Smb(_) => self.smb()?.read(at, max),
        }
    }
}

// ── Registering ──────────────────────────────────────────────────────────────

/// A registered file, ready for the tool set.
#[derive(Debug)]
pub struct Registered {
    pub description: FileDescription,
    pub access: crate::mcp_tool::FileAccess,
    pub source: crate::mcp_tool::FileSource,
    pub fit: Fit,
    /// The data file's size, and the limit it was compared with.
    pub size: u64,
    pub limit: u64,
}

/// The largest `.cidx` read — a definition is a few kilobytes.
const MAX_CIDX_BYTES: u64 = 16 * 1024 * 1024;

/// Register the data file at `data` described by the `.cidx` at `cidx`, under
/// `name` (the `.cidx`'s own file name when empty). Nothing is ever written
/// to either file (R14, R15); a file that could only be read by writing —
/// a recovery journal beside it — is refused (R16).
pub fn register(
    data: &str,
    cidx: &str,
    name: &str,
    limit: u64,
    free: &dyn FreeMemory,
    fetch: &dyn Fetch,
) -> Result<Registered, Refusal> {
    use crate::mcp_tool::{FileAccess, FileSource};

    let data_at = Location::parse(data)?;
    let cidx_at = Location::parse(cidx)?;

    if fetch.size(&data_at.with_suffix(".jrn"))?.is_some() {
        return Err(Refusal::new(
            code::JOURNAL_PRESENT,
            format!(
                "an interrupted write left a recovery journal beside {}; it cannot be \
                 read without changing it — open it once with the application that owns it",
                data_at.display()
            ),
        ));
    }
    let size = fetch
        .size(&data_at)?
        .ok_or_else(|| Refusal::new(code::NOT_FOUND, format!("{} does not exist", data_at.display())))?;
    let cidx_text = match fetch.read(&cidx_at, MAX_CIDX_BYTES) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(r) if r.code == code::NOT_FOUND => {
            return Err(Refusal::new(code::CIDX_NOT_FOUND, format!("{} does not exist", cidx_at.display())))
        }
        Err(r) => return Err(r),
    };
    let free = free.available_bytes();
    let corrupt = |e: String| Refusal::new(code::CORRUPT, format!("{} is damaged: {e}", data_at.display()));

    let (checked, source, fit) = match &data_at {
        Location::Fs(path) => {
            use std::io::Read;
            let mut head = [0u8; 8];
            std::fs::File::open(path)
                .and_then(|mut f| f.read_exact(&mut head))
                .map_err(|e| io_refusal(&data_at, &e))?;
            match container(&head)? {
                Container::Disk => {
                    let stored = crate::indexed_disk::DiskIndexedFile::inspect_path(path)
                        .map_err(|e| corrupt(e.to_string()))?
                        .ok_or_else(|| corrupt("no schema header".into()))?;
                    let checked = validate(&cidx_text, &stored)?;
                    let needs_copy = crate::indexed_disk::DiskIndexedFile::input_needs_copy(path)
                        .map_err(|e| corrupt(e.to_string()))?;
                    let src = Source { container: Container::Disk, on_file_system: true, needs_copy };
                    match decide(src, size, limit, free)? {
                        Fit::Memory => {
                            let records = crate::indexed_disk::read_disk_container(
                                path,
                                checked.record_len,
                                &checked.primary,
                                &checked.alternates,
                                true,
                            )
                            .map_err(|st| corrupt(format!("FILE STATUS {st}")))?;
                            (checked, FileSource::Loaded(std::sync::Arc::new(records)), Fit::Memory)
                        }
                        Fit::InPlace => {
                            let alternates = checked.alternates.clone();
                            (checked, FileSource::InPlace { alternates }, Fit::InPlace)
                        }
                    }
                }
                Container::Memory => {
                    let src = Source { container: Container::Memory, on_file_system: true, needs_copy: false };
                    decide(src, size, limit, free)?;
                    let bytes = fetch.read(&data_at, size)?;
                    loaded_from_bytes(&bytes, &cidx_text, Container::Memory, corrupt)?
                }
            }
        }
        Location::Smb(_) => {
            // A share's file is never read in place, whatever its format, so
            // the size decides before a byte is downloaded (R20).
            let src = Source { container: Container::Disk, on_file_system: false, needs_copy: false };
            decide(src, size, limit, free)?;
            let bytes = fetch.read(&data_at, size)?;
            let kind = container(&bytes[..bytes.len().min(8)])?;
            loaded_from_bytes(&bytes, &cidx_text, kind, corrupt)?
        }
    };

    let mut description = checked.description;
    if !name.trim().is_empty() {
        description.name = name.trim().to_ascii_uppercase();
    }
    let access = FileAccess {
        // Shown to no model; for Fs the real path, for smb:// the masked one.
        path: match &data_at {
            Location::Fs(p) => p.clone(),
            Location::Smb(u) => PathBuf::from(u.to_string()),
        },
        record_len: checked.record_len,
        primary: checked.primary,
        columns: checked.columns,
    };
    Ok(Registered {
        description,
        access,
        source,
        fit,
        size,
        limit,
    })
}

/// A file whose bytes are already in memory — a local `PRCIDX1`, or anything
/// from a share: check its schema, then hold its records.
fn loaded_from_bytes(
    bytes: &[u8],
    cidx_text: &str,
    kind: Container,
    corrupt: impl Fn(String) -> Refusal,
) -> Result<(Checked, crate::mcp_tool::FileSource, Fit), Refusal> {
    let records = match kind {
        Container::Memory => {
            let stored = crate::indexed::IndexedFile::inspect_bytes(bytes)
                .map_err(|e| corrupt(e.to_string()))?
                .ok_or_else(|| corrupt("no schema".into()))?;
            let checked = validate(cidx_text, &stored)?;
            let records = crate::indexed::IndexedFile::records_from_bytes(bytes, checked.record_len, checked.primary.clone())
                .map_err(|e| corrupt(e.to_string()))?;
            return Ok((checked, crate::mcp_tool::FileSource::Loaded(std::sync::Arc::new(records)), Fit::Memory));
        }
        // A paged container is read through its engine, from a private
        // temporary copy that is deleted however this ends (R21).
        Container::Disk => TempCopy::new(bytes).map_err(|e| corrupt(e.to_string()))?,
    };
    let stored = crate::indexed_disk::DiskIndexedFile::inspect_path(&records.0)
        .map_err(|e| corrupt(e.to_string()))?
        .ok_or_else(|| corrupt("no schema header".into()))?;
    let checked = validate(cidx_text, &stored)?;
    let rows = crate::indexed_disk::read_disk_container(
        &records.0,
        checked.record_len,
        &checked.primary,
        &checked.alternates,
        true,
    )
    .map_err(|st| corrupt(format!("FILE STATUS {st}")))?;
    Ok((checked, crate::mcp_tool::FileSource::Loaded(std::sync::Arc::new(rows)), Fit::Memory))
}

/// A private temporary copy of downloaded bytes, deleted when dropped.
struct TempCopy(PathBuf);

impl TempCopy {
    fn new(bytes: &[u8]) -> std::io::Result<TempCopy> {
        use std::io::Write;
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "prc-registered-{}-{}.idx",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let copy = TempCopy(path);
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&copy.0)?
            .write_all(bytes)?;
        Ok(copy)
    }
}

impl Drop for TempCopy {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let mut jrn = self.0.clone().into_os_string();
        jrn.push(".jrn");
        let _ = std::fs::remove_file(jrn);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_forms_are_classified() {
        match Location::parse("smb://finance/data/orders.dat").unwrap() {
            Location::Smb(u) => {
                assert_eq!((u.host.as_str(), u.port, u.share.as_str(), u.path.as_str()), ("finance", 445, "data", "orders.dat"));
                assert_eq!(u.user, None);
            }
            other => panic!("{other:?}"),
        }
        let u = SmbUrl::parse("SMB://CORP;ana:s%40cret@10.0.0.5:1445/Share/2025/orders.dat").unwrap();
        assert_eq!(u.domain.as_deref(), Some("CORP"));
        assert_eq!(u.user.as_deref(), Some("ana"));
        assert_eq!(u.password.as_deref(), Some("s@cret"), "percent-decoded");
        assert_eq!((u.port, u.server()), (1445, "10.0.0.5:1445".to_string()));
        assert_eq!(u.path, "2025/orders.dat");
        assert!(matches!(Location::parse("/Volumes/finance/orders.dat").unwrap(), Location::Fs(_)));
        assert!(matches!(Location::parse("/mnt/share/orders.dat").unwrap(), Location::Fs(_)));
        for bad in ["", "smb://", "smb://server", "smb://server/share", "smb://server/share/../x", "smb://h:99999/s/f"] {
            assert_eq!(Location::parse(bad).unwrap_err().code, code::BAD_PATH, "{bad:?}");
        }
        if cfg!(not(windows)) {
            assert_eq!(Location::parse(r"\\server\share\f.dat").unwrap_err().code, code::BAD_PATH);
        }
        let jrn = Location::parse("smb://s/sh/orders.dat").unwrap().with_suffix(".jrn");
        assert_eq!(jrn.display(), "smb://s/sh/orders.dat.jrn");
    }

    /// R13 — no way of printing an address shows its password.
    #[test]
    fn a_password_never_prints() {
        let secret = "Tr0ub4dor&3";
        let u = SmbUrl::parse(&format!("smb://CORP;ana:{secret}@server/share/f.dat")).unwrap();
        let loc = Location::Smb(u.clone());
        for shown in [u.to_string(), format!("{u:?}"), format!("{loc:?}"), loc.display()] {
            assert!(!shown.contains(secret), "{shown}");
            assert!(shown.contains("ana:****@"), "{shown}");
        }
        let err = SmbUrl::parse(&format!("smb://ana:{secret}@server:bad/share/f")).unwrap_err();
        assert!(!err.message.contains(secret));
    }

    fn field(level: u8, name: &str, at: Option<(u32, u32)>, comment: &str) -> cobolt_indexed::IndexedField {
        cobolt_indexed::IndexedField {
            level,
            name: name.into(),
            pic: at.map(|(_, l)| format!("X({l})")).unwrap_or_default(),
            usage: cobolt_indexed::FieldUsage::Display,
            offset: at.map(|a| a.0),
            length: at.map(|a| a.1),
            comment: comment.into(),
            grid_control: None,
            occurs: None,
            redefines: None,
            synchronized: false,
            children: Vec::new(),
        }
    }

    /// ORDERS-FILE: 15-byte records, ORDER-ID (0,5) the primary key.
    fn orders() -> cobolt_indexed::IndexedDefinition {
        let mut def = cobolt_indexed::IndexedDefinition::new("ORDERS-FILE", "orders.dat");
        def.record_format = cobolt_indexed::RecordFormatDef::Fixed { length: 15 };
        def.comment = "Customer orders".into();
        let mut root = field(1, "ORDER-REC", None, "");
        root.children = vec![
            field(5, "ORDER-ID", Some((0, 5)), "Order number"),
            field(5, "CUSTOMER", Some((5, 10)), "Customer name"),
        ];
        def.fields = vec![root];
        def.keys.primary.parts = vec![cobolt_indexed::KeyPartDef {
            field_name: "ORDER-ID".into(),
            offset: 0,
            length: 5,
            encoding: Default::default(),
        }];
        def
    }

    fn stored() -> IndexedFileInfo {
        IndexedFileInfo {
            record_format: crate::indexed::RecordFormat::Fixed { length: 15 },
            key_count: 1,
            total_key_length: 5,
            primary: KeyDescriptor {
                key_number: 1,
                name: Some("ORDER-ID".into()),
                parts: vec![KeyPart { offset: 0, length: 5, encoding: KeyEncoding::Bytes }],
                duplicates_allowed: false,
                ordering: KeyOrdering::Ascending,
            },
            alternates: Vec::new(),
        }
    }

    fn check(def: &cobolt_indexed::IndexedDefinition) -> Result<Checked, Refusal> {
        validate(&cobolt_indexed::save_indexed_to_string(def).unwrap(), &stored())
    }

    /// R3–R5 / AC2 — the .cidx gives the layout, and each way it can fail to
    /// describe the file has its own code.
    #[test]
    fn a_cidx_is_checked_against_the_file() {
        let ok = check(&orders()).unwrap();
        assert_eq!((ok.record_len, ok.primary.offset, ok.primary.len), (15, 0, 5));
        assert_eq!(ok.columns.iter().map(|c| (c.name.as_str(), c.offset, c.len)).collect::<Vec<_>>(),
            [("ORDER-ID", 0, 5), ("CUSTOMER", 5, 10)]);
        assert_eq!(ok.description.purpose, "Customer orders");

        let mut cases: Vec<(&str, cobolt_indexed::IndexedDefinition, &str)> = Vec::new();
        let mut d = orders();
        d.record_format = cobolt_indexed::RecordFormatDef::Fixed { length: 20 };
        cases.push(("record length 20", d, code::RECORD_LENGTH_MISMATCH));
        let mut d = orders();
        d.keys.primary.parts[0].offset = 5;
        d.keys.primary.parts[0].length = 10;
        cases.push(("primary key moved", d, code::KEY_MISMATCH));
        let mut d = orders();
        d.keys.alternates.push(cobolt_indexed::KeyDef {
            name: None,
            parts: vec![cobolt_indexed::KeyPartDef { field_name: "CUSTOMER".into(), offset: 5, length: 10, encoding: Default::default() }],
            duplicates_allowed: true,
            ordering: cobolt_indexed::KeyOrderingDef::Ascending,
        });
        cases.push(("an alternate the file lacks", d, code::KEY_MISMATCH));
        let mut d = orders();
        d.keys.primary.parts.clear();
        cases.push(("a key with no parts", d, code::KEY_MISMATCH));
        let mut d = orders();
        d.comment.clear();
        cases.push(("no purpose", d, code::NO_PURPOSE));
        let mut d = orders();
        for c in &mut d.fields[0].children {
            c.comment.clear();
        }
        cases.push(("no field descriptions", d, code::NO_FIELD_DESCRIPTIONS));
        let mut d = orders();
        d.fields.clear();
        cases.push(("no fields", d, code::NO_FIELDS));
        for (what, def, want) in &cases {
            assert_eq!(check(def).unwrap_err().code, *want, "{what}");
        }
        assert_eq!(validate("not xml at all", &stored()).unwrap_err().code, code::NO_FIELDS);
        println!("cidx checks: 1 accepted, {} refused with their codes + garbage → NO-FIELDS", cases.len());
    }

    /// R17–R20 — every branch of the fit test, each refusal with both numbers.
    #[test]
    fn the_fit_test() {
        const MB: u64 = 1024 * 1024;
        let src = |container, on_file_system, needs_copy| Source { container, on_file_system, needs_copy };
        let disk = src(Container::Disk, true, false);
        let mem = src(Container::Memory, true, false);
        let smb = src(Container::Disk, false, false);
        assert_eq!(decide(disk, 10 * MB, 64 * MB, 1024 * MB), Ok(Fit::Memory));
        assert_eq!(decide(disk, 100 * MB, 64 * MB, 1024 * MB), Ok(Fit::InPlace), "over the limit");
        assert_eq!(decide(disk, 10 * MB, 64 * MB, 16 * MB), Ok(Fit::InPlace), "over half the free memory");
        let r = decide(mem, 100 * MB, 64 * MB, 1024 * MB).unwrap_err();
        assert_eq!((r.code, r.size, r.bound), (code::TOO_LARGE_FOR_LIMIT, 100 * MB, 64 * MB));
        let r = decide(mem, 10 * MB, 64 * MB, 16 * MB).unwrap_err();
        assert_eq!((r.code, r.size, r.bound), (code::TOO_LARGE_FOR_FREE_MEMORY, 10 * MB, 8 * MB));
        let r = decide(smb, 100 * MB, 64 * MB, 1024 * MB).unwrap_err();
        assert_eq!(r.code, code::TOO_LARGE_FOR_LIMIT);
        assert!(r.message.contains("smb://"), "{}", r.message);
        let r = decide(src(Container::Disk, true, true), 100 * MB, 64 * MB, 1024 * MB).unwrap_err();
        assert_eq!(r.code, code::NEEDS_UPGRADE);
        assert_eq!(decide(smb, 64 * MB, 64 * MB, 128 * MB), Ok(Fit::Memory), "limits are inclusive");
    }
}
