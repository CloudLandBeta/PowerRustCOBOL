// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! Guards on the installer's source (task T14): `wix/product.wxs` and the release workflow that builds it.
//!
//! The WiX is the one part of the setup that cannot run on a developer's machine: it is linked by `light` on a
//! Windows runner and run by Windows Installer. What can be checked here is what a later edit could silently break
//! and that nothing else would notice until a customer's install:
//!
//! * the order of the script (the preparation after the files and before the example copy, and the rollback before
//!   both), because a cancelled install must not leave a copied project, and a cancel in a step must clean its cache;
//! * that every preparation action is conditioned on the choice, so *Skip* and a silent install without the opt-in
//!   install exactly today's files (AC1, AC2);
//! * that only the actions that may hear Cancel can fail the install, so a preparation that cannot finish never fails
//!   it (R12);
//! * that the page is reached only when there is something to prepare;
//! * that the data the steps are given is what the supervisor reads, and the DLL names exports that exist;
//! * that the file is ASCII (the database code page) and the workflow builds it the way its header says.
//!
//! Each assertion fails when the source is edited the wrong way; `the_order_guard_fails_when_the_seed_copy_moves_up`
//! shows it once, on a deliberately wrong sequence.

use crate::data::{RunData, Spawn};
use crate::progress::Part;

const WXS: &str = include_str!("../wix/product.wxs");
const WXL: &str = include_str!("../wix/prc-strings.wxl");
const WORKFLOW: &str = include_str!("../../../.github/workflows/build-all-platforms.yml");

/// The five entry points the DLL exports, and the files that define them.
const EXPORTS: [(&str, &str); 5] = [
    ("PrcFacts", include_str!("facts/system.rs")),
    ("PrcRun", include_str!("supervisor.rs")),
    ("PrcTicks", include_str!("entries.rs")),
    ("PrcRollback", include_str!("entries.rs")),
    ("PrcReadResult", include_str!("entries.rs")),
];

/// The text of one attribute, `name="value"`, from the start tag in `tag`.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let from = tag.find(&key)? + key.len();
    let len = tag[from..].find('"')?;
    Some(&tag[from..from + len])
}

/// A condition as written: the text of an element, without a `CDATA` wrapper.
fn condition(text: &str) -> String {
    text.trim()
        .trim_start_matches("<![CDATA[")
        .trim_end_matches("]]>")
        .trim()
        .to_owned()
}

/// The text between `<name ...>` and `</name>`, for the first such element.
fn block<'a>(text: &'a str, name: &str) -> &'a str {
    let open = text
        .find(&format!("<{name}>"))
        .unwrap_or_else(|| panic!("no <{name}> in the WiX"));
    let from = open + name.len() + 2;
    let end = text[from..]
        .find(&format!("</{name}>"))
        .unwrap_or_else(|| panic!("no </{name}> in the WiX"));
    &text[from..from + end]
}

/// One scheduled action: `<Custom Action=".." After="..">condition</Custom>`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Scheduled {
    action: String,
    /// `"After"` or `"Before"`, with the action it is relative to.
    relation: (String, String),
    condition: String,
}

fn schedule(sequence: &str) -> Vec<Scheduled> {
    sequence
        .split("<Custom ")
        .skip(1)
        .map(|part| {
            let tag_end = part.find('>').expect("a <Custom> tag is closed");
            let tag = format!(" {}", &part[..tag_end]);
            let body_end = part
                .find("</Custom>")
                .expect("a <Custom> element is closed");
            let relation = if let Some(a) = attr(&tag, "After") {
                ("After".to_owned(), a.to_owned())
            } else {
                (
                    "Before".to_owned(),
                    attr(&tag, "Before").expect("After or Before").to_owned(),
                )
            };
            Scheduled {
                action: attr(&tag, "Action").expect("an Action").to_owned(),
                relation,
                condition: condition(&part[tag_end + 1..body_end]),
            }
        })
        .collect()
}

/// The order the script runs a chain of `After` links in, from the first anchor: each action follows the one it
/// names. A broken chain (an action that names one that is not there) stops it.
fn chain_from(anchor: &str, actions: &[Scheduled]) -> Vec<String> {
    let mut order = vec![anchor.to_owned()];
    while let Some(next) = actions
        .iter()
        .find(|s| s.relation == ("After".to_owned(), order.last().unwrap().clone()))
    {
        order.push(next.action.clone());
        if order.len() > 64 {
            break;
        }
    }
    order
}

/// Every `<CustomAction .../>` start tag, with a leading space so `attr` finds its first attribute.
fn custom_actions() -> Vec<String> {
    WXS.split("<CustomAction ")
        .skip(1)
        .map(|part| {
            format!(
                " {}",
                &part[..part.find("/>").expect("a CustomAction is self-closed")]
            )
        })
        .collect()
}

fn custom_action(id: &str) -> String {
    custom_actions()
        .into_iter()
        .find(|tag| attr(tag, "Id") == Some(id))
        .unwrap_or_else(|| panic!("no CustomAction {id}"))
}

fn exec() -> Vec<Scheduled> {
    schedule(block(WXS, "InstallExecuteSequence"))
}

fn ui() -> Vec<Scheduled> {
    // The UI sequence is inside the <UI Id="PrcUI"> element, which also holds <Show> lines; only the <Custom> ones
    // are the preparation's.
    schedule(block(WXS, "InstallUISequence"))
}

/// The order check, on any sequence text, so it can be shown to fail.
fn order_is_right(actions: &[Scheduled]) -> Result<(), String> {
    let want = [
        "InstallFiles",
        "SetPrcRollback",
        "PrcRollback",
        "SetPrcRust",
        "PrcRust",
        "SetPrcCompile",
        "PrcCompile",
        "SetSeedExamples",
        "SeedExamples",
    ];
    let got = chain_from("InstallFiles", actions);
    if got != want {
        return Err(format!(
            "the script after InstallFiles must run {want:?}, but the chain is {got:?}"
        ));
    }
    Ok(())
}

#[test]
fn the_preparation_runs_after_the_files_and_before_the_example_copy_and_the_rollback_comes_first() {
    let actions = exec();
    order_is_right(&actions).unwrap();
    // The bar's room is made while the script is being built, before the files.
    let ticks = actions
        .iter()
        .find(|s| s.action == "PrcTicks")
        .expect("PrcTicks is scheduled");
    assert_eq!(
        ticks.relation,
        ("After".to_owned(), "InstallInitialize".to_owned())
    );
}

#[test]
fn the_order_guard_fails_when_the_seed_copy_moves_up() {
    // The copy of the example project right after the files, as it was before the preparation existed: a Cancel in
    // the compile would then leave a copied project behind, which is user code and is never rolled back.
    let mut wrong = exec();
    for s in &mut wrong {
        match s.action.as_str() {
            "SetSeedExamples" => s.relation = ("After".to_owned(), "InstallFiles".to_owned()),
            "SetPrcRollback" => s.relation = ("After".to_owned(), "SeedExamples".to_owned()),
            _ => {}
        }
    }
    let err = order_is_right(&wrong).expect_err("a sequence with the copy first must be refused");
    assert!(err.contains("must run"), "{err}");
}

#[test]
fn every_preparation_action_is_conditioned_on_the_choice_and_never_runs_on_a_removal() {
    let actions = exec();
    for name in [
        "PrcTicks",
        "SetPrcRollback",
        "PrcRollback",
        "SetPrcRust",
        "PrcRust",
        "SetPrcCompile",
        "PrcCompile",
    ] {
        let s = actions
            .iter()
            .find(|s| s.action == name)
            .unwrap_or_else(|| panic!("{name} is not scheduled"));
        assert!(
            s.condition.contains("PRC_PREPARE = \"1\""),
            "{name} must run only when the developer chose Prepare now: `{}`",
            s.condition
        );
        assert!(
            s.condition.contains("NOT REMOVE"),
            "{name} must not run on an uninstall: `{}`",
            s.condition
        );
    }
    // The compile needs the linker; the Rust step does not.
    for name in ["SetPrcCompile", "PrcCompile"] {
        let s = actions.iter().find(|s| s.action == name).unwrap();
        assert!(
            s.condition.contains("PRC_LINKER"),
            "{name}: `{}`",
            s.condition
        );
    }
    // The copy of the example project is the one action that is not the choice's: it always ran, and still does.
    let seed = actions.iter().find(|s| s.action == "SeedExamples").unwrap();
    assert_eq!(seed.condition, "NOT REMOVE");
}

#[test]
fn facts_are_gathered_for_a_wizard_in_the_ui_sequence_and_for_a_silent_install_in_the_execute_sequence(
) {
    let (ui, exec) = (ui(), exec());
    let in_ui = ui
        .iter()
        .find(|s| s.action == "PrcFacts")
        .expect("PrcFacts in the UI sequence");
    assert_eq!(in_ui.condition, "NOT Installed");
    let in_exec = exec
        .iter()
        .find(|s| s.action == "PrcFacts")
        .expect("PrcFacts in the execute sequence");
    assert!(
        in_exec.condition.contains("UILevel < 5") && in_exec.condition.contains("NOT Installed"),
        "a wizard has gathered them already: `{}`",
        in_exec.condition
    );
    // The result is read after the install and only when the developer chose Prepare now.
    let read = ui
        .iter()
        .find(|s| s.action == "PrcReadResult")
        .expect("PrcReadResult");
    assert_eq!(
        read.relation,
        ("After".to_owned(), "ExecuteAction".to_owned())
    );
    assert!(
        read.condition.contains("PRC_PREPARE = \"1\""),
        "{}",
        read.condition
    );
}

#[test]
fn only_the_actions_that_hear_cancel_may_fail_the_install() {
    // PrcRun, PrcTicks: IDCANCEL is their one answer that is not success. Everything else never fails.
    for (id, entry, check) in [
        ("PrcFacts", "PrcFacts", false),
        ("PrcTicks", "PrcTicks", true),
        ("PrcRust", "PrcRun", true),
        ("PrcCompile", "PrcRun", true),
        ("PrcRollback", "PrcRollback", false),
        ("PrcReadResult", "PrcReadResult", false),
    ] {
        let tag = custom_action(id);
        assert_eq!(attr(&tag, "DllEntry"), Some(entry), "{id}");
        assert_eq!(
            attr(&tag, "BinaryKey"),
            Some("PrcCA"),
            "{id} must run the setup library"
        );
        let want = if check { "check" } else { "ignore" };
        assert_eq!(
            attr(&tag, "Return"),
            Some(want),
            "{id}: Return must be {want} (only a Cancel may end the install)"
        );
    }
}

#[test]
fn the_steps_are_the_installing_users_and_the_rollback_is_the_systems() {
    for id in ["PrcRust", "PrcCompile"] {
        let tag = custom_action(id);
        assert_eq!(attr(&tag, "Execute"), Some("deferred"), "{id}");
        assert_eq!(
            attr(&tag, "Impersonate"),
            Some("yes"),
            "{id} runs as the installing user (the cache and the Rust are theirs)"
        );
    }
    let rollback = custom_action("PrcRollback");
    assert_eq!(attr(&rollback, "Execute"), Some("rollback"));
    assert_eq!(attr(&rollback, "Impersonate"), Some("no"));
    // The immediate Set* actions hand the deferred ones their data through a property of the same name.
    for (set, property) in [
        ("SetPrcRust", "PrcRust"),
        ("SetPrcCompile", "PrcCompile"),
        ("SetPrcRollback", "PrcRollback"),
    ] {
        assert_eq!(
            attr(&custom_action(set), "Property"),
            Some(property),
            "{set}"
        );
    }
}

#[test]
fn the_dll_is_the_binary_and_every_entry_point_it_names_exists() {
    assert!(
        WXS.contains("<Binary Id=\"PrcCA\" SourceFile=\"$(var.PrcDll)\" />"),
        "the setup library is the Binary table's entry"
    );
    for tag in custom_actions() {
        if attr(&tag, "BinaryKey") == Some("PrcCA") {
            let entry = attr(&tag, "DllEntry").expect("a DllEntry");
            let (_, source) = EXPORTS
                .iter()
                .find(|(name, _)| *name == entry)
                .unwrap_or_else(|| {
                    panic!("the WiX names {entry}, which the library does not list as exported")
                });
            assert!(
                source.contains(&format!("pub extern \"system\" fn {entry}(")),
                "{entry} is named by the WiX but not defined as an exported entry point"
            );
            assert!(
                source.contains("#[no_mangle]"),
                "{entry}'s file has no #[no_mangle]: the DLL would not export it by its name"
            );
        }
    }
    // The release workflow checks the same five names in the built DLL.
    for (name, _) in EXPORTS {
        assert!(
            WORKFLOW.contains(name),
            "the release workflow's export check must name {name}"
        );
    }
}

/// The data of a step with its properties filled in as the installer would.
fn expanded(id: &str) -> String {
    let tag = custom_action(id);
    let mut value = attr(&tag, "Value")
        .expect("a Value")
        .replace("&quot;", "\"");
    for (name, with) in [
        ("[INSTALLFOLDER]", "C:\\Program Files\\PowerRustCOBOL\\"),
        ("[PRC_TICKS_RUST]", "50"),
        ("[PRC_TICKS_COMPILE]", "950"),
        ("[PRC_RESULT_FILE]", "C:\\T\\result.txt"),
        ("[PRC_CANCEL_FILE]", "C:\\T\\cancel.txt"),
        (
            "[PRC_BUILD_FOLDER]",
            "C:\\Users\\dev\\AppData\\Local\\PowerRustCOBOL\\builds",
        ),
    ] {
        value = value.replace(name, with);
    }
    value
}

#[test]
fn the_data_of_each_step_is_what_the_supervisor_reads() {
    let rust = RunData::parse(&expanded("SetPrcRust")).expect("the Rust step's data parses");
    assert_eq!(rust.spawn, Spawn::AsUser);
    assert_eq!((rust.watchdog_secs, rust.kill_on_cancel), (900, true));
    assert_eq!((rust.part, rust.ticks), (Some(Part::Rust), 50));
    assert_eq!(rust.result_file.as_deref(), Some("C:\\T\\result.txt"));
    assert_eq!(rust.cancel_file.as_deref(), Some("C:\\T\\cancel.txt"));
    assert!(
        rust.command.contains("--prepare-platform --step rust"),
        "{}",
        rust.command
    );

    let compile =
        RunData::parse(&expanded("SetPrcCompile")).expect("the compile step's data parses");
    assert_eq!(compile.spawn, Spawn::AsUser);
    assert_eq!((compile.watchdog_secs, compile.kill_on_cancel), (900, true));
    assert_eq!((compile.part, compile.ticks), (Some(Part::Release), 950));
    assert!(
        compile.command.contains("--step compile"),
        "{}",
        compile.command
    );
    // One build folder for the page, the compile and the rollback.
    assert!(
        compile
            .command
            .contains("--build-root \"C:\\Users\\dev\\AppData\\Local\\PowerRustCOBOL\\builds\""),
        "{}",
        compile.command
    );
    // The ticks of the two steps are the bar's whole total.
    assert_eq!(rust.ticks + compile.ticks, crate::ticks::TOTAL);
    // The rollback is handed the build folder alone.
    assert_eq!(
        attr(&custom_action("SetPrcRollback"), "Value"),
        Some("[PRC_BUILD_FOLDER]")
    );
}

#[test]
fn what_the_actions_read_across_the_client_and_the_server_is_public_and_secure() {
    for name in [
        "PREPARE_PLATFORM",
        "PRC_PINNED_RUST",
        "PRC_PREPARE",
        "PRC_LINKER",
        "PRC_BUILD_FOLDER",
        "PRC_RESULT_FILE",
        "PRC_CANCEL_FILE",
    ] {
        let line = WXS
            .lines()
            .find(|l| l.contains(&format!("<Property Id=\"{name}\"")))
            .unwrap_or_else(|| panic!("{name} is not declared"));
        assert!(
            line.contains("Secure=\"yes\""),
            "{name} must be secure to reach the elevated server: {line}"
        );
    }
    assert!(WXS.contains("<Property Id=\"PRC_PINNED_RUST\" Value=\"$(var.RustVersion)\""));
}

#[test]
fn the_preparation_page_is_reached_only_when_there_is_something_to_prepare() {
    // Every way into the page names PRC_NEED; every way past it names NOT PRC_NEED.
    let mut into = 0;
    for line in WXS
        .lines()
        .filter(|l| l.contains("Value=\"PrcPrepareDlg\""))
    {
        into += 1;
        let cond = condition(
            line.split_once('>')
                .map_or("", |(_, rest)| rest)
                .split("</Publish>")
                .next()
                .unwrap_or(""),
        );
        assert!(
            cond.contains("PRC_NEED") && !cond.contains("NOT PRC_NEED")
                || cond.contains("NOT Installed AND PRC_NEED"),
            "a way into PrcPrepareDlg without PRC_NEED: `{cond}`  ({line})"
        );
    }
    assert_eq!(
        into, 2,
        "one way in from the folder page and one back from 'ready'"
    );
    // It is never shown by the UI sequence itself (a repair and a removal must not meet it).
    let ui_block = block(WXS, "InstallUISequence");
    assert!(
        !ui_block.contains("PrcPrepareDlg"),
        "the page is reached through the wizard's buttons only"
    );
    // The page's own Next clears or sets what the cancel confirmation adds.
    assert!(WXS.contains("Property=\"PRC_CANCEL_NOTE\" Value=\"[PRC_CANCEL_TEXT]\""));
    assert!(WXL.contains("[PRC_CANCEL_NOTE]") && WXL.contains("Id=\"CancelDlgText\""));
}

#[test]
fn the_source_is_ascii_because_the_database_code_page_is_not_utf8() {
    for (what, text) in [("product.wxs", WXS), ("prc-strings.wxl", WXL)] {
        if let Some((line, bad)) = text
            .lines()
            .enumerate()
            .find_map(|(n, l)| l.chars().find(|c| !c.is_ascii()).map(|c| (n + 1, c)))
        {
            panic!("{what} line {line} holds {bad:?}: light refuses what code page 1252 cannot represent (LGHT0311)");
        }
    }
    // XML comments may not hold a double hyphen.
    let mut rest = WXS;
    while let Some(start) = rest.find("<!--") {
        let after = &rest[start + 4..];
        let end = after.find("-->").expect("a comment is closed");
        assert!(
            !after[..end].contains("--"),
            "a double hyphen inside a comment: {}",
            &after[..end.min(60)]
        );
        rest = &after[end + 3..];
    }
}

#[test]
fn the_release_workflow_builds_the_library_and_the_installer_the_way_the_wix_header_says() {
    // The library: the C runtime linked in, its own target folder, the exports and the dependencies checked.
    for needle in [
        "RUSTFLAGS: \"-C target-feature=+crt-static\"",
        "CARGO_TARGET_DIR: target-setup",
        "cargo build --release -p cobolt-setup --lib",
        "vcruntime140",
        "dist/prc/cobolt_setup.dll",
    ] {
        assert!(
            WORKFLOW.contains(needle),
            "the release workflow lost `{needle}`"
        );
    }
    // The installer: this file, the Rust it pins, the library, and the one overridden string.
    for needle in [
        "crates/cobolt-setup/wix/product.wxs",
        "-dRustVersion=\"$RELEASE_RUST\"",
        "-dPrcDll=dist/prc/cobolt_setup.dll",
        "-loc crates/cobolt-setup/wix/prc-strings.wxl",
        "-cultures:en-us",
    ] {
        assert!(
            WORKFLOW.contains(needle),
            "the release workflow lost `{needle}`"
        );
    }
    // The heredoc is gone: the file is the only copy.
    assert!(
        !WORKFLOW.contains("cat > product.wxs"),
        "the installer's source must live in one place"
    );
}

// ── The bundle (task T19) ────────────────────────────────────────────────────────────────────────────────────────

const BUNDLE: &str = include_str!("../wix/bundle.wxs");
const BUNDLE_WXL: &str = include_str!("../wix/bundle.wxl");
const MANIFEST: &str = include_str!("../Cargo.toml");

/// Every `<ExitCode Value=".." Behavior=".."/>` row of the bundle, as `(value, behavior)`.
fn exit_rows() -> Vec<(u32, String)> {
    BUNDLE
        .split("<ExitCode ")
        .skip(1)
        .map(|part| {
            let tag = format!(
                " {}",
                &part[..part.find('>').expect("an <ExitCode> tag is closed")]
            );
            (
                attr(&tag, "Value")
                    .expect("a Value")
                    .parse()
                    .expect("a number"),
                attr(&tag, "Behavior").expect("a Behavior").to_owned(),
            )
        })
        .collect()
}

#[test]
fn every_exit_code_the_build_tools_program_can_return_is_a_row_of_the_bundle() {
    use crate::buildtools::{decide, Run, BUSY, CANCELLED, RESTART_REQUIRED};
    use crate::prepare::Linker;

    // The rows the bundle declares: success, a restart for Windows to schedule, and the two that end the bundle.
    let rows = exit_rows();
    assert_eq!(
        rows,
        [
            (0, "success".to_owned()),
            (RESTART_REQUIRED, "scheduleReboot".to_owned()),
            (CANCELLED, "error".to_owned()),
            (BUSY, "error".to_owned()),
        ],
        "the ExitCode rows of bundle.wxs and the program's constants must agree"
    );

    // And the program returns nothing else, for any ending of Microsoft's installer.
    let mut codes: Vec<u32> = (0..=9999).collect();
    codes.extend([u32::MAX, 0x8000_4005]);
    let mut returned = std::collections::BTreeSet::new();
    for code in codes {
        for linker in [Linker::Present, Linker::Missing] {
            returned.insert(decide(&Run::Exited(code), linker).exit);
        }
    }
    returned.insert(decide(&Run::TimedOut, Linker::Missing).exit);
    returned.insert(decide(&Run::NotRun("x".into()), Linker::Missing).exit);
    let declared: std::collections::BTreeSet<u32> = rows.iter().map(|(v, _)| *v).collect();
    assert_eq!(
        returned, declared,
        "the program returned an exit code the bundle has no row for (or a row is never returned)"
    );
}

#[test]
fn the_bundle_runs_the_build_tools_program_first_and_then_the_msi_with_its_own_wizard() {
    let exe = BUNDLE
        .find("<ExePackage ")
        .expect("the Build Tools package");
    let msi = BUNDLE.find("<MsiPackage ").expect("the .msi package");
    assert!(
        exe < msi,
        "the build tools must be installed before the .msi runs"
    );

    let exe_tag = &BUNDLE[exe..exe + BUNDLE[exe..].find('>').unwrap()];
    let msi_tag = &BUNDLE[msi..msi + BUNDLE[msi..].find('>').unwrap()];
    let (exe_tag, msi_tag) = (format!(" {exe_tag}"), format!(" {msi_tag}"));
    // The program the package carries is the binary the crate builds, never uninstalled, never skipped.
    assert_eq!(attr(&exe_tag, "Name"), Some("prc-buildtools.exe"));
    assert!(
        MANIFEST.contains("name = \"prc-buildtools\""),
        "Cargo.toml builds a different program than the bundle carries"
    );
    assert_eq!(attr(&exe_tag, "Permanent"), Some("yes"));
    assert_eq!(attr(&exe_tag, "Vital"), Some("yes"));
    assert_eq!(attr(&exe_tag, "PerMachine"), Some("yes"));
    // The .msi shows the wizard it has when installed alone, and the bundle's entry replaces its own.
    assert_eq!(attr(&msi_tag, "DisplayInternalUI"), Some("yes"));
    assert!(BUNDLE.contains("Name=\"ARPSYSTEMCOMPONENT\" Value=\"1\""));
    // The silent opt-in goes through, and can be given on the bundle's command line.
    assert!(BUNDLE.contains("<MsiProperty Name=\"PREPARE_PLATFORM\" Value=\"[PREPARE_PLATFORM]\""));
    let variable = BUNDLE
        .split("<Variable ")
        .nth(1)
        .expect("the PREPARE_PLATFORM variable");
    assert!(
        variable.starts_with("Name=\"PREPARE_PLATFORM\"")
            && variable.contains("bal:Overridable=\"yes\"")
    );
    // The same property is the one the .msi reads (a public property, set on a silent install).
    assert!(
        WXS.contains("<Property Id=\"PREPARE_PLATFORM\""),
        "product.wxs no longer declares the PREPARE_PLATFORM property the bundle passes"
    );
}

#[test]
fn the_bundles_page_asks_for_microsofts_terms_and_says_what_setup_will_do() {
    assert!(
        BUNDLE.contains("LicenseUrl=\"https://"),
        "the box needs a link to accept"
    );
    for id in ["InstallLicenseLinkText", "InstallAcceptCheckbox"] {
        assert!(
            BUNDLE_WXL.contains(&format!("Id=\"{id}\"")),
            "bundle.wxl no longer overrides {id}"
        );
    }
    assert!(
        BUNDLE_WXL.contains("&lt;a href=&quot;#&quot;&gt;"),
        "the paragraph needs the link WixStdBA opens (the bundle's LicenseUrl)"
    );
    for what in [
        "Microsoft C++ build tools",
        "downloads and installs them",
        "about 2 GB",
    ] {
        assert!(BUNDLE_WXL.contains(what), "the page lost: {what}");
    }
}

#[test]
fn the_bundle_source_is_ascii_and_its_comments_are_well_formed() {
    for (what, text) in [("bundle.wxs", BUNDLE), ("bundle.wxl", BUNDLE_WXL)] {
        if let Some((line, bad)) = text
            .lines()
            .enumerate()
            .find_map(|(n, l)| l.chars().find(|c| !c.is_ascii()).map(|c| (n + 1, c)))
        {
            panic!("{what} line {line} holds {bad:?}");
        }
        let mut rest = text;
        while let Some(start) = rest.find("<!--") {
            let after = &rest[start + 4..];
            let end = after.find("-->").expect("a comment is closed");
            assert!(
                !after[..end].contains("--"),
                "{what}: a double hyphen inside a comment: {}",
                &after[..end.min(60)]
            );
            rest = &after[end + 3..];
        }
    }
}

#[test]
fn the_release_workflow_builds_the_bundle_after_the_msi_and_publishes_both() {
    for needle in [
        "cargo build --release -p cobolt-setup --lib --bin prc-buildtools",
        "prc-buildtools.exe needs",
        "dist/prc/prc-buildtools.exe",
        "crates/cobolt-setup/wix/bundle.wxs",
        "-ext WixBalExtension",
        "-dBuildToolsExe=dist/prc/prc-buildtools.exe",
        "-loc crates/cobolt-setup/wix/bundle.wxl",
        "\"dist/${PKG}-setup.exe\"",
        "-name '*-setup.exe'",
        "windows-x86_64-setup.exe",
        "windows-x86_64.msi",
    ] {
        assert!(
            WORKFLOW.contains(needle),
            "the release workflow lost `{needle}`"
        );
    }
    let msi = WORKFLOW
        .find("Build the installer (.msi)")
        .expect("the .msi step");
    let bundle = WORKFLOW
        .find("Build the setup bundle (-setup.exe)")
        .expect("the bundle step");
    assert!(
        msi < bundle,
        "the bundle chains the .msi, so it is built after it"
    );
}
