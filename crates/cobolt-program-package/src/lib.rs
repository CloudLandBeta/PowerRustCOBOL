// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The payload inside a built Desktop program (DESKTOP-WASM-001 T14, plan §2 and §4.2).
//!
//! A program built on the WebAssembly path is **one file**: the prebuilt runner's executable with
//! the program inside it. This crate defines what goes inside and how it is put there.
//!
//! * **The payload** ([`encode`], [`decode`]): a table of contents of `{kind, offset, len, sha256}`
//!   entries ([`Kind`]: the module, its machine code, the engine that made the machine code), then
//!   the parts. The table of contents carries its own sha256, so a damaged one is refused before
//!   any part is read; each part is checked against its sha256 when it is taken.
//! * **The carriers** ([`embed`], [`locate`]):
//!   * on **ELF** and **PE**, an overlay after the image, found through the `PRLOC` trailer at the
//!     very end of the file;
//!   * on **Mach-O**, a `__PRPAYLOAD` segment inserted before `__LINKEDIT`, and the file signed
//!     again ad hoc, in pure Rust (an overlay would break the signature).
//! * **[`RunnerSettings`]**: what the Cargo path bakes into `main.rs`, as data, made by the one
//!   function that also feeds the Cargo path (T28).
//! * **[`DesktopSections`]** (T28, C1): the settings, the forms as code-free descriptions, the
//!   menus and the theme art, as custom sections of the program's module.

pub mod backend;
mod codesign;
mod macho;
mod overlay;
pub mod sections;
mod settings;

use std::fmt;
use std::path::Path;

use sha2::{Digest, Sha256};

pub use backend::BackendContainer;
pub use sections::{
    DesktopSections, MenuSource, SectionError, ThemeArt, ART_SECTION, FORMS_SECTION, MENUS_SECTION, SETTINGS_SECTION,
};
pub use settings::{FocusRing, RunnerSettings};

/// The payload's magic, at its first byte.
pub const PAYLOAD_MAGIC: [u8; 8] = *b"PRPAYLD\0";
/// The payload format this crate writes. A reader accepts every version up to its own.
pub const PAYLOAD_VERSION: u32 = 1;
const HEADER_LEN: usize = 56;
const ENTRY_LEN: usize = 56;
/// More parts than this is a damaged table of contents, not a program.
const MAX_PARTS: usize = 1024;

/// What a part of the payload is. A kind this reader does not know is kept and passed by, so a
/// newer Build's payload still loads in an older runner as far as that runner can go.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// The program's WebAssembly module.
    Module,
    /// wasmtime's machine code for the module (`cobolt_runner_engine::precompile`).
    MachineCode,
    /// The compatibility hash of the engine that made the machine code (32 bytes).
    MachineCodeEngine,
    /// A built Backend (spec 114 R3): its [`BackendContainer`], interface, exports and module. Its machine code is
    /// kinds 2 and 3, as a Desktop program's is. A runner that predates it reads the code 4 as `Other(4)` and passes it by.
    Backend,
    /// A kind added after this reader.
    Other(u32),
}

impl Kind {
    pub fn code(self) -> u32 {
        match self {
            Kind::Module => 1,
            Kind::MachineCode => 2,
            Kind::MachineCodeEngine => 3,
            Kind::Backend => 4,
            Kind::Other(n) => n,
        }
    }

    pub fn from_code(n: u32) -> Kind {
        match n {
            1 => Kind::Module,
            2 => Kind::MachineCode,
            3 => Kind::MachineCodeEngine,
            4 => Kind::Backend,
            n => Kind::Other(n),
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Module => f.write_str("MODULE"),
            Kind::MachineCode => f.write_str("MACHINE_CODE"),
            Kind::MachineCodeEngine => f.write_str("MACHINE_CODE_ENGINE"),
            Kind::Backend => f.write_str("BACKEND"),
            Kind::Other(n) => write!(f, "kind {n}"),
        }
    }
}

/// Why a payload could not be put in or taken out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackageError {
    /// The file is not an executable this crate carries a payload in.
    UnsupportedFormat(String),
    /// The executable carries no payload: a bare runner, or one whose payload was stripped.
    NoPayload,
    /// The executable already carries a payload; a Build starts from the bare runner.
    AlreadyPackaged,
    /// The locator, the segment or the table of contents does not hold together.
    Damaged(String),
    /// A part's bytes do not match the sha256 its table of contents recorded.
    PartDamaged(Kind),
    /// The payload was written by a newer Build than this reader.
    NewerFormat(u32),
    /// A Mach-O runner was linked without room for one more load command.
    NoHeaderRoom { free: usize, needed: usize },
    /// The program does not fit the format's 32-bit file offsets.
    TooLarge,
}

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageError::UnsupportedFormat(why) => write!(f, "not an executable that can carry a program: {why}"),
            PackageError::NoPayload => f.write_str("the executable carries no program (PRLOC payload missing)"),
            PackageError::AlreadyPackaged => f.write_str("the executable already carries a program"),
            PackageError::Damaged(why) => write!(f, "the program inside the executable is damaged: {why}"),
            PackageError::PartDamaged(kind) => write!(f, "the program's {kind} does not match its checksum"),
            PackageError::NewerFormat(v) => {
                write!(f, "the program was built by a newer PowerRustCOBOL (payload format {v}, this runner reads up to {PAYLOAD_VERSION})")
            }
            PackageError::NoHeaderRoom { free, needed } => write!(
                f,
                "the runner's Mach-O header has {free} free bytes and the payload segment needs {needed}; link the runner with -Wl,-headerpad,0x1000"
            ),
            PackageError::TooLarge => f.write_str("the program is too large for the executable format"),
        }
    }
}

impl std::error::Error for PackageError {}

/// The sha256 of `bytes`.
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// One part of a decoded payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part<'a> {
    pub kind: Kind,
    pub bytes: &'a [u8],
    /// The sha256 the Build recorded for `bytes`.
    pub sha256: [u8; 32],
}

impl<'a> Part<'a> {
    /// The bytes, when they match their recorded sha256.
    pub fn verified(&self) -> Result<&'a [u8], PackageError> {
        if sha256(self.bytes) == self.sha256 { Ok(self.bytes) } else { Err(PackageError::PartDamaged(self.kind)) }
    }
}

/// A decoded payload: its format version and its parts, in table-of-contents order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Payload<'a> {
    pub version: u32,
    pub parts: Vec<Part<'a>>,
}

impl<'a> Payload<'a> {
    /// The first part of `kind`, unchecked.
    pub fn part(&self, kind: Kind) -> Option<Part<'a>> {
        self.parts.iter().copied().find(|p| p.kind == kind)
    }

    /// The module, checked: a program whose module is damaged cannot run.
    pub fn module(&self) -> Result<&'a [u8], PackageError> {
        self.part(Kind::Module).ok_or_else(|| PackageError::Damaged("no MODULE part".into()))?.verified()
    }

    /// The Backend, checked and read: a payload that carries none is a Desktop program, not a Backend.
    pub fn backend(&self) -> Result<BackendContainer, PackageError> {
        let part = self.part(Kind::Backend).ok_or_else(|| PackageError::Damaged("no BACKEND part".into()))?;
        BackendContainer::decode(part.verified()?)
    }
}

/// The payload for `parts`, in that order: the header, the table of contents, then each part at a
/// 16-byte boundary.
pub fn encode(parts: &[(Kind, &[u8])]) -> Vec<u8> {
    let toc_end = HEADER_LEN + ENTRY_LEN * parts.len();
    let mut offsets = Vec::with_capacity(parts.len());
    let mut at = align16(toc_end);
    for (_, bytes) in parts {
        offsets.push(at);
        at = align16(at + bytes.len());
    }
    let total = offsets.last().zip(parts.last()).map_or(toc_end, |(o, (_, b))| o + b.len());
    let mut out = vec![0u8; total];
    out[..8].copy_from_slice(&PAYLOAD_MAGIC);
    out[8..12].copy_from_slice(&PAYLOAD_VERSION.to_le_bytes());
    out[12..16].copy_from_slice(&(parts.len() as u32).to_le_bytes());
    out[16..24].copy_from_slice(&(total as u64).to_le_bytes());
    for (i, ((kind, bytes), off)) in parts.iter().zip(&offsets).enumerate() {
        let e = HEADER_LEN + i * ENTRY_LEN;
        out[e..e + 4].copy_from_slice(&kind.code().to_le_bytes());
        out[e + 8..e + 16].copy_from_slice(&(*off as u64).to_le_bytes());
        out[e + 16..e + 24].copy_from_slice(&(bytes.len() as u64).to_le_bytes());
        out[e + 24..e + 56].copy_from_slice(&sha256(bytes));
        out[*off..*off + bytes.len()].copy_from_slice(bytes);
    }
    let toc = toc_hash(&out, toc_end);
    out[24..56].copy_from_slice(&toc);
    out
}

/// The table of contents' own sha256: the header's first 24 bytes and every entry.
fn toc_hash(payload: &[u8], toc_end: usize) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(&payload[..24]);
    h.update(&payload[HEADER_LEN..toc_end]);
    h.finalize().into()
}

/// Read a payload's table of contents. Refused: a wrong magic, a newer format, a table of contents
/// whose sha256 does not hold, a part outside the payload. Each part is checked when it is taken.
pub fn decode(payload: &[u8]) -> Result<Payload<'_>, PackageError> {
    let damaged = |why: &str| PackageError::Damaged(why.to_string());
    if payload.len() < HEADER_LEN || payload[..8] != PAYLOAD_MAGIC {
        return Err(damaged("no payload header"));
    }
    let version = u32::from_le_bytes(payload[8..12].try_into().unwrap());
    let count = u32::from_le_bytes(payload[12..16].try_into().unwrap()) as usize;
    let total = u64::from_le_bytes(payload[16..24].try_into().unwrap());
    if count > MAX_PARTS {
        return Err(damaged("the table of contents lists too many parts"));
    }
    let toc_end = HEADER_LEN + ENTRY_LEN * count;
    if toc_end > payload.len() || total != payload.len() as u64 {
        return Err(damaged("the table of contents does not fit the payload"));
    }
    if toc_hash(payload, toc_end)[..] != payload[24..56] {
        return Err(damaged("the table of contents does not match its checksum"));
    }
    if version > PAYLOAD_VERSION {
        return Err(PackageError::NewerFormat(version));
    }
    let mut parts = Vec::with_capacity(count);
    for i in 0..count {
        let e = HEADER_LEN + i * ENTRY_LEN;
        let kind = Kind::from_code(u32::from_le_bytes(payload[e..e + 4].try_into().unwrap()));
        let off = u64::from_le_bytes(payload[e + 8..e + 16].try_into().unwrap());
        let len = u64::from_le_bytes(payload[e + 16..e + 24].try_into().unwrap());
        let end = off.checked_add(len).ok_or_else(|| damaged("a part's length overflows"))?;
        if off < toc_end as u64 || end > payload.len() as u64 {
            return Err(damaged("a part lies outside the payload"));
        }
        let sha256: [u8; 32] = payload[e + 24..e + 56].try_into().unwrap();
        parts.push(Part { kind, bytes: &payload[off as usize..end as usize], sha256 });
    }
    Ok(Payload { version, parts })
}

fn align16(n: usize) -> usize {
    (n + 15) & !15
}

/// The executable formats a payload travels in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Elf,
    Pe,
    MachO,
}

/// Which format `exe` is, by its first bytes.
pub fn format_of(exe: &[u8]) -> Result<Format, PackageError> {
    if exe.len() >= 4 && exe[..4] == [0x7f, b'E', b'L', b'F'] {
        return Ok(Format::Elf);
    }
    if exe.len() >= 4 && exe[..4] == [0xcf, 0xfa, 0xed, 0xfe] {
        return Ok(Format::MachO);
    }
    if exe.len() >= 4 && (exe[..4] == [0xca, 0xfe, 0xba, 0xbe] || exe[..4] == [0xbe, 0xba, 0xfe, 0xca]) {
        return Err(PackageError::UnsupportedFormat("a universal (fat) Mach-O; each architecture has its own runner".into()));
    }
    if exe.len() >= 0x40 && exe[..2] == *b"MZ" {
        let pe = u32::from_le_bytes(exe[0x3c..0x40].try_into().unwrap()) as usize;
        if exe.get(pe..pe + 4) == Some(b"PE\0\0") {
            return Ok(Format::Pe);
        }
    }
    Err(PackageError::UnsupportedFormat("neither ELF, PE nor 64-bit Mach-O".into()))
}

/// `runner` with `payload` inside it, on its format's carrier. `identifier` names the program in a
/// Mach-O code signature (the built file's name); the other formats do not use it.
pub fn embed(runner: &[u8], payload: &[u8], identifier: &str) -> Result<Vec<u8>, PackageError> {
    match format_of(runner)? {
        Format::Elf | Format::Pe => overlay::embed(runner, payload),
        Format::MachO => macho::embed(runner, payload, identifier),
    }
}

/// The payload inside `exe`.
pub fn locate(exe: &[u8]) -> Result<&[u8], PackageError> {
    match format_of(exe)? {
        Format::Elf | Format::Pe => overlay::locate(exe),
        Format::MachO => macho::locate(exe),
    }
}

/// Write an executable whole: a new file beside `path`, renamed into place, executable on Unix. A
/// new file rather than one rewritten in place, so macOS checks its new signature afresh.
pub fn write_executable(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}
