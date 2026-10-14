// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Is there a newer PowerRustCOBOL release than the one running?
//!
//! Asked once per start-up, on a background thread, against the repository's
//! GitHub releases. A newer release invites the developer to download it; a
//! refusal is **not** remembered, so the question is asked again the next time
//! the IDE starts (operator, 2026-09-14). Nothing is downloaded or installed
//! automatically — the developer is sent to the release page and chooses.
//!
//! Every failure is silent. Offline, rate-limited, behind a proxy that eats the
//! request, a GitHub response shaped differently than expected: all of them mean
//! "no update to offer", never a dialog and never a log line the developer has
//! to dismiss. An update check that interrupts the work it interrupts for is
//! worse than no update check.
//!
//! **The version offered is the one the release's files carry, not the one its
//! tag claims.** A release is made by hand: the tag is typed when the workflow is
//! run, and the files inside are named by the build (`PowerRustCOBOL-1.91.38-…`).
//! On 2026-10-14 the latest release was tagged `1.91.138` and held 1.91.38 files;
//! comparing the tag told every build below 1.91.138 that it was out of date and
//! offered a download older than the copy it replaced. The file names are what
//! would be installed, so they decide ([`release_from`]).

use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

/// Where releases live. The repository is fixed: this asks about *this*
/// product, not about whatever a setting points at.
const RELEASES_API: &str =
    "https://api.github.com/repos/CloudLandBeta/PowerRustCOBOL/releases/latest";

/// Short: this runs while the IDE is starting, and a slow network must never be
/// something the developer notices.
const TIMEOUT: Duration = Duration::from_secs(10);

/// A release newer than the running build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    /// The release's version (`1.70.29`): the one its files are named for, or —
    /// when no file names a version — the one its tag spells.
    pub version: String,
    /// The release's human name, when it has one.
    pub name: String,
    /// The release page — where the developer is sent. Always populated.
    pub page_url: String,
    /// The installer for *this* machine, when one is unambiguous. Shown so the
    /// developer knows which of nine files to take; `None` on a platform where
    /// the choice is theirs (Linux ships `.deb`, `.rpm` and `.tar.gz`).
    pub asset_name: Option<String>,
}

/// Start the check. The receiver yields exactly one message: `Some` when a
/// newer release exists, `None` for every other outcome including failure.
///
/// Never blocks the caller, and the thread is detached — if the IDE closes
/// before the reply arrives, the send fails harmlessly into a dropped channel.
pub fn spawn_check(current: &str) -> Receiver<Option<ReleaseInfo>> {
    let (tx, rx) = channel();
    let current = current.to_owned();
    std::thread::spawn(move || {
        let _ = tx.send(fetch_latest().filter(|r| is_newer(&r.version, &current)));
    });
    rx
}

/// Ask GitHub. `None` on any failure at all — see the module note.
fn fetch_latest() -> Option<ReleaseInfo> {
    // Mirror `external_crates_service`: ureq with default features off has no
    // TLS until a connector is handed to it explicitly.
    let mut builder = ureq::AgentBuilder::new()
        .timeout(TIMEOUT)
        .user_agent(&format!("PowerRustCOBOL/{}", crate::version::VERSION));
    if let Ok(c) = native_tls::TlsConnector::new() {
        builder = builder.tls_connector(std::sync::Arc::new(c));
    }
    let body: serde_json::Value = builder
        .build()
        .get(RELEASES_API)
        .set("Accept", "application/vnd.github+json")
        .call()
        .ok()?
        .into_json()
        .ok()?;

    // A draft is not published and a prerelease is not for general use; neither
    // is something to interrupt a developer about.
    if body.get("draft").and_then(|v| v.as_bool()) == Some(true)
        || body.get("prerelease").and_then(|v| v.as_bool()) == Some(true)
    {
        return None;
    }

    let tag = body.get("tag_name")?.as_str()?.trim().to_owned();
    let page_url = body.get("html_url")?.as_str()?.to_owned();
    let name = body.get("name").and_then(|v| v.as_str()).map(str::to_owned);
    let assets: Vec<String> = body
        .get("assets")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.get("name")?.as_str().map(|s| s.to_owned()))
                .collect()
        })
        .unwrap_or_default();
    release_from(&tag, name.as_deref(), page_url, assets)
}

/// What the files of a release say its version is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FilesVersion {
    /// No file is named for a version (no files at all, or only `SHA256SUMS`).
    None,
    /// Every file that names a version names this one.
    One(String),
    /// The files name different versions: a release assembled from two builds.
    Disagree,
}

/// The version the files of a release are named for. The build names them
/// `PowerRustCOBOL-<x.y.z>-<platform>.<ext>`; a name of any other shape (the
/// checksum list) says nothing and is skipped.
fn version_of_files(assets: &[String]) -> FilesVersion {
    let mut found: Vec<(String, (u32, u32, u32))> = Vec::new();
    for name in assets {
        let Some(rest) = name.strip_prefix("PowerRustCOBOL-") else { continue };
        let Some((version, _)) = rest.split_once('-') else { continue };
        // Strictly three numbers: `1.91.38`, not `1.91` and not `1.91.38rc`.
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() != 3 || !parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())) {
            continue;
        }
        let Some(parsed) = parse_version(version) else { continue };
        if !found.iter().any(|(_, p)| *p == parsed) {
            found.push((version.to_owned(), parsed));
        }
    }
    match found.len() {
        0 => FilesVersion::None,
        1 => FilesVersion::One(found.remove(0).0),
        _ => FilesVersion::Disagree,
    }
}

/// Turn what GitHub said about a release into an offer — or into nothing.
///
/// The **files** decide the version. They are what the developer would install,
/// and a tag is typed by a person: tagged `1.91.138` over 1.91.38 files, the
/// release is a 1.91.38 build and is offered, shown and compared as one. A tag
/// that agrees with its files changes nothing but its spelling (`v1.70.29` is
/// shown as `1.70.29`).
///
/// * the files name one version → that version, whatever the tag says;
/// * no file names a version → the tag, as before: there is nothing else to go by;
/// * the files name **different** versions → nothing. A release assembled from two
///   builds is not one to send anybody to, and this module's rule for what it
///   cannot understand is silence.
fn release_from(tag: &str, name: Option<&str>, page_url: String, assets: Vec<String>) -> Option<ReleaseInfo> {
    let version = match version_of_files(&assets) {
        FilesVersion::One(v) => v,
        FilesVersion::None => tag.trim().to_owned(),
        FilesVersion::Disagree => return None,
    };
    Some(ReleaseInfo {
        asset_name: asset_for_this_platform(&assets),
        name: name.unwrap_or(&version).to_owned(),
        version,
        page_url,
    })
}

/// Is `latest` a higher version than `current`?
///
/// Compared **numerically, component by component** — never as text. The
/// releases are `x.y.z` and a string comparison puts `1.70.9` above `1.70.28`,
/// which would offer a developer on 28 an "update" to 9.
///
/// All three levels count (operator, 2026-09-14, revising an earlier
/// major/minor-only instruction): a fix release is a new binary and is offered
/// like any other.
///
/// Anything unparseable answers `false`. A tag this version does not understand
/// is not a reason to send someone to a download page.
pub fn is_newer(latest: &str, current: &str) -> bool {
    let (Some(a), Some(b)) = (parse_version(latest), parse_version(current)) else {
        return false;
    };
    a > b
}

/// `1.70.28` → `(1, 70, 28)`. Tolerates a `v` prefix and trailing text after
/// the patch (`1.70.28-rc1`), because a tag is written by a human.
fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let s = s.trim().trim_start_matches(['v', 'V']);
    let mut it = s.split('.');
    let major = it.next()?.trim().parse().ok()?;
    let minor = it.next()?.trim().parse().ok()?;
    // `28`, `28-rc1` and `28+build` all mean patch 28.
    let rest = it.next().unwrap_or("0");
    let digits: String = rest.trim().chars().take_while(|c| c.is_ascii_digit()).collect();
    let patch = digits.parse().unwrap_or(0);
    Some((major, minor, patch))
}

/// The one file to point at on this machine, when that is unambiguous.
///
/// macOS and Windows each ship one installer, so naming it saves the developer
/// picking from nine. Linux ships `.deb`, `.rpm` and `.tar.gz` and the right
/// answer depends on a distribution this cannot detect, so it names none and
/// the release page does the explaining.
fn asset_for_this_platform(assets: &[String]) -> Option<String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let want_ext = match os {
        "macos" => ".dmg",
        "windows" => ".msi",
        _ => return None,
    };
    let os_tag = if os == "macos" { "macos" } else { "windows" };
    assets
        .iter()
        .find(|n| n.contains(os_tag) && n.contains(arch) && n.ends_with(want_ext))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All three levels are offered — the operator revised an earlier
    /// major/minor-only instruction on 2026-09-14.
    #[test]
    fn a_newer_release_at_any_level_is_offered() {
        assert!(is_newer("1.70.29", "1.70.28"), "fix");
        assert!(is_newer("1.71.0", "1.70.28"), "minor");
        assert!(is_newer("2.0.0", "1.70.28"), "major");
    }

    /// The case that happens on the developer's own machine every day: the
    /// build in front of them is ahead of anything released.
    #[test]
    fn a_build_newer_than_the_release_is_never_offered_an_update() {
        assert!(!is_newer("1.70.12", "1.70.28"));
        assert!(!is_newer("1.70.28", "1.70.28"), "equal is not newer");
        assert!(!is_newer("1.69.99", "1.70.0"));
    }

    /// The reason this is not a string comparison. Lexically "9" > "2", so
    /// text ordering offers someone on 1.70.28 an "update" to 1.70.9.
    #[test]
    fn versions_compare_numerically_not_as_text() {
        assert!(!is_newer("1.70.9", "1.70.28"));
        assert!(is_newer("1.70.28", "1.70.9"));
        assert!(is_newer("1.100.0", "1.99.0"));
    }

    /// A tag this version cannot read is not a reason to send anyone anywhere.
    #[test]
    fn an_unreadable_tag_offers_nothing() {
        assert!(!is_newer("latest", "1.70.28"));
        assert!(!is_newer("", "1.70.28"));
        assert!(!is_newer("1.70.29", "not-a-version"));
    }

    #[test]
    fn a_v_prefix_and_a_suffix_are_tolerated() {
        assert_eq!(parse_version("v1.70.28"), Some((1, 70, 28)));
        assert_eq!(parse_version("1.70.28-rc1"), Some((1, 70, 28)));
        assert_eq!(parse_version("1.70"), Some((1, 70, 0)));
    }

    /// Naming the wrong file is worse than naming none, so an architecture that
    /// does not match is not offered.
    #[test]
    fn the_named_asset_matches_this_machine_or_there_is_none() {
        let assets: Vec<String> = [
            "PowerRustCOBOL-1.70.29-linux-x86_64.deb",
            "PowerRustCOBOL-1.70.29-macos-aarch64.dmg",
            "PowerRustCOBOL-1.70.29-macos-x86_64.dmg",
            "PowerRustCOBOL-1.70.29-windows-x86_64.msi",
            "SHA256SUMS",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        match asset_for_this_platform(&assets) {
            Some(name) => {
                assert!(name.contains(std::env::consts::ARCH), "wrong arch: {name}");
                assert!(
                    name.ends_with(".dmg") || name.ends_with(".msi"),
                    "not an installer: {name}"
                );
            }
            // Linux, or an architecture this release does not ship.
            None => assert!(
                std::env::consts::OS != "macos" && std::env::consts::OS != "windows"
                    || !assets.iter().any(|a| a.contains(std::env::consts::ARCH)),
            ),
        }
    }

    /// An empty asset list must not panic or invent a file name.
    #[test]
    fn no_assets_names_no_file() {
        assert_eq!(asset_for_this_platform(&[]), None);
    }

    /// A release as GitHub lists it: the nine files of one build and the checksums.
    fn files_of(version: &str) -> Vec<String> {
        let mut v: Vec<String> = [
            "linux-x86_64.deb",
            "linux-x86_64.rpm",
            "linux-x86_64.tar.gz",
            "macos-aarch64.dmg",
            "macos-aarch64.tar.gz",
            "macos-x86_64.dmg",
            "macos-x86_64.tar.gz",
            "windows-x86_64.msi",
            "windows-x86_64.zip",
        ]
        .iter()
        .map(|p| format!("PowerRustCOBOL-{version}-{p}"))
        .collect();
        v.push("SHA256SUMS".to_owned());
        v
    }

    fn offered(tag: &str, files: Vec<String>, running: &str) -> Option<String> {
        release_from(tag, Some("a release"), "https://example/r".to_owned(), files)
            .filter(|r| is_newer(&r.version, running))
            .map(|r| r.version)
    }

    /// The release of 2026-10-14, exactly: tagged `1.91.138`, holding 1.91.38 files.
    #[test]
    fn a_tag_that_does_not_match_its_files_is_not_believed() {
        let files = files_of("1.91.38");
        // The copy that was shown the dialog (1.91.39) is newer than what the files are.
        assert_eq!(offered("1.91.138", files.clone(), "1.91.39"), None);
        // …a build that really is older is offered the files' version, not the tag's.
        assert_eq!(offered("1.91.138", files.clone(), "1.91.30"), Some("1.91.38".to_owned()));
        // The tag alone would have offered 1.91.138 to both.
        assert!(is_newer("1.91.138", "1.91.39"));
        println!(
            "tag 1.91.138 over 1.91.38 files → running 1.91.39: {:?}; running 1.91.30: {:?}",
            offered("1.91.138", files.clone(), "1.91.39"),
            offered("1.91.138", files, "1.91.30")
        );
    }

    /// The files are the truth in the other direction too: a tag lower than its files.
    #[test]
    fn a_tag_lower_than_its_files_offers_the_files_version() {
        assert_eq!(offered("1.91.38", files_of("1.91.138"), "1.91.39"), Some("1.91.138".to_owned()));
    }

    /// A tag that agrees with its files changes nothing except how it is spelled.
    #[test]
    fn a_tag_that_matches_its_files_offers_what_it_always_did() {
        assert_eq!(offered("1.91.50", files_of("1.91.50"), "1.91.39"), Some("1.91.50".to_owned()));
        assert_eq!(offered("v1.91.50", files_of("1.91.50"), "1.91.39"), Some("1.91.50".to_owned()), "shown without the v");
        assert_eq!(offered("1.91.50", files_of("1.91.50"), "1.91.50"), None, "the running version is not an update");
    }

    /// Nothing in the files names a version: the tag is all there is, as before.
    #[test]
    fn a_release_with_no_versioned_file_is_judged_by_its_tag() {
        assert_eq!(offered("1.91.50", vec![], "1.91.39"), Some("1.91.50".to_owned()));
        assert_eq!(offered("1.91.50", vec!["SHA256SUMS".to_owned()], "1.91.39"), Some("1.91.50".to_owned()));
        // A name of another shape says nothing either.
        assert_eq!(
            offered("1.91.50", vec!["notes.txt".to_owned(), "PowerRustCOBOL-1.91-macos.dmg".to_owned()], "1.91.39"),
            Some("1.91.50".to_owned())
        );
    }

    /// A release assembled from two builds is nobody's update.
    #[test]
    fn files_that_disagree_with_each_other_offer_nothing() {
        let mut files = files_of("1.91.50");
        files.push("PowerRustCOBOL-1.91.38-windows-x86_64.msi".to_owned());
        assert_eq!(offered("1.91.50", files, "1.91.39"), None);
    }

    #[test]
    fn the_version_of_the_files_reads_the_build_names_and_nothing_else() {
        assert_eq!(version_of_files(&files_of("1.80.188")), FilesVersion::One("1.80.188".to_owned()));
        assert_eq!(version_of_files(&["SHA256SUMS".to_owned()]), FilesVersion::None);
        assert_eq!(version_of_files(&["PowerRustCOBOL-1.91.38rc-macos.dmg".to_owned()]), FilesVersion::None, "strictly x.y.z");
        assert_eq!(version_of_files(&["PowerRustCOBOL-v1.91.38-macos.dmg".to_owned()]), FilesVersion::None);
        assert_eq!(version_of_files(&[]), FilesVersion::None);
    }

    /// The installer is still named for this machine.
    #[test]
    fn the_installer_named_is_the_one_in_the_files() {
        let r = release_from("1.91.138", None, "https://example/r".to_owned(), files_of("1.91.38")).unwrap();
        assert_eq!(r.version, "1.91.38");
        if let Some(a) = &r.asset_name {
            assert!(a.contains("1.91.38"), "{a}");
        }
        assert_eq!(r.name, "1.91.38", "with no release name, the version stands in");
    }
}
