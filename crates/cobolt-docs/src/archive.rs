// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! ZIP, TAR and gzip, walked in memory within [`Limits`] (spec 074 R6, R15,
//! R16). One budget covers every level of nesting, so a small archive cannot
//! unpack past it however it is built. Nothing is ever written to disk, and a
//! member's path is only ever a name. A member that cannot be read is reported
//! and the rest of the archive still is (R14); an archive past a bound is
//! reported as too large, whole.

use std::io::{Cursor, Read};

use crate::sniff::Kind;
use crate::{code, convert_at, Converted, Limits, Part, Skip, PATH_SEPARATOR};

pub struct Budget {
    bytes_left: u64,
    files_left: usize,
    max_depth: u8,
    limits: Limits,
}

impl Budget {
    pub fn new(limits: &Limits) -> Self {
        Budget {
            bytes_left: limits.max_unpacked_bytes,
            files_left: limits.max_files,
            max_depth: limits.max_depth,
            limits: *limits,
        }
    }

    fn too_large(&self, what: &str) -> Skip {
        let l = &self.limits;
        Skip::new(
            code::TOO_LARGE,
            format!(
                "the archive {what} (limits: {} MB unpacked, {} files, {} levels of nesting)",
                l.max_unpacked_bytes / (1024 * 1024),
                l.max_files,
                l.max_depth
            ),
        )
    }

    /// Read at most what is left of the byte budget, and charge it.
    fn take(&mut self, reader: impl Read) -> Result<Result<Vec<u8>, String>, Skip> {
        let mut data = Vec::new();
        if let Err(e) = reader.take(self.bytes_left + 1).read_to_end(&mut data) {
            return Ok(Err(e.to_string()));
        }
        if data.len() as u64 > self.bytes_left {
            return Err(self.too_large("unpacks past its size limit"));
        }
        self.bytes_left -= data.len() as u64;
        Ok(Ok(data))
    }

    fn count_file(&mut self) -> Result<(), Skip> {
        if self.files_left == 0 {
            return Err(self.too_large("holds more files than allowed"));
        }
        self.files_left -= 1;
        Ok(())
    }
}

/// Expand an archive at nesting `depth` (1 = the outermost).
pub fn expand(
    kind: Kind,
    name: &str,
    bytes: &[u8],
    depth: u8,
    budget: &mut Budget,
) -> Result<Converted, Skip> {
    if depth > budget.max_depth {
        return Err(budget.too_large("is nested too deeply"));
    }
    let mut out = Converted::default();
    match kind {
        Kind::Gzip => {
            let data = budget
                .take(flate2::read::GzDecoder::new(bytes))?
                .map_err(|e| Skip::new(code::DAMAGED, format!("the compressed file could not be read: {e}")))?;
            // gzip wraps one file: transparent, not a level of its own.
            return convert_at(&strip_gz(name), &data, depth - 1, budget);
        }
        Kind::Zip => {
            let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
                .map_err(|e| Skip::new(code::DAMAGED, format!("the archive could not be read: {e}")))?;
            for i in 0..zip.len() {
                let listed = zip.name_for_index(i).map(clean).unwrap_or_default();
                let (member, data) = {
                    let file = match zip.by_index(i) {
                        Ok(f) => f,
                        Err(e) => {
                            out.skipped.push((listed, member_error(&e.to_string())));
                            continue;
                        }
                    };
                    if file.is_dir() {
                        continue;
                    }
                    let member = clean(file.name());
                    if is_junk(&member) {
                        continue;
                    }
                    budget.count_file()?;
                    if file.size() > budget.bytes_left {
                        return Err(budget.too_large("unpacks past its size limit"));
                    }
                    match budget.take(file)? {
                        Ok(data) => (member, data),
                        Err(e) => {
                            out.skipped.push((member, member_error(&e)));
                            continue;
                        }
                    }
                };
                add_member(&mut out, member, &data, depth, budget)?;
            }
        }
        Kind::Tar => {
            let mut tar = tar::Archive::new(Cursor::new(bytes));
            let entries = tar
                .entries()
                .map_err(|e| Skip::new(code::DAMAGED, format!("the archive could not be read: {e}")))?;
            for entry in entries {
                let (member, data) = {
                    let entry = match entry {
                        Ok(e) => e,
                        // A broken header ends what can be read of a TAR.
                        Err(e) => {
                            out.skipped.push((String::new(), member_error(&e.to_string())));
                            break;
                        }
                    };
                    if !entry.header().entry_type().is_file() {
                        continue;
                    }
                    let member = clean(&String::from_utf8_lossy(&entry.path_bytes()));
                    if is_junk(&member) {
                        continue;
                    }
                    budget.count_file()?;
                    if entry.size() > budget.bytes_left {
                        return Err(budget.too_large("unpacks past its size limit"));
                    }
                    match budget.take(entry)? {
                        Ok(data) => (member, data),
                        Err(e) => {
                            out.skipped.push((member, member_error(&e)));
                            continue;
                        }
                    }
                };
                add_member(&mut out, member, &data, depth, budget)?;
            }
        }
        _ => unreachable!("expand is called for archives only"),
    }
    Ok(out)
}

/// Convert one member and file its parts under its path. Only a bound
/// crossed stops the archive; any other failure is the member's own.
fn add_member(
    out: &mut Converted,
    member: String,
    data: &[u8],
    depth: u8,
    budget: &mut Budget,
) -> Result<(), Skip> {
    let within = |inner: Option<String>| match inner {
        Some(inner) => format!("{member}{PATH_SEPARATOR}{inner}"),
        None => member.clone(),
    };
    match convert_at(&member, data, depth, budget) {
        Ok(converted) => {
            out.parts.extend(converted.parts.into_iter().map(|p| Part {
                name: Some(within(p.name)),
                markdown: p.markdown,
            }));
            out.skipped
                .extend(converted.skipped.into_iter().map(|(n, s)| (within(Some(n)), s)));
            Ok(())
        }
        Err(skip) if skip.code == code::TOO_LARGE => Err(skip),
        Err(skip) => {
            out.skipped.push((member.clone(), skip));
            Ok(())
        }
    }
}

fn member_error(e: &str) -> Skip {
    if e.to_ascii_lowercase().contains("password") {
        Skip::new(code::PASSWORD_PROTECTED, "the archive member is password protected")
    } else {
        Skip::new(code::DAMAGED, format!("the archive member could not be read: {e}"))
    }
}

/// A member's path as a name only: separators unified, and empty, `.`, `..`
/// and drive segments dropped, so no path can point outside anything (R16).
fn clean(path: &str) -> String {
    path.split(['/', '\\'])
        .filter(|s| !s.is_empty() && *s != "." && *s != ".." && !s.ends_with(':'))
        .collect::<Vec<_>>()
        .join("/")
}

/// Operating-system litter, not documents.
fn is_junk(member: &str) -> bool {
    let last = member.rsplit('/').next().unwrap_or(member);
    member.is_empty()
        || member.split('/').any(|s| s == "__MACOSX")
        || last.starts_with("._")
        || last == ".DS_Store"
        || last.eq_ignore_ascii_case("Thumbs.db")
}

/// The name of what a gzip file holds.
fn strip_gz(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".tgz") {
        format!("{}.tar", &name[..name.len() - 4])
    } else if lower.ends_with(".gz") {
        name[..name.len() - 3].to_string()
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_paths_are_names_only() {
        assert_eq!(clean("../../escape.txt"), "escape.txt");
        assert_eq!(clean("/etc/passwd"), "etc/passwd");
        assert_eq!(clean("C:\\Windows\\..\\a.txt"), "Windows/a.txt");
        assert_eq!(clean("legal/./nda.docx"), "legal/nda.docx");
        assert!(is_junk("__MACOSX/legal/._nda.docx"));
        assert!(is_junk("legal/.DS_Store"));
        assert!(!is_junk("legal/nda.docx"));
        assert_eq!(strip_gz("docs.tar.gz"), "docs.tar");
        assert_eq!(strip_gz("docs.TGZ"), "docs.tar");
    }
}
