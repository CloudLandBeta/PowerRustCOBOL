// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The project's files through MCP (spec 084 Phase D, R49–R55, R65): read,
//! list, search, write, edit, rename, remove, import, and the gap report.
//! With these, Claude Code never needs its own file tools on a project.
//!
//! Every path is project-relative and resolved by
//! [`ProjectRoot::resolve`], which refuses `..`, absolute paths and links that
//! lead out. A write goes through the host: a file the IDE holds unsaved is
//! refused (R55), a file of a kind the project lists is recorded as
//! `add_to_project` records it, and the host is told what was written, so the
//! IDE reloads it and knows the change came through the connection (R68).

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use cobolt_mcp::Tool;
use serde_json::{json, Value};

use crate::host::{FileList, ProjectHost};
use crate::root::ProjectRoot;

/// The largest text `read_file` answers whole; a bigger file is read by lines.
const TEXT_LIMIT: usize = 2 * 1024 * 1024;
/// The largest binary file `read_file` answers.
const BINARY_LIMIT: usize = 12 * 1024 * 1024;
/// Most matches `search_files` answers.
const SEARCH_LIMIT: usize = 200;
/// Where a removed file goes (R52): inside the project, never in a list.
pub const RECYCLE: &str = ".recycle";
/// Folders a search or a listing never descends into: build output and
/// version control, which are not the project's sources.
const SKIP_DIRS: [&str; 5] = ["target", "dist", ".git", RECYCLE, "node_modules"];

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Tool {
    Tool {
        name: name.into(),
        description: Some(description.into()),
        input_schema: json!({
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        }),
        annotations: None,
    }
}

fn s(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

pub fn tools() -> Vec<Tool> {
    vec![
        tool(
            "read_file",
            "Read a file of the project. A text file comes whole, or lines `start_line`..`end_line` (1-based, \
             inclusive). A binary file (an image, a font, a data file) comes as base64 with its type. Read-only.",
            json!({
                "path": s("Project-relative, e.g. forms/MAIN.cfrm or src/TAXES.cbl."),
                "start_line": { "type": "integer" },
                "end_line": { "type": "integer" },
            }),
            &["path"],
        ),
        tool(
            "list_folder",
            "List a folder of the project: each entry's name, whether it is a file or a folder, and a file's size. \
             The project folder when `path` is absent. Read-only.",
            json!({ "path": s("Optional: a project-relative folder.") }),
            &[],
        ),
        tool(
            "search_files",
            "Search the project's text files for `query` and answer each match's file, line number and line. \
             `files` narrows by file name (`*.cbl`, `forms/*.cfrm`); `path` narrows to a folder. Case-insensitive \
             unless `case_sensitive`. Build output and .git are not searched. Read-only.",
            json!({
                "query": s("The text to find."),
                "path": s("Optional: a project-relative folder to search in."),
                "files": s("Optional: a file-name pattern with * and ?, matched against the project-relative path or the name."),
                "case_sensitive": { "type": "boolean" },
                "limit": { "type": "integer", "description": "Most matches; 200 when absent." },
            }),
            &["query"],
        ),
        tool(
            "write_file",
            "Create a file, or replace a file's whole content, with `text` or (for a binary file) `base64`. A file of \
             a kind the project lists (a form, Common Code, an indexed definition, an asset, a Backend program) is \
             recorded in the project. Refused for generated code (only regenerate and build write it), the project \
             file, and a file with unsaved changes in the IDE. Changes the file.",
            json!({
                "path": s("Project-relative."),
                "text": s("The whole new content, for a text file."),
                "base64": s("The whole new content as base64, for a binary file."),
                "create_only": { "type": "boolean", "description": "Refuse when the file exists." },
            }),
            &["path"],
        ),
        tool(
            "edit_file",
            "Change a text file by replacing `old_text`, which must occur exactly once (or every occurrence with \
             `replace_all`), with `new_text`. Same refusals as write_file. Changes the file.",
            json!({
                "path": s("Project-relative."),
                "old_text": s("The exact text to replace, with enough around it to be unique."),
                "new_text": s("What it becomes."),
                "replace_all": { "type": "boolean" },
            }),
            &["path", "old_text", "new_text"],
        ),
        tool(
            "rename_file",
            "Rename or move a file or folder of the project. The project's lists, its main-form seal and the IDE's \
             open tabs follow. Refused when the target exists, for generated code and the project file, and for a \
             file with unsaved changes in the IDE. Changes files.",
            json!({ "path": s("Project-relative, what to move."), "to": s("Project-relative, where it goes.") }),
            &["path", "to"],
        ),
        tool(
            "remove_file",
            "Remove a file or folder from the project. It is moved, not deleted: the answer says where it went \
             (under .recycle/), and moving it back restores it. The project's lists follow. Refused for the project \
             file and a file with unsaved changes in the IDE. Changes files.",
            json!({ "path": s("Project-relative.") }),
            &["path"],
        ),
        tool(
            "import_file",
            "Copy a file from outside the project, which the developer named, into the project, byte for byte, and \
             record it as write_file does. `to` is a project-relative folder (ending in /) or file path. Never \
             overwrites. Changes files.",
            json!({
                "source": s("The absolute path of the file to copy in."),
                "to": s("Project-relative destination, e.g. Assets/ or Assets/logo.png."),
            }),
            &["source", "to"],
        ),
        tool(
            "write_gap_report",
            "Write a gap report — something the product cannot do yet — to docs/compiler-requests/<date>-<topic>.md, \
             in the format the gap-report skill gives. Answers its path. Writes the file.",
            json!({
                "topic": s("A short kebab-case topic, e.g. datagrid-cell-tooltips."),
                "text": s("The report, Markdown."),
            }),
            &["topic", "text"],
        ),
    ]
}

fn opt_str(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn need_str(args: &Value, key: &str, tool: &str) -> Result<String, String> {
    opt_str(args, key)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("{tool} needs a '{key}'"))
}

/// Why a file tool may not write `rel`, if it may not (R54).
fn protected(root: &ProjectRoot, rel: &str) -> Option<String> {
    let rel = rel.replace('\\', "/");
    let first = rel.split('/').next().unwrap_or_default();
    if first.eq_ignore_ascii_case("generated") {
        return Some(format!("'{rel}' is generated code; regenerate and build write it, never a file tool"));
    }
    if let Ok(view) = cobolt_compiler::project_manifest_view(root.manifest()) {
        if view.generated.iter().any(|g| g.eq_ignore_ascii_case(&rel)) {
            return Some(format!("'{rel}' is generated code; regenerate and build write it, never a file tool"));
        }
    }
    if rel.eq_ignore_ascii_case(&root.manifest_name())
        || (!rel.contains('/') && (rel.ends_with(".project.toml") || rel.eq_ignore_ascii_case("cobolt.toml")))
    {
        return Some(
            "the project file is never written by hand: it carries a seal. Use add_to_project, set_definition, \
             set_project_settings or the other project tools"
                .to_owned(),
        );
    }
    None
}

fn unsaved_refusal(rel: &str) -> String {
    format!("'{rel}' has unsaved changes in the IDE; ask the developer to save or close it, then try again")
}

/// The list a newly written file belongs in, or `None` for a file no list
/// holds (data, the menu sidecar of a SideMenu, documentation outside docs/…).
fn list_for(root: &ProjectRoot, rel: &str) -> Option<FileList> {
    let rel = rel.replace('\\', "/");
    let first = rel.split('/').next().unwrap_or_default().to_ascii_lowercase();
    if !rel.contains('/') || matches!(first.as_str(), "data" | "docs" | "copybooks" | "generated" | "dist" | "target" | ".recycle") {
        return None;
    }
    if rel.to_ascii_lowercase().ends_with(".yaml") || rel.to_ascii_lowercase().ends_with(".yml") {
        return None;
    }
    let kind = cobolt_compiler::project_manifest_view(root.manifest()).map(|v| v.kind).unwrap_or_default();
    match FileList::of_path_in(&rel, kind) {
        FileList::Generated | FileList::Documentation => None,
        list => Some(list),
    }
}

fn record_if_listed<H: ProjectHost>(host: &mut H, root: &ProjectRoot, rel: &str) -> Result<Option<&'static str>, String> {
    match list_for(root, rel) {
        Some(list) => {
            host.record(rel, list)?;
            Ok(Some(list.key()))
        }
        None => Ok(None),
    }
}

fn is_text(bytes: &[u8]) -> bool {
    !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok()
}

fn mime_of(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("woff2") => "font/woff2",
        Some("wav") => "audio/wav",
        Some("mp3") => "audio/mpeg",
        Some("pdf") => "application/pdf",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

pub fn read_file(root: &ProjectRoot, args: &Value) -> Result<Value, String> {
    let rel = need_str(args, "path", "read_file")?;
    let abs = root.resolve(&rel)?;
    if abs.is_dir() {
        return Err(format!("'{rel}' is a folder; list it with list_folder"));
    }
    let bytes = std::fs::read(&abs).map_err(|e| format!("'{rel}' cannot be read ({e})"))?;
    if !is_text(&bytes) {
        if bytes.len() > BINARY_LIMIT {
            return Err(format!("'{rel}' is a binary file of {} bytes, more than read_file answers", bytes.len()));
        }
        return Ok(json!({
            "path": rel,
            "binary": true,
            "mime": mime_of(&abs),
            "bytes": bytes.len(),
            "base64": cobolt_mcp::base64(&bytes),
        }));
    }
    let text = String::from_utf8(bytes).unwrap_or_default();
    let total = text.lines().count();
    let start = args.get("start_line").and_then(Value::as_u64).map(|n| n.max(1) as usize);
    let end = args.get("end_line").and_then(Value::as_u64).map(|n| n as usize);
    if start.is_none() && end.is_none() {
        if text.len() > TEXT_LIMIT {
            return Err(format!(
                "'{rel}' has {total} lines ({} bytes); read it by lines with start_line and end_line",
                text.len()
            ));
        }
        return Ok(json!({ "path": rel, "lines": total, "text": text }));
    }
    let start = start.unwrap_or(1);
    let end = end.unwrap_or(total).min(total);
    if start > end.max(1) && total > 0 {
        return Err(format!("'{rel}' has {total} lines; lines {start}..{end} are not in it"));
    }
    let slice: Vec<&str> = text.lines().skip(start - 1).take(end.saturating_sub(start - 1)).collect();
    Ok(json!({ "path": rel, "lines": total, "start_line": start, "end_line": end, "text": slice.join("\n") }))
}

pub fn list_folder(root: &ProjectRoot, args: &Value) -> Result<Value, String> {
    let rel = opt_str(args, "path").filter(|p| !p.trim().is_empty() && p.trim() != ".");
    let abs = match &rel {
        Some(r) => root.resolve(r)?,
        None => root.dir().to_path_buf(),
    };
    if !abs.is_dir() {
        return Err(format!("'{}' is not a folder", rel.as_deref().unwrap_or(".")));
    }
    let mut entries = Vec::new();
    let mut items: Vec<_> = std::fs::read_dir(&abs).map_err(|e| format!("cannot list it ({e})"))?.flatten().collect();
    items.sort_by_key(|e| e.file_name());
    for e in items {
        let name = e.file_name().to_string_lossy().into_owned();
        let Ok(meta) = e.metadata() else { continue };
        if meta.is_dir() {
            entries.push(json!({ "name": name, "kind": "folder" }));
        } else {
            entries.push(json!({ "name": name, "kind": "file", "bytes": meta.len() }));
        }
    }
    Ok(json!({ "path": rel.unwrap_or_else(|| ".".into()), "entries": entries }))
}

/// `*` any run of characters, `?` one; case-insensitive.
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti, mut star, mut mark) = (0usize, 0usize, None::<usize>, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut items: Vec<_> = rd.flatten().collect();
    items.sort_by_key(|e| e.file_name());
    for e in items {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        match e.file_type() {
            Ok(t) if t.is_dir() => {
                if !SKIP_DIRS.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
                    walk(&path, out);
                }
            }
            Ok(t) if t.is_file() => out.push(path),
            _ => {}
        }
    }
}

pub fn search_files(root: &ProjectRoot, args: &Value) -> Result<Value, String> {
    let query = need_str(args, "query", "search_files")?;
    let case = args.get("case_sensitive").and_then(Value::as_bool).unwrap_or(false);
    let limit = args.get("limit").and_then(Value::as_u64).map_or(SEARCH_LIMIT, |n| (n as usize).clamp(1, 2000));
    let base = match opt_str(args, "path").filter(|p| !p.trim().is_empty()) {
        Some(p) => root.resolve(&p)?,
        None => root.dir().to_path_buf(),
    };
    let pattern = opt_str(args, "files").filter(|p| !p.trim().is_empty());
    let needle = if case { query.clone() } else { query.to_lowercase() };
    let mut files = Vec::new();
    walk(&base, &mut files);
    let mut matches = Vec::new();
    let mut searched = 0usize;
    let mut truncated = false;
    'files: for abs in files {
        let Some(rel) = root.relative(&abs) else { continue };
        if let Some(p) = &pattern {
            let name = abs.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if !glob_match(p, &rel) && !glob_match(p, &name) {
                continue;
            }
        }
        let Ok(bytes) = std::fs::read(&abs) else { continue };
        if bytes.len() > TEXT_LIMIT || !is_text(&bytes) {
            continue;
        }
        searched += 1;
        let text = String::from_utf8_lossy(&bytes);
        for (n, line) in text.lines().enumerate() {
            let hay = if case { line.to_owned() } else { line.to_lowercase() };
            if hay.contains(&needle) {
                if matches.len() == limit {
                    truncated = true;
                    break 'files;
                }
                matches.push(json!({ "file": rel, "line": n + 1, "text": line.trim_end() }));
            }
        }
    }
    Ok(json!({ "query": query, "files_searched": searched, "matches": matches, "truncated": truncated }))
}

/// The bytes a write carries: `text`, or `base64`.
fn content(args: &Value, tool: &str) -> Result<Vec<u8>, String> {
    match (args.get("text").and_then(Value::as_str), args.get("base64").and_then(Value::as_str)) {
        (Some(_), Some(_)) => Err(format!("{tool} takes 'text' or 'base64', not both")),
        (Some(t), None) => Ok(t.as_bytes().to_vec()),
        (None, Some(b)) => cobolt_mcp::base64_decode(b).ok_or_else(|| "'base64' is not base64".to_owned()),
        (None, None) => Err(format!("{tool} needs 'text' (or 'base64' for a binary file)")),
    }
}

pub(crate) fn write_checked<H: ProjectHost>(host: &mut H, root: &ProjectRoot, rel: &str, bytes: &[u8]) -> Result<(PathBuf, Option<&'static str>), String> {
    let abs = root.resolve(rel)?;
    if let Some(why) = protected(root, rel) {
        return Err(why);
    }
    if host.unsaved(&abs) {
        return Err(unsaved_refusal(rel));
    }
    if abs.is_dir() {
        return Err(format!("'{rel}' is a folder"));
    }
    if let Some(parent) = abs.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot make the folder for '{rel}' ({e})"))?;
    }
    std::fs::write(&abs, bytes).map_err(|e| format!("'{rel}' cannot be written ({e})"))?;
    let rel_norm = root.relative(&abs).unwrap_or_else(|| rel.to_owned());
    let recorded = record_if_listed(host, root, &rel_norm)?;
    host.written(std::slice::from_ref(&abs));
    Ok((abs, recorded))
}

pub fn write_file<H: ProjectHost>(host: &mut H, root: &ProjectRoot, args: &Value) -> Result<Value, String> {
    let rel = need_str(args, "path", "write_file")?;
    let bytes = content(args, "write_file")?;
    let abs = root.resolve(&rel)?;
    let existed = abs.is_file();
    if existed && args.get("create_only").and_then(Value::as_bool).unwrap_or(false) {
        return Err(format!("'{rel}' exists; create_only refuses to replace it"));
    }
    let (_, recorded) = write_checked(host, root, &rel, &bytes)?;
    Ok(json!({ "written": rel, "bytes": bytes.len(), "created": !existed, "recorded_in": recorded }))
}

pub fn edit_file<H: ProjectHost>(host: &mut H, root: &ProjectRoot, args: &Value) -> Result<Value, String> {
    let rel = need_str(args, "path", "edit_file")?;
    let old = opt_str(args, "old_text").filter(|t| !t.is_empty()).ok_or("edit_file needs a non-empty 'old_text'")?;
    let new = opt_str(args, "new_text").ok_or("edit_file needs a 'new_text' (empty removes the text)")?;
    let abs = root.resolve(&rel)?;
    let bytes = std::fs::read(&abs).map_err(|e| format!("'{rel}' cannot be read ({e})"))?;
    if !is_text(&bytes) {
        return Err(format!("'{rel}' is not a text file; replace it with write_file"));
    }
    let text = String::from_utf8(bytes).unwrap_or_default();
    let count = text.matches(old.as_str()).count();
    let all = args.get("replace_all").and_then(Value::as_bool).unwrap_or(false);
    if count == 0 {
        return Err(format!("'old_text' is not in '{rel}'; read the file again and copy the text exactly"));
    }
    if count > 1 && !all {
        return Err(format!("'old_text' occurs {count} times in '{rel}'; give more of the text around it, or set replace_all"));
    }
    let changed = if all { text.replace(old.as_str(), &new) } else { text.replacen(old.as_str(), &new, 1) };
    write_checked(host, root, &rel, changed.as_bytes())?;
    Ok(json!({ "edited": rel, "replacements": if all { count } else { 1 } }))
}

pub fn rename_file<H: ProjectHost>(host: &mut H, root: &ProjectRoot, args: &Value) -> Result<Value, String> {
    let from = need_str(args, "path", "rename_file")?;
    let to = need_str(args, "to", "rename_file")?;
    let from_abs = root.resolve(&from)?;
    let to_abs = root.resolve(&to)?;
    if !from_abs.exists() {
        return Err(format!("'{from}' does not exist"));
    }
    if to_abs.exists() {
        return Err(format!("'{to}' exists; rename_file never overwrites"));
    }
    for rel in [&from, &to] {
        if let Some(why) = protected(root, rel) {
            return Err(why);
        }
    }
    if host.unsaved(&from_abs) {
        return Err(unsaved_refusal(&from));
    }
    if let Some(parent) = to_abs.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot make the folder for '{to}' ({e})"))?;
    }
    std::fs::rename(&from_abs, &to_abs).map_err(|e| format!("'{from}' cannot be moved ({e})"))?;
    let from_rel = root.relative(&from_abs).unwrap_or(from.clone());
    let to_rel = root.relative(&to_abs).unwrap_or(to.clone());
    if let Err(e) = host.relist(&from_rel, Some(&to_rel)) {
        // Put it back: a file the project no longer finds is worse than no rename.
        let _ = std::fs::rename(&to_abs, &from_abs);
        return Err(format!("the project could not follow the rename, so nothing changed ({e})"));
    }
    host.written(&[from_abs, to_abs]);
    Ok(json!({ "renamed": from_rel, "to": to_rel }))
}

/// `YYYYMMDD-HHMMSS`, UTC — the recycle folder's name for one removal.
fn stamp(now: SystemTime) -> String {
    let secs = now.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// `YYYY-MM-DD`, UTC.
fn date(now: SystemTime) -> String {
    let s = stamp(now);
    format!("{}-{}-{}", &s[0..4], &s[4..6], &s[6..8])
}

pub fn remove_file<H: ProjectHost>(host: &mut H, root: &ProjectRoot, args: &Value) -> Result<Value, String> {
    let rel = need_str(args, "path", "remove_file")?;
    let abs = root.resolve(&rel)?;
    if !abs.exists() {
        return Err(format!("'{rel}' does not exist"));
    }
    if abs == root.dir() {
        return Err("the project folder itself cannot be removed".to_owned());
    }
    if rel.replace('\\', "/").split('/').next() == Some(RECYCLE) {
        return Err(format!("'{rel}' is already removed; it is in {RECYCLE}/"));
    }
    if rel.eq_ignore_ascii_case(&root.manifest_name()) {
        return Err("the project file cannot be removed".to_owned());
    }
    if host.unsaved(&abs) {
        return Err(unsaved_refusal(&rel));
    }
    let rel_norm = root.relative(&abs).unwrap_or(rel.clone());
    let mut dest_rel = format!("{RECYCLE}/{}/{rel_norm}", stamp(SystemTime::now()));
    let mut n = 2;
    while root.dir().join(&dest_rel).exists() {
        dest_rel = format!("{RECYCLE}/{}-{n}/{rel_norm}", stamp(SystemTime::now()));
        n += 1;
    }
    let dest = root.dir().join(&dest_rel);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot make {RECYCLE}/ ({e})"))?;
    }
    std::fs::rename(&abs, &dest).map_err(|e| format!("'{rel}' cannot be moved to {RECYCLE}/ ({e})"))?;
    if let Err(e) = host.relist(&rel_norm, None) {
        let _ = std::fs::rename(&dest, &abs);
        return Err(format!("the project could not follow the removal, so nothing changed ({e})"));
    }
    host.written(&[abs, dest]);
    Ok(json!({
        "removed": rel_norm,
        "recover_from": dest_rel,
        "note": "moved, not deleted: rename_file it back to its old path to restore it",
    }))
}

pub fn import_file<H: ProjectHost>(host: &mut H, root: &ProjectRoot, args: &Value) -> Result<Value, String> {
    let source = PathBuf::from(need_str(args, "source", "import_file")?);
    if !source.is_absolute() {
        return Err("import_file: 'source' must be an absolute path".to_owned());
    }
    if !source.is_file() {
        return Err(format!("'{}' is not a file", source.display()));
    }
    let to = need_str(args, "to", "import_file")?;
    let name = source.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or("the source has no file name")?;
    let to_abs = root.resolve(&to)?;
    let rel = if to.ends_with('/') || to.ends_with('\\') || to_abs.is_dir() {
        format!("{}/{name}", to.trim_end_matches(['/', '\\']))
    } else {
        to.clone()
    };
    if root.resolve(&rel)?.exists() {
        return Err(format!("'{rel}' exists; import_file never overwrites"));
    }
    let bytes = std::fs::read(&source).map_err(|e| format!("'{}' cannot be read ({e})", source.display()))?;
    let (_, recorded) = write_checked(host, root, &rel, &bytes)?;
    Ok(json!({ "imported": rel, "bytes": bytes.len(), "recorded_in": recorded }))
}

pub fn write_gap_report<H: ProjectHost>(host: &mut H, root: &ProjectRoot, args: &Value) -> Result<Value, String> {
    let topic = need_str(args, "topic", "write_gap_report")?;
    let text = need_str(args, "text", "write_gap_report")?;
    let slug: String = topic
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        return Err("write_gap_report: 'topic' needs letters or digits".to_owned());
    }
    let day = date(SystemTime::now());
    let mut rel = format!("docs/compiler-requests/{day}-{slug}.md");
    let mut n = 2;
    while root.dir().join(&rel).exists() {
        rel = format!("docs/compiler-requests/{day}-{slug}-{n}.md");
        n += 1;
    }
    let (_, _) = write_checked(host, root, &rel, text.as_bytes())?;
    Ok(json!({ "written": rel }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::HeadlessHost;

    fn project(name: &str) -> (PathBuf, ProjectRoot, HeadlessHost) {
        let dir = std::env::temp_dir().join(format!("prc-084d-files-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("forms")).unwrap();
        let mut form = cobolt_forms::Form::new("MAIN", "MAIN", 300, 200);
        form.main_form = true;
        cobolt_forms::save_form(&form, &dir.join("forms/MAIN.cfrm")).unwrap();
        std::fs::write(
            dir.join("Demo.project.toml"),
            "[project]\nname = \"Demo\"\nversion = \"1.0.0\"\n\n[files]\nforms = [\"forms/MAIN.cfrm\"]\nsources = []\n",
        )
        .unwrap();
        crate::host::record_in_manifest(&dir.join("Demo.project.toml"), "forms/MAIN.cfrm", FileList::Forms).unwrap();
        let root = ProjectRoot::open(&dir).unwrap();
        let host = HeadlessHost::new(dir.join("Demo.project.toml"), "test");
        (dir, root, host)
    }

    fn manifest(dir: &Path) -> String {
        std::fs::read_to_string(dir.join("Demo.project.toml")).unwrap()
    }

    /// AC23: every file tool, end to end on a temporary project.
    #[test]
    fn the_file_tools_read_search_write_edit_rename_remove_and_import() {
        let (dir, root, mut host) = project("all");
        // Write Common Code: recorded in sources.
        let w = write_file(&mut host, &root, &json!({"path": "src/TAXES.cbl", "text": "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. TAXES.\n       PROCEDURE DIVISION.\n       CALC-PARA.\n           GOBACK.\n"})).unwrap();
        assert_eq!(w["recorded_in"], "sources", "{w}");
        assert!(manifest(&dir).contains("src/TAXES.cbl"));
        // Read whole and by lines.
        let r = read_file(&root, &json!({"path": "src/TAXES.cbl"})).unwrap();
        assert_eq!(r["lines"], 5);
        let r = read_file(&root, &json!({"path": "src/TAXES.cbl", "start_line": 2, "end_line": 3})).unwrap();
        assert_eq!(r["text"], "       PROGRAM-ID. TAXES.\n       PROCEDURE DIVISION.");
        // Search: file and line.
        let f = search_files(&root, &json!({"query": "calc-para", "files": "*.cbl"})).unwrap();
        assert_eq!(f["matches"][0]["file"], "src/TAXES.cbl");
        assert_eq!(f["matches"][0]["line"], 4);
        // Edit by exact replacement; a missing or repeated text is refused.
        edit_file(&mut host, &root, &json!({"path": "src/TAXES.cbl", "old_text": "GOBACK.", "new_text": "EXIT PROGRAM."})).unwrap();
        assert!(std::fs::read_to_string(dir.join("src/TAXES.cbl")).unwrap().contains("EXIT PROGRAM."));
        assert!(edit_file(&mut host, &root, &json!({"path": "src/TAXES.cbl", "old_text": "nope", "new_text": "x"})).is_err());
        assert!(edit_file(&mut host, &root, &json!({"path": "src/TAXES.cbl", "old_text": "       ", "new_text": "x"})).unwrap_err().contains("times"));
        // Rename a listed form: the manifest follows, the seal is restated.
        let before = manifest(&dir);
        rename_file(&mut host, &root, &json!({"path": "forms/MAIN.cfrm", "to": "forms/HOME.cfrm"})).unwrap();
        let after = manifest(&dir);
        assert!(after.contains("forms/HOME.cfrm") && !after.contains("forms/MAIN.cfrm"), "{after}");
        assert!(dir.join("forms/HOME.cfrm").is_file() && !dir.join("forms/MAIN.cfrm").exists());
        assert_ne!(before, after);
        // Remove: moved to .recycle, unlisted, recoverable.
        let gone = remove_file(&mut host, &root, &json!({"path": "src/TAXES.cbl"})).unwrap();
        let back = gone["recover_from"].as_str().unwrap().to_owned();
        assert!(dir.join(&back).is_file(), "{back}");
        assert!(!manifest(&dir).contains("src/TAXES.cbl"));
        rename_file(&mut host, &root, &json!({"path": back, "to": "src/TAXES.cbl"})).unwrap();
        assert!(dir.join("src/TAXES.cbl").is_file(), "recovered");
        // Import a PNG byte for byte; recorded as an asset.
        let png: Vec<u8> = (0u8..=255).chain([0, 0, 13, 10]).collect();
        let src = std::env::temp_dir().join(format!("prc-084d-logo-{}.png", std::process::id()));
        std::fs::write(&src, &png).unwrap();
        let i = import_file(&mut host, &root, &json!({"source": src.to_string_lossy(), "to": "Assets/"})).unwrap();
        let name = src.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(std::fs::read(dir.join("Assets").join(&name)).unwrap(), png);
        assert_eq!(i["recorded_in"], "assets");
        let b = read_file(&root, &json!({"path": format!("Assets/{name}")})).unwrap();
        assert_eq!(b["mime"], "image/png");
        assert_eq!(cobolt_mcp::base64_decode(b["base64"].as_str().unwrap()).unwrap(), png, "base64 round trip");
        // A binary write through base64.
        write_file(&mut host, &root, &json!({"path": "Assets/copy.png", "base64": b["base64"]})).unwrap();
        assert_eq!(std::fs::read(dir.join("Assets/copy.png")).unwrap(), png);
        // Refusals: generated code, the manifest, outside the project.
        for (path, why) in [("generated/MAIN.cbl", "generated"), ("Demo.project.toml", "seal"), ("../x.cbl", "climbs")] {
            let e = write_file(&mut host, &root, &json!({"path": path, "text": "x"})).unwrap_err();
            assert!(e.contains(why), "{path}: {e}");
        }
        // A gap report lands where the skill says.
        let g = write_gap_report(&mut host, &root, &json!({"topic": "DataGrid cell tooltips", "text": "# Gap\n"})).unwrap();
        assert!(g["written"].as_str().unwrap().starts_with("docs/compiler-requests/") && g["written"].as_str().unwrap().ends_with("-datagrid-cell-tooltips.md"), "{g}");
        // A listing.
        let l = list_folder(&root, &json!({})).unwrap();
        assert!(l["entries"].as_array().unwrap().iter().any(|e| e["name"] == "forms" && e["kind"] == "folder"));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_file(&src);
        println!("file tools: write+record, read whole/lines, search, edit (3 cases), rename+reseal, remove+recover, import 260 bytes, base64 write, 3 refusals, gap report, listing");
    }

    #[test]
    fn the_recycle_stamp_and_glob_are_right() {
        assert_eq!(stamp(UNIX_EPOCH), "19700101-000000");
        assert_eq!(stamp(UNIX_EPOCH + std::time::Duration::from_secs(1_763_510_400 + 3_723)), "20251119-010203");
        assert_eq!(date(UNIX_EPOCH + std::time::Duration::from_secs(951_782_400)), "2000-02-29");
        assert!(glob_match("*.cbl", "TAXES.cbl"));
        assert!(glob_match("forms/*.cfrm", "forms/MAIN.cfrm"));
        assert!(!glob_match("*.cbl", "MAIN.cfrm"));
        assert!(glob_match("?AIN*", "main.cfrm"));
        assert_eq!(cobolt_mcp::base64_decode("aGVsbG8="), Some(b"hello".to_vec()));
        assert_eq!(cobolt_mcp::base64_decode("aGVsbG8"), Some(b"hello".to_vec()));
        assert_eq!(cobolt_mcp::base64_decode("a"), None);
    }
}
