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
        let entries = match list {
            FileList::Forms => &view.forms,
            FileList::Indexed => &view.indexed,
            FileList::Sources => &view.sources,
            FileList::Generated => &view.generated,
            FileList::Assets => &view.assets,
            FileList::Documentation => &view.documentation,
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
    Ok(json!({
        "project": view.name,
        "manifest": root.manifest_name(),
        "files": lists,
        "compiler_requests": compiler_requests(root),
    }))
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
}
