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
use std::collections::BTreeMap;

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

/// What a control marked `LoadingEffect` shows while a call to the Backend is
/// in flight and no data has arrived yet (2.0.413). The per-control property
/// stays the switch for WHICH controls show it; this project setting chooses
/// WHAT they show. More effects can be added here later: the player and the
/// IDE's combobox read the list from this one enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LoadingEffect {
    /// Nothing: marked controls stay as they are while a call is in flight.
    None,
    /// Pale diagonal stripes that move from left to right over the control,
    /// its content blurred behind them. The default (a project that never
    /// chose keeps the effect its marked controls always meant).
    #[default]
    MovingStripes,
}

impl LoadingEffect {
    /// Every effect, in the order the combobox lists them.
    pub const ALL: [LoadingEffect; 2] = [LoadingEffect::None, LoadingEffect::MovingStripes];

    pub fn as_str(self) -> &'static str {
        match self {
            LoadingEffect::None => "none",
            LoadingEffect::MovingStripes => "moving-stripes",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|o| o.as_str() == s.trim().to_ascii_lowercase())
    }

    /// `serde` writes the key only when it is not the default, so an existing
    /// manifest is saved unchanged.
    pub fn is_default(&self) -> bool {
        *self == LoadingEffect::default()
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

/// The payload limit a project has unless it sets one (WASM-017 R8, D3):
/// 384 KiB, the player's frame limit before the setting existed, so an older
/// project behaves as it did.
pub const DEFAULT_MAX_PAYLOAD_BYTES: u32 = 384 * 1024;

fn default_max_payload() -> u32 {
    DEFAULT_MAX_PAYLOAD_BYTES
}

fn is_default_max_payload(n: &u32) -> bool {
    *n == DEFAULT_MAX_PAYLOAD_BYTES
}

/// How long a `CALL … WITH WEBSOCKET LINKAGE` waits for its answer unless the
/// project says otherwise (WASM-005 R11): 60 s, the backend client's wait
/// before the setting existed, so an older project behaves as it did.
pub const DEFAULT_CALL_TIMEOUT_MS: u32 = 60_000;

fn default_call_timeout() -> u32 {
    DEFAULT_CALL_TIMEOUT_MS
}

fn is_default_call_timeout(n: &u32) -> bool {
    *n == DEFAULT_CALL_TIMEOUT_MS
}

fn is_true(b: &bool) -> bool {
    *b
}

/// The project stylesheet's file name in the forms folder (WASM-007 Q3, Q3a).
pub const APPLICATION_STYLESHEET: &str = "application.css";

fn web_stylesheet() -> String {
    format!("{}/{APPLICATION_STYLESHEET}", ProjectKind::Web.forms_folder())
}

fn is_web_stylesheet(s: &String) -> bool {
    *s == web_stylesheet()
}

fn mobile_stylesheet() -> String {
    format!("{}/{APPLICATION_STYLESHEET}", ProjectKind::Mobile.forms_folder())
}

fn is_mobile_stylesheet(s: &String) -> bool {
    *s == mobile_stylesheet()
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

    /// The WebKit engines in the matrix, by name.
    pub fn webkit_browsers(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.safari {
            v.push("Safari");
        }
        if self.ios_webview {
            v.push("iOS web view");
        }
        v
    }
}

/// Why a Web project's variants cannot be built (WASM-017 R9, AC7): "multi-
/// memory only" was asked for while the matrix has WebKit engines, which run
/// only the single-memory module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantProblem {
    /// The WebKit browsers of the matrix (`Safari`, `iOS web view`).
    pub browsers: Vec<&'static str>,
}

impl std::fmt::Display for VariantProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the single-memory fallback is off, but the target browsers include {}, which run only the single-memory module; turn the fallback on or remove them from the browsers",
            self.browsers.join(", ")
        )
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

/// `[web.unsafe]` / `[mobile.unsafe]` — settings that weaken a protection (WASM-013 R11). Every one is off by default,
/// a warning in Development and a refusal in a Release build; the table is written only when one is on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsafeSettings {
    /// Serve and talk plain `ws://` beyond loopback.
    #[serde(default)]
    pub plain_ws: bool,
    /// Skip the secret scan of the bundle.
    #[serde(default)]
    pub skip_secret_scan: bool,
    /// Send the Content-Security-Policy as report-only: the browser reports a violation and blocks nothing.
    #[serde(default)]
    pub csp_report_only: bool,
}

impl UnsafeSettings {
    /// The TOML key path of every setting that is on, for `table` (`web` or `mobile`).
    pub fn enabled(&self, table: &str) -> Vec<String> {
        let mut out = Vec::new();
        for (on, key) in [(self.plain_ws, "plain_ws"), (self.skip_secret_scan, "skip_secret_scan"), (self.csp_report_only, "csp_report_only")] {
            if on {
                out.push(format!("{table}.unsafe.{key}"));
            }
        }
        out
    }

    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// `[mobile.pinning]` — the certificate pins of a Mobile build's web view (WASM-013 R14): a list of SPKI SHA-256 pins
/// (`sha256/<base64>`) and the date they expire.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinningSettings {
    #[serde(default)]
    pub pins: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
}

/// How many days before `[mobile.pinning] expires` Check starts to say so.
pub const PIN_WARN_DAYS: i64 = 30;

/// Whether `s` is a pin: `sha256/` and the standard base64 (with its `=`) of exactly 32 bytes, the SHA-256 of a certificate's public key.
pub fn is_pin(s: &str) -> bool {
    let Some(b) = s.strip_prefix("sha256/") else { return false };
    let b = b.as_bytes();
    // 32 bytes are 256 bits: 43 characters of 6 bits and one `=`; the 43rd carries 4 bits of the key and two that must be zero.
    if b.len() != 44 || b[43] != b'=' {
        return false;
    }
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    b[..43].iter().all(|c| value(*c).is_some()) && value(b[42]).is_some_and(|v| v & 0b11 == 0)
}

impl PinningSettings {
    /// What is wrong with this table, as `(warnings, errors)` (WASM-013 R14). A pin that cannot be checked, too few pins and an
    /// expiry that is missing, unreadable or past are errors: a pinned app that cannot reach its backend is the failure this
    /// exists to avoid, and a single pin leaves no way out of a key change. Within [`PIN_WARN_DAYS`] of the expiry it warns, in
    /// the form `pin-expiring <date> <days>` (the IDE says it in its own language). Pins are named by position, never quoted.
    /// Pinning is off, and this is not called, when the table is absent.
    pub fn check(&self, now_unix: i64) -> (Vec<String>, Vec<String>) {
        let mut warnings = Vec::new();
        let mut errors = Vec::new();
        let mut good = std::collections::BTreeSet::new();
        for (i, pin) in self.pins.iter().enumerate() {
            if is_pin(pin) {
                if !good.insert(pin.as_str()) {
                    errors.push(format!("[mobile.pinning] pins: pin {} repeats an earlier one", i + 1));
                }
            } else {
                errors.push(format!("[mobile.pinning] pins: pin {} is not sha256/ and the base64 of 32 bytes", i + 1));
            }
        }
        if good.len() < 2 {
            errors.push(format!("[mobile.pinning] pins: at least two different pins are required (the live key and a backup), found {}", good.len()));
        }
        match self.expires.as_deref().map(str::trim).filter(|e| !e.is_empty()) {
            None => errors.push("[mobile.pinning] expires: required (a date, YYYY-MM-DD)".into()),
            Some(text) => match cobolt_wire::security_txt::expiry_unix(text) {
                None => errors.push("[mobile.pinning] expires: must be a date, YYYY-MM-DD (or YYYY-MM-DDTHH:MM:SSZ)".into()),
                Some(at) if at <= now_unix => errors.push("[mobile.pinning] expires: the date has passed; pin the current keys and set a later date".into()),
                Some(at) => {
                    let days = (at - now_unix) / 86_400;
                    if days < PIN_WARN_DAYS {
                        warnings.push(format!("pin-expiring {} {}", text.get(..10).unwrap_or(text), days));
                    }
                }
            },
        }
        (warnings, errors)
    }
}

/// `[security]` — what the generated `/.well-known/security.txt` says (WASM-013 Q4, RFC 9116).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecuritySettings {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub contact: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub policy: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub expires: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub preferred_languages: String,
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
    /// The largest text or JSON frame that crosses the boundary, in bytes
    /// (WASM-017 R8, D3): the program module's transport-arena frame, the
    /// player's frame limit and what the Backend is told to accept. Written
    /// only when it is not the default.
    #[serde(default = "default_max_payload", skip_serializing_if = "is_default_max_payload")]
    pub max_payload_bytes: u32,
    /// How long a remote call waits for the Backend's answer, in milliseconds
    /// (WASM-005 R11); the client gives up with a transport timeout after it.
    /// Written only when it is not the default.
    #[serde(default = "default_call_timeout", skip_serializing_if = "is_default_call_timeout")]
    pub call_timeout_ms: u32,
    /// What a control marked `LoadingEffect` shows while a call to the Backend
    /// is in flight (2.0.413): `none` or `moving-stripes`. Written only when
    /// it is not the default (`moving-stripes`), so an existing manifest is
    /// saved unchanged and its marked controls keep their stripes.
    #[serde(default, skip_serializing_if = "LoadingEffect::is_default")]
    pub loading_effect: LoadingEffect,
    /// Carry the single-memory module beside the multi-memory one (WASM-017
    /// R9, D5). On by default; off asks for "multi-memory only", which the
    /// Build refuses while the matrix has a WebKit engine
    /// ([`WebSettings::variant_problem`]). Written only when off.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub single_memory_fallback: bool,
    /// The project stylesheet (WASM-007 Q3), relative to the project folder, in
    /// `@layer pr-app` under every form's own sheet. Written only when it is not
    /// the default, `webforms/application.css`.
    #[serde(default = "web_stylesheet", skip_serializing_if = "is_web_stylesheet")]
    pub stylesheet: String,
    /// A whole-theme stylesheet that replaces the built-in web theme (WASM-007
    /// Q5): the player adopts it as `pr-theme` and loads no built-in one.
    /// Written only when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_file: Option<String>,
    /// The browser Preview and Run open the page in (WASM-008 R12, Q7):
    /// empty = the system's default browser; a known id (`chrome`,
    /// `chromium`, `edge`, `firefox`, `safari`, `brave`); anything else = an
    /// application path the developer confirms once on this machine. Written
    /// only when set.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub preview_browser: String,
    /// The form a signed-out user is sent to when they open a form that requires sign-in
    /// (WASM-009 R11, D11). `None`: the first form holding an Authentication control.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign_in_form: Option<String>,
    /// `[web.unsafe]` (WASM-013 R11). Written only when a setting is on.
    #[serde(default, rename = "unsafe", skip_serializing_if = "UnsafeSettings::is_default")]
    pub unsafe_: UnsafeSettings,
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
            max_payload_bytes: DEFAULT_MAX_PAYLOAD_BYTES,
            call_timeout_ms: DEFAULT_CALL_TIMEOUT_MS,
            loading_effect: LoadingEffect::default(),
            single_memory_fallback: true,
            stylesheet: web_stylesheet(),
            theme_file: None,
            preview_browser: String::new(),
            sign_in_form: None,
            unsafe_: UnsafeSettings::default(),
        }
    }
}

impl WebSettings {
    /// The program-module variants this Web build emits (WASM-017 R9, D5):
    /// the multi-memory module when asked for, **and** the single-memory one
    /// unless "multi-memory only" was asked for (the fallback off) on a matrix
    /// with no WebKit engine — the loader then probes which to use. With WebKit
    /// in the matrix the single-memory module is never dropped silently:
    /// [`WebSettings::variant_problem`] refuses the build instead.
    pub fn module_variants(&self) -> ModuleVariants {
        ModuleVariants {
            multi_memory: self.multi_memory,
            single_memory: !self.multi_memory || self.single_memory_fallback || self.browsers.includes_webkit(),
        }
    }

    /// Why Check and Build refuse these variants: "multi-memory only" (the
    /// fallback off, multi-memory on) with Safari or the iOS web view in the
    /// matrix (WASM-017 AC7).
    pub fn variant_problem(&self) -> Option<VariantProblem> {
        let browsers = self.browsers.webkit_browsers();
        (self.multi_memory && !self.single_memory_fallback && !browsers.is_empty()).then_some(VariantProblem { browsers })
    }
}

/// `[mobile]` — a Mobile project's settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileSettings {
    pub platform: MobilePlatform,
    pub device: DeviceClass,
    #[serde(default)]
    pub orientation: Orientation,
    /// The oldest operating system the application runs on (WASM-012
    /// D-MinOS): `auto` is the floor of this PowerRustCOBOL major
    /// ([`min_os_floor`]: the last version released in 2024, Android 15 =
    /// API level `35`, iOS `18.0`); otherwise an Android API level or an iOS
    /// version at or above that floor. [`MobileSettings::resolved_min_os`]
    /// resolves `auto` and refuses a value below the floor.
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
    /// The payload limit, as [`WebSettings::max_payload_bytes`].
    #[serde(default = "default_max_payload", skip_serializing_if = "is_default_max_payload")]
    pub max_payload_bytes: u32,
    /// The remote-call wait, as [`WebSettings::call_timeout_ms`].
    #[serde(default = "default_call_timeout", skip_serializing_if = "is_default_call_timeout")]
    pub call_timeout_ms: u32,
    /// The loading effect, as [`WebSettings::loading_effect`].
    #[serde(default, skip_serializing_if = "LoadingEffect::is_default")]
    pub loading_effect: LoadingEffect,
    /// The application's identity on the device (WASM-012 R6): reverse-DNS,
    /// two or more segments, each `[A-Za-z][A-Za-z0-9_]*`
    /// ([`validate_app_id`]). Empty until set; a package build needs it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub app_id: String,
    /// The launcher icon, a square image of at least 1024 px in `Assets/`
    /// (R6); every platform size is made from it at Build.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub icon: String,
    /// The launch screen image in `Assets/` (R6). Optional.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub splash: String,
    /// The device features the application declares (R7): `secure-storage`,
    /// `open-address`, `deep-link` (`mobile_features`). A program that uses
    /// one it does not declare is refused at Build with the line.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    /// The project stylesheet, as [`WebSettings::stylesheet`]; the default is
    /// `mobileforms/application.css`.
    #[serde(default = "mobile_stylesheet", skip_serializing_if = "is_mobile_stylesheet")]
    pub stylesheet: String,
    /// The whole-theme file, as [`WebSettings::theme_file`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_file: Option<String>,
    /// The browser of the fallback Preview at a phone width (WASM-008 R15,
    /// C15), as [`WebSettings::preview_browser`]. Written only when set.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub preview_browser: String,
    /// `[mobile.unsafe]` (WASM-013 R11).
    #[serde(default, rename = "unsafe", skip_serializing_if = "UnsafeSettings::is_default")]
    pub unsafe_: UnsafeSettings,
    /// `[mobile.pinning]` (WASM-013 R14).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinning: Option<PinningSettings>,
    /// `[mobile.app_name]` — the application's name under its icon, per
    /// language (`en`, `es`, `pt`, `fr`, `ja`, `zh`); `en` is the one used
    /// where a language has none (R6). Empty = the project's name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub app_name: BTreeMap<String, String>,
    /// `[mobile.rationale.<feature>]` — why the application needs a feature
    /// that asks the user's permission, per language (R8; WASM-012 O10: a
    /// table of its own, because `features` is the list).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rationale: BTreeMap<String, BTreeMap<String, String>>,
    /// `[mobile.signing]` — the NAMES of the release signing material (R4).
    /// The passwords are never here: they live in the operating system's
    /// secret store and reach the build in memory.
    #[serde(default, skip_serializing_if = "MobileSigning::is_default")]
    pub signing: MobileSigning,
    /// `[mobile.native]` — the native project made for each platform (WASM-012 R16–R18): where it
    /// is in the project folder, empty while there is none. Written only once one is made.
    #[serde(default, skip_serializing_if = "NativeProjects::is_default")]
    pub native: NativeProjects,
}

/// `[mobile.native]` (WASM-012 R18): per platform, the folder of the native project the platform's
/// own tools open — the Android Studio project, the Xcode project — relative to the project folder,
/// or empty when none was made (declined in New Project, or not yet made from Project Settings).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeProjects {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub android: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ios: String,
}

impl NativeProjects {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// `[mobile.signing]` (WASM-012 R4): where the signing material is and what it
/// is called. None of these is a secret; the keystore must sit outside the
/// project folder ([`keystore_inside_project`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileSigning {
    /// The Android keystore file (`.jks`/`.keystore`), an absolute path.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub android_keystore: String,
    /// The key's alias inside the keystore.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub android_alias: String,
    /// The Apple developer team id (ten characters).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub apple_team_id: String,
    /// The signing identity's name in the login keychain.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub apple_identity: String,
    /// The provisioning profile's name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub apple_profile: String,
}

impl MobileSigning {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// The oldest operating system a Mobile project of this PowerRustCOBOL major
/// may target (WASM-012 D-MinOS, operator's Q3): the last version released in
/// 2024, raised one year per major. Android 15 is API level 35 (released
/// 2024-09-03); iOS 18 was released 2024-09-16 (plan §9 V0 record).
pub fn min_os_floor(platform: MobilePlatform) -> &'static str {
    match platform {
        MobilePlatform::Android => "35",
        MobilePlatform::Ios => "18.0",
    }
}

/// `major.minor` of an iOS version, or the API level of Android, as numbers
/// that compare (`"18"` and `"18.0"` are equal).
fn os_version_key(platform: MobilePlatform, v: &str) -> Option<(u32, u32)> {
    let v = v.trim();
    match platform {
        MobilePlatform::Android => v.parse::<u32>().ok().map(|n| (n, 0)),
        MobilePlatform::Ios => {
            let mut parts = v.split('.');
            let major = parts.next()?.parse::<u32>().ok()?;
            let minor = match parts.next() {
                Some(m) => m.parse::<u32>().ok()?,
                None => 0,
            };
            parts.next().map_or(Some((major, minor)), |p| p.parse::<u32>().ok().map(|_| (major, minor)))
        }
    }
}

/// Is `id` a valid application id (WASM-012 R6)? Reverse-DNS: two or more
/// segments separated by dots, each `[A-Za-z][A-Za-z0-9_]*`. `Err` says why.
pub fn validate_app_id(id: &str) -> Result<(), String> {
    let id = id.trim();
    if id.is_empty() {
        return Err("the application id is not set; give it a reverse-DNS name such as com.example.orders".to_owned());
    }
    let segments: Vec<&str> = id.split('.').collect();
    if segments.len() < 2 {
        return Err(format!("the application id '{id}' needs two or more parts separated by dots, such as com.example.orders"));
    }
    for s in &segments {
        let mut chars = s.chars();
        let ok_first = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
        // Letters and digits only: Android takes `_`, but the Tauri identifier and an iOS bundle
        // id do not (found 2026-11-07: `tauri android build` refuses `com.example.power_mobile`,
        // which `tauri android init` and Android accept).
        if !ok_first || !chars.all(|c| c.is_ascii_alphanumeric()) {
            return Err(format!(
                "the application id '{id}' has the part '{s}': each part starts with a letter and holds only letters and digits (no _ or -)"
            ));
        }
    }
    Ok(())
}

/// The application id a new Mobile project starts with (WASM-012 R6): `com.example.` and the
/// project's name in lower case, letters and digits only — what Android Studio offers for a new
/// project, valid for [`validate_app_id`] and for the Tauri identifier, which takes no `_`. The
/// developer replaces it with their own in Project Settings, Mobile, Application id; a store takes
/// no `com.example` id.
pub fn default_app_id(project_name: &str) -> String {
    let name = project_name.trim();
    let name = name.strip_suffix(".project").unwrap_or(name);
    let mut last: String = name.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect();
    if !last.starts_with(|c: char| c.is_ascii_alphabetic()) {
        last.insert_str(0, "app");
    }
    format!("com.example.{last}")
}

/// The build number of a version (WASM-012 D-BuildNo): `major*10000 +
/// minor*100 + patch`, the Android `versionCode` and the iOS
/// `CFBundleVersion`. `Err` when the version is not `x.y.z` with `y`, `z`
/// below 100.
pub fn build_number(version: &str) -> Result<u32, String> {
    let nums: Option<Vec<u32>> = version.trim().split('.').map(|p| p.parse::<u32>().ok()).collect();
    match nums.as_deref() {
        // Android's versionCode is at least 1 and at most 2,100,000,000.
        Some(&[major, minor, patch]) if minor < 100 && patch < 100 && major < 210_000 && (major, minor, patch) != (0, 0, 0) => {
            Ok(major * 10000 + minor * 100 + patch)
        }
        _ => Err(format!(
            "the project version '{version}' cannot give a build number: write it as major.minor.patch, minor and patch below 100"
        )),
    }
}

/// Does `keystore` point inside `project_dir` (WASM-012 R4)? A project folder
/// is shared and committed, and the keystore file is itself a secret, so a
/// keystore there is refused. A relative path is taken from the project.
pub fn keystore_inside_project(project_dir: &std::path::Path, keystore: &str) -> bool {
    let keystore = keystore.trim();
    if keystore.is_empty() {
        return false;
    }
    let path = std::path::Path::new(keystore);
    let path = if path.is_absolute() { path.to_path_buf() } else { project_dir.join(path) };
    resolve(&path).starts_with(resolve(project_dir))
}

/// `p` with its deepest existing ancestor canonicalized (symbolic links such
/// as macOS's `/var` → `/private/var` resolved) and the rest, which may not
/// exist yet, normalized onto it.
fn resolve(p: &std::path::Path) -> std::path::PathBuf {
    let p = normalize(p);
    let mut existing = p.as_path();
    let mut rest = Vec::new();
    loop {
        if let Ok(canon) = std::fs::canonicalize(existing) {
            return rest.iter().rev().fold(canon, |acc, c| acc.join(c));
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_owned());
                existing = parent;
            }
            _ => return p,
        }
    }
}

/// `a/./b/../c` → `a/c`, for a path that does not exist yet.
fn normalize(p: &std::path::Path) -> std::path::PathBuf {
    let mut out = std::path::PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
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
            max_payload_bytes: DEFAULT_MAX_PAYLOAD_BYTES,
            call_timeout_ms: DEFAULT_CALL_TIMEOUT_MS,
            loading_effect: LoadingEffect::default(),
            stylesheet: mobile_stylesheet(),
            theme_file: None,
            preview_browser: String::new(),
            unsafe_: UnsafeSettings::default(),
            pinning: None,
            app_id: String::new(),
            icon: String::new(),
            splash: String::new(),
            features: Vec::new(),
            app_name: BTreeMap::new(),
            rationale: BTreeMap::new(),
            signing: MobileSigning::default(),
            native: NativeProjects::default(),
        }
    }

    /// `min_os` as the build uses it (WASM-012 D-MinOS): `auto` (or empty)
    /// is [`min_os_floor`]; a value at or above the floor is itself; a value
    /// below it, or one that is not a version of the platform, is `Err`
    /// naming the floor.
    pub fn resolved_min_os(&self) -> Result<String, String> {
        let floor = min_os_floor(self.platform);
        let v = self.min_os.trim();
        if v.is_empty() || v.eq_ignore_ascii_case("auto") {
            return Ok(floor.to_owned());
        }
        let (what, example) = match self.platform {
            MobilePlatform::Android => ("an Android API level", "35"),
            MobilePlatform::Ios => ("an iOS version", "18.0"),
        };
        let Some(key) = os_version_key(self.platform, v) else {
            return Err(format!("min_os = \"{v}\" is not {what} (such as \"{example}\") or \"auto\""));
        };
        let floor_key = os_version_key(self.platform, floor).unwrap_or((0, 0));
        if key < floor_key {
            return Err(format!(
                "min_os = \"{v}\" is below the oldest {} this version of PowerRustCOBOL supports, {floor}; use \"auto\" or {floor} and newer",
                match self.platform {
                    MobilePlatform::Android => "Android API level",
                    MobilePlatform::Ios => "iOS version",
                }
            ));
        }
        Ok(v.to_owned())
    }

    /// The application's name in `lang`, else in English, else `fallback`
    /// (the project's name).
    pub fn display_name<'a>(&'a self, lang: &str, fallback: &'a str) -> &'a str {
        self.app_name
            .get(lang)
            .or_else(|| self.app_name.get("en"))
            .map(String::as_str)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(fallback)
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

    /// WASM-017 R8, D3: one payload limit; an old `[web]`/`[mobile]` table
    /// reads 384 KiB and is written back without the key; a set value
    /// round-trips.
    #[test]
    fn the_payload_limit_defaults_to_384_kib_and_is_written_only_when_set_017() {
        let old: WebSettings = toml::from_str("backend_url = \"wss://api.example.com/ws\"").unwrap();
        assert_eq!(old.max_payload_bytes, 384 * 1024);
        assert!(!toml::to_string_pretty(&old).unwrap().contains("max_payload_bytes"), "an old manifest is saved unchanged");
        let set: WebSettings = toml::from_str("max_payload_bytes = 524288").unwrap();
        assert_eq!(set.max_payload_bytes, 512 * 1024);
        assert!(toml::to_string_pretty(&set).unwrap().contains("max_payload_bytes = 524288"));
        let mobile: MobileSettings = toml::from_str("platform = \"ios\"\ndevice = \"phone\"").unwrap();
        assert_eq!(mobile.max_payload_bytes, DEFAULT_MAX_PAYLOAD_BYTES);
        assert_eq!(MobileSettings::new(MobilePlatform::Android, DeviceClass::Phone).max_payload_bytes, DEFAULT_MAX_PAYLOAD_BYTES);
        println!("payload limit: old table 393216, set 524288 round-trips, mobile default 393216");
    }

    /// 2.0.413: the loading effect is a project setting. An old `[web]`/`[mobile]` table reads "moving stripes" and is
    /// written back without the key; `none` and a named effect round-trip; an unknown name is refused.
    #[test]
    fn the_loading_effect_defaults_to_moving_stripes_and_is_written_only_when_chosen() {
        let old: WebSettings = toml::from_str("backend_url = \"wss://api.example.com/ws\"").unwrap();
        assert_eq!(old.loading_effect, LoadingEffect::MovingStripes);
        assert!(!toml::to_string_pretty(&old).unwrap().contains("loading_effect"), "an old manifest is saved unchanged");
        let none: WebSettings = toml::from_str("loading_effect = \"none\"").unwrap();
        assert_eq!(none.loading_effect, LoadingEffect::None);
        assert!(toml::to_string_pretty(&none).unwrap().contains("loading_effect = \"none\""));
        let stripes: WebSettings = toml::from_str("loading_effect = \"moving-stripes\"").unwrap();
        assert_eq!(stripes.loading_effect, LoadingEffect::MovingStripes);
        assert!(!toml::to_string_pretty(&stripes).unwrap().contains("loading_effect"), "the default is never written");
        let mobile: MobileSettings = toml::from_str("platform = \"ios\"\ndevice = \"phone\"\nloading_effect = \"none\"").unwrap();
        assert_eq!(mobile.loading_effect, LoadingEffect::None);
        assert_eq!(MobileSettings::new(MobilePlatform::Android, DeviceClass::Phone).loading_effect, LoadingEffect::MovingStripes);
        assert!(toml::from_str::<WebSettings>("loading_effect = \"confetti\"").is_err());
        for e in LoadingEffect::ALL {
            assert_eq!(LoadingEffect::parse(e.as_str()), Some(e), "{e:?} reads back from its own name");
        }
        println!("loading effect: old table = moving-stripes (key not written), none round-trips, unknown refused, {} effects listed", LoadingEffect::ALL.len());
    }

    /// WASM-005 R11: an old `[web]`/`[mobile]` table reads the 60 s remote-call
    /// wait and is written back without the key; a set value round-trips.
    #[test]
    fn the_call_timeout_defaults_to_60_s_and_is_written_only_when_set_005() {
        let old: WebSettings = toml::from_str("backend_url = \"wss://api.example.com/ws\"").unwrap();
        assert_eq!(old.call_timeout_ms, 60_000);
        assert!(!toml::to_string_pretty(&old).unwrap().contains("call_timeout_ms"), "an old manifest is saved unchanged");
        let set: WebSettings = toml::from_str("call_timeout_ms = 15000").unwrap();
        assert_eq!(set.call_timeout_ms, 15_000);
        assert!(toml::to_string_pretty(&set).unwrap().contains("call_timeout_ms = 15000"));
        let mobile: MobileSettings = toml::from_str("platform = \"android\"\ndevice = \"tablet\"").unwrap();
        assert_eq!(mobile.call_timeout_ms, DEFAULT_CALL_TIMEOUT_MS);
        assert_eq!(MobileSettings::new(MobilePlatform::Ios, DeviceClass::Phone).call_timeout_ms, DEFAULT_CALL_TIMEOUT_MS);
        let set: MobileSettings = toml::from_str("platform = \"ios\"\ndevice = \"phone\"\ncall_timeout_ms = 5000").unwrap();
        assert_eq!(set.call_timeout_ms, 5_000);
        println!("call timeout: old table 60000, set 15000 round-trips, mobile default 60000, mobile set 5000");
    }

    /// WASM-007 Q3a: the project stylesheet and the whole-theme file. A manifest
    /// without them loads with the defaults (in the kind's forms folder) and is
    /// written back as it was; a set value round-trips.
    #[test]
    fn the_stylesheet_keys_default_into_the_forms_folder_and_are_written_only_when_set() {
        let web_text = "backend_url = \"wss://api.example.com/ws\"\n";
        let web: WebSettings = toml::from_str(web_text).unwrap();
        assert_eq!((web.stylesheet.as_str(), web.theme_file.as_deref()), ("webforms/application.css", None));
        let written = toml::to_string_pretty(&web).unwrap();
        assert!(!written.contains("stylesheet") && !written.contains("theme_file"), "{written}");
        let mobile_text = "platform = \"android\"\ndevice = \"phone\"\nmulti_memory = true\n";
        let mobile: MobileSettings = toml::from_str(mobile_text).unwrap();
        assert_eq!((mobile.stylesheet.as_str(), mobile.theme_file.as_deref()), ("mobileforms/application.css", None));
        assert_eq!(MobileSettings::new(MobilePlatform::Ios, DeviceClass::Phone).stylesheet, "mobileforms/application.css");
        let written = toml::to_string_pretty(&mobile).unwrap();
        assert!(!written.contains("stylesheet") && !written.contains("theme_file"), "{written}");
        // Byte for byte: a table as the IDE wrote it before the keys existed is written back as it was.
        let old = "platform = \"android\"\ndevice = \"phone\"\norientation = \"auto\"\nmin_os = \"auto\"\nstyle = \"spa\"\nmulti_memory = true\nbackend_url = \"\"\n";
        assert_eq!(toml::to_string(&toml::from_str::<MobileSettings>(old).unwrap()).unwrap(), old);
        let old_web = toml::to_string(&WebSettings::default()).unwrap();
        assert_eq!(toml::to_string(&toml::from_str::<WebSettings>(&old_web).unwrap()).unwrap(), old_web);

        let set: WebSettings = toml::from_str("stylesheet = \"webforms/site.css\"\ntheme_file = \"webforms/brand-theme.css\"").unwrap();
        let text = toml::to_string_pretty(&set).unwrap();
        assert!(text.contains("stylesheet = \"webforms/site.css\"") && text.contains("theme_file = \"webforms/brand-theme.css\""), "{text}");
        assert_eq!(toml::from_str::<WebSettings>(&text).unwrap(), set);
        println!("stylesheet keys: web default {}, mobile default {}, absent keys not written, set values round-trip", web.stylesheet, mobile.stylesheet);
    }

    /// WASM-008 T30 (R12, Q7): `preview_browser` in `[web]` and `[mobile]`.
    /// An existing table (no key) loads and saves byte for byte; an id and a
    /// path round-trip; the key is absent from the saved text when empty.
    #[test]
    fn the_preview_browser_is_optional_and_only_written_when_set_008() {
        let old_web = "style = \"spa\"\nmulti_memory = true\nisolation = \"require-corp\"\nbackend_url = \"wss://api.example.com/ws\"\n\n[browsers]\nchromium = true\nfirefox = true\nsafari = true\nios_webview = true\nandroid_webview = true\n";
        let web: WebSettings = toml::from_str(old_web).unwrap();
        assert_eq!(web.preview_browser, "");
        assert_eq!(toml::to_string(&web).unwrap(), old_web, "an existing [web] table is saved unchanged");
        let old_mobile = "platform = \"android\"\ndevice = \"phone\"\norientation = \"portrait\"\nmin_os = \"auto\"\nstyle = \"spa\"\nmulti_memory = true\nbackend_url = \"\"\n";
        let mobile: MobileSettings = toml::from_str(old_mobile).unwrap();
        assert_eq!(mobile.preview_browser, "");
        assert_eq!(toml::to_string(&mobile).unwrap(), old_mobile, "an existing [mobile] table is saved unchanged");
        assert!(!toml::to_string(&WebSettings::default()).unwrap().contains("preview_browser"));
        assert!(!toml::to_string(&MobileSettings::new(MobilePlatform::Ios, DeviceClass::Phone)).unwrap().contains("preview_browser"));

        for value in ["firefox", "/Applications/Firefox Developer Edition.app", "C:\\Tools\\browser.exe"] {
            let set = WebSettings { preview_browser: value.into(), ..WebSettings::default() };
            let text = toml::to_string(&set).unwrap();
            assert!(text.contains("preview_browser = "), "{text}");
            assert_eq!(toml::from_str::<WebSettings>(&text).unwrap(), set);
            let set = MobileSettings { preview_browser: value.into(), ..MobileSettings::new(MobilePlatform::Android, DeviceClass::Phone) };
            let text = toml::to_string(&set).unwrap();
            assert_eq!(toml::from_str::<MobileSettings>(&text).unwrap(), set);
            println!("preview_browser {value:?} round-trips in [web] and [mobile]");
        }
        println!("preview_browser: existing [web] ({} bytes) and [mobile] ({} bytes) saved byte for byte", old_web.len(), old_mobile.len());
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
        assert_eq!((v.multi_memory, v.single_memory), (true, true), "no WebKit engine, fallback on (the default): both");
        chromium_only.single_memory_fallback = false;
        let v = chromium_only.module_variants();
        assert_eq!((v.multi_memory, v.single_memory), (true, false), "fallback off, no WebKit: multi-memory only");
        assert_eq!(chromium_only.variant_problem(), None);

        chromium_only.multi_memory = false;
        let v = chromium_only.module_variants();
        assert_eq!((v.multi_memory, v.single_memory), (false, true), "multi-memory off: single only");
    }

    /// WASM-017 AC7, D5: "multi-memory only" is the fallback turned off; with
    /// Safari or the iOS web view in the matrix it is refused naming them,
    /// never dropped silently; an old table reads the fallback on and is
    /// written back without the key.
    #[test]
    fn multi_memory_only_is_refused_while_webkit_is_in_the_matrix_017() {
        let old: WebSettings = toml::from_str("multi_memory = true").unwrap();
        assert!(old.single_memory_fallback, "an old table reads the fallback on");
        assert!(!toml::to_string_pretty(&old).unwrap().contains("single_memory_fallback"));
        let v = old.module_variants();
        assert_eq!((v.multi_memory, v.single_memory), (true, true), "default Web: both");
        assert_eq!(old.variant_problem(), None);

        let mut off = WebSettings { single_memory_fallback: false, ..WebSettings::default() };
        assert!(toml::to_string_pretty(&off).unwrap().contains("single_memory_fallback = false"));
        let p = off.variant_problem().expect("refused with WebKit in the matrix");
        assert_eq!(p.browsers, ["Safari", "iOS web view"]);
        assert!(p.to_string().contains("Safari, iOS web view"), "{p}");
        off.browsers.ios_webview = false;
        assert_eq!(off.variant_problem().unwrap().browsers, ["Safari"]);
        off.browsers.safari = false;
        assert_eq!(off.variant_problem(), None);
        let v = off.module_variants();
        assert_eq!((v.multi_memory, v.single_memory), (true, false), "fallback off without WebKit: multi only");
        off.multi_memory = false;
        off.browsers.safari = true;
        assert_eq!(off.variant_problem(), None, "multi-memory off is the single module, never refused");

        let ios = MobileSettings::new(MobilePlatform::Ios, DeviceClass::Phone).module_variants();
        assert_eq!((ios.multi_memory, ios.single_memory), (false, true), "iOS: single only by default");
        println!("AC7: default both; fallback off + Safari/iOS refused naming them; off without WebKit multi only; iOS single only");
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

    /// WASM-013 T5: a manifest without the security tables loads and saves unchanged; each table round-trips;
    /// `enabled` names the key paths that are on.
    #[test]
    fn the_security_tables_round_trip_and_an_old_manifest_is_saved_unchanged() {
        let old: WebSettings = toml::from_str("backend_url = \"wss://api.example.com/ws\"").unwrap();
        assert!(old.unsafe_.is_default() && old.unsafe_.enabled("web").is_empty());
        let saved = toml::to_string_pretty(&old).unwrap();
        assert!(!saved.contains("unsafe") && !saved.contains("security") && !saved.contains("pinning"), "{saved}");

        let web: WebSettings = toml::from_str("[unsafe]\nplain_ws = true\ncsp_report_only = true\n").unwrap();
        assert_eq!(web.unsafe_.enabled("web"), ["web.unsafe.plain_ws", "web.unsafe.csp_report_only"]);
        let back: WebSettings = toml::from_str(&toml::to_string_pretty(&web).unwrap()).unwrap();
        assert_eq!(back, web);

        let mobile: MobileSettings = toml::from_str("platform = \"ios\"\ndevice = \"phone\"\n[unsafe]\nskip_secret_scan = true\n[pinning]\npins = [\"sha256/AAAA\", \"sha256/BBBB\"]\nexpires = \"2027-01-31\"\n").unwrap();
        assert_eq!(mobile.unsafe_.enabled("mobile"), ["mobile.unsafe.skip_secret_scan"]);
        let pins = mobile.pinning.as_ref().unwrap();
        assert_eq!((pins.pins.len(), pins.expires.as_deref()), (2, Some("2027-01-31")));
        let back: MobileSettings = toml::from_str(&toml::to_string_pretty(&mobile).unwrap()).unwrap();
        assert_eq!(back, mobile);
        let plain: MobileSettings = toml::from_str("platform = \"android\"\ndevice = \"tablet\"").unwrap();
        let saved = toml::to_string_pretty(&plain).unwrap();
        assert!(!saved.contains("unsafe") && !saved.contains("pinning"), "{saved}");

        let sec: SecuritySettings = toml::from_str("contact = \"mailto:security@example.com\"\nexpires = \"2027-01-01T00:00:00Z\"\n").unwrap();
        assert_eq!((sec.contact.as_str(), sec.policy.as_str()), ("mailto:security@example.com", ""));
        let back: SecuritySettings = toml::from_str(&toml::to_string_pretty(&sec).unwrap()).unwrap();
        assert_eq!(back, sec);
        println!("security tables: old manifest unchanged, [unsafe], [pinning], [security] round-trip, enabled() names the keys");
    }

    /// WASM-013 T21 (R14): `[mobile.pinning]` needs two different, well-formed pins and a date that has not passed, and warns
    /// within 30 days of it. An absent table is pinning off (`check` is not called; the manifest keeps `pinning: None`).
    #[test]
    fn the_pinning_table_needs_two_pins_and_a_date_and_warns_a_month_before_it() {
        let now = cobolt_wire::security_txt::expiry_unix("2026-11-06T00:00:00Z").unwrap();
        // 43 characters and `=`: the 43rd must carry two zero bits, so it is one of A E I M Q U Y c g k o s w 0 4 8.
        let pin = |c: char, last: char| format!("sha256/{}{last}=", c.to_string().repeat(42));
        let (a, b) = (pin('A', 'A'), pin('x', 'E'));
        let table = |pins: &[&str], expires: Option<&str>| PinningSettings { pins: pins.iter().map(|p| p.to_string()).collect(), expires: expires.map(str::to_owned) };
        let mut cases = 0usize;
        let mut case = |label: &str, t: PinningSettings, warn: &[&str], err: &[&str]| {
            let given = t.pins.clone();
            let (w, e) = t.check(now);
            assert_eq!(w.len(), warn.len(), "{label}: warnings {w:?}");
            assert_eq!(e.len(), err.len(), "{label}: errors {e:?}");
            for (got, want) in w.iter().chain(&e).zip(warn.iter().chain(err)) {
                assert!(got.contains(want), "{label}: {got:?} should contain {want:?}");
            }
            for text in w.iter().chain(&e) {
                for pin in given.iter().filter(|p| !p.is_empty()) {
                    assert!(!text.contains(pin.as_str()), "{label}: a pin is named by its position, never quoted: {text}");
                }
            }
            cases += 1;
        };
        case("two pins and a date far off", table(&[&a, &b], Some("2027-06-30")), &[], &[]);
        case("a full time stamp", table(&[&a, &b], Some("2027-06-30T12:00:00Z")), &[], &[]);
        case("one pin", table(&[&a], Some("2027-06-30")), &[], &["at least two different pins are required (the live key and a backup), found 1"]);
        case("no pin", table(&[], Some("2027-06-30")), &[], &["found 0"]);
        case("the same pin twice", table(&[&a, &a], Some("2027-06-30")), &[], &["pin 2 repeats an earlier one", "found 1"]);
        for (label, bad) in [
            ("a pin too short", "sha256/AAAA".to_string()),
            ("no sha256/ prefix", format!("{}=", "A".repeat(43))),
            ("sha1, not sha256", format!("sha1/{}=", "A".repeat(43))),
            ("a character outside base64", format!("sha256/{}!=", "A".repeat(42))),
            ("the url-safe alphabet", format!("sha256/{}_=", "A".repeat(42))),
            ("two bits that are not zero", pin('A', 'B')),
            ("no padding", format!("sha256/{}", "A".repeat(44))),
            ("a pin of 33 bytes", format!("sha256/{}=", "A".repeat(44))),
            ("empty", String::new()),
        ] {
            case(label, table(&[&a, &bad], Some("2027-06-30")), &[], &["pin 2 is not sha256/ and the base64 of 32 bytes", "found 1"]);
        }
        case("no date", table(&[&a, &b], None), &[], &["expires: required"]);
        case("a blank date", table(&[&a, &b], Some("  ")), &[], &["expires: required"]);
        case("not a date", table(&[&a, &b], Some("soon")), &[], &["must be a date"]);
        case("a day that does not exist", table(&[&a, &b], Some("2027-02-30")), &[], &["must be a date"]);
        case("yesterday", table(&[&a, &b], Some("2026-11-05")), &[], &["the date has passed"]);
        case("a time stamp that has just passed", table(&[&a, &b], Some("2026-11-06T00:00:00Z")), &[], &["the date has passed"]);
        case("today, to its last second", table(&[&a, &b], Some("2026-11-06")), &["pin-expiring 2026-11-06 0"], &[]);
        case("29 days and a day to go", table(&[&a, &b], Some("2026-12-05")), &["pin-expiring 2026-12-05 29"], &[]);
        case("30 days and a day to go", table(&[&a, &b], Some("2026-12-06")), &[], &[]);
        case("everything wrong at once", table(&[&a], Some("2020-01-01")), &[], &["found 1", "the date has passed"]);
        // Off by default: an old manifest has no table, and is saved without one.
        let plain: MobileSettings = toml::from_str("platform = \"android\"\ndevice = \"phone\"").unwrap();
        assert!(plain.pinning.is_none());
        println!("pinning: {cases} cases (each refusal, the repeat, the 30-day edge at 29 and 30 days, today); a pin is named by position, never quoted; absent table = off");
    }

    /// WASM-012 plan §3 as corrected by O10: `features` is the list and the
    /// rationale texts are a table of their own.
    const FULL_MOBILE_TABLE: &str = r#"platform = "android"
device = "phone"
orientation = "portrait"
min_os = "auto"
style = "spa"
multi_memory = true
backend_url = ""
app_id = "com.example.orders"
icon = "Assets/icon.png"
splash = "Assets/splash.png"
features = ["secure-storage", "open-address", "deep-link"]

[app_name]
en = "Orders"
es = "Pedidos"

[rationale.secure-storage]
en = "Keeps you signed in on this phone."

[signing]
android_keystore = "/Users/me/keys/orders.jks"
android_alias = "orders"
apple_team_id = "ABCDE12345"
apple_identity = "Apple Distribution: Example Ltd"
apple_profile = "Orders App Store"
"#;

    #[test]
    fn the_full_mobile_table_round_trips() {
        let m: MobileSettings = toml::from_str(FULL_MOBILE_TABLE).unwrap();
        assert_eq!(m.app_id, "com.example.orders");
        assert_eq!(m.icon, "Assets/icon.png");
        assert_eq!(m.splash, "Assets/splash.png");
        assert_eq!(m.features, ["secure-storage", "open-address", "deep-link"]);
        assert_eq!(m.display_name("es", "P"), "Pedidos");
        assert_eq!(m.display_name("ja", "P"), "Orders", "a language without a name takes the English one");
        assert_eq!(m.rationale["secure-storage"]["en"], "Keeps you signed in on this phone.");
        assert_eq!(m.signing.android_alias, "orders");
        assert_eq!(m.signing.apple_profile, "Orders App Store");
        let again: MobileSettings = toml::from_str(&toml::to_string(&m).unwrap()).unwrap();
        assert_eq!(again, m);
        println!("[mobile]: 11 values and 3 tables (app_name, rationale.<feature>, signing) read and written back equal");
    }

    #[test]
    fn an_old_mobile_table_loads_with_every_new_default_and_saves_unchanged() {
        let old = MobileSettings::new(MobilePlatform::Android, DeviceClass::Phone);
        let text = toml::to_string(&old).unwrap();
        for key in ["app_id", "icon", "splash", "features", "app_name", "rationale", "signing"] {
            assert!(!text.contains(key), "a default {key} is not written: {text}");
        }
        let loaded: MobileSettings = toml::from_str(&text).unwrap();
        assert!(loaded.app_id.is_empty() && loaded.features.is_empty() && loaded.app_name.is_empty());
        assert!(loaded.rationale.is_empty() && loaded.signing.is_default());
        assert_eq!(toml::to_string(&loaded).unwrap(), text, "saved byte for byte");
        assert_eq!(loaded.display_name("en", "Orders.project"), "Orders.project");
        println!("an old [mobile] table ({} bytes) loads with 7 new defaults and saves byte for byte", text.len());
    }

    #[test]
    fn app_ids_are_reverse_dns() {
        for good in ["com.example.app", "com.example.orders2", "a.b"] {
            assert!(validate_app_id(good).is_ok(), "{good}");
        }
        for (bad, says) in [
            ("example", "two or more parts"),
            ("1com.x", "'1com'"),
            ("com..x", "the part ''"),
            ("com.ex-ample", "'ex-ample'"),
            ("com.example.orders_2", "no _ or -"),
            ("", "not set"),
        ] {
            let why = validate_app_id(bad).unwrap_err();
            assert!(why.contains(says), "{bad}: {why}");
        }
        println!("app_id: 3 accepted, 6 refused with the part named (an _ among them: Tauri and iOS take none)");
    }

    #[test]
    fn a_new_mobile_project_starts_with_a_valid_application_id_012() {
        for (name, want) in [
            ("PowerMobileHello.project", "com.example.powermobilehello"),
            ("Orders", "com.example.orders"),
            ("my-app 2", "com.example.myapp2"),
            ("2048.project", "com.example.app2048"),
            ("Ação", "com.example.ao"),
            ("", "com.example.app"),
        ] {
            let id = default_app_id(name);
            assert_eq!(id, want, "{name}");
            assert!(validate_app_id(&id).is_ok() && !id.contains('_'), "{id}");
        }
        println!("default app_id: 6 names, 6 valid reverse-DNS ids without _");
    }

    #[test]
    fn min_os_resolves_to_the_floor_and_refuses_below_it() {
        let android = |v: &str| MobileSettings { min_os: v.into(), ..MobileSettings::new(MobilePlatform::Android, DeviceClass::Phone) };
        let ios = |v: &str| MobileSettings { min_os: v.into(), ..MobileSettings::new(MobilePlatform::Ios, DeviceClass::Phone) };
        assert_eq!(android("auto").resolved_min_os().unwrap(), "35");
        assert_eq!(android("").resolved_min_os().unwrap(), "35");
        assert_eq!(android("36").resolved_min_os().unwrap(), "36");
        assert_eq!(ios("auto").resolved_min_os().unwrap(), "18.0");
        assert_eq!(ios("18").resolved_min_os().unwrap(), "18");
        assert_eq!(ios("26.0").resolved_min_os().unwrap(), "26.0");
        let low = android("26").resolved_min_os().unwrap_err();
        assert!(low.contains("below") && low.contains("35"), "{low}");
        let low = ios("17.4").resolved_min_os().unwrap_err();
        assert!(low.contains("below") && low.contains("18.0"), "{low}");
        assert!(android("16.0").resolved_min_os().unwrap_err().contains("not an Android API level"));
        assert!(ios("eighteen").resolved_min_os().unwrap_err().contains("not an iOS version"));
        println!("min_os: auto -> 35 / 18.0; 36, 18, 26.0 kept; 26 and 17.4 refused naming the floor; 2 malformed refused");
    }

    #[test]
    fn the_build_number_comes_from_the_version() {
        assert_eq!(build_number("2.0.29").unwrap(), 20029);
        assert_eq!(build_number("1.0.0").unwrap(), 10000);
        assert_eq!(build_number("0.1.0").unwrap(), 100);
        assert_eq!(build_number("10.99.99").unwrap(), 109999);
        for bad in ["1.0", "1.100.0", "1.0.100", "0.0.0", "1.0.0-beta", ""] {
            assert!(build_number(bad).is_err(), "{bad}");
        }
        println!("build number: 4 versions -> 20029 10000 100 109999; 6 refused");
    }

    #[test]
    fn a_keystore_inside_the_project_is_refused() {
        let dir = std::env::temp_dir().join(format!("prc-keystore-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("keys")).unwrap();
        let inside = dir.join("keys").join("app.jks");
        assert!(keystore_inside_project(&dir, &inside.to_string_lossy()));
        assert!(keystore_inside_project(&dir, "keys/app.jks"), "relative = from the project");
        assert!(keystore_inside_project(&dir, "./keys/../app.jks"));
        let outside = std::env::temp_dir().join("elsewhere-app.jks");
        assert!(!keystore_inside_project(&dir, &outside.to_string_lossy()));
        assert!(!keystore_inside_project(&dir, "../elsewhere.jks"));
        assert!(!keystore_inside_project(&dir, ""));
        let _ = std::fs::remove_dir_all(&dir);
        println!("keystore: 3 paths inside the project refused, 2 outside and an empty one accepted");
    }
}
