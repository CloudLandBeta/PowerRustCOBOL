// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The web Preview, on the IDE's side** (WASM-008, WASM-002 R8).
//!
//! A Web or Mobile project is not run by the interpreter: its Build produces a
//! bundle, a small server hands the bundle to a **browser**, and the browser
//! is the Preview (the operator's ruling of 2026-10-23, WASM-008 R12–R16). This
//! module is everything the IDE does around that:
//!
//! 1. a **build job** on a thread of its own, through the
//!    [`WebBuildBackend`] the program linked (the IDE never waits for it);
//! 2. the **development server** that serves the bundle on a loopback port,
//!    with the Preview script that updates the page in place and reports its
//!    console, errors and update times back (`devserver`);
//! 3. the **launcher**: the browser the project names (`preview_browser`), or
//!    the system's default one — the IDE opens no window of its own for the
//!    page, and starts no web-view helper (that one is headless, for the
//!    agents' tools and the tests);
//! 4. a **watcher** over the project's own folders: when a form, a stylesheet,
//!    a program or the Backend changes on disk, the bundle is built again and
//!    the open page takes it in place.
//!
//! What it says goes through [`Note`]s — a message with its facts, not its
//! words — so the Output panel shows it in the language the developer chose,
//! from the `Tr` tables.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cobolt_compiler::project_kind::{DeviceClass, Isolation, MobilePlatform, Orientation, ProjectKind};
use cobolt_project_tools::web::devbackend::{self, DevBackendPlan};
use cobolt_project_tools::web::devserver::{self, DevBackend, PageEvent, PageStrings, PhoneFrame, ServeOptions, ServerHandle};
use cobolt_project_tools::web::presets;
use cobolt_project_tools::web::preview::{self, BrowserChoice};
use cobolt_project_tools::web::{BuildReport, PartSize, PlayerPinError, WebBuildBackend, WebBuildError, WebBuildRequest, WebDiagnostic, WebDiagnosticKind};

use crate::backend_run::{BackendEvent, BackendRun};
use crate::i18n::{Language, Tr};

/// How often the watcher looks at the project's folders (WASM-008 D6: a
/// change is reported after one more, quiet, look).
pub const WATCH_EVERY: Duration = Duration::from_millis(150);
/// The notes the Preview keeps.
const MAX_NOTES: usize = 300;
/// A page that logs in a loop must not flood the Output panel: this many
/// console and error lines are taken from the page at a time, the rest of
/// that burst is dropped.
const CHATTER_PER_POLL: usize = 40;
/// How long a Preview waits for the Backend it started to say where it
/// listens (the first start of a Backend reads its programs; a few seconds).
pub const BACKEND_WAIT: Duration = Duration::from_secs(60);
/// The Backend's last lines kept while it starts, to say why when it fails.
const BACKEND_TAIL: usize = 8;

// ── What the Preview says ───────────────────────────────────────────────────

/// Something the Preview has to tell the developer: the facts, not the words.
#[derive(Debug, Clone, PartialEq)]
pub enum Note {
    Building,
    NotLinked,
    BuildDone { ms: u64, bytes: u64, files: usize, startup_ms: Option<f64> },
    BuildWarning(WebDiagnostic),
    /// What a refused build could not compile: where, the construct, the alternative.
    BuildRefused(Vec<WebDiagnostic>),
    BuildFailed(String),
    /// The player the bundle carries: its variants, then the parts loaded on first use (WASM-016 R13).
    PlayerSizes(Vec<PartSize>),
    /// The project pins a player the Build cannot use (WASM-016 R11).
    PlayerPin(PlayerPinError),
    /// A Mobile build ends at the bundle until the mobile shell is linked.
    PackageNote,
    NoBundle,
    Serving { name: String, url: String },
    ServerFailed(String),
    /// The page was opened at `url` in `browser` (empty: the system's default).
    Opened { browser: String, url: String },
    /// The browser the project names could not be started; the default is used.
    BrowserFailed { name: String, why: String },
    /// The application path the project names is not confirmed on this machine.
    BrowserNotConfirmed { path: String },
    /// A rebuild finished this long after the change that caused it.
    Updated { ms: u64 },
    BackendOff,
    /// The Preview starts the project's Backend on `listen` and waits up to `secs` for it.
    BackendStarting { listen: String, secs: u64 },
    /// The Backend listens on `url`; the page is built for it.
    BackendReady { url: String },
    /// The port of `backend/backend.toml` is taken: the Backend starts on a free one.
    BackendPortBusy { port: u16 },
    /// The manifest's loopback address is taken: no Backend is started.
    BackendTaken { url: String },
    /// The manifest names another machine: no local Backend is started.
    BackendRemote { url: String },
    /// The IDE's Backend listens on `running`, the page is built for `page`.
    BackendMismatch { running: String, page: String },
    /// The Backend ended (exit `code`) before it listened, with its own last lines.
    BackendFailed { code: Option<i32>, why: Vec<String> },
    /// The Backend did not say where it listens within `secs`.
    BackendTimeout { secs: u64 },
    /// A console line of the page, from form `form` (empty: unknown).
    Console { level: String, text: String, form: String },
    /// An uncaught error or a rejected promise of the page.
    PageError { message: String, source: String, line: u32, form: String },
    /// A COBOL run-time error the player recorded: the program unit and its line.
    CobolError { form: String, unit: String, line: u32, code: String, message: String },
    /// The page took a change in place (`how`, what `app.update` answered) in `ms`.
    PageUpdated { how: String, ms: f64 },
    /// The Preview was stopped; an open tab says so at its next polls.
    PreviewStopped,
    /// The served `config.json` carries the development token of the Backend
    /// at `url`, which the player does not send yet (WASM-008 T38; WASM-016).
    TokenUnused { url: String },
    /// A Mobile Preview in the browser at a phone's size (R15): no device
    /// tools (`install`, what was not found) or no device running (`None`).
    NotTheDevice { platform: String, install: Option<String>, width: u32, height: u32 },
    /// The application runs on `device`, served from `url` (R14).
    OnDevice { device: String, url: String },
    /// `device` could not run it; the browser at a phone's size follows.
    DeviceFailed { device: String, why: String },
    /// A line of the device's log.
    DeviceLog(String),
}

/// What kind of in-place update `app.update` answered (WASM-016 R16).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateKind {
    /// Stylesheets or the theme, every instance and value kept.
    Styles,
    /// A new program module: the application restarted in the same player.
    Program,
    /// New form descriptions: the application restarted in the same player.
    Forms,
    /// The page reloaded (another player, worker or style).
    Reload,
    /// Nothing to take.
    Unchanged,
}

impl UpdateKind {
    /// The kind of `how` (`"stylesheet, stylesheet of MAIN-FORM"`, `"program"`, …).
    pub fn of(how: &str) -> UpdateKind {
        let parts: Vec<&str> = how.split(',').map(str::trim).collect();
        if parts.contains(&"program") {
            UpdateKind::Program
        } else if parts.contains(&"forms") {
            UpdateKind::Forms
        } else if parts.iter().any(|p| *p == "reload" || *p == "unmounted") {
            UpdateKind::Reload
        } else if parts.iter().all(|p| p.is_empty() || *p == "unchanged") {
            UpdateKind::Unchanged
        } else {
            UpdateKind::Styles
        }
    }
}

/// `form: line`, or the line alone when the page could not tell its form.
fn with_form(form: &str, line: String) -> String {
    if form.is_empty() {
        line
    } else {
        format!("{form}: {line}")
    }
}

/// How a note is coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warn,
    Error,
}

/// `template` with each `{}` replaced, in order, by the next of `args`.
fn fill(template: &str, args: &[&str]) -> String {
    let mut out = template.to_owned();
    for a in args {
        out = out.replacen("{}", a, 1);
    }
    out
}

/// A diagnostic of the generator in the language of `tr`: a known kind is said
/// from its `Tr` template with the subject filled in, free text as it is
/// (WASM-010 D6). The location stays in front.
pub fn diagnostic_text(d: &WebDiagnostic, tr: &Tr) -> String {
    // The Build's gate over the project's inputs (WASM-013 T7): an `unsafe` setting is said in the developer's language
    // with its key; a possible secret likewise. A secret that stops the Build keeps the English `file:line: secret (…)`
    // line, like the compiler's own diagnostics.
    if d.kind == WebDiagnosticKind::Other && d.location == "manifest" {
        let key = d.message.split_whitespace().next().unwrap_or_default();
        return fill(if d.message.contains("refuses it") { tr.pk_unsafe_refused } else { tr.pk_unsafe_on }, &[key]);
    }
    if d.kind == WebDiagnosticKind::Other && d.message.starts_with("possible secret (") {
        return fill(tr.pk_scan_possible, &[&format!("{} ({})", d.location, d.subject)]);
    }
    let template = match d.kind {
        WebDiagnosticKind::Other => None,
        WebDiagnosticKind::BackendOnly => Some(tr.wasm_diag_backend_only),
        WebDiagnosticKind::Unsupported => Some(tr.wasm_diag_unsupported),
        WebDiagnosticKind::UnknownLayer => Some(tr.wasm_diag_unknown_layer),
        WebDiagnosticKind::UnknownLayerProperty => Some(tr.wasm_diag_unknown_layer_property),
        WebDiagnosticKind::LayerWrite => Some(tr.wasm_diag_layer_write),
        WebDiagnosticKind::TooManyHostReads => Some(tr.wasm_diag_too_many_reads),
        WebDiagnosticKind::AcceptIgnored => Some(tr.wasm_warn_accept_ignored),
        WebDiagnosticKind::RedefinesMayDiffer => Some(tr.wasm_warn_redefines),
        WebDiagnosticKind::InstancesOutOfRange => Some(tr.msg_instances_out_of_range),
        WebDiagnosticKind::MultiMemoryOnlyWithWebKit => Some(tr.pk_err_multi_memory_only_with_webkit),
        WebDiagnosticKind::SecretOnForm => Some(tr.wasm_diag_secret_on_form),
        WebDiagnosticKind::UnavailableOnWeb => Some(tr.wasm_diag_unavailable_on_web),
        WebDiagnosticKind::NoAccessibleName => Some(tr.wasm_warn_no_accessible_name),
    };
    let text = match template {
        // The second `{}`, where a template has one, is the alternative (WASM-004 R5).
        Some(t) => fill(t, &[&d.subject, &d.alternative]),
        None => d.message.clone(),
    };
    if d.location.is_empty() {
        text
    } else {
        format!("{}: {text}", d.location)
    }
}

/// A size for people: `812 B`, `12.5 KB`, `1.20 MB`.
pub fn human_bytes(bytes: u64) -> String {
    const K: f64 = 1024.0;
    let v = bytes as f64;
    if bytes < 1024 {
        format!("{bytes} B")
    } else if v < K * K {
        format!("{:.1} KB", v / K)
    } else {
        format!("{:.2} MB", v / (K * K))
    }
}

impl Note {
    pub fn severity(&self) -> Severity {
        match self {
            Note::BuildFailed(_)
            | Note::BuildRefused(_)
            | Note::PlayerPin(_)
            | Note::ServerFailed(_)
            | Note::PageError { .. }
            | Note::CobolError { .. }
            | Note::BackendFailed { .. }
            | Note::BackendTimeout { .. } => {
                Severity::Error
            }
            Note::Console { level, .. } if level == "error" => Severity::Error,
            Note::NotLinked
            | Note::NoBundle
            | Note::BuildWarning(_)
            | Note::BrowserFailed { .. }
            | Note::BrowserNotConfirmed { .. }
            | Note::BackendOff
            | Note::BackendPortBusy { .. }
            | Note::BackendTaken { .. }
            | Note::BackendMismatch { .. }
            | Note::TokenUnused { .. }
            | Note::DeviceFailed { .. } => Severity::Warn,
            Note::Console { level, .. } if level == "warn" => Severity::Warn,
            _ => Severity::Info,
        }
    }

    /// The note, one line at a time, in the language of `tr`.
    pub fn lines(&self, tr: &Tr) -> Vec<String> {
        match self {
            Note::Building => vec![tr.pk_prev_building.to_owned()],
            Note::NotLinked => vec![tr.pk_build_not_linked.to_owned()],
            Note::BuildDone { ms, bytes, files, startup_ms } => {
                let startup = startup_ms.map_or_else(|| tr.pk_build_startup_none.to_owned(), |v| format!("{v:.0} ms"));
                vec![fill(tr.pk_build_done, &[&ms.to_string(), &human_bytes(*bytes), &files.to_string(), &startup])]
            }
            // The budget warning and the missing player are English report
            // lines of the Build; the heading above them is the developer's
            // language (WASM-002 T21).
            Note::BuildWarning(w) if w.message.starts_with(cobolt_project_tools::web::OVER_BUDGET_PREFIX) => {
                vec![tr.web_build_over_budget.to_owned(), format!("  {}", w.message)]
            }
            Note::BuildWarning(w) => vec![fill(tr.pk_build_warning, &[&diagnostic_text(w, tr)])],
            Note::BuildRefused(lines) => {
                let secret = lines.iter().any(|l| l.kind == WebDiagnosticKind::Other && l.message.starts_with("secret ("));
                let unsafe_only = lines.iter().all(|l| l.location == "manifest");
                let mut out = if unsafe_only { Vec::new() } else { vec![(if secret { tr.pk_scan_failed } else { tr.pk_build_refused }).to_owned()] };
                out.extend(lines.iter().map(|l| format!("  {}", diagnostic_text(l, tr))));
                out
            }
            Note::BuildFailed(why) if why.starts_with(cobolt_project_tools::web::PLAYER_UNAVAILABLE_PREFIX) => {
                vec![tr.web_build_player_unavailable.to_owned(), format!("  {why}")]
            }
            Note::BuildFailed(why) => vec![fill(tr.pk_build_failed, &[why])],
            Note::PlayerSizes(sizes) => {
                let list = |parts: bool| {
                    sizes.iter().filter(|p| p.name.starts_with("parts/") == parts).map(|p| format!("{} {}", p.name.trim_start_matches("parts/"), human_bytes(p.bytes))).collect::<Vec<_>>().join(", ")
                };
                let (variants, parts) = (list(false), list(true));
                let mut out = vec![fill(tr.web_build_player_sizes, &[&variants])];
                if !parts.is_empty() {
                    out.push(fill(tr.web_build_player_parts, &[&parts]));
                }
                out
            }
            Note::PlayerPin(PlayerPinError::NotKept { pinned, kept }) => vec![fill(tr.web_build_pin_not_kept, &[pinned, &kept.join(", ")])],
            Note::PlayerPin(PlayerPinError::CannotEmit { pinned, oldest: Some(oldest), .. }) => {
                vec![fill(tr.web_build_pin_cannot_emit, &[pinned, oldest])]
            }
            Note::PlayerPin(PlayerPinError::CannotEmit { pinned, oldest: None, .. }) => vec![fill(tr.web_build_pin_cannot_emit_none, &[pinned])],
            Note::PackageNote => vec![tr.pk_build_package_note.to_owned()],
            Note::NoBundle => vec![tr.pk_prev_no_bundle.to_owned()],
            Note::Serving { name, url } => vec![fill(tr.pk_prev_serving, &[name, url])],
            Note::ServerFailed(why) => vec![fill(tr.pk_prev_server_failed, &[why])],
            Note::Opened { browser, url } if browser.is_empty() => vec![fill(tr.pk_prev_opened_default, &[url])],
            Note::Opened { browser, url } => vec![fill(tr.pk_prev_opened_in, &[url, browser])],
            Note::BrowserFailed { name, why } => vec![fill(tr.pk_prev_browser_failed, &[name, why])],
            Note::BrowserNotConfirmed { path } => vec![fill(tr.pk_prev_browser_not_confirmed, &[path])],
            Note::Updated { ms } => vec![fill(tr.pk_prev_updated, &[&ms.to_string()])],
            Note::BackendOff => vec![tr.pk_prev_backend_off.to_owned()],
            Note::BackendStarting { listen, secs } => vec![fill(tr.pk_prev_backend_starting, &[listen, &secs.to_string()])],
            Note::BackendReady { url } => vec![fill(tr.pk_prev_backend_ready, &[url])],
            Note::BackendPortBusy { port } => vec![fill(tr.pk_prev_backend_port_busy, &[&port.to_string()])],
            Note::BackendTaken { url } => vec![fill(tr.pk_prev_backend_taken, &[url])],
            Note::BackendRemote { url } => vec![fill(tr.pk_prev_backend_remote, &[url])],
            Note::BackendMismatch { running, page } => vec![fill(tr.pk_prev_backend_mismatch, &[running, page])],
            Note::BackendFailed { code, why } => {
                let code = code.map_or_else(|| "-".to_owned(), |c| c.to_string());
                let mut out = vec![fill(tr.pk_prev_backend_failed, &[&code])];
                out.extend(why.iter().map(|l| format!("  {l}")));
                out
            }
            Note::BackendTimeout { secs } => vec![fill(tr.pk_prev_backend_timeout, &[&secs.to_string()])],
            Note::Console { level, text, form } => vec![with_form(form, fill(tr.pk_prev_console, &[level, text]))],
            Note::PageError { message, source, line, form } => {
                vec![with_form(form, fill(tr.pk_prev_error, &[message, source, &line.to_string()]))]
            }
            Note::CobolError { form, unit, line, code, message } => {
                vec![with_form(form, fill(tr.pk_prev_cobol_error, &[code, message, unit, &line.to_string()]))]
            }
            Note::PageUpdated { how, ms } => {
                let ms = format!("{ms:.1}");
                vec![match UpdateKind::of(how) {
                    UpdateKind::Styles => fill(tr.pk_prev_upd_styles, &[&ms, how]),
                    UpdateKind::Program => fill(tr.pk_prev_upd_program, &[&ms]),
                    UpdateKind::Forms => fill(tr.pk_prev_upd_forms, &[&ms]),
                    UpdateKind::Reload => fill(tr.pk_prev_upd_reload, &[&ms]),
                    UpdateKind::Unchanged => fill(tr.pk_prev_upd_unchanged, &[&ms]),
                }]
            }
            Note::PreviewStopped => vec![tr.pk_prev_stopped.to_owned()],
            Note::TokenUnused { url } => vec![tr.pk_prev_token_unused.replacen("{}", url, 1)],
            Note::NotTheDevice { platform, install, width, height } => vec![
                match install {
                    Some(tool) => tr.pk_prev_not_device_tools.replacen("{}", platform, 1).replacen("{}", tool, 1),
                    None => tr.pk_prev_not_device_none.replacen("{}", platform, 1),
                },
                tr.pk_prev_phone_status.replacen("{}", &width.to_string(), 1).replacen("{}", &height.to_string(), 1),
            ],
            Note::OnDevice { device, url } => vec![tr.pk_prev_on_device.replacen("{}", device, 1).replacen("{}", url, 1)],
            Note::DeviceFailed { device, why } => vec![tr.pk_prev_device_failed.replacen("{}", device, 1).replacen("{}", why, 1)],
            Note::DeviceLog(line) => vec![line.clone()],
        }
    }

    /// The note as one piece of text.
    pub fn text(&self, tr: &Tr) -> String {
        self.lines(tr).join("\n")
    }
}

/// Is `line` a token (a JWT: `eyJ…`, three dotted parts, no space)? The
/// development token `rcrun backend run` prints is never kept or repeated.
pub fn looks_like_a_token(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("eyJ") && t.split('.').count() == 3 && !t.contains(' ')
}

/// The texts the served page shows, in the developer's language (WASM-008 §2.2).
pub fn page_strings(tr: &Tr) -> PageStrings {
    PageStrings {
        stopped: tr.pk_page_stopped.to_owned(),
        errors_title: tr.pk_page_errors_title.to_owned(),
        dismiss: tr.pk_page_dismiss.to_owned(),
        not_the_device: tr.pk_page_not_device.to_owned(),
    }
}

// ── The watcher ─────────────────────────────────────────────────────────────

/// Watches the folders a build reads — the forms (and with them their
/// stylesheets), Common Code, the Backend and the Assets — and the manifest.
///
/// It asks the file system for the paths, sizes and modification times and
/// nothing else, and it waits for the folders to be still for one look before
/// it reports a change, so a save that writes several files is one rebuild.
#[derive(Debug)]
pub struct SourceWatcher {
    roots: Vec<PathBuf>,
    manifest: PathBuf,
    last: u64,
    checked: Option<Instant>,
    pending: Option<Instant>,
}

impl SourceWatcher {
    pub fn new(manifest: &Path, kind: ProjectKind, has_backend: bool) -> Self {
        let dir = manifest.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut folders = vec![kind.forms_folder(), "src", "Assets"];
        if has_backend {
            folders.push(cobolt_compiler::project_kind::BACKEND_FOLDER);
        }
        let mut w = Self {
            roots: folders.into_iter().map(|f| dir.join(f)).collect(),
            manifest: manifest.to_path_buf(),
            last: 0,
            checked: None,
            pending: None,
        };
        w.last = w.fingerprint();
        w
    }

    fn fingerprint(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for r in &self.roots {
            devserver::fingerprint(r).hash(&mut h);
        }
        if let Ok(m) = std::fs::metadata(&self.manifest) {
            m.len().hash(&mut h);
            if let Ok(t) = m.modified().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).map_err(std::io::Error::other)) {
                t.as_nanos().hash(&mut h);
            }
        }
        h.finish()
    }

    /// Look at the folders if [`WATCH_EVERY`] has passed since the last look.
    /// `Some(when)` — when the change was first seen — once they have changed
    /// and been still for one look.
    pub fn poll(&mut self, now: Instant) -> Option<Instant> {
        if self.checked.is_some_and(|c| now.saturating_duration_since(c) < WATCH_EVERY) {
            return None;
        }
        self.checked = Some(now);
        let fp = self.fingerprint();
        if fp != self.last {
            self.last = fp;
            self.pending.get_or_insert(now);
            return None;
        }
        self.pending.take()
    }

    /// Take the folders as they are now as seen, so a write the Preview has
    /// already shown (WASM-007 T23's stylesheet) does not rebuild. Not while a
    /// change is pending: that one still rebuilds, and the write with it.
    pub fn absorb(&mut self) {
        if self.pending.is_none() {
            self.last = self.fingerprint();
        }
    }
}

// ── Devices (WASM-008 R14, D12) ─────────────────────────────────────────────

/// The seam to a device or an emulator: a Mobile Preview runs there first.
/// WASM-012's `mobile_toolchain` / `mobile_devices` implement it; until then
/// [`NoDeviceTools`] is the only one, and every Mobile Preview takes the
/// browser at a phone's size.
pub trait DeviceRunner: Send + Sync {
    /// `Err(tool)`: what is not on this machine to run on `platform`.
    fn detect(&self, platform: MobilePlatform) -> Result<(), String>;
    /// The devices and running emulators of `platform`, the first one used.
    fn list(&self, platform: MobilePlatform) -> Vec<String>;
    /// Run the application served at `dev_url` on `device`.
    fn run(&self, device: &str, dev_url: &str) -> Result<(), String>;
    /// The device's log lines since the last call.
    fn log(&self) -> Vec<String>;
}

/// No device tools on this machine (until WASM-012).
pub struct NoDeviceTools;

impl DeviceRunner for NoDeviceTools {
    fn detect(&self, platform: MobilePlatform) -> Result<(), String> {
        Err(device_tool(platform).to_owned())
    }

    fn list(&self, _platform: MobilePlatform) -> Vec<String> {
        Vec::new()
    }

    fn run(&self, _device: &str, _dev_url: &str) -> Result<(), String> {
        Err(String::new())
    }

    fn log(&self) -> Vec<String> {
        Vec::new()
    }
}

/// The tool a platform's devices are reached with (a name, not a sentence).
pub fn device_tool(platform: MobilePlatform) -> &'static str {
    match platform {
        MobilePlatform::Android => "adb",
        MobilePlatform::Ios => "xcrun simctl",
    }
}

/// The platform's name, as its maker writes it.
fn platform_name(platform: MobilePlatform) -> &'static str {
    match platform {
        MobilePlatform::Android => "Android",
        MobilePlatform::Ios => "iOS",
    }
}

// ── The Preview ─────────────────────────────────────────────────────────────

/// What a start asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// Build the bundle and say how it went.
    Build,
    /// Build, serve and open the page in the browser — Preview and Run alike
    /// (WASM-008 R12; D7: two actions, the same page).
    Show,
}

/// What the Preview needs to know about the project it was started for.
#[derive(Clone)]
pub struct Launch {
    pub manifest: PathBuf,
    /// The project's name, for the *Serving* note.
    pub name: String,
    pub kind: ProjectKind,
    /// A Mobile project's `[mobile]` platform, device and orientation.
    pub platform: Option<MobilePlatform>,
    pub device: Option<DeviceClass>,
    pub orientation: Option<Orientation>,
    /// Where a Mobile Preview runs first (R14).
    pub devices: Arc<dyn DeviceRunner>,
    pub has_backend: bool,
    pub backend: Arc<dyn WebBuildBackend>,
    /// The browser the project names (`preview_browser`, WASM-008 R12, Q7).
    pub browser: BrowserChoice,
    /// A [`BrowserChoice::Path`] the developer confirmed on this machine for
    /// this project (WASM-008 D11); an unconfirmed path is never started.
    pub path_confirmed: bool,
    /// The texts the served page shows, in the developer's language.
    pub page_strings: PageStrings,
    /// The developer's language, for a failed build's lines over the page (R7).
    pub language: Language,
    /// The Backend address the page is built for instead of `[web] backend_url`:
    /// the one the Backend the Preview started reports (the manifest names none).
    pub backend_url: Option<String>,
}

enum JobKind {
    Initial(Intent),
    /// A rebuild after the sources changed at this moment.
    Rebuild(Instant),
}

struct Done {
    outcome: Result<BuildReport, WebBuildError>,
    bundle: Option<PathBuf>,
    isolation: Isolation,
}

struct Job {
    rx: Receiver<Done>,
    kind: JobKind,
}

/// A start waiting for the project's Backend to say where it listens.
struct AwaitBackend {
    intent: Intent,
    deadline: Instant,
    /// Build the page for the address the Backend reports.
    bake: bool,
    /// `[web] backend_url`, to compare with where the Backend listens.
    manifest_url: String,
    /// Its last lines, said when it fails.
    tail: Vec<String>,
}

struct Live {
    server: ServerHandle,
    watcher: SourceWatcher,
}

/// The web Preview of the open project. One at a time.
#[derive(Default)]
pub struct WebPreview {
    launch: Option<Launch>,
    job: Option<Job>,
    live: Option<Live>,
    backend_wait: Option<AwaitBackend>,
    /// A change seen while a build was running: build again when it ends.
    rebuild_after: Option<Instant>,
    notes: Vec<Note>,
    outbox: Vec<Note>,
    /// Where the page was last opened, and in what (the launcher's decision).
    opened: Option<(String, BrowserChoice)>,
    /// The address of the Backend this Preview runs against (it said Ready).
    backend_for: Option<String>,
    /// That Backend's address and token, once both were printed: put into
    /// the served `config.json`, never into the bundle (WASM-008 R6).
    dev_backend: Option<DevBackend>,
    /// The application runs on a device: its log is read each poll.
    on_device: bool,
}

impl WebPreview {
    /// Is a build running?
    pub fn building(&self) -> bool {
        self.job.is_some()
    }

    /// Is a bundle being served?
    pub fn serving(&self) -> bool {
        self.live.is_some()
    }

    /// Is there anything running, building, serving or waiting for the Backend?
    pub fn active(&self) -> bool {
        self.job.is_some() || self.live.is_some() || self.backend_wait.is_some()
    }

    /// Is a start waiting for the Backend to listen?
    pub fn awaiting_backend(&self) -> bool {
        self.backend_wait.is_some()
    }

    /// The Backend address the page is built for, when it is not the manifest's.
    pub fn baked_backend_url(&self) -> Option<&str> {
        self.launch.as_ref().and_then(|l| l.backend_url.as_deref())
    }

    /// The address being served.
    pub fn url(&self) -> Option<String> {
        self.live.as_ref().map(|l| l.server.url())
    }

    /// The address last opened and the browser it was opened in.
    pub fn opened(&self) -> Option<&(String, BrowserChoice)> {
        self.opened.as_ref()
    }

    /// The notes of this Preview, oldest first.
    pub fn notes(&self) -> &[Note] {
        &self.notes
    }

    /// The notes not yet handed to the Output panel.
    pub fn take_outbox(&mut self) -> Vec<Note> {
        std::mem::take(&mut self.outbox)
    }

    fn say(&mut self, note: Note) {
        self.outbox.push(note.clone());
        self.notes.push(note);
        if self.notes.len() > MAX_NOTES {
            let drop = self.notes.len() - MAX_NOTES;
            self.notes.drain(..drop);
        }
    }

    /// Start: build, then (for [`Intent::Show`]) serve and open. `false` when a
    /// build is already running. A Preview of the same project that is already
    /// serving is **reused**: the same address is opened again, so a tab the
    /// developer has open keeps working (WASM-008 §1 R12).
    pub fn start(&mut self, launch: Launch, intent: Intent) -> bool {
        if self.job.is_some() || self.backend_wait.is_some() {
            return false;
        }
        if self.reuse(&launch, intent) {
            return true;
        }
        self.begin(intent);
        self.build(launch, intent);
        true
    }

    /// Open the page being served again, when `launch` is the project it serves.
    fn reuse(&mut self, launch: &Launch, intent: Intent) -> bool {
        if intent != Intent::Show || self.live.is_none() {
            return false;
        }
        let same = self.launch.as_ref().is_some_and(|l| l.manifest == launch.manifest);
        if !same {
            return false;
        }
        // The browser setting may have changed since; the server and the watcher stay.
        if let Some(l) = self.launch.as_mut() {
            l.browser = launch.browser.clone();
            l.path_confirmed = launch.path_confirmed;
            l.page_strings = launch.page_strings.clone();
            l.language = launch.language;
        }
        if let Some(live) = &self.live {
            live.server.set_page_strings(launch.page_strings.clone());
        }
        self.open_page();
        true
    }

    /// A start that shows the page is a new Preview: the old one goes.
    fn begin(&mut self, intent: Intent) {
        if intent != Intent::Build {
            // A Preview of another project is replaced, not stacked.
            self.live = None;
            self.notes.clear();
            self.opened = None;
            self.backend_for = None;
            self.dev_backend = None;
            self.on_device = false;
        }
    }

    fn build(&mut self, launch: Launch, intent: Intent) {
        self.say(Note::Building);
        self.spawn_build(&launch, JobKind::Initial(intent));
        self.launch = Some(launch);
    }

    /// Start a Preview of a project with a Backend (WASM-008 R6): the Backend
    /// first, then the build, so that the page is built for the address the
    /// Backend really listens on ([`devbackend::plan`] decides it). The IDE's
    /// one Backend is `ide_backend`: when it is already running for this
    /// project it is used as it is; otherwise the Preview starts it there and
    /// waits — for at most [`BACKEND_WAIT`] — for its address, which
    /// [`WebPreview::pump_backend`] hands over. A start that only builds, or a
    /// project without a Backend, is [`WebPreview::start`].
    ///
    /// `None` when a start is already under way; else whether a Backend was
    /// started now (the caller stops it with the Preview).
    pub fn start_with_backend(
        &mut self,
        mut launch: Launch,
        intent: Intent,
        ide_backend: &mut Option<BackendRun>,
        rcrun: &Path,
    ) -> Option<bool> {
        if self.job.is_some() || self.backend_wait.is_some() {
            return None;
        }
        let project_dir = launch.manifest.parent().map(Path::to_path_buf).unwrap_or_default();
        let exports = project_dir.join(cobolt_compiler::project_kind::BACKEND_FOLDER).join("exports.toml");
        if self.reuse(&launch, intent) {
            return Some(false);
        }
        if intent == Intent::Build || !exports.is_file() {
            return self.start(launch, intent).then_some(false);
        }
        let manifest_url = WebBuildRequest::from_manifest(&launch.manifest).map(|r| r.web.backend_url).unwrap_or_default();
        let plan = devbackend::plan(&manifest_url, devbackend::configured_listen(&project_dir).as_deref(), devbackend::loopback_port_is_free);
        launch.backend_url = None;
        self.begin(intent);
        let (listen, bake, busy_port) = match plan {
            DevBackendPlan::Remote { url } => {
                self.say(Note::BackendRemote { url });
                self.build(launch, intent);
                return Some(false);
            }
            DevBackendPlan::Taken { url } => {
                self.say(Note::BackendTaken { url });
                self.build(launch, intent);
                return Some(false);
            }
            DevBackendPlan::Start { listen, bake_reported, busy_port } => (listen, bake_reported, busy_port),
        };
        let wait = |intent| AwaitBackend { intent, deadline: Instant::now() + BACKEND_WAIT, bake, manifest_url: manifest_url.clone(), tail: Vec::new() };
        // The Backend the IDE already runs for this project is used as it is.
        let same = |run: &BackendRun| {
            run.is_running() && std::fs::canonicalize(&run.project_dir).ok() == std::fs::canonicalize(&project_dir).ok()
        };
        if let Some(run) = ide_backend.as_ref().filter(|r| same(r)) {
            match run.url().map(str::to_owned) {
                Some(url) => self.listening(launch, wait(intent), &url),
                None => {
                    self.say(Note::BackendStarting { listen, secs: BACKEND_WAIT.as_secs() });
                    self.backend_wait = Some(wait(intent));
                    self.launch = Some(launch);
                }
            }
            return Some(false);
        }
        if let Some(port) = busy_port {
            self.say(Note::BackendPortBusy { port });
        }
        match BackendRun::spawn_with(rcrun, &project_dir, Some(&listen)) {
            Ok(run) => {
                if let Some(mut other) = ide_backend.replace(run) {
                    other.stop();
                }
                self.say(Note::BackendStarting { listen, secs: BACKEND_WAIT.as_secs() });
                self.backend_wait = Some(wait(intent));
                self.launch = Some(launch);
                Some(true)
            }
            Err(e) => {
                self.say(Note::BackendFailed { code: None, why: vec![format!("{}: {e}", rcrun.display())] });
                Some(false)
            }
        }
    }

    /// The Backend listens on `url`: build the page for it (or for the
    /// manifest's address, which is then compared with it).
    fn listening(&mut self, mut launch: Launch, wait: AwaitBackend, url: &str) {
        if wait.bake {
            launch.backend_url = Some(url.to_owned());
        }
        self.backend_for = Some(url.to_owned());
        self.say(Note::BackendReady { url: url.to_owned() });
        if !wait.bake && devbackend::origin(&wait.manifest_url) != devbackend::origin(url) {
            self.say(Note::BackendMismatch { running: url.to_owned(), page: wait.manifest_url.clone() });
        }
        self.build(launch, wait.intent);
    }

    /// Once a frame: read what the IDE's Backend printed, hand the Preview
    /// what it waits for (the address, or the end and its last lines), and
    /// give the events back for the Output panel.
    pub fn pump_backend(&mut self, ide_backend: &mut Option<BackendRun>) -> Vec<BackendEvent> {
        let Some(run) = ide_backend.as_mut() else { return Vec::new() };
        let events = run.poll();
        for event in &events {
            self.on_backend_event(event);
        }
        // The token is printed after the address, maybe in a later poll.
        let dev = match (&self.backend_for, run.is_running(), run.url(), run.token()) {
            (Some(ours), true, Some(url), Some(token)) if ours == url => Some(DevBackend { url: url.to_owned(), token: token.to_owned() }),
            _ => None,
        };
        if dev != self.dev_backend {
            self.dev_backend = dev;
            if let Some(live) = &self.live {
                live.server.set_dev_backend(self.dev_backend.clone());
            }
            // Never silent: calls will be refused until the player sends the token.
            if let Some(d) = &self.dev_backend {
                self.say(Note::TokenUnused { url: d.url.clone() });
            }
        }
        events
    }

    /// One thing the IDE's Backend did.
    pub fn on_backend_event(&mut self, event: &BackendEvent) {
        let Some(wait) = self.backend_wait.as_mut() else { return };
        match event {
            BackendEvent::Line(line) => {
                // Never a token, even in a failure's text: a JWT, `eyJ…` in three dotted parts and no space.
                let token = looks_like_a_token(line);
                if !line.trim().is_empty() && !token {
                    wait.tail.push(line.clone());
                    if wait.tail.len() > BACKEND_TAIL {
                        wait.tail.remove(0);
                    }
                }
            }
            BackendEvent::Listening(url) => {
                let wait = self.backend_wait.take().expect("checked above");
                if let Some(launch) = self.launch.clone() {
                    self.listening(launch, wait, url);
                }
            }
            BackendEvent::Exited(code) => {
                let wait = self.backend_wait.take().expect("checked above");
                self.say(Note::BackendFailed { code: *code, why: wait.tail });
            }
        }
    }

    fn spawn_build(&mut self, launch: &Launch, kind: JobKind) {
        let (tx, rx) = mpsc::channel();
        let backend = launch.backend.clone();
        let manifest = launch.manifest.clone();
        let backend_url = launch.backend_url.clone();
        let spawned = std::thread::Builder::new().name("web-build".into()).spawn(move || {
            let done = match WebBuildRequest::from_manifest(&manifest) {
                Err(e) => Done { outcome: Err(WebBuildError::Failed(e)), bundle: None, isolation: Isolation::Off },
                Ok(mut request) => Done {
                    bundle: Some(backend.bundle_dir(&request)),
                    isolation: request.web.isolation,
                    outcome: {
                        // The player shipped beside this IDE (WASM-016 T14), not a path of the machine that built it.
                        request.player_dir = Some(crate::app::web_player_dir());
                        // The address of the Backend this Preview started (the manifest names none).
                        if let Some(url) = backend_url {
                            request.web.backend_url = url;
                        }
                        backend.build(&request)
                    },
                },
            };
            let _ = tx.send(done);
        });
        match spawned {
            Ok(_) => {
                // The page being served is told of this build once, at its end, never in mid-write (WASM-008 T47).
                if let Some(live) = &self.live {
                    live.server.build_started();
                }
                self.job = Some(Job { rx, kind });
            }
            Err(e) => self.say(Note::BuildFailed(e.to_string())),
        }
    }

    /// End everything: the server and the watcher. A build that is still
    /// running finishes on its thread and its result is dropped. A browser tab
    /// left open says, at its next polls, that the Preview has stopped.
    pub fn stop(&mut self) {
        if self.live.take().is_some() {
            self.say(Note::PreviewStopped);
        }
        self.job = None;
        self.backend_wait = None;
        self.rebuild_after = None;
        self.backend_for = None;
        self.dev_backend = None;
        self.on_device = false;
    }

    /// Close the Preview (Stop, another project, the IDE closing).
    pub fn close(&mut self) {
        self.stop();
    }

    /// Look at everything that moves: the build, the web view's events, the
    /// sources. `true` while the Preview wants another frame soon.
    pub fn poll(&mut self, now: Instant) -> bool {
        if self.backend_wait.as_ref().is_some_and(|w| now >= w.deadline) {
            self.backend_wait = None;
            self.say(Note::BackendTimeout { secs: BACKEND_WAIT.as_secs() });
        }
        self.poll_job();
        self.poll_events();
        self.poll_device();
        let changed = self.live.as_mut().and_then(|l| l.watcher.poll(now));
        if let Some(when) = changed {
            self.rebuild_after.get_or_insert(when);
        }
        if self.job.is_none() {
            if let Some(changed) = self.rebuild_after.take() {
                if let Some(launch) = self.launch.clone() {
                    self.spawn_build(&launch, JobKind::Rebuild(changed));
                }
            }
        }
        self.active()
    }

    fn poll_job(&mut self) {
        let Some(job) = self.job.as_ref() else { return };
        let done = match job.rx.try_recv() {
            Ok(done) => done,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Done {
                outcome: Err(WebBuildError::Failed("the build stopped unexpectedly".to_owned())),
                bundle: None,
                isolation: Isolation::Off,
            },
        };
        let Some(Job { kind, .. }) = self.job.take() else { return };
        self.finish(kind, done);
    }

    fn finish(&mut self, kind: JobKind, done: Done) {
        let mut built = false;
        let mut not_linked = false;
        match done.outcome {
            Ok(report) => {
                built = true;
                self.say(Note::BuildDone {
                    ms: report.duration_ms,
                    bytes: report.size.total_bytes,
                    files: report.size.parts.len(),
                    startup_ms: report.startup_ms,
                });
                if !report.player.is_empty() {
                    self.say(Note::PlayerSizes(report.player));
                }
                for w in report.warnings {
                    self.say(Note::BuildWarning(w));
                }
            }
            Err(WebBuildError::NotYetLinked) => {
                not_linked = true;
                self.say(Note::NotLinked);
            }
            Err(WebBuildError::Refused(lines)) => self.say(Note::BuildRefused(lines)),
            Err(WebBuildError::Failed(why)) => self.say(Note::BuildFailed(why)),
            Err(WebBuildError::PlayerPin(e)) => self.say(Note::PlayerPin(e)),
        }
        // R7, AC5: the page keeps the last good bundle (the generator replaces
        // it whole, or not at all) and shows a failed build's lines over it;
        // the next good build takes them away.
        if let (JobKind::Rebuild(_), Some(live)) = (&kind, &self.live) {
            let errors = match (built || not_linked, self.notes.last(), self.launch.as_ref()) {
                (false, Some(note), Some(launch)) => note.lines(&launch.language.tr()),
                _ => Vec::new(),
            };
            live.server.set_build_errors(errors);
        }
        // The build's end is the one change the served page sees (WASM-008 T47).
        if let Some(live) = &self.live {
            if built {
                live.server.notify_changed();
            } else {
                live.server.build_failed();
            }
        }
        match kind {
            // A Build ends here; a Mobile one says where it stops.
            JobKind::Initial(Intent::Build) => {
                if built && self.launch.as_ref().is_some_and(|l| l.kind == ProjectKind::Mobile) {
                    self.say(Note::PackageNote);
                }
            }
            JobKind::Initial(Intent::Show) => {
                // A build that worked is shown. So is the bundle that is already
                // there when no generator is linked — it may have been made
                // somewhere else. A build that failed shows nothing: an old page
                // under a new error would mislead.
                match done.bundle {
                    Some(bundle) if built || not_linked => self.serve(&bundle, done.isolation),
                    Some(_) => {}
                    None => self.say(Note::NoBundle),
                }
            }
            JobKind::Rebuild(changed) => {
                if built {
                    let ms = Instant::now().saturating_duration_since(changed).as_millis().min(u128::from(u64::MAX)) as u64;
                    self.say(Note::Updated { ms });
                }
            }
        }
    }

    fn serve(&mut self, bundle: &Path, isolation: Isolation) {
        let Some(launch) = self.launch.clone() else { return };
        if !bundle.join("index.html").is_file() {
            self.say(Note::NoBundle);
            return;
        }
        let options = ServeOptions { port: 0, isolation, live_reload: true };
        let server = match launch.backend.serve(bundle, &options) {
            Ok(s) => s,
            Err(e) => {
                self.say(Note::ServerFailed(e.to_string()));
                return;
            }
        };
        server.set_page_strings(launch.page_strings.clone());
        server.set_dev_backend(self.dev_backend.clone());
        self.say(Note::Serving { name: launch.name.clone(), url: server.url() });
        // A Backend this Preview did not arrange (a start without one) is said to be off.
        if launch.has_backend && !self.notes.iter().any(|n| matches!(n, Note::BackendReady { .. } | Note::BackendTaken { .. } | Note::BackendRemote { .. })) {
            self.say(Note::BackendOff);
        }
        let watcher = SourceWatcher::new(&launch.manifest, launch.kind, launch.has_backend);
        self.live = Some(Live { server, watcher });
        self.open_page();
    }

    /// Open the served page in the project's browser (WASM-008 R12): a named
    /// one or a confirmed path, else — and whenever that one cannot be
    /// started — the system's default, said in the Output panel.
    fn open_page(&mut self) {
        let Some(mut url) = self.url() else { return };
        if self.launch.as_ref().is_some_and(|l| l.kind == ProjectKind::Mobile) {
            match self.open_on_device(&url) {
                Some(phone) => url = phone,
                None => return,
            }
        }
        let Some(launch) = self.launch.as_ref() else { return };
        let mut choice = launch.browser.clone();
        if let BrowserChoice::Path(p) = &choice {
            if !launch.path_confirmed {
                self.say(Note::BrowserNotConfirmed { path: p.display().to_string() });
                choice = BrowserChoice::System;
            }
        }
        if choice != BrowserChoice::System {
            match preview::open_in_browser(&url, &choice) {
                Ok(()) => {
                    self.say(Note::Opened { browser: choice.label(), url: url.clone() });
                    self.opened = Some((url, choice));
                    return;
                }
                Err(why) => {
                    self.say(Note::BrowserFailed { name: choice.label(), why });
                    choice = BrowserChoice::System;
                }
            }
        }
        match preview::open_in_browser(&url, &choice) {
            Ok(()) => {
                self.say(Note::Opened { browser: String::new(), url: url.clone() });
                self.opened = Some((url, choice));
            }
            Err(why) => self.say(Note::BrowserFailed { name: String::new(), why }),
        }
    }

    /// A Mobile Preview (R14, R15, D12): the first device or running emulator
    /// runs the application (`None`); without one, the phone page is set up at
    /// `[mobile] device` and `orientation` and its address is answered, to be
    /// opened in the project's browser, and the Output panel says why.
    fn open_on_device(&mut self, url: &str) -> Option<String> {
        let launch = self.launch.as_ref()?;
        let platform = launch.platform.unwrap_or(MobilePlatform::Android);
        let devices = launch.devices.clone();
        let landscape = launch.orientation == Some(Orientation::Landscape);
        let view = presets::default_for(ProjectKind::Mobile, launch.device).viewport(landscape);
        let install = devices.detect(platform).err();
        if install.is_none() {
            if let Some(device) = devices.list(platform).into_iter().next() {
                match devices.run(&device, url) {
                    Ok(()) => {
                        self.on_device = true;
                        self.say(Note::OnDevice { device, url: url.to_owned() });
                        return None;
                    }
                    Err(why) => self.say(Note::DeviceFailed { device, why }),
                }
            }
        }
        let live = self.live.as_ref()?;
        live.server.set_phone(Some(PhoneFrame { width: view.width, height: view.height }));
        let phone = live.server.phone_url();
        self.say(Note::NotTheDevice { platform: platform_name(platform).to_owned(), install, width: view.width, height: view.height });
        Some(phone)
    }

    /// The device's log, at most [`CHATTER_PER_POLL`] lines a poll.
    fn poll_device(&mut self) {
        if !self.on_device {
            return;
        }
        let Some(devices) = self.launch.as_ref().map(|l| l.devices.clone()) else { return };
        for line in devices.log().into_iter().take(CHATTER_PER_POLL) {
            self.say(Note::DeviceLog(line));
        }
    }

    /// What the served page reported (WASM-008 R13, R5, R3): its console
    /// lines, uncaught errors, COBOL run-time errors and in-place update
    /// times, each with its form; at most [`CHATTER_PER_POLL`] a poll, the
    /// rest of a burst dropped.
    fn poll_events(&mut self) {
        let events = match self.live.as_ref() {
            Some(l) => l.server.take_page_events(),
            None => return,
        };
        for event in events.into_iter().take(CHATTER_PER_POLL) {
            self.say(match event {
                PageEvent::Console { level, text, form } => Note::Console { level, text, form },
                PageEvent::Error { message, source, line, form } => Note::PageError { message, source, line, form },
                PageEvent::CobolError { form, unit, line, code, message } => Note::CobolError { form, unit, line, code, message },
                PageEvent::Updated { how, ms, .. } => Note::PageUpdated { how, ms },
            });
        }
    }

    /// The CSS-only fast path (WASM-007 R11, T23; the operator's decision of
    /// 2026-10-31): the stylesheet window's Apply has written form `form`'s
    /// sheet to the project; put it straight into the bundle being served and
    /// count the change, with no Build and no watcher wait, so the open page
    /// takes it at its next poll. `false` when nothing is served, a build is
    /// running, or the sheet would be refused — the normal Build then follows
    /// the write and reports what it finds.
    pub fn apply_form_stylesheet(&mut self, form: &str, css: &str, layers: &[String]) -> bool {
        let started = Instant::now();
        if self.job.is_some() || self.rebuild_after.is_some() {
            return false;
        }
        let Some(live) = self.live.as_mut() else { return false };
        if live.server.apply_form_stylesheet(form, css, layers).is_err() {
            return false;
        }
        live.watcher.absorb();
        let ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        self.say(Note::Updated { ms });
        true
    }
}

/// What other modules' tests use to drive a Preview (the settings' path confirmation).
#[cfg(test)]
pub(crate) mod tests_support {
    pub(crate) use super::tests::open_with;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;
    use cobolt_project_tools::web::BundleSize;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("prc-ide-preview-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const MANIFEST: &str = "[project]\nname = \"P.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n\
        [files]\nforms = [\"webforms/main-form.cfrm\"]\n\n[web]\nmulti_memory = true\n";

    /// A project folder with a manifest and one form.
    fn project(tag: &str) -> PathBuf {
        let dir = scratch(tag);
        std::fs::create_dir_all(dir.join("webforms")).unwrap();
        std::fs::write(dir.join("webforms/main-form.cfrm"), "<Form/>").unwrap();
        std::fs::write(dir.join("P.project.toml"), MANIFEST).unwrap();
        dir.join("P.project.toml")
    }

    /// A generator that writes a tiny bundle and counts its builds.
    struct Fake {
        builds: AtomicUsize,
        fail: Mutex<Option<WebBuildError>>,
        write_bundle: bool,
        /// The Backend address of every build, as the generator was asked for it.
        urls: Mutex<Vec<String>>,
    }

    impl Fake {
        fn new() -> Arc<Self> {
            Arc::new(Self { builds: AtomicUsize::new(0), fail: Mutex::new(None), write_bundle: true, urls: Mutex::new(Vec::new()) })
        }
    }

    impl WebBuildBackend for Fake {
        fn name(&self) -> &str {
            "fake"
        }

        fn build(&self, request: &WebBuildRequest) -> Result<BuildReport, WebBuildError> {
            let n = self.builds.fetch_add(1, Ordering::SeqCst) + 1;
            self.urls.lock().unwrap().push(request.web.backend_url.clone());
            if let Some(e) = self.fail.lock().unwrap().clone() {
                return Err(e);
            }
            if self.write_bundle {
                std::fs::create_dir_all(&request.out_dir).unwrap();
                std::fs::write(request.out_dir.join("index.html"), format!("<html><body>build {n}</body></html>")).unwrap();
                std::fs::write(request.out_dir.join("app.css"), "body{}").unwrap();
            }
            Ok(BuildReport {
                bundle_dir: request.out_dir.clone(),
                size: BundleSize { total_bytes: 3000, parts: vec![PartSize { name: "index.html".into(), bytes: 3000 }] },
                startup_ms: Some(12.0),
                warnings: vec!["a warning".into()],
                duration_ms: 3,
                ..BuildReport::default()
            })
        }
    }

    fn launch(manifest: &Path, backend: Arc<dyn WebBuildBackend>) -> Launch {
        Launch {
            manifest: manifest.to_path_buf(),
            name: "P".into(),
            kind: ProjectKind::Web,
            platform: None,
            device: None,
            orientation: None,
            devices: Arc::new(NoDeviceTools),
            has_backend: true,
            backend,
            browser: BrowserChoice::System,
            path_confirmed: false,
            page_strings: page_strings(&Language::English.tr()),
            language: Language::English,
            backend_url: None,
        }
    }

    fn wait_until(preview: &mut WebPreview, what: &str, mut done: impl FnMut(&WebPreview) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut now = Instant::now();
        while !done(preview) {
            assert!(Instant::now() < deadline, "timed out waiting for {what}; notes: {:?}", preview.notes());
            std::thread::sleep(Duration::from_millis(10));
            now += Duration::from_millis(500);
            preview.poll(now);
        }
    }

    /// A stand-in for the headless helper that leaves a mark if anything starts
    /// it: the interactive Preview never does (WASM-008 R16, AC15).
    #[cfg(unix)]
    fn marking_helper(dir: &Path) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let mark = dir.join("helper-was-started");
        let helper = dir.join("marking-helper");
        std::fs::write(&helper, format!("#!/bin/sh\ntouch '{}'\necho '{{\"event\":\"ready\"}}'\nexit 0\n", mark.display())).unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();
        (helper, mark)
    }

    /// Start a Preview of a scratch project whose browser is `choice`, wait for
    /// the page to be opened, and answer what it said.
    pub(crate) fn open_with(choice: BrowserChoice, confirmed: bool) -> Vec<Note> {
        quiet();
        let manifest = project(&format!("open-with-{confirmed}"));
        let mut p = WebPreview::default();
        let mut l = launch(&manifest, Fake::new());
        l.browser = choice;
        l.path_confirmed = confirmed;
        assert!(p.start(l, Intent::Show));
        wait_until(&mut p, "the page to be opened", |p| p.opened().is_some());
        let notes = p.notes().to_vec();
        p.close();
        let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
        notes
    }

    /// A device runner that has one device and two log lines, and writes down
    /// what it was asked, in order.
    struct FakeDevices {
        calls: Mutex<Vec<String>>,
        log: Mutex<Vec<String>>,
    }

    impl DeviceRunner for FakeDevices {
        fn detect(&self, platform: MobilePlatform) -> Result<(), String> {
            self.calls.lock().unwrap().push(format!("detect {}", platform.as_str()));
            Ok(())
        }

        fn list(&self, platform: MobilePlatform) -> Vec<String> {
            self.calls.lock().unwrap().push(format!("list {}", platform.as_str()));
            vec!["Pixel 8".into(), "Pixel Tablet".into()]
        }

        fn run(&self, device: &str, dev_url: &str) -> Result<(), String> {
            self.calls.lock().unwrap().push(format!("run {device} {dev_url}"));
            Ok(())
        }

        fn log(&self) -> Vec<String> {
            std::mem::take(&mut *self.log.lock().unwrap())
        }
    }

    fn mobile_launch(manifest: &Path, devices: Arc<dyn DeviceRunner>, device: DeviceClass, orientation: Orientation) -> Launch {
        let mut l = launch(manifest, Fake::new());
        l.kind = ProjectKind::Mobile;
        l.platform = Some(MobilePlatform::Android);
        l.device = Some(device);
        l.orientation = Some(orientation);
        l.devices = devices;
        l
    }

    /// AC14, the IDE half: with no device tools a Mobile Preview opens the
    /// phone page at `[mobile] device` and `orientation`, and says why.
    #[test]
    fn a_mobile_preview_without_device_tools_opens_the_phone_page_008() {
        quiet();
        for (device, orientation, w, h) in [(DeviceClass::Phone, Orientation::Portrait, 390, 844), (DeviceClass::Tablet, Orientation::Landscape, 1180, 820)] {
            let manifest = project(&format!("phone-{}-{}", device.as_str(), orientation.as_str()));
            let mut p = WebPreview::default();
            assert!(p.start(mobile_launch(&manifest, Arc::new(NoDeviceTools), device, orientation), Intent::Show));
            wait_until(&mut p, "the phone page to be opened", |p| p.opened().is_some());
            let (opened, _) = p.opened().cloned().unwrap();
            assert!(opened.ends_with(devserver::PHONE_PATH), "{opened}");
            let note = Note::NotTheDevice { platform: "Android".into(), install: Some("adb".into()), width: w, height: h };
            assert!(p.notes().contains(&note), "{:?}", p.notes());
            let page = http_get(&p.url().unwrap(), devserver::PHONE_PATH);
            assert!(page.contains(&format!("width=\"{w}\" height=\"{h}\"")), "{page}");
            assert!(page.contains(Language::English.tr().pk_page_not_device), "the banner: {page}");
            for &lang in Language::ALL {
                let lines = note.lines(&lang.tr());
                assert_eq!(lines.len(), 2, "{lang:?}");
                assert!(lines.iter().all(|l| !l.contains("{}")), "{lang:?} {lines:?}");
                assert!(lines[0].contains("adb") && lines[0].contains("Android") && lines[1].contains(&w.to_string()), "{lang:?} {lines:?}");
            }
            println!("{}: {opened} at {w} x {h}; {}", device.as_str(), note.text(&Language::English.tr()).replace('\n', " / "));
            p.close();
            let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
        }
    }

    /// AC13, the hand-over only (the real device run waits for WASM-012): a
    /// runner with a device gets the development address, no browser is
    /// opened, and the device's log reaches the Output notes.
    #[test]
    fn a_mobile_preview_hands_the_address_to_a_device_and_reads_its_log_008() {
        quiet();
        let manifest = project("on-device");
        let fake = Arc::new(FakeDevices { calls: Mutex::new(Vec::new()), log: Mutex::new(vec!["I/chromium: started".into(), "I/chromium: MAIN-FORM shown".into()]) });
        let mut p = WebPreview::default();
        assert!(p.start(mobile_launch(&manifest, fake.clone(), DeviceClass::Phone, Orientation::Auto), Intent::Show));
        wait_until(&mut p, "the device run", |p| p.notes().iter().any(|n| matches!(n, Note::OnDevice { .. })));
        let url = p.url().unwrap();
        p.poll(Instant::now());
        let calls = fake.calls.lock().unwrap().clone();
        assert_eq!(calls, ["detect android".to_owned(), "list android".to_owned(), format!("run Pixel 8 {url}")]);
        assert!(p.opened().is_none(), "no browser when a device runs it");
        assert!(p.notes().contains(&Note::OnDevice { device: "Pixel 8".into(), url: url.clone() }));
        assert!(p.notes().contains(&Note::DeviceLog("I/chromium: MAIN-FORM shown".into())), "{:?}", p.notes());
        assert!(!p.notes().iter().any(|n| matches!(n, Note::NotTheDevice { .. })));
        for c in &calls {
            println!("runner: {c}");
        }
        p.close();
        let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
    }

    /// No test opens a browser window on the developer's screen.
    fn quiet() {
        std::env::set_var(preview::NO_BROWSER_ENV, "1");
    }

    /// The change count the server answers: `{"version":N}`.
    fn version_of(url: &str) -> u64 {
        let body = http_get(url, "/__prc/version");
        body.trim().trim_start_matches("{\"version\":").trim_end_matches('}').parse().unwrap_or_else(|_| panic!("not a version: {body}"))
    }

    /// The body of the answer to `GET path`.
    fn http_get(url: &str, path: &str) -> String {
        use std::io::{Read, Write};
        let port: u16 = url.trim_end_matches('/').rsplit(':').next().unwrap().parse().unwrap();
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.write_all(format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n").as_bytes()).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out.split_once("\r\n\r\n").map(|(_, body)| body.to_owned()).unwrap_or(out)
    }

    // ── notes ──

    #[test]
    fn every_generator_diagnostic_kind_is_said_in_all_six_languages() {
        let kinds = [
            WebDiagnosticKind::BackendOnly,
            WebDiagnosticKind::Unsupported,
            WebDiagnosticKind::UnknownLayer,
            WebDiagnosticKind::UnknownLayerProperty,
            WebDiagnosticKind::LayerWrite,
            WebDiagnosticKind::TooManyHostReads,
            WebDiagnosticKind::AcceptIgnored,
            WebDiagnosticKind::RedefinesMayDiffer,
            WebDiagnosticKind::InstancesOutOfRange,
            WebDiagnosticKind::MultiMemoryOnlyWithWebKit,
            WebDiagnosticKind::SecretOnForm,
            WebDiagnosticKind::UnavailableOnWeb,
            WebDiagnosticKind::NoAccessibleName,
        ];
        let mut said = 0;
        for &lang in Language::ALL {
            let tr = lang.tr();
            for kind in kinds {
                let d = WebDiagnostic { location: "webforms/a.cfrm:7".into(), kind, subject: "SUBJ-9".into(), message: "english".into(), alternative: "ALT-9".into() };
                let text = diagnostic_text(&d, &tr);
                assert!(text.starts_with("webforms/a.cfrm:7: "), "{lang:?} {kind:?}: {text}");
                assert!(text.contains("SUBJ-9") && !text.contains("{}") && !text.contains("english"), "{lang:?} {kind:?}: {text}");
                assert!(!text.to_lowercase().contains("cobolt"), "{lang:?} {kind:?}: {text}");
                if kind == WebDiagnosticKind::UnavailableOnWeb {
                    assert!(text.contains("ALT-9"), "{lang:?}: the alternative is said: {text}");
                }
                said += 1;
            }
            // Free text is said as it is.
            let other = WebDiagnostic::from("a.cfrm:3: EXEC RUST - move it to the Backend");
            assert_eq!(diagnostic_text(&other, &tr), "a.cfrm:3: EXEC RUST - move it to the Backend");
        }
        eprintln!("generator diagnostics: {said} kind × language texts, none empty");
        assert_eq!(said, kinds.len() * Language::ALL.len());
    }

    /// WASM-002 T21: the budget warning and the missing player get the
    /// developer's heading; the English detail follows it.
    #[test]
    fn the_budget_warning_and_the_missing_player_have_a_translated_heading() {
        let over = format!("{} 5100000 bytes, over the budget", cobolt_project_tools::web::OVER_BUDGET_PREFIX);
        let gone = format!("{}: no folder assets/web-player", cobolt_project_tools::web::PLAYER_UNAVAILABLE_PREFIX);
        for &lang in Language::ALL {
            let tr = lang.tr();
            let w = Note::BuildWarning(over.clone().into()).lines(&tr);
            assert_eq!(w, vec![tr.web_build_over_budget.to_owned(), format!("  {over}")], "{lang:?}");
            let f = Note::BuildFailed(gone.clone()).lines(&tr);
            assert_eq!(f, vec![tr.web_build_player_unavailable.to_owned(), format!("  {gone}")], "{lang:?}");
        }
    }

    #[test]
    fn every_note_reads_in_all_six_languages_without_a_gap() {
        let notes = vec![
            Note::Building,
            Note::NotLinked,
            Note::BuildDone { ms: 120, bytes: 1_300_000, files: 4, startup_ms: Some(8.4) },
            Note::BuildDone { ms: 5, bytes: 900, files: 1, startup_ms: None },
            Note::BuildWarning("w".into()),
            Note::BuildRefused(vec!["a.cfrm:3: EXEC RUST - move it to the Backend".into()]),
            Note::BuildFailed("boom".into()),
            Note::PackageNote,
            Note::NoBundle,
            Note::Serving { name: "P".into(), url: "http://127.0.0.1:1/".into() },
            Note::ServerFailed("busy".into()),
            Note::Opened { browser: String::new(), url: "http://127.0.0.1:1/".into() },
            Note::Opened { browser: "Firefox".into(), url: "http://127.0.0.1:1/".into() },
            Note::Updated { ms: 180 },
            Note::BrowserFailed { name: "Firefox".into(), why: "Firefox was not found on this computer".into() },
            Note::BrowserNotConfirmed { path: "/Applications/Other.app".into() },
            Note::Opened { browser: "/Applications/Other.app".into(), url: "http://127.0.0.1:1/".into() },
            Note::BackendOff,
            Note::Console { level: "log".into(), text: "hi".into(), form: String::new() },
            Note::PageError { message: "x is not defined".into(), source: "app.js".into(), line: 12, form: "MAIN-FORM".into() },
            Note::BuildWarning(
                format!("{} 5100000 bytes, over the budget of 5000000 bytes", cobolt_project_tools::web::OVER_BUDGET_PREFIX).into(),
            ),
            Note::BuildFailed(format!("{}: no folder assets/web-player", cobolt_project_tools::web::PLAYER_UNAVAILABLE_PREFIX)),
            Note::PlayerSizes(vec![
                PartSize { name: "plain".into(), bytes: 2_846_052 },
                PartSize { name: "threaded".into(), bytes: 2_670_938 },
                PartSize { name: "parts/mermaid".into(), bytes: 2_218_954 },
            ]),
            Note::PlayerPin(PlayerPinError::NotKept { pinned: "1.0.0".into(), kept: vec!["2.0.0".into(), "2.1.0".into()] }),
            Note::PlayerPin(PlayerPinError::CannotEmit { pinned: "1.9.9".into(), abi: 2, description: 1, oldest: Some("2.0.0".into()) }),
            Note::PlayerPin(PlayerPinError::CannotEmit { pinned: "1.9.9".into(), abi: 2, description: 1, oldest: None }),
            Note::BackendStarting { listen: "127.0.0.1:8443".into(), secs: 60 },
            Note::BackendReady { url: "ws://127.0.0.1:8443/ws".into() },
            Note::BackendPortBusy { port: 8443 },
            Note::BackendTaken { url: "ws://127.0.0.1:8750/ws".into() },
            Note::BackendRemote { url: "wss://api.example.com/ws".into() },
            Note::BackendMismatch { running: "ws://127.0.0.1:8443/ws".into(), page: "ws://127.0.0.1:8750/ws".into() },
            Note::BackendFailed { code: Some(1), why: vec!["rcrun backend: no exports.toml".into()] },
            Note::BackendFailed { code: None, why: vec![] },
            Note::BackendTimeout { secs: 60 },
            Note::CobolError { form: "MAIN-FORM".into(), unit: "BTN-OK-CLICK".into(), line: 7, code: "EC-SIZE".into(), message: "size error".into() },
            Note::PageUpdated { how: "stylesheet, stylesheet of MAIN-FORM".into(), ms: 12.25 },
            Note::PageUpdated { how: "program".into(), ms: 80.0 },
            Note::PageUpdated { how: "forms".into(), ms: 40.0 },
            Note::PageUpdated { how: "reload".into(), ms: 3.0 },
            Note::PageUpdated { how: "unchanged".into(), ms: 1.0 },
            Note::PreviewStopped,
            Note::TokenUnused { url: "ws://127.0.0.1:8443/ws".into() },
            Note::NotTheDevice { platform: "Android".into(), install: Some("adb".into()), width: 390, height: 844 },
            Note::NotTheDevice { platform: "iOS".into(), install: None, width: 820, height: 1180 },
            Note::OnDevice { device: "Pixel 8".into(), url: "http://127.0.0.1:5173/".into() },
            Note::DeviceFailed { device: "Pixel 8".into(), why: "offline".into() },
            Note::DeviceLog("I/chromium: hello".into()),
        ];
        let mut shown = 0;
        for &lang in Language::ALL {
            let tr = lang.tr();
            for n in &notes {
                let text = n.text(&tr);
                assert!(!text.trim().is_empty(), "{lang:?} {n:?}");
                assert!(!text.contains("{}"), "{lang:?} {n:?}: {text}");
                shown += 1;
            }
        }
        // The facts reach the sentence, whatever the language.
        let en = Language::English.tr();
        let done = notes[2].text(&en);
        assert!(done.contains("120 ms") && done.contains("1.24 MB") && done.contains("4 files") && done.contains("8 ms"), "{done}");
        assert!(notes[3].text(&en).contains("not measured"));
        assert!(notes[5].lines(&en).len() == 2 && notes[5].lines(&en)[1].starts_with("  a.cfrm:3"));
        assert_eq!(notes[18].severity(), Severity::Info);
        assert_eq!(Note::Console { level: "error".into(), text: String::new(), form: String::new() }.severity(), Severity::Error);
        // WASM-008 T36: the form leads the page's lines; a COBOL error names its unit and line.
        assert_eq!(notes[19].text(&en), "MAIN-FORM: Page error: x is not defined (app.js:12)");
        let cobol = notes[35].text(&en);
        assert!(cobol.starts_with("MAIN-FORM: ") && cobol.contains("size error") && cobol.contains("(BTN-OK-CLICK:7)") && cobol.contains("EC-SIZE"), "{cobol}");
        assert!(notes[36].text(&en).contains("12.2 ms") && notes[36].text(&en).contains("stylesheet of MAIN-FORM"));
        let kinds: std::collections::BTreeSet<String> = (36..41).map(|i| notes[i].text(&en)).collect();
        assert_eq!(kinds.len(), 5, "one Output line per kind of update");
        assert_eq!(notes[14].severity(), Severity::Warn);
        // WASM-016 T27: the player's sizes, its parts on a line of their own, and the pin's facts in the sentence.
        let sizes = notes[22].lines(&en);
        assert_eq!(sizes, ["Player size: plain 2.71 MB, threaded 2.55 MB", "Loaded on first use: mermaid 2.12 MB"], "{sizes:?}");
        assert!(notes[23].text(&en).contains("1.0.0") && notes[23].text(&en).contains("2.0.0, 2.1.0"));
        assert!(notes[24].text(&en).contains("1.9.9") && notes[24].text(&en).contains("2.0.0"));
        assert_eq!(notes[25].severity(), Severity::Error);
        // The Backend's start: its facts in the sentence, its own words under it.
        assert!(notes[26].text(&en).contains("127.0.0.1:8443") && notes[26].text(&en).contains("60 s"));
        assert!(notes[27].text(&en).contains("ws://127.0.0.1:8443/ws"));
        assert!(notes[28].text(&en).contains("8443") && notes[28].severity() == Severity::Warn);
        let failed = notes[32].lines(&en);
        assert!(failed[0].contains("exit 1") && failed[1] == "  rcrun backend: no exports.toml", "{failed:?}");
        assert_eq!(notes[32].severity(), Severity::Error);
        assert_eq!(notes[34].severity(), Severity::Error);
        println!("web preview: {shown} note renderings ({} notes x 6 languages), none empty, none with a gap", notes.len());
    }

    #[test]
    fn sizes_are_written_for_people() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(1023), "1023 B");
        assert_eq!(human_bytes(1024), "1.0 KB");
        assert_eq!(human_bytes(12_800), "12.5 KB");
        assert_eq!(human_bytes(1_258_291), "1.20 MB");
    }

    // ── watcher ──

    #[test]
    fn a_change_is_reported_once_the_folders_are_still() {
        let manifest = project("watch");
        let dir = manifest.parent().unwrap().to_path_buf();
        let mut w = SourceWatcher::new(&manifest, ProjectKind::Web, false);
        let t0 = Instant::now();
        assert_eq!(w.poll(t0), None, "nothing changed");
        assert_eq!(w.poll(t0 + Duration::from_millis(100)), None, "too soon to look again");
        assert_eq!(WATCH_EVERY, Duration::from_millis(150), "WASM-008 D6");

        std::fs::write(dir.join("webforms/main-form.cfrm"), "<Form changed='yes'/>").unwrap();
        std::fs::write(dir.join("webforms/theme.css"), "body{color:red}").unwrap();
        let t1 = t0 + Duration::from_millis(500);
        assert_eq!(w.poll(t1), None, "seen, but wait for it to settle");
        let t2 = t1 + Duration::from_millis(500);
        assert_eq!(w.poll(t2), Some(t1), "still: reported, with the time it was first seen");
        assert_eq!(w.poll(t2 + Duration::from_millis(500)), None, "and only once");

        // A folder the build does not read is not watched.
        std::fs::create_dir_all(dir.join("dist/web")).unwrap();
        std::fs::write(dir.join("dist/web/index.html"), "x").unwrap();
        assert_eq!(w.poll(t2 + Duration::from_millis(1000)), None);
        assert_eq!(w.poll(t2 + Duration::from_millis(1500)), None);
        // The manifest is.
        std::fs::write(&manifest, format!("{MANIFEST}# edited\n")).unwrap();
        assert_eq!(w.poll(t2 + Duration::from_millis(2000)), None);
        assert!(w.poll(t2 + Duration::from_millis(2500)).is_some());

        // In real time (a watcher of its own: the looks above were at made-up instants): a save is
        // reported after the look that sees it and one quiet look (D6).
        let mut w = SourceWatcher::new(&manifest, ProjectKind::Web, false);
        let _ = w.poll(Instant::now());
        let t = Instant::now();
        std::fs::write(dir.join("webforms/main-form.cfrm"), "<Form again='yes'/>").unwrap();
        let reported = loop {
            if let Some(seen) = w.poll(Instant::now()) {
                break seen;
            }
            assert!(t.elapsed() < Duration::from_secs(5), "the change was not reported");
            std::thread::sleep(Duration::from_millis(5));
        };
        let after = t.elapsed();
        assert!(reported >= t);
        println!("watcher: a save reported {} ms after it (looks every {} ms, one quiet look)", after.as_millis(), WATCH_EVERY.as_millis());
        let _ = std::fs::remove_dir_all(dir);
    }

    // ── the whole Preview, with a generator and a helper that are stand-ins ──

    #[test]
    fn a_failed_or_unlinked_build_says_so_and_shows_what_it_can() {
        quiet();
        let manifest = project("fail");
        let dir = manifest.parent().unwrap().to_path_buf();
        let backend = Fake::new();
        *backend.fail.lock().unwrap() = Some(WebBuildError::Refused(vec!["a.cfrm:3: EXEC RUST - move it to the Backend".into()]));
        let mut p = WebPreview::default();
        assert!(p.start(launch(&manifest, backend.clone()), Intent::Build));
        assert!(!p.start(launch(&manifest, backend.clone()), Intent::Build), "one build at a time");
        wait_until(&mut p, "the build", |p| !p.building());
        assert!(matches!(p.notes().last(), Some(Note::BuildRefused(l)) if l.len() == 1), "{:?}", p.notes());
        assert!(!p.serving());

        // Not linked, and no bundle: it says both.
        let mut p = WebPreview::default();
        let unlinked: Arc<dyn WebBuildBackend> = Arc::new(cobolt_project_tools::web::NotYetLinked);
        p.start(launch(&manifest, unlinked.clone()), Intent::Show);
        wait_until(&mut p, "the build", |p| !p.building());
        assert_eq!(p.notes().iter().filter(|n| matches!(n, Note::NotLinked | Note::NoBundle)).count(), 2, "{:?}", p.notes());
        assert!(!p.serving());

        // Not linked, but a bundle is there from somebody else: it is shown.
        std::fs::create_dir_all(dir.join("dist/web")).unwrap();
        std::fs::write(dir.join("dist/web/index.html"), "<html>made elsewhere</html>").unwrap();
        let mut p = WebPreview::default();
        p.start(launch(&manifest, unlinked), Intent::Show);
        wait_until(&mut p, "the server", |p| p.serving());
        assert!(p.notes().contains(&Note::NotLinked));
        assert!(http_get(&p.url().unwrap(), "/").contains("made elsewhere"));
        p.close();
        assert!(!p.serving());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// WASM-007 T23 (the operator's option (a)): Apply's sheet goes straight into the served bundle and is
    /// counted at once — no Build, and the watcher does not rebuild for it; a sheet the Build would refuse
    /// takes the normal path, a Build, which reports it.
    #[test]
    fn an_applied_stylesheet_reaches_the_open_preview_without_a_build() {
        quiet();
        let manifest = project("fastcss");
        let dir = manifest.parent().unwrap().to_path_buf();
        let backend = Fake::new();
        let mut p = WebPreview::default();
        assert!(p.start(launch(&manifest, backend.clone()), Intent::Show));
        wait_until(&mut p, "the Preview to be showing", |p| p.serving());
        let root = p.live.as_ref().unwrap().server.root().to_path_buf();
        std::fs::write(root.join("config.json"), r#"{"forms":[{"name":"main-form","url":"forms/main-form.1.json","integrity":"x"}]}"#).unwrap();
        let url = p.url().unwrap();
        let mut now = Instant::now();
        p.poll(now);

        let css = "#LBL-OUT { word-spacing: 7px }\n";
        std::fs::write(dir.join("webforms/main-form.css"), css).unwrap();
        let v = version_of(&url);
        assert!(p.apply_form_stylesheet("main-form", css, &[]), "{:?}", p.notes());
        assert!(version_of(&url) > v, "counted at once");
        assert!(matches!(p.notes().last(), Some(Note::Updated { .. })), "{:?}", p.notes());
        let config: serde_json::Value = serde_json::from_str(&http_get(&url, "/config.json")).unwrap();
        let sheet = config["forms"][0]["css"]["url"].as_str().unwrap().to_owned();
        assert_eq!(http_get(&url, &format!("/{sheet}")), css);
        for _ in 0..6 {
            now += WATCH_EVERY;
            p.poll(now);
        }
        assert!(!p.building());
        assert_eq!(backend.builds.load(Ordering::SeqCst), 1, "no Build for the sheet the Preview already shows");

        // A sheet the Build refuses is not put in place; the watcher's Build follows the write.
        let bad = ".a { background: url(https://evil.example/x) }\n";
        std::fs::write(dir.join("webforms/main-form.css"), bad).unwrap();
        assert!(!p.apply_form_stylesheet("main-form", bad, &[]));
        wait_until(&mut p, "the Build after a refused sheet", |p| !p.building() && p.notes().iter().filter(|n| matches!(n, Note::Updated { .. })).count() == 2);
        assert_eq!(backend.builds.load(Ordering::SeqCst), 2);
        p.close();
        // Nothing is served: nothing to do.
        assert!(!p.apply_form_stylesheet("main-form", css, &[]));
        let _ = std::fs::remove_dir_all(&dir);
        println!("web preview fast path: {sheet} served and counted without a Build (1 build); a refused sheet took the Build (2 builds)");
    }

    #[test]
    fn building_serving_opening_and_rebuilding_on_a_change() {
        quiet();
        let manifest = project("flow");
        let dir = manifest.parent().unwrap().to_path_buf();
        let backend = Fake::new();
        let mut p = WebPreview::default();
        let t = Instant::now();
        assert!(p.start(launch(&manifest, backend.clone()), Intent::Show));
        wait_until(&mut p, "the Preview to be showing", |p| p.serving());

        let kinds: Vec<&str> = p
            .notes()
            .iter()
            .map(|n| match n {
                Note::Building => "building",
                Note::BuildDone { .. } => "done",
                Note::BuildWarning(_) => "warning",
                Note::Serving { .. } => "serving",
                Note::BackendOff => "backend-off",
                Note::Opened { .. } => "opened",
                _ => "other",
            })
            .collect();
        assert_eq!(kinds, ["building", "done", "warning", "serving", "backend-off", "opened"], "{:?}", p.notes());
        let url = p.url().unwrap();
        let page = http_get(&url, "/");
        assert!(page.contains("build 1") && page.contains("/__prc/livereload.js"), "the page is served with live reload: {page}");
        assert_eq!(p.take_outbox().len(), 6);
        assert!(p.take_outbox().is_empty(), "each note reaches the Output panel once");
        // The served script carries the IDE's texts for the page.
        assert!(http_get(&url, devserver::RELOAD_SCRIPT_PATH).contains(Language::English.tr().pk_page_stopped));

        // A change in a form: the bundle is built again and the page is told.
        let version_before = version_of(&url);
        std::fs::write(dir.join("webforms/main-form.cfrm"), "<Form edited='true'/>").unwrap();
        wait_until(&mut p, "the rebuild", |p| p.notes().iter().any(|n| matches!(n, Note::Updated { .. })));
        assert_eq!(backend.builds.load(Ordering::SeqCst), 2);
        assert!(http_get(&url, "/").contains("build 2"), "the new bundle is the one served");
        let version_after = version_of(&url);
        assert!(version_after > version_before, "the page's next poll sees a new version ({version_before} -> {version_after})");

        // R7, AC5: a failed rebuild leaves the last good bundle served and puts its lines on
        // `/__prc/version` for the overlay; the next good build takes them away.
        let version_json = |url: &str| serde_json::from_str::<serde_json::Value>(&http_get(url, "/__prc/version")).unwrap();
        *backend.fail.lock().unwrap() = Some(WebBuildError::Failed("webforms/main-form.cfrm:1: broken on purpose".into()));
        std::fs::write(dir.join("webforms/main-form.cfrm"), "<Form broken='true'/>").unwrap();
        wait_until(&mut p, "the failed rebuild", |p| p.notes().iter().any(|n| matches!(n, Note::BuildFailed(_))));
        let errors = version_json(&url)["errors"].clone();
        assert!(errors[0].as_str().unwrap().contains("broken on purpose"), "{errors}");
        assert!(http_get(&url, "/").contains("build 2"), "the last good bundle is still the one served");
        *backend.fail.lock().unwrap() = None;
        std::fs::write(dir.join("webforms/main-form.cfrm"), "<Form fixed='true'/>").unwrap();
        wait_until(&mut p, "the good rebuild", |p| p.notes().iter().filter(|n| matches!(n, Note::Updated { .. })).count() == 2);
        assert_eq!(version_json(&url).get("errors"), None, "a good build takes the errors away");
        assert!(http_get(&url, "/").contains("build 4"));

        p.close();
        assert!(!p.active());
        let _ = std::fs::remove_dir_all(&dir);
        println!("web preview: build -> serve -> browser in {} ms; a form edit rebuilt and republished", t.elapsed().as_millis());
    }

    /// `POST /__prc/log` as the served Preview script sends it.
    fn post_events(url: &str, body: &str) -> u16 {
        use std::io::{Read, Write};
        let port: u16 = url.trim_end_matches('/').rsplit(':').next().unwrap().parse().unwrap();
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(
            s,
            "POST /__prc/log HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out.split_whitespace().nth(1).unwrap().parse().unwrap()
    }

    /// WASM-008 T36 (R13, R5, R3; AC12, AC4, AC2): what the page posts becomes
    /// Output lines with the form's name, a COBOL error with its unit and line,
    /// an update with how and how long; a flood is cut at 40 a poll.
    #[test]
    fn page_events_become_output_lines_with_the_form_008() {
        quiet();
        let manifest = project("events");
        let dir = manifest.parent().unwrap().to_path_buf();
        let mut p = WebPreview::default();
        assert!(p.start(launch(&manifest, Fake::new()), Intent::Show));
        wait_until(&mut p, "the Preview to be showing", |p| p.serving());
        let url = p.url().unwrap();
        let _ = p.take_outbox();
        let batch = r#"[{"event":"console","level":"error","text":"boom","form":"MAIN-FORM"},
            {"event":"error","message":"x is not defined","source":"app.js","line":12,"form":"MAIN-FORM"},
            {"event":"cobol-error","form":"ORDERS","unit":"BTN-SAVE-CLICK","line":41,"code":"EC-SIZE","message":"size error on WS-TOTAL"},
            {"event":"updated","how":"stylesheet","ms":18.4,"form":"MAIN-FORM"},
            {"event":"updated","how":"program","ms":95.0,"form":"MAIN-FORM"}]"#;
        assert_eq!(post_events(&url, batch), 204);
        p.poll(Instant::now());
        let en = Language::English.tr();
        let lines: Vec<String> = p.take_outbox().iter().flat_map(|n| n.lines(&en)).collect();
        for l in &lines {
            println!("  output: {l}");
        }
        assert_eq!(lines.len(), 5, "{lines:?}");
        assert!(lines[0].starts_with("MAIN-FORM: ") && lines[0].contains("boom"));
        assert!(lines[1].starts_with("MAIN-FORM: ") && lines[1].contains("x is not defined (app.js:12)"));
        assert!(lines[2].starts_with("ORDERS: ") && lines[2].contains("(BTN-SAVE-CLICK:41)") && lines[2].contains("size error on WS-TOTAL"));
        assert!(lines[3].contains("18.4 ms") && lines[3].contains("stylesheet"));
        assert!(lines[4].contains("95.0 ms") && lines[4].contains("new program"));
        for &lang in Language::ALL {
            for n in p.notes().iter().rev().take(5) {
                let t = n.text(&lang.tr());
                assert!(!t.contains("{}") && !t.trim().is_empty(), "{lang:?}: {t}");
            }
        }

        // A flood: two posts of 40 between two polls; 40 reach the Output panel.
        let forty = format!("[{}]", vec![r#"{"event":"console","level":"log","text":"again","form":"MAIN-FORM"}"#; 40].join(","));
        assert_eq!(post_events(&url, &forty), 204);
        assert_eq!(post_events(&url, &forty), 204);
        p.poll(Instant::now());
        let flood = p.take_outbox().len();
        assert_eq!(flood, CHATTER_PER_POLL, "a flood stays at {CHATTER_PER_POLL} a poll");
        p.poll(Instant::now());
        assert!(p.take_outbox().is_empty(), "the rest of the burst was dropped");
        println!("page events: 5 kinds became 5 Output lines; a burst of 80 gave {flood} lines");
        p.close();
        assert_eq!(p.take_outbox(), vec![Note::PreviewStopped]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WASM-008 T35 (R12, R16; AC11, AC15): Preview and Run open the browser
    /// the project names, else the system's default; a named browser that is
    /// not there, a path that is missing or not confirmed, falls back to the
    /// default and says so; a second Preview reuses the address; no helper
    /// process is ever started.
    #[cfg(unix)]
    #[test]
    fn preview_opens_the_projects_browser_reuses_its_address_and_starts_no_helper() {
        quiet();
        let manifest = project("browser");
        let dir = manifest.parent().unwrap().to_path_buf();
        let (helper, mark) = marking_helper(&dir);
        std::env::set_var(preview::HELPER_ENV, &helper);
        let backend = Fake::new();
        let run = |choice: BrowserChoice, confirmed: bool| {
            let mut p = WebPreview::default();
            let mut l = launch(&manifest, backend.clone());
            l.browser = choice;
            l.path_confirmed = confirmed;
            assert!(p.start(l, Intent::Show));
            wait_until(&mut p, "the page to be opened", |p| p.opened().is_some() || p.notes().iter().any(|n| matches!(n, Note::ServerFailed(_))));
            p
        };

        let p = run(BrowserChoice::System, false);
        let (url, used) = p.opened().unwrap().clone();
        assert_eq!(used, BrowserChoice::System);
        assert!(p.notes().contains(&Note::Opened { browser: String::new(), url: url.clone() }));
        println!("no setting      -> the system browser at {url}");

        let firefox = preview::KnownBrowser::Firefox;
        let p = run(BrowserChoice::Named(firefox), false);
        if firefox.is_present() {
            assert_eq!(p.opened().unwrap().1, BrowserChoice::Named(firefox));
            println!("firefox         -> Firefox");
        } else {
            assert_eq!(p.opened().unwrap().1, BrowserChoice::System);
            assert!(p.notes().iter().any(|n| matches!(n, Note::BrowserFailed { name, .. } if name == "Firefox")), "{:?}", p.notes());
            println!("firefox         -> not on this machine: BrowserFailed, then the system browser");
        }
        if let Some(present) = preview::KnownBrowser::ALL.into_iter().find(|b| b.is_present()) {
            let p = run(BrowserChoice::Named(present), false);
            assert_eq!(p.opened().unwrap().1, BrowserChoice::Named(present));
            assert!(p.notes().iter().any(|n| matches!(n, Note::Opened { browser, .. } if browser == present.name())));
            println!("{:<15} -> {} (present here)", present.id(), present.name());
        }

        let missing = BrowserChoice::Path(PathBuf::from("/definitely/not/a/browser"));
        let p = run(missing.clone(), true);
        assert_eq!(p.opened().unwrap().1, BrowserChoice::System);
        assert!(p.notes().iter().any(|n| matches!(n, Note::BrowserFailed { name, .. } if name == "/definitely/not/a/browser")), "{:?}", p.notes());
        println!("a missing path  -> BrowserFailed naming it, then the system browser");

        let unconfirmed = BrowserChoice::Path(std::env::current_exe().unwrap());
        let p = run(unconfirmed, false);
        assert_eq!(p.opened().unwrap().1, BrowserChoice::System, "an unconfirmed path is never started");
        assert!(p.notes().iter().any(|n| matches!(n, Note::BrowserNotConfirmed { .. })), "{:?}", p.notes());
        println!("unconfirmed path -> BrowserNotConfirmed, then the system browser");

        // Preview again: the same address, no new build, no new server.
        let mut p = run(BrowserChoice::System, false);
        let first = p.url().unwrap();
        let builds = backend.builds.load(Ordering::SeqCst);
        assert!(p.start(launch(&manifest, backend.clone()), Intent::Show));
        assert!(!p.building(), "nothing is built again");
        assert_eq!(p.url().unwrap(), first);
        assert_eq!(p.opened().unwrap().0, first, "the same address is opened again");
        assert_eq!(backend.builds.load(Ordering::SeqCst), builds);
        assert_eq!(p.notes().iter().filter(|n| matches!(n, Note::Opened { .. })).count(), 2);
        println!("Preview again   -> {first} reused, {builds} builds in all");

        std::thread::sleep(Duration::from_millis(200));
        assert!(!mark.exists(), "the interactive Preview started the web-view helper");
        std::env::remove_var(preview::HELPER_ENV);
        let _ = std::fs::remove_dir_all(&dir);
        println!("no helper process was started by any Preview (its mark is absent)");
    }

    // ── WASM-008 T47: the acceptance criteria on the real build, in the headless helper ──

    /// The helper the workspace built (`cargo build -p cobolt-web-preview`), beside
    /// the test binaries' folder: `target/<profile>/prc-web-preview`.
    fn built_helper() -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let p = exe.parent()?.parent()?.join(preview::HELPER_NAME);
        p.is_file().then_some(p)
    }

    /// The real generator with the repository's committed player (outside an
    /// install `web_player_dir()` is `assets/web-player` of the working folder),
    /// which refuses on purpose while `fail` is set — AC5's stand-in.
    #[derive(Default)]
    struct RealBuild {
        builds: AtomicUsize,
        fail: std::sync::atomic::AtomicBool,
    }

    impl WebBuildBackend for RealBuild {
        fn name(&self) -> &str {
            "generator"
        }

        fn bundle_dir(&self, request: &WebBuildRequest) -> PathBuf {
            cobolt_project_tools::web::generator::GeneratorBackend.bundle_dir(request)
        }

        fn build(&self, request: &WebBuildRequest) -> Result<BuildReport, WebBuildError> {
            self.builds.fetch_add(1, Ordering::SeqCst);
            if self.fail.load(Ordering::SeqCst) {
                return Err(WebBuildError::Refused(vec!["webforms/main-form.wcfrm:3: broken on purpose by the test".into()]));
            }
            let mut request = request.clone();
            request.player_dir = Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/web-player"));
            cobolt_project_tools::web::generator::GeneratorBackend.build(&request)
        }
    }

    /// A form with a label, a greeting button and a button whose handler
    /// subscripts past its table (a COBOL run-time error, with its unit and line).
    fn hello_form(caption: &str) -> String {
        let event = |control: &str, data: &str, body: &str| {
            format!(
                "<Event name=\"onClick\" paragraph=\"{control}--ONCLICK\"><![CDATA[       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n{data}       PROCEDURE DIVISION.\n{body}]]></Event>"
            )
        };
        let greet = event("BTN-GREET", "", "           MOVE \"Hello\" TO LBL-OUT::Caption.\n");
        let fail = event(
            "BTN-ERR",
            "       WORKING-STORAGE SECTION.\n       01 WS-T.\n          05 WS-E PIC X OCCURS 3.\n       01 WS-I PIC 9 VALUE 7.\n",
            "           MOVE \"X\" TO WS-E(WS-I).\n",
        );
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Hello" width="900" height="600" index-base="1" main-form="true">
  <Control id="LBL-OUT" type="Label" x="50" y="40" w="300" h="30"><Property name="Caption">{caption}</Property></Control>
  <Control id="BTN-GREET" type="Button" x="50" y="100" w="120" h="30" tab-order="1"><Property name="Caption">Greet</Property>{greet}</Control>
  <Control id="BTN-ERR" type="Button" x="200" y="100" w="120" h="30" tab-order="2"><Property name="Caption">Fail</Property>{fail}</Control>
</Form>
"#
        )
    }

    /// Poll the Preview and the helper together until `pick` answers.
    fn drive<T>(
        p: &mut WebPreview,
        s: &mut preview::PreviewSession,
        seen: &mut Vec<preview::Event>,
        secs: u64,
        what: &str,
        mut pick: impl FnMut(&[preview::Event], &WebPreview) -> Option<T>,
    ) -> T {
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            p.poll(Instant::now());
            seen.extend(s.poll());
            if let Some(t) = pick(seen, p) {
                return t;
            }
            assert!(Instant::now() < deadline, "timed out waiting for {what}; notes: {:?}; events: {seen:?}", p.notes());
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Poll the Preview alone until `done` (a real build takes seconds, not the 30 s of `wait_until`'s fake clock).
    fn drive_preview(p: &mut WebPreview, secs: u64, what: &str, mut done: impl FnMut(&WebPreview) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(secs);
        while !done(p) {
            assert!(Instant::now() < deadline, "timed out waiting for {what}; notes: {:?}", p.notes());
            p.poll(Instant::now());
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// WASM-008 T47 (AC11, AC12, AC5, AC14): the Preview on the real Web Build
    /// (the generator and the committed player), its browser launch decided
    /// with `PRC_PREVIEW_NO_BROWSER`, and the page it serves driven in the
    /// headless helper. Skips with a printed reason when the helper is not
    /// built or there is no web view, unless `PRC_REQUIRE_WEBVIEW=1`.
    #[test]
    fn acceptance_on_the_real_build_in_the_headless_helper_008() {
        use cobolt_project_tools::web::preview::{Emulation, Event, PreviewSession};
        use serde_json::json;
        quiet();
        let require = std::env::var("PRC_REQUIRE_WEBVIEW").as_deref() == Ok("1");
        let Some(helper) = built_helper() else {
            assert!(!require, "PRC_REQUIRE_WEBVIEW=1 and the helper is not built (cargo build -p cobolt-web-preview)");
            println!("skipped: the helper is not built (cargo build -p cobolt-web-preview)");
            return;
        };
        let en = Language::English.tr();

        // ── AC11: the Preview of a Web project opens the browser; a click in the page works ──
        let dir = scratch("t47-web");
        std::fs::create_dir_all(dir.join("webforms")).unwrap();
        let manifest = dir.join("Hello.project.toml");
        std::fs::write(
            &manifest,
            "[project]\nname = \"Hello.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n[files]\nforms = [\"webforms/main-form.wcfrm\"]\n\n[web]\nmulti_memory = true\n",
        )
        .unwrap();
        std::fs::write(dir.join("webforms/main-form.wcfrm"), hello_form("Ready")).unwrap();
        std::fs::write(dir.join("webforms/application.css"), ".pr-form-root { letter-spacing: 0px }\n").unwrap();
        let backend = Arc::new(RealBuild::default());
        let mut l = launch(&manifest, backend.clone());
        l.name = "Hello".into();
        l.has_backend = false;
        let mut p = WebPreview::default();
        let t = Instant::now();
        assert!(p.start(l.clone(), Intent::Show));
        drive_preview(&mut p, 300, "the Preview to be opened", |p| p.opened().is_some() || !p.active());
        let (url, choice) = p.opened().cloned().unwrap_or_else(|| panic!("not opened: {:?}", p.notes()));
        assert_eq!(choice, BrowserChoice::System);
        println!("AC11: default -> {:?} ({} ms, real build)", Note::Opened { browser: String::new(), url: url.clone() }.text(&en), t.elapsed().as_millis());
        if let Some(present) = preview::KnownBrowser::ALL.into_iter().find(|b| b.is_present()) {
            let mut named = l.clone();
            named.browser = BrowserChoice::Named(present);
            assert!(p.start(named, Intent::Show), "the same Preview, opened again");
            assert_eq!(p.opened().unwrap().1, BrowserChoice::Named(present));
            println!("AC11: named   -> {}", p.notes().last().unwrap().text(&en));
        }
        let builds_at_open = backend.builds.load(Ordering::SeqCst);

        // A page script that throws, as an application's own script can (the bundle's CSP takes `'self'` only).
        let root = p.live.as_ref().unwrap().server.root().to_path_buf();
        std::fs::write(root.join("thrower.js"), "setTimeout(function () { throw new Error('thrown on purpose by the test'); }, 300);\n").unwrap();
        let index = std::fs::read_to_string(root.join("index.html")).unwrap();
        std::fs::write(root.join("index.html"), index.replacen("</body>", "<script src=\"/thrower.js\"></script></body>", 1)).unwrap();

        let page = format!("{url}?pr-form=main-form");
        let mut s = PreviewSession::start(&page, 900, 600, &Emulation::default(), &helper).unwrap();
        match s.ready(Duration::from_secs(30)) {
            Ok(visible) => assert_eq!(visible, Some(false), "AC15: no window on show"),
            Err(e) => {
                p.close();
                let _ = std::fs::remove_dir_all(&dir);
                assert!(!require, "PRC_REQUIRE_WEBVIEW=1 and no web view: {e}");
                println!("skipped: no web view on this system ({e})");
                return;
            }
        }
        let mut seen: Vec<Event> = Vec::new();
        let (values, _, errors) = s
            .script(vec![json!({"wait": {"control": "BTN-GREET"}}), json!({"click": {"control": "BTN-GREET"}}), json!({"get": {"control": "LBL-OUT", "property": "Caption"}})], 30)
            .unwrap();
        assert_eq!(values, vec![json!("Hello")], "AC11: a scripted click changed the label ({errors:?})");
        println!("AC11: a click on BTN-GREET in the served page set LBL-OUT to \"Hello\"; the Preview itself starts no helper (T35's assertion)");

        // ── AC12: console, uncaught and COBOL errors carry the form; they reach the Output notes ──
        let (_, _, errors) = s.script(vec![json!({"click": {"control": "BTN-ERR"}}), json!({"wait": {"ms": 600}})], 30).unwrap();
        println!("AC12: the page's own errors during the click: {errors:?}");
        let page_events: Vec<PageEvent> = drive(&mut p, &mut s, &mut seen, 30, "the page's error events", |seen, _| {
            let got: Vec<PageEvent> = seen
                .iter()
                .filter_map(|e| match e {
                    Event::Page(pe @ PageEvent::Console { level, .. }) if level == "error" => Some(pe.clone()),
                    Event::Page(pe @ (PageEvent::Error { .. } | PageEvent::CobolError { .. })) => Some(pe.clone()),
                    _ => None,
                })
                .collect();
            let has = |f: &dyn Fn(&PageEvent) -> bool| got.iter().any(f);
            (has(&|e| matches!(e, PageEvent::Console { .. }))
                && has(&|e| matches!(e, PageEvent::Error { message, .. } if message.contains("thrown on purpose")))
                && has(&|e| matches!(e, PageEvent::CobolError { .. })))
            .then_some(got)
        });
        // Inside the helper the Preview script posts over `window.ipc` (T33); in a browser the same JSON
        // goes to `/__prc/log`. It is posted there as the browser would, and read back as Output notes.
        let _ = p.take_outbox();
        let body = format!("[{}]", page_events.iter().map(PageEvent::to_line).collect::<Vec<_>>().join(","));
        assert_eq!(post_events(&url, &body), 204);
        p.poll(Instant::now());
        let lines: Vec<String> = p.take_outbox().iter().flat_map(|n| n.lines(&en)).collect();
        for l in &lines {
            println!("AC12: output: {l}");
        }
        for e in &page_events {
            let (form, kind) = match e {
                PageEvent::Console { form, .. } => (form, "console.error"),
                PageEvent::Error { form, .. } => (form, "thrown error"),
                PageEvent::CobolError { form, .. } => (form, "COBOL run-time error"),
                PageEvent::Updated { form, .. } => (form, "update"),
            };
            assert!(!form.is_empty(), "{kind} carries its form: {e:?}");
            assert!(lines.iter().any(|l| l.starts_with(&format!("{form}: "))), "{kind} reached the Output with its form: {lines:?}");
        }
        let cobol = page_events.iter().find_map(|e| if let PageEvent::CobolError { unit, line, .. } = e { Some((unit.clone(), *line)) } else { None }).unwrap();
        assert!(!cobol.0.is_empty(), "the COBOL error names its unit: {page_events:?}");
        println!("AC12: the COBOL run-time error's unit {:?}, line {}", cobol.0, cobol.1);

        // ── AC12: a property, a class rule and the stylesheet arrive in place — no page reload ──
        let loads = |seen: &[Event]| seen.iter().filter(|e| matches!(e, Event::Loading { .. })).count();
        let mut measured: Vec<(String, String, f64)> = Vec::new();
        let mut in_place = |p: &mut WebPreview, s: &mut PreviewSession, seen: &mut Vec<Event>, kind: &str, change: &dyn Fn(&mut WebPreview)| {
            let from = seen.len();
            change(p);
            let (how, ms) = drive(p, s, seen, 180, kind, |seen, _| {
                seen[from..].iter().find_map(|e| match e {
                    Event::Page(PageEvent::Updated { how, ms, .. }) if how != "unchanged" => Some((how.clone(), *ms)),
                    _ => None,
                })
            });
            // A reload would follow at once: give it the time to show.
            let until = Instant::now() + Duration::from_millis(600);
            while Instant::now() < until {
                seen.extend(s.poll());
                std::thread::sleep(Duration::from_millis(20));
            }
            assert!(how != "reload" && how != "unmounted", "{kind}: {how}");
            assert_eq!(loads(&seen[from..]), 0, "{kind}: the page did not load again ({how}); events: {:?}", &seen[from..]);
            measured.push((kind.to_owned(), how, ms));
        };
        let loads_before = loads(&seen);
        for n in 1..=5 {
            let form = dir.join("webforms/main-form.wcfrm");
            in_place(&mut p, &mut s, &mut seen, &format!("property {n}/5"), &|_| std::fs::write(&form, hello_form(&format!("Ready {n}"))).unwrap());
            let (values, _, _) = s.script(vec![json!({"get": {"control": "LBL-OUT", "property": "Caption"}})], 30).unwrap();
            assert_eq!(values, vec![json!(format!("Ready {n}"))], "the new Caption is on the page");
        }
        let css = dir.join("webforms/application.css");
        in_place(&mut p, &mut s, &mut seen, "class rule", &|_| std::fs::write(&css, ".pr-form-root { letter-spacing: 1px }\n").unwrap());
        let sheet = "#LBL-OUT { word-spacing: 7px }\n";
        std::fs::write(dir.join("webforms/main-form.css"), sheet).unwrap();
        in_place(&mut p, &mut s, &mut seen, "stylesheet", &|p| assert!(p.apply_form_stylesheet("main-form", sheet, &[]), "{:?}", p.notes()));
        drop(in_place);
        let (values, _, _) = s.script(vec![json!({"get": {"control": "LBL-OUT", "property": "Caption"}})], 30).unwrap();
        assert_eq!(values, vec![json!("Ready 5")], "the stylesheet restarted nothing");
        for (kind, how, ms) in &measured {
            println!("AC12: {kind:<14} -> updated in place, how = {how:?}, {ms:.1} ms (D6's working 500 ms; Q5 to confirm)");
        }
        assert_eq!(loads(&seen), loads_before, "no reload across the 7 changes");
        let builds_now = backend.builds.load(Ordering::SeqCst);
        println!("AC12: 7 of 7 changes in place, 5 of 5 property changes; {} builds after the first; page loads {}", builds_now - builds_at_open, loads(&seen));

        // ── AC5: a failed build keeps the last good page and draws its errors over it ──
        backend.fail.store(true, Ordering::SeqCst);
        std::fs::write(dir.join("webforms/main-form.wcfrm"), hello_form("Broken")).unwrap();
        drive_preview(&mut p, 120, "the refused build", |p| p.notes().iter().any(|n| matches!(n, Note::BuildRefused(_))));
        let (values, _, errors) = s
            .script(
                vec![
                    json!({"wait": {"control": "preview-errors"}}),
                    json!({"text": {"control": "preview-errors"}}),
                    json!({"get": {"control": "LBL-OUT", "property": "Caption"}}),
                ],
                30,
            )
            .unwrap();
        assert!(errors.is_empty(), "{errors:?}");
        let overlay = values[0].as_str().unwrap_or_default().to_owned();
        assert!(overlay.contains(en.pk_page_errors_title) && overlay.contains("broken on purpose by the test"), "AC5: the overlay: {overlay:?}");
        assert_eq!(values[1], json!("Ready 5"), "AC5: the last good bundle is still the page");
        println!("AC5: the overlay read back: {:?}; LBL-OUT still {}", overlay.replace('\n', " | "), values[1]);
        backend.fail.store(false, Ordering::SeqCst);
        s.close();

        // AC14, plan §9: `/__prc/phone` carries the bundle's isolation headers, so a same-origin frame is
        // isolated whenever the bundle is — here a Web bundle served `require-corp`.
        let isolation = WebBuildRequest::from_manifest(&manifest).unwrap().web.isolation;
        assert_ne!(isolation, Isolation::Off);
        let server = &p.live.as_ref().unwrap().server;
        server.set_phone(Some(PhoneFrame { width: 390, height: 844 }));
        let mut s = PreviewSession::start(&server.phone_url(), 600, 1000, &Emulation::default(), &helper).unwrap();
        assert_eq!(s.ready(Duration::from_secs(30)).unwrap(), Some(false));
        let (values, _, errors) = s
            .script(
                vec![
                    json!({"wait": {"control": "phone-frame"}}),
                    json!({"page": {"property": "innerWidth", "frame": "phone-frame"}}),
                    json!({"page": {"property": "crossOriginIsolated", "frame": "phone-frame"}}),
                    json!({"page": {"property": "crossOriginIsolated"}}),
                ],
                30,
            )
            .unwrap();
        assert!(errors.is_empty(), "{errors:?}");
        println!("AC14: a {} bundle at /__prc/phone: frame innerWidth {}, crossOriginIsolated {} in the frame, {} around it", isolation.as_str(), values[0], values[1], values[2]);
        assert_eq!(values, vec![json!(390), json!(true), json!(true)], "the frame keeps the bundle's isolation");
        s.close();
        p.close();
        let _ = std::fs::remove_dir_all(&dir);

        // ── AC14: no device → the phone page; the application in its frame at the phone's width, isolated ──
        let dir = scratch("t47-mobile");
        std::fs::create_dir_all(dir.join("mobileforms")).unwrap();
        let manifest = dir.join("Phone.project.toml");
        std::fs::write(
            &manifest,
            "[project]\nname = \"Phone.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"mobile\"\n\n[files]\nforms = [\"mobileforms/main-form.mcfrm\"]\n\n[mobile]\nplatform = \"android\"\ndevice = \"phone\"\norientation = \"portrait\"\nmulti_memory = true\n",
        )
        .unwrap();
        std::fs::write(dir.join("mobileforms/main-form.mcfrm"), hello_form("Phone")).unwrap();
        let mut l = launch(&manifest, backend.clone());
        l.kind = ProjectKind::Mobile;
        l.has_backend = false;
        l.platform = Some(MobilePlatform::Android);
        l.device = Some(DeviceClass::Phone);
        l.orientation = Some(Orientation::Portrait);
        let mut p = WebPreview::default();
        assert!(p.start(l, Intent::Show));
        drive_preview(&mut p, 300, "the phone page to be opened", |p| p.opened().is_some() || !p.active());
        let (phone, _) = p.opened().cloned().unwrap_or_else(|| panic!("not opened: {:?}", p.notes()));
        assert!(phone.ends_with(devserver::PHONE_PATH), "{phone}");
        let preset = presets::default_for(ProjectKind::Mobile, Some(DeviceClass::Phone)).viewport(false);
        let mut s = PreviewSession::start(&phone, 600, 1000, &Emulation::default(), &helper).unwrap();
        assert_eq!(s.ready(Duration::from_secs(30)).unwrap(), Some(false));
        let (values, _, errors) = s
            .script(
                vec![
                    json!({"wait": {"control": "phone-frame"}}),
                    json!({"page": {"property": "innerWidth", "frame": "phone-frame"}}),
                    json!({"page": {"property": "crossOriginIsolated", "frame": "phone-frame"}}),
                    json!({"page": {"property": "location.pathname", "frame": "phone-frame"}}),
                    json!({"page": {"property": "location.pathname"}}),
                ],
                30,
            )
            .unwrap();
        assert!(errors.is_empty(), "{errors:?}");
        println!("AC14: {phone}: frame innerWidth {}, crossOriginIsolated {}, frame at {}, page at {} (preset {} x {})", values[0], values[1], values[2], values[3], preset.width, preset.height);
        assert_eq!(values[0], json!(preset.width), "AC14: the frame is the preset's CSS width");
        // A Mobile bundle is served without isolation headers (WASM-017: a shell serves none,
        // `WebBuildRequest::from_manifest`); the frame is isolated exactly as the page alone would be.
        let isolated = WebBuildRequest::from_manifest(&manifest).unwrap().web.isolation != Isolation::Off;
        assert_eq!(values[1], json!(isolated), "AC14: the frame keeps the bundle's isolation");
        assert_eq!(values[2], json!("/"), "the application runs in the frame");
        assert_eq!(values[3], json!(devserver::PHONE_PATH));
        assert!(p.notes().iter().any(|n| matches!(n, Note::NotTheDevice { width, .. } if *width == preset.width)), "{:?}", p.notes());
        s.close();
        p.close();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── the Backend during a Preview (WASM-008 R6) ──

    /// A Web project with a Backend, made by the IDE's own scaffold; its
    /// `backend.toml` listens on `listen`, its manifest names `backend_url`.
    fn backend_project(tag: &str, listen: &str, backend_url: &str) -> PathBuf {
        let dir = scratch(tag).join("Shop");
        use cobolt_project_tools::create::{create_project_with, CreateRequest, BACKEND_CONFIG};
        create_project_with(&dir, &CreateRequest::new("Shop").template("web")).expect("a Web project");
        std::fs::write(dir.join(BACKEND_CONFIG), format!("[server]\nlisten = \"{listen}\"\n")).unwrap();
        let manifest = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.to_string_lossy().ends_with(".project.toml"))
            .expect("a manifest");
        if !backend_url.is_empty() {
            let text = std::fs::read_to_string(&manifest).unwrap();
            let mut doc: toml_edit::DocumentMut = text.parse().unwrap();
            doc["web"]["backend_url"] = toml_edit::value(backend_url);
            std::fs::write(&manifest, doc.to_string()).unwrap();
        }
        manifest
    }

    /// `target/<profile>/rcrun` (built by `cargo build -p cobolt-cli --bin rcrun`).
    fn built_rcrun() -> PathBuf {
        let exe = std::env::current_exe().unwrap();
        let rcrun = exe.parent().and_then(|d| d.parent()).unwrap().join(format!("rcrun{}", std::env::consts::EXE_SUFFIX));
        assert!(rcrun.exists(), "{} is missing: run `cargo build -p cobolt-cli --bin rcrun` first", rcrun.display());
        rcrun
    }

    /// A free port of `range`, a share of the ports these tests may use (8450-8469):
    /// each test its own share, so tests running together never pick the same one.
    fn free_test_port(range: std::ops::Range<u16>) -> u16 {
        range.clone().find(|p| devbackend::loopback_port_is_free(*p)).unwrap_or_else(|| panic!("no free port in {range:?}"))
    }

    /// What the IDE does every frame, until `done`: the Backend's events to the Preview, then the Preview.
    fn pump_until(p: &mut WebPreview, ide: &mut Option<BackendRun>, what: &str, secs: u64, mut done: impl FnMut(&WebPreview) -> bool) -> Vec<BackendEvent> {
        let deadline = Instant::now() + Duration::from_secs(secs);
        let mut events = Vec::new();
        while !done(p) {
            assert!(Instant::now() < deadline, "timed out waiting for {what}; notes: {:?}; events: {events:?}", p.notes());
            events.extend(p.pump_backend(ide));
            p.poll(Instant::now());
            std::thread::sleep(Duration::from_millis(20));
        }
        events
    }

    fn get_health(url: &str) -> String {
        use std::io::{Read, Write};
        let port: u16 = url.trim_start_matches("ws://127.0.0.1:").split('/').next().unwrap().parse().unwrap();
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        write!(s, "GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
        let mut raw = String::new();
        let _ = s.read_to_string(&mut raw);
        raw
    }

    /// The manifest names no Backend address: Run starts the Backend on the
    /// port of `backend.toml`, waits for it, and builds the page for the
    /// address it reports; nothing is built before it listens.
    #[test]
    fn a_preview_starts_the_backend_waits_for_it_and_builds_the_page_for_where_it_listens() {
        quiet();
        let rcrun = built_rcrun();
        let port = free_test_port(8450..8455);
        let manifest = backend_project("be-start", &format!("127.0.0.1:{port}"), "");
        let backend = Fake::new();
        let (mut p, mut ide) = (WebPreview::default(), None);
        let t = Instant::now();
        assert_eq!(p.start_with_backend(launch(&manifest, backend.clone()), Intent::Show, &mut ide, &rcrun), Some(true));
        assert!(p.awaiting_backend() && p.active() && ide.is_some());
        assert_eq!(backend.builds.load(Ordering::SeqCst), 0, "nothing is built before the Backend listens");
        assert!(matches!(&p.notes()[0], Note::BackendStarting { listen, .. } if *listen == format!("127.0.0.1:{port}")), "{:?}", p.notes());
        let events = pump_until(&mut p, &mut ide, "the page to be served", 90, |p| p.serving());
        let url = format!("ws://127.0.0.1:{port}/ws");
        assert!(events.contains(&BackendEvent::Listening(url.clone())), "{events:?}");
        assert_eq!(*backend.urls.lock().unwrap(), [url.clone()], "the page is built for the address the Backend reports");
        assert_eq!(p.baked_backend_url(), Some(url.as_str()));
        assert!(p.notes().contains(&Note::BackendReady { url: url.clone() }));
        assert!(!p.notes().contains(&Note::BackendOff), "{:?}", p.notes());
        assert!(get_health(&url).starts_with("HTTP/1.1 200"), "the Backend answers where the page calls");
        println!("backend listening and the page served {} ms after Run; built for {url}", t.elapsed().as_millis());
        // WASM-008 R6: the served config.json names the Backend and carries its token; the file does not.
        let root = p.live.as_ref().unwrap().server.root().to_path_buf();
        let on_disk = br#"{"backend":{"origins":[],"unsafe_plain_ws":false}}"#;
        std::fs::write(root.join("config.json"), on_disk).unwrap();
        let page = p.url().unwrap();
        let served = |p: &mut WebPreview, ide: &mut Option<BackendRun>| {
            let _ = p.pump_backend(ide);
            serde_json::from_str::<serde_json::Value>(&http_get(&page, "/config.json")).unwrap()
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut config = served(&mut p, &mut ide);
        while config["backend"].get("dev_token").is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
            config = served(&mut p, &mut ide);
        }
        let token = ide.as_ref().and_then(|r| r.token()).expect("the token was printed").to_owned();
        assert_eq!(config["backend"]["dev_token"], serde_json::json!(token), "{config}");
        assert_eq!(config["backend"]["unsafe_plain_ws"], serde_json::json!(true));
        assert_eq!(config["backend"]["origins"], serde_json::json!([url]));
        assert_eq!(std::fs::read(root.join("config.json")).unwrap(), on_disk.to_vec(), "the bundle is not touched");
        let mut stack = vec![root.clone()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let path = e.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    assert!(!String::from_utf8_lossy(&std::fs::read(&path).unwrap()).contains(&token), "{} carries the token", path.display());
                }
            }
        }
        let en = Language::English.tr();
        assert!(events.iter().all(|e| !matches!(e, BackendEvent::Line(l) if l.contains(&token))), "a line carried the token");
        assert!(p.notes().iter().all(|n| !n.text(&en).contains(&token)), "a note carried the token");
        assert!(p.notes().contains(&Note::TokenUnused { url: url.clone() }), "why calls will be refused: {:?}", p.notes());
        println!("served config.json carries the development token; config.json on disk byte-identical; no line or note carries it");
        // A rebuild after a change keeps the address.
        std::fs::write(manifest.parent().unwrap().join("webforms/extra.css"), "x{}").unwrap();
        pump_until(&mut p, &mut ide, "a rebuild", 30, |_| backend.builds.load(Ordering::SeqCst) >= 2);
        assert_eq!(backend.urls.lock().unwrap()[1], url, "a rebuild keeps the Backend's address");
        // Stopping the Preview stops the Backend it started (the IDE's rule in `show_web_preview`).
        p.close();
        assert!(!p.active());
        ide.take().unwrap().stop();
        assert!(devbackend::loopback_port_is_free(port), "port {port} is free again");
    }

    /// The port of `backend.toml` is taken: the Backend starts on a free one
    /// instead — before the build, so the page still calls where it listens.
    #[test]
    fn a_configured_port_in_use_moves_the_backend_and_the_page_with_it() {
        quiet();
        let rcrun = built_rcrun();
        let held = std::net::TcpListener::bind(("127.0.0.1", free_test_port(8455..8460))).unwrap();
        let busy = held.local_addr().unwrap().port();
        let manifest = backend_project("be-busy", &format!("127.0.0.1:{busy}"), "");
        let backend = Fake::new();
        let (mut p, mut ide) = (WebPreview::default(), None);
        assert_eq!(p.start_with_backend(launch(&manifest, backend.clone()), Intent::Show, &mut ide, &rcrun), Some(true));
        assert!(p.notes().contains(&Note::BackendPortBusy { port: busy }), "{:?}", p.notes());
        pump_until(&mut p, &mut ide, "the page to be served", 90, |p| p.serving());
        let url = backend.urls.lock().unwrap()[0].clone();
        assert!(url.starts_with("ws://127.0.0.1:") && !url.contains(&format!(":{busy}/")), "{url}");
        assert_eq!(ide.as_ref().and_then(|r| r.url()), Some(url.as_str()));
        assert!(get_health(&url).starts_with("HTTP/1.1 200"));
        println!("port {busy} held by another program: the Backend listens on {url} and the page is built for it");
        ide.take().unwrap().stop();
        drop(held);
    }

    /// A Backend the IDE already runs for the project is used as it is: no
    /// second start, and the Preview does not own it.
    #[test]
    fn a_backend_already_running_for_the_project_is_used_not_restarted() {
        quiet();
        let rcrun = built_rcrun();
        let manifest = backend_project("be-reuse", "127.0.0.1:0", "");
        let mut ide = Some(BackendRun::spawn(&rcrun, manifest.parent().unwrap()).unwrap());
        let deadline = Instant::now() + Duration::from_secs(60);
        while ide.as_ref().unwrap().url().is_none() {
            assert!(Instant::now() < deadline);
            let _ = ide.as_mut().unwrap().poll();
            std::thread::sleep(Duration::from_millis(20));
        }
        let url = ide.as_ref().unwrap().url().unwrap().to_owned();
        let backend = Fake::new();
        let mut p = WebPreview::default();
        assert_eq!(p.start_with_backend(launch(&manifest, backend.clone()), Intent::Show, &mut ide, &rcrun), Some(false));
        pump_until(&mut p, &mut ide, "the page to be served", 30, |p| p.serving());
        assert_eq!(*backend.urls.lock().unwrap(), [url.clone()]);
        assert!(!p.notes().iter().any(|n| matches!(n, Note::BackendStarting { .. })), "{:?}", p.notes());
        ide.take().unwrap().stop();
    }

    /// The manifest's address is honoured: elsewhere, nothing is started;
    /// on this machine but taken, nothing is started and nothing moves.
    #[test]
    fn a_manifest_address_is_honoured_as_it_is() {
        quiet();
        let rcrun = PathBuf::from("/nonexistent/rcrun");
        let manifest = backend_project("be-remote", "127.0.0.1:0", "wss://api.example.com/ws");
        let backend = Fake::new();
        let (mut p, mut ide) = (WebPreview::default(), None);
        assert_eq!(p.start_with_backend(launch(&manifest, backend.clone()), Intent::Show, &mut ide, &rcrun), Some(false));
        assert!(ide.is_none());
        wait_until(&mut p, "the page to be served", |p| p.serving());
        assert_eq!(*backend.urls.lock().unwrap(), ["wss://api.example.com/ws"]);
        assert!(p.notes().contains(&Note::BackendRemote { url: "wss://api.example.com/ws".into() }));
        assert_eq!(p.baked_backend_url(), None);

        let held = std::net::TcpListener::bind(("127.0.0.1", free_test_port(8460..8465))).unwrap();
        let taken = format!("ws://127.0.0.1:{}/ws", held.local_addr().unwrap().port());
        let manifest = backend_project("be-taken", "127.0.0.1:0", &taken);
        let backend = Fake::new();
        let mut p = WebPreview::default();
        assert_eq!(p.start_with_backend(launch(&manifest, backend.clone()), Intent::Show, &mut ide, &rcrun), Some(false));
        assert!(ide.is_none());
        wait_until(&mut p, "the page to be served", |p| p.serving());
        assert_eq!(*backend.urls.lock().unwrap(), [taken.clone()]);
        assert!(p.notes().contains(&Note::BackendTaken { url: taken }));
        drop(held);
    }

    /// A Backend that cannot start says why, in its own words, and nothing is
    /// built; one that never says where it listens is given up on in time.
    #[cfg(unix)]
    #[test]
    fn a_backend_that_fails_or_never_listens_is_said_and_stops_the_preview() {
        use std::os::unix::fs::PermissionsExt;
        quiet();
        let manifest = backend_project("be-fail", "127.0.0.1:0", "");
        let dir = manifest.parent().unwrap().to_path_buf();
        let script = |name: &str, body: &str| {
            let path = dir.join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path
        };
        let failing = script("failing-rcrun", "echo 'rcrun backend: backend.toml: server.listen is wrong' >&2\nexit 1");
        let backend = Fake::new();
        let (mut p, mut ide) = (WebPreview::default(), None);
        assert_eq!(p.start_with_backend(launch(&manifest, backend.clone()), Intent::Show, &mut ide, &failing), Some(true));
        pump_until(&mut p, &mut ide, "the failure", 30, |p| !p.awaiting_backend());
        let failed = p.notes().iter().find_map(|n| if let Note::BackendFailed { code, why } = n { Some((*code, why.clone())) } else { None });
        assert_eq!(failed, Some((Some(1), vec!["rcrun backend: backend.toml: server.listen is wrong".to_owned()])), "{:?}", p.notes());
        assert!(!p.active() && backend.builds.load(Ordering::SeqCst) == 0, "nothing built, nothing left running");

        let silent = script("silent-rcrun", "exec sleep 30");
        let mut p = WebPreview::default();
        let mut ide = None;
        assert_eq!(p.start_with_backend(launch(&manifest, backend.clone()), Intent::Show, &mut ide, &silent), Some(true));
        p.poll(Instant::now() + BACKEND_WAIT + Duration::from_millis(1));
        assert!(p.notes().contains(&Note::BackendTimeout { secs: BACKEND_WAIT.as_secs() }), "{:?}", p.notes());
        assert!(!p.active(), "the Preview stops; the IDE then stops the Backend it started");
        ide.take().unwrap().stop();

        let missing = PathBuf::from("/nonexistent/rcrun");
        let mut p = WebPreview::default();
        assert_eq!(p.start_with_backend(launch(&manifest, backend.clone()), Intent::Show, &mut None, &missing), Some(false));
        assert!(p.notes().iter().any(|n| matches!(n, Note::BackendFailed { code: None, why } if why[0].contains("/nonexistent/rcrun"))), "{:?}", p.notes());
        assert_eq!(backend.builds.load(Ordering::SeqCst), 0);
    }

    /// The Run of a real project, by hand: what the IDE's Run does for a Web
    /// project with a Backend — the generator the IDE links, the real
    /// `rcrun backend run`, the development server — kept up until a stop file
    /// appears, so that a browser can look at the page.
    /// `PRC_PROOF_MANIFEST=<manifest> PRC_PROOF_URL_FILE=<file> PRC_PROOF_STOP_FILE=<file>
    /// cargo test -p cobolt-ide --bin cobolt-ide proof_run_for_web -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn proof_run_for_web_starts_the_backend_and_serves_a_page_built_for_it() {
        quiet();
        let manifest = PathBuf::from(std::env::var("PRC_PROOF_MANIFEST").expect("PRC_PROOF_MANIFEST"));
        let url_file = PathBuf::from(std::env::var("PRC_PROOF_URL_FILE").expect("PRC_PROOF_URL_FILE"));
        let stop_file = PathBuf::from(std::env::var("PRC_PROOF_STOP_FILE").expect("PRC_PROOF_STOP_FILE"));
        let en = Language::English.tr();
        // `web_player_dir()` answers `assets/web-player` of the working folder outside an install.
        std::env::set_current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
        let launch = Launch {
            manifest: manifest.clone(),
            name: "proof".into(),
            kind: ProjectKind::Web,
            platform: None,
            device: None,
            orientation: None,
            devices: Arc::new(NoDeviceTools),
            has_backend: true,
            backend: Arc::new(cobolt_project_tools::web::generator::GeneratorBackend),
            browser: BrowserChoice::System,
            path_confirmed: false,
            page_strings: page_strings(&en),
            language: Language::English,
            backend_url: None,
        };
        let (mut p, mut ide) = (WebPreview::default(), None);
        let t = Instant::now();
        let started = p.start_with_backend(launch, Intent::Show, &mut ide, &built_rcrun());
        println!("start_with_backend -> {started:?}");
        let say = |p: &mut WebPreview, events: Vec<BackendEvent>| {
            for n in p.take_outbox() {
                for l in n.lines(&en) {
                    println!("[{:>6} ms] note: {l}", t.elapsed().as_millis());
                }
            }
            for e in events {
                let text = match e {
                    BackendEvent::Line(l) if looks_like_a_token(&l) => "<development token>".to_owned(),
                    other => format!("{other:?}"),
                };
                println!("[{:>6} ms] backend: {text}", t.elapsed().as_millis());
            }
        };
        let deadline = Instant::now() + Duration::from_secs(300);
        while !p.serving() {
            assert!(p.active(), "the Preview ended: {:?}", p.notes());
            assert!(Instant::now() < deadline, "not served in 300 s: {:?}", p.notes());
            let events = p.pump_backend(&mut ide);
            p.poll(Instant::now());
            say(&mut p, events);
            std::thread::sleep(Duration::from_millis(20));
        }
        let page = p.url().unwrap();
        let baked = p.baked_backend_url().unwrap_or("(the manifest's)").to_owned();
        std::fs::write(&url_file, format!("{page}\n{baked}\n")).unwrap();
        println!("SERVED {page} in {} ms; page built for {baked}; backend {:?}", t.elapsed().as_millis(), ide.as_ref().and_then(|r| r.url()));
        let hold = Instant::now() + Duration::from_secs(900);
        while !stop_file.exists() && Instant::now() < hold {
            let events = p.pump_backend(&mut ide);
            p.poll(Instant::now());
            say(&mut p, events);
            std::thread::sleep(Duration::from_millis(50));
        }
        // What the IDE does on Stop: the Preview closes, the Backend it started stops.
        p.close();
        if started == Some(true) {
            if let Some(mut run) = ide.take() {
                run.stop();
            }
        }
        println!("STOPPED: preview active {}, backend {}", p.active(), if ide.is_some() { "kept (not started by the Preview)" } else { "stopped" });
    }

    /// No English text reaches a widget from this file: every word the
    /// developer reads is a `Tr` field or a fact (a number, an address).
    #[test]
    fn this_file_has_no_literal_text_for_a_widget() {
        let src = include_str!("web_preview.rs");
        let body = src.split("#[cfg(test)]").next().unwrap();
        for (n, line) in body.lines().enumerate() {
            let t = line.trim_start();
            if t.starts_with("//") {
                continue;
            }
            for needle in ["Label::new(\"", "Button::new(\"", ".button(\"", ".label(\"", "on_hover_text(\"", "check(ui, width, &mut self.controls.dark, \""] {
                if let Some(at) = t.find(needle) {
                    let rest = &t[at + needle.len()..];
                    assert!(!rest.chars().next().is_some_and(|c| c.is_alphabetic()), "line {}: a literal word reaches a widget: {t}", n + 1);
                }
            }
        }
    }

    /// WASM-013 T8: an `unsafe` setting and a possible secret are said in each language with their key or place;
    /// a secret that stops the Build keeps the English line and gets the scan's heading.
    #[test]
    fn the_gate_lines_read_in_all_six_languages() {
        use cobolt_project_tools::web::WebDiagnosticKind::Other;
        let on = WebDiagnostic { location: "manifest".into(), kind: Other, subject: String::new(), message: "web.unsafe.plain_ws is on: for development only; a release build refuses it".into(), alternative: String::new() };
        let refused = WebDiagnostic { message: "web.unsafe.plain_ws is on: a release build refuses it; remove it from the manifest".into(), ..on.clone() };
        let possible = WebDiagnostic { location: "webforms/a.wcfrm:7".into(), kind: Other, subject: "high-entropy string".into(), message: "possible secret (high-entropy string); the value is not shown".into(), alternative: String::new() };
        let secret = WebDiagnostic { location: "webforms/a.wcfrm:9".into(), kind: Other, subject: "cloud access key id".into(), message: "secret (cloud access key id); the value is not shown".into(), alternative: String::new() };
        let mut distinct = std::collections::BTreeSet::new();
        for &lang in Language::ALL {
            let tr = lang.tr();
            let a = diagnostic_text(&on, &tr);
            let b = diagnostic_text(&refused, &tr);
            let c = diagnostic_text(&possible, &tr);
            for t in [&a, &b] {
                assert!(t.contains("web.unsafe.plain_ws") && !t.contains("{}"), "{lang:?}: {t}");
            }
            assert!(c.contains("webforms/a.wcfrm:7") && c.contains("high-entropy string") && !c.contains("{}"), "{lang:?}: {c}");
            assert!(!tr.pk_scan_failed.trim().is_empty());
            distinct.insert(a);
            let lines = Note::BuildRefused(vec![secret.clone()]).lines(&tr);
            assert_eq!(lines[0], tr.pk_scan_failed);
            assert!(lines[1].contains("secret (cloud access key id)") && !lines[1].contains("{}"), "{lines:?}");
            assert_eq!(Note::BuildRefused(vec![refused.clone()]).lines(&tr).len(), 1, "an unsafe refusal needs no heading");
        }
        assert_eq!(distinct.len(), Language::ALL.len(), "six different sentences");
        println!("gate lines: unsafe on/refused, possible secret, secret heading read in {} languages", Language::ALL.len());
    }
}
