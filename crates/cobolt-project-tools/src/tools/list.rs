// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `list_files` — what the manifest tracks, and the gap reports.

use serde_json::{json, Map, Value};

use crate::host::FileList;
use crate::root::ProjectRoot;

/// The folder an agent writes gap reports into (spec 080 R17).
pub const COMPILER_REQUESTS_DIR: &str = "docs/compiler-requests";

pub fn run(root: &ProjectRoot) -> Result<Value, String> {
    let view = cobolt_compiler::project_manifest_view(root.manifest())?;
    let mut lists = Map::new();
    for list in FileList::ALL {
        // Only a Web or Mobile project has a Backend; a Desktop project's
        // answer is what it always was (WASM-001 R11).
        if list == FileList::Backend && !view.kind.has_backend() {
            continue;
        }
        let entries = match list {
            FileList::Forms => &view.forms,
            FileList::Indexed => &view.indexed,
            FileList::Sources => &view.sources,
            FileList::Generated => &view.generated,
            FileList::Assets => &view.assets,
            FileList::Documentation => &view.documentation,
            FileList::Backend => &view.backend,
        };
        let rows: Vec<Value> = entries
            .iter()
            .map(|rel| {
                // A manifest entry that escapes the project is reported, never
                // probed.
                let exists = root.resolve(rel).map(|p| p.is_file()).unwrap_or(false);
                json!({ "path": rel, "exists": exists })
            })
            .collect();
        lists.insert(list.key().to_owned(), Value::Array(rows));
    }
    let mut answer = json!({
        "project": view.name,
        "manifest": root.manifest_name(),
        "kind": view.kind.as_str(),
        "files": lists,
        "compiler_requests": compiler_requests(root),
    });
    // What a Web or Mobile project adds to the picture (WASM-001 R12): which
    // folder holds its forms, its Backend and the programs the Backend
    // exports, and — for a Web project — where its static assets sit.
    if view.kind.has_backend() {
        answer["forms_folder"] = json!(view.kind.forms_folder());
        answer["backend"] = json!({
            "folder": cobolt_compiler::project_kind::BACKEND_FOLDER,
            "exports": view.backend_exports,
        });
        answer["web_assets"] = json!(web_assets(root));
    }
    // A Mobile project's native projects (WASM-012 R18): what the manifest records per platform,
    // whether it is on disk, and the folder the platform's own tool opens.
    if let Some(m) = view.mobile.as_ref() {
        answer["native"] = native_projects(root, &m.native);
    }
    Ok(answer)
}

/// `[mobile.native]` as the agent sees it: per platform, the folder recorded (`null` when none was
/// made), whether the project is there, and the folder Android Studio opens.
fn native_projects(root: &ProjectRoot, recorded: &cobolt_compiler::project_kind::NativeProjects) -> Value {
    use cobolt_compiler::mobile_native as mn;
    let or_null = |s: &str| if s.trim().is_empty() { Value::Null } else { json!(s.trim()) };
    let studio = root.relative(&mn::android_studio_dir(root.dir())).unwrap_or_else(|| format!("{}/{}", mn::ANDROID_DIR, mn::ANDROID_STUDIO_SUBDIR));
    json!({
        "android": { "recorded": or_null(&recorded.android), "exists": mn::android_exists(root.dir()), "android_studio_folder": studio },
        "ios": { "recorded": or_null(&recorded.ios), "exists": false },
    })
}

/// The web side of a Web or Mobile project on disk: the form stylesheets
/// (`*.css` beside the forms) and the built bundle under `dist/web`, if there is
/// one — folder, file count and bytes, so an agent sees what a Build left.
fn web_assets(root: &ProjectRoot) -> Value {
    fn walk(dir: &std::path::Path, out: &mut Vec<(std::path::PathBuf, u64)>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if let Ok(m) = e.metadata() {
                out.push((p, m.len()));
            }
        }
    }
    let view = cobolt_compiler::project_manifest_view(root.manifest()).ok();
    let forms_folder = view.as_ref().map(|v| v.kind.forms_folder()).unwrap_or("webforms");
    let mut sheets = Vec::new();
    if let Ok(dir) = root.resolve(forms_folder) {
        let mut found = Vec::new();
        walk(&dir, &mut found);
        for (path, _) in found {
            if path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("css")) {
                if let Some(rel) = root.relative(&path) {
                    sheets.push(rel);
                }
            }
        }
    }
    sheets.sort();
    let bundle_rel = crate::web::BUNDLE_SUBDIR;
    let (mut files, mut bytes) = (0u64, 0u64);
    let exists = match root.resolve(bundle_rel) {
        Ok(dir) if dir.is_dir() => {
            let mut found = Vec::new();
            walk(&dir, &mut found);
            files = found.len() as u64;
            bytes = found.iter().map(|(_, n)| n).sum();
            true
        }
        _ => false,
    };
    json!({
        "stylesheets": sheets,
        "bundle": { "folder": bundle_rel, "exists": exists, "files": files, "bytes": bytes },
    })
}

/// `docs/compiler-requests/*.md`, newest first by the `YYYY-MM-DD` prefix.
pub fn compiler_requests(root: &ProjectRoot) -> Vec<String> {
    let Ok(dir) = root.resolve(COMPILER_REQUESTS_DIR) else {
        return Vec::new();
    };
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_file())
                .filter_map(|e| e.file_name().to_str().map(str::to_owned))
                .filter(|n| n.to_ascii_lowercase().ends_with(".md"))
                .collect()
        })
        .unwrap_or_default();
    names.sort_by(|a, b| b.cmp(a));
    names
        .into_iter()
        .map(|n| format!("{COMPILER_REQUESTS_DIR}/{n}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_reports_every_list_and_the_gap_reports() {
        let dir = std::env::temp_dir().join(format!("prc-080-list-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for d in ["forms", "indexed", "src", "docs/compiler-requests"] {
            std::fs::create_dir_all(dir.join(d)).unwrap();
        }
        std::fs::write(
            dir.join("Demo.project.toml"),
            "[project]\nname = \"Demo\"\nversion = \"1.0.0\"\nmain = \"\"\n[files]\n\
             forms = [\"forms/A.cfrm\", \"forms/Gone.cfrm\"]\nindexed = [\"indexed/a.cidx\"]\n\
             sources = [\"src/x.cbl\"]\n",
        )
        .unwrap();
        for f in ["forms/A.cfrm", "indexed/a.cidx", "src/x.cbl"] {
            std::fs::write(dir.join(f), "x").unwrap();
        }
        for r in ["2026-09-30-older.md", "2026-10-01-newer.md", "notes.txt"] {
            std::fs::write(dir.join("docs/compiler-requests").join(r), "r").unwrap();
        }
        let root = ProjectRoot::open(&dir).unwrap();
        let v = run(&root).unwrap();
        let forms = v["files"]["forms"].as_array().unwrap();
        assert_eq!(forms[0], json!({"path":"forms/A.cfrm","exists":true}));
        assert_eq!(forms[1], json!({"path":"forms/Gone.cfrm","exists":false}));
        assert_eq!(v["files"]["indexed"][0]["exists"], true);
        assert_eq!(v["files"]["sources"][0]["exists"], true);
        assert_eq!(
            v["compiler_requests"],
            json!(["docs/compiler-requests/2026-10-01-newer.md", "docs/compiler-requests/2026-09-30-older.md"])
        );
        assert_eq!(v["manifest"], "Demo.project.toml");
        println!(
            "list: 6 lists reported, 4 entries (3 exist, 1 missing), 2 reports newest first, 1 non-.md ignored"
        );
    }

    #[test]
    fn a_mobile_project_reports_its_native_projects_012() {
        let dir = std::env::temp_dir().join(format!("prc-012-list-native-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("mobileforms")).unwrap();
        let manifest = "[project]\nname = \"M.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"mobile\"\n[files]\n\
                        [mobile]\nplatform = \"android\"\ndevice = \"phone\"\n";
        std::fs::write(dir.join("M.project.toml"), manifest).unwrap();
        let root = ProjectRoot::open(&dir).unwrap();
        let v = run(&root).unwrap();
        let none = json!({
            "android": { "recorded": null, "exists": false, "android_studio_folder": "android/src-tauri/gen/android" },
            "ios": { "recorded": null, "exists": false },
        });
        assert_eq!(v["native"], none, "{v:#}");
        // Made and recorded: the Gradle project the Tauri command line writes, and `[mobile.native]`.
        let studio = dir.join("android/src-tauri/gen/android/app");
        std::fs::create_dir_all(&studio).unwrap();
        std::fs::write(studio.join("build.gradle.kts"), "// generated").unwrap();
        std::fs::write(dir.join("M.project.toml"), format!("{manifest}[mobile.native]\nandroid = \"android\"\n")).unwrap();
        let v = run(&ProjectRoot::open(&dir).unwrap()).unwrap();
        assert_eq!(v["native"]["android"]["recorded"], "android");
        assert_eq!(v["native"]["android"]["exists"], true);
        let _ = std::fs::remove_dir_all(&dir);
        println!("list: a Mobile project's native projects, missing then made and recorded");
    }
}
