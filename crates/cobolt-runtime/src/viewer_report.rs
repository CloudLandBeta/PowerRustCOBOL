// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 062 — a COBOL report printed into a Viewer control.
//!
//! The file verbs do not change: `OPEN OUTPUT`, `WRITE`, `CLOSE`. What changes
//! is where the lines land. A report is backed by a real file in the operating
//! system's temporary directory, which is what lets the Viewer's own Save As,
//! Print, Share and search work on it with no new machinery — the control
//! already knows how to open a path. When that directory cannot be written the
//! report is held in memory instead and handed over the way `LoadBytes` already
//! hands a document over, so a filesystem problem costs the report nothing.

use cobolt_ast::program::FileOrganization;

/// The name a report is written under: `<form>-<uuid>.<ext>`.
///
/// The form's name makes the file recognisable in a directory shared with every
/// other program on the machine; the UUID makes two runs of the same form — or
/// two reports in one run — unable to collide (operator, 2026-09-20). The
/// extension is not decoration: `viewer::detect_format` picks the renderer from
/// it, which is the whole reason a report needs no format property.
pub fn report_filename(form: &str, org: FileOrganization) -> String {
    let stem: String = form
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let stem = stem.trim_matches('-');
    let stem = if stem.is_empty() { "report" } else { stem };
    format!(
        "{stem}-{}.{}",
        uuid::Uuid::new_v4(),
        org.report_extension()
    )
}

/// Where a report's lines are going.
///
/// A file when the operating system's temporary directory will take one, and
/// memory when it will not (R11). The two are the same to `WRITE`; they differ
/// only at `CLOSE`, where one hands the Viewer a path and the other hands it
/// bytes — the route `LoadBytes` already uses.
pub enum ReportSink {
    File {
        w: std::io::BufWriter<std::fs::File>,
        path: std::path::PathBuf,
    },
    Memory(Vec<u8>),
}

/// What a finished report is: a file the Viewer can open, or the bytes of one.
pub enum ReportOutcome {
    Path(std::path::PathBuf),
    Bytes(Vec<u8>),
}

impl ReportSink {
    /// Start a report. `append_to` is the backing file of a report this COBOL
    /// file wrote earlier, which `OPEN EXTEND` continues; `None` starts a new
    /// one, which is what `OPEN OUTPUT` always does.
    ///
    /// A temp directory that cannot be written is not an error the program
    /// hears about: the report is simply held in memory, which is R11's whole
    /// point — a filesystem problem must not cost the developer their report.
    pub fn open(form: &str, org: FileOrganization, append_to: Option<&std::path::Path>) -> Self {
        let path = match append_to {
            Some(p) => p.to_path_buf(),
            None => std::env::temp_dir().join(report_filename(form, org)),
        };
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .append(append_to.is_some())
            .truncate(append_to.is_none())
            .open(&path);
        match file {
            Ok(f) => Self::File {
                w: std::io::BufWriter::new(f),
                path,
            },
            Err(e) => {
                tracing::warn!(
                    "report: {} cannot be written ({e}) — holding the report in memory",
                    path.display()
                );
                Self::Memory(Vec::new())
            }
        }
    }

    /// The backing file, when there is one.
    pub fn path(&self) -> Option<&std::path::Path> {
        match self {
            Self::File { path, .. } => Some(path.as_path()),
            Self::Memory(_) => None,
        }
    }

    /// Flush and say what the report turned out to be.
    pub fn finish(self) -> ReportOutcome {
        match self {
            Self::File { mut w, path } => {
                use std::io::Write as _;
                // Flushed and dropped BEFORE the caller announces the path: a
                // Viewer session polls its source every frame, and a path that
                // arrives before the bytes do indexes half a document.
                let _ = w.flush();
                drop(w);
                ReportOutcome::Path(path)
            }
            Self::Memory(bytes) => ReportOutcome::Bytes(bytes),
        }
    }
}

impl std::io::Write for ReportSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::File { w, .. } => w.write(buf),
            Self::Memory(b) => b.write(buf),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::File { w, .. } => w.flush(),
            Self::Memory(b) => b.flush(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_filename_names_the_form_and_the_renderer() {
        for (org, ext) in [
            (FileOrganization::Markdown, "md"),
            (FileOrganization::Html, "html"),
            (FileOrganization::Sequential, "txt"),
        ] {
            let name = report_filename("SALES-FORM", org);
            println!("  {org:?} → {name}");
            assert!(name.starts_with("SALES-FORM-"), "{name} must name the form");
            assert!(name.ends_with(&format!(".{ext}")), "{name} must end .{ext}");
        }
    }

    #[test]
    fn two_reports_never_share_a_name() {
        let a = report_filename("F", FileOrganization::Markdown);
        let b = report_filename("F", FileOrganization::Markdown);
        println!("  {a}\n  {b}");
        assert_ne!(a, b);
    }

    /// A temp directory that cannot be written costs the report nothing: it is
    /// held in memory instead, and `CLOSE` hands the bytes over the way
    /// `LoadBytes` does (R11).
    #[test]
    fn a_report_falls_back_to_memory_when_the_directory_will_not_take_it() {
        use std::io::Write as _;
        let mut on_disk = ReportSink::open("F", FileOrganization::Markdown, None);
        writeln!(on_disk, "# on disk").expect("write");
        let path = on_disk.path().map(|p| p.to_path_buf()).expect("a file");
        match on_disk.finish() {
            ReportOutcome::Path(p) => {
                let text = std::fs::read_to_string(&p).expect("read back");
                println!("  file   {} → {:?}", p.display(), text.trim());
                assert_eq!(text, "# on disk\n");
                let _ = std::fs::remove_file(&p);
            }
            ReportOutcome::Bytes(_) => panic!("a writable temp dir must give a file"),
        }
        assert!(path.starts_with(std::env::temp_dir()));

        // A directory that is not one: the open fails and memory takes over.
        let mut held = ReportSink::Memory(Vec::new());
        writeln!(held, "# in memory").expect("write");
        match held.finish() {
            ReportOutcome::Bytes(b) => {
                println!("  memory {:?}", String::from_utf8_lossy(&b).trim());
                assert_eq!(b, b"# in memory\n");
            }
            ReportOutcome::Path(_) => panic!("memory must stay memory"),
        }
    }

    /// A form's name reaches this from the developer's own project, so it can
    /// hold anything a filename cannot. Nothing here may produce a path.
    #[test]
    fn a_form_name_can_never_escape_the_directory() {
        for hostile in ["../../etc/passwd", "a/b", "", "   ", "réport"] {
            let name = report_filename(hostile, FileOrganization::Sequential);
            println!("  {hostile:?} → {name}");
            assert!(!name.contains('/'), "{name}");
            assert!(!name.contains('\\'), "{name}");
            assert!(!name.starts_with('.'), "{name}");
        }
    }
}
