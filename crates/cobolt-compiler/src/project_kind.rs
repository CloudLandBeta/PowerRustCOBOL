// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The kind of a project, and the settings that belong to it** (WASM-001).
//!
//! A project is a desktop application (today's behaviour), a Web application
//! (WebAssembly in a browser) or a Mobile application (the same frontend in a
//! Tauri shell, Android or iOS). The Backend is **not** a kind: it is the
//! `backend/` part of every Web or Mobile project (operator, 2026-10-20).
//!
//! These are the *manifest's* types. The IDE's `CoboltProject`, the compiler's
//! manifest reader, `rcrun` and the coding-agent tools all read the same
//! fields, so the type lives here, below all of them — the way an External
//! Crate pin does. Every field has a serde default and a Desktop project writes
//! none of it: an old manifest loads as Desktop and is not modified by loading,
//! and a new Desktop manifest is byte-for-byte what it was before kinds existed.

use serde::{Deserialize, Serialize};

/// What a project builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectKind {
    /// A native desktop application — every project made before kinds existed.
    #[default]
    Desktop,
    /// A WebAssembly application for the browser, with a Backend part.
    Web,
    /// An Android or iOS application (a Tauri shell around the web frontend),
    /// with a Backend part.
    Mobile,
}

impl ProjectKind {
    /// Every kind, in the order the New Project dialog lists them.
    pub const ALL: [ProjectKind; 3] = [ProjectKind::Desktop, ProjectKind::Web, ProjectKind::Mobile];

    /// The name the manifest and the tools use.
    pub fn as_str(self) -> &'static str {
        match self {
            ProjectKind::Desktop => "desktop",
            ProjectKind::Web => "web",
            ProjectKind::Mobile => "mobile",
        }
    }

    /// The kind named `s` (case-blind), if there is one.
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str().eq_ignore_ascii_case(s.trim()))
    }

    /// `serde`'s `skip_serializing_if`: a Desktop project writes no kind.
    pub fn is_desktop(&self) -> bool {
        *self == ProjectKind::Desktop
    }

    /// Does the kind carry a Backend part (WASM-001 R6, D13)?
    pub fn has_backend(self) -> bool {
        !self.is_desktop()
    }

    /// The folder the project's forms live in (WASM-001 Q8).
    pub fn forms_folder(self) -> &'static str {
        match self {
            ProjectKind::Desktop => "forms",
            ProjectKind::Web => "webforms",
            ProjectKind::Mobile => "mobileforms",
        }
    }

    /// The extension of the kind's form files, without the dot (WASM-001 Q2):
    /// one form model, three names, so the kind stays visible on disk.
    pub fn form_extension(self) -> &'static str {
        match self {
            ProjectKind::Desktop => "cfrm",
            ProjectKind::Web => "wcfrm",
            ProjectKind::Mobile => "mcfrm",
        }
    }

    /// The kind a form file's extension names, if it names one.
    pub fn of_form_extension(ext: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.form_extension().eq_ignore_ascii_case(ext))
    }
}

/// Every form file extension, Desktop's first (WASM-001 Q2).
pub const FORM_EXTENSIONS: [&str; 3] = ["cfrm", "wcfrm", "mcfrm"];

/// Is `ext` (without the dot, any case) the extension of a form file of any kind?
/// The one test every reader uses instead of comparing with `"cfrm"` (plan D-T15a).
pub fn is_form_extension(ext: &str) -> bool {
    FORM_EXTENSIONS.iter().any(|e| e.eq_ignore_ascii_case(ext))
}

/// Is `path` a form file of any kind, judged by its extension?
pub fn is_form_path(path: &std::path::Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(is_form_extension)
}

/// The folder of the Backend part of a Web or Mobile project (WASM-001 Q8).
pub const BACKEND_FOLDER: &str = "backend";

/// A Mobile project's operating system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MobilePlatform {
    Android,
    Ios,
}

impl MobilePlatform {
    pub fn as_str(self) -> &'static str {
        match self {
            MobilePlatform::Android => "android",
            MobilePlatform::Ios => "ios",
        }
    }
}

/// The class of device a Mobile project is laid out for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceClass {
    Phone,
    Tablet,
}

impl DeviceClass {
    pub fn as_str(self) -> &'static str {
        match self {
            DeviceClass::Phone => "phone",
            DeviceClass::Tablet => "tablet",
        }
    }
}

/// The orientation a Mobile application opens in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    /// Follows the device.
    #[default]
    Auto,
    Portrait,
    Landscape,
}

impl Orientation {
    pub fn as_str(self) -> &'static str {
        match self {
            Orientation::Auto => "auto",
            Orientation::Portrait => "portrait",
            Orientation::Landscape => "landscape",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [Orientation::Auto, Orientation::Portrait, Orientation::Landscape]
            .into_iter()
            .find(|o| o.as_str() == s.trim().to_ascii_lowercase())
    }
}

/// How a Web or Mobile application's forms open each other (WASM-011 R1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApplicationStyle {
    /// One page; forms load into the application shell. The default.
    #[default]
    Spa,
    /// Each form is a page (or screen) of its own, as each is a window on the
    /// desktop.
    Standalone,
}

impl ApplicationStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            ApplicationStyle::Spa => "spa",
            ApplicationStyle::Standalone => "standalone",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [ApplicationStyle::Spa, ApplicationStyle::Standalone]
            .into_iter()
            .find(|o| o.as_str() == s.trim().to_ascii_lowercase())
    }
}

/// The cross-origin isolation a Web bundle is served with (WASM-002 R20).
/// Threads need the page to be isolated; nothing needs threads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Isolation {
    /// Serve no isolation headers: the plain player and the main thread.
    Off,
    /// `Cross-Origin-Opener-Policy: same-origin` and
    /// `Cross-Origin-Embedder-Policy: require-corp` — every browser.
    #[default]
    RequireCorp,
    /// The same, with `Cross-Origin-Embedder-Policy: credentialless` — lets
    /// cross-origin resources without credentials load; Safari lacks it.
    Credentialless,
}

impl Isolation {
    pub fn as_str(self) -> &'static str {
        match self {
            Isolation::Off => "off",
            Isolation::RequireCorp => "require-corp",
            Isolation::Credentialless => "credentialless",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [Isolation::Off, Isolation::RequireCorp, Isolation::Credentialless]
            .into_iter()
            .find(|o| o.as_str() == s.trim().to_ascii_lowercase())
    }

    /// The two response headers that make a page cross-origin isolated, or
    /// none for [`Isolation::Off`].
    pub fn headers(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Isolation::Off => &[],
            Isolation::RequireCorp => &[
                ("Cross-Origin-Opener-Policy", "same-origin"),
                ("Cross-Origin-Embedder-Policy", "require-corp"),
            ],
            Isolation::Credentialless => &[
                ("Cross-Origin-Opener-Policy", "same-origin"),
                ("Cross-Origin-Embedder-Policy", "credentialless"),
            ],
        }
    }
}

fn yes() -> bool {
    true
}

/// The browsers a Web build promises to run in (WASM-002 R18): the loader
/// probes the browser it finds against this matrix and the WebAssembly feature
/// baseline. All on by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserMatrix {
    /// Chrome and Edge (Chromium).
    #[serde(default = "yes")]
    pub chromium: bool,
    #[serde(default = "yes")]
    pub firefox: bool,
    /// Safari on macOS.
    #[serde(default = "yes")]
    pub safari: bool,
    /// The iOS web view (WKWebView), which every iOS browser and the Tauri iOS
    /// shell use.
    #[serde(default = "yes")]
    pub ios_webview: bool,
    /// Android's system WebView.
    #[serde(default = "yes")]
    pub android_webview: bool,
}

impl Default for BrowserMatrix {
    fn default() -> Self {
        Self { chromium: true, firefox: true, safari: true, ios_webview: true, android_webview: true }
    }
}

impl BrowserMatrix {
    /// Is a WebKit engine in the matrix (Safari or the iOS web view)? Those
    /// are the engines that need the single-memory program module
    /// (WASM-017 R9).
    pub fn includes_webkit(&self) -> bool {
        self.safari || self.ios_webview
    }
}

/// Which program-module variants a build emits (WASM-017 R9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleVariants {
    /// The multi-memory module (the transport arena is memory 1).
    pub multi_memory: bool,
    /// The single-memory module (the arena is a region of memory 0).
    pub single_memory: bool,
}

impl ModuleVariants {
    /// How many program modules the bundle carries.
    pub fn count(self) -> usize {
        usize::from(self.multi_memory) + usize::from(self.single_memory)
    }
}

/// `[web]` — a Web project's settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebSettings {
    /// SPA (default) or standalone forms.
    #[serde(default)]
    pub style: ApplicationStyle,
    /// The browsers the build promises to run in.
    #[serde(default)]
    pub browsers: BrowserMatrix,
    /// Emit the multi-memory program module (the default); off emits only the
    /// single-memory one.
    #[serde(default = "yes")]
    pub multi_memory: bool,
    /// The isolation headers the bundle is served with.
    #[serde(default)]
    pub isolation: Isolation,
    /// Where the application's Backend listens in production: `wss://…`, or a
    /// loopback `ws://…` in development. Empty until the developer sets it.
    #[serde(default)]
    pub backend_url: String,
    /// The player the Build copies (WASM-016 R11, Q6): absent = the one this
    /// IDE ships; a version (`"2.0.0"`) pins an older player the IDE keeps.
    /// Written only when set, so an existing manifest is saved unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player: Option<String>,
}

impl Default for WebSettings {
    fn default() -> Self {
        Self {
            style: ApplicationStyle::default(),
            browsers: BrowserMatrix::default(),
            multi_memory: true,
            isolation: Isolation::default(),
            backend_url: String::new(),
            player: None,
        }
    }
}

impl WebSettings {
    /// The program-module variants this Web build emits (WASM-017 R9): the
    /// multi-memory module when asked for, **and** the single-memory one
    /// whenever multi-memory is off or the matrix includes Safari or the iOS
    /// web view — the loader then probes which to use.
    pub fn module_variants(&self) -> ModuleVariants {
        ModuleVariants {
            multi_memory: self.multi_memory,
            single_memory: !self.multi_memory || self.browsers.includes_webkit(),
        }
    }
}

/// `[mobile]` — a Mobile project's settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileSettings {
    pub platform: MobilePlatform,
    pub device: DeviceClass,
    #[serde(default)]
    pub orientation: Orientation,
    /// The oldest operating system the application runs on: `auto` (the
    /// newest the toolchain supports), an Android API level (`26`) or an iOS
    /// version (`16.0`).
    #[serde(default = "auto")]
    pub min_os: String,
    #[serde(default)]
    pub style: ApplicationStyle,
    /// Emit the multi-memory program module. Off by default for iOS, whose web
    /// view needs the single-memory one (WASM-017 R9). [`MobileSettings::new`]
    /// sets it per platform and the manifest always carries it.
    #[serde(default = "yes")]
    pub multi_memory: bool,
    #[serde(default)]
    pub backend_url: String,
    /// The player the Build copies, as [`WebSettings::player`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player: Option<String>,
}

fn auto() -> String {
    "auto".to_owned()
}

impl MobileSettings {
    /// The defaults for a platform and device: iOS starts with multi-memory
    /// off, a phone opens in portrait, a tablet follows the device.
    pub fn new(platform: MobilePlatform, device: DeviceClass) -> Self {
        Self {
            platform,
            device,
            orientation: match device {
                DeviceClass::Phone => Orientation::Portrait,
                DeviceClass::Tablet => Orientation::Auto,
            },
            min_os: auto(),
            style: ApplicationStyle::default(),
            multi_memory: platform == MobilePlatform::Android,
            backend_url: String::new(),
            player: None,
        }
    }

    /// The program-module variants a Mobile build emits: an iOS build always
    /// carries the single-memory module, an Android build the multi-memory
    /// one unless turned off (and then the single-memory one instead).
    pub fn module_variants(&self) -> ModuleVariants {
        match self.platform {
            MobilePlatform::Ios => ModuleVariants { multi_memory: self.multi_memory, single_memory: true },
            MobilePlatform::Android => ModuleVariants { multi_memory: self.multi_memory, single_memory: !self.multi_memory },
        }
    }
}

/// `[backend]` — the Backend part of a Web or Mobile project (WASM-003 R1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendSettings {
    /// Retired as the list of exported programs (WASM-003 D5): that is
    /// `backend/exports.toml`, which carries roles and `public` as well. Still
    /// read, so an older manifest loads and the IDE can migrate it once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exports: Vec<String>,
    /// `per-call` (the default: fresh `WORKING-STORAGE` at every call) or
    /// `per-connection` (WASM-003 Q2). Written only when it is not the default,
    /// so an existing manifest is saved unchanged.
    #[serde(default = "per_call", skip_serializing_if = "is_per_call")]
    pub state: String,
}

impl Default for BackendSettings {
    fn default() -> Self {
        BackendSettings { exports: Vec::new(), state: per_call() }
    }
}

impl BackendSettings {
    /// `state` as the server reads it; `Err` names the two values.
    pub fn state_mode(&self) -> Result<cobolt_server::backend::StateMode, String> {
        cobolt_server::backend::StateMode::parse(&self.state)
    }
}

fn per_call() -> String {
    cobolt_server::backend::StateMode::PER_CALL.to_owned()
}

fn is_per_call(state: &String) -> bool {
    state.trim().is_empty() || state == cobolt_server::backend::StateMode::PER_CALL
}

/// `[desktop]` — the options of the Desktop templates the New Project dialog
/// offers (window style, UI framework, High DPI, GPU acceleration). Written
/// only when a choice differs from the default, so a default Desktop project's
/// manifest is unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopSettings {
    #[serde(default = "tabbed")]
    pub window_style: String,
    #[serde(default = "power_ui")]
    pub ui_framework: String,
    #[serde(default = "yes")]
    pub high_dpi: bool,
    #[serde(default = "yes")]
    pub gpu_acceleration: bool,
}

fn tabbed() -> String {
    "tabbed".to_owned()
}

fn power_ui() -> String {
    "powerui".to_owned()
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self { window_style: tabbed(), ui_framework: power_ui(), high_dpi: true, gpu_acceleration: true }
    }
}

/// Is a `ws://` or `wss://` address one a Backend may be reached at
/// (WASM-003 R5)? `wss` always; plain `ws` only for a loopback address.
/// Empty is accepted — the address is not set yet.
pub fn backend_url_problem(url: &str) -> Option<BackendUrlProblem> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }
    let lower = url.to_ascii_lowercase();
    let (secure, rest) = if let Some(rest) = lower.strip_prefix("wss://") {
        (true, rest)
    } else if let Some(rest) = lower.strip_prefix("ws://") {
        (false, rest)
    } else {
        return Some(BackendUrlProblem::Scheme);
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = match authority.rsplit_once('@') {
        Some((_, h)) => h,
        None => authority,
    };
    let host = if let Some(stripped) = host.strip_prefix('[') {
        stripped.split(']').next().unwrap_or("")
    } else {
        host.rsplit_once(':').map(|(h, p)| if p.chars().all(|c| c.is_ascii_digit()) { h } else { host }).unwrap_or(host)
    };
    if host.is_empty() {
        return Some(BackendUrlProblem::Host);
    }
    if !secure && !(host == "localhost" || host == "127.0.0.1" || host == "::1" || host.ends_with(".localhost")) {
        return Some(BackendUrlProblem::PlainNotLoopback);
    }
    None
}

/// Why a Backend address is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendUrlProblem {
    /// Not `ws://` or `wss://`.
    Scheme,
    /// No host after the scheme.
    Host,
    /// Plain `ws://` to an address that is not loopback (WASM-003 R5).
    PlainNotLoopback,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_has_its_own_form_extension_and_only_those_are_forms() {
        for k in ProjectKind::ALL {
            println!("{:<8} forms are .{}", k.as_str(), k.form_extension());
            assert_eq!(ProjectKind::of_form_extension(k.form_extension()), Some(k));
            assert!(FORM_EXTENSIONS.contains(&k.form_extension()));
        }
        assert_eq!(ProjectKind::Desktop.form_extension(), "cfrm");
        assert_eq!(ProjectKind::Web.form_extension(), "wcfrm");
        assert_eq!(ProjectKind::Mobile.form_extension(), "mcfrm");
        for yes in ["cfrm", "wcfrm", "mcfrm", "WCFRM", "Cfrm"] {
            assert!(is_form_extension(yes), "{yes} is a form extension");
        }
        for no in ["cfrmx", "frm", "", "cbl", ".cfrm"] {
            assert!(!is_form_extension(no), "{no:?} is not a form extension");
        }
        assert!(is_form_path(std::path::Path::new("webforms/main-form.wcfrm")));
        assert!(!is_form_path(std::path::Path::new("webforms/main-form")));
    }

    #[test]
    fn a_kind_round_trips_and_parses() {
        for k in ProjectKind::ALL {
            assert_eq!(ProjectKind::parse(k.as_str()), Some(k));
            assert_eq!(ProjectKind::parse(&k.as_str().to_uppercase()), Some(k));
        }
        assert_eq!(ProjectKind::parse("backend"), None, "the Backend is not a kind (D13)");
        assert_eq!(ProjectKind::default(), ProjectKind::Desktop);
        assert!(ProjectKind::Desktop.is_desktop() && !ProjectKind::Web.is_desktop());
        assert!(!ProjectKind::Desktop.has_backend() && ProjectKind::Web.has_backend() && ProjectKind::Mobile.has_backend());
    }

    #[test]
    fn the_forms_folders_are_the_ones_of_wasm_001_q8() {
        assert_eq!(ProjectKind::Desktop.forms_folder(), "forms");
        assert_eq!(ProjectKind::Web.forms_folder(), "webforms");
        assert_eq!(ProjectKind::Mobile.forms_folder(), "mobileforms");
        assert_eq!(BACKEND_FOLDER, "backend");
    }

    #[test]
    fn every_web_setting_has_a_default_so_an_old_table_loads() {
        let s: WebSettings = toml::from_str("").unwrap();
        assert_eq!(s, WebSettings::default());
        assert!(s.multi_memory && s.browsers.chromium && s.browsers.safari);
        assert_eq!(s.isolation, Isolation::RequireCorp);
        assert_eq!(s.style, ApplicationStyle::Spa);
    }

    /// WASM-016 R11: a project may pin an older player; a manifest without the
    /// key reads as "the current one" and is written back without it.
    #[test]
    fn the_player_pin_is_optional_and_only_written_when_set() {
        let old: WebSettings = toml::from_str("backend_url = \"wss://api.example.com/ws\"").unwrap();
        assert_eq!(old.player, None);
        assert!(!toml::to_string_pretty(&old).unwrap().contains("player"), "an old manifest is saved unchanged");
        let pinned: WebSettings = toml::from_str("player = \"2.0.0\"").unwrap();
        assert_eq!(pinned.player.as_deref(), Some("2.0.0"));
        assert!(toml::to_string_pretty(&pinned).unwrap().contains("player = \"2.0.0\""));
        let mobile: MobileSettings = toml::from_str("platform = \"android\"\ndevice = \"phone\"\nplayer = \"2.0.0\"").unwrap();
        assert_eq!(mobile.player.as_deref(), Some("2.0.0"));
        let unpinned: MobileSettings = toml::from_str("platform = \"ios\"\ndevice = \"tablet\"").unwrap();
        assert_eq!(unpinned.player, None);
    }

    #[test]
    fn web_settings_round_trip_through_toml() {
        let mut s = WebSettings::default();
        s.style = ApplicationStyle::Standalone;
        s.multi_memory = false;
        s.isolation = Isolation::Credentialless;
        s.browsers.firefox = false;
        s.backend_url = "wss://api.example.com/ws".into();
        let text = toml::to_string_pretty(&s).unwrap();
        assert!(text.contains("isolation = \"credentialless\""), "{text}");
        assert_eq!(toml::from_str::<WebSettings>(&text).unwrap(), s);
    }

    /// WASM-017 R9: the single-memory module is carried whenever the matrix
    /// includes Safari or the iOS web view — which the default matrix does — and
    /// always when multi-memory is off.
    #[test]
    fn the_module_variants_follow_the_matrix_and_the_option() {
        let default = WebSettings::default().module_variants();
        assert_eq!((default.multi_memory, default.single_memory), (true, true), "the default matrix includes Safari");
        assert_eq!(default.count(), 2);

        let mut chromium_only = WebSettings::default();
        chromium_only.browsers = BrowserMatrix { chromium: true, firefox: true, safari: false, ios_webview: false, android_webview: true };
        let v = chromium_only.module_variants();
        assert_eq!((v.multi_memory, v.single_memory), (true, false), "no WebKit engine: multi-memory only");

        chromium_only.multi_memory = false;
        let v = chromium_only.module_variants();
        assert_eq!((v.multi_memory, v.single_memory), (false, true), "multi-memory off: single only");
    }

    /// WASM-017 R9: an iOS build starts with multi-memory off and always carries the single-memory module.
    #[test]
    fn ios_defaults_multi_memory_off_and_always_carries_the_single_memory_module() {
        let ios = MobileSettings::new(MobilePlatform::Ios, DeviceClass::Phone);
        assert!(!ios.multi_memory, "iOS default is off");
        let v = ios.module_variants();
        assert!(v.single_memory && !v.multi_memory);
        let mut forced = ios.clone();
        forced.multi_memory = true;
        let v = forced.module_variants();
        assert!(v.single_memory && v.multi_memory, "iOS always carries the single-memory module, even with multi-memory on");

        let android = MobileSettings::new(MobilePlatform::Android, DeviceClass::Tablet);
        assert!(android.multi_memory);
        let v = android.module_variants();
        assert!(v.multi_memory && !v.single_memory);
    }

    #[test]
    fn a_phone_opens_in_portrait_and_a_tablet_follows_the_device() {
        assert_eq!(MobileSettings::new(MobilePlatform::Android, DeviceClass::Phone).orientation, Orientation::Portrait);
        assert_eq!(MobileSettings::new(MobilePlatform::Ios, DeviceClass::Tablet).orientation, Orientation::Auto);
    }

    #[test]
    fn mobile_settings_round_trip_and_min_os_defaults_to_auto() {
        let s = MobileSettings::new(MobilePlatform::Ios, DeviceClass::Tablet);
        let text = toml::to_string_pretty(&s).unwrap();
        assert!(text.contains("platform = \"ios\"") && text.contains("device = \"tablet\""), "{text}");
        assert_eq!(toml::from_str::<MobileSettings>(&text).unwrap(), s);
        let minimal: MobileSettings = toml::from_str("platform = \"android\"\ndevice = \"phone\"\nmulti_memory = true").unwrap();
        assert_eq!(minimal.min_os, "auto");
    }

    #[test]
    fn isolation_headers_are_the_pair_that_isolates_a_page() {
        assert!(Isolation::Off.headers().is_empty());
        for iso in [Isolation::RequireCorp, Isolation::Credentialless] {
            let h = iso.headers();
            assert_eq!(h[0], ("Cross-Origin-Opener-Policy", "same-origin"));
            assert_eq!(h[1].0, "Cross-Origin-Embedder-Policy");
        }
        assert_eq!(Isolation::RequireCorp.headers()[1].1, "require-corp");
        assert_eq!(Isolation::Credentialless.headers()[1].1, "credentialless");
        for iso in [Isolation::Off, Isolation::RequireCorp, Isolation::Credentialless] {
            assert_eq!(Isolation::parse(iso.as_str()), Some(iso));
        }
    }

    /// WASM-003 R5: `wss` always; plain `ws` only to a loopback address.
    #[test]
    fn a_backend_address_is_wss_or_loopback_ws() {
        for ok in [
            "",
            "wss://api.example.com/ws",
            "WSS://api.example.com:8443",
            "ws://127.0.0.1:8750",
            "ws://localhost/ws",
            "ws://[::1]:9000/x",
            "ws://app.localhost:80",
        ] {
            assert_eq!(backend_url_problem(ok), None, "{ok:?} should be accepted");
        }
        assert_eq!(backend_url_problem("http://api.example.com"), Some(BackendUrlProblem::Scheme));
        assert_eq!(backend_url_problem("api.example.com"), Some(BackendUrlProblem::Scheme));
        assert_eq!(backend_url_problem("wss://"), Some(BackendUrlProblem::Host));
        assert_eq!(backend_url_problem("ws://api.example.com"), Some(BackendUrlProblem::PlainNotLoopback));
        assert_eq!(backend_url_problem("ws://192.168.1.10:8750"), Some(BackendUrlProblem::PlainNotLoopback));
        assert_eq!(
            backend_url_problem("ws://localhost.evil.com"),
            Some(BackendUrlProblem::PlainNotLoopback),
            "a name that merely starts with localhost is not loopback"
        );
        assert_eq!(
            backend_url_problem("ws://user@127.0.0.1.evil.com:80"),
            Some(BackendUrlProblem::PlainNotLoopback),
            "userinfo does not make a host loopback"
        );
    }

    /// WASM-003 T3 (Q2): `[backend] state` — absent is `per-call`, `per-connection`
    /// survives a save, the default is not written, any other word is refused
    /// naming the two values; the old `exports` list still loads.
    #[test]
    fn backend_state_round_trips_and_refuses_an_unknown_word() {
        #[derive(Serialize, Deserialize)]
        struct T {
            backend: BackendSettings,
        }
        let absent: T = toml::from_str("[backend]\nexports = [\"HEALTH\"]\n").unwrap();
        assert_eq!(absent.backend.state, "per-call");
        assert_eq!(absent.backend.exports, ["HEALTH"], "an older manifest still loads");
        assert_eq!(absent.backend.state_mode(), Ok(cobolt_server::backend::StateMode::PerCall));
        let saved = toml::to_string(&absent).unwrap();
        assert!(!saved.contains("state"), "the default is not written: {saved}");

        let kept: T = toml::from_str("[backend]\nstate = \"per-connection\"\n").unwrap();
        let saved = toml::to_string(&kept).unwrap();
        let again: T = toml::from_str(&saved).unwrap();
        assert_eq!(again.backend.state_mode(), Ok(cobolt_server::backend::StateMode::PerConnection), "{saved}");

        let wrong: T = toml::from_str("[backend]\nstate = \"session\"\n").unwrap();
        let e = wrong.backend.state_mode().unwrap_err();
        assert!(e.contains("\"session\"") && e.contains("per-call") && e.contains("per-connection"), "{e}");
        println!("absent → per-call (not written back); per-connection → {:?}; \"session\" → {e}", saved.trim());
    }
}
