// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Packaging a Web project to install** (spec 113): one function, [`package_web`], that the IDE, `rcrun package` and the
//! MCP tool all call. It checks the project, builds the page and the Backend through the paths the Build already has, makes
//! the production `backend.toml`, runs the new Backend with `--check` against it, and writes **exactly two files** into the
//! folder it is given: `<name>.zip` (what the running application reads) and the install script of the system it runs on
//! (`<name>-install.sh`, or `<name>-install.ps1` on Windows).
//!
//! ```text
//! <name>.zip
//!   web/                the bundle, without hosting/ (the deployment kit has no place on a server that is installed)
//!   bin/<exe>           the Backend, and bin/sbom.cdx.json
//!   backend.toml        production: made from backend/backend.toml of the project (see [`production_config`])
//!   data/               the project's data/, only a seed for a first install
//!   assets/KB/          the project's Application Knowledge Base, when it has one
//!   LICENSE  NOTICE  NOTICE-RUNTIME.txt
//! ```
//!
//! Nothing is written into the chosen folder until everything else has succeeded: a refusal or a failure leaves it as it was.
//! The text of a refusal is English, like every report of the tools; the IDE says the rest in the developer's language.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use cobolt_compiler::project_kind::ProjectKind;
use cobolt_web_bundle::install::{BackendFacts, Install, Lang, TargetOs};
use toml_edit::{Array, DocumentMut, Item, Value};

use super::{BuildProfile, WebBuildError, WebBuildRequest};

/// One step of the packaging, for the progress line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// The project's settings, checked before anything is built or written.
    Check,
    /// The complete Web Build (the page).
    Page,
    /// The Backend's executable.
    Backend,
    /// The new Backend started with `--check` against the production settings.
    Prove,
    /// The zip.
    Zip,
    /// The install script.
    Script,
    /// Both files are in the chosen folder.
    Done,
}

impl Step {
    /// What the step does, in English, for the tools' own reports (the IDE says it in the developer's language).
    pub fn label(self) -> &'static str {
        match self {
            Step::Check => "checking the project's settings",
            Step::Page => "building the page",
            Step::Backend => "building the Backend (the first time it compiles the platform and takes longer)",
            Step::Prove => "starting the Backend with the production settings, to check them",
            Step::Zip => "writing the zip",
            Step::Script => "writing the install script",
            Step::Done => "done",
        }
    }

    /// A stable word for each step (the IDE maps it to its own text).
    pub fn key(self) -> &'static str {
        match self {
            Step::Check => "check",
            Step::Page => "page",
            Step::Backend => "backend",
            Step::Prove => "prove",
            Step::Zip => "zip",
            Step::Script => "script",
            Step::Done => "done",
        }
    }
}

/// Why a package was not made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageError {
    /// The project's settings cannot give an installation: the text names the setting and where to set it.
    Refused(String),
    /// A tool failed (the build, the Backend, the disk).
    Failed(String),
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageError::Refused(why) | PackageError::Failed(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for PackageError {}

/// What a package is, once made.
#[derive(Debug, Clone, PartialEq)]
pub struct Packaged {
    pub zip: PathBuf,
    pub script: PathBuf,
    pub zip_bytes: u64,
    /// The SHA-256 of the zip, the one the script checks.
    pub sha256: String,
    pub files_in_zip: usize,
    /// The package carries a Backend (a project with none gets the site only).
    pub has_backend: bool,
    /// The page's Backend address is `wss://`: the script asks for a certificate and a key.
    pub wss: bool,
    /// The secrets the Backend reads, by name; the script wants each in its environment file.
    pub required_secrets: Vec<String>,
    /// Things the developer should know that did not stop the package.
    pub notes: Vec<String>,
    /// Seconds each step took, in order.
    pub timings: Vec<(Step, f64)>,
    pub seconds: f64,
}

impl Packaged {
    /// The one command that installs, as the script's own usage shows it.
    pub fn command(&self) -> String {
        let name = self.script.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let secrets = !self.required_secrets.is_empty();
        if name.ends_with(".ps1") {
            let wss = if self.wss { " -Cert FILE -Key FILE" } else { "" };
            let env = if secrets { " -EnvFile FILE" } else { "" };
            format!("powershell -ExecutionPolicy Bypass -File .\\{name} -NginxDir <nginx folder>{wss}{env}")
        } else {
            let wss = if self.wss { " --cert FILE --key FILE" } else { "" };
            let env = if secrets { " --env-file FILE" } else { "" };
            format!("sudo bash ./{name} <nginx folder>{wss}{env}")
        }
    }
}

// ───────────────────────────── the production settings ─────────────────────────────

/// The Backend's settings as a server runs them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Production {
    /// The text of the production `backend.toml`.
    pub text: String,
    /// `[server] listen`, as the project set it (loopback).
    pub listen: String,
    /// The page's Backend address is `wss://`.
    pub uses_wss: bool,
    /// The origin added to `allowed_origins` (the page's own, from `backend_url`).
    pub origin: String,
    /// The name nginx answers to: the host of `backend_url`.
    pub server_name: String,
    /// The names the Backend reads as `env:NAME`.
    pub required_secrets: Vec<String>,
}

struct BackendUrl {
    secure: bool,
    host: String,
    port: Option<u16>,
}

fn is_loopback(host: &str) -> bool {
    let h = host.trim_matches(|c| c == '[' || c == ']').to_ascii_lowercase();
    h == "localhost" || h == "::1" || h == "0.0.0.0" || h == "::" || h.starts_with("127.")
}

/// `host:port` or `[v6]:port`, split.
fn split_host_port(s: &str) -> (String, Option<u16>) {
    if let Some(rest) = s.strip_prefix('[') {
        if let Some((host, tail)) = rest.split_once(']') {
            return (format!("[{host}]"), tail.strip_prefix(':').and_then(|p| p.parse().ok()));
        }
    }
    match s.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty() => (h.to_owned(), p.parse().ok()),
        _ => (s.to_owned(), None),
    }
}

fn parse_backend_url(url: &str) -> Result<BackendUrl, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("[web] backend_url is empty, so the page has no Backend to reach. Set it in Web Settings to the public address, for example wss://app.example.com/ws".into());
    }
    let (secure, rest) = if let Some(r) = url.strip_prefix("wss://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("ws://") {
        (false, r)
    } else {
        return Err(format!("[web] backend_url = \"{url}\" does not begin with wss:// or ws://. Set it in Web Settings, for example wss://app.example.com/ws"));
    };
    let authority = rest.split('/').next().unwrap_or("");
    let (host, port) = split_host_port(authority);
    if host.is_empty() {
        return Err(format!("[web] backend_url = \"{url}\" names no host. Set it in Web Settings, for example wss://app.example.com/ws"));
    }
    if is_loopback(&host) {
        return Err(format!(
            "[web] backend_url = \"{url}\" points at the computer of whoever opens the page. In an installation it is the public address nginx answers on: set it in Web Settings, for example wss://app.example.com/ws"
        ));
    }
    Ok(BackendUrl { secure, host, port })
}

impl BackendUrl {
    /// The origin of the page: `https://host[:port]` for `wss://`, `http://…` for `ws://` (a default port left out).
    fn origin(&self) -> String {
        let (scheme, default) = if self.secure { ("https", 443) } else { ("http", 80) };
        match self.port {
            Some(p) if p != default => format!("{scheme}://{}:{p}", self.host),
            _ => format!("{scheme}://{}", self.host),
        }
    }
}

/// Every string of the document, each handed to `f`; a `Some` it returns replaces the string.
fn strings_mut(item: &mut Item, f: &mut dyn FnMut(&str) -> Option<String>) {
    match item {
        Item::Value(v) => value_strings_mut(v, f),
        Item::Table(t) => {
            for (_, i) in t.iter_mut() {
                strings_mut(i, f);
            }
        }
        Item::ArrayOfTables(a) => {
            for t in a.iter_mut() {
                for (_, i) in t.iter_mut() {
                    strings_mut(i, f);
                }
            }
        }
        Item::None => {}
    }
}

fn value_strings_mut(v: &mut Value, f: &mut dyn FnMut(&str) -> Option<String>) {
    let replacement = if let Value::String(s) = &*v { f(s.value()) } else { None };
    if let Some(new) = replacement {
        *v = Value::from(new);
        return;
    }
    match v {
        Value::Array(a) => {
            for x in a.iter_mut() {
                value_strings_mut(x, f);
            }
        }
        Value::InlineTable(t) => {
            for (_, x) in t.iter_mut() {
                value_strings_mut(x, f);
            }
        }
        _ => {}
    }
}

fn secrets_of(doc: &DocumentMut) -> Vec<String> {
    let mut found = BTreeSet::new();
    let mut probe = doc.clone();
    strings_mut(probe.as_item_mut(), &mut |s| {
        if let Some(name) = s.strip_prefix("env:") {
            let name = name.trim();
            if !name.is_empty() {
                found.insert(name.to_owned());
            }
        }
        None
    });
    found.into_iter().collect()
}

/// Make the production `backend.toml` from the project's own `source` (its text; `label` names it in a refusal) and the page's
/// `backend_url`. The project's comments and key order stay; the origin of `backend_url` joins `[server] allowed_origins`, once.
///
/// A refusal names the setting and where to set it. What the Backend itself refuses in a release start (an `unsafe_…` setting,
/// a secret written by value) is not repeated here: [`prove`] runs the Backend's own check.
pub fn production_config(source: &str, label: &str, backend_url: &str) -> Result<Production, String> {
    let url = parse_backend_url(backend_url)?;
    let mut doc: DocumentMut = source.parse().map_err(|e| format!("{label} is not valid TOML: {e}"))?;

    let listen = doc.get("server").and_then(|s| s.get("listen")).and_then(Item::as_str).map(str::to_owned);
    let Some(listen) = listen else {
        return Err(format!("{label} has no [server] listen. Set it to a loopback address, for example listen = \"127.0.0.1:8443\": nginx faces the network and hands the Backend what it needs"));
    };
    let (listen_host, listen_port) = split_host_port(&listen);
    if !is_loopback(&listen_host) || listen_host == "0.0.0.0" || listen_host == "::" || listen_port.is_none() {
        return Err(format!(
            "{label}: [server] listen = \"{listen}\" is not a loopback address with a port. This package installs behind nginx: set listen = \"127.0.0.1:8443\" and nginx will be the one that faces the network"
        ));
    }
    if doc.get("tls").is_some() {
        return Err(format!("{label} has a [tls] section. This package installs behind nginx, which ends TLS: remove [tls] and give the certificate and key to the install script instead"));
    }
    if doc.get("auth").is_none() && doc.get("identity").is_none() {
        return Err(format!(
            "{label} has neither [auth] (issuer and audiences of the tokens the Backend accepts) nor [identity] (its own sign-in). A Backend that anyone may call is not installed: set one of them in {label}"
        ));
    }

    let origin = url.origin();
    if doc.get("server").and_then(Item::as_table_like).is_none() {
        return Err(format!("{label}: [server] is not a table"));
    }
    let server = doc.get_mut("server").and_then(Item::as_table_like_mut).expect("checked above");
    let entry = server.entry("allowed_origins").or_insert(Item::Value(Value::Array(Array::new())));
    let Some(list) = entry.as_array_mut() else {
        return Err(format!("{label}: [server] allowed_origins is not a list of origins"));
    };
    if !list.iter().any(|v| v.as_str() == Some(origin.as_str())) {
        list.push(origin.as_str());
    }

    let required_secrets = secrets_of(&doc);
    let header = format!(
        "# Made by PowerRustCOBOL AI (Package) from {label}, for the server this package installs on.\n# The install script keeps a copy of the file that was there before it replaces it.\n"
    );
    Ok(Production {
        text: format!("{header}{doc}"),
        listen,
        uses_wss: url.secure,
        origin,
        server_name: url.host.clone(),
        required_secrets,
    })
}

/// Run the packaged Backend with `--check` against `production`, in a folder of its own, and return the address it would listen
/// on. Every `env:NAME` the settings read is given a harmless value in the **child's** environment only, and every `file:PATH`
/// a file of its own in that folder, so the check proves the settings and reads nothing of this computer's.
pub fn prove(production: &Production, backend_exe: &Path) -> Result<String, String> {
    let work = scratch_dir("prove")?;
    let result = (|| {
        let mut doc: DocumentMut = production.text.parse().map_err(|e| format!("the production settings are not valid TOML: {e}"))?;
        let mut n = 0;
        strings_mut(doc.as_item_mut(), &mut |s| {
            let path = s.strip_prefix("file:")?;
            if path.trim().is_empty() {
                return None;
            }
            n += 1;
            let name = format!("secret-{n}.txt");
            std::fs::write(work.join(&name), "harmless").ok()?;
            Some(format!("file:{name}"))
        });
        std::fs::write(work.join("backend.toml"), doc.to_string()).map_err(|e| format!("cannot write the settings to check: {e}"))?;
        let mut cmd = Command::new(backend_exe);
        cmd.args(["--check", "--config", "backend.toml"]).current_dir(&work);
        for name in &production.required_secrets {
            cmd.env(name, "harmless");
        }
        let out = cmd.output().map_err(|e| format!("cannot run {}: {e}", backend_exe.display()))?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if !out.status.success() {
            let why = stderr.trim();
            return Err(format!("the Backend does not accept the production settings:\n{}", if why.is_empty() { stdout.trim() } else { why }));
        }
        stdout
            .lines()
            .find_map(|l| l.strip_prefix("configuration ok:"))
            .and_then(|l| l.split(" on ").nth(1))
            .and_then(|l| l.split(" (").next())
            .map(|a| a.trim().to_owned())
            .ok_or_else(|| format!("the Backend's check said something unexpected: {}", stdout.trim()))
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

// ───────────────────────────── the zip ─────────────────────────────

/// One file of the zip: its name in it, its bytes' source and its mode.
struct Entry {
    name: String,
    source: Source,
    mode: u32,
}

enum Source {
    Path(PathBuf),
    Bytes(Vec<u8>),
}

fn walk_files(base: &Path, dir: &Path, skip: &dyn Fn(&str) -> bool, out: &mut Vec<(String, PathBuf)>) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        let rel = p.strip_prefix(base).unwrap_or(&p).to_string_lossy().replace('\\', "/");
        if skip(&rel) {
            continue;
        }
        if p.is_dir() {
            walk_files(base, &p, skip, out)?;
        } else {
            out.push((rel, p));
        }
    }
    Ok(())
}

/// Write the zip: entries in name order, a fixed time and fixed modes, so the same inputs give the same bytes.
fn write_zip(path: &Path, mut entries: Vec<Entry>) -> Result<(u64, String, usize), String> {
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, DateTime};
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    let file = std::fs::File::create(path).map_err(|e| format!("cannot create {}: {e}", path.display()))?;
    let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
    let count = entries.len();
    for e in entries {
        let bytes = match e.source {
            Source::Bytes(b) => b,
            Source::Path(p) => std::fs::read(&p).map_err(|err| format!("cannot read {}: {err}", p.display()))?,
        };
        let opts = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .last_modified_time(DateTime::default())
            .unix_permissions(e.mode)
            .large_file(bytes.len() as u64 >= 0xFFFF_FFFF);
        zip.start_file(e.name.as_str(), opts).map_err(|err| format!("zip: {}: {err}", e.name))?;
        zip.write_all(&bytes).map_err(|err| format!("zip: {}: {err}", e.name))?;
    }
    let mut writer = zip.finish().map_err(|e| format!("zip: {e}"))?;
    writer.flush().map_err(|e| format!("zip: {e}"))?;
    drop(writer);
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read back {}: {e}", path.display()))?;
    Ok((bytes.len() as u64, cobolt_web_bundle::hash::sha256_hex(&bytes), count))
}

fn scratch_dir(what: &str) -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("prc-package-{what}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    Ok(dir)
}

/// The name a file of the package starts with: the project's name with what a file name cannot hold made a dash.
pub fn file_stem(project: &str) -> String {
    let mut s: String = project.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '_' | '.') { c } else { '-' }).collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s = s.trim_matches(|c| c == '-' || c == '.').to_owned();
    if s.is_empty() {
        "app".into()
    } else {
        s
    }
}

/// The names of the two files a package of `project` is: the zip and the install script of the system this runs on.
pub fn file_names(project: &str) -> [String; 2] {
    let stem = file_stem(project);
    [format!("{stem}.zip"), format!("{stem}-install.{}", TargetOs::host().script_ext())]
}

/// Everything the zip holds, from the project's folder, the bundle, the Backend and the production settings.
fn zip_entries(project_dir: &Path, bundle: &Path, backend: Option<(&Path, &Production)>) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    let mut files = Vec::new();
    // The deployment kit (`hosting/`) is for hosting the files somewhere else: an installed server has no use for it; a source map is a debug file.
    walk_files(bundle, bundle, &|rel| rel == "hosting" || rel.ends_with(".map"), &mut files).map_err(|e| format!("cannot read the bundle {}: {e}", bundle.display()))?;
    for (rel, path) in files {
        entries.push(Entry { name: format!("web/{rel}"), source: Source::Path(path), mode: 0o644 });
    }
    if let Some((exe, production)) = backend {
        let exe_name = exe.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        entries.push(Entry { name: format!("bin/{exe_name}"), source: Source::Path(exe.to_path_buf()), mode: 0o755 });
        if let Some(sbom) = exe.parent().map(|d| d.join("sbom.cdx.json")).filter(|p| p.is_file()) {
            entries.push(Entry { name: "bin/sbom.cdx.json".into(), source: Source::Path(sbom), mode: 0o644 });
        }
        entries.push(Entry { name: "backend.toml".into(), source: Source::Bytes(production.text.clone().into_bytes()), mode: 0o644 });
        let mut data = Vec::new();
        let data_dir = project_dir.join("data");
        if data_dir.is_dir() {
            // The seed is the data the developer made; a `.lck` beside a file is the lock of a run that has ended.
            walk_files(&data_dir, &data_dir, &|rel| rel.ends_with(".lck"), &mut data).map_err(|e| format!("cannot read {}: {e}", data_dir.display()))?;
        }
        for (rel, path) in data {
            entries.push(Entry { name: format!("data/{rel}"), source: Source::Path(path), mode: 0o644 });
        }
        let kb_dir = project_dir.join("assets").join("KB");
        if kb_dir.is_dir() {
            let mut kb = Vec::new();
            walk_files(&kb_dir, &kb_dir, &|_| false, &mut kb).map_err(|e| format!("cannot read {}: {e}", kb_dir.display()))?;
            for (rel, path) in kb {
                entries.push(Entry { name: format!("assets/KB/{rel}"), source: Source::Path(path), mode: 0o644 });
            }
        }
    }
    entries.push(Entry { name: "LICENSE".into(), source: Source::Bytes(cobolt_compiler::LICENSE_TEXT.as_bytes().to_vec()), mode: 0o644 });
    entries.push(Entry { name: "NOTICE".into(), source: Source::Bytes(cobolt_compiler::NOTICE_TEXT.as_bytes().to_vec()), mode: 0o644 });
    entries.push(Entry { name: "NOTICE-RUNTIME.txt".into(), source: Source::Bytes(cobolt_compiler::RUNTIME_NOTICE_TEXT.as_bytes().to_vec()), mode: 0o644 });
    Ok(entries)
}

// ───────────────────────────── the function ─────────────────────────────

/// Package the Web project at `manifest` into `out_dir`: the zip and the install script, and nothing else. `lang` is the code of
/// the language the script speaks (`en es pt ja zh fr`; `jp` and `cn` are read too; any other is English). `progress` is called
/// as each [`Step`] begins.
///
/// The caller has saved what is open (the Build reads the files); this regenerates nothing of the project's.
pub fn package_web(manifest: &Path, out_dir: &Path, lang: &str, progress: &mut dyn FnMut(Step)) -> Result<Packaged, PackageError> {
    let started = Instant::now();
    let mut timings: Vec<(Step, f64)> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut mark = |step: Step, progress: &mut dyn FnMut(Step), t: &mut Instant| {
        if let Some(last) = timings.last_mut() {
            last.1 = t.elapsed().as_secs_f64();
        }
        *t = Instant::now();
        timings.push((step, 0.0));
        progress(step);
    };
    let mut clock = Instant::now();

    // ── Check: the settings, before anything is built or written ──
    mark(Step::Check, progress, &mut clock);
    let mut request = WebBuildRequest::from_manifest(manifest).map_err(PackageError::Refused)?;
    if request.kind != ProjectKind::Web {
        return Err(PackageError::Refused("only a Web project is packaged for a server; a Mobile project is built for its platform".into()));
    }
    request.profile = BuildProfile::Release;
    let project_dir = request.project_dir.clone();
    let backend_dir = project_dir.join("backend");
    let has_backend = backend_dir.join("exports.toml").is_file();
    let production = if has_backend {
        let source_path = backend_dir.join("backend.toml");
        let source = std::fs::read_to_string(&source_path).map_err(|_| PackageError::Refused(format!("{} was not found: the Backend needs its settings to be installed", source_path.display())))?;
        Some(production_config(&source, "backend/backend.toml", &request.web.backend_url).map_err(PackageError::Refused)?)
    } else {
        None
    };

    // ── Page: the complete Web Build, as the Build button runs it ──
    mark(Step::Page, progress, &mut clock);
    let report = match super::backend().build(&request) {
        Ok(r) => r,
        Err(e @ WebBuildError::Refused(_)) => return Err(PackageError::Refused(e.to_string())),
        Err(e) => return Err(PackageError::Failed(e.to_string())),
    };
    let bundle = report.bundle_dir.clone();
    let hosting_nginx = std::fs::read_to_string(bundle.join("hosting").join("nginx.conf")).map_err(|e| PackageError::Failed(format!("the Build wrote no hosting/nginx.conf to read the security headers from: {e}")))?;
    let headers = cobolt_web_bundle::csp::headers_of_nginx_snippet(&hosting_nginx);
    if headers.is_empty() {
        return Err(PackageError::Failed("the Build's hosting/nginx.conf holds no security headers".into()));
    }

    // ── Backend and Prove ──
    let mut exe: Option<PathBuf> = None;
    let mut addr = String::new();
    if let Some(production) = &production {
        mark(Step::Backend, progress, &mut clock);
        if cfg!(windows) {
            notes.push("The Backend is built with its C runtime inside, so it starts on a Windows that has not installed the Visual C++ Redistributable; the first package compiles the platform for that and takes longer.".into());
        }
        let opts = cobolt_compiler::backend_build::BackendBuildOptions { debug: false, verbose: false, workspace_root: None, accept_breaking: false };
        let built = cobolt_compiler::backend_build::build_backend(&project_dir, &opts).map_err(|e| PackageError::Failed(format!("the Backend did not build: {e}")))?;
        notes.extend(built.notes.iter().cloned());
        mark(Step::Prove, progress, &mut clock);
        addr = prove(production, &built.binary).map_err(PackageError::Refused)?;
        exe = Some(built.binary);
    }

    // ── Zip ──
    mark(Step::Zip, progress, &mut clock);
    let stage = scratch_dir("stage").map_err(PackageError::Failed)?;
    let outcome = (|| -> Result<Packaged, PackageError> {
        let stem = file_stem(&request.name);
        let entries = zip_entries(&project_dir, &bundle, exe.as_deref().zip(production.as_ref())).map_err(PackageError::Failed)?;
        let zip_path = stage.join(format!("{stem}.zip"));
        let (zip_bytes, sha256, files_in_zip) = write_zip(&zip_path, entries).map_err(PackageError::Failed)?;

        // ── Script ──
        mark(Step::Script, progress, &mut clock);
        let port = addr.rsplit_once(':').and_then(|(_, p)| p.parse::<u16>().ok()).unwrap_or(0);
        let facts = match (&production, &exe) {
            (Some(p), Some(exe)) => Some(BackendFacts {
                exe: exe.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                addr: addr.clone(),
                port,
                required_secrets: p.required_secrets.clone(),
                has_kb: project_dir.join("assets").join("KB").is_dir(),
            }),
            _ => None,
        };
        let install = Install {
            project: &request.name,
            stem: stem.clone(),
            slug: cobolt_web_bundle::deploy::slug(&request.name),
            os: TargetOs::host(),
            lang: Lang::from_code(lang),
            zip_sha256: sha256.clone(),
            headers: &headers,
            backend: facts,
            server_name: production.as_ref().map(|p| p.server_name.clone()).unwrap_or_else(|| "_".into()),
            wss: production.as_ref().is_some_and(|p| p.uses_wss),
        };
        let script_path = stage.join(install.script_name());
        std::fs::write(&script_path, install.script()).map_err(|e| PackageError::Failed(format!("cannot write the script: {e}")))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755));
        }

        // ── into the chosen folder: both are copied beside their final names first, so a failure leaves nothing new ──
        let created_dir = !out_dir.exists();
        std::fs::create_dir_all(out_dir).map_err(|e| PackageError::Failed(format!("cannot create {}: {e}", out_dir.display())))?;
        let final_zip = out_dir.join(format!("{stem}.zip"));
        let final_script = out_dir.join(install.script_name());
        let (part_zip, part_script) = (out_dir.join(format!(".{stem}.zip.part")), out_dir.join(format!(".{}.part", install.script_name())));
        let moved = std::fs::copy(&zip_path, &part_zip).and_then(|_| std::fs::copy(&script_path, &part_script)).and_then(|_| std::fs::rename(&part_zip, &final_zip)).and_then(|_| std::fs::rename(&part_script, &final_script));
        if let Err(e) = moved {
            let _ = std::fs::remove_file(&part_zip);
            let _ = std::fs::remove_file(&part_script);
            if created_dir {
                let _ = std::fs::remove_dir(out_dir);
            }
            return Err(PackageError::Failed(format!("cannot write into {}: {e}", out_dir.display())));
        }
        mark(Step::Done, progress, &mut clock);
        Ok(Packaged {
            zip: final_zip,
            script: final_script,
            zip_bytes,
            sha256,
            files_in_zip,
            has_backend: production.is_some(),
            wss: install.wss,
            required_secrets: production.as_ref().map(|p| p.required_secrets.clone()).unwrap_or_default(),
            notes: Vec::new(),
            timings: Vec::new(),
            seconds: 0.0,
        })
    })();
    let _ = std::fs::remove_dir_all(&stage);
    let mut packaged = outcome?;
    if let Some(last) = timings.last_mut() {
        last.1 = clock.elapsed().as_secs_f64();
    }
    packaged.notes = notes;
    packaged.timings = timings;
    packaged.seconds = started.elapsed().as_secs_f64();
    Ok(packaged)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "# the project's own comment\n[server]\nlisten = \"127.0.0.1:8443\"\n\n[auth]\nissuer = [\"https://login.example.com\"]\naudiences = [\"api\"]\n\n[[configuration]]\nname = \"CRM\"\nAuthToken = \"env:CRM_TOKEN\"\nOther = \"env:ALPHA_KEY\"\nDup = \"env:CRM_TOKEN\"\n";

    #[test]
    fn the_production_settings_keep_the_comments_and_add_the_origin_once() {
        let mut checks = 0;
        let p = production_config(GOOD, "backend/backend.toml", "wss://oficina.example.com/ws").unwrap();
        assert!(p.text.starts_with("# Made by PowerRustCOBOL AI (Package) from backend/backend.toml"));
        assert!(p.text.contains("# the project's own comment"), "the developer's comment survives");
        assert!(p.text.contains("allowed_origins = [\"https://oficina.example.com\"]"), "{}", p.text);
        assert_eq!((p.uses_wss, p.origin.as_str(), p.server_name.as_str(), p.listen.as_str()), (true, "https://oficina.example.com", "oficina.example.com", "127.0.0.1:8443"));
        assert_eq!(p.required_secrets, ["ALPHA_KEY", "CRM_TOKEN"], "each name once, in order");
        checks += 7;
        // A port other than the default stays in the origin; ws:// is http.
        assert_eq!(production_config(GOOD, "x", "wss://h.example.com:8443/ws").unwrap().origin, "https://h.example.com:8443");
        assert_eq!(production_config(GOOD, "x", "wss://h.example.com:443/ws").unwrap().origin, "https://h.example.com");
        let plain = production_config(GOOD, "x", "ws://h.example.com/ws").unwrap();
        assert_eq!((plain.origin.as_str(), plain.uses_wss), ("http://h.example.com", false));
        checks += 3;
        // An origin the project already lists is not added again; others are kept.
        let listed = GOOD.replace("listen = \"127.0.0.1:8443\"", "listen = \"127.0.0.1:8443\"\nallowed_origins = [\"https://a.example.com\", \"https://oficina.example.com\"]");
        let p = production_config(&listed, "x", "wss://oficina.example.com/ws").unwrap();
        assert_eq!(p.text.matches("https://oficina.example.com").count(), 1);
        assert!(p.text.contains("https://a.example.com"));
        // Running it again over its own output changes nothing more.
        let again = production_config(&p.text, "x", "wss://oficina.example.com/ws").unwrap();
        assert_eq!(again.text.matches("https://oficina.example.com").count(), 1);
        checks += 4;
        println!("production settings: {checks} checks, 0 failed");
    }

    #[test]
    fn every_setting_the_package_cannot_install_is_refused_naming_it() {
        let ok_url = "wss://oficina.example.com/ws";
        let cases: [(&str, String, &str, &str); 10] = [
            ("no [server] listen", "[auth]\nissuer = [\"https://i\"]\n".into(), ok_url, "no [server] listen"),
            ("a network listen address", GOOD.replace("127.0.0.1:8443", "0.0.0.0:8443"), ok_url, "loopback"),
            ("a public listen address", GOOD.replace("127.0.0.1:8443", "10.0.0.5:8443"), ok_url, "loopback"),
            ("a [tls] section", format!("{GOOD}\n[tls]\ncert_chain = \"c.pem\"\n"), ok_url, "[tls]"),
            ("neither [auth] nor [identity]", "[server]\nlisten = \"127.0.0.1:8443\"\n".into(), ok_url, "[auth]"),
            ("an empty backend_url", GOOD.into(), "", "backend_url is empty"),
            ("a backend_url on this computer", GOOD.into(), "ws://127.0.0.1:8443", "computer of whoever opens the page"),
            ("a backend_url that is not a socket address", GOOD.into(), "https://oficina.example.com/ws", "wss:// or ws://"),
            ("settings that are not TOML", "[server\nlisten".into(), ok_url, "not valid TOML"),
            ("allowed_origins that is not a list", GOOD.replace("listen = \"127.0.0.1:8443\"", "listen = \"127.0.0.1:8443\"\nallowed_origins = \"https://a\""), ok_url, "not a list"),
        ];
        for (what, source, url, expect) in &cases {
            let e = production_config(source, "backend/backend.toml", url).expect_err(what);
            assert!(e.contains(expect), "{what}: {e}");
        }
        println!("production settings: {} refusals, each names its setting", cases.len());
    }

    /// A project folder with a bundle, a Backend, data and a knowledge base, built by hand.
    fn fixture(root: &Path) -> (PathBuf, PathBuf, PathBuf) {
        let project = root.join("proj");
        let bundle = project.join("dist").join("web");
        for (rel, text) in [
            ("dist/web/index.html", "<html></html>"),
            ("dist/web/app.12345678.js", "js"),
            ("dist/web/hosting/nginx.conf", "hosted elsewhere"),
            ("dist/web/hosting/deploy/README-en.md", "kit"),
            ("dist/web/app.js.map", "map"),
            ("dist/web/.well-known/security.txt", "Contact: x"),
            ("data/customers.dat", "rec1"),
            ("data/sub/orders.dat", "rec2"),
            ("data/customers.dat.lck", ""),
            ("assets/KB/index.json", "{}"),
            ("forms/main.cfrm", "a form"),
            ("src/COMMON.cbl", "cobol"),
            ("generated/main.cbl", "cobol"),
            ("bin/app-backend", "ELF"),
            ("bin/sbom.cdx.json", "{}"),
        ] {
            let p = project.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        (project.clone(), bundle, project.join("bin").join("app-backend"))
    }

    fn listing(zip_path: &Path) -> Vec<String> {
        let mut z = zip::ZipArchive::new(std::fs::File::open(zip_path).unwrap()).unwrap();
        (0..z.len()).map(|i| z.by_index(i).unwrap().name().unwrap().into_owned()).collect()
    }

    #[test]
    fn the_zip_holds_what_runs_and_nothing_else_and_is_the_same_bytes_twice() {
        let started = Instant::now();
        let tmp = tempfile::tempdir().unwrap();
        let (project, bundle, exe) = fixture(tmp.path());
        let production = production_config(GOOD, "backend/backend.toml", "wss://oficina.example.com/ws").unwrap();
        let entries = || zip_entries(&project, &bundle, Some((&exe, &production))).unwrap();
        let (a, b) = (tmp.path().join("a.zip"), tmp.path().join("b.zip"));
        let (bytes_a, sha_a, n) = write_zip(&a, entries()).unwrap();
        let (bytes_b, sha_b, _) = write_zip(&b, entries()).unwrap();
        assert_eq!((bytes_a, &sha_a), (bytes_b, &sha_b), "the same inputs give the same bytes");
        assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
        let names = listing(&a);
        assert_eq!(n, names.len());
        for want in ["web/index.html", "web/app.12345678.js", "web/.well-known/security.txt", "bin/app-backend", "bin/sbom.cdx.json", "backend.toml", "data/customers.dat", "data/sub/orders.dat", "assets/KB/index.json", "LICENSE", "NOTICE", "NOTICE-RUNTIME.txt"] {
            assert!(names.iter().any(|x| x == want), "{want} is in the zip: {names:?}");
        }
        for bad in ["hosting", "forms/", "src/", "generated/", "agentic_ai", ".cfrm", ".wcfrm", ".cbl", ".map", "Project KB", ".pdb", ".lck"] {
            assert!(!names.iter().any(|x| x.contains(bad)), "{bad} must not be in the zip: {names:?}");
        }
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "entries are in name order");
        // The production settings, and the modes, are what the script expects.
        let mut z = zip::ZipArchive::new(std::fs::File::open(&a).unwrap()).unwrap();
        let mut text = String::new();
        std::io::Read::read_to_string(&mut z.by_name("backend.toml").unwrap(), &mut text).unwrap();
        assert!(text.contains("allowed_origins"));
        assert_eq!(z.by_name("bin/app-backend").unwrap().unix_mode(), Some(0o100755));
        assert_eq!(z.by_name("web/index.html").unwrap().unix_mode(), Some(0o100644));
        // A project with no Backend: the site only.
        let site_only = tmp.path().join("site.zip");
        write_zip(&site_only, zip_entries(&project, &bundle, None).unwrap()).unwrap();
        let site = listing(&site_only);
        assert!(site.iter().all(|x| x.starts_with("web/") || x == "LICENSE" || x.starts_with("NOTICE")), "{site:?}");
        println!(
            "zip: {n} entries, {bytes_a} bytes, sha256 {}..., twice byte-equal; hosting/forms/src/generated/maps left out; site-only has {} entries; {} ms",
            &sha_a[..12],
            site.len(),
            started.elapsed().as_millis()
        );
    }

    #[test]
    fn the_file_stem_is_one_a_file_can_hold() {
        for (project, want) in [("PowerOficina", "PowerOficina"), ("Power Oficina", "Power-Oficina"), ("a/b:c", "a-b-c"), ("***", "app"), ("Ação", "A-o")] {
            assert_eq!(file_stem(project), want, "{project}");
        }
    }

    #[test]
    fn a_project_that_is_not_web_is_refused_before_anything_is_written() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("out");
        let missing = tmp.path().join("nope.project.toml");
        let mut steps = Vec::new();
        let e = package_web(&missing, &out, "en", &mut |s| steps.push(s)).unwrap_err();
        assert!(matches!(e, PackageError::Refused(_)), "{e}");
        assert_eq!(steps, [Step::Check], "a refusal stops at the first step");
        assert!(!out.exists(), "and the chosen folder was not made");
    }
}
