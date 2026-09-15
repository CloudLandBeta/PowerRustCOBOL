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
    /// The release's version, as its tag spells it (`1.70.29`).
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

    let version = body.get("tag_name")?.as_str()?.trim().to_owned();
    let page_url = body.get("html_url")?.as_str()?.to_owned();
    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(&version)
        .to_owned();
    let assets: Vec<String> = body
        .get("assets")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.get("name")?.as_str().map(|s| s.to_owned()))
                .collect()
        })
        .unwrap_or_default();

    Some(ReleaseInfo {
        asset_name: asset_for_this_platform(&assets),
        version,
        name,
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
}
