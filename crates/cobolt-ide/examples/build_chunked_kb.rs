// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! Regenerate the PREBUILT chunked System Knowledge Base shipped inside the
//! IDE binary (`assets/knowledge/chunked.data`).
//!
//! Run whenever the reference documentation changes (the freshness test
//! `prebuilt_chunked_kb_matches_the_published_documentation` fails until you
//! do):
//!
//! ```sh
//! cargo run -p cobolt-ide --example build_chunked_kb
//! ```
//!
//! Requires the semantic model (`multilingual-e5-small`) to be downloaded —
//! a store built with the hashing fallback would ship no usable embeddings
//! and is refused.

use std::path::PathBuf;

fn main() -> Result<(), String> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("assets").join("knowledge").join("chunked.data"));

    if !cobolt_agents::project_knowledge::semantic_model_is_ready() {
        return Err(
            "the semantic model is not downloaded — a prebuilt store needs real embeddings. \
             Download it in the IDE (Models Manager) first."
                .into(),
        );
    }

    // Publish the reference documentation from THIS build into a scratch
    // root, then chunk-embed it into a fresh store.
    let scratch = std::env::temp_dir().join(format!("prc-build-chunked-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
    cobolt_compiler::publish_system_documentation(&scratch).map_err(|e| e.to_string())?;
    let store = scratch.join("chunked.data");
    let mut last_report = std::time::Instant::now();
    let records = cobolt_agents::chunked_knowledge::sync_tree_with_progress(
        &scratch,
        &store,
        &mut |done, total, subject| {
            if last_report.elapsed().as_millis() >= 500 || done + 1 == total {
                println!("  embedding {done}/{total} — {subject}");
                last_report = std::time::Instant::now();
            }
        },
    )?;

    // Verify the store answers for every published document under the
    // semantic stamp before shipping it.
    let documents = cobolt_agents::chunked_knowledge::tree_documents(&scratch)?;
    let stale = cobolt_agents::chunked_knowledge::stale_documents(
        &store,
        &documents,
        "bert-multilingual-e5-small",
    )?;
    if !stale.is_empty() {
        return Err(format!("store is stale for {stale:?} right after building"));
    }

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::copy(&store, &out).map_err(|e| e.to_string())?;
    let size = std::fs::metadata(&out).map_err(|e| e.to_string())?.len();
    let _ = std::fs::remove_dir_all(&scratch);
    println!(
        "prebuilt chunked KB: {records} records from {} documents → {} ({size} bytes)",
        documents.len(),
        out.display()
    );
    println!("Rebuild the IDE so the new store is embedded in the binary.");
    Ok(())
}
