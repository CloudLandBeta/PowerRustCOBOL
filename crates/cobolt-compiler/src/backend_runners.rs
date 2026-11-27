// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Backend runner of another system (spec 115 R1, R2, R7, R8, R13, R15).
//!
//! A Web project is packaged for the system it will run on. The Backend in the package is the application's payload
//! joined to that system's prebuilt runner (`prc-backend-runner`), and a package never carries a runner nobody verified:
//! the SHA-256 of every runner file is compared with `runners/manifest.txt`, the list the installer of this release
//! brought, **whatever its origin**: a folder the developer named, the cache, or a download. The list uses the format
//! of the platform libraries' own manifest (`warmup`): `<sha256>  <system>/<file>`.
//!
//! This module only finds, fetches and verifies. The system of the machine that packages never comes here: it keeps the
//! runner that ships beside the program ([`crate::backend_module::find_runner`]), as before.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::backend_module::{RunnerFiles, BACKEND_RUNNER_SBOM};
use crate::warmup::{parse_manifest, run, sha256_file, RELEASES_URL};

/// The environment variable that names a runners' list other than the installer's (a test, an installation that keeps it
/// elsewhere), as [`crate::backend_module::BACKEND_RUNNER_ENV`] names the runner.
pub const MANIFEST_ENV: &str = "PRC_RUNNERS_MANIFEST";
/// The environment variable that names the folder `--runner-dir` names (the flag wins).
pub const RUNNER_DIR_ENV: &str = "PRC_BACKEND_RUNNER_DIR";
/// The runner's file name, without the system's suffix; the manifest and the release attachments are named after it.
const RUNNER_STEM: &str = "prc-backend-runner";

/// An operating system a Backend runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Os {
    Linux,
    MacOs,
    Windows,
}

/// A processor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Arch {
    X86_64,
    Aarch64,
}

/// A system a package is for: the operating system, then the processor, spelled `linux-x86_64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct System {
    pub os: Os,
    pub arch: Arch,
}

impl fmt::Display for System {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let os = match self.os {
            Os::Linux => "linux",
            Os::MacOs => "macos",
            Os::Windows => "windows",
        };
        let arch = match self.arch {
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
        };
        write!(f, "{os}-{arch}")
    }
}

impl System {
    /// What the code can make a package for besides the machine's own system. Which of these the developer is offered
    /// is [`System::offered`]: only those the installer's list has a runner for.
    pub const TARGETS: [System; 2] = [System { os: Os::Linux, arch: Arch::X86_64 }, System { os: Os::Linux, arch: Arch::Aarch64 }];

    /// The system this code was compiled for; `None` on one the release has no runner for.
    pub fn host() -> Option<System> {
        let os = if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else if cfg!(target_os = "linux") {
            Os::Linux
        } else {
            return None;
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => Arch::X86_64,
            "aarch64" => Arch::Aarch64,
            _ => return None,
        };
        Some(System { os, arch })
    }

    /// `linux-x86_64`, as [`System`]'s `Display` writes it; one spelling only (lower case, a hyphen).
    pub fn parse(word: &str) -> Option<System> {
        let (os, arch) = word.split_once('-')?;
        let os = match os {
            "linux" => Os::Linux,
            "macos" => Os::MacOs,
            "windows" => Os::Windows,
            _ => return None,
        };
        let arch = match arch {
            "x86_64" => Arch::X86_64,
            "aarch64" => Arch::Aarch64,
            _ => return None,
        };
        Some(System { os, arch })
    }

    /// The suffix of a program's file name on this system.
    pub fn exe_suffix(self) -> &'static str {
        if self.os == Os::Windows {
            ".exe"
        } else {
            ""
        }
    }

    /// The runner's file name on this system.
    pub fn runner_name(self) -> String {
        format!("{RUNNER_STEM}{}", self.exe_suffix())
    }

    /// The systems a package can be made for here: the machine's own, then every [`System::TARGETS`] entry the list
    /// has a runner (and its bill of materials) for. The one list the IDE, `rcrun` and the tools read (R2).
    pub fn offered(manifest: &RunnerManifest) -> Vec<System> {
        let host = System::host();
        let mut out: Vec<System> = host.into_iter().collect();
        for target in System::TARGETS {
            if Some(target) != host && manifest.expected(target, RunnerFile::Runner).is_some() && manifest.expected(target, RunnerFile::Sbom).is_some() {
                out.push(target);
            }
        }
        out
    }

    /// The system a word names, if it is one of `offered`; otherwise the refusal that names them.
    pub fn choose(word: &str, offered: &[System]) -> Result<System, String> {
        match System::parse(word) {
            Some(system) if offered.contains(&system) => Ok(system),
            _ => Err(format!(
                "not a system this installation can package for: \"{word}\"; the systems are: {}",
                offered.iter().map(System::to_string).collect::<Vec<_>>().join(", ")
            )),
        }
    }
}

/// Which of a runner's two files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerFile {
    /// The runner program.
    Runner,
    /// Its bill of materials.
    Sbom,
}

impl RunnerFile {
    fn name(self, system: System) -> String {
        match self {
            RunnerFile::Runner => system.runner_name(),
            RunnerFile::Sbom => BACKEND_RUNNER_SBOM.to_string(),
        }
    }

    /// The release attachment: `prc-backend-runner-<version>-<system>` and the same with `.sbom.json`.
    pub fn asset_name(self, version: &str, system: System) -> String {
        match self {
            RunnerFile::Runner => format!("{RUNNER_STEM}-{version}-{system}{}", system.exe_suffix()),
            RunnerFile::Sbom => format!("{RUNNER_STEM}-{version}-{system}.sbom.json"),
        }
    }

    /// Where the release keeps the attachment, in the order they are tried: a release's tag is typed when it is
    /// published, and both spellings (`2.1.76` and `v2.1.76`) are in use (`warmup::libs_asset_urls` says why).
    pub fn urls(self, version: &str, system: System) -> [String; 2] {
        let name = self.asset_name(version, system);
        [format!("{RELEASES_URL}/{version}/{name}"), format!("{RELEASES_URL}/v{version}/{name}")]
    }
}

/// The list of runners of this release, with the SHA-256 of each file: what a runner is verified against (R7).
#[derive(Debug, Clone, Default)]
pub struct RunnerManifest {
    entries: Vec<(String, String)>,
    /// What was searched, for the message that says there is none.
    searched: String,
}

impl RunnerManifest {
    /// The list in `text`: `<sha256>  <system>/<file>` lines.
    pub fn parse(text: &str) -> Self {
        Self { entries: parse_manifest(text).into_iter().map(|(h, n)| (h.to_string(), n.trim().to_string())).collect(), searched: String::new() }
    }

    /// The installer's list: [`MANIFEST_ENV`], else `runners/manifest.txt` in the folder that holds the platform
    /// (the SDK beside the program, as `libs/` is). Empty when there is none: a checkout, or a release that carries none.
    pub fn find() -> Self {
        let path = std::env::var_os(MANIFEST_ENV).map(PathBuf::from).or_else(|| crate::resolve_workspace_root(None).map(|root| root.join("runners").join("manifest.txt")));
        let mut found = path.as_deref().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| Self::parse(&t)).unwrap_or_default();
        found.searched = match &path {
            Some(p) => p.display().to_string(),
            None => format!("beside the program, or in {MANIFEST_ENV}"),
        };
        found
    }

    /// The SHA-256 the list gives `file` of `system`, if it lists it.
    pub fn expected(&self, system: System, file: RunnerFile) -> Option<&str> {
        let name = format!("{system}/{}", file.name(system));
        self.entries.iter().find(|(_, n)| *n == name).map(|(h, _)| h.as_str())
    }

    /// Whether the list has anything: a checkout and an older release have none.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Why a runner was not given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerError {
    /// This installation carries no list of runners, so none can be verified.
    NoList { system: System, searched: String },
    /// The list does not name a runner for the system.
    NotListed { system: System },
    /// A file is not the one the release published.
    Differs { system: System, file: PathBuf, expected: String, found: String },
    /// There is no runner for the system on this machine and nothing was allowed to fetch it.
    Missing { system: System, searched: Vec<String>, expected: String },
    /// The download failed.
    Fetch { system: System, url: String, why: String },
    /// A file could not be read or written.
    Io { what: String },
}

impl fmt::Display for RunnerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunnerError::NoList { system, searched } => write!(
                f,
                "no runner is offered for {system}: this installation carries no list of runners to verify one against (looked at {searched}); a package for another system is made with the program the release installs"
            ),
            RunnerError::NotListed { system } => write!(f, "no runner is offered for {system}: this release's list of runners does not name one"),
            RunnerError::Differs { system, file, expected, found } => write!(
                f,
                "the runner file {} for {system} is not the one the release published (expected SHA-256 {expected}, found {found}); it was not used and nothing was written",
                file.display()
            ),
            RunnerError::Missing { system, searched, expected } => write!(
                f,
                "the runner for {system} is not on this machine (looked in {}); it is the release's file with SHA-256 {expected}: put it and its bill of materials in a folder and name it (`--runner-dir`, `runner_dir`), or allow the download (`--fetch-runner`, `fetch_runner`)",
                searched.join(", ")
            ),
            RunnerError::Fetch { system, url, why } => write!(f, "the runner for {system} could not be downloaded from {url}: {why}; download it by hand and name its folder (`--runner-dir`, `runner_dir`)"),
            RunnerError::Io { what } => write!(f, "{what}"),
        }
    }
}

impl std::error::Error for RunnerError {}

/// How a file is got from an address. A trait so a test substitutes it; [`CurlFetch`] is the real one.
pub trait Fetch {
    /// Download `url` into the file `to`.
    fn get(&self, url: &str, to: &Path) -> Result<(), String>;
}

/// The download `warmup::repair_libs` does: `curl`, https only, TLS 1.2 or newer, failing on an HTTP error.
pub struct CurlFetch;

/// The arguments of the download: the two the platform libraries' own download uses, `--proto =https --tlsv1.2`, and the
/// ones that make an HTTP error an error.
pub(crate) fn curl_args(url: &str, to: &Path) -> Vec<String> {
    ["--fail", "--location", "--silent", "--show-error", "--proto", "=https", "--tlsv1.2", "-o"].iter().map(|s| s.to_string()).chain([to.to_string_lossy().into_owned(), url.to_string()]).collect()
}

impl Fetch for CurlFetch {
    fn get(&self, url: &str, to: &Path) -> Result<(), String> {
        let args = curl_args(url, to);
        run("curl", &args.iter().map(String::as_str).collect::<Vec<_>>())
    }
}

/// Where a runner may come from.
pub struct RunnerSource<'a> {
    /// The release these programs belong to: names the cache and the attachment.
    pub version: &'a str,
    /// A folder the developer named: the only place looked in when given (`<dir>/<system>/` with the manifest's own names,
    /// or the release's attachments as downloaded, `prc-backend-runner-<version>-<system>`).
    pub runner_dir: Option<&'a Path>,
    /// The build root: the cache is `<cache_root>/runners/<version>/<system>/`.
    pub cache_root: &'a Path,
    /// The download, when the caller allowed it. Never set by a caller that was not asked to (R8).
    pub fetch: Option<&'a dyn Fetch>,
}

impl RunnerSource<'_> {
    fn cache_dir(&self, system: System) -> PathBuf {
        self.cache_root.join("runners").join(self.version).join(system.to_string())
    }
}

enum Check {
    Absent,
    Ok,
    Differs(String),
}

fn check(path: &Path, expected: &str) -> Result<Check, RunnerError> {
    if !path.is_file() {
        return Ok(Check::Absent);
    }
    let found = sha256_file(path).map_err(|e| RunnerError::Io { what: format!("cannot read {}: {e}", path.display()) })?;
    Ok(if found == expected { Check::Ok } else { Check::Differs(found) })
}

fn io<T>(what: &str, path: &Path, r: std::io::Result<T>) -> Result<T, RunnerError> {
    r.map_err(|e| RunnerError::Io { what: format!("cannot {what} {}: {e}", path.display()) })
}

/// A file of the runner, verified, in the cache. `fill` writes the candidate into the `.part` file; it is compared with
/// the list **before** it takes its place, and a wrong one leaves nothing behind.
fn settle(system: System, shown: &Path, dest: &Path, expected: &str, fill: impl FnOnce(&Path) -> Result<(), RunnerError>) -> Result<(), RunnerError> {
    let part = dest.with_file_name(format!(".{}.part", dest.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()));
    if let Some(dir) = dest.parent() {
        io("create", dir, std::fs::create_dir_all(dir))?;
    }
    let result = fill(&part).and_then(|_| match check(&part, expected)? {
        Check::Ok => io("place", dest, std::fs::rename(&part, dest)),
        Check::Differs(found) => Err(RunnerError::Differs { system, file: shown.to_path_buf(), expected: expected.to_string(), found }),
        Check::Absent => Err(RunnerError::Io { what: format!("{} was not written", part.display()) }),
    });
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

/// The two files of `system`'s runner, found in the folder the developer named.
fn in_dir(dir: &Path, version: &str, system: System) -> Option<(PathBuf, PathBuf)> {
    let nested = dir.join(system.to_string());
    let found = (nested.join(RunnerFile::Runner.name(system)), nested.join(RunnerFile::Sbom.name(system)));
    let release = (dir.join(RunnerFile::Runner.asset_name(version, system)), dir.join(RunnerFile::Sbom.asset_name(version, system)));
    [found, release].into_iter().find(|(r, s)| r.is_file() && s.is_file())
}

/// The runner and its bill of materials for `system`, an operating system or a processor other than this machine's: in
/// the cache, verified. The order is the folder the developer named (the only place then), the cache, and, only when the
/// caller was allowed to, the download. **Every** file is compared with `manifest` before it is used (R7): one the
/// release did not publish is refused, naming the hash expected and the one found, and nothing is left behind.
pub fn runner_for(system: System, manifest: &RunnerManifest, source: &RunnerSource) -> Result<RunnerFiles, RunnerError> {
    if manifest.is_empty() {
        return Err(RunnerError::NoList { system, searched: manifest.searched.clone() });
    }
    let want = |file| manifest.expected(system, file).map(str::to_string).ok_or(RunnerError::NotListed { system });
    let (want_runner, want_sbom) = (want(RunnerFile::Runner)?, want(RunnerFile::Sbom)?);
    let dir = source.cache_dir(system);
    let files = RunnerFiles { runner: dir.join(RunnerFile::Runner.name(system)), sbom: dir.join(RunnerFile::Sbom.name(system)) };
    let wanted = [(&files.runner, want_runner.as_str(), RunnerFile::Runner), (&files.sbom, want_sbom.as_str(), RunnerFile::Sbom)];

    if let Some(named) = source.runner_dir {
        let (runner, sbom) = in_dir(named, source.version, system).ok_or_else(|| RunnerError::Missing {
            system,
            searched: vec![named.join(system.to_string()).display().to_string(), named.display().to_string()],
            expected: want_runner.clone(),
        })?;
        for ((dest, expected, _), from) in wanted.iter().zip([&runner, &sbom]) {
            settle(system, from, dest, expected, |part| io("copy", from, std::fs::copy(from, part)).map(|_| ()))?;
        }
        return Ok(files);
    }

    // The cache is not trusted either: a file that does not verify is replaced by a download, or refused.
    let mut differs = None;
    let mut all_ok = true;
    for (dest, expected, _) in &wanted {
        match check(dest, expected)? {
            Check::Ok => {}
            Check::Absent => all_ok = false,
            Check::Differs(found) => {
                all_ok = false;
                differs.get_or_insert(RunnerError::Differs { system, file: (*dest).clone(), expected: expected.to_string(), found });
            }
        }
    }
    if all_ok {
        return Ok(files);
    }
    let Some(fetch) = source.fetch else {
        return Err(differs.unwrap_or(RunnerError::Missing { system, searched: vec![dir.display().to_string()], expected: want_runner }));
    };
    for (dest, expected, file) in &wanted {
        if matches!(check(dest, expected)?, Check::Ok) {
            continue;
        }
        let urls = file.urls(source.version, system);
        settle(system, dest, dest, expected, |part| {
            let first = fetch.get(&urls[0], part);
            match first {
                Ok(()) => Ok(()),
                Err(_) => fetch.get(&urls[1], part).map_err(|why| RunnerError::Fetch { system, url: urls[1].clone(), why }),
            }
        })?;
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::time::Instant;

    const LINUX: System = System { os: Os::Linux, arch: Arch::X86_64 };
    const ARM: System = System { os: Os::Linux, arch: Arch::Aarch64 };

    /// The result block every test of this module ends with: the cases it ran, their times and the tally.
    struct Block {
        name: &'static str,
        cases: Vec<(String, bool, f64)>,
        started: Instant,
    }

    impl Block {
        fn new(name: &'static str) -> Self {
            Self { name, cases: Vec::new(), started: Instant::now() }
        }

        fn case(&mut self, what: &str, ok: bool, started: Instant) {
            self.cases.push((what.to_string(), ok, started.elapsed().as_secs_f64() * 1000.0));
            assert!(ok, "{}: case failed: {what}", self.name);
        }

        fn print(self) {
            let passed = self.cases.iter().filter(|c| c.1).count();
            println!("\n== {} ==", self.name);
            for (what, ok, ms) in &self.cases {
                println!("  {} {what:<78} {ms:>8.2} ms", if *ok { "ok  " } else { "FAIL" });
            }
            println!("  {} cases, {passed} passed, {} failed, {:.1} ms", self.cases.len(), self.cases.len() - passed, self.started.elapsed().as_secs_f64() * 1000.0);
        }
    }

    /// A stand-in runner: the ELF magic and some bytes, which is a format `embed` accepts, never a program that runs.
    fn stand_in(seed: u8, len: usize) -> Vec<u8> {
        let mut bytes = vec![0x7f, b'E', b'L', b'F'];
        bytes.extend((0..len).map(|i| seed.wrapping_add(i as u8)));
        bytes
    }

    fn hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("prc-runners-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The list a release publishes for `systems`, from the runner bytes and the bill of materials text.
    fn list(systems: &[System], runner: &[u8], sbom: &[u8]) -> RunnerManifest {
        let text: String = systems.iter().map(|s| format!("{}  {s}/{}\n{}  {s}/{BACKEND_RUNNER_SBOM}\n", hex(runner), s.runner_name(), hex(sbom))).collect();
        RunnerManifest::parse(&text)
    }

    /// A fetcher that answers from a table and counts what it was asked.
    struct Table {
        files: HashMap<String, Vec<u8>>,
        asked: RefCell<Vec<String>>,
    }

    impl Fetch for Table {
        fn get(&self, url: &str, to: &Path) -> Result<(), String> {
            self.asked.borrow_mut().push(url.to_string());
            match self.files.get(url) {
                Some(bytes) => std::fs::write(to, bytes).map_err(|e| e.to_string()),
                None => Err("404".to_string()),
            }
        }
    }

    #[test]
    fn the_one_list_of_systems() {
        let mut b = Block::new("System: the one list of systems (R1, R2, AC8)");
        let t = Instant::now();
        let spellings = [(LINUX, "linux-x86_64"), (ARM, "linux-aarch64"), (System { os: Os::MacOs, arch: Arch::Aarch64 }, "macos-aarch64"), (System { os: Os::Windows, arch: Arch::X86_64 }, "windows-x86_64")];
        b.case("every spelling is written and read back", spellings.iter().all(|(s, w)| s.to_string() == *w && System::parse(w) == Some(*s)), t);
        let t = Instant::now();
        b.case("one spelling only: upper case, an underscore and a missing processor are not systems", ["Linux-x86_64", "linux_x86_64", "linux-X86_64", "linux", "x86_64-linux", "", "linux-"].iter().all(|w| System::parse(w).is_none()), t);
        let t = Instant::now();
        b.case("the file names: .exe for Windows only", System { os: Os::Windows, arch: Arch::X86_64 }.runner_name() == "prc-backend-runner.exe" && LINUX.runner_name() == "prc-backend-runner", t);

        let t = Instant::now();
        let none = System::offered(&RunnerManifest::default());
        b.case("with no list only this machine's system is offered", none == System::host().into_iter().collect::<Vec<_>>(), t);
        let t = Instant::now();
        let with = System::offered(&list(&[LINUX], &stand_in(1, 8), b"{}"));
        b.case("the host is first, and linux-x86_64 once the list has a runner for it", with.first().copied() == System::host() && with.contains(&LINUX), t);
        let t = Instant::now();
        let no_arm = !with.contains(&ARM) && with.iter().collect::<std::collections::HashSet<_>>().len() == with.len();
        b.case("a system the list does not name is not offered; none twice", no_arm, t);
        let t = Instant::now();
        let half = RunnerManifest::parse(&format!("{}  linux-x86_64/prc-backend-runner\n", hex(b"x")));
        b.case("a runner without its bill of materials is not offered", !System::offered(&half).contains(&LINUX) || System::host() == Some(LINUX), t);

        let t = Instant::now();
        let refusal = System::choose("bogus", &with).unwrap_err();
        b.case("a word outside the list is refused naming the list", refusal.contains("\"bogus\"") && with.iter().all(|s| refusal.contains(&s.to_string())), t);
        let t = Instant::now();
        let ok = System::choose("linux-x86_64", &with) == Ok(LINUX);
        b.case("a word in the list is chosen", ok, t);
        let t = Instant::now();
        b.case("a spelling that is a system but not offered is refused the same way", System::choose("linux-aarch64", &with).is_err(), t);
        b.print();
    }

    #[test]
    fn a_runner_is_verified_against_the_installers_list() {
        let mut b = Block::new("Runner verification: expected and found, never a file nobody verified (R7, AC6)");
        let (runner, sbom) = (stand_in(3, 4096), br#"{"components":[]}"#.to_vec());
        let manifest = list(&[LINUX], &runner, &sbom);
        let base = scratch("verify");
        let (dir, cache) = (base.join("given"), base.join("cache"));
        std::fs::create_dir_all(dir.join("linux-x86_64")).unwrap();
        std::fs::write(dir.join("linux-x86_64").join("prc-backend-runner"), &runner).unwrap();
        std::fs::write(dir.join("linux-x86_64").join(BACKEND_RUNNER_SBOM), &sbom).unwrap();
        fn source<'a>(runner_dir: Option<&'a Path>, cache_root: &'a Path) -> RunnerSource<'a> {
            RunnerSource { version: "9.9.9", runner_dir, cache_root, fetch: None }
        }
        let given: &Path = &dir;

        let t = Instant::now();
        let got = runner_for(LINUX, &manifest, &source(Some(given), &cache)).unwrap();
        b.case("a good file in the folder verifies and is placed in the cache", std::fs::read(&got.runner).unwrap() == runner && std::fs::read(&got.sbom).unwrap() == sbom && got.sbom == got.runner.with_file_name(BACKEND_RUNNER_SBOM), t);

        let t = Instant::now();
        let mut bad = runner.clone();
        bad[100] ^= 1;
        std::fs::write(dir.join("linux-x86_64").join("prc-backend-runner"), &bad).unwrap();
        let _ = std::fs::remove_dir_all(&cache);
        let err = runner_for(LINUX, &manifest, &source(Some(given), &cache)).unwrap_err();
        let named =matches!(&err, RunnerError::Differs { expected, found, .. } if *expected == hex(&runner) && *found == hex(&bad));
        b.case("one byte changed: refused, and the message names the expected and the found hash", named && err.to_string().contains(&hex(&runner)) && err.to_string().contains(&hex(&bad)), t);
        let t = Instant::now();
        let left: Vec<_> = std::fs::read_dir(cache.join("runners/9.9.9/linux-x86_64")).map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
        b.case("the refused file left nothing in the cache, not even a .part", !left.iter().any(|n| n == "prc-backend-runner" || n.ends_with(".part")), t);

        let t = Instant::now();
        let empty = runner_for(LINUX, &RunnerManifest::default(), &source(Some(given), &cache)).unwrap_err();
        b.case("no list at all: no runner is offered, and the message says why", matches!(empty, RunnerError::NoList { .. }) && empty.to_string().contains("no list of runners"), t);
        let t = Instant::now();
        let other = runner_for(ARM, &manifest, &source(Some(given), &cache)).unwrap_err();
        b.case("a system the list does not name is not served from another system's entry", matches!(other, RunnerError::NotListed { system } if system == ARM), t);
        let t = Instant::now();
        let nowhere = base.join("nowhere");
        let none = runner_for(LINUX, &manifest, &source(Some(&nowhere), &cache)).unwrap_err();
        b.case("a folder without the runner is refused naming where it looked", matches!(&none, RunnerError::Missing { searched, .. } if searched.iter().any(|s| s.contains("nowhere"))), t);
        let _ = std::fs::remove_dir_all(&base);
        b.print();
    }

    #[test]
    fn where_a_runner_comes_from() {
        let mut b = Block::new("Runner source: folder, cache, download (R7, R8, R15)");
        let (runner, sbom) = (stand_in(5, 2048), br#"{"components":[1]}"#.to_vec());
        let manifest = list(&[LINUX], &runner, &sbom);
        let base = scratch("source");
        let urls = RunnerFile::Runner.urls("9.9.9", LINUX);
        let sbom_urls = RunnerFile::Sbom.urls("9.9.9", LINUX);
        let table = |runner: &[u8]| Table { files: HashMap::from([(urls[1].clone(), runner.to_vec()), (sbom_urls[1].clone(), sbom.clone())]), asked: RefCell::new(Vec::new()) };
        let src = |cache: &Path, fetch: Option<&dyn Fetch>| -> RunnerFiles {
            runner_for(LINUX, &manifest, &RunnerSource { version: "9.9.9", runner_dir: None, cache_root: cache, fetch }).unwrap()
        };
        let cache = base.join("cache");

        let t = Instant::now();
        let none = runner_for(LINUX, &manifest, &RunnerSource { version: "9.9.9", runner_dir: None, cache_root: &cache, fetch: None }).unwrap_err();
        b.case("nothing on the machine and no permission to download: refused naming the flag and the hash", matches!(&none, RunnerError::Missing { .. }) && none.to_string().contains("--fetch-runner") && none.to_string().contains("--runner-dir") && none.to_string().contains(&hex(&runner)), t);

        let t = Instant::now();
        let fetcher = table(&runner);
        let got = src(&cache, Some(&fetcher));
        b.case("allowed to download: both files verified and placed in the cache", std::fs::read(&got.runner).unwrap() == runner && std::fs::read(&got.sbom).unwrap() == sbom, t);
        let t = Instant::now();
        b.case("each file is asked for at the bare tag first, then at the v tag", *fetcher.asked.borrow() == [urls[0].clone(), urls[1].clone(), sbom_urls[0].clone(), sbom_urls[1].clone()], t);

        let t = Instant::now();
        let before = fetcher.asked.borrow().len();
        let again = src(&cache, Some(&fetcher));
        b.case("a verified cache is used and the download is not called", fetcher.asked.borrow().len() == before && again.runner == got.runner, t);

        let t = Instant::now();
        let mut tampered = runner.clone();
        tampered[7] ^= 0xff;
        std::fs::write(&got.runner, &tampered).unwrap();
        let refused = runner_for(LINUX, &manifest, &RunnerSource { version: "9.9.9", runner_dir: None, cache_root: &cache, fetch: None }).unwrap_err();
        b.case("an altered file in the cache is refused (the cache is not trusted)", matches!(&refused, RunnerError::Differs { found, .. } if *found == hex(&tampered)), t);
        let t = Instant::now();
        let healed = src(&cache, Some(&fetcher));
        b.case("with the download allowed, an altered cache file is replaced by the release's", std::fs::read(&healed.runner).unwrap() == runner, t);

        let t = Instant::now();
        let other_cache = base.join("cache2");
        let wrong = table(&stand_in(9, 2048));
        let err = runner_for(LINUX, &manifest, &RunnerSource { version: "9.9.9", runner_dir: None, cache_root: &other_cache, fetch: Some(&wrong) }).unwrap_err();
        let dir = other_cache.join("runners/9.9.9/linux-x86_64");
        let leftovers = std::fs::read_dir(&dir).map(|d| d.flatten().count()).unwrap_or(0);
        b.case("a download that is not the release's file is refused and leaves no file, no .part", matches!(err, RunnerError::Differs { .. }) && leftovers == 0, t);

        let t = Instant::now();
        let down = Table { files: HashMap::new(), asked: RefCell::new(Vec::new()) };
        let err = runner_for(LINUX, &manifest, &RunnerSource { version: "9.9.9", runner_dir: None, cache_root: &base.join("cache3"), fetch: Some(&down) }).unwrap_err();
        b.case("a download that fails names the address, and the way round it", matches!(&err, RunnerError::Fetch { url, .. } if url.contains("/v9.9.9/")) && err.to_string().contains("--runner-dir") && down.asked.borrow().len() == 2, t);
        let _ = std::fs::remove_dir_all(&base);
        b.print();
    }

    #[test]
    fn the_release_attachments_are_named_for_the_version_and_the_system() {
        let mut b = Block::new("Release attachments and the download's arguments (R8, R13, R15)");
        let t = Instant::now();
        b.case("the runner and its bill of materials are named after the version and the system", RunnerFile::Runner.asset_name("2.1.76", LINUX) == "prc-backend-runner-2.1.76-linux-x86_64" && RunnerFile::Sbom.asset_name("2.1.76", LINUX) == "prc-backend-runner-2.1.76-linux-x86_64.sbom.json", t);
        let t = Instant::now();
        let urls = RunnerFile::Runner.urls("2.1.76", LINUX);
        b.case("both tag spellings are tried, the bare one first, over https", urls[0].starts_with("https://github.com/") && urls[0].contains("/releases/download/2.1.76/") && urls[1].contains("/releases/download/v2.1.76/"), t);
        let t = Instant::now();
        let args = curl_args("https://example.invalid/x", Path::new("/tmp/x.part"));
        let has = |a: &str| args.iter().any(|x| x == a);
        let proto = args.windows(2).any(|w| w[0] == "--proto" && w[1] == "=https");
        b.case("curl is told https only, TLS 1.2, to fail on an HTTP error and to follow redirects", proto && has("--tlsv1.2") && has("--fail") && has("--location"), t);
        let t = Instant::now();
        let dir = scratch("curl");
        let part = dir.join("never.part");
        let unreachable = CurlFetch.get("https://127.0.0.1:1/prc-backend-runner", &part);
        b.case("an unreachable address is an error, not a file", unreachable.is_err() && !part.exists(), t);
        let _ = std::fs::remove_dir_all(&dir);
        b.print();
    }
}
