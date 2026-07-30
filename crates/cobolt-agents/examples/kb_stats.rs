// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! Throwaway diagnostic: per-store record/char/token counts for chunked
//! Knowledge Base stores, using the same ≈4-chars/token estimate as the
//! RAG-efficiency stat. An optional `--subject=<PREFIX>` restricts the count
//! to one subject family (e.g. `--subject=Label` counts the Label control
//! record plus its `Label · <name>` property/event/method records and
//! `Label › <heading>` sections), with a per-kind breakdown.
//!
//! Usage: kb_stats [--subject=PREFIX] <store.data> [more stores...]

use std::collections::BTreeMap;
use std::path::Path;

use redb::{Database, ReadableTable, TableDefinition};
use serde::Deserialize;

const CHUNKS: TableDefinition<&str, &[u8]> = TableDefinition::new("chunks");

/// Positional mirror of `chunked_knowledge::ChunkRecord` (bincode is
/// field-order based, so the shapes must match exactly).
#[derive(Deserialize)]
struct ChunkRecord {
    subject: String,
    kind: String,
    content: String,
    #[allow(dead_code)]
    parent: Option<String>,
    #[allow(dead_code)]
    dimensions: u32,
    #[allow(dead_code)]
    embedding: Vec<f32>,
    #[allow(dead_code)]
    embedder: String,
}

fn matches(subject: &str, prefix: &str) -> bool {
    subject == prefix
        || subject.starts_with(&format!("{prefix} \u{b7}")) // "Label · onClick"
        || subject.starts_with(&format!("{prefix} \u{203a}")) // "Label › Section"
}

fn main() {
    let mut filter: Option<String> = None;
    let mut paths: Vec<String> = Vec::new();
    for arg in std::env::args().skip(1) {
        if let Some(prefix) = arg.strip_prefix("--subject=") {
            filter = Some(prefix.to_string());
        } else {
            paths.push(arg);
        }
    }
    let mut grand_records = 0_u64;
    let mut grand_chars = 0_u64;
    for path in &paths {
        let path = Path::new(path);
        if !path.exists() {
            println!("{}: (absent)", path.display());
            continue;
        }
        let database = match Database::create(path) {
            Ok(db) => db,
            Err(e) => {
                println!("{}: ERROR {e}", path.display());
                continue;
            }
        };
        let read = database.begin_read().expect("read txn");
        let chunks = match read.open_table(CHUNKS) {
            Ok(table) => table,
            Err(_) => {
                println!("{}: no chunks table", path.display());
                continue;
            }
        };
        let mut records = 0_u64;
        let mut chars = 0_u64;
        let mut by_kind: BTreeMap<String, (u64, u64)> = BTreeMap::new();
        for entry in chunks.iter().expect("iter") {
            let (_key, value) = entry.expect("entry");
            let Ok(record) = bincode::deserialize::<ChunkRecord>(value.value()) else {
                continue;
            };
            if let Some(prefix) = &filter {
                if !matches(&record.subject, prefix) {
                    continue;
                }
            }
            let n = record.content.chars().count() as u64;
            records += 1;
            chars += n;
            let slot = by_kind.entry(record.kind).or_insert((0, 0));
            slot.0 += 1;
            slot.1 += n;
        }
        grand_records += records;
        grand_chars += chars;
        println!(
            "{}\n  records: {records}\n  chars: {chars}\n  tokens (chars/4): {}",
            path.display(),
            chars / 4
        );
        for (kind, (count, kchars)) in &by_kind {
            println!(
                "    {kind}: {count} record(s), {kchars} chars, {} tokens",
                kchars / 4
            );
        }
    }
    println!(
        "TOTAL: {grand_records} record(s), {grand_chars} chars, {} tokens",
        grand_chars / 4
    );
}
