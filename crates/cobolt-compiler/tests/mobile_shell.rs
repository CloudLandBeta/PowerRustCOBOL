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
    s.app_id = "com.example.powermobile".into();
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
    assert!(entry.contains("app_id: \"com.example.powermobile\", title: \"Hello\", single_page: true"), "{entry}");

    // tauri.conf.json and the capability.
    let conf = json(&staged.app_dir.join("tauri.conf.json"));
    assert_eq!(conf["identifier"], "com.example.powermobile");
    assert_eq!(conf["productName"], "Hello");
    assert_eq!(conf["version"], "1.2.3");
    assert_eq!(conf["bundle"]["android"]["versionCode"], 10203);
    assert_eq!(conf["bundle"]["android"]["minSdkVersion"], 35);
    assert_eq!(conf["bundle"]["iOS"]["minimumSystemVersion"], "18.0");
    assert_eq!(conf["app"]["windows"], serde_json::json!([]), "the shell opens the one window itself");
    assert_eq!(conf["plugins"]["deep-link"]["mobile"][0]["scheme"][0], "com.example.powermobile", "the scheme is the application id");
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
    let tools = mb::Toolchain { android_home: dir.join("sdk"), ndk_home: dir.join("sdk/ndk/29.0.1"), java_home: dir.join("jdk") };
    assert_eq!(mn::refresh_android(&req, &checked, &tools), Ok(false), "no Gradle project yet: nothing to bring up to date");

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
    assert_eq!(mn::refresh_android(&req, &checked, &tools), Ok(true));
    let cargo_config = read(&dir.join("android/src-tauri/.cargo/config.toml"));
    assert!(cargo_config.contains("[target.aarch64-linux-android]") && cargo_config.contains("aarch64-linux-android35-clang"), "{cargo_config}");
    assert!(read(&dir.join("android/.gitignore")).contains("src-tauri/.cargo/"), "the linker paths are this computer's");
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
    assert!(app_gradle.contains("com.example.powermobile"), "the identity is the Gradle namespace");
    assert!(app_gradle.contains("prcRelease"));
    assert_eq!(read(&dir.join("android/frontend/index.html")), mb::NOT_BUILT_PAGE);
    assert!(dir.join("android/.gitignore").is_file());
    println!("Android Studio project generated in {:.1} s ({lines} tool lines): {}", t.elapsed().as_secs_f64(), studio.display());
}

/// WASM-012 R16 (T25c), Open in Android Studio: the Tauri command line its builds talk to is the
/// debug build of one target that opens Android Studio and stays; it is started as its own program;
/// the native project's Rust build starts from the package build's only where there is none; and
/// nothing is started for a project that cannot have the Android Studio project.
#[test]
fn open_in_android_studio_starts_the_tauri_command_its_builds_need_012() {
    use cobolt_compiler::mobile_native as mn;
    let t = Instant::now();
    let args = mn::studio_args(AndroidAbi::Aarch64);
    assert_eq!(args, ["android", "build", "--ci", "--debug", "--apk", "--target", "aarch64", "--open"]);
    assert_eq!(mn::studio_args(AndroidAbi::X86_64)[6], "x86_64");
    let program = mn::cargo_tauri();
    let exe = program.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_owned();
    assert!(exe == "cargo-tauri" || exe == "cargo-tauri.exe", "{}", program.display());

    // How Cargo links in Android Studio: the NDK's clang of the API level, for the four targets, with the shell's flags.
    let ndk = scratch("studio-ndk");
    std::fs::create_dir_all(ndk.join("toolchains/llvm/prebuilt/linux-x86_64/bin")).unwrap();
    std::fs::write(ndk.join("toolchains/llvm/prebuilt/README"), "not a host").unwrap();
    let config = mn::cargo_config(&ndk, 35);
    let parsed: toml::Value = toml::from_str(&config).expect("valid TOML");
    let targets = parsed["target"].as_table().unwrap();
    assert_eq!(targets.len(), 4);
    let suffix = if cfg!(windows) { ".cmd" } else { "" };
    for (triple, clang) in [
        ("aarch64-linux-android", "aarch64-linux-android"),
        ("armv7-linux-androideabi", "armv7a-linux-androideabi"),
        ("i686-linux-android", "i686-linux-android"),
        ("x86_64-linux-android", "x86_64-linux-android"),
    ] {
        let t = &targets[triple];
        let linker = PathBuf::from(t["linker"].as_str().unwrap());
        assert_eq!(linker, ndk.join("toolchains/llvm/prebuilt/linux-x86_64/bin").join(format!("{clang}35-clang{suffix}")), "{triple}");
        let flags: Vec<&str> = t["rustflags"].as_array().unwrap().iter().map(|f| f.as_str().unwrap()).collect();
        assert_eq!(flags, ["-Clink-arg=-landroid", "-Clink-arg=-llog", "-Clink-arg=-lOpenSLES"]);
    }

    // The seed: cloned where the native project has no build, never over one.
    let dir = scratch("studio-seed");
    let from = dir.join("staging/src-tauri/target");
    std::fs::create_dir_all(from.join("debug")).unwrap();
    std::fs::write(from.join("debug/lib.rlib"), "warm").unwrap();
    let to = dir.join("android/src-tauri/target");
    let mut lines = Vec::new();
    mn::seed_rust_build(&dir.join("absent"), &to, &mut |l| lines.push(l.to_owned()));
    assert!(!to.exists() && lines.is_empty(), "nothing to seed from: nothing made");
    mn::seed_rust_build(&from, &to, &mut |l| lines.push(l.to_owned()));
    if cfg!(target_os = "macos") {
        assert_eq!(read(&to.join("debug/lib.rlib")), "warm");
        assert_eq!(lines.len(), 1, "{lines:?}");
        std::fs::write(from.join("debug/lib.rlib"), "newer").unwrap();
        mn::seed_rust_build(&from, &to, &mut |l| lines.push(l.to_owned()));
        assert_eq!(read(&to.join("debug/lib.rlib")), "warm", "a build that is there is kept");
        assert_eq!(lines.len(), 1);
    } else {
        assert!(!to.exists(), "only a copy-on-write clone seeds");
    }

    // Refused before anything starts: an iOS project, a project without its application id.
    let mut ios = settings(&[]);
    ios.platform = MobilePlatform::Ios;
    let req = request(&dir, ios, Profile::Debug);
    let err = mn::open_android_studio(&req, &dir.join("logs/studio.log")).expect_err("iOS refused");
    assert!(err.contains("iOS"), "{err}");
    let mut no_id = settings(&[]);
    no_id.app_id.clear();
    let err = mn::open_android_studio(&request(&dir, no_id, Profile::Debug), &dir.join("logs/studio.log")).expect_err("no id refused");
    assert!(!err.is_empty());
    assert!(dir.join("logs/studio.log").is_file(), "the log is written from the first line");
    assert!(!mn::android_exists(&dir));
    println!(
        "open in Android Studio: `{} {}`; Cargo's linker for 4 Android targets (API 35, the NDK's host folder found); \
         seed cloned once and kept ({}); iOS and a missing application id refused, nothing started ({} ms)",
        exe,
        args.join(" "),
        if cfg!(target_os = "macos") { "macOS clone" } else { "no clone off macOS" },
        t.elapsed().as_millis()
    );
}

/// WASM-012 AC12 on this computer's tools, Android Studio's half on the command line: Open in
/// Android Studio starts the Tauri command line (which builds once and opens Android Studio on the
/// project); while it runs, the Gradle build Android Studio's Run makes (`assembleArm64Debug`, the
/// arm64 variant) builds the application with its bundle; a running emulator or phone gets it and
/// runs it. `PRC_ANDROID_STAGING` may name a package build's staging folder to seed the Rust build
/// from (the IDE's `…/builds/cobolt-build-powermobilehello-android`).
/// `cargo test -p cobolt-compiler --test mobile_shell gradle_builds_the_android_studio_project -- --ignored --nocapture`
#[test]
#[ignore = "needs the Android SDK, the NDK, Java 17 to 21, the Tauri CLI and Android Studio; opens Android Studio"]
fn gradle_builds_the_android_studio_project_while_the_tauri_command_runs_ac12() {
    use cobolt_compiler::mobile_native as mn;
    use std::process::Command;
    let t = Instant::now();
    let dir = project("ac12", &[]);
    std::fs::write(dir.join("dist/web/index.html"), "<!doctype html><title>AC12</title><p id=\"ac12\">built by Android Studio's Gradle</p>").unwrap();
    let mut req = request(&dir, settings(&[]), Profile::Debug);
    if let Some(staging) = std::env::var_os("PRC_ANDROID_STAGING") {
        req.staging_dir = PathBuf::from(staging);
    }
    let log = dir.join("studio.log");
    /// Ends the Tauri command line on every way out of the test, a failed assertion included.
    struct Stop(std::process::Child);
    impl Drop for Stop {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = Stop(mn::open_android_studio(&req, &log).unwrap_or_else(|e| panic!("not started: {e}")));
    let started = t.elapsed();
    // The command line builds once, then opens Android Studio and waits.
    let deadline = Instant::now() + std::time::Duration::from_secs(40 * 60);
    loop {
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        if text.contains("Opening Android Studio") {
            break;
        }
        if let Some(status) = child.0.try_wait().unwrap() {
            panic!("the Tauri command line ended ({status}) before opening Android Studio:\n{text}");
        }
        assert!(Instant::now() < deadline, "not open after 40 minutes:\n{text}");
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    let opened = t.elapsed();
    let studio = mn::android_studio_dir(&dir);
    let tools = mb::Toolchain::detect(AndroidAbi::Aarch64).expect("tools");
    let gradlew = studio.join(if cfg!(windows) { "gradlew.bat" } else { "gradlew" });
    let mut gradle = Command::new(&gradlew);
    gradle.arg("assembleArm64Debug").current_dir(&studio).env_remove("CARGO_TARGET_DIR");
    for (k, v) in tools.env() {
        gradle.env(k, v);
    }
    let out = gradle.output().expect("gradlew runs");
    let gradle_text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let built = t.elapsed();
    assert!(out.status.success(), "Gradle failed while the Tauri command line ran:\n{gradle_text}");
    let apk = studio.join("app/build/outputs/apk/arm64/debug/app-arm64-debug.apk");
    let bytes = std::fs::metadata(&apk).map(|m| m.len()).unwrap_or(0);
    assert!(bytes > 0, "no package at {}", apk.display());
    let still = child.0.try_wait().unwrap().is_none();
    drop(child);
    assert!(still, "the Tauri command line keeps running until it is stopped");
    // On a running emulator or phone: install it and see it run.
    let adb = tools.adb();
    let devices = Command::new(&adb).arg("devices").output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let serial = devices.lines().skip(1).find_map(|l| l.strip_suffix("\tdevice").map(str::to_owned));
    let ran = serial.as_ref().map(|s| {
        let install = Command::new(&adb).args(["-s", s, "install", "-r"]).arg(&apk).output().expect("adb install");
        assert!(install.status.success(), "{}", String::from_utf8_lossy(&install.stderr));
        let id = "com.example.powermobile";
        let _ = Command::new(&adb).args(["-s", s, "shell", "am", "force-stop", id]).output();
        let launch = Command::new(&adb).args(["-s", s, "shell", "monkey", "-p", id, "-c", "android.intent.category.LAUNCHER", "1"]).output().expect("adb monkey");
        assert!(launch.status.success());
        std::thread::sleep(std::time::Duration::from_secs(8));
        let pid = Command::new(&adb).args(["-s", s, "shell", "pidof", id]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned()).unwrap_or_default();
        assert!(!pid.is_empty(), "the application is not running on {s}");
        format!("installed and running on {s} (pid {pid})")
    });
    let log_lines = std::fs::read_to_string(&log).map(|t| t.lines().count()).unwrap_or(0);
    println!(
        "AC12 (command line): started in {:.1} s, Android Studio opened at {:.1} s, `gradlew assembleArm64Debug` built {:.1} MB at {:.1} s \
         (Gradle {:.1} s) while the Tauri command line ran; {}; {log_lines} log lines; seeded: {}",
        started.as_secs_f64(),
        opened.as_secs_f64(),
        bytes as f64 / 1_048_576.0,
        built.as_secs_f64(),
        (built - opened).as_secs_f64(),
        ran.unwrap_or_else(|| "no emulator or phone attached".into()),
        std::env::var_os("PRC_ANDROID_STAGING").is_some()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ── iOS (WASM-012 R1, R2, R5; the simulator application) ────────────────────────────────────────

fn ios_settings() -> MobileSettings {
    let mut s = MobileSettings::new(MobilePlatform::Ios, DeviceClass::Phone);
    s.app_id = "com.example.powermobile".into();
    s
}

#[test]
fn an_ios_project_stages_the_info_plist_the_tauri_command_line_merges() {
    let t = Instant::now();
    let dir = project("ios-stage", &[]);
    let mut s = ios_settings();
    s.orientation = Orientation::Portrait;
    s.app_name.insert("en".into(), "Olá & <Hi>".into());
    s.app_name.insert("es".into(), "Hola".into());
    s.app_name.insert("ja".into(), "こんにちは".into());
    s.app_name.insert("zh".into(), "你好".into());
    s.app_name.insert("fr".into(), "   ".into());
    let req = request(&dir, s, Profile::Debug);
    let checked = mb::check(&req, None).expect("a complete iOS project passes");
    assert_eq!((checked.build_number, checked.ios_min_version.as_str()), (10203, "18.0"), "1.2.3 is build 10203; auto is iOS 18");
    let staged = mb::stage(&req, &checked).unwrap();
    assert_eq!(staged.apple_dir, staged.app_dir.join("gen/apple"));

    // The name under the icon, escaped, with the languages that have one (a blank name has none).
    let plist = read(&staged.app_dir.join("Info.ios.plist"));
    assert!(plist.contains("<key>CFBundleDisplayName</key>\n\t<string>Olá &amp; &lt;Hi&gt;</string>"), "{plist}");
    let localizations: Vec<&str> = plist.split("<key>CFBundleLocalizations</key>").nth(1).unwrap().split("</array>").next().unwrap().matches("<string>").map(|_| "").collect();
    assert_eq!(localizations.len(), 4, "en, es, ja and zh-Hans: {plist}");
    for l in ["<string>en</string>", "<string>es</string>", "<string>ja</string>", "<string>zh-Hans</string>"] {
        assert!(plist.contains(l), "{l} in {plist}");
    }
    assert!(!plist.contains("<string>fr</string>") && !plist.contains("<string>pt</string>"));
    // Portrait: the iPhone's one, and the iPad's two.
    assert!(plist.contains("<key>UISupportedInterfaceOrientations</key>\n\t<array>\n\t\t<string>UIInterfaceOrientationPortrait</string>\n\t</array>"), "{plist}");
    assert!(plist.contains("<key>UISupportedInterfaceOrientations~ipad</key>") && plist.contains("UIInterfaceOrientationPortraitUpsideDown"), "{plist}");
    let mut landscape = req.clone();
    landscape.settings.orientation = Orientation::Landscape;
    let l = mb::ios_info_plist(&landscape);
    assert!(l.contains("UIInterfaceOrientationLandscapeLeft") && l.contains("UIInterfaceOrientationLandscapeRight") && !l.contains("Portrait"), "{l}");
    let mut auto = req.clone();
    auto.settings.orientation = Orientation::Auto;
    assert!(!mb::ios_info_plist(&auto).contains("UISupportedInterfaceOrientations"), "auto leaves what Tauri wrote");
    // The property list is well-formed to the system's own reader.
    if cfg!(target_os = "macos") {
        for text in [&plist, &l] {
            let path = dir.join("Info.ios.check.plist");
            std::fs::write(&path, text).unwrap();
            let out = std::process::Command::new("plutil").arg("-lint").arg(&path).output().expect("plutil runs on a Mac");
            assert!(out.status.success(), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        }
    }

    // tauri.conf.json: a file-safe product name (the Xcode target and the `.app`), CFBundleVersion.
    let conf = json(&staged.app_dir.join("tauri.conf.json"));
    assert_eq!(conf["productName"], "PowerMobileHello");
    assert_eq!(conf["bundle"]["iOS"]["minimumSystemVersion"], "18.0");
    assert_eq!(conf["bundle"]["iOS"]["bundleVersion"], "10203");
    assert_eq!(conf["version"], "1.2.3");

    // An Android project is as it was: the name under the icon is the product name, no iOS build number, no plist.
    let mut android = settings(&[]);
    android.app_name.insert("en".into(), "Olá".into());
    let areq = request(&project("ios-stage-android", &[]), android, Profile::Debug);
    let astaged = mb::stage(&areq, &mb::check(&areq, None).unwrap()).unwrap();
    let aconf = json(&astaged.app_dir.join("tauri.conf.json"));
    assert_eq!(aconf["productName"], "Olá");
    assert!(aconf["bundle"]["iOS"].get("bundleVersion").is_none());
    assert!(!astaged.app_dir.join("Info.ios.plist").exists());
    let _ = std::fs::remove_dir_all(&dir);
    println!(
        "ios staging: Info.ios.plist with the name escaped (Olá &amp; &lt;Hi&gt;), 4 languages (a blank name skipped), portrait/landscape/auto orientations, plutil -lint ok; \
         tauri.conf productName PowerMobileHello, CFBundleVersion 10203, minimum iOS 18.0; the Android project unchanged; {} ms",
        t.elapsed().as_millis()
    );
}

#[test]
fn an_ios_release_is_refused_and_the_application_goes_to_dist_mobile_ios() {
    let dir = project("ios-release", &[]);
    let problems = mb::check(&request(&dir, ios_settings(), Profile::Release), None).unwrap_err();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("Apple developer team") && problems[0].contains("Debug"), "{problems:?}");
    assert!(!problems[0].contains("keystore"), "an iOS release is not asked for an Android keystore");
    assert!(mb::check(&request(&dir, ios_settings(), Profile::Debug), None).is_ok());
    let ios = request(&dir, ios_settings(), Profile::Debug);
    assert!(ios.package_path().ends_with("dist/mobile/ios/PowerMobileHello-debug.app"), "{}", ios.package_path().display());
    assert!(request(&dir, settings(&[]), Profile::Debug).package_path().ends_with("dist/mobile/android/PowerMobileHello-debug.apk"));
    // Each builder refuses the other platform's project before any tool runs.
    let tools = mb::IosToolchain { path_dirs: Vec::new(), xcodegen: PathBuf::from("/nowhere/xcodegen") };
    let wrong = mb::build_ios(&request(&dir, settings(&[]), Profile::Debug), &tools, &mut |_| {}).unwrap_err();
    assert!(wrong.contains("platform is Android"), "{wrong}");
    let _ = std::fs::remove_dir_all(&dir);
    println!("ios package: release refused with one sentence (no keystore asked), debug passes; .app under dist/mobile/ios, .apk under dist/mobile/android; an Android project refused by build_ios");
}

/// What a Mac that can build the package has.
fn ios_ready() -> mb::IosFacts {
    mb::IosFacts {
        macos: true,
        apple_silicon: true,
        xcode: Some(27),
        xcodegen: Some(PathBuf::from("/opt/homebrew/bin/xcodegen")),
        idevicesyslog: Some(PathBuf::from("/opt/homebrew/bin/idevicesyslog")),
        pod: Some(PathBuf::from("/opt/homebrew/bin/pod")),
        tauri_cli: None,
        rust_target: None,
    }
}

#[test]
fn each_ios_tool_hidden_in_turn_is_named_with_the_command_that_gets_it() {
    assert!(mb::ios_problems(&ios_ready()).is_empty(), "a Mac with everything is ready");
    type Hide = fn(&mut mb::IosFacts);
    let cases: [(&str, Hide, &str, &str); 7] = [
        ("Xcode", |f| f.xcode = None, "Xcode", "Mac App Store"),
        ("Xcode 15", |f| f.xcode = Some(15), "Xcode 16 or later", "update it"),
        ("XcodeGen", |f| f.xcodegen = None, "XcodeGen", "brew install xcodegen"),
        ("libimobiledevice", |f| f.idevicesyslog = None, "libimobiledevice", "brew install libimobiledevice"),
        ("CocoaPods", |f| f.pod = None, "CocoaPods", "brew install cocoapods"),
        ("Tauri CLI", |f| f.tauri_cli = Some(mb::Missing { what: "tauri-cli 2.12.4".into(), how: mb::INSTALL_TAURI_CLI.into() }), "tauri-cli", "cargo install tauri-cli"),
        ("Rust target", |f| f.rust_target = Some(mb::Missing { what: "the Rust target aarch64-apple-ios-sim".into(), how: "rustup target add aarch64-apple-ios-sim".into() }), "Rust target", "rustup target add aarch64-apple-ios-sim"),
    ];
    let mut rows = Vec::new();
    for (hidden, hide, what, how) in cases {
        let mut f = ios_ready();
        hide(&mut f);
        let problems = mb::ios_problems(&f);
        assert_eq!(problems.len(), 1, "{hidden}: {problems:?}");
        assert!(problems[0].what.contains(what) && problems[0].how.contains(how), "{hidden}: {problems:?}");
        rows.push(format!("{hidden} -> {}", problems[0].how.split(':').last().unwrap_or("").trim()));
    }
    // Several at once are all named; Xcode is never offered as an install command.
    let mut f = ios_ready();
    (f.xcode, f.xcodegen, f.pod) = (None, None, None);
    let all = mb::ios_problems(&f);
    assert_eq!(all.len(), 3);
    assert!(all.iter().all(|m| !m.how.contains("brew install xcode") || m.how.contains("xcodegen")), "{all:?}");
    assert!(all[0].how.contains("never installs Xcode"));
    // Off macOS: one reason, nothing else; an Intel Mac: the simulator target is Apple silicon's.
    let mut f = ios_ready();
    f.macos = false;
    f.xcodegen = None;
    let off = mb::ios_problems(&f);
    assert_eq!(off.len(), 1);
    assert!(off[0].how.contains("only be built on macOS"));
    let mut f = ios_ready();
    f.apple_silicon = false;
    assert!(mb::ios_problems(&f)[0].how.contains("Apple-silicon"));
    println!("ios tools: ready Mac -> 0 problems; 7 tools hidden one at a time -> 1 each, named with the command ({}); 3 hidden together -> 3; off macOS -> 1; Intel -> 1", rows.join("; "));
}

#[test]
fn the_ios_tools_are_read_from_what_they_print_and_found_where_homebrew_puts_them() {
    assert_eq!(mb::parse_xcode_version("Xcode 27.2\nBuild version 18C5033c\n"), Some(27));
    assert_eq!(mb::parse_xcode_version("Xcode 16.4"), Some(16));
    assert_eq!(mb::parse_xcode_version("xcode-select: error: tool 'xcodebuild' requires Xcode, but active developer directory is a command line tools instance"), None);
    let dir = scratch("ios-tools");
    let (a, b) = (dir.join("a"), dir.join("b"));
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    std::fs::write(b.join("xcodegen"), "#!/bin/sh\n").unwrap();
    std::fs::create_dir(a.join("pod")).unwrap();
    assert_eq!(mb::find_tool("xcodegen", &[a.clone(), b.clone()]), Some(b.join("xcodegen")));
    assert_eq!(mb::find_tool("pod", &[a.clone(), b.clone()]), None, "a folder is not a tool");
    assert_eq!(mb::find_tool("idevicesyslog", &[a, b.clone()]), None);
    // The build's environment: the tools' folders before the PATH, a UTF-8 locale for CocoaPods.
    let tools = mb::IosToolchain { path_dirs: vec![b.clone()], xcodegen: b.join("xcodegen") };
    let env = tools.env();
    let path = env.iter().find(|(k, _)| *k == "PATH").map(|(_, v)| v.clone()).unwrap();
    assert_eq!(std::env::split_paths(&path).next(), Some(b));
    assert_eq!(env.iter().find(|(k, _)| *k == "LANG").map(|(_, v)| v.to_string_lossy().into_owned()).as_deref(), Some("en_US.UTF-8"));
    let _ = std::fs::remove_dir_all(&dir);
    println!("ios tools: 3 Xcode outputs read (27, 16, a command-line-tools refusal -> none); xcodegen found in the second folder, a folder named pod ignored, a missing tool none; PATH leads with the tool folder, LANG en_US.UTF-8");
}

#[test]
fn the_name_under_the_ios_icon_is_a_strings_file_per_language_and_the_project_is_made_again_only_when_one_changes() {
    let dir = project("ios-names", &[]);
    let mut s = ios_settings();
    s.app_name.insert("es".into(), "Hola \"mundo\" \\ 1".into());
    s.app_name.insert("ja".into(), "こんにちは".into());
    let mut req = request(&dir, s, Profile::Debug);
    let checked = mb::check(&req, None).unwrap();
    let staged = mb::stage(&req, &checked).unwrap();
    let target = staged.apple_dir.join("powermobilehello_iOS");
    std::fs::create_dir_all(&target).unwrap();

    assert!(mb::patch_ios_project(&staged, &req).unwrap(), "two languages written: the project is made again");
    let es = read(&target.join("es.lproj/InfoPlist.strings"));
    assert!(es.contains("\"CFBundleDisplayName\" = \"Hola \\\"mundo\\\" \\\\ 1\";"), "{es}");
    assert!(read(&target.join("ja.lproj/InfoPlist.strings")).contains("\"CFBundleDisplayName\" = \"こんにちは\";"));
    assert!(!target.join("en.lproj").exists() && !target.join("pt.lproj").exists());
    assert!(!mb::patch_ios_project(&staged, &req).unwrap(), "nothing changed: no new project, so Xcode rebuilds nothing");
    req.settings.app_name.remove("ja");
    assert!(mb::patch_ios_project(&staged, &req).unwrap(), "a language taken away");
    assert!(!target.join("ja.lproj").exists() && target.join("es.lproj").exists());
    assert!(!mb::patch_ios_project(&staged, &req).unwrap());
    assert_eq!(mb::ios_strings_value("a\nb"), "\"a b\"", "a line break is a space");

    // The generated project is the project's own, or it is generated again from nothing.
    assert!(!mb::apple_project_current(&staged, &req, &checked), "no project yet");
    let yml = "settingGroups:\n  app:\n    base:\n      PRODUCT_NAME: PowerMobileHello\n      PRODUCT_BUNDLE_IDENTIFIER: com.example.powermobile\n  options:\n    deploymentTarget:\n      iOS: 18.0\ntargets:\n  powermobilehello_iOS:\n    type: application\n";
    std::fs::write(staged.apple_dir.join("project.yml"), yml).unwrap();
    assert!(mb::apple_project_current(&staged, &req, &checked));
    for (what, changed) in [
        ("another application id", yml.replace("com.example.powermobile", "com.example.other")),
        ("another product name", yml.replace("PowerMobileHello", "Other")),
        ("another floor", yml.replace("iOS: 18.0", "iOS: 15.0")),
        ("another target", yml.replace("powermobilehello_iOS", "other_iOS")),
    ] {
        std::fs::write(staged.apple_dir.join("project.yml"), changed).unwrap();
        assert!(!mb::apple_project_current(&staged, &req, &checked), "{what}");
    }
    let _ = std::fs::remove_dir_all(&dir);
    println!("ios names: es and ja written as InfoPlist.strings (quote and backslash escaped), 2nd pass changed nothing, a removed language deleted its folder; generated project current for the same id/name/target/floor, regenerated for 4 changes");
}
