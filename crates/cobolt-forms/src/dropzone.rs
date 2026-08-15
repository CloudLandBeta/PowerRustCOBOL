// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What a FileDropZone accepts, and where it puts it.
//!
//! A drop zone used to take whatever was dropped on it and report the paths
//! where they lay. A form that only wants spreadsheets, or that must not be
//! handed a two-gigabyte video, or that wants its intake gathered into one
//! folder, had to write all of that itself in COBOL — before it could even see
//! the file names.
//!
//! Three properties answer those, and this module is the whole of their logic:
//!
//! | Property | Meaning |
//! |----------|---------|
//! | `AllowedExtensions` | `xlsx, csv` — what the zone takes. Empty accepts anything. |
//! | `MaximumFileSizeKB` | Largest file the zone takes, in KB. `0` is no limit. |
//! | `DestinationFolder` | Where accepted files are copied. Empty leaves them where they are. |
//!
//! Every decision here is a pure function of the properties and the file, so
//! the rules can be tested without a form, a drop or a filesystem.

use std::path::{Path, PathBuf};

/// Why a dropped file was turned away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    /// Its extension is not in `AllowedExtensions`.
    Extension,
    /// It is larger than `MaximumFileSizeKB`.
    TooBig,
}

impl Rejection {
    /// The word a handler reads next to the file name in `RejectedFiles`.
    pub fn as_str(self) -> &'static str {
        match self {
            Rejection::Extension => "extension",
            Rejection::TooBig => "too-big",
        }
    }
}

/// The extensions a filter admits, lower-cased and without dots.
///
/// Written the way a developer would: `"xlsx, csv"`, `".XLSX .CSV"`,
/// `"xlsx;csv"` all mean the same two.
pub fn parse_extensions(filter: &str) -> Vec<String> {
    filter
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .map(|s| s.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Does `path` carry one of the filter's extensions? An empty filter accepts
/// everything, including a file with no extension at all.
pub fn extension_allowed(filter: &str, path: &str) -> bool {
    let wanted = parse_extensions(filter);
    if wanted.is_empty() {
        return true;
    }
    match Path::new(path).extension().and_then(|e| e.to_str()) {
        Some(ext) => wanted.iter().any(|w| w == &ext.to_ascii_lowercase()),
        None => false,
    }
}

/// Is `size_bytes` within `max_kb`? `0` (or a negative) means no limit.
pub fn size_allowed(max_kb: i64, size_bytes: u64) -> bool {
    if max_kb <= 0 {
        return true;
    }
    size_bytes <= (max_kb as u64).saturating_mul(1024)
}

/// Where a file lands in `folder`, without overwriting anything already there:
/// `report.csv`, then `report (2).csv`, then `report (3).csv`.
///
/// `exists` answers "is this path taken?" — the filesystem at runtime, a fixture
/// in the tests. Returns `None` only when the source has no file name.
pub fn destination_path(
    folder: &Path,
    source: &Path,
    exists: &dyn Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let name = source.file_name()?;
    let candidate = folder.join(name);
    if !exists(&candidate) {
        return Some(candidate);
    }
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_owned();
    let ext = source.extension().and_then(|s| s.to_str()).unwrap_or("");
    for n in 2..1000 {
        let file = if ext.is_empty() {
            format!("{stem} ({n})")
        } else {
            format!("{stem} ({n}).{ext}")
        };
        let candidate = folder.join(file);
        if !exists(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// One dropped file, once the zone has looked at it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Kept — at this path. The destination when the file was copied, the
    /// source when `DestinationFolder` is empty.
    Accepted(String),
    /// Turned away, with the reason.
    Rejected(String, Rejection),
}

/// Apply `AllowedExtensions` and `MaximumFileSizeKB` to one file.
///
/// `size_of` answers the file's size in bytes; `None` (unreadable, or a path
/// the host cannot stat) is treated as within the limit, since a zone must not
/// silently swallow a file it merely failed to measure.
pub fn judge(
    filter: &str,
    max_kb: i64,
    path: &str,
    size_of: &dyn Fn(&str) -> Option<u64>,
) -> Result<(), Rejection> {
    if !extension_allowed(filter, path) {
        return Err(Rejection::Extension);
    }
    if let Some(size) = size_of(path) {
        if !size_allowed(max_kb, size) {
            return Err(Rejection::TooBig);
        }
    }
    Ok(())
}

/// What a zone did with one drop.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Intake {
    /// Where each accepted file now is — the copy in `DestinationFolder`, or
    /// the original path when no destination is set. This is `DroppedFiles`.
    pub accepted: Vec<String>,
    /// The files turned away, each with its reason. This is `RejectedFiles`.
    pub rejected: Vec<(String, Rejection)>,
}

impl Intake {
    /// `RejectedFiles`, one `path<TAB>reason` line per refusal — the shape a
    /// COBOL handler can UNSTRING without guessing where the reason starts.
    pub fn rejected_lines(&self) -> Vec<String> {
        self.rejected
            .iter()
            .map(|(p, r)| format!("{p}\t{}", r.as_str()))
            .collect()
    }
}

/// Run a drop through the zone's rules: judge every file, then copy what it
/// accepts into `destination` (when one is set).
///
/// A file the zone accepts but cannot copy — the folder is unwritable, the disk
/// is full — is reported at its ORIGINAL path rather than dropped: the form
/// still gets the file it was given, and the developer's handler still runs.
pub fn take_files(paths: &[String], filter: &str, max_kb: i64, destination: &str) -> Intake {
    let size_of = |p: &str| std::fs::metadata(p).ok().map(|m| m.len());
    let exists = |p: &Path| p.exists();

    let folder = destination.trim();
    let folder = (!folder.is_empty()).then(|| PathBuf::from(folder));
    if let Some(dir) = folder.as_ref() {
        let _ = std::fs::create_dir_all(dir);
    }

    let mut intake = Intake::default();
    for path in paths {
        match judge(filter, max_kb, path, &size_of) {
            Err(reason) => intake.rejected.push((path.clone(), reason)),
            Ok(()) => {
                let landed = folder
                    .as_ref()
                    .and_then(|dir| destination_path(dir, Path::new(path), &exists))
                    .filter(|dest| std::fs::copy(path, dest).is_ok())
                    .map(|dest| dest.display().to_string())
                    .unwrap_or_else(|| path.clone());
                intake.accepted.push(landed);
            }
        }
    }
    intake
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_filter_is_read_the_way_a_developer_writes_it() {
        for spelling in ["xlsx,csv", "xlsx, csv", ".XLSX .CSV", "xlsx; csv", " xlsx  csv "] {
            assert_eq!(
                parse_extensions(spelling),
                vec!["xlsx".to_owned(), "csv".to_owned()],
                "{spelling:?}"
            );
        }
        assert!(parse_extensions("").is_empty(), "no filter is no filter");

        assert!(extension_allowed("xlsx, csv", "/tmp/Report.CSV"), "case-blind");
        assert!(!extension_allowed("xlsx, csv", "/tmp/report.pdf"));
        assert!(!extension_allowed("xlsx", "/tmp/README"), "no extension, filtered out");
        assert!(extension_allowed("", "/tmp/anything.at.all"), "empty accepts all");

        println!(
            "\n  drop filter — \"xlsx, csv\" accepts Report.CSV, refuses report.pdf and a \
             file with no extension; an empty filter accepts everything\n"
        );
    }

    #[test]
    fn the_size_limit_is_in_kilobytes_and_zero_means_no_limit() {
        assert!(size_allowed(0, u64::MAX), "0 KB is 'no limit', not 'nothing'");
        assert!(size_allowed(-1, 5_000));
        assert!(size_allowed(10, 10 * 1024), "exactly the limit is allowed");
        assert!(!size_allowed(10, 10 * 1024 + 1));
        println!("\n  drop size — 10 KB admits 10240 bytes and refuses 10241; 0 admits anything\n");
    }

    #[test]
    fn a_destination_never_overwrites_what_is_already_there() {
        let folder = Path::new("/inbox");
        let taken = |p: &Path| {
            matches!(
                p.to_str(),
                Some("/inbox/report.csv") | Some("/inbox/report (2).csv")
            )
        };
        assert_eq!(
            destination_path(folder, Path::new("/from/report.csv"), &taken),
            Some(PathBuf::from("/inbox/report (3).csv"))
        );
        assert_eq!(
            destination_path(folder, Path::new("/from/fresh.csv"), &|_: &Path| false),
            Some(PathBuf::from("/inbox/fresh.csv"))
        );
        // A name with no extension still gets a suffix, not a clobber.
        let taken_readme = |p: &Path| p.to_str() == Some("/inbox/README");
        assert_eq!(
            destination_path(folder, Path::new("/from/README"), &taken_readme),
            Some(PathBuf::from("/inbox/README (2)"))
        );
        println!(
            "\n  drop destination — report.csv lands as \"report (3).csv\" when the first two \
             names are taken; nothing is ever overwritten\n"
        );
    }

    #[test]
    fn judging_reports_which_rule_turned_a_file_away() {
        let big = |_: &str| Some(50 * 1024 + 1);
        let small = |_: &str| Some(1024_u64);

        assert_eq!(judge("csv", 50, "/t/a.csv", &small), Ok(()));
        assert_eq!(
            judge("csv", 50, "/t/a.pdf", &small),
            Err(Rejection::Extension)
        );
        assert_eq!(judge("csv", 50, "/t/a.csv", &big), Err(Rejection::TooBig));
        // Extension is judged first: a file that fails both is reported as the
        // wrong KIND of file, which is the more useful answer.
        assert_eq!(judge("csv", 50, "/t/a.pdf", &big), Err(Rejection::Extension));
        // A file the host cannot measure is not silently swallowed.
        assert_eq!(judge("csv", 1, "/t/a.csv", &|_| None), Ok(()));

        println!(
            "\n  drop verdicts — extension is checked before size, and an unmeasurable file \
             is admitted rather than dropped in silence\n"
        );
    }

    /// The whole intake, over a real folder: what is taken is copied and
    /// reported at its NEW home, what is refused is reported with its reason,
    /// and nothing already in the destination is overwritten.
    #[test]
    fn a_drop_is_filtered_copied_and_reported() {
        let root = std::env::temp_dir().join(format!("prc-dropzone-{}", std::process::id()));
        let from = root.join("from");
        let inbox = root.join("inbox");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&from).expect("scratch");

        let write = |name: &str, bytes: usize| {
            let p = from.join(name);
            std::fs::write(&p, vec![b'x'; bytes]).expect("write");
            p.display().to_string()
        };
        let good = write("report.csv", 512);
        let wrong_kind = write("photo.png", 512);
        let too_big = write("huge.csv", 4 * 1024 + 1);
        // Something already sitting in the destination under the same name.
        std::fs::create_dir_all(&inbox).expect("inbox");
        std::fs::write(inbox.join("report.csv"), b"older").expect("existing");

        let intake = take_files(
            &[good.clone(), wrong_kind.clone(), too_big.clone()],
            "csv",
            4,
            &inbox.display().to_string(),
        );

        assert_eq!(intake.accepted.len(), 1, "only the CSV within the limit");
        let landed = PathBuf::from(&intake.accepted[0]);
        assert_eq!(
            landed.file_name().and_then(|n| n.to_str()),
            Some("report (2).csv"),
            "the file already there is not overwritten"
        );
        assert_eq!(std::fs::read(&landed).unwrap().len(), 512, "the copy is the dropped file");
        assert_eq!(
            std::fs::read(inbox.join("report.csv")).unwrap(),
            b"older".to_vec(),
            "…and what was there is untouched"
        );
        assert_eq!(
            intake.rejected,
            vec![
                (wrong_kind, Rejection::Extension),
                (too_big, Rejection::TooBig)
            ]
        );
        assert!(intake.rejected_lines()[0].ends_with("\textension"));

        println!(
            "\n  drop intake — of 3 files: 1 accepted and copied to {:?}, 1 refused for its \
             extension, 1 for its size; the existing report.csv was left alone\n",
            landed.file_name().unwrap()
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
