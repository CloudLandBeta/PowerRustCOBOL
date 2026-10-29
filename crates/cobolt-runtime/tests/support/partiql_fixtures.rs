// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The `CUSTOMERS` and `ORDERS` files of spec 108 §7, for the PartiQL tests:
//! their record descriptions, keys, a builder for each of the three engines,
//! and record images built the way the runtime writes them.
//!
//! Test support only.
#![allow(dead_code)]

use cobolt_runtime::files::{encode_field, FieldPos};
use cobolt_runtime::indexed::{IndexedFile, IndexedStore, KeySpec, OpenMode};
use cobolt_runtime::indexed_disk::DiskIndexedFile;
use cobolt_runtime::indexed_redb::RedbIndexedFile;
use cobolt_runtime::partiql::schema::{KeyInput, Schema};
use cobolt_runtime::partiql::value::Value;

pub const CUSTOMERS_FD: &str = "FD CUSTOMERS.\n01 CUST-REC.\n   05 CUST-ID PIC 9(8).\n   05 CUST-NAME PIC X(40).\n   05 CUST-BALANCE PIC S9(6)V99.\n   05 BILLING.\n      10 CITY PIC X(20).\n";
pub const CUSTOMERS_LEN: usize = 76;

pub const ORDERS_FD: &str = "FD ORDERS.\n01 ORD-REC.\n   05 CUST-ID PIC 9(8).\n   05 ORDER-NO PIC 9(6).\n   05 ORDER-DATE PIC X(10).\n   05 ORDER-TOTAL PIC 9(7)V99.\n   05 ITEM-QTY PIC 9(3) OCCURS 5.\n";
pub const ORDERS_LEN: usize = 48;

pub fn customers_schema() -> Schema {
    Schema::from_fd_text(
        "CUSTOMERS",
        CUSTOMERS_FD,
        KeyInput { name: None, parts: vec![("CUST-ID".into(), 0, 8)], duplicates: false },
        vec![KeyInput { name: Some("CUST-NAME".into()), parts: vec![("CUST-NAME".into(), 8, 40)], duplicates: true }],
    )
    .expect("customers schema")
}

pub fn orders_schema() -> Schema {
    Schema::from_fd_text(
        "ORDERS",
        ORDERS_FD,
        KeyInput { name: None, parts: vec![("CUST-ID".into(), 0, 8), ("ORDER-NO".into(), 8, 6)], duplicates: false },
        vec![KeyInput { name: Some("BY-DATE".into()), parts: vec![("ORDER-DATE".into(), 14, 10)], duplicates: true }],
    )
    .expect("orders schema")
}

pub fn customers_keys() -> (KeySpec, Vec<KeySpec>) {
    (
        KeySpec { parts: Vec::new(), offset: 0, len: 8, duplicates: false },
        vec![KeySpec { parts: Vec::new(), offset: 8, len: 40, duplicates: true }],
    )
}

pub fn orders_keys() -> (KeySpec, Vec<KeySpec>) {
    (
        KeySpec::split(vec![(0, 8), (8, 6)], false),
        vec![KeySpec { parts: Vec::new(), offset: 14, len: 10, duplicates: true }],
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    Prcidxd1,
    Redb,
    Memory,
}

impl Engine {
    pub const ALL: [Engine; 3] = [Engine::Prcidxd1, Engine::Redb, Engine::Memory];

    pub fn name(self) -> &'static str {
        match self {
            Engine::Prcidxd1 => "PRCIDXD1",
            Engine::Redb => "redb",
            Engine::Memory => "STORAGE IS MEMORY",
        }
    }
}

/// A scratch directory under the system temp dir, removed on drop.
pub struct Scratch(pub std::path::PathBuf);

impl Scratch {
    pub fn new(tag: &str) -> Scratch {
        let p = std::env::temp_dir().join(format!("prc-partiql-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Scratch(p)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An open, empty file on `engine`, open for `mode` (created first when the
/// engine keeps a container on disk).
pub fn open_store(
    engine: Engine,
    dir: &Scratch,
    name: &str,
    len: usize,
    keys: (KeySpec, Vec<KeySpec>),
    mode: OpenMode,
) -> Box<dyn IndexedStore> {
    let path = dir.0.join(format!("{name}.{}", match engine {
        Engine::Prcidxd1 => "idx",
        Engine::Redb => "redb",
        Engine::Memory => "mem",
    }));
    let (p, a) = keys;
    let mut s: Box<dyn IndexedStore> = match engine {
        Engine::Prcidxd1 => Box::new(DiskIndexedFile::new(&path, len, p, a)),
        Engine::Redb => Box::new(RedbIndexedFile::new(&path, len, p, a)),
        Engine::Memory => {
            // A MEMORY file opens INPUT only unless it persists (the runtime's rule).
            let mut m = IndexedFile::new(&path, len, p, a);
            m.set_persist(true);
            Box::new(m)
        }
    };
    if !path.exists() || engine == Engine::Memory {
        assert_eq!(s.open(OpenMode::Output), "00", "create {}", engine.name());
        s.close();
    }
    let st = s.open(mode);
    assert!(st == "00" || st == "05", "open {}: {st}", engine.name());
    s
}

fn put(f: &FieldPos, v: Value, rec: &mut [u8]) {
    encode_field(f, &v, rec).expect("fixture value fits");
}

/// A `CUSTOMERS` record image.
pub fn customer(schema: &Schema, id: i64, name: &str, balance_cents: i64, city: &str) -> Vec<u8> {
    let mut rec = vec![b' '; CUSTOMERS_LEN];
    let f = |n: &str| &schema.fields.iter().find(|f| f.name == n).unwrap().occ[0];
    put(f("CUST-ID"), Value::int(id), &mut rec);
    put(f("CUST-NAME"), Value::text(name), &mut rec);
    put(
        f("CUST-BALANCE"),
        Value::Num(cobolt_runtime::value::CobolNumeric::new(balance_cents as i128, 2)),
        &mut rec,
    );
    put(f("CITY"), Value::text(city), &mut rec);
    rec
}

/// An `ORDERS` record image.
pub fn order(schema: &Schema, cust: i64, no: i64, date: &str, total_cents: i64, qty: [i64; 5]) -> Vec<u8> {
    let mut rec = vec![b' '; ORDERS_LEN];
    let f = |n: &str| schema.fields.iter().find(|f| f.name == n).unwrap();
    put(&f("CUST-ID").occ[0], Value::int(cust), &mut rec);
    put(&f("ORDER-NO").occ[0], Value::int(no), &mut rec);
    put(&f("ORDER-DATE").occ[0], Value::text(date), &mut rec);
    put(
        &f("ORDER-TOTAL").occ[0],
        Value::Num(cobolt_runtime::value::CobolNumeric::new(total_cents as i128, 2)),
        &mut rec,
    );
    for (i, q) in qty.iter().enumerate() {
        put(&f("ITEM-QTY").occ[i], Value::int(*q), &mut rec);
    }
    rec
}

/// A deterministic pseudo-random sequence (a fixed LCG), so runs repeat.
pub struct Lcg(pub u64);

impl Lcg {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}
