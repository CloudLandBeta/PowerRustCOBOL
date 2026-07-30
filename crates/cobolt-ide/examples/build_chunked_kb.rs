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
    // Command-line progress: an 80-step character bar. Completed steps are
    // dots, the frontier is a `*`; when the `*` disappears (all 80 dots) the
    // indexing is done. Redrawn in place with `\r`, only when the frontier
    // actually moves.
    const BAR_STEPS: usize = 80;
    let mut last_cell = usize::MAX;
    let records = cobolt_agents::chunked_knowledge::sync_tree_with_progress(
        &scratch,
        &store,
        &mut |done, total, _subject| {
            if total == 0 {
                return;
            }
            let cell = (done * BAR_STEPS) / total; // 0..=BAR_STEPS
            if cell == last_cell {
                return;
            }
            last_cell = cell;
            let mut bar = String::with_capacity(BAR_STEPS);
            for i in 0..BAR_STEPS {
                bar.push(if i < cell {
                    '.'
                } else if i == cell {
                    '*'
                } else {
                    ' '
                });
            }
            print!("\r{bar}");
            use std::io::Write as _;
            let _ = std::io::stdout().flush();
        },
    )?;
    // Land the cursor on a fresh line after the bar (a no-op visual change
    // when nothing needed re-embedding and the bar never drew).
    if last_cell != usize::MAX {
        println!("\r{}", ".".repeat(BAR_STEPS));
    }
    // GPU runs full speed; the CPU fallback runs low-power (capped threads).
    if let Some(dev) = cobolt_agents::bert_embedder::active_device_label() {
        println!("embedding device: {dev}");
    }

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
