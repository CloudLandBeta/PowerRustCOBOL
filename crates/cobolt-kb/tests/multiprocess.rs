// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Several **processes** using one collection at once (spec 068 R8–R11, AC4).
//!
//! The test binary runs copies of itself as children: `child_worker` does
//! nothing unless `KB_CHILD_ROLE` is set, and the parent sets it. Phase 1: two
//! children search while a third indexes. Phase 2: two children index at the
//! same time. Afterwards redb's integrity check must pass and every document
//! must be indexed. This is the local half of AC4; the same run on an SMB share
//! between two machines is an operator step.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use cobolt_kb::convert::Converters;
use cobolt_kb::embed::HashingEmbedder;
use cobolt_kb::refresh::{put, refresh, Scope};
use cobolt_kb::search::search;
use cobolt_kb::store::Collection;

const COLLECTION: &str = "shared";

fn spawn(role: &str, location: &Path, tag: &str) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_worker", "--nocapture", "--test-threads=1"])
        .env("KB_CHILD_ROLE", role)
        .env("KB_CHILD_LOCATION", location)
        .env("KB_CHILD_TAG", tag)
        .spawn()
        .expect("spawn a child test process")
}

fn open(location: &Path) -> Collection {
    let mut c = Collection::open(location, COLLECTION).unwrap();
    c.set_write_wait(Duration::from_secs(60));
    c
}

/// The body of a child process. Inert when run as an ordinary test.
#[test]
fn child_worker() {
    let Ok(role) = std::env::var("KB_CHILD_ROLE") else {
        return;
    };
    let location = PathBuf::from(std::env::var("KB_CHILD_LOCATION").unwrap());
    let tag = std::env::var("KB_CHILD_TAG").unwrap();
    let c = open(&location);
    let conv = Converters::default();
    let cancel = AtomicBool::new(false);
    match role.as_str() {
        "search" => {
            let mut answered = 0;
            for i in 0..200 {
                let r = search(&c, &HashingEmbedder, &format!("policy number {}", i % 50), 5).unwrap();
                if !r.hits.is_empty() {
                    answered += 1;
                }
            }
            assert_eq!(answered, 200, "every search answered");
        }
        "index" => {
            for i in 0..30 {
                let body = format!("# {tag} note {i}\nWritten by process {tag}, entry {i}.");
                put(&c, &format!("{tag}/note{i:02}.md"), body.as_bytes(), &HashingEmbedder, &conv, &mut |_| {}, &cancel)
                    .unwrap();
            }
        }
        other => panic!("unknown role {other}"),
    }
}

fn wait_all(children: Vec<Child>) {
    for mut c in children {
        let status = c.wait().unwrap();
        assert!(status.success(), "a child process failed: {status}");
    }
}

#[test]
fn three_processes_share_one_collection() {
    if std::env::var("KB_CHILD_ROLE").is_ok() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let location = root.path();
    {
        let c = open(location);
        for i in 0..50 {
            std::fs::write(
                c.documents_dir().join(format!("policy{i:02}.md")),
                format!("# Policy number {i}\nThe rule for case {i}."),
            )
            .unwrap();
        }
        refresh(&c, &HashingEmbedder, &Converters::default(), Scope::All, &mut |_| {}, &AtomicBool::new(false))
            .unwrap();
    }

    let t1 = Instant::now();
    wait_all(vec![
        spawn("search", location, "s1"),
        spawn("search", location, "s2"),
        spawn("index", location, "w1"),
    ]);
    let phase1 = t1.elapsed();

    let t2 = Instant::now();
    wait_all(vec![spawn("index", location, "w2"), spawn("index", location, "w3")]);
    let phase2 = t2.elapsed();

    let mut c = open(location);
    assert!(c.check_integrity().unwrap(), "redb's integrity check passes");
    let sources = c.sources().unwrap();
    assert_eq!(sources.len(), 50 + 30 * 3, "every document from every process is indexed");
    for tag in ["w1", "w2", "w3"] {
        let hits = search(&c, &HashingEmbedder, &format!("written by process {tag}"), 3).unwrap().hits;
        assert!(hits.iter().any(|h| h.document.starts_with(tag)), "{tag}'s notes are found");
    }
    println!(
        "multi-process: phase 1 (2 searchers × 200 searches + 1 writer × 30 documents) {} ms; \
         phase 2 (2 writers × 30 documents) {} ms; {} documents indexed; integrity OK",
        phase1.as_millis(),
        phase2.as_millis(),
        sources.len()
    );
}
