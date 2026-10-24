// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The web Preview, on the IDE's side** (WASM-008, WASM-002 R8).
//!
//! A Web or Mobile project is not run by the interpreter: its Build produces a
//! bundle, a small server hands the bundle to a web view, and the web view is
//! the Preview. This module is everything the IDE does around that:
//!
//! 1. a **build job** on a thread of its own, through the
//!    [`WebBuildBackend`] the program linked (the IDE never waits for it);
//! 2. the **development server** that serves the bundle on a loopback port;
//! 3. the **web view** — the helper process, or the system browser when the
//!    helper is not installed — and the commands the controls send it;
//! 4. a **watcher** over the project's own folders: when a form, a stylesheet,
//!    a program or the Backend changes on disk, the bundle is built again and
//!    the open page reloads itself (live reload);
//! 5. the **controls window**: device preset, orientation, pixel ratio, colour
//!    scheme, language, safe-area insets, Reload, Developer tools, Open in the
//!    browser, Close.
//!
//! What it says goes through [`Note`]s — a message with its facts, not its
//! words — so the Output panel and the controls window both show it in the
//! language the developer chose, from the `Tr` tables.
//!
//! **The controls window never changes size by itself.** Its width and its
//! height are constants; every row in it has a fixed height and its text is
//! truncated, never negotiated; the log at the bottom is a scroll area that
//! takes whatever the rows leave. There is nothing in it that can ask for
//! more room, in any of the six languages (`tests::the_controls_window_keeps_its_size`).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cobolt_compiler::project_kind::{DeviceClass, Isolation, Orientation, ProjectKind};
use cobolt_project_tools::web::devserver::{self, ServeOptions, ServerHandle};
use cobolt_project_tools::web::presets::{self, PresetClass, Viewport, PIXEL_RATIOS, PRESETS};
use cobolt_project_tools::web::preview::{self, Command, Emulation, Event, Mode, PreviewSession};
use cobolt_project_tools::web::{BuildReport, PartSize, PlayerPinError, WebBuildBackend, WebBuildError, WebBuildRequest, WebDiagnostic, WebDiagnosticKind};

use crate::i18n::Tr;

/// How often the watcher looks at the project's folders.
pub const WATCH_EVERY: Duration = Duration::from_millis(400);
/// The controls window's size. Constants, on purpose.
pub const WINDOW_W: f32 = 420.0;
/// The body's height; the window is this plus its chrome (title bar and margins).
pub const BODY_H: f32 = 480.0;
/// The notes the window keeps.
const MAX_NOTES: usize = 300;
/// A page that logs in a loop must not flood the Output panel: this many
/// console and error lines are taken from the helper at a time, the rest of
/// that burst is dropped.
const CHATTER_PER_POLL: usize = 40;

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
    HelperFailed(String),
    ModeWebView,
    ModeBrowser,
    /// The page tried to go to this address.
    Blocked(String),
    /// A rebuild finished this long after the change that caused it.
    Updated { ms: u64 },
    Closed,
    BackendOff,
    Console { level: String, text: String },
    PageError { message: String, source: String, line: u32 },
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
    };
    let text = match template {
        Some(t) => fill(t, &[&d.subject]),
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
            | Note::HelperFailed(_)
            | Note::PageError { .. } => {
                Severity::Error
            }
            Note::Console { level, .. } if level == "error" => Severity::Error,
            Note::NotLinked
            | Note::NoBundle
            | Note::BuildWarning(_)
            | Note::ModeBrowser
            | Note::Blocked(_)
            | Note::BackendOff => Severity::Warn,
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
                let mut out = vec![tr.pk_build_refused.to_owned()];
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
            Note::HelperFailed(why) => vec![fill(tr.pk_prev_helper_failed, &[why])],
            Note::ModeWebView => vec![tr.pk_prev_mode_webview.to_owned()],
            Note::ModeBrowser => vec![tr.pk_prev_mode_browser.to_owned()],
            Note::Blocked(url) => vec![fill(tr.pk_prev_blocked, &[url])],
            Note::Updated { ms } => vec![fill(tr.pk_prev_updated, &[&ms.to_string()])],
            Note::Closed => vec![tr.pk_prev_closed.to_owned()],
            Note::BackendOff => vec![tr.pk_prev_backend_off.to_owned()],
            Note::Console { level, text } => vec![fill(tr.pk_prev_console, &[level, text])],
            Note::PageError { message, source, line } => {
                vec![fill(tr.pk_prev_error, &[message, source, &line.to_string()])]
            }
        }
    }

    /// The note as one piece of text.
    pub fn text(&self, tr: &Tr) -> String {
        self.lines(tr).join("\n")
    }
}

// ── The controls ────────────────────────────────────────────────────────────

/// What the developer has chosen to preview the page as.
#[derive(Debug, Clone, PartialEq)]
pub struct Controls {
    /// A [`presets::PRESETS`] id.
    pub preset: &'static str,
    pub landscape: bool,
    pub dpr: f32,
    pub dark: bool,
    /// A language tag; empty leaves the web view's own.
    pub locale: String,
    /// Tell the page the device's safe-area insets.
    pub safe_area: bool,
}

impl Default for Controls {
    fn default() -> Self {
        Self::initial(ProjectKind::Web, None, None, "")
    }
}

impl Controls {
    /// The controls a project's Preview opens with: the preset its kind and
    /// device ask for, in the orientation the project declares.
    pub fn initial(kind: ProjectKind, device: Option<DeviceClass>, orientation: Option<Orientation>, locale: &str) -> Self {
        let preset = presets::default_for(kind, device);
        Self {
            preset: preset.id,
            // A desktop page is wide by nature; a phone or a tablet is upright
            // unless the project says landscape ("follows the device" opens upright).
            landscape: preset.class == PresetClass::Desktop || orientation == Some(Orientation::Landscape),
            dpr: preset.dpr,
            dark: false,
            locale: locale.trim().to_owned(),
            safe_area: preset.class != PresetClass::Desktop,
        }
    }

    /// Choose another device: its pixel ratio and its insets come with it, and
    /// so does its natural orientation when it is a different kind of screen
    /// (a desktop is wide, a phone upright).
    pub fn choose(&mut self, id: &'static str) {
        if let Some(p) = presets::preset(id) {
            let was_desktop = presets::preset(self.preset).is_some_and(|q| q.class == PresetClass::Desktop);
            let is_desktop = p.class == PresetClass::Desktop;
            if was_desktop != is_desktop {
                self.landscape = is_desktop;
            }
            self.preset = p.id;
            self.dpr = p.dpr;
            self.safe_area = !is_desktop;
        }
    }

    /// The device in its orientation, at the chosen pixel ratio.
    pub fn viewport(&self) -> Viewport {
        let preset = presets::preset(self.preset).unwrap_or(&PRESETS[0]);
        preset.viewport(self.landscape).with_dpr(self.dpr)
    }

    /// The window's size, in points.
    pub fn window(&self) -> (u32, u32) {
        self.viewport().window_size()
    }

    /// The insets the page is told: top, right, bottom, left.
    pub fn insets(&self) -> [u32; 4] {
        if self.safe_area {
            let s = self.viewport().safe_area;
            [s.top, s.right, s.bottom, s.left]
        } else {
            [0; 4]
        }
    }

    /// What an `open` carries.
    pub fn emulation(&self) -> Emulation {
        Emulation {
            dpr: Some(self.dpr),
            dark: Some(self.dark),
            locale: (!self.locale.is_empty()).then(|| self.locale.clone()),
            safe_area: Some(self.insets()),
        }
    }

    /// The commands that bring a web view showing `before` to this: nothing
    /// for what did not change, and **one** `Emulate` for the pixel ratio, the
    /// language and the insets together, so the page loads once.
    pub fn changes_from(&self, before: &Controls) -> Vec<Command> {
        let mut out = Vec::new();
        if self.window() != before.window() {
            let (width, height) = self.window();
            out.push(Command::Size { width, height });
        }
        if self.dark != before.dark {
            out.push(Command::Scheme { dark: self.dark });
        }
        let (now, was) = (self.emulation(), before.emulation());
        if now.dpr != was.dpr || now.locale != was.locale || now.safe_area != was.safe_area {
            out.push(Command::Emulate {
                dpr: (now.dpr != was.dpr).then_some(self.dpr),
                locale: if now.locale != was.locale { now.locale.clone() } else { None },
                safe_area: (now.safe_area != was.safe_area).then(|| self.insets()),
            });
        }
        out
    }
}

/// Is `tag` a language tag a page can be told (`pt-BR`, `en`, `zh-Hans-CN`)?
pub fn valid_locale(tag: &str) -> bool {
    let t = tag.trim();
    (2..=35).contains(&t.len())
        && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && t.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && !t.starts_with('-')
        && !t.ends_with('-')
        && !t.contains("--")
}

/// The height of a row of the controls window.
const ROW_H: f32 = 22.0;

/// The height of a button of the controls window.
const BUTTON_H: f32 = 24.0;
/// The width of the column of names at the left of the controls.
const LABEL_W: f32 = 130.0;

/// A button whose text is cut at the button's edge, never wrapped.
fn button(text: &str) -> egui::Button<'_> {
    egui::Button::new(text).truncate()
}

/// A button of exactly `size`, enabled or not, its text also on hover.
fn sized_button(ui: &mut egui::Ui, enabled: bool, size: [f32; 2], text: &str) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| ui.add_sized(size, button(text))).inner.on_hover_text(text)
}

/// A checkbox in a box of exactly `width` x [`ROW_H`] whose label is cut, never
/// wrapped, so no language can make the row taller or wider.
fn check(ui: &mut egui::Ui, width: f32, value: &mut bool, text: &str) {
    let size = egui::vec2(width.max(20.0), ROW_H);
    ui.allocate_ui_with_layout(size, egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.set_min_size(size);
        ui.set_max_size(size);
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        ui.checkbox(value, text).on_hover_text(text);
    });
}

/// A preset's name in the developer's language.
pub fn preset_text<'a>(tr: &'a Tr, id: &str) -> &'a str {
    match id {
        "phone-compact" => tr.pk_ps_phone_compact,
        "phone" => tr.pk_ps_phone,
        "phone-large" => tr.pk_ps_phone_large,
        "tablet" => tr.pk_ps_tablet,
        "tablet-large" => tr.pk_ps_tablet_large,
        "desktop" => tr.pk_ps_desktop,
        "desktop-wide" => tr.pk_ps_desktop_wide,
        _ => "",
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
}

// ── The Preview ─────────────────────────────────────────────────────────────

/// Where a started Preview is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The system web view (the helper process), with the device controls.
    WebView,
    /// The system browser — what *Run* does for a Web or Mobile project.
    Browser,
}

/// What a start asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// Build the bundle and say how it went.
    Build,
    /// Build, serve and show.
    Show(Target),
}

/// What the Preview needs to know about the project it was started for.
#[derive(Clone)]
pub struct Launch {
    pub manifest: PathBuf,
    /// The project's name, for the window title and the *Serving* note.
    pub name: String,
    /// The OS window's title, in the developer's language.
    pub title: String,
    pub kind: ProjectKind,
    pub device: Option<DeviceClass>,
    pub orientation: Option<Orientation>,
    /// The project's language tag; empty when it has none.
    pub locale: String,
    pub has_backend: bool,
    pub backend: Arc<dyn WebBuildBackend>,
    /// The helper process, if it is installed.
    pub helper: Option<PathBuf>,
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

struct Live {
    server: ServerHandle,
    session: PreviewSession,
    watcher: SourceWatcher,
    /// What the web view was last told, so a change sends only the difference.
    applied: Controls,
}

/// The web Preview of the open project. One at a time.
#[derive(Default)]
pub struct WebPreview {
    launch: Option<Launch>,
    job: Option<Job>,
    live: Option<Live>,
    /// A change seen while a build was running: build again when it ends.
    rebuild_after: Option<Instant>,
    pub controls: Controls,
    locale_edit: String,
    notes: Vec<Note>,
    outbox: Vec<Note>,
    blocked: Option<String>,
    /// The height the log had in the last frame the window was drawn.
    log_h: f32,
    /// Is the controls window shown?
    pub open: bool,
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

    /// Is there anything running, building or serving?
    pub fn active(&self) -> bool {
        self.job.is_some() || self.live.is_some()
    }

    /// The address being served.
    pub fn url(&self) -> Option<String> {
        self.live.as_ref().map(|l| l.server.url())
    }

    /// How the page is shown, once it is.
    pub fn mode(&self) -> Option<Mode> {
        self.live.as_ref().map(|l| l.session.mode())
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
        if let Note::Blocked(url) = &note {
            self.blocked = Some(url.clone());
        }
        self.outbox.push(note.clone());
        self.notes.push(note);
        if self.notes.len() > MAX_NOTES {
            let drop = self.notes.len() - MAX_NOTES;
            self.notes.drain(..drop);
        }
    }

    /// Start: build, then (for [`Intent::Show`]) serve and show. `false` when a
    /// build is already running.
    pub fn start(&mut self, launch: Launch, intent: Intent) -> bool {
        if self.job.is_some() {
            return false;
        }
        if intent != Intent::Build {
            // A Preview already showing is replaced, not stacked.
            self.live = None;
            self.notes.clear();
            self.blocked = None;
            self.controls = Controls::initial(launch.kind, launch.device, launch.orientation, &launch.locale);
            self.locale_edit = self.controls.locale.clone();
            self.open = true;
        }
        self.say(Note::Building);
        self.spawn_build(&launch, JobKind::Initial(intent));
        self.launch = Some(launch);
        true
    }

    fn spawn_build(&mut self, launch: &Launch, kind: JobKind) {
        let (tx, rx) = mpsc::channel();
        let backend = launch.backend.clone();
        let manifest = launch.manifest.clone();
        let spawned = std::thread::Builder::new().name("web-build".into()).spawn(move || {
            let done = match WebBuildRequest::from_manifest(&manifest) {
                Err(e) => Done { outcome: Err(WebBuildError::Failed(e)), bundle: None, isolation: Isolation::Off },
                Ok(mut request) => Done {
                    bundle: Some(backend.bundle_dir(&request)),
                    isolation: request.web.isolation,
                    outcome: {
                        // The player shipped beside this IDE (WASM-016 T14), not a path of the machine that built it.
                        request.player_dir = Some(crate::app::web_player_dir());
                        backend.build(&request)
                    },
                },
            };
            let _ = tx.send(done);
        });
        match spawned {
            Ok(_) => self.job = Some(Job { rx, kind }),
            Err(e) => self.say(Note::BuildFailed(e.to_string())),
        }
    }

    /// End everything: the web view, the server, the watcher. A build that is
    /// still running finishes on its thread and its result is dropped.
    pub fn stop(&mut self) {
        self.live = None;
        self.job = None;
        self.rebuild_after = None;
    }

    /// Close the Preview and its window.
    pub fn close(&mut self) {
        self.stop();
        self.open = false;
    }

    /// Look at everything that moves: the build, the web view's events, the
    /// sources. `true` while the Preview wants another frame soon.
    pub fn poll(&mut self, now: Instant) -> bool {
        self.poll_job();
        self.poll_events();
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
        match kind {
            // A Build ends here; a Mobile one says where it stops.
            JobKind::Initial(Intent::Build) => {
                if built && self.launch.as_ref().is_some_and(|l| l.kind == ProjectKind::Mobile) {
                    self.say(Note::PackageNote);
                }
            }
            JobKind::Initial(Intent::Show(target)) => {
                // A build that worked is shown. So is the bundle that is already
                // there when no generator is linked — it may have been made
                // somewhere else. A build that failed shows nothing: an old page
                // under a new error would mislead.
                match done.bundle {
                    Some(bundle) if built || not_linked => self.serve(&bundle, done.isolation, target),
                    Some(_) => {}
                    None => self.say(Note::NoBundle),
                }
            }
            JobKind::Rebuild(changed) => {
                if built {
                    if let Some(live) = &self.live {
                        live.server.notify_changed();
                    }
                    let ms = Instant::now().saturating_duration_since(changed).as_millis().min(u128::from(u64::MAX)) as u64;
                    self.say(Note::Updated { ms });
                }
            }
        }
    }

    fn serve(&mut self, bundle: &Path, isolation: Isolation, target: Target) {
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
        let url = server.url();
        self.say(Note::Serving { name: launch.name.clone(), url: url.clone() });
        let (w, h) = self.controls.window();
        let helper = if target == Target::WebView { launch.helper.as_deref() } else { None };
        let session = match PreviewSession::start(&url, &launch.title, w, h, &self.controls.emulation(), helper) {
            Ok(s) => s,
            Err(e) => {
                self.say(Note::HelperFailed(e));
                return;
            }
        };
        self.say(if session.mode() == Mode::WebView { Note::ModeWebView } else { Note::ModeBrowser });
        if launch.has_backend {
            self.say(Note::BackendOff);
        }
        let watcher = SourceWatcher::new(&launch.manifest, launch.kind, launch.has_backend);
        self.live = Some(Live { server, session, watcher, applied: self.controls.clone() });
    }

    fn poll_events(&mut self) {
        let events = match self.live.as_mut() {
            Some(l) => l.session.poll(),
            None => return,
        };
        let mut chatter = 0usize;
        for event in events {
            match event {
                Event::Ready => {}
                Event::Console { level, text } => {
                    chatter += 1;
                    if chatter <= CHATTER_PER_POLL {
                        self.say(Note::Console { level, text });
                    }
                }
                Event::Error { message, source, line } => {
                    chatter += 1;
                    if chatter <= CHATTER_PER_POLL {
                        self.say(Note::PageError { message, source, line });
                    }
                }
                Event::Blocked { url } => self.say(Note::Blocked(url)),
                Event::Failed { message } => {
                    // No web view here: say why, and show the page in the browser instead.
                    self.say(Note::HelperFailed(message));
                    self.fall_back_to_the_browser();
                    return;
                }
                Event::Closed => {
                    self.live = None;
                    self.rebuild_after = None;
                    self.say(Note::Closed);
                    return;
                }
            }
        }
    }

    fn fall_back_to_the_browser(&mut self) {
        let Some(title) = self.launch.as_ref().map(|l| l.title.clone()) else { return };
        let (w, h) = self.controls.window();
        let emulation = self.controls.emulation();
        let Some(live) = self.live.as_mut() else { return };
        live.session.close();
        match PreviewSession::start(&live.server.url(), &title, w, h, &emulation, None) {
            Ok(session) => {
                live.session = session;
                self.say(Note::ModeBrowser);
            }
            Err(e) => {
                self.live = None;
                self.say(Note::HelperFailed(e));
            }
        }
    }

    /// Send the web view what changed in the controls since it was last told.
    pub fn apply_controls(&mut self) {
        let Some(live) = self.live.as_mut() else { return };
        if live.session.mode() != Mode::WebView || live.applied == self.controls {
            return;
        }
        for command in self.controls.changes_from(&live.applied) {
            let _ = live.session.send(&command);
        }
        live.applied = self.controls.clone();
    }

    /// Load the page again.
    pub fn reload(&mut self) {
        if let Some(live) = self.live.as_mut() {
            if live.session.mode() == Mode::WebView {
                let _ = live.session.send(&Command::Reload);
            } else {
                // The browser tab reloads itself when the server counts a change.
                live.server.notify_changed();
            }
        }
    }

    /// Open the web view's developer tools.
    pub fn devtools(&mut self) {
        if let Some(live) = self.live.as_mut() {
            let _ = live.session.send(&Command::Devtools);
        }
    }

    /// Show the served page in the system browser as well.
    pub fn open_in_browser(&mut self) {
        let Some(url) = self.url() else { return };
        if let Err(e) = preview::open_in_system_browser(&url) {
            self.say(Note::HelperFailed(e));
        }
    }

    // ── The controls window ─────────────────────────────────────────────────

    /// The controls window. Does nothing while it is not [`WebPreview::open`].
    pub fn show(&mut self, ctx: &egui::Context, tr: &Tr) {
        if !self.open {
            return;
        }
        let mut open = true;
        let screen = ctx.content_rect();
        let default_pos = egui::pos2((screen.right() - WINDOW_W - 24.0).max(12.0), (screen.top() + 84.0).max(12.0));
        egui::Window::new(egui::RichText::new(tr.pk_prev_title).strong())
            .id(egui::Id::new("web_preview_controls"))
            .open(&mut open)
            .collapsible(false)
            // NEVER `resizable(true)`: see the module comment.
            .resizable(false)
            // The outer width; the height is whatever the one exact body needs, plus
            // the same chrome every frame (`panels::fixed_dialog` explains why).
            .fixed_size([WINDOW_W, 0.0])
            .default_pos(default_pos)
            .show(ctx, |ui| {
                let body = egui::vec2(crate::panels::fixed_dialog::content_width(ui, WINDOW_W), BODY_H);
                ui.set_width(body.x);
                // The window learns its size from ONE allocation of exactly `body`.
                // The body is drawn in a child of that rectangle, which tells the
                // window nothing of what it draws and clips whatever would pass the
                // edge, so nothing in it — a language, a log, a button — can make
                // the window larger.
                let (rect, _) = ui.allocate_exact_size(body, egui::Sense::hover());
                let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)));
                inner.set_clip_rect(rect.intersect(ui.clip_rect()));
                self.body(&mut inner, tr, body.x, ctx);
            });
        if !open {
            self.close();
        }
    }

    fn body(&mut self, ui: &mut egui::Ui, tr: &Tr, width: f32, ctx: &egui::Context) {
        let before = self.controls.clone();
        let web_view = self.mode() == Some(Mode::WebView);
        // Rows close together: the window is a palette, not a form.
        ui.spacing_mut().item_spacing.y = 4.0;
        let half = ((width - ui.spacing().item_spacing.x) / 2.0).floor();

        // A name on the left in a column of LABEL_W (cut, with the whole name on hover),
        // its control in the rest of the row.
        let field_w = (width - LABEL_W - ui.spacing().item_spacing.x).max(40.0);
        let label = |ui: &mut egui::Ui, text: &str| {
            ui.add_sized([LABEL_W, ROW_H], egui::Label::new(text).truncate()).on_hover_text(text);
        };
        ui.add_enabled_ui(web_view, |ui| {
            ui.horizontal(|ui| {
                label(ui, tr.pk_prev_device);
                let shown = preset_text(tr, self.controls.preset).to_owned();
                egui::ComboBox::from_id_salt("web_preview_device").width(field_w).selected_text(shown).show_ui(ui, |ui| {
                    for p in PRESETS {
                        if ui.selectable_label(self.controls.preset == p.id, preset_text(tr, p.id)).clicked() {
                            self.controls.choose(p.id);
                        }
                    }
                });
            });
            check(ui, width, &mut self.controls.landscape, tr.pk_prev_landscape);
            ui.horizontal(|ui| {
                label(ui, tr.pk_prev_dpr);
                egui::ComboBox::from_id_salt("web_preview_dpr")
                    .width(field_w)
                    .selected_text(format!("{}×", self.controls.dpr))
                    .show_ui(ui, |ui| {
                        for &d in PIXEL_RATIOS {
                            ui.selectable_value(&mut self.controls.dpr, d, format!("{d}×"));
                        }
                    });
            });
            check(ui, width, &mut self.controls.dark, tr.pk_prev_dark);
            ui.horizontal(|ui| {
                label(ui, tr.pk_prev_locale);
                let edit = ui.add_sized([field_w, ROW_H], egui::TextEdit::singleline(&mut self.locale_edit).hint_text("en-US"));
                if edit.lost_focus() && valid_locale(&self.locale_edit) {
                    self.controls.locale = self.locale_edit.trim().to_owned();
                }
            });
            check(ui, width, &mut self.controls.safe_area, tr.pk_prev_safe_area);
        });
        // Whatever changed reaches the web view now, not on a later frame.
        if self.controls != before {
            self.apply_controls();
        }

        ui.add_space(4.0);
        let serving = self.live.is_some();
        // Every button is exactly `half` x BUTTON_H: no text and no state can change a row.
        let cell = [half, BUTTON_H];
        ui.horizontal(|ui| {
            if sized_button(ui, serving, cell, tr.pk_prev_reload).clicked() {
                self.reload();
            }
            if sized_button(ui, web_view, cell, tr.pk_prev_devtools).clicked() {
                self.devtools();
            }
        });
        ui.horizontal(|ui| {
            if sized_button(ui, serving, cell, tr.pk_prev_open_browser).clicked() {
                self.open_in_browser();
            }
            if sized_button(ui, self.active(), cell, tr.pk_prev_close).clicked() {
                self.close();
            }
        });

        // The page tried to leave: a button, always the same height, there or not.
        match (self.blocked.clone(), tr.pk_prev_open_blocked) {
            (Some(url), label) => {
                let external = url.starts_with("http://") || url.starts_with("https://");
                let response = ui.add_enabled_ui(external, |ui| ui.add_sized([width, BUTTON_H], button(label))).inner;
                if response.on_hover_text(&url).clicked() {
                    ctx.open_url(egui::OpenUrl::new_tab(url));
                }
            }
            (None, _) => {
                ui.allocate_space(egui::vec2(width, BUTTON_H));
            }
        }

        ui.separator();
        let log_h = ui.available_height().max(60.0);
        self.log_h = log_h;
        egui::ScrollArea::vertical()
            .id_salt("web_preview_log")
            .max_height(log_h)
            .min_scrolled_height(log_h)
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for note in &self.notes {
                    let colour = match note.severity() {
                        Severity::Info => ui.visuals().text_color(),
                        Severity::Warn => ui.visuals().warn_fg_color,
                        Severity::Error => ui.visuals().error_fg_color,
                    };
                    ui.add(egui::Label::new(egui::RichText::new(note.text(tr)).small().color(colour)).wrap());
                }
            });
    }
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
    }

    impl Fake {
        fn new() -> Arc<Self> {
            Arc::new(Self { builds: AtomicUsize::new(0), fail: Mutex::new(None), write_bundle: true })
        }
    }

    impl WebBuildBackend for Fake {
        fn name(&self) -> &str {
            "fake"
        }

        fn build(&self, request: &WebBuildRequest) -> Result<BuildReport, WebBuildError> {
            let n = self.builds.fetch_add(1, Ordering::SeqCst) + 1;
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

    fn launch(manifest: &Path, backend: Arc<dyn WebBuildBackend>, helper: Option<PathBuf>) -> Launch {
        Launch {
            manifest: manifest.to_path_buf(),
            name: "P".into(),
            title: "Web Preview - P".into(),
            kind: ProjectKind::Web,
            device: None,
            orientation: None,
            locale: "pt-BR".into(),
            has_backend: true,
            backend,
            helper,
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

    /// A stand-in for the helper: it logs every line it is given and says
    /// `closed` when asked to close.
    #[cfg(unix)]
    fn fake_helper(dir: &Path) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let log = dir.join("helper.log");
        let helper = dir.join("fake-helper");
        std::fs::write(
            &helper,
            format!(
                "#!/bin/sh\necho '{{\"event\":\"ready\"}}'\nwhile read line; do\n  echo \"$line\" >> '{}'\n  case \"$line\" in\n    *'\"cmd\":\"close\"'*) echo '{{\"event\":\"closed\"}}'; exit 0;;\n  esac\ndone\n",
                log.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();
        (helper, log)
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

    // ── controls ──

    #[test]
    fn a_projects_kind_and_device_choose_its_first_preview() {
        let web = Controls::initial(ProjectKind::Web, None, None, "");
        assert_eq!((web.preset, web.dpr, web.safe_area), ("desktop", 1.0, false));
        assert_eq!(web.window(), (1280, 800), "a desktop page opens wide");
        let phone = Controls::initial(ProjectKind::Mobile, Some(DeviceClass::Phone), Some(Orientation::Portrait), "pt-BR");
        assert_eq!((phone.preset, phone.dpr, phone.safe_area, phone.locale.as_str()), ("phone", 3.0, true, "pt-BR"));
        assert_eq!(phone.window(), (390, 844));
        let tablet = Controls::initial(ProjectKind::Mobile, Some(DeviceClass::Tablet), Some(Orientation::Landscape), "");
        assert_eq!(tablet.preset, "tablet");
        assert_eq!(tablet.window(), (1180, 820), "a landscape tablet is wider than tall");
        let auto = Controls::initial(ProjectKind::Mobile, Some(DeviceClass::Phone), Some(Orientation::Auto), "");
        assert_eq!(auto.window(), (390, 844), "'follows the device' opens upright");
    }

    #[test]
    fn choosing_a_device_brings_its_ratio_and_insets() {
        let mut c = Controls::initial(ProjectKind::Web, None, None, "");
        c.choose("phone-large");
        assert_eq!((c.dpr, c.safe_area, c.insets()), (3.0, true, [59, 0, 34, 0]));
        c.choose("desktop-wide");
        assert_eq!((c.dpr, c.safe_area, c.insets()), (1.0, false, [0; 4]));
        c.choose("no-such-preset");
        assert_eq!(c.preset, "desktop-wide", "an unknown id changes nothing");
    }

    #[test]
    fn a_change_sends_only_what_changed_and_loads_the_page_once() {
        let was = Controls::initial(ProjectKind::Mobile, Some(DeviceClass::Phone), Some(Orientation::Portrait), "pt-BR");
        assert!(was.changes_from(&was).is_empty());

        let mut now = was.clone();
        now.dark = true;
        assert_eq!(now.changes_from(&was), vec![Command::Scheme { dark: true }]);

        let mut turned = was.clone();
        turned.landscape = true;
        let cmds = turned.changes_from(&was);
        assert_eq!(cmds[0], Command::Size { width: 844, height: 390 });
        assert_eq!(cmds.len(), 2, "the insets turn with the screen: {cmds:?}");
        assert!(matches!(cmds[1], Command::Emulate { dpr: None, locale: None, safe_area: Some([0, 47, 20, 47]) }));

        // A new device changes size, ratio and insets: three things, ONE emulate (one reload).
        let mut tablet = was.clone();
        tablet.choose("tablet");
        let cmds = tablet.changes_from(&was);
        assert_eq!(cmds.iter().filter(|c| matches!(c, Command::Emulate { .. })).count(), 1);
        assert_eq!(cmds[0], Command::Size { width: 820, height: 1180 });
        assert_eq!(cmds[1], Command::Emulate { dpr: Some(2.0), locale: None, safe_area: Some([24, 0, 20, 0]) });

        let mut lang = was.clone();
        lang.locale = "ja".into();
        assert_eq!(lang.changes_from(&was), vec![Command::Emulate { dpr: None, locale: Some("ja".into()), safe_area: None }]);
    }

    #[test]
    fn only_a_language_tag_is_told_to_a_page() {
        for ok in ["en", "pt-BR", "zh-Hans-CN", "ja", "es-419"] {
            assert!(valid_locale(ok), "{ok}");
        }
        for bad in ["", "e", "-en", "en-", "en--US", "en US", "en_US", "1en", "en;alert(1)", "en'", &"a".repeat(36)] {
            assert!(!valid_locale(bad), "{bad:?}");
        }
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
        ];
        let mut said = 0;
        for &lang in Language::ALL {
            let tr = lang.tr();
            for kind in kinds {
                let d = WebDiagnostic { location: "webforms/a.cfrm:7".into(), kind, subject: "SUBJ-9".into(), message: "english".into() };
                let text = diagnostic_text(&d, &tr);
                assert!(text.starts_with("webforms/a.cfrm:7: "), "{lang:?} {kind:?}: {text}");
                assert!(text.contains("SUBJ-9") && !text.contains("{}") && !text.contains("english"), "{lang:?} {kind:?}: {text}");
                assert!(!text.to_lowercase().contains("cobolt"), "{lang:?} {kind:?}: {text}");
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
            Note::HelperFailed("none".into()),
            Note::ModeWebView,
            Note::ModeBrowser,
            Note::Blocked("https://example.com/".into()),
            Note::Updated { ms: 180 },
            Note::Closed,
            Note::BackendOff,
            Note::Console { level: "log".into(), text: "hi".into() },
            Note::PageError { message: "x is not defined".into(), source: "app.js".into(), line: 12 },
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
        assert_eq!(Note::Console { level: "error".into(), text: String::new() }.severity(), Severity::Error);
        assert_eq!(notes[14].severity(), Severity::Warn);
        // WASM-016 T27: the player's sizes, its parts on a line of their own, and the pin's facts in the sentence.
        let sizes = notes[22].lines(&en);
        assert_eq!(sizes, ["Player size: plain 2.71 MB, threaded 2.55 MB", "Loaded on first use: mermaid 2.12 MB"], "{sizes:?}");
        assert!(notes[23].text(&en).contains("1.0.0") && notes[23].text(&en).contains("2.0.0, 2.1.0"));
        assert!(notes[24].text(&en).contains("1.9.9") && notes[24].text(&en).contains("2.0.0"));
        assert_eq!(notes[25].severity(), Severity::Error);
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
        assert!(p.start(launch(&manifest, backend.clone(), None), Intent::Build));
        assert!(!p.start(launch(&manifest, backend.clone(), None), Intent::Build), "one build at a time");
        wait_until(&mut p, "the build", |p| !p.building());
        assert!(matches!(p.notes().last(), Some(Note::BuildRefused(l)) if l.len() == 1), "{:?}", p.notes());
        assert!(!p.serving());

        // Not linked, and no bundle: it says both.
        let mut p = WebPreview::default();
        let unlinked: Arc<dyn WebBuildBackend> = Arc::new(cobolt_project_tools::web::NotYetLinked);
        p.start(launch(&manifest, unlinked.clone(), None), Intent::Show(Target::WebView));
        wait_until(&mut p, "the build", |p| !p.building());
        assert_eq!(p.notes().iter().filter(|n| matches!(n, Note::NotLinked | Note::NoBundle)).count(), 2, "{:?}", p.notes());
        assert!(!p.serving());

        // Not linked, but a bundle is there from somebody else: it is shown (the stand-in helper).
        #[cfg(unix)]
        {
            std::fs::create_dir_all(dir.join("dist/web")).unwrap();
            std::fs::write(dir.join("dist/web/index.html"), "<html>made elsewhere</html>").unwrap();
            let (helper, _log) = fake_helper(&dir);
            let mut p = WebPreview::default();
            p.start(launch(&manifest, unlinked, Some(helper)), Intent::Show(Target::WebView));
            wait_until(&mut p, "the server", |p| p.serving());
            assert!(p.notes().contains(&Note::NotLinked));
            assert!(http_get(&p.url().unwrap(), "/").contains("made elsewhere"));
            p.close();
            assert!(!p.serving());
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn building_serving_showing_following_the_controls_and_rebuilding_on_a_change() {
        quiet();
        let manifest = project("flow");
        let dir = manifest.parent().unwrap().to_path_buf();
        let (helper, log) = fake_helper(&dir);
        let backend = Fake::new();
        let mut p = WebPreview::default();
        let t = Instant::now();
        assert!(p.start(launch(&manifest, backend.clone(), Some(helper)), Intent::Show(Target::WebView)));
        assert!(p.open, "the controls window opens with the Preview");
        wait_until(&mut p, "the Preview to be showing", |p| p.serving());

        let kinds: Vec<&str> = p
            .notes()
            .iter()
            .map(|n| match n {
                Note::Building => "building",
                Note::BuildDone { .. } => "done",
                Note::BuildWarning(_) => "warning",
                Note::Serving { .. } => "serving",
                Note::ModeWebView => "web-view",
                Note::BackendOff => "backend-off",
                _ => "other",
            })
            .collect();
        assert_eq!(kinds, ["building", "done", "warning", "serving", "web-view", "backend-off"], "{:?}", p.notes());
        assert_eq!(p.mode(), Some(Mode::WebView));
        let url = p.url().unwrap();
        let page = http_get(&url, "/");
        assert!(page.contains("build 1") && page.contains("/__prc/livereload.js"), "the page is served with live reload: {page}");
        assert_eq!(p.take_outbox().len(), 6);
        assert!(p.take_outbox().is_empty(), "each note reaches the Output panel once");

        // The controls reach the web view: a phone in landscape in the dark.
        p.controls.choose("phone");
        p.controls.landscape = true;
        p.controls.dark = true;
        p.apply_controls();
        p.reload();
        let deadline = Instant::now() + Duration::from_secs(30);
        let lines = loop {
            let text = std::fs::read_to_string(&log).unwrap_or_default();
            if text.lines().count() >= 5 || Instant::now() > deadline {
                break text;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let lines: Vec<&str> = lines.lines().collect();
        assert!(lines[0].contains(r#""cmd":"open""#) && lines[0].contains(r#""locale":"pt-BR""#) && lines[0].contains(r#""width":1280"#), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains(r#""cmd":"size","width":844,"height":390"#)), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains(r#""cmd":"scheme","dark":true"#)), "{lines:?}");
        assert_eq!(lines.iter().filter(|l| l.contains(r#""cmd":"emulate""#)).count(), 1, "one reload for ratio and insets: {lines:?}");
        assert!(lines.iter().any(|l| l.contains(r#""cmd":"reload""#)), "{lines:?}");

        // A change in a form: the bundle is built again and the page is told.
        let version_before = version_of(&url);
        std::fs::write(dir.join("webforms/main-form.cfrm"), "<Form edited='true'/>").unwrap();
        wait_until(&mut p, "the rebuild", |p| p.notes().iter().any(|n| matches!(n, Note::Updated { .. })));
        assert_eq!(backend.builds.load(Ordering::SeqCst), 2);
        assert!(http_get(&url, "/").contains("build 2"), "the new bundle is the one served");
        let version_after = version_of(&url);
        assert!(version_after > version_before, "the page's next poll sees a new version and reloads ({version_before} -> {version_after})");

        // A page that closes its window ends the Preview and says so; the IDE goes on.
        p.close();
        assert!(!p.active() && !p.open);
        let _ = std::fs::remove_dir_all(&dir);
        println!(
            "web preview: build -> serve -> web view in {} ms; a form edit rebuilt and republished; controls reached the helper as {} lines ({} emulate)",
            t.elapsed().as_millis(),
            lines.len(),
            lines.iter().filter(|l| l.contains("emulate")).count()
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_web_view_that_cannot_start_leaves_the_browser_not_nothing() {
        // The page is shown in the browser when the helper reports it has no web view.
        use std::os::unix::fs::PermissionsExt;
        quiet();
        let manifest = project("nohelper");
        let dir = manifest.parent().unwrap().to_path_buf();
        let helper = dir.join("no-web-view");
        std::fs::write(&helper, "#!/bin/sh\necho '{\"event\":\"failed\",\"message\":\"no web view here\"}'\necho '{\"event\":\"closed\"}'\nexit 3\n").unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut p = WebPreview::default();
        p.start(launch(&manifest, Fake::new(), Some(helper)), Intent::Show(Target::WebView));
        wait_until(&mut p, "the fall-back", |p| p.notes().contains(&Note::ModeBrowser));
        let n = p.notes();
        let failed = n.iter().position(|x| *x == Note::HelperFailed("no web view here".into())).expect("the helper's reason is said");
        let browser = n.iter().position(|x| *x == Note::ModeBrowser).unwrap();
        assert!(failed < browser, "the reason comes first: {n:?}");
        assert_eq!(p.mode(), Some(Mode::SystemBrowser));
        assert!(p.serving(), "the page is still served, for the browser");
        p.close();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── the window ──

    /// A Preview with every kind of thing to show: a long log, a blocked link.
    fn busy() -> WebPreview {
        let mut p = WebPreview::default();
        p.open = true;
        p.controls = Controls::initial(ProjectKind::Mobile, Some(DeviceClass::Phone), None, "pt-BR");
        p.locale_edit = "pt-BR".into();
        for i in 0..40 {
            p.say(Note::Console { level: "log".into(), text: format!("line {i} with a long text that wraps around the edge of the log, for sure, in any language") });
        }
        p.say(Note::BuildRefused(vec!["a.cfrm:3: EXEC RUST - move it to the Backend, which can run it".into(); 5]));
        p.say(Note::ModeBrowser);
        p.say(Note::Blocked("https://example.com/a/very/long/address/that/goes/on/and/on?with=query&and=more".into()));
        p.say(Note::BackendOff);
        p
    }

    /// A Preview that has said nothing yet.
    fn empty() -> WebPreview {
        let mut p = WebPreview::default();
        p.open = true;
        p
    }

    /// The window's rectangle after each of `frames` frames, and the height the log had in the last.
    fn window_rect(lang: Language, frames: usize, make: impl Fn() -> WebPreview) -> (Vec<egui::Rect>, f32) {
        let ctx = egui::Context::default();
        crate::app::apply_glass_visuals(&ctx, crate::theme::default_theme());
        let tr = lang.tr();
        let mut p = make();
        let mut rects = Vec::new();
        for _ in 0..frames {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0)));
            ctx.begin_pass(input);
            p.show(&ctx, &tr);
            let mut full = ctx.end_pass();
            full.textures_delta.clear();
            rects.push(ctx.memory(|m| m.area_rect(egui::Id::new("web_preview_controls"))).expect("the window registered an area"));
        }
        (rects, p.log_h)
    }

    #[test]
    fn the_controls_window_keeps_its_size() {
        let mut checked = 0;
        let mut sizes = std::collections::BTreeSet::new();
        let mut min_log = f32::MAX;
        for &lang in Language::ALL {
            for (state, make) in [("busy", busy as fn() -> WebPreview), ("empty", empty as fn() -> WebPreview)] {
                let (rects, log_h) = window_rect(lang, 12, make);
                assert!(log_h >= 100.0, "{lang:?} {state}: the log has room to be read ({log_h} points)");
                min_log = min_log.min(log_h);
                let first = rects[2];
                for (i, r) in rects.iter().enumerate().skip(2) {
                    assert_eq!(r.size(), first.size(), "{lang:?} {state}: frame {i} changed the window's size");
                    assert_eq!(r.min, first.min, "{lang:?} {state}: frame {i} moved the window");
                }
                assert!(
                    (first.width() - WINDOW_W).abs() < 2.0 && first.height() > BODY_H && first.height() < BODY_H + 120.0,
                    "{lang:?} {state}: {:?}",
                    first.size()
                );
                sizes.insert(((first.width() * 10.0) as i32, (first.height() * 10.0) as i32));
                checked += rects.len() - 2;
            }
        }
        assert_eq!(sizes.len(), 1, "the window is the same size in every language and state: {sizes:?}");
        let (w, h) = sizes.into_iter().next().unwrap();
        println!(
            "web preview: the controls window held {} x {} over {checked} frames (6 languages x 2 states, one size for all); the log kept at least {min_log:.0} points",
            w as f32 / 10.0,
            h as f32 / 10.0
        );
    }

    #[test]
    fn nothing_is_drawn_while_the_window_is_closed() {
        let ctx = egui::Context::default();
        let mut p = WebPreview::default();
        ctx.begin_pass(egui::RawInput::default());
        p.show(&ctx, &Language::English.tr());
        let mut full = ctx.end_pass();
        full.textures_delta.clear();
        assert!(ctx.memory(|m| m.area_rect(egui::Id::new("web_preview_controls"))).is_none());
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
}
