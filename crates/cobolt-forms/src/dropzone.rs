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
//!
//! # Two moments, or one
//!
//! By default a drop is one moment: the zone judges each file and copies what it
//! accepts, then and there. `StageOnly` splits that in two, so the person doing
//! the dropping gets a say before anything is written:
//!
//! 1. **Drop** — [`stage_files`] judges, and nothing is copied. The accepted
//!    files are *staged* at their original paths and listed for review.
//! 2. **Confirm** — the form's own COBOL calls `CommitFiles()`, and
//!    [`commit_files`] copies the files still included into
//!    `DestinationFolder`, reporting what happened to each.
//!
//! [`take_files`] is those two run back to back, which is the default path — so
//! there is exactly ONE routine in this crate that copies a dropped file.

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

/// Judge a drop and keep what it accepts WITHOUT copying anything.
///
/// The accepted paths are the originals — the files have not moved. This is the
/// first half of a `StageOnly` zone's drop; [`commit_files`] is the second.
pub fn stage_files(paths: &[String], filter: &str, max_kb: i64) -> Intake {
    let size_of = |p: &str| std::fs::metadata(p).ok().map(|m| m.len());
    let mut intake = Intake::default();
    for path in paths {
        match judge(filter, max_kb, path, &size_of) {
            Err(reason) => intake.rejected.push((path.clone(), reason)),
            Ok(()) => intake.accepted.push(path.clone()),
        }
    }
    intake
}

/// What became of one file at commit time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitOutcome {
    /// Copied, and now at this path. Also the outcome when no destination is
    /// set: the file stays where it lies and that path is where it is.
    Copied(String),
    /// The zone could not put it there. Carries the reason, which is the
    /// filesystem's own — the folder is read-only, the disk is full, the source
    /// has vanished since it was dropped.
    Failed(String),
}

impl CommitOutcome {
    /// The path a form should now use: the copy when there is one, and the
    /// original when the copy failed — a form must still get the file it was
    /// given (the same promise [`take_files`] has always made).
    pub fn path_or(&self, original: &str) -> String {
        match self {
            CommitOutcome::Copied(p) => p.clone(),
            CommitOutcome::Failed(_) => original.to_owned(),
        }
    }

    pub fn copied(&self) -> bool {
        matches!(self, CommitOutcome::Copied(_))
    }
}

/// Copy each of `paths` into `destination`, in order, one outcome per file.
///
/// An empty `destination` copies nothing and reports every file `Copied` at its
/// own path — the zone was never asked to move anything, so every file is
/// exactly where it should be.
///
/// This is the only routine in this crate that copies a dropped file.
pub fn commit_files(paths: &[String], destination: &str) -> Vec<CommitOutcome> {
    let exists = |p: &Path| p.exists();
    let folder = destination.trim();
    if folder.is_empty() {
        return paths
            .iter()
            .map(|p| CommitOutcome::Copied(p.clone()))
            .collect();
    }
    let dir = PathBuf::from(folder);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        let reason = e.to_string();
        return paths
            .iter()
            .map(|_| CommitOutcome::Failed(reason.clone()))
            .collect();
    }
    paths
        .iter()
        .map(|path| match destination_path(&dir, Path::new(path), &exists) {
            None => CommitOutcome::Failed("no file name".to_owned()),
            Some(dest) => match std::fs::copy(path, &dest) {
                Ok(_) => CommitOutcome::Copied(dest.display().to_string()),
                Err(e) => CommitOutcome::Failed(e.to_string()),
            },
        })
        .collect()
}

/// Run a drop through the zone's rules: judge every file, then copy what it
/// accepts into `destination` (when one is set).
///
/// A file the zone accepts but cannot copy — the folder is unwritable, the disk
/// is full — is reported at its ORIGINAL path rather than dropped: the form
/// still gets the file it was given, and the developer's handler still runs.
pub fn take_files(paths: &[String], filter: &str, max_kb: i64, destination: &str) -> Intake {
    let mut intake = stage_files(paths, filter, max_kb);
    let outcomes = commit_files(&intake.accepted, destination);
    for (slot, outcome) in intake.accepted.iter_mut().zip(outcomes) {
        let at = outcome.path_or(slot);
        *slot = at;
    }
    intake
}

// ── The review list ───────────────────────────────────────────────────────────
//
// A `StageOnly` zone shows what it is holding in a companion ListBox, one row
// per staged file, each row tick-boxed so the person who dropped them can leave
// one out before the form goes ahead. These build what that list reads.

/// Bytes as megabytes to three decimals, the way the platform's own file
/// browser counts them: 1 MB is 1,000,000 bytes, so a size read here matches the
/// size read there.
pub fn size_mb(size_bytes: u64) -> f64 {
    size_bytes as f64 / 1_000_000.0
}

/// One row of the review list: the file's path, then its size.
///
/// `12.345 MB` to three decimals. A file whose size cannot be read says so
/// rather than claiming `0.000 MB` — a zone must not report a measurement it
/// does not have.
pub fn row_label(path: &str, size_bytes: Option<u64>) -> String {
    match size_bytes {
        Some(bytes) => format!("{path} ({:.3} MB)", size_mb(bytes)),
        None => format!("{path} (size unavailable)"),
    }
}

/// The same row once the form has gone ahead: a tick and the path the file now
/// lives at, or a cross and why it did not get there.
pub fn committed_row_label(
    original: &str,
    size_bytes: Option<u64>,
    outcome: &CommitOutcome,
) -> String {
    match outcome {
        CommitOutcome::Copied(at) => format!("✓ {}", row_label(at, size_bytes)),
        CommitOutcome::Failed(why) => format!("✗ {} — {why}", row_label(original, size_bytes)),
    }
}

/// A row the person doing the dropping unticked: still listed, so they can see
/// what they left out and put it back, and skipped when the form goes ahead.
pub fn excluded_row_label(path: &str, size_bytes: Option<u64>) -> String {
    format!("— {} (excluded)", row_label(path, size_bytes))
}

/// The line under the list, before the form goes ahead:
/// `3 files staged, 24.310 MB`.
pub fn staged_summary(sizes: &[Option<u64>]) -> String {
    let total: u64 = sizes.iter().flatten().sum();
    let files = if sizes.len() == 1 { "file" } else { "files" };
    format!(
        "{} {files} staged, {:.3} MB",
        sizes.len(),
        size_mb(total)
    )
}

/// The same line afterwards: `7 of 8 copied, 24.310 MB`, counting the bytes that
/// actually landed. `8 of 8` is the whole batch through.
pub fn commit_summary(outcomes: &[(Option<u64>, CommitOutcome)]) -> String {
    let copied: Vec<&(Option<u64>, CommitOutcome)> =
        outcomes.iter().filter(|(_, o)| o.copied()).collect();
    let bytes: u64 = copied.iter().filter_map(|(s, _)| *s).sum();
    format!(
        "{} of {} copied, {:.3} MB",
        copied.len(),
        outcomes.len(),
        size_mb(bytes)
    )
}

// ── One drop, one answer ──────────────────────────────────────────────────────

/// A zone's settings, as one argument.
#[derive(Clone, Copy, Debug)]
pub struct ZoneRules<'a> {
    pub filter: &'a str,
    pub max_kb: i64,
    pub destination: &'a str,
    /// `StageOnly` — hold the files and let the form confirm, instead of copying
    /// now.
    pub stage_only: bool,
    /// `FileListControl` — the companion ListBox, or `""` for no list.
    pub list_id: &'a str,
    /// The zone's `StagedFiles` as it stands, so a second drop adds to the first
    /// instead of replacing it.
    pub already_staged: &'a str,
}

/// What one drop changes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DropWrites {
    /// `(control id, property, value)` — the zone's own properties and, when one
    /// is wired up, the companion list's.
    pub updates: Vec<(String, String, String)>,
    /// How many files the zone took, and how many it turned away — which of
    /// `onFilesDropped` / `onFilesRejected` the host should fire.
    pub accepted: usize,
    pub rejected: usize,
}

/// Everything a drop does to a zone, in one place, so the OS drag-drop path and
/// the click-to-browse picker cannot drift apart about what a drop means.
///
/// Dropping the same file twice adds nothing the second time: a staged list is a
/// set of files to submit, and two identical rows could not be told apart in it.
pub fn apply_drop(zone_id: &str, paths: &[String], rules: ZoneRules<'_>) -> DropWrites {
    let mut writes = DropWrites::default();
    let set = |w: &mut DropWrites, id: &str, key: &str, val: String| {
        w.updates.push((id.to_owned(), key.to_owned(), val));
    };

    if !rules.stage_only {
        // The way it has always worked: judge, copy, report where they landed.
        let intake = take_files(paths, rules.filter, rules.max_kb, rules.destination);
        writes.accepted = intake.accepted.len();
        writes.rejected = intake.rejected.len();
        set(&mut writes, zone_id, "DroppedFiles", intake.accepted.join("\n"));
        set(
            &mut writes,
            zone_id,
            "RejectedFiles",
            intake.rejected_lines().join("\n"),
        );
        return writes;
    }

    let intake = stage_files(paths, rules.filter, rules.max_kb);
    writes.rejected = intake.rejected.len();

    // Everything held so far, plus what just arrived, each file once.
    let mut staged: Vec<String> = rules
        .already_staged
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    for path in &intake.accepted {
        if !staged.iter().any(|s| s == path) {
            staged.push(path.clone());
            writes.accepted += 1;
        }
    }

    let sizes: Vec<Option<u64>> = staged
        .iter()
        .map(|p| std::fs::metadata(p).ok().map(|m| m.len()))
        .collect();
    let rows: Vec<String> = staged
        .iter()
        .zip(&sizes)
        .map(|(p, s)| row_label(p, *s))
        .collect();

    set(&mut writes, zone_id, "StagedFiles", staged.join("\n"));
    // A staged drop has copied nothing, so `DroppedFiles` is where the files
    // still are — the handler sees what arrived without being told it moved.
    set(&mut writes, zone_id, "DroppedFiles", staged.join("\n"));
    set(
        &mut writes,
        zone_id,
        "RejectedFiles",
        intake.rejected_lines().join("\n"),
    );
    set(&mut writes, zone_id, "CommitSummary", staged_summary(&sizes));
    if !rules.list_id.trim().is_empty() {
        let list = rules.list_id.trim();
        // Every new row arrives ticked: the person doing the dropping unticks
        // what they did not mean to send, rather than ticking what they did.
        set(&mut writes, list, "Items", rows.join("\n"));
        set(&mut writes, list, "CheckedItems", rows.join("\n"));
        set(&mut writes, list, "ShowCheckBoxes", "1".to_owned());
    }
    writes
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

    /// A `StageOnly` drop writes NOTHING. That is the whole promise: the person
    /// who dropped the files gets to look at the list and leave one out before
    /// the form goes ahead, and until it does the destination folder is
    /// untouched — not even created.
    #[test]
    fn staging_a_drop_copies_nothing_until_the_form_goes_ahead() {
        let root = std::env::temp_dir().join(format!("prc-dz-stage-{}", std::process::id()));
        let from = root.join("from");
        let inbox = root.join("inbox");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&from).expect("scratch");

        let write = |name: &str, bytes: usize| {
            let p = from.join(name);
            std::fs::write(&p, vec![b'x'; bytes]).expect("write");
            p.display().to_string()
        };
        let a = write("alpha.csv", 1_500_000);
        let b = write("beta.csv", 2_250_000);
        let wrong = write("gamma.png", 10);

        // ── Drop ──────────────────────────────────────────────────────────
        let staged = stage_files(&[a.clone(), b.clone(), wrong.clone()], "csv", 0);
        assert_eq!(
            staged.accepted,
            vec![a.clone(), b.clone()],
            "staged files stay at their ORIGINAL paths"
        );
        assert_eq!(staged.rejected, vec![(wrong, Rejection::Extension)]);
        assert!(
            !inbox.exists(),
            "staging must not even create the destination folder"
        );

        // ── The review list ───────────────────────────────────────────────
        let sizes: Vec<Option<u64>> = staged
            .accepted
            .iter()
            .map(|p| std::fs::metadata(p).ok().map(|m| m.len()))
            .collect();
        let rows: Vec<String> = staged
            .accepted
            .iter()
            .zip(&sizes)
            .map(|(p, s)| row_label(p, *s))
            .collect();
        assert!(
            rows[0].ends_with(" (1.500 MB)") && rows[0].starts_with(&a),
            "a row is the path then the size in MB to 3 decimals, got {:?}",
            rows[0]
        );
        assert!(rows[1].ends_with(" (2.250 MB)"), "got {:?}", rows[1]);
        assert_eq!(staged_summary(&sizes), "2 files staged, 3.750 MB");
        // One file. Not "1 files".
        assert_eq!(staged_summary(&sizes[..1]), "1 file staged, 1.500 MB");
        // A file that cannot be measured says so rather than claiming 0.000 MB.
        assert_eq!(row_label("/gone.csv", None), "/gone.csv (size unavailable)");
        // An unticked row stays visible, so the exclusion is something the
        // person can see and undo.
        assert!(excluded_row_label(&a, sizes[0]).contains("(excluded)"));

        // ── Confirm, with beta left out ───────────────────────────────────
        let included = vec![a.clone()];
        let outcomes = commit_files(&included, &inbox.display().to_string());
        assert_eq!(outcomes.len(), 1);
        let landed = match &outcomes[0] {
            CommitOutcome::Copied(p) => p.clone(),
            other => panic!("alpha must have been copied, got {other:?}"),
        };
        assert_eq!(
            std::fs::read(&landed).unwrap().len(),
            1_500_000,
            "the copy is the file that was dropped"
        );
        assert!(
            !inbox.join("beta.csv").exists(),
            "the unticked file must not be copied"
        );
        let paired = vec![(sizes[0], outcomes[0].clone())];
        assert_eq!(commit_summary(&paired), "1 of 1 copied, 1.500 MB");
        assert!(committed_row_label(&a, sizes[0], &outcomes[0]).starts_with("✓ "));

        // ── A destination that cannot be written ──────────────────────────
        //    The form still gets the file, at the path it was dropped from,
        //    and the row says why it did not move.
        let blocked = commit_files(&included, "/dev/null/nope");
        assert!(matches!(blocked[0], CommitOutcome::Failed(_)));
        assert_eq!(blocked[0].path_or(&a), a, "a failed copy keeps the original");
        let row = committed_row_label(&a, sizes[0], &blocked[0]);
        assert!(row.starts_with("✗ ") && row.contains(" — "), "got {row:?}");
        assert_eq!(
            commit_summary(&[(sizes[0], blocked[0].clone())]),
            "0 of 1 copied, 0.000 MB"
        );

        println!(
            "\n  drop staging — 3 dropped, 1 refused (extension), 2 staged with nothing \
             written (inbox not created); rows \"<path> (1.500 MB)\" / \"(2.250 MB)\", \
             summary \"2 files staged, 3.750 MB\"; confirming with 1 unticked copied \
             1 of 1 (1.500 MB) and left beta.csv alone; an unwritable destination \
             reported \"0 of 1 copied\" and kept the original path\n"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// `apply_drop` is what BOTH ways in — an OS drag and the native picker —
    /// run, so the two cannot drift about what a drop means. It also has to
    /// leave a default zone behaving exactly as it always did.
    #[test]
    fn one_drop_one_answer_for_the_drag_and_the_picker() {
        let root = std::env::temp_dir().join(format!("prc-dz-apply-{}", std::process::id()));
        let from = root.join("from");
        let inbox = root.join("inbox");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&from).expect("scratch");
        let write = |name: &str, bytes: usize| {
            let p = from.join(name);
            std::fs::write(&p, vec![b'x'; bytes]).expect("write");
            p.display().to_string()
        };
        let a = write("alpha.csv", 1_000_000);
        let b = write("beta.csv", 2_000_000);
        let dest = inbox.display().to_string();
        let got = |w: &DropWrites, id: &str, key: &str| -> Option<String> {
            w.updates
                .iter()
                .find(|(i, k, _)| i == id && k == key)
                .map(|(_, _, v)| v.clone())
        };

        // ── Default zone: unchanged. Copy now, report the new paths, and say
        //    nothing about staging at all.
        let now = apply_drop(
            "FDZ-1",
            &[a.clone()],
            ZoneRules {
                filter: "csv",
                max_kb: 0,
                destination: &dest,
                stage_only: false,
                list_id: "LST-1",
                already_staged: "",
            },
        );
        assert_eq!((now.accepted, now.rejected), (1, 0));
        let copied = got(&now, "FDZ-1", "DroppedFiles").expect("DroppedFiles");
        assert!(
            copied.starts_with(&inbox.display().to_string()),
            "a default zone still copies on the drop, got {copied:?}"
        );
        assert!(
            got(&now, "FDZ-1", "StagedFiles").is_none()
                && got(&now, "LST-1", "Items").is_none(),
            "a default zone must not touch StagedFiles or the list"
        );

        // ── Staged zone: nothing copied, the list filled and fully ticked.
        let _ = std::fs::remove_dir_all(&inbox);
        let staged = apply_drop(
            "FDZ-1",
            &[a.clone(), b.clone()],
            ZoneRules {
                filter: "csv",
                max_kb: 0,
                destination: &dest,
                stage_only: true,
                list_id: "LST-1",
                already_staged: "",
            },
        );
        assert_eq!((staged.accepted, staged.rejected), (2, 0));
        assert!(!inbox.exists(), "a staged drop writes nothing");
        assert_eq!(
            got(&staged, "FDZ-1", "StagedFiles"),
            Some(format!("{a}\n{b}"))
        );
        let items = got(&staged, "LST-1", "Items").expect("the list is filled");
        assert_eq!(items.lines().count(), 2);
        assert!(items.lines().next().unwrap().ends_with(" (1.000 MB)"));
        assert_eq!(
            got(&staged, "LST-1", "CheckedItems"),
            Some(items.clone()),
            "every new row arrives ticked — the operator unticks, never ticks"
        );
        assert_eq!(
            got(&staged, "LST-1", "ShowCheckBoxes").as_deref(),
            Some("1"),
            "the review list needs its tick boxes whatever the designer left"
        );
        // 1,000,000 + 2,000,000 bytes is exactly 3.000 MB at 1 MB = 1,000,000.
        assert_eq!(
            got(&staged, "FDZ-1", "CommitSummary").as_deref(),
            Some("2 files staged, 3.000 MB"),
            "the summary totals the staged bytes"
        );

        // ── A second drop ADDS, and the same file twice is held once.
        let again = apply_drop(
            "FDZ-1",
            &[a.clone()],
            ZoneRules {
                filter: "csv",
                max_kb: 0,
                destination: &dest,
                stage_only: true,
                list_id: "LST-1",
                already_staged: &format!("{a}\n{b}"),
            },
        );
        assert_eq!(
            got(&again, "FDZ-1", "StagedFiles"),
            Some(format!("{a}\n{b}")),
            "a file already staged is not staged twice"
        );
        assert_eq!(
            again.accepted, 0,
            "…and nothing new arrived, so onFilesDropped must not fire"
        );

        // ── No list wired up: the zone still stages, there is just nothing to
        //    review in. Naming a control that is gone is the same case.
        let listless = apply_drop(
            "FDZ-1",
            &[b.clone()],
            ZoneRules {
                filter: "csv",
                max_kb: 0,
                destination: &dest,
                stage_only: true,
                list_id: "",
                already_staged: "",
            },
        );
        assert_eq!(got(&listless, "FDZ-1", "StagedFiles"), Some(b.clone()));
        assert!(
            listless.updates.iter().all(|(i, _, _)| i == "FDZ-1"),
            "with no list named, nothing may be written to another control"
        );

        println!(
            "\n  drop routing — one `apply_drop` for the drag and the picker: a default \
             zone copies on the drop (DroppedFiles under the destination, StagedFiles \
             untouched); a StageOnly zone writes nothing, fills 2 ticked rows + \
             ShowCheckBoxes, and re-dropping a staged file adds 0; with no list named, \
             every write stays on the zone\n"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
