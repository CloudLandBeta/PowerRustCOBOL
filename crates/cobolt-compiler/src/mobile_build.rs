// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The mobile package** (WASM-012 R1–R7, T11–T13): a Mobile project's Web bundle wrapped in the
//! PowerRustCOBOL mobile shell (Tauri 2) and built into an Android package (`.apk`).
//!
//! The Build of a Mobile project first makes its Web bundle (`dist/web`), exactly as a Web
//! project's. This module then:
//!
//! 1. **checks** what a package needs and the bundle cannot say ([`check`]): the application id, a
//!    version that gives a build number, the oldest operating system, the declared device features
//!    against the ones the program uses (the bundle manifest's `builtins`), the icon, and for a
//!    release the signing material;
//! 2. **stages** a build folder outside the project ([`stage`]): the bundle as `frontend/`, the
//!    shell crate as `shell/` (its sources are compiled into this program, so an installed IDE
//!    needs no source tree), and a generated application crate `src-tauri/` — its `Cargo.toml`,
//!    build script, three-line entry point, `tauri.conf.json` and capability file, all made from
//!    `[mobile]`;
//! 3. **runs the Tauri 2 command line** ([`build_android`]): `android init` the first time (the
//!    Gradle project Tauri generates lives in the staging folder, never in the project), `icon`,
//!    and `android build`, with the Android SDK, NDK and a Java 17–21 found by [`Toolchain`];
//! 4. **copies the package** into the project as `dist/mobile/android/<Name>-<debug|release>.apk`.
//!
//! No Kotlin, Swift, Java or Gradle file is kept in the repository or the project (D-Native): the
//! two edits the generated Gradle project needs — the screen orientation and the release signing,
//! read from the build's environment (D-Sign) — are made to it in the staging folder each build.
//! The signing passwords reach Gradle only as environment variables of the build process: they are
//! never written to a file.
//!
//! **iOS** ([`build_ios`], WASM-012 R1, T11 iOS half) is the same four steps with Xcode in place of
//! Gradle, on a Mac only: the package is the **simulator** application (`dist/mobile/ios/<Name>-debug.app`,
//! Apple silicon). A release archive needs the developer's Apple team, identity and provisioning
//! profile and is refused with the reason (R5). What Tauri's `ios init` generates (an XcodeGen
//! project) lives in the staging folder too; the orientation, the version, the name under the icon
//! and its translations reach it through the files Tauri itself merges at every build
//! (`Info.ios.plist`) and the `InfoPlist.strings` of each language, never by editing a generated
//! file. [`IosToolchain`] checks Xcode, XcodeGen, libimobiledevice and CocoaPods first and names
//! each one that is missing with the command that installs it: `ios init` would install them by
//! itself, which this program never lets it do.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use serde_json::{json, Value};

use crate::mobile_features::{self, Feature};
use crate::project_kind::{self, ApplicationStyle, MobilePlatform, MobileSettings, Orientation};

/// The Tauri command line this release builds with (D-Pin, plan §9 V0 record).
pub const TAURI_CLI_VERSION: &str = "2.12.4";

/// The command that installs it.
pub const INSTALL_TAURI_CLI: &str = "cargo install tauri-cli --version 2.12.4 --locked";

/// The shell crate, compiled into this program and written into every staging folder.
const SHELL_LIB: &str = include_str!("../../cobolt-mobile-shell/src/lib.rs");
const SHELL_MANIFEST: &str = include_str!("../../cobolt-mobile-shell/Cargo.toml");
/// The shell's lock: the staged application starts from exactly the closure the shell's
/// dependency guard checked (D-Pin).
const SHELL_LOCK: &str = include_str!("../../cobolt-mobile-shell/Cargo.lock");
/// The launcher icon of a project that names none: the PowerRustCOBOL icon (1024 px).
const DEFAULT_ICON: &[u8] = include_bytes!("../../../assets/images/powerrustcobol-icon.png");

/// Where the packages go, inside the project.
pub const PACKAGE_SUBDIR: &str = "dist/mobile";

/// The Web bundle a package wraps, inside the project.
pub const BUNDLE_SUBDIR: &str = "dist/web";

/// What the Tauri command line calls the iOS simulator of an Apple-silicon Mac (`--target`), the Rust
/// target it compiles to, and the folder under `gen/apple/build/` it writes the application in.
pub const IOS_TAURI_TARGET: &str = "aarch64-sim";
pub const IOS_RUST_TARGET: &str = "aarch64-apple-ios-sim";
const IOS_BUILD_SUBDIR: &str = "arm64-sim";

/// The environment variables the generated Gradle project reads the release signing from.
pub const ENV_KEYSTORE: &str = "PRC_ANDROID_KEYSTORE";
pub const ENV_KEY_ALIAS: &str = "PRC_ANDROID_KEY_ALIAS";
pub const ENV_STORE_PASSWORD: &str = "PRC_ANDROID_STORE_PASSWORD";
pub const ENV_KEY_PASSWORD: &str = "PRC_ANDROID_KEY_PASSWORD";

/// A development package (debuggable, signed with the SDK's debug key) or a release one (signed
/// with the developer's key).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Debug,
    Release,
}

impl Profile {
    pub fn as_str(self) -> &'static str {
        match self {
            Profile::Debug => "debug",
            Profile::Release => "release",
        }
    }
}

/// The processor an Android package is built for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AndroidAbi {
    /// Every phone of the supported Android versions, and the emulator on an Apple-silicon Mac.
    Aarch64,
    /// The emulator on an Intel computer.
    X86_64,
    Armv7,
    I686,
}

impl AndroidAbi {
    /// The name the Tauri command line takes (`--target`).
    pub fn tauri_target(self) -> &'static str {
        match self {
            AndroidAbi::Aarch64 => "aarch64",
            AndroidAbi::X86_64 => "x86_64",
            AndroidAbi::Armv7 => "armv7",
            AndroidAbi::I686 => "i686",
        }
    }

    /// The Rust target it compiles to.
    pub fn rust_target(self) -> &'static str {
        match self {
            AndroidAbi::Aarch64 => "aarch64-linux-android",
            AndroidAbi::X86_64 => "x86_64-linux-android",
            AndroidAbi::Armv7 => "armv7-linux-androideabi",
            AndroidAbi::I686 => "i686-linux-android",
        }
    }

    /// From a device's `ro.product.cpu.abi` (`arm64-v8a`, `x86_64`, `armeabi-v7a`, `x86`).
    pub fn from_device_abi(abi: &str) -> Option<Self> {
        match abi.trim() {
            "arm64-v8a" => Some(AndroidAbi::Aarch64),
            "x86_64" => Some(AndroidAbi::X86_64),
            "armeabi-v7a" => Some(AndroidAbi::Armv7),
            "x86" => Some(AndroidAbi::I686),
            _ => None,
        }
    }
}

/// The two passwords of a release signing. They come from the operating system's secret store or
/// the build's environment, are handed to Gradle as environment variables of its process, and are
/// never printed: `Debug` hides them.
#[derive(Clone)]
pub struct SigningSecrets {
    pub store_password: String,
    pub key_password: String,
}

impl std::fmt::Debug for SigningSecrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SigningSecrets { .. }")
    }
}

/// What one package build is asked to make.
#[derive(Debug, Clone)]
pub struct PackageRequest {
    pub project_dir: PathBuf,
    /// The project's name (the manifest's, without `.project`): the name under the icon when
    /// `[mobile.app_name]` gives none, and the package file's name.
    pub project_name: String,
    /// `[project] version` (`x.y.z`): the package's version and its build number.
    pub version: String,
    pub settings: MobileSettings,
    /// The Web bundle (`dist/web` of the project).
    pub bundle_dir: PathBuf,
    /// The staging folder, outside the project; kept between builds (the Rust and Gradle caches
    /// live in it, so the second build takes a fraction of the first).
    pub staging_dir: PathBuf,
    pub profile: Profile,
    pub abi: AndroidAbi,
}

impl PackageRequest {
    /// The request for the project whose manifest is `manifest_path`.
    pub fn from_manifest(manifest_path: &Path, staging_dir: PathBuf, profile: Profile, abi: AndroidAbi) -> Result<Self, String> {
        let view = crate::project_manifest_view(manifest_path)?;
        if view.kind != crate::ProjectKind::Mobile {
            return Err(format!("the project is a {} project; only a Mobile project has a mobile package", view.kind.as_str()));
        }
        let settings = view.mobile.clone().ok_or_else(|| "the project file has no [mobile] table".to_owned())?;
        let project_dir = manifest_path.parent().map(Path::to_path_buf).unwrap_or_default();
        Ok(Self {
            bundle_dir: project_dir.join(BUNDLE_SUBDIR),
            project_dir,
            project_name: application_name(&view.name),
            version: view.version.clone(),
            settings,
            staging_dir,
            profile,
            abi,
        })
    }

    /// The Rust crate name of the staged application.
    pub fn crate_name(&self) -> String {
        crate_name(&self.project_name)
    }

    /// The name under the icon (English, the default language of the package).
    pub fn display_name(&self) -> &str {
        self.settings.display_name("en", &self.project_name)
    }

    /// Where the package is copied in the project: an `.apk` for Android, an `.app` folder (the
    /// application the simulator installs) for iOS.
    pub fn package_path(&self) -> PathBuf {
        let extension = match self.settings.platform {
            MobilePlatform::Android => "apk",
            MobilePlatform::Ios => "app",
        };
        let file = format!("{}-{}.{extension}", file_stem(&self.project_name), self.profile.as_str());
        self.project_dir.join(PACKAGE_SUBDIR).join(self.settings.platform.as_str()).join(file)
    }
}

/// The project's name without the `.project` a manifest's name carries.
pub fn application_name(manifest_name: &str) -> String {
    let n = manifest_name.trim();
    n.strip_suffix(".project").unwrap_or(n).to_owned()
}

/// A Rust crate name made of the name's letters and digits, in lower case (`PowerMobileHello` →
/// `powermobilehello`); `app` before it when it would not start with a letter.
pub fn crate_name(name: &str) -> String {
    let s: String = name.chars().filter(char::is_ascii_alphanumeric).collect::<String>().to_ascii_lowercase();
    if s.starts_with(|c: char| c.is_ascii_alphabetic()) {
        s
    } else {
        format!("app{s}")
    }
}

/// A file name made of the name's letters, digits, `-` and `_`.
fn file_stem(name: &str) -> String {
    let s: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    if s.is_empty() {
        "application".to_owned()
    } else {
        s
    }
}

/// The deep-link scheme of an application id, as the shell reads it
/// (`cobolt_mobile_shell::deep_link_scheme`): the id in lower case, `_` written `-`.
pub fn deep_link_scheme(app_id: &str) -> String {
    app_id.trim().to_ascii_lowercase().replace('_', "-")
}

/// The width and height of a PNG image, from its header.
pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[..8] != SIGNATURE || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

/// The built-in calls the program uses, from the bundle's manifests (`manifest.<hash>.json`).
pub fn bundle_builtins(bundle_dir: &Path) -> Result<BTreeSet<String>, String> {
    let entries = std::fs::read_dir(bundle_dir)
        .map_err(|e| format!("the Web bundle {} cannot be read ({e}); build the project first", bundle_dir.display()))?;
    let mut out = BTreeSet::new();
    let mut seen = false;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !(name.starts_with("manifest.") && name.ends_with(".json")) {
            continue;
        }
        seen = true;
        let text = std::fs::read_to_string(entry.path()).map_err(|e| format!("{name} cannot be read ({e})"))?;
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("{name} is not valid JSON ({e})"))?;
        if let Some(list) = v.get("builtins").and_then(Value::as_array) {
            out.extend(list.iter().filter_map(Value::as_str).map(str::to_owned));
        }
    }
    if !seen {
        return Err(format!("the Web bundle {} has no manifest; build the project first", bundle_dir.display()));
    }
    Ok(out)
}

/// What [`check`] found good: the values the staged files are made from.
#[derive(Debug, Clone)]
pub struct Checked {
    pub app_id: String,
    pub build_number: u32,
    /// The oldest Android API level, and the oldest iOS version.
    pub android_min_sdk: u32,
    pub ios_min_version: String,
    /// The declared features, in the table's order.
    pub features: Vec<&'static Feature>,
    /// The launcher icon's bytes (the project's, or the PowerRustCOBOL icon).
    pub icon: Vec<u8>,
}

/// Check what a package needs, all at once: `Err` lists every problem, each a sentence that says
/// what to change. `signing` is looked at only for a release.
pub fn check(req: &PackageRequest, signing: Option<&SigningSecrets>) -> Result<Checked, Vec<String>> {
    check_parts(req, signing, true)
}

/// [`check`] without the Web bundle: what the shell itself needs (the identity, the version, the
/// minimum OS, the declared features, the icon). The native project of a project not built yet is
/// made from it (WASM-012 R16); its bundle arrives with the first Build.
pub fn check_shell(req: &PackageRequest) -> Result<Checked, Vec<String>> {
    check_parts(req, None, false)
}

fn check_parts(req: &PackageRequest, signing: Option<&SigningSecrets>, with_bundle: bool) -> Result<Checked, Vec<String>> {
    let s = &req.settings;
    let mut problems = Vec::new();
    if let Err(e) = project_kind::validate_app_id(&s.app_id) {
        problems.push(format!("{e} (Mobile settings, Application id)"));
    }
    let build_number = project_kind::build_number(&req.version).map_err(|e| problems.push(e)).unwrap_or(0);
    let min_os = s.resolved_min_os().map_err(|e| problems.push(e)).unwrap_or_default();
    let (android_min_sdk, ios_min_version) = match s.platform {
        MobilePlatform::Android => (min_os.parse().unwrap_or(0), project_kind::min_os_floor(MobilePlatform::Ios).to_owned()),
        MobilePlatform::Ios => (project_kind::min_os_floor(MobilePlatform::Android).parse().unwrap_or(35), min_os),
    };
    // Each already a sentence that names the feature and the ones there are.
    problems.extend(mobile_features::unknown_declared(&s.features));
    let bundled = with_bundle && req.bundle_dir.join("index.html").is_file();
    if with_bundle && !bundled {
        problems.push(format!("the Web bundle {} is missing; build the project first", req.bundle_dir.display()));
    }
    if bundled {
        match bundle_builtins(&req.bundle_dir) {
            Ok(builtins) => {
                let used = mobile_features::features_of_builtins(builtins.iter().map(String::as_str));
                for f in used {
                    if !s.features.iter().any(|d| d.trim().eq_ignore_ascii_case(f)) {
                        problems.push(format!(
                            "the program uses the device feature '{f}' but the project does not declare it; add it to Mobile settings, Device features"
                        ));
                    }
                }
            }
            Err(e) => problems.push(e),
        }
    }
    let icon = if s.icon.trim().is_empty() {
        DEFAULT_ICON.to_vec()
    } else {
        let path = req.project_dir.join(s.icon.trim());
        match std::fs::read(&path) {
            Ok(bytes) => {
                match png_size(&bytes) {
                    Some((w, h)) if w == h && w >= 1024 => {}
                    Some((w, h)) => problems.push(format!("the icon {} is {w} x {h} px; it must be a square PNG of at least 1024 px", s.icon.trim())),
                    None => problems.push(format!("the icon {} is not a PNG image", s.icon.trim())),
                }
                bytes
            }
            Err(e) => {
                problems.push(format!("the icon {} cannot be read ({e})", path.display()));
                Vec::new()
            }
        }
    };
    if req.profile == Profile::Release && s.platform == MobilePlatform::Ios {
        // R5: never an unsigned archive, never one signed with a development identity. The archive
        // needs the Apple developer account's material, which this program neither holds nor tests.
        problems.push(
            "an iOS release archive needs your Apple developer team, signing identity and provisioning profile, and this version does not build it; build the development package (Debug), which runs on the iOS simulator"
                .to_owned(),
        );
    } else if req.profile == Profile::Release {
        let sg = &s.signing;
        if sg.android_keystore.trim().is_empty() || sg.android_alias.trim().is_empty() {
            problems.push("a release package needs the keystore and the key's alias (Mobile settings, Signing)".to_owned());
        } else if project_kind::keystore_inside_project(&req.project_dir, &sg.android_keystore) {
            problems.push("the keystore is inside the project folder, which is shared and committed; move it outside the project".to_owned());
        } else if !Path::new(sg.android_keystore.trim()).is_file() {
            problems.push(format!("the keystore {} does not exist", sg.android_keystore.trim()));
        }
        match signing {
            Some(sec) if !sec.store_password.is_empty() && !sec.key_password.is_empty() => {}
            _ => problems.push(format!(
                "a release package needs the keystore's and the key's passwords: store them in the system's secret store from Mobile settings, Signing, or set {ENV_STORE_PASSWORD} and {ENV_KEY_PASSWORD} for the build"
            )),
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    Ok(Checked {
        app_id: s.app_id.trim().to_owned(),
        build_number,
        android_min_sdk,
        ios_min_version,
        features: mobile_features::declared_features(&s.features),
        icon,
    })
}

fn has(features: &[&Feature], name: &str) -> bool {
    features.iter().any(|f| f.name == name)
}

/// The staged application's `Cargo.toml`.
pub fn app_manifest(crate_name: &str, features: &[&Feature]) -> String {
    let shell_features: Vec<String> = features.iter().map(|f| format!("\"{}\"", f.name)).collect();
    let mut deps = String::new();
    if has(features, "open-address") {
        deps.push_str("tauri-plugin-opener = \"=2.7.2\"\n");
    }
    if has(features, "deep-link") {
        deps.push_str("tauri-plugin-deep-link = \"=2.6.3\"\n");
    }
    format!(
        r#"# Generated by PowerRustCOBOL AI for the mobile package (WASM-012). Rewritten at every Build.
[package]
name = "{crate_name}"
version = "1.0.0"
edition = "2021"
publish = false

[workspace]

[lib]
name = "{crate_name}_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = {{ version = "=2.7.4", default-features = false }}

[dependencies]
tauri = {{ version = "=2.12.5", default-features = false, features = ["wry", "compression"] }}
cobolt-mobile-shell = {{ path = "../shell", features = [{shell_features}] }}
{deps}serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"

# A development package: the shell is not debugged from the IDE, so no debug information (it was
# 120 MB of a 140 MB package, and every install copied it).
[profile.dev]
debug = false
strip = "debuginfo"

[profile.release]
codegen-units = 1
lto = true
opt-level = "s"
panic = "abort"
strip = true
"#,
        shell_features = shell_features.join(", "),
    )
}

/// The staged application's build script: the shell's commands are declared, so their
/// permissions (`allow-secure-store`…) exist for the capability file.
pub fn app_build_script(features: &[&Feature]) -> String {
    let attributes = if has(features, "secure-storage") {
        "tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[\"secure_store\", \"secure_load\", \"secure_delete\"]))"
    } else {
        "tauri_build::Attributes::new()"
    };
    format!(
        "// Generated by PowerRustCOBOL AI for the mobile package (WASM-012).\nfn main() {{\n    tauri_build::try_build({attributes}).expect(\"the mobile shell's build configuration\");\n}}\n"
    )
}

/// The staged application's entry point: the shell, with what it needs to know.
pub fn app_entry(app_id: &str, title: &str, style: ApplicationStyle) -> String {
    format!(
        "// Generated by PowerRustCOBOL AI for the mobile package (WASM-012).\n#[cfg_attr(mobile, tauri::mobile_entry_point)]\npub fn run() {{\n    cobolt_mobile_shell::run(\n        tauri::generate_context!(),\n        cobolt_mobile_shell::ShellConfig {{ app_id: {app_id:?}, title: {title:?}, single_page: {} }},\n    );\n}}\n",
        style == ApplicationStyle::Spa
    )
}

/// The staged `tauri.conf.json`.
pub fn tauri_conf(req: &PackageRequest, c: &Checked) -> Value {
    // The name under an Android icon is the product's name. On iOS the product name is the name of
    // the Xcode target, the executable and the `.app` folder: a file-safe one, while the name under
    // the icon (`CFBundleDisplayName`, with its translations) is written to `Info.ios.plist`.
    let product = match req.settings.platform {
        MobilePlatform::Android => req.display_name().to_owned(),
        MobilePlatform::Ios => file_stem(&req.project_name),
    };
    let mut conf = json!({
        "$schema": "https://schema.tauri.app/config/2",
        "productName": product,
        "version": req.version.trim(),
        "identifier": c.app_id,
        "build": { "frontendDist": "../frontend" },
        // The one window is opened by the shell, with its navigation guard; the page carries its
        // own Content-Security-Policy (WASM-013), so Tauri adds none.
        "app": { "windows": [], "security": { "csp": null } },
        "bundle": {
            "active": true,
            "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.png"],
            "android": { "minSdkVersion": c.android_min_sdk, "versionCode": c.build_number },
            "iOS": { "minimumSystemVersion": c.ios_min_version },
        },
    });
    if req.settings.platform == MobilePlatform::Ios {
        // `CFBundleVersion`, the build number (D-BuildNo), beside the version's own
        // `CFBundleShortVersionString`.
        conf["bundle"]["iOS"]["bundleVersion"] = json!(c.build_number.to_string());
    }
    if has(&c.features, "deep-link") {
        conf["plugins"] = json!({ "deep-link": { "mobile": [{ "scheme": [deep_link_scheme(&c.app_id)], "appLink": false }] } });
    }
    conf
}

/// The iOS orientations a project allows, for the iPhone and for the iPad (`Auto`: the device's, which
/// is what Tauri's template writes, so nothing is overridden).
fn ios_orientations(orientation: Orientation) -> Option<(&'static [&'static str], &'static [&'static str])> {
    match orientation {
        Orientation::Auto => None,
        Orientation::Portrait => Some((&["UIInterfaceOrientationPortrait"], &["UIInterfaceOrientationPortrait", "UIInterfaceOrientationPortraitUpsideDown"])),
        Orientation::Landscape => {
            const LANDSCAPE: &[&str] = &["UIInterfaceOrientationLandscapeLeft", "UIInterfaceOrientationLandscapeRight"];
            Some((LANDSCAPE, LANDSCAPE))
        }
    }
}

/// A text for a property list (`&`, `<` and `>` written as entities).
fn plist_text(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// The language folders of the name under an iOS icon (`<lang>.lproj`), for the languages of
/// `[mobile.app_name]` that have a name. `en` is the development language and needs none.
fn ios_lproj(lang: &str) -> Option<&'static str> {
    match lang {
        "es" => Some("es"),
        "pt" => Some("pt"),
        "fr" => Some("fr"),
        "ja" => Some("ja"),
        "zh" => Some("zh-Hans"),
        _ => None,
    }
}

/// The languages with a name of their own, as `(lproj, name)`, in the table's order.
fn ios_names(req: &PackageRequest) -> Vec<(&'static str, String)> {
    ["es", "pt", "fr", "ja", "zh"]
        .iter()
        .filter_map(|lang| {
            let name = req.settings.app_name.get(*lang)?.trim();
            (!name.is_empty()).then(|| (ios_lproj(lang).unwrap_or_default(), name.to_owned()))
        })
        .collect()
}

/// `src-tauri/Info.ios.plist`: the keys the Tauri command line merges over the generated
/// `Info.plist` at every iOS build — the name under the icon, the languages it is translated into
/// and, when the project sets one, the orientations. Nothing else: the version and the build number
/// come from `tauri.conf.json`, and no device feature of the first set has a usage key.
pub fn ios_info_plist(req: &PackageRequest) -> String {
    let mut body = format!("\t<key>CFBundleDisplayName</key>\n\t<string>{}</string>\n", plist_text(req.display_name()));
    let mut languages = vec!["en"];
    languages.extend(ios_names(req).into_iter().map(|(lproj, _)| lproj));
    body.push_str("\t<key>CFBundleLocalizations</key>\n\t<array>\n");
    for l in languages {
        body.push_str(&format!("\t\t<string>{l}</string>\n"));
    }
    body.push_str("\t</array>\n");
    if let Some((phone, pad)) = ios_orientations(req.settings.orientation) {
        for (key, values) in [("UISupportedInterfaceOrientations", phone), ("UISupportedInterfaceOrientations~ipad", pad)] {
            body.push_str(&format!("\t<key>{key}</key>\n\t<array>\n"));
            for v in values {
                body.push_str(&format!("\t\t<string>{v}</string>\n"));
            }
            body.push_str("\t</array>\n");
        }
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- Generated by PowerRustCOBOL AI from [mobile] (WASM-012). Rewritten at every Build. -->\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n{body}</dict>\n</plist>\n"
    )
}

/// A name for an `InfoPlist.strings` file: quoted, with `"` and `\` escaped.
pub fn ios_strings_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' | '\r' => out.push(' '),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The staged capability file: what the page may ask of the shell, and nothing more.
pub fn capability(features: &[&Feature]) -> Value {
    let mut permissions = vec!["core:default".to_owned()];
    for f in features {
        // The opener is called by the shell's navigation guard, from Rust: the page needs no
        // permission for it.
        if f.name == "open-address" {
            continue;
        }
        permissions.extend(f.permissions.iter().map(|p| (*p).to_owned()));
    }
    json!({
        "$schema": "../gen/schemas/mobile-schema.json",
        "identifier": "default",
        "description": "What the PowerRustCOBOL page may ask of the mobile shell (generated)",
        "windows": ["main"],
        "permissions": permissions,
    })
}

/// The Android manifest with the activity's screen orientation set (or left to the device).
pub fn patch_orientation(manifest: &str, orientation: Orientation) -> String {
    const ACTIVITY: &str = "android:name=\".MainActivity\"";
    const ATTR: &str = "android:screenOrientation=\"";
    let mut text = manifest.to_owned();
    if let Some(start) = text.find(ATTR) {
        if let Some(len) = text[start + ATTR.len()..].find('"') {
            // The attribute and the white space before it (the line break it was written on).
            let end = start + ATTR.len() + len + 1;
            let from = text[..start].trim_end().len();
            text.replace_range(from..end, "");
        }
    }
    let value = match orientation {
        Orientation::Auto => return text,
        Orientation::Portrait => "portrait",
        Orientation::Landscape => "landscape",
    };
    text.replacen(ACTIVITY, &format!("{ACTIVITY}\n            android:screenOrientation=\"{value}\""), 1)
}

/// The marker of the signing block in the generated Gradle script.
const SIGNING_MARKER: &str = "// PowerRustCOBOL: release signing (WASM-012 D-Sign)";

/// The generated Gradle script with the release signing read from the build's environment
/// (D-Sign). Applied once; a script that has it is returned as it is.
pub fn patch_signing(gradle: &str) -> String {
    if gradle.contains(SIGNING_MARKER) {
        return gradle.to_owned();
    }
    let block = format!(
        "android {{\n    {SIGNING_MARKER}: the names and passwords come from the build's environment, never from a file.\n    signingConfigs {{\n        create(\"prcRelease\") {{\n            val keystore = System.getenv(\"{ENV_KEYSTORE}\")\n            if (keystore != null) {{\n                storeFile = file(keystore)\n                storePassword = System.getenv(\"{ENV_STORE_PASSWORD}\")\n                keyAlias = System.getenv(\"{ENV_KEY_ALIAS}\")\n                keyPassword = System.getenv(\"{ENV_KEY_PASSWORD}\")\n            }}\n        }}\n    }}\n"
    );
    let text = gradle.replacen("android {\n", &block, 1);
    text.replacen(
        "getByName(\"release\") {\n",
        &format!("getByName(\"release\") {{\n            if (System.getenv(\"{ENV_KEYSTORE}\") != null) {{ signingConfig = signingConfigs.getByName(\"prcRelease\") }}\n"),
        1,
    )
}

/// Android's resource name for a language of `[mobile.app_name]`.
fn android_values_dir(lang: &str) -> Option<&'static str> {
    match lang {
        "es" => Some("values-es"),
        "pt" => Some("values-pt"),
        "fr" => Some("values-fr"),
        "ja" => Some("values-ja"),
        "zh" => Some("values-zh"),
        _ => None,
    }
}

/// A string resource's text, escaped for Android's `strings.xml`.
pub fn android_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\'' => out.push_str("\\'"),
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '@' | '?' if out.is_empty() => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// The folders of a staging.
#[derive(Debug, Clone)]
pub struct Staged {
    pub root: PathBuf,
    /// `src-tauri/`, the generated application crate.
    pub app_dir: PathBuf,
    /// The Gradle project Tauri generates (`src-tauri/gen/android`).
    pub android_dir: PathBuf,
    /// The XcodeGen and Xcode project Tauri generates (`src-tauri/gen/apple`).
    pub apple_dir: PathBuf,
    /// The icon the platform sizes are made from.
    pub icon: PathBuf,
}

/// Write `text` to `path` unless it already holds it (an unchanged file keeps its time, so cargo
/// and Gradle do not rebuild what did not change).
pub(crate) fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_changed(path, bytes).map(|_| ())
}

/// [`write_if_changed`], saying whether it wrote.
pub(crate) fn write_changed(path: &Path, bytes: &[u8]) -> Result<bool, String> {
    if std::fs::read(path).is_ok_and(|old| old == bytes) {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{} cannot be created ({e})", dir.display()))?;
    }
    std::fs::write(path, bytes).map_err(|e| format!("{} cannot be written ({e})", path.display()))?;
    Ok(true)
}

/// Copy the folder `from` to `to`, replacing what `to` held.
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    if to.exists() {
        std::fs::remove_dir_all(to).map_err(|e| format!("{} cannot be emptied ({e})", to.display()))?;
    }
    std::fs::create_dir_all(to).map_err(|e| format!("{} cannot be created ({e})", to.display()))?;
    let entries = std::fs::read_dir(from).map_err(|e| format!("{} cannot be read ({e})", from.display()))?;
    for entry in entries.flatten() {
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_tree(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst).map_err(|e| format!("{} cannot be copied ({e})", src.display()))?;
        }
    }
    Ok(())
}

/// The page the shell shows before the project's first Build put its bundle there (a native project
/// made with a new project, WASM-012 R16).
pub const NOT_BUILT_PAGE: &str = "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1, viewport-fit=cover\"><title>Not built yet</title></head>\n<body style=\"font-family: system-ui, sans-serif; margin: 48px 24px\"><p>This application has not been built yet. Build the project in PowerRustCOBOL AI: the Build puts the application here.</p></body></html>\n";

/// Write the staging folder for `req`. The Web bundle becomes `frontend/`; with no bundle yet the
/// shell shows [`NOT_BUILT_PAGE`] until a Build brings one.
pub fn stage(req: &PackageRequest, c: &Checked) -> Result<Staged, String> {
    let root = req.staging_dir.clone();
    let app_dir = root.join("src-tauri");
    if req.bundle_dir.join("index.html").is_file() {
        copy_tree(&req.bundle_dir, &root.join("frontend"))?;
    } else if !root.join("frontend/index.html").is_file() {
        write_if_changed(&root.join("frontend/index.html"), NOT_BUILT_PAGE.as_bytes())?;
    }
    write_if_changed(&root.join("shell/Cargo.toml"), SHELL_MANIFEST.as_bytes())?;
    write_if_changed(&root.join("shell/src/lib.rs"), SHELL_LIB.as_bytes())?;
    let crate_name = req.crate_name();
    write_if_changed(&app_dir.join("Cargo.toml"), app_manifest(&crate_name, &c.features).as_bytes())?;
    if !app_dir.join("Cargo.lock").exists() {
        write_if_changed(&app_dir.join("Cargo.lock"), SHELL_LOCK.as_bytes())?;
    }
    write_if_changed(&app_dir.join("build.rs"), app_build_script(&c.features).as_bytes())?;
    write_if_changed(&app_dir.join("src/lib.rs"), app_entry(&c.app_id, req.display_name(), req.settings.style).as_bytes())?;
    let pretty = |v: &Value| serde_json::to_string_pretty(v).unwrap_or_default() + "\n";
    write_if_changed(&app_dir.join("tauri.conf.json"), pretty(&tauri_conf(req, c)).as_bytes())?;
    write_if_changed(&app_dir.join("capabilities/default.json"), pretty(&capability(&c.features)).as_bytes())?;
    if req.settings.platform == MobilePlatform::Ios {
        write_if_changed(&app_dir.join("Info.ios.plist"), ios_info_plist(req).as_bytes())?;
    }
    let icon = root.join("icon.png");
    write_if_changed(&icon, &c.icon)?;
    Ok(Staged { android_dir: app_dir.join("gen/android"), apple_dir: app_dir.join("gen/apple"), app_dir, icon, root })
}

/// The edits the generated Xcode project needs, made after `ios init`: the name under the icon in
/// each language `[mobile.app_name]` gives is an `InfoPlist.strings` in the target's folder, which
/// XcodeGen turns into the project's localised resources. `Ok(true)`: a language was added, changed
/// or removed, so the project has to be generated again ([`build_ios`] runs `xcodegen generate`).
pub fn patch_ios_project(staged: &Staged, req: &PackageRequest) -> Result<bool, String> {
    let target = staged.apple_dir.join(format!("{}_iOS", req.crate_name()));
    let names = ios_names(req);
    let mut changed = false;
    for lang in ["es", "pt", "fr", "ja", "zh"] {
        let Some(lproj) = ios_lproj(lang) else { continue };
        let dir = target.join(format!("{lproj}.lproj"));
        match names.iter().find(|(l, _)| *l == lproj) {
            Some((_, name)) => {
                let text = format!("/* Generated by PowerRustCOBOL AI from [mobile.app_name]. */\n\"CFBundleDisplayName\" = {};\n", ios_strings_value(name));
                changed |= write_changed(&dir.join("InfoPlist.strings"), text.as_bytes())?;
            }
            None if dir.exists() => {
                std::fs::remove_dir_all(&dir).map_err(|e| format!("{} cannot be removed ({e})", dir.display()))?;
                changed = true;
            }
            None => {}
        }
    }
    Ok(changed)
}

/// The edits the generated Gradle project needs, made after `android init`: the orientation, the
/// release signing, and the name under the icon in each language `[mobile.app_name]` gives.
pub fn patch_android_project(staged: &Staged, req: &PackageRequest) -> Result<(), String> {
    let manifest = staged.android_dir.join("app/src/main/AndroidManifest.xml");
    let text = std::fs::read_to_string(&manifest).map_err(|e| format!("{} cannot be read ({e})", manifest.display()))?;
    write_if_changed(&manifest, patch_orientation(&text, req.settings.orientation).as_bytes())?;
    let gradle = staged.android_dir.join("app/build.gradle.kts");
    let text = std::fs::read_to_string(&gradle).map_err(|e| format!("{} cannot be read ({e})", gradle.display()))?;
    write_if_changed(&gradle, patch_signing(&text).as_bytes())?;
    for (lang, name) in &req.settings.app_name {
        let Some(dir) = android_values_dir(lang) else { continue };
        if name.trim().is_empty() {
            continue;
        }
        let n = android_string(name.trim());
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!-- Generated by PowerRustCOBOL AI from [mobile.app_name]. -->\n<resources>\n    <string name=\"app_name\">{n}</string>\n    <string name=\"main_activity_title\">{n}</string>\n</resources>\n"
        );
        write_if_changed(&staged.android_dir.join("app/src/main/res").join(dir).join("strings.xml"), xml.as_bytes())?;
    }
    Ok(())
}

/// The Android tools a package build needs, found on this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toolchain {
    pub android_home: PathBuf,
    pub ndk_home: PathBuf,
    pub java_home: PathBuf,
}

/// A tool a package build needs and this computer lacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    /// Its short name (`tauri-cli 2.12.4`, `the Android NDK`), for a sentence that names it.
    pub what: String,
    /// What is wrong and how to get it, a sentence of its own.
    pub how: String,
}

impl Toolchain {
    /// Find the Android SDK, its newest NDK and a Java 17 to 21, and check the Tauri command line
    /// and the Rust target. `Err` lists what is missing, each with how to get it.
    pub fn detect(abi: AndroidAbi) -> Result<Self, Vec<Missing>> {
        let mut missing = Vec::new();
        let mut lack = |what: &str, how: String| missing.push(Missing { what: what.to_owned(), how });
        if let Some(m) = tauri_cli_problem() {
            lack(&m.what, m.how);
        }
        if let Some(m) = rust_target_problem(abi.rust_target()) {
            lack(&m.what, m.how);
        }
        let android_home = android_sdk();
        let ndk_home = android_home.as_deref().and_then(ndk_of);
        let java_home = java_home();
        match &android_home {
            None => lack(
                "the Android SDK",
                "the Android SDK was not found: install Android Studio (its SDK Manager installs the SDK) or set ANDROID_HOME".to_owned(),
            ),
            Some(_) if ndk_home.is_none() => lack(
                "the Android NDK",
                "the Android NDK was not found: install it from Android Studio, SDK Manager, SDK Tools, NDK (Side by side), or set NDK_HOME".to_owned(),
            ),
            Some(_) => {}
        }
        if java_home.is_none() {
            lack("a Java 17 to 21", "a Java Development Kit 17 to 21 was not found: install one (Temurin 21) or set JAVA_HOME to it".to_owned());
        }
        match (android_home, ndk_home, java_home) {
            (Some(android_home), Some(ndk_home), Some(java_home)) if missing.is_empty() => Ok(Self { android_home, ndk_home, java_home }),
            _ => Err(missing),
        }
    }

    /// The environment of a build process.
    pub fn env(&self) -> Vec<(&'static str, PathBuf)> {
        vec![("ANDROID_HOME", self.android_home.clone()), ("NDK_HOME", self.ndk_home.clone()), ("JAVA_HOME", self.java_home.clone())]
    }

    /// `adb` of the SDK.
    pub fn adb(&self) -> PathBuf {
        self.android_home.join("platform-tools").join(if cfg!(windows) { "adb.exe" } else { "adb" })
    }
}

/// The Tauri command line at the version this release builds with: what is wrong with it, or `None`.
fn tauri_cli_problem() -> Option<Missing> {
    let what = format!("tauri-cli {TAURI_CLI_VERSION}");
    let how = match tauri_cli_version() {
        Some(v) if v == TAURI_CLI_VERSION => return None,
        Some(v) => format!("the Tauri command line is {v}; this release builds with {TAURI_CLI_VERSION}: {INSTALL_TAURI_CLI}"),
        None => format!("the Tauri command line is not installed: {INSTALL_TAURI_CLI}"),
    };
    Some(Missing { what, how })
}

/// `target` of the Rust compiler, when `rustup` says it is not installed (it is not asked when
/// `rustup` is not there).
fn rust_target_problem(target: &str) -> Option<Missing> {
    let installed = rust_targets()?;
    (!installed.iter().any(|t| t == target)).then(|| Missing {
        what: format!("the Rust target {target}"),
        how: format!("the Rust target {target} is not installed: rustup target add {target}"),
    })
}

/// The installed Tauri command line's version (`cargo tauri --version` says `tauri-cli 2.12.4`).
pub fn tauri_cli_version() -> Option<String> {
    let out = Command::new("cargo").args(["tauri", "--version"]).output().ok()?;
    parse_tauri_cli_version(&String::from_utf8_lossy(&out.stdout))
}

/// The version in `tauri-cli 2.12.4`.
pub fn parse_tauri_cli_version(text: &str) -> Option<String> {
    text.lines().find_map(|l| l.trim().strip_prefix("tauri-cli ").map(|v| v.trim().to_owned()))
}

fn rust_targets() -> Option<Vec<String>> {
    let out = Command::new("rustup").args(["target", "list", "--installed"]).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).lines().map(|l| l.trim().to_owned()).collect())
}

/// The Android SDK: `ANDROID_HOME`, `ANDROID_SDK_ROOT`, or where Android Studio puts it.
pub fn android_sdk() -> Option<PathBuf> {
    let from_env = ["ANDROID_HOME", "ANDROID_SDK_ROOT"].iter().filter_map(|k| std::env::var_os(k)).map(PathBuf::from);
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    let usual = home.into_iter().flat_map(|h| {
        [h.join("Library/Android/sdk"), h.join("Android/Sdk"), h.join("AppData/Local/Android/Sdk")]
    });
    from_env.chain(usual).find(|p| p.join("platform-tools").is_dir() || p.join("platforms").is_dir())
}

/// The NDK: `NDK_HOME` when it exists, else the newest one the SDK holds.
fn ndk_of(sdk: &Path) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("NDK_HOME").map(PathBuf::from).filter(|p| p.is_dir()) {
        return Some(p);
    }
    let names: Vec<String> = std::fs::read_dir(sdk.join("ndk")).ok()?.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    newest_version(&names).map(|n| sdk.join("ndk").join(n))
}

/// The highest of dotted version names (`29.0.14206865` above `27.1.12297006`).
pub fn newest_version(names: &[String]) -> Option<String> {
    let key = |n: &str| -> Option<Vec<u64>> { n.split('.').map(|p| p.parse::<u64>().ok()).collect() };
    names.iter().filter_map(|n| key(n).map(|k| (k, n))).max().map(|(_, n)| n.clone())
}

/// The major version in a JDK's `release` file (`JAVA_VERSION="21.0.5"` → 21; `"1.8.0_402"` → 8).
pub fn parse_java_release(text: &str) -> Option<u32> {
    let line = text.lines().find_map(|l| l.trim().strip_prefix("JAVA_VERSION="))?;
    let v = line.trim().trim_matches('"');
    let mut parts = v.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

/// The Java versions the Android Gradle plugin of Tauri 2.12 runs on.
const JAVA_RANGE: std::ops::RangeInclusive<u32> = 17..=21;

fn java_ok(home: &Path) -> bool {
    std::fs::read_to_string(home.join("release")).ok().and_then(|t| parse_java_release(&t)).is_some_and(|v| JAVA_RANGE.contains(&v))
}

/// A Java 17 to 21: `JAVA_HOME`, the system's choice (macOS `java_home`), or Android Studio's own.
fn java_home() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("JAVA_HOME").map(PathBuf::from).filter(|p| java_ok(p)) {
        return Some(p);
    }
    if cfg!(target_os = "macos") {
        for v in ["21", "17"] {
            if let Ok(out) = Command::new("/usr/libexec/java_home").args(["-v", v]).output() {
                let p = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
                if out.status.success() && java_ok(&p) {
                    return Some(p);
                }
            }
        }
    }
    let studio = [
        PathBuf::from("/Applications/Android Studio.app/Contents/jbr/Contents/Home"),
        PathBuf::from("/opt/android-studio/jbr"),
        PathBuf::from("C:/Program Files/Android/Android Studio/jbr"),
    ];
    studio.into_iter().find(|p| java_ok(p))
}

/// What an iOS package build asks of this computer, as found (no judgement yet): [`ios_problems`]
/// says what is wrong with it.
#[derive(Debug, Clone, Default)]
pub struct IosFacts {
    pub macos: bool,
    pub apple_silicon: bool,
    /// The major version of the selected Xcode (`xcodebuild -version`).
    pub xcode: Option<u32>,
    pub xcodegen: Option<PathBuf>,
    /// `idevicesyslog`, of libimobiledevice: the Tauri command line checks for it.
    pub idevicesyslog: Option<PathBuf>,
    /// CocoaPods' `pod`, which the Tauri command line checks for too.
    pub pod: Option<PathBuf>,
    pub tauri_cli: Option<Missing>,
    pub rust_target: Option<Missing>,
}

/// The oldest Xcode that has the iOS 18 SDK, the floor of 2.x (D-MinOS).
const XCODE_FLOOR: u32 = 16;

/// Where Homebrew puts its tools; a program started from the Dock has neither on its `PATH`.
const HOMEBREW_DIRS: [&str; 2] = ["/opt/homebrew/bin", "/usr/local/bin"];

/// What is wrong with `facts`, each with how to get it. Xcode is never installed by this program,
/// and neither is anything else: the Tauri command line would install XcodeGen, libimobiledevice
/// and CocoaPods by itself (and CocoaPods with `sudo`), so they are named here first (R2).
pub fn ios_problems(f: &IosFacts) -> Vec<Missing> {
    let mut out = Vec::new();
    let mut lack = |what: &str, how: &str| out.push(Missing { what: what.to_owned(), how: how.to_owned() });
    if !f.macos {
        lack("macOS", "an iOS package can only be built on macOS; the web bundle is built and can be taken there");
        return out;
    }
    if !f.apple_silicon {
        lack("an Apple-silicon Mac", "this release builds the iOS simulator package (the Rust target aarch64-apple-ios-sim) on an Apple-silicon Mac");
    }
    match f.xcode {
        None => lack(
            "Xcode",
            "Xcode was not found: install Xcode 16 or later from the Mac App Store, open it once to accept its licence, and select it with `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer` (PowerRustCOBOL AI never installs Xcode)",
        ),
        Some(v) if v < XCODE_FLOOR => lack(
            "Xcode 16 or later",
            &format!("the selected Xcode is {v}; this release needs Xcode {XCODE_FLOOR} or later, which has the iOS 18 SDK: update it from the Mac App Store"),
        ),
        Some(_) => {}
    }
    if f.xcodegen.is_none() {
        lack("XcodeGen", "XcodeGen was not found; the Tauri command line makes the Xcode project with it: brew install xcodegen");
    }
    if f.idevicesyslog.is_none() {
        lack(
            "libimobiledevice",
            "libimobiledevice (its idevicesyslog) was not found; the Tauri command line looks for it before an iOS build: brew install libimobiledevice",
        );
    }
    if f.pod.is_none() {
        lack("CocoaPods", "CocoaPods was not found; the Tauri command line looks for it before an iOS build: brew install cocoapods");
    }
    out.extend(f.tauri_cli.clone());
    out.extend(f.rust_target.clone());
    out
}

/// The major version in `xcodebuild -version` (`Xcode 27.2` and a build line).
pub fn parse_xcode_version(text: &str) -> Option<u32> {
    let line = text.lines().find_map(|l| l.trim().strip_prefix("Xcode "))?;
    line.split('.').next()?.trim().parse().ok()
}

/// `name` in the first of `dirs` that holds it as a file.
pub fn find_tool(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    dirs.iter().map(|d| d.join(name)).find(|p| p.is_file())
}

/// The folders tools are looked for in: the `PATH`, then Homebrew's.
fn tool_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.extend(HOMEBREW_DIRS.iter().map(PathBuf::from));
    dirs
}

/// The Apple tools an iOS package build runs, found on this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IosToolchain {
    /// The folders of the tools found, for the front of the build's `PATH`.
    pub path_dirs: Vec<PathBuf>,
    /// `xcodegen`, which makes the Xcode project again when a language is added.
    pub xcodegen: PathBuf,
}

impl IosToolchain {
    /// Look at this computer ([`IosFacts`]) and judge it ([`ios_problems`]).
    pub fn detect() -> Result<Self, Vec<Missing>> {
        let macos = cfg!(target_os = "macos");
        let dirs = tool_dirs();
        let facts = IosFacts {
            macos,
            apple_silicon: std::env::consts::ARCH == "aarch64",
            xcode: macos
                .then(|| Command::new("xcodebuild").arg("-version").output().ok())
                .flatten()
                .filter(|o| o.status.success())
                .and_then(|o| parse_xcode_version(&String::from_utf8_lossy(&o.stdout))),
            xcodegen: find_tool("xcodegen", &dirs),
            idevicesyslog: find_tool("idevicesyslog", &dirs),
            pod: find_tool("pod", &dirs),
            tauri_cli: tauri_cli_problem(),
            rust_target: rust_target_problem(IOS_RUST_TARGET),
        };
        let problems = ios_problems(&facts);
        let (Some(xcodegen), true) = (facts.xcodegen.clone(), problems.is_empty()) else {
            return Err(problems);
        };
        let mut path_dirs: Vec<PathBuf> = Vec::new();
        for tool in [&facts.xcodegen, &facts.idevicesyslog, &facts.pod].into_iter().flatten() {
            if let Some(d) = tool.parent().map(Path::to_path_buf).filter(|d| !path_dirs.contains(d)) {
                path_dirs.push(d);
            }
        }
        Ok(Self { path_dirs, xcodegen })
    }

    /// The environment of a build process: the tools' folders in front of the `PATH`, and a UTF-8
    /// locale (CocoaPods refuses to run without one).
    pub fn env(&self) -> Vec<(&'static str, OsString)> {
        let mut paths = self.path_dirs.clone();
        if let Some(p) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&p));
        }
        let path = std::env::join_paths(paths).unwrap_or_default();
        vec![("PATH", path), ("LANG", OsString::from("en_US.UTF-8"))]
    }
}

/// A built package.
#[derive(Debug, Clone)]
pub struct Package {
    /// The package in the project (`dist/mobile/android/<Name>-debug.apk`).
    pub path: PathBuf,
    pub bytes: u64,
    pub elapsed_ms: u128,
    /// The device features it was built with.
    pub features: Vec<&'static str>,
    /// What to check before publishing it (the features' store notes).
    pub checklist: Vec<String>,
}

/// Run `program args` in `dir` with the toolchain's environment (and `extra`), every line of its
/// output to `log`. `Err` carries the last lines when it fails.
pub(crate) fn run_logged(dir: &Path, args: &[&str], tools: &Toolchain, extra: &[(&str, String)], log: &mut dyn FnMut(&str)) -> Result<(), String> {
    let env: Vec<(&str, OsString)> = tools.env().into_iter().map(|(k, v)| (k, v.into_os_string())).collect();
    run_program("cargo", dir, args, &env, extra, log)
}

/// [`run_logged`] for any program, with the environment `env` and `extra`.
pub(crate) fn run_program(
    program: &str,
    dir: &Path,
    args: &[&str],
    env: &[(&str, OsString)],
    extra: &[(&str, String)],
    log: &mut dyn FnMut(&str),
) -> Result<(), String> {
    log(&format!("$ {program} {}", args.join(" ")));
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(dir).stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
    for (k, v) in env {
        cmd.env(k, v);
    }
    for (k, v) in extra {
        cmd.env(k, v);
    }
    // A CARGO_TARGET_DIR of the developer's shell would send the build elsewhere; the staging
    // folder keeps its own.
    cmd.env_remove("CARGO_TARGET_DIR");
    let mut child = cmd.spawn().map_err(|e| format!("{program} could not be started ({e}){}", if program == "cargo" { "; is Rust installed?" } else { "" }))?;
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let mut readers = Vec::new();
    if let Some(out) = child.stdout.take() {
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || {
            for l in BufReader::new(out).lines().map_while(Result::ok) {
                let _ = tx.send(l);
            }
        }));
    }
    if let Some(err) = child.stderr.take() {
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || {
            for l in BufReader::new(err).lines().map_while(Result::ok) {
                let _ = tx.send(l);
            }
        }));
    }
    drop(tx);
    let mut tail: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    for line in rx {
        log(&line);
        if tail.len() == 40 {
            tail.pop_front();
        }
        tail.push_back(line);
    }
    for r in readers {
        let _ = r.join();
    }
    let status = child.wait().map_err(|e| format!("{program} {} did not finish ({e})", args.join(" ")))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} {} failed ({status}):\n{}", args.join(" "), Vec::from(tail).join("\n")))
    }
}

/// Build the Android package of `req`: check, stage, run the Tauri command line, copy the
/// package into the project. `log` receives every line of the tools' output.
pub fn build_android(req: &PackageRequest, tools: &Toolchain, signing: Option<&SigningSecrets>, log: &mut dyn FnMut(&str)) -> Result<Package, String> {
    if req.settings.platform != MobilePlatform::Android {
        return Err("the project's platform is iOS; an Android package is built for an Android project".to_owned());
    }
    let started = Instant::now();
    let checked = check(req, signing).map_err(|p| p.join("\n"))?;
    log(&format!("Staging the mobile shell in {}", req.staging_dir.display()));
    let staged = stage(req, &checked)?;
    if !staged.android_dir.join("app/build.gradle.kts").is_file() {
        run_logged(&staged.app_dir, &["tauri", "android", "init", "--ci", "--skip-targets-install"], tools, &[], log)?;
    }
    patch_android_project(&staged, req)?;
    // The project's own Android Studio project, when it has one, takes the same bundle and sources
    // (WASM-012 R16, R18); a failure there is said and does not stop the package.
    match crate::mobile_native::refresh_android(req, &checked, tools) {
        Ok(true) => log(&format!("Android Studio project brought up to date: {}", crate::mobile_native::android_studio_dir(&req.project_dir).display())),
        Ok(false) => {}
        Err(why) => log(&format!("The Android Studio project could not be brought up to date: {why}")),
    }
    let icon = staged.icon.to_string_lossy().into_owned();
    run_logged(&staged.app_dir, &["tauri", "icon", &icon], tools, &[], log)?;
    let mut args = vec!["tauri", "android", "build", "--ci", "--apk", "--target", req.abi.tauri_target()];
    if req.profile == Profile::Debug {
        args.insert(3, "--debug");
    }
    let mut extra: Vec<(&str, String)> = Vec::new();
    if req.profile == Profile::Release {
        if let Some(sec) = signing {
            let sg = &req.settings.signing;
            extra.push((ENV_KEYSTORE, sg.android_keystore.trim().to_owned()));
            extra.push((ENV_KEY_ALIAS, sg.android_alias.trim().to_owned()));
            extra.push((ENV_STORE_PASSWORD, sec.store_password.clone()));
            extra.push((ENV_KEY_PASSWORD, sec.key_password.clone()));
        }
    }
    // Gradle updates a debug package in place and keeps the room of every entry it replaced (a
    // package that once held a larger native library stays that large), and a build that fails
    // would leave the last one to be found: every build starts from no package.
    while let Some(old) = built_apk(&staged.android_dir, req.profile) {
        std::fs::remove_file(&old).map_err(|e| format!("the previous package {} cannot be removed ({e})", old.display()))?;
    }
    run_logged(&staged.app_dir, &args, tools, &extra, log)?;
    let built = built_apk(&staged.android_dir, req.profile)
        .ok_or_else(|| format!("the build finished but no package was found under {}", staged.android_dir.join("app/build/outputs/apk").display()))?;
    let dest = req.package_path();
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{} cannot be created ({e})", dir.display()))?;
    }
    std::fs::copy(&built, &dest).map_err(|e| format!("the package cannot be copied to {} ({e})", dest.display()))?;
    let bytes = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
    let names: Vec<String> = checked.features.iter().map(|f| f.name.to_owned()).collect();
    log(&format!("Package: {} ({:.1} MB)", dest.display(), bytes as f64 / 1_048_576.0));
    Ok(Package {
        path: dest,
        bytes,
        elapsed_ms: started.elapsed().as_millis(),
        features: checked.features.iter().map(|f| f.name).collect(),
        checklist: mobile_features::checklist(&names),
    })
}

/// The package Gradle wrote: the universal one of the profile (`app-universal-debug.apk`,
/// `app-universal-release.apk`, or `-release-unsigned` without signing).
pub fn built_apk(android_dir: &Path, profile: Profile) -> Option<PathBuf> {
    let dir = android_dir.join("app/build/outputs/apk/universal").join(profile.as_str());
    let names = [format!("app-universal-{}.apk", profile.as_str()), format!("app-universal-{}-unsigned.apk", profile.as_str())];
    names.iter().map(|n| dir.join(n)).find(|p| p.is_file())
}

/// The Xcode project Tauri generated is the project's: it carries its application id, its product
/// name, its target and the oldest iOS it was made for. A project made for another identity or
/// another floor is generated again, from nothing (the folder is the build's, never the developer's).
pub fn apple_project_current(staged: &Staged, req: &PackageRequest, c: &Checked) -> bool {
    let Ok(yml) = std::fs::read_to_string(staged.apple_dir.join("project.yml")) else { return false };
    [
        format!("PRODUCT_BUNDLE_IDENTIFIER: {}", c.app_id),
        format!("PRODUCT_NAME: {}", file_stem(&req.project_name)),
        format!("{}_iOS:", req.crate_name()),
        format!("iOS: {}", c.ios_min_version),
    ]
    .iter()
    .all(|line| yml.lines().any(|l| l.trim() == line))
}

/// The size of every file under `dir`.
pub fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|e| {
            let p = e.path();
            if p.is_dir() {
                dir_size(&p)
            } else {
                e.metadata().map(|m| m.len()).unwrap_or(0)
            }
        })
        .sum()
}

/// Build the iOS package of `req` — the application the iOS simulator runs: check, stage, run the
/// Tauri command line and Xcode, copy the application into the project. `log` receives every line
/// of the tools' output. A release archive is refused by [`check`].
pub fn build_ios(req: &PackageRequest, tools: &IosToolchain, log: &mut dyn FnMut(&str)) -> Result<Package, String> {
    if req.settings.platform != MobilePlatform::Ios {
        return Err("the project's platform is Android; an iOS package is built for an iOS project".to_owned());
    }
    let started = Instant::now();
    let checked = check(req, None).map_err(|p| p.join("\n"))?;
    log(&format!("Staging the mobile shell in {}", req.staging_dir.display()));
    let staged = stage(req, &checked)?;
    let env = tools.env();
    let cargo = |args: &[&str], log: &mut dyn FnMut(&str)| run_program("cargo", &staged.app_dir, args, &env, &[], log);
    if !apple_project_current(&staged, req, &checked) {
        if staged.apple_dir.exists() {
            std::fs::remove_dir_all(&staged.apple_dir).map_err(|e| format!("{} cannot be emptied ({e})", staged.apple_dir.display()))?;
        }
        cargo(&["tauri", "ios", "init", "--ci", "--skip-targets-install"], log)?;
    }
    if patch_ios_project(&staged, req)? {
        run_program(&tools.xcodegen.to_string_lossy(), &staged.apple_dir, &["generate"], &env, &[], log)?;
    }
    let icon = staged.icon.to_string_lossy().into_owned();
    cargo(&["tauri", "icon", &icon], log)?;
    // A build that fails must not leave the last application to be found.
    while let Some(old) = built_ios_app(&staged.apple_dir) {
        std::fs::remove_dir_all(&old).map_err(|e| format!("the previous application {} cannot be removed ({e})", old.display()))?;
    }
    cargo(&["tauri", "ios", "build", "--debug", "--target", IOS_TAURI_TARGET, "--ci"], log)?;
    let built = built_ios_app(&staged.apple_dir)
        .ok_or_else(|| format!("the build finished but no application was found under {}", staged.apple_dir.join("build").join(IOS_BUILD_SUBDIR).display()))?;
    let dest = req.package_path();
    copy_tree(&built, &dest)?;
    let bytes = dir_size(&dest);
    let names: Vec<String> = checked.features.iter().map(|f| f.name.to_owned()).collect();
    log(&format!("Package: {} ({:.1} MB)", dest.display(), bytes as f64 / 1_048_576.0));
    Ok(Package {
        path: dest,
        bytes,
        elapsed_ms: started.elapsed().as_millis(),
        features: checked.features.iter().map(|f| f.name).collect(),
        checklist: mobile_features::checklist(&names),
    })
}

/// The application Xcode wrote for the simulator (`gen/apple/build/arm64-sim/<Name>.app`).
pub fn built_ios_app(apple_dir: &Path) -> Option<PathBuf> {
    let dir = apple_dir.join("build").join(IOS_BUILD_SUBDIR);
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| p.is_dir() && p.extension().is_some_and(|x| x == "app"))
}
