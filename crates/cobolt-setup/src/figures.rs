// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! The numbers the setup's page shows: time, disk and download sizes (R19).
//!
//! They are constants in ONE place so that the page, the free-space rule and
//! the tests read the same values, and so the day they are measured (task T16,
//! on a reference Windows machine, with the system's default protection on)
//! they are changed here and nowhere else.
//!
//! **Until then they are estimates, and the page says so.** [`Figures::measured`]
//! is `false`, the page words every figure as an estimate, and an ignored test
//! (`the_figures_are_measured`) stays as the visible gate: the feature is not
//! releasable while it is ignored.

/// A megabyte, as the page counts them (decimal: it is what download pages say).
pub const MB: u64 = 1_000_000;
/// A gigabyte, as the page counts them.
pub const GB: u64 = 1_000 * MB;

/// What the preparation takes and uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Figures {
    /// `true` only when every number below came from a measured run (task T16).
    pub measured: bool,
    /// Minutes the compile of both profiles takes on a modest machine, at full speed.
    pub compile_minutes: u32,
    /// What the compiled platform occupies in the developer's build folder: release and debug, every variant.
    pub cache_bytes: u64,
    /// The Rust toolchain: download, and what it occupies once installed (in the user's profile).
    pub rust_download_bytes: u64,
    /// See [`Figures::rust_download_bytes`].
    pub rust_installed_bytes: u64,
    /// The Visual Studio Build Tools: download, and what they occupy on the system volume.
    pub build_tools_download_bytes: u64,
    /// See [`Figures::build_tools_download_bytes`].
    pub build_tools_installed_bytes: u64,
    /// Free space kept beyond what is needed on a volume, so a build never meets a full disk.
    pub margin_bytes: u64,
}

/// The figures in force.
///
/// ESTIMATES, from what the project already knows and not from a run of this
/// feature: the shipped third-party libraries are 0.8 to 0.96 GB per platform in
/// release 2.0.613 (release profile only; debug is larger); the hosted Windows
/// runner compiles the release libraries in 38 minutes (release 2.0.613); the
/// Build Tools are "multi-gigabyte" by Microsoft's own description. Task T16
/// replaces every one of them with a measurement.
pub const FIGURES: Figures = Figures {
    measured: false,
    compile_minutes: 60,
    cache_bytes: 5 * GB,
    rust_download_bytes: 250 * MB,
    rust_installed_bytes: 600 * MB,
    build_tools_download_bytes: 1_500 * MB,
    build_tools_installed_bytes: 6 * GB,
    margin_bytes: 2 * GB,
};

/// A size in words a person reads: `250 MB`, `1.5 GB`, `6 GB`.
pub fn human(bytes: u64) -> String {
    if bytes < GB {
        // Whole megabytes, rounded up to the next ten so a figure never promises less than it takes.
        let mb = bytes.div_ceil(MB);
        return format!("{} MB", mb.div_ceil(10) * 10);
    }
    let tenths = bytes.saturating_mul(10).div_ceil(GB);
    if tenths.is_multiple_of(10) {
        format!("{} GB", tenths / 10)
    } else {
        format!("{}.{} GB", tenths / 10, tenths % 10)
    }
}

/// A duration in words: `about an hour`, `about 45 minutes`, `about 1 hour 30 minutes`.
pub fn duration_words(minutes: u32) -> String {
    match minutes {
        0 => "no time".to_owned(),
        1 => "about a minute".to_owned(),
        2..=89 if (55..=65).contains(&minutes) => "about an hour".to_owned(),
        2..=89 => format!("about {minutes} minutes"),
        _ => {
            let (h, m) = (minutes / 60, minutes % 60);
            let hours = if h == 1 {
                "1 hour".to_owned()
            } else {
                format!("{h} hours")
            };
            if m == 0 {
                format!("about {hours}")
            } else {
                format!("about {hours} {m} minutes")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_said_the_way_a_download_page_says_them() {
        let cases = [
            (0, "0 MB"),
            (1, "10 MB"),
            (250 * MB, "250 MB"),
            (251 * MB, "260 MB"),
            (GB, "1 GB"),
            (1_500 * MB, "1.5 GB"),
            (6 * GB, "6 GB"),
            (6 * GB + 1, "6.1 GB"),
            (123 * GB, "123 GB"),
        ];
        for (bytes, words) in cases {
            assert_eq!(human(bytes), words, "{bytes} bytes");
        }
    }

    #[test]
    fn durations_are_said_in_plain_words() {
        let cases = [
            (1, "about a minute"),
            (45, "about 45 minutes"),
            (55, "about an hour"),
            (60, "about an hour"),
            (65, "about an hour"),
            (70, "about 70 minutes"),
            (90, "about 1 hour 30 minutes"),
            (120, "about 2 hours"),
            (150, "about 2 hours 30 minutes"),
        ];
        for (minutes, words) in cases {
            assert_eq!(duration_words(minutes), words, "{minutes} minutes");
        }
    }

    /// The visible gate of R19. It stays ignored until task T16 has measured the figures on a reference
    /// Windows machine and written them into [`FIGURES`]; the feature is not releasable before that.
    #[test]
    #[ignore = "T16: the figures are estimates until they are measured on a reference Windows machine"]
    #[allow(clippy::assertions_on_constants)]
    fn the_figures_are_measured() {
        assert!(FIGURES.measured, "FIGURES still holds estimates");
    }
}
