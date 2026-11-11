// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The iOS package, end to end** (WASM-012 R1, AC1 iOS half; WASM-002 AC7 iOS half): an iPhone
//! project is made with `create_project`, built by the `build` tool with the real generator — its
//! Web bundle, then the simulator application — and the application is installed on the iOS
//! simulator, started, and photographed.
//!
//! It runs real tools (Xcode, XcodeGen, CocoaPods, the Tauri command line) and compiles the shell for
//! the simulator, minutes the first time, so it is `#[ignore]`d:
//! `cargo test -p cobolt-project-tools --test ios_package -- --ignored --nocapture`.
//! `PRC_IOS_SIMULATOR` names a simulator (its name or UDID; else the newest iPhone), and
//! `PRC_IOS_SCREENSHOT` a folder for the picture of the running application.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cobolt_project_tools::web::{self, generator::GeneratorBackend};
use cobolt_project_tools::{HeadlessHost, ProjectTools};
use serde_json::{json, Value};

fn xcrun(args: &[&str]) -> Result<String, String> {
    let out = Command::new("xcrun").args(args).output().map_err(|e| format!("xcrun: {e}"))?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!("xcrun {} failed ({}): {}", args.join(" "), out.status, text.trim()))
    }
}

/// The simulator to run on: `PRC_IOS_SIMULATOR`, else the newest iPhone.
fn simulator() -> Result<(String, String, bool), String> {
    let list: Value = serde_json::from_str(&xcrun(&["simctl", "list", "devices", "available", "-j"])?).map_err(|e| e.to_string())?;
    let wanted = std::env::var("PRC_IOS_SIMULATOR").ok();
    let mut runtimes: Vec<(&String, &Value)> = list["devices"].as_object().ok_or("no device list")?.iter().collect();
    runtimes.sort_by(|a, b| b.0.cmp(a.0));
    // The newest runtime's phones (`iPhone 18 Pro`, not the two-screen `iPhone Duo`), one already running first.
    let mut found: Vec<(String, String, bool)> = Vec::new();
    for (_, devices) in runtimes {
        for d in devices.as_array().into_iter().flatten() {
            let (name, udid) = (d["name"].as_str().unwrap_or(""), d["udid"].as_str().unwrap_or(""));
            let hit = match &wanted {
                Some(w) => w == name || w == udid,
                None => name.strip_prefix("iPhone ").is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit())),
            };
            if hit {
                found.push((name.to_owned(), udid.to_owned(), d["state"] == "Booted"));
            }
        }
    }
    found.iter().find(|f| f.2).or(found.first()).cloned().ok_or_else(|| "no iPhone simulator is installed".to_owned())
}

fn plist_json(app: &Path) -> Value {
    let out = Command::new("plutil").args(["-convert", "json", "-o", "-"]).arg(app.join("Info.plist")).output().expect("plutil runs on a Mac");
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("Info.plist is not readable: {e}"))
}

#[test]
#[ignore = "needs Xcode 16+, XcodeGen, libimobiledevice, CocoaPods, tauri-cli 2.12.4, the aarch64-apple-ios-sim target and an iOS simulator; compiles for minutes the first time"]
fn an_iphone_project_builds_its_simulator_application_and_it_runs_on_the_simulator() {
    web::link(Arc::new(GeneratorBackend));
    let folder = std::env::temp_dir().join(format!("prc-ios-package-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    let mut tools = ProjectTools::new(HeadlessHost::new(std::env::temp_dir().join("prc-ios-package-nothing-here"), "test"));
    tools.call("create_project", &json!({"folder": folder.to_string_lossy(), "name": "PowerIosHello", "template": "iphone"})).unwrap_or_else(|e| panic!("{e}"));

    // The name under the icon in two languages, and a portrait application: written as the Settings page does.
    let manifest = folder.join("PowerIosHello.project.toml");
    let mut text = std::fs::read_to_string(&manifest).unwrap();
    text = text.replace("orientation = \"auto\"", "orientation = \"portrait\"");
    text.push_str("\n[mobile.app_name]\nen = \"Hello iOS\"\nes = \"Hola iOS\"\n");
    std::fs::write(&manifest, text).unwrap();
    let view = cobolt_compiler::project_manifest_view(&manifest).unwrap();
    let mobile = view.mobile.clone().expect("a Mobile project");
    assert_eq!(mobile.platform.as_str(), "ios");
    let app_id = mobile.app_id.trim().to_owned();

    // Build: the Web bundle, then the package; a call waits 40 s, the next one waits on the same build.
    let t = Instant::now();
    let mut answer = tools.call("build", &json!({})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(answer["status"], "built", "{answer}");
    let first_call_ms = t.elapsed().as_millis();
    let deadline = Instant::now() + Duration::from_secs(40 * 60);
    while answer["package"]["status"] == "running" {
        assert!(Instant::now() < deadline, "the package did not finish in 40 minutes: {answer}");
        answer = tools.call("build", &json!({})).unwrap_or_else(|e| panic!("{e}"));
    }
    let build_ms = t.elapsed().as_millis();
    let package = &answer["package"];
    assert_eq!(package["built"], true, "{package}");
    let rel = package["app"].as_str().expect("the application's path");
    assert_eq!(rel, "dist/mobile/ios/PowerIosHello-debug.app");
    let app = folder.join(rel);
    let bytes = package["bytes"].as_u64().unwrap_or(0);
    assert!(app.join("Info.plist").is_file() && bytes > 1_000_000, "{rel}: {bytes} bytes");

    // What the application declares: identity, versions, floor, orientation, names, icon, signature, the bundle.
    let info = plist_json(&app);
    assert_eq!(info["CFBundleIdentifier"], app_id);
    assert_eq!(info["CFBundleShortVersionString"], view.version);
    assert_eq!(info["CFBundleVersion"], cobolt_compiler::project_kind::build_number(&view.version).unwrap().to_string());
    assert_eq!(info["MinimumOSVersion"], "18.0");
    assert_eq!(info["CFBundleDisplayName"], "Hello iOS");
    assert_eq!(info["UISupportedInterfaceOrientations"], json!(["UIInterfaceOrientationPortrait"]));
    assert!(info["CFBundleLocalizations"].as_array().unwrap().iter().any(|l| l == "es"));
    assert!(app.join("es.lproj/InfoPlist.strings").is_file(), "the Spanish name is in the application");
    assert!(app.join("Assets.car").is_file() && app.join("_CodeSignature").is_dir());
    // The Web bundle is not a folder of the application: Tauri compiles `frontendDist` into the executable.
    // What can be said without running it is that the executable is larger than the bundle it carries;
    // that it carries the right one is the picture of the running application.
    let executable = std::fs::metadata(app.join("PowerIosHello")).expect("the executable").len();
    let bundle_bytes = cobolt_compiler::mobile_build::dir_size(&folder.join("dist/web"));
    assert!(bundle_bytes > 100_000 && executable > bundle_bytes, "the executable ({executable} B) must carry the bundle ({bundle_bytes} B)");
    let spec = Command::new("codesign").args(["--verify", "--deep", "--strict"]).arg(&app).output().expect("codesign");
    assert!(spec.status.success(), "the copy in the project keeps a valid signature: {}", String::from_utf8_lossy(&spec.stderr));

    // On the simulator: install, start, still running a few seconds later, photographed.
    let (name, udid, was_booted) = simulator().unwrap_or_else(|e| panic!("{e}"));
    let t1 = Instant::now();
    /// Shuts down a simulator the test booted, on every way out, a failed assertion included.
    struct Shutdown(Option<String>);
    impl Drop for Shutdown {
        fn drop(&mut self) {
            if let Some(udid) = &self.0 {
                let _ = xcrun(&["simctl", "shutdown", udid]);
            }
        }
    }
    let _shutdown = Shutdown((!was_booted).then(|| udid.clone()));
    if !was_booted {
        xcrun(&["simctl", "bootstatus", &udid, "-b"]).unwrap_or_else(|e| panic!("{e}"));
    }
    let booted_ms = t1.elapsed().as_millis();
    let _ = xcrun(&["simctl", "terminate", &udid, &app_id]);
    xcrun(&["simctl", "install", &udid, &app.to_string_lossy()]).unwrap_or_else(|e| panic!("{e}"));
    let launched = xcrun(&["simctl", "launch", &udid, &app_id]).unwrap_or_else(|e| panic!("{e}"));
    std::thread::sleep(Duration::from_secs(8));
    let running = xcrun(&["simctl", "spawn", &udid, "launchctl", "list"]).unwrap_or_default();
    let pid = running.lines().find(|l| l.contains(&format!("UIKitApplication:{app_id}"))).and_then(|l| l.split_whitespace().next()).unwrap_or("-").to_owned();
    let shot = std::env::var_os("PRC_IOS_SCREENSHOT").map(PathBuf::from).map(|d| {
        let _ = std::fs::create_dir_all(&d);
        let path = d.join("PowerIosHello-simulator.png");
        xcrun(&["simctl", "io", &udid, "screenshot", &path.to_string_lossy()]).unwrap_or_else(|e| panic!("{e}"));
        path
    });
    let _ = xcrun(&["simctl", "terminate", &udid, &app_id]);
    assert!(pid != "-" && pid.parse::<u32>().is_ok(), "the application is not running on {name}: launchctl said «{pid}»; launch said «{}»", launched.trim());
    let _ = std::fs::remove_dir_all(&folder);
    println!(
        "iOS package (iPhone template, real generator): Build {build_ms} ms (the first call, which waits up to 40 s on the package, returned at {first_call_ms} ms; the package step {} s), {:.1} MB application {rel}; \
         Info.plist: id {app_id}, version {} build {}, iOS {}, portrait, name «Hello iOS» + es; icon and signature present, executable {:.0} MB carrying a {:.1} MB bundle; \
         on {name} ({} the simulator, ready in {booted_ms} ms): installed, started (pid {pid}), still running after 8 s; picture: {}",
        package["elapsed_s"].as_u64().unwrap_or(0),
        bytes as f64 / 1_048_576.0,
        view.version,
        info["CFBundleVersion"].as_str().unwrap_or("?"),
        info["MinimumOSVersion"].as_str().unwrap_or("?"),
        executable as f64 / 1_048_576.0,
        bundle_bytes as f64 / 1_048_576.0,
        if was_booted { "was already running" } else { "booted headless, shut down after" },
        shot.map(|p| p.display().to_string()).unwrap_or_else(|| "not asked (PRC_IOS_SCREENSHOT)".into()),
    );
}
