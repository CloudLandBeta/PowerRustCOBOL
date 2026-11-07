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
    // Only the application's own processes are followed: its user id from `pm list packages -U`,
    // which also lists the packages whose id merely contains the one asked for.
    let listed = "package:com.powerrustcobol.mobilehello.extra uid:10240\npackage:com.powerrustcobol.mobilehello uid:10233\r\n";
    assert_eq!(md::parse_package_uid(listed, "com.powerrustcobol.mobilehello"), Some(10233));
    assert_eq!(md::parse_package_uid("", "com.powerrustcobol.mobilehello"), None);
    assert_eq!(
        md::logcat_args(Some(10233)).join(" "),
        "logcat -T 1 -v brief --uid=10233 Tauri/Console:V RustStdoutStderr:V AndroidRuntime:E *:S"
    );
    assert!(!md::logcat_args(None).iter().any(|a| a.starts_with("--uid")), "unknown user id: the whole device's tags, as before");
    // A start that answered a failure is asked again only when it was not taken: the activity is
    // listed one second after a start, before `pidof` knows the process.
    let taken = "  Task id #42\n    * Hist  #0: ActivityRecord{109047888 u0 com.powerrustcobol.mobilehello/.MainActivity t42}\n";
    assert!(md::has_activity(taken, "com.powerrustcobol.mobilehello"));
    assert!(!md::has_activity(taken, "com.powerrustcobol.mobile"), "another application's id that is a prefix of it");
    assert!(!md::has_activity("  ResumedActivity: ActivityRecord{1 u0 com.google.android.apps.nexuslauncher/.NexusLauncherActivity t1}\n", "com.powerrustcobol.mobilehello"));
    // The application is started as a tap on its icon starts it: the launcher's intent, Tauri's activity.
    assert_eq!(
        md::launch_args("com.example.hello").join(" "),
        "shell am start -a android.intent.action.MAIN -c android.intent.category.LAUNCHER -n com.example.hello/.MainActivity"
    );
    println!(
        "adb parsing: 2 devices of 4 lines (ready, unauthorized), model label, empty list; 7 log lines told apart (web view noise, shell, console error, \
         a DISPLAY, a continuation line, crash, system) and each shown as the program wrote it; the user id of 2 listed packages and of none; \
         the log arguments with and without it; a taken start told from 2 that were not"
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
    // What `adb devices -l` answered with the emulator running and a phone that has not accepted this computer yet.
    let devices = md::parse_devices(
        "List of devices attached\nemulator-5554          device product:sdk_gphone64_arm64 model:sdk_gphone64_arm64 device:emu64a transport_id:1\n\
         R58N12ABC              device usb:1-1 product:shiba model:Pixel_8 device:shiba transport_id:2\n\
         R3CN90XYZ              unauthorized usb:1-2 transport_id:3\n",
    );
    let running = [("emulator-5554".to_owned(), "Medium_Phone_API_37.0".to_owned())];
    // Running devices come first, an emulator named by its own name; an emulator already running is not offered a
    // second time; a phone that is not ready is not offered.
    let targets = md::targets(&devices, &running, &s(&["Medium_Phone_API_37.0", "Pixel_9"]));
    let shown: Vec<(&str, &str, bool)> = targets.iter().map(|t| (t.id.as_str(), t.label.as_str(), t.running)).collect();
    assert_eq!(
        shown,
        [("emulator-5554", "Medium Phone API 37.0 (emulator-5554)", true), ("R58N12ABC", "Pixel 8 (R58N12ABC)", true), ("Pixel_9", "Pixel 9", false)]
    );
    // A choice made while the emulator was not running still names it once it runs.
    assert!(targets[0].is("emulator-5554") && targets[0].is("Medium_Phone_API_37.0") && !targets[1].is("Medium_Phone_API_37.0"));
    assert!(targets[2].is("Pixel_9") && targets[1].emulator.is_none());
    // Nothing running: the emulators that can be started.
    let idle = md::targets(&[], &[], &s(&["Medium_Phone_API_37.0"]));
    assert_eq!(idle.iter().map(|t| (t.id.as_str(), t.running)).collect::<Vec<_>>(), [("Medium_Phone_API_37.0", false)]);
    assert!(md::targets(&[], &[], &[]).is_empty(), "no device and no emulator: the Preview's browser page");
    assert_eq!(md::emulator_args("Medium_Phone_API_37.0").join(" "), "-avd Medium_Phone_API_37.0");
    println!(
        "emulator parsing: 2 emulators listed, a note line refused, empty list; the running emulator's name of 3 answers; boot done of 3 answers; \
         the package service of 3 answers; \
         targets: 2 running (the emulator by its name) + 1 startable, an unauthorized phone left out, a running emulator not offered twice, \
         a choice by the emulator's name found after it started; 1 startable alone; none"
    );
}

/// The generated Gradle project's two files the package build edits, as `tauri android init` writes them (the parts
/// the edits look at).
fn fake_gradle_project(android: &Path) {
    let app = android.join("src-tauri/gen/android/app");
    std::fs::create_dir_all(app.join("src/main")).unwrap();
    std::fs::write(
        app.join("src/main/AndroidManifest.xml"),
        "<manifest>\n    <application>\n        <activity\n            android:name=\".MainActivity\"\n            android:exported=\"true\">\n        </activity>\n    </application>\n</manifest>\n",
    )
    .unwrap();
    std::fs::write(app.join("build.gradle.kts"), "android {\n    buildTypes {\n        getByName(\"release\") {\n        }\n    }\n}\n").unwrap();
}

/// Every file under `dir`, relative to it.
fn files_under(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out.sort();
    out
}

/// WASM-012 R16–R18 (T25c): the native project of a project not built yet is the shell staged into `android/` with a
/// page saying to build; its identity is needed, its bundle is not; a Build's refresh brings the bundle and the edits
/// into an Android Studio project that exists and leaves a project without one alone; nothing of the application
/// but the bundle is in it — no COBOL, no form, no secret.
#[test]
fn the_android_studio_project_lives_in_the_project_and_follows_the_build_012() {
    use cobolt_compiler::mobile_native as mn;
    let dir = scratch("native");
    std::fs::create_dir_all(dir.join("mobileforms")).unwrap();
    std::fs::write(dir.join("mobileforms/main-form.mcfrm"), "<Form/>").unwrap();
    let req = request(&dir, settings(&["secure-storage"]), Profile::Debug);
    assert_eq!((mn::android_root(&dir), mn::android_studio_dir(&dir)), (dir.join("android"), dir.join("android/src-tauri/gen/android")));
    assert!(!mn::android_exists(&dir));

    // Not built yet: a package is refused for want of the bundle, the native project's shell is not.
    let refused = mb::check(&req, None).unwrap_err();
    assert!(refused.iter().any(|p| p.contains("build the project first")), "{refused:?}");
    let checked = mb::check_shell(&req).expect("the shell needs no bundle");
    let mut no_id = req.clone();
    no_id.settings.app_id.clear();
    assert!(mb::check_shell(&no_id).unwrap_err().iter().any(|p| p.contains("Application id")), "the identity is needed");
    let mut ios = req.clone();
    ios.settings.platform = MobilePlatform::Ios;
    assert!(mn::create_android(&ios, &mut |_| {}).unwrap_err().contains("iOS"));

    // Staged into android/ (what create_android does before the tools run): the page that says to build.
    let mut native = req.clone();
    native.staging_dir = mn::android_root(&dir);
    let staged = mb::stage(&native, &checked).unwrap();
    assert_eq!(read(&staged.root.join("frontend/index.html")), mb::NOT_BUILT_PAGE);
    assert!(staged.app_dir.join("tauri.conf.json").is_file() && staged.root.join("shell/src/lib.rs").is_file());
    assert_eq!(mn::refresh_android(&req, &checked), Ok(false), "no Gradle project yet: nothing to bring up to date");

    // The Gradle project there (as `tauri android init` leaves it), then a Build: the bundle and the edits arrive.
    fake_gradle_project(&mn::android_root(&dir));
    assert!(mn::android_exists(&dir));
    let built = project("native-built", &["SECURESTORE"]);
    let web = dir.join("dist/web");
    std::fs::create_dir_all(web.join("player")).unwrap();
    for f in ["index.html", "player/player.js", "manifest.0123abcd.json"] {
        std::fs::copy(built.join("dist/web").join(f), web.join(f)).unwrap();
    }
    let checked = mb::check(&req, None).expect("built now");
    assert_eq!(mn::refresh_android(&req, &checked), Ok(true));
    assert_eq!(read(&dir.join("android/frontend/index.html")), read(&web.join("index.html")), "the same bundle as the package");
    assert!(dir.join("android/frontend/player/player.js").is_file());
    let manifest = read(&dir.join("android/src-tauri/gen/android/app/src/main/AndroidManifest.xml"));
    assert!(manifest.contains("android:screenOrientation=\"portrait\""), "{manifest}");
    assert!(read(&dir.join("android/src-tauri/gen/android/app/build.gradle.kts")).contains("prcRelease"), "the signing hook");

    // Android Studio shows the Gradle project under the project's name (Tauri writes none): set once, replaced, quoted.
    let tauri_settings = "include ':app'\n\napply from: 'tauri.settings.gradle'\n";
    let named = mn::patch_project_name(tauri_settings, "PowerMobileHello");
    assert!(named.starts_with("rootProject.name = 'PowerMobileHello' //") && named.ends_with("apply from: 'tauri.settings.gradle'\n"), "{named}");
    let renamed = mn::patch_project_name(&named, "Hello Again");
    assert_eq!(renamed.matches("rootProject.name").count(), 1, "{renamed}");
    assert!(renamed.starts_with("rootProject.name = 'Hello Again' //"));
    assert_eq!(mn::patch_project_name(&renamed, "Hello Again"), renamed, "the same name changes nothing");
    assert!(mn::patch_project_name(tauri_settings, "O'Neil").starts_with("rootProject.name = 'O\\'Neil' //"));

    // R18: no COBOL, no form, no secret.
    let listed = files_under(&mn::android_root(&dir));
    let foreign: Vec<&String> = listed.iter().filter(|f| [".cbl", ".cob", ".cpy", ".cfrm", ".wcfrm", ".mcfrm", ".cidx"].iter().any(|x| f.ends_with(x))).collect();
    assert!(foreign.is_empty(), "{foreign:?}");
    for f in &listed {
        let text = String::from_utf8_lossy(&std::fs::read(mn::android_root(&dir).join(f)).unwrap()).into_owned();
        assert!(!text.contains("PASSWORD=") && !text.contains("AKIA") && !text.contains("BEGIN PRIVATE KEY"), "{f}");
    }
    println!(
        "native project: android/ and its Gradle folder; refused package without a bundle, the shell without one accepted; no identity refused; \
         iOS refused; staged with the not-built page; refresh: none -> false, Gradle project -> true with the bundle, the orientation and the signing hook; \
         {} files, no COBOL, no form, no secret",
        listed.len()
    );
}

/// WASM-012 R16, AC12 (T25c), on this computer's tools: the Android Studio project of the example, made in a copy of
/// it, is a Gradle project named after the application, with the identity and the not-built page or the bundle.
/// `cargo test -p cobolt-compiler --test mobile_shell the_android_studio_project_is_generated -- --ignored --nocapture`
#[test]
#[ignore = "needs the Android SDK, the NDK, Java 17 to 21 and the Tauri CLI"]
fn the_android_studio_project_is_generated_by_the_tauri_command_line() {
    use cobolt_compiler::mobile_native as mn;
    let t = Instant::now();
    let dir = scratch("native-live");
    let req = request(&dir, settings(&["secure-storage"]), Profile::Debug);
    let mut lines = 0usize;
    let made = mn::create_android(&req, &mut |_| lines += 1).expect("created");
    let studio = mn::android_studio_dir(&dir);
    assert_eq!(made, mn::Created::Ready(studio.clone()));
    let settings_gradle = read(&studio.join("settings.gradle"));
    assert!(settings_gradle.starts_with("rootProject.name = 'PowerMobileHello'"), "named after the project: {settings_gradle}");
    assert!(settings_gradle.contains("include ':app'"));
    let app_gradle = read(&studio.join("app/build.gradle.kts"));
    assert!(app_gradle.contains("com.example.power_mobile"), "the identity is the Gradle namespace");
    assert!(app_gradle.contains("prcRelease"));
    assert_eq!(read(&dir.join("android/frontend/index.html")), mb::NOT_BUILT_PAGE);
    assert!(dir.join("android/.gitignore").is_file());
    println!("Android Studio project generated in {:.1} s ({lines} tool lines): {}", t.elapsed().as_secs_f64(), studio.display());
}
