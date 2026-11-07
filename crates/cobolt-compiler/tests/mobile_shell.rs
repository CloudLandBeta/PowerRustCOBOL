// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The mobile package's staging (WASM-012 T11–T13): what a Mobile project's `[mobile]` becomes in
//! the staged shell, what is refused before any tool runs, and the two edits made to the Gradle
//! project Tauri generates. The tools themselves (Tauri, Gradle, the SDK) are exercised by the
//! package build on a computer that has them (plan §9, T24); nothing here needs them.

use std::path::{Path, PathBuf};
use std::time::Instant;

use cobolt_compiler::mobile_build::{self as mb, AndroidAbi, PackageRequest, Profile, SigningSecrets};
use cobolt_compiler::project_kind::{ApplicationStyle, DeviceClass, MobilePlatform, MobileSettings, Orientation};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("prc-mobile-shell-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A project folder with a built Web bundle whose manifest lists `builtins`.
fn project(name: &str, builtins: &[&str]) -> PathBuf {
    let dir = scratch(name);
    let web = dir.join("dist/web");
    std::fs::create_dir_all(web.join("player")).unwrap();
    std::fs::write(web.join("index.html"), "<!doctype html><title>t</title>").unwrap();
    std::fs::write(web.join("player/player.js"), "// player").unwrap();
    let manifest = serde_json::json!({ "abi": 1, "builtins": builtins });
    std::fs::write(web.join("manifest.0123abcd.json"), manifest.to_string()).unwrap();
    dir
}

fn request(dir: &Path, settings: MobileSettings, profile: Profile) -> PackageRequest {
    PackageRequest {
        project_dir: dir.to_path_buf(),
        project_name: "PowerMobileHello".into(),
        version: "1.2.3".into(),
        settings,
        bundle_dir: dir.join("dist/web"),
        staging_dir: dir.join("staging"),
        profile,
        abi: AndroidAbi::Aarch64,
    }
}

fn settings(features: &[&str]) -> MobileSettings {
    let mut s = MobileSettings::new(MobilePlatform::Android, DeviceClass::Phone);
    s.app_id = "com.example.power_mobile".into();
    s.features = features.iter().map(|f| (*f).to_owned()).collect();
    s
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn json(p: &Path) -> serde_json::Value {
    serde_json::from_str(&read(p)).unwrap()
}

#[test]
fn staging_writes_the_shell_and_an_application_made_from_the_mobile_settings() {
    let t = Instant::now();
    let dir = project("stage", &["SECURESTORE", "SECURELOAD"]);
    let mut s = settings(&["secure-storage", "deep-link"]);
    s.app_name.insert("en".into(), "Hello".into());
    let req = request(&dir, s, Profile::Debug);
    let checked = mb::check(&req, None).expect("a complete project passes");
    assert_eq!((checked.build_number, checked.android_min_sdk), (10203, 35), "1.2.3 is build 10203; auto is API 35");
    let staged = mb::stage(&req, &checked).unwrap();

    // The bundle, the shell's own sources, the lock it was checked with.
    assert!(staged.root.join("frontend/index.html").is_file() && staged.root.join("frontend/player/player.js").is_file());
    assert_eq!(read(&staged.root.join("shell/src/lib.rs")), read(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../cobolt-mobile-shell/src/lib.rs"))));
    assert!(read(&staged.app_dir.join("Cargo.lock")).contains("name = \"tauri\"\nversion = \"2.12.5\""), "the shell's lock pins Tauri 2.12.5");

    // The application crate.
    let cargo = read(&staged.app_dir.join("Cargo.toml"));
    assert!(cargo.contains("name = \"powermobilehello\"") && cargo.contains("name = \"powermobilehello_lib\""), "{cargo}");
    assert!(cargo.contains("features = [\"secure-storage\", \"deep-link\"]") && cargo.contains("tauri-plugin-deep-link = \"=2.6.3\""), "{cargo}");
    assert!(!cargo.contains("tauri-plugin-opener"), "a feature not declared is not linked");
    assert!(read(&staged.app_dir.join("build.rs")).contains("\"secure_store\", \"secure_load\", \"secure_delete\""));
    let entry = read(&staged.app_dir.join("src/lib.rs"));
    assert!(entry.contains("app_id: \"com.example.power_mobile\", title: \"Hello\", single_page: true"), "{entry}");

    // tauri.conf.json and the capability.
    let conf = json(&staged.app_dir.join("tauri.conf.json"));
    assert_eq!(conf["identifier"], "com.example.power_mobile");
    assert_eq!(conf["productName"], "Hello");
    assert_eq!(conf["version"], "1.2.3");
    assert_eq!(conf["bundle"]["android"]["versionCode"], 10203);
    assert_eq!(conf["bundle"]["android"]["minSdkVersion"], 35);
    assert_eq!(conf["bundle"]["iOS"]["minimumSystemVersion"], "18.0");
    assert_eq!(conf["app"]["windows"], serde_json::json!([]), "the shell opens the one window itself");
    assert_eq!(conf["plugins"]["deep-link"]["mobile"][0]["scheme"][0], "com.example.power-mobile", "a scheme has no _");
    let cap = json(&staged.app_dir.join("capabilities/default.json"));
    let perms: Vec<&str> = cap["permissions"].as_array().unwrap().iter().filter_map(|v| v.as_str()).collect();
    assert_eq!(perms, ["core:default", "allow-secure-store", "allow-secure-load", "allow-secure-delete", "deep-link:default"]);

    // The icon of a project that names none, and a second staging changes nothing it need not.
    assert_eq!(mb::png_size(&std::fs::read(&staged.icon).unwrap()), Some((1024, 1024)));
    let before = std::fs::metadata(staged.app_dir.join("tauri.conf.json")).unwrap().modified().unwrap();
    mb::stage(&req, &checked).unwrap();
    let after = std::fs::metadata(staged.app_dir.join("tauri.conf.json")).unwrap().modified().unwrap();
    assert_eq!(before, after, "an unchanged file keeps its time, so nothing is rebuilt for it");

    // A standalone application with no feature: no plugin, no command, no deep link.
    let mut plain = settings(&[]);
    plain.style = ApplicationStyle::Standalone;
    let dir2 = project("stage-plain", &[]);
    let req2 = request(&dir2, plain, Profile::Debug);
    let c2 = mb::check(&req2, None).unwrap();
    let st2 = mb::stage(&req2, &c2).unwrap();
    assert!(read(&st2.app_dir.join("Cargo.toml")).contains("features = []"));
    assert!(!read(&st2.app_dir.join("build.rs")).contains("secure_store"));
    assert!(read(&st2.app_dir.join("src/lib.rs")).contains("title: \"PowerMobileHello\", single_page: false"));
    assert!(json(&st2.app_dir.join("tauri.conf.json")).get("plugins").is_none());
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
    println!(
        "mobile staging: 2 projects staged (secure-storage + deep-link SPA; standalone with no feature): 9 files each, \
         tauri.conf identifier/version/versionCode/minSdk/scheme, capability of 5 permissions, re-stage left the files untouched; {} ms",
        t.elapsed().as_millis()
    );
}

#[test]
fn every_problem_is_named_before_any_tool_runs() {
    let dir = project("check", &["SECURESTORE"]);
    // A 512 px icon, a version with no build number, no application id, an undeclared feature,
    // an unknown one, and a release with no signing.
    std::fs::create_dir_all(dir.join("Assets")).unwrap();
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    png.extend_from_slice(&512u32.to_be_bytes());
    png.extend_from_slice(&512u32.to_be_bytes());
    std::fs::write(dir.join("Assets/icon.png"), &png).unwrap();
    let mut s = settings(&["camera"]);
    s.app_id = String::new();
    s.icon = "Assets/icon.png".into();
    let mut req = request(&dir, s, Profile::Release);
    req.version = "1.0".into();
    let problems = mb::check(&req, None).unwrap_err();
    let all = problems.join("\n");
    for expect in ["application id is not set", "cannot give a build number", "names 'camera', which is not a device feature", "uses the device feature 'secure-storage'", "512 x 512 px", "keystore and the key's alias", "passwords"] {
        assert!(all.contains(expect), "missing «{expect}» in:\n{all}");
    }

    // A keystore inside the project is refused even with its passwords.
    let mut s = settings(&["secure-storage"]);
    s.signing.android_keystore = dir.join("release.jks").to_string_lossy().into_owned();
    s.signing.android_alias = "upload".into();
    let req = request(&dir, s, Profile::Release);
    let secrets = SigningSecrets { store_password: "fake-store-pw".into(), key_password: "fake-key-pw".into() };
    let problems = mb::check(&req, Some(&secrets)).unwrap_err();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("inside the project folder"));
    assert_eq!(format!("{secrets:?}"), "SigningSecrets { .. }", "a password is never printed");

    // No bundle: build the project first.
    let empty = scratch("check-empty");
    let problems = mb::check(&request(&empty, settings(&[]), Profile::Debug), None).unwrap_err();
    assert!(problems.iter().any(|p| p.contains("build the project first")), "{problems:?}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&empty);
    println!("mobile check: 7 problems named at once (id, version, unknown and undeclared feature, icon size, signing names, passwords); keystore inside the project refused; missing bundle refused");
}

const MANIFEST: &str = r#"<manifest>
    <application>
        <activity
            android:configChanges="orientation|keyboardHidden"
            android:launchMode="singleTask"
            android:name=".MainActivity"
            android:exported="true">
        </activity>
    </application>
</manifest>"#;

#[test]
fn the_orientation_is_set_replaced_and_removed_in_the_android_manifest() {
    let portrait = mb::patch_orientation(MANIFEST, Orientation::Portrait);
    assert!(portrait.contains("android:name=\".MainActivity\"\n            android:screenOrientation=\"portrait\"\n            android:exported"), "{portrait}");
    let landscape = mb::patch_orientation(&portrait, Orientation::Landscape);
    assert_eq!(landscape.matches("screenOrientation").count(), 1);
    assert!(landscape.contains("screenOrientation=\"landscape\""));
    assert_eq!(mb::patch_orientation(&landscape, Orientation::Auto), MANIFEST, "auto leaves the activity as Tauri wrote it");
    println!("orientation: portrait set, landscape replaced it (one attribute), auto removed it back to the generated manifest");
}

#[test]
fn the_release_signing_comes_from_the_environment_and_is_added_once() {
    let gradle = "android {\n    compileSdk = 37\n    buildTypes {\n        getByName(\"release\") {\n            optimization {\n            }\n        }\n    }\n}\n";
    let patched = mb::patch_signing(gradle);
    for expect in ["System.getenv(\"PRC_ANDROID_KEYSTORE\")", "System.getenv(\"PRC_ANDROID_STORE_PASSWORD\")", "System.getenv(\"PRC_ANDROID_KEY_PASSWORD\")", "signingConfig = signingConfigs.getByName(\"prcRelease\")"] {
        assert!(patched.contains(expect), "{expect} in\n{patched}");
    }
    assert_eq!(mb::patch_signing(&patched), patched, "applied once");
    println!("signing: 4 environment reads added to the generated Gradle script, the release type signs with them, a second pass changes nothing");
}

#[test]
fn the_toolchain_answers_are_read_from_what_the_tools_print() {
    assert_eq!(mb::parse_tauri_cli_version("tauri-cli 2.12.4\n").as_deref(), Some("2.12.4"));
    assert_eq!(mb::parse_tauri_cli_version("error: no such command: `tauri`"), None);
    assert_eq!(mb::parse_java_release("IMPLEMENTOR=\"Oracle\"\nJAVA_VERSION=\"21.0.5\"\n"), Some(21));
    assert_eq!(mb::parse_java_release("JAVA_VERSION=\"1.8.0_402\""), Some(8));
    assert_eq!(mb::parse_java_release("JAVA_VERSION=\"25\""), Some(25));
    let ndks = ["27.1.12297006", "29.0.14206865", "26.3.11579264", "README"].map(String::from);
    assert_eq!(mb::newest_version(&ndks).as_deref(), Some("29.0.14206865"));
    assert_eq!(AndroidAbi::from_device_abi("arm64-v8a"), Some(AndroidAbi::Aarch64));
    assert_eq!(AndroidAbi::from_device_abi("x86_64").map(AndroidAbi::rust_target), Some("x86_64-linux-android"));
    assert_eq!(mb::crate_name("PowerMobileHello"), "powermobilehello");
    assert_eq!(mb::crate_name("2-Orders App"), "app2ordersapp");
    assert_eq!(mb::application_name("PowerMobileHello.project"), "PowerMobileHello");
    assert_eq!(mb::android_string("Tom's \"A&B\" <1>"), "Tom\\'s \\\"A&amp;B\\\" &lt;1&gt;");
    let dir = scratch("names");
    let req = request(&dir, settings(&[]), Profile::Release);
    assert!(req.package_path().ends_with("dist/mobile/android/PowerMobileHello-release.apk"));
    let _ = std::fs::remove_dir_all(&dir);
    println!("toolchain parsing: tauri-cli version (2 forms), Java release (3 forms), newest NDK of 4 names, 2 device ABIs, 2 crate names, app name, Android string escaping, package path");
}

/// `adb devices -l` and `adb logcat -v brief`, recorded on the Android emulator of the V0 record
/// (Medium_Phone_API_37.0, 2026-11-06).
#[test]
fn the_devices_and_the_application_log_are_read_from_adb() {
    use cobolt_compiler::mobile_devices::{self as md, Device};
    let listed = "* daemon started successfully\nList of devices attached\nemulator-5554          device product:sdk_gphone64_arm64 model:sdk_gphone64_arm64 device:emu64a transport_id:2\nR58N12ABC   unauthorized usb:1-1 transport_id:3\n\n";
    let devices = md::parse_devices(listed);
    assert_eq!(
        devices,
        [
            Device { serial: "emulator-5554".into(), state: "device".into(), model: "sdk_gphone64_arm64".into() },
            Device { serial: "R58N12ABC".into(), state: "unauthorized".into(), model: String::new() },
        ]
    );
    assert!(devices[0].ready() && !devices[1].ready(), "a phone that has not accepted this computer is not used");
    assert_eq!(devices[0].label(), "sdk gphone64 arm64 (emulator-5554)");
    assert!(md::parse_devices("List of devices attached\n\n").is_empty());
    // Each line of `adb logcat -v brief`, and what the IDE's Output shows of it (None: not shown).
    let log = [
        ("I/RustStdoutStderr( 3581): [WARNING:net/dns/dns_config_service_android.cc:69] Failed to read DnsConfig.", None),
        (
            "I/RustStdoutStderr( 3581): PowerRustCOBOL mobile shell: the deep link could not open its form: x",
            Some("PowerRustCOBOL mobile shell: the deep link could not open its form: x"),
        ),
        (
            "E/Tauri/Console( 3581): File:  - Line 1 - Msg: Applying inline style violates the following Content Security Policy directive",
            Some("Applying inline style violates the following Content Security Policy directive"),
        ),
        (
            "I/Tauri/Console(14101): File: http://tauri.localhost/player/plain-51ca6bec/player.js - Line 1350 - Msg: PowerMobileHello: Hello, Ada. I will remember you.",
            Some("PowerMobileHello: Hello, Ada. I will remember you."),
        ),
        ("I/Tauri/Console(14101): the second line of a message", Some("the second line of a message")),
        ("E/AndroidRuntime( 3581): FATAL EXCEPTION: main", Some("E/AndroidRuntime( 3581): FATAL EXCEPTION: main")),
        ("I/ActivityManager(  612): Start proc 3581:com.example.powermobilehello/u0a210", None),
    ];
    for (line, shown) in log {
        assert_eq!(md::is_application_log(line), shown.is_some(), "{line}");
        assert_eq!(md::application_log(line), shown, "{line}");
    }
    // The application is started as a tap on its icon starts it: the launcher's intent, Tauri's activity.
    assert_eq!(
        md::launch_args("com.example.hello").join(" "),
        "shell am start -a android.intent.action.MAIN -c android.intent.category.LAUNCHER -n com.example.hello/.MainActivity"
    );
    println!(
        "adb parsing: 2 devices of 4 lines (ready, unauthorized), model label, empty list; 7 log lines told apart (web view noise, shell, console error, \
         a DISPLAY, a continuation line, crash, system) and each shown as the program wrote it"
    );
}

/// The emulators a Run can start, from what `emulator -list-avds`, `adb emu avd name` and
/// `getprop sys.boot_completed` printed on this Mac (2026-11-06).
#[test]
fn the_emulators_that_can_be_started_are_read_from_the_sdk() {
    use cobolt_compiler::mobile_devices as md;
    let listed = "Medium_Phone_API_37.0\nPixel_9-Pro.API_35\n";
    assert_eq!(md::parse_avds(listed), ["Medium_Phone_API_37.0", "Pixel_9-Pro.API_35"]);
    // A note the emulator prints about itself is not an emulator.
    assert_eq!(md::parse_avds("INFO    | Storing crashdata in: /tmp/x\nMedium_Phone_API_37.0\n\n"), ["Medium_Phone_API_37.0"]);
    assert!(md::parse_avds("").is_empty());
    assert_eq!(md::parse_avd_name("Medium_Phone_API_37.0\r\nOK\r\n").as_deref(), Some("Medium_Phone_API_37.0"));
    assert_eq!(md::parse_avd_name("OK\r\n"), None);
    assert_eq!(md::parse_avd_name("KO: unknown command\r\n"), None);
    assert!(md::boot_completed("1\n") && !md::boot_completed("") && !md::boot_completed("0\n"));
    // `sys.boot_completed` stayed 1 while Android restarted its system on a starved emulator; the
    // package service did not answer until it was back.
    assert!(md::package_service_found("Service package: found\n"));
    assert!(!md::package_service_found("Service package: not found\n") && !md::package_service_found(""));
    let s = |v: &[&str]| v.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
    // Running devices come first; an emulator already running is not offered a second time.
    let targets = md::targets(&s(&["emulator-5554", "R58N12ABC"]), &s(&["Medium_Phone_API_37.0"]), &s(&["Medium_Phone_API_37.0", "Pixel_9"]));
    assert_eq!(targets, ["emulator-5554", "R58N12ABC", "Pixel_9"]);
    // Nothing running: the emulators that can be started, so a Run starts the first.
    assert_eq!(md::targets(&[], &[], &s(&["Medium_Phone_API_37.0"])), ["Medium_Phone_API_37.0"]);
    assert!(md::targets(&[], &[], &[]).is_empty(), "no device and no emulator: the Preview's browser page");
    assert_eq!(md::emulator_args("Medium_Phone_API_37.0").join(" "), "-avd Medium_Phone_API_37.0");
    println!(
        "emulator parsing: 2 emulators listed, a note line refused, empty list; the running emulator's name of 3 answers; boot done of 3 answers; \
         the package service of 3 answers; \
         targets: 2 running + 1 startable (a running emulator not offered twice), 1 startable alone, none"
    );
}
