// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 108, T7–T10 — the executor on `CUSTOMERS` and `ORDERS` (spec §7), each
//! loaded with `PARTIQL_N` records (default 100,000), on `PRCIDXD1`, redb and
//! `STORAGE IS MEMORY`. Keyed, range and scan reads equal a `START` / `READ`
//! loop record for record; every write form, the rows of R27, the units of
//! AC6, the `?` typing, and the throughput of each phase.
//!
//! One test per engine, one result block each (GOLDEN RULE #7).

#[path = "support/partiql_fixtures.rs"]
mod fx;

use std::time::Instant;

use cobolt_runtime::files::decode_field;
use cobolt_runtime::indexed::{IndexedStore, OpenMode, ReadDir, StartOp};
use cobolt_runtime::partiql::analyze::{analyze, Analyzed};
use cobolt_runtime::partiql::exec::{bind, execute, execute_unit, Arg, Outcome};
use cobolt_runtime::partiql::parser::{parse_script, parse_statement};
use cobolt_runtime::partiql::schema::Schema;
use cobolt_runtime::partiql::value::Value;
use cobolt_runtime::value::CobolNumeric;
use fx::*;

fn n_records(engine: Engine) -> usize {
    // `STORAGE IS MEMORY` answers every READ NEXT by copying and searching its whole key
    // list (indexed.rs `read_seq`), so a sequential read of n records costs O(n²): the same
    // run on 100,000 records would take hours. It is held to 3,000 (PARTIQL_N_MEMORY).
    let var = if engine == Engine::Memory { "PARTIQL_N_MEMORY" } else { "PARTIQL_N" };
    let default = if engine == Engine::Memory { 3_000 } else { 100_000 };
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn num(n: i64) -> Arg {
    Arg::Value(Value::int(n))
}

fn s(t: &str) -> Arg {
    Arg::Value(Value::text(t))
}

fn dec(cents: i64) -> Arg {
    Arg::Value(Value::Num(CobolNumeric::new(cents as i128, 2)))
}

/// Parse, analyse, bind and run one statement.
fn run(store: &mut dyn IndexedStore, schema: &Schema, sql: &str, args: &[Arg]) -> Outcome {
    let a = match parse_statement(sql).and_then(|p| analyze(&p, schema, false)) {
        Ok(a) => a,
        Err(e) => return Outcome::failed(e),
    };
    match bind(&[&a], args) {
        Ok(p) => execute(store, schema, &a, &p),
        Err(e) => Outcome::failed(e),
    }
}

fn run_unit(store: &mut dyn IndexedStore, schema: &Schema, sql: &str, args: &[Arg]) -> Outcome {
    let fail = Outcome::failed;
    let parsed = match parse_script(sql) {
        Ok(p) => p,
        Err(e) => return fail(e),
    };
    let mut list: Vec<Analyzed> = Vec::new();
    for p in &parsed {
        match analyze(p, schema, true) {
            Ok(a) => list.push(a),
            Err(e) => return fail(e),
        }
    }
    let refs: Vec<&Analyzed> = list.iter().collect();
    match bind(&refs, args) {
        Ok(p) => execute_unit(store, schema, &list, &p),
        Err(e) => fail(e),
    }
}

fn texts(o: &Outcome) -> Vec<String> {
    o.items.iter().map(|i| i.values.iter().map(Value::display).collect::<Vec<_>>().join("\t")).collect()
}

fn first_int(o: &Outcome) -> i64 {
    match &o.items[0].values[0] {
        Value::Num(n) => n.mantissa as i64,
        v => panic!("{v:?}"),
    }
}

#[derive(Default)]
struct Block {
    phases: Vec<(String, u128, String)>,
    checks: usize,
    failures: Vec<String>,
    forms: Vec<String>,
}

impl Block {
    fn check(&mut self, ok: bool, what: impl Into<String>) {
        self.checks += 1;
        if !ok {
            self.failures.push(what.into());
        }
    }

    fn phase(&mut self, name: &str, started: Instant, rate: String) {
        self.phases.push((name.to_string(), started.elapsed().as_millis(), rate));
    }

    fn form(&mut self, f: &str) {
        if !self.forms.iter().any(|x| x == f) {
            self.forms.push(f.to_string());
        }
    }
}

fn per_sec(count: usize, started: Instant, unit: &str) -> String {
    let secs = started.elapsed().as_secs_f64().max(1e-9);
    format!("{count} {unit} = {:.0} {unit}/s", count as f64 / secs)
}

/// The records a `START` / `READ NEXT` loop returns for ids in `[lo, hi]`.
fn loop_range(store: &mut dyn IndexedStore, schema: &Schema, lo: i64, hi: i64) -> Vec<(i64, String)> {
    let f = |n: &str| &schema.fields.iter().find(|f| f.name == n).unwrap().occ[0];
    store.set_key_of_reference(0);
    let mut out = Vec::new();
    if store.start(StartOp::Ge, format!("{lo:08}").as_bytes()) != "00" {
        return out;
    }
    while let (Some(rec), "00" | "02") = store.read_seq(ReadDir::Next) {
        let Value::Num(id) = decode_field(f("CUST-ID"), &rec) else { panic!() };
        if id.mantissa as i64 > hi {
            break;
        }
        out.push((id.mantissa as i64, decode_field(f("CUST-NAME"), &rec).display()));
    }
    out
}

fn load(
    engine: Engine,
    dir: &Scratch,
    n: usize,
    b: &mut Block,
) -> (Box<dyn IndexedStore>, Box<dyn IndexedStore>, Schema, Schema) {
    let cs = customers_schema();
    let os = orders_schema();
    let mut cust = open_store(engine, dir, "customers", CUSTOMERS_LEN, customers_keys(), OpenMode::Io);
    let mut ord = open_store(engine, dir, "orders", ORDERS_LEN, orders_keys(), OpenMode::Io);
    let names: Vec<String> = (0..2000).map(|i| if i == 0 { "Ana".to_string() } else { format!("Cliente {i:04}") }).collect();
    let t = Instant::now();
    for i in 0..n {
        let id = (i as i64 + 1) * 2; // even ids: odd ones are free for INSERT
        let name = &names[i % names.len()];
        let bal = ((i as i64 * 7919) % 2_000_001) - 1_000_000;
        assert_eq!(cust.write(&customer(&cs, id, name, bal, "Recife")), "00", "load customer {id}");
    }
    b.phase("load CUSTOMERS (WRITE verb path)", t, per_sec(n, t, "records"));
    let t = Instant::now();
    let customers_with_orders = (n / 10).max(1);
    for i in 0..n {
        let cust_id = ((i % customers_with_orders) as i64 + 1) * 2;
        let no = (i / customers_with_orders) as i64 + 1;
        let date = format!("2026-{:02}-{:02}", 1 + (i % 12), 1 + (i % 28));
        assert_eq!(ord.write(&order(&os, cust_id, no, &date, (i as i64 * 31) % 900_000, [1, 2, 3, 4, 5])), "00");
    }
    b.phase("load ORDERS (WRITE verb path)", t, per_sec(n, t, "records"));
    (cust, ord, cs, os)
}

fn run_engine(engine: Engine) {
    let n = n_records(engine);
    let dir = Scratch::new(match engine {
        Engine::Prcidxd1 => "exec-p",
        Engine::Redb => "exec-r",
        Engine::Memory => "exec-m",
    });
    let started = Instant::now();
    let mut b = Block::default();
    let (mut cust, mut ord, cs, os) = load(engine, &dir, n, &mut b);
    let rng = &mut Lcg(2026);

    // ── AC1: keyed reads equal read_key. ──
    b.form("SELECT keyed");
    let reads = (n / 10).clamp(200, 5000);
    let t = Instant::now();
    let mut bad = 0;
    for _ in 0..reads {
        let id = (rng.next() % (n as u64 + 10)) as i64 + 1;
        let o = run(cust.as_mut(), &cs, "SELECT CUST-ID, CUST-NAME FROM CUSTOMERS WHERE CUST-ID = ?", &[num(id)]);
        let present = id % 2 == 0 && id <= n as i64 * 2;
        if o.error.is_some() || o.count != usize::from(present) || o.class != Some(cobolt_runtime::partiql::plan::ReadClass::Keyed) {
            bad += 1;
        }
    }
    b.check(bad == 0, format!("keyed reads: {bad} wrong"));
    b.phase("SELECT keyed", t, per_sec(reads, t, "statements"));

    // ── AC1: range reads equal the START / READ NEXT loop. ──
    b.form("SELECT range (BETWEEN)");
    b.form("SELECT ... ORDER BY DESC");
    let windows = 40;
    let width = (n as i64 / 20).clamp(40, 2000);
    let t = Instant::now();
    let mut rows = 0usize;
    for _ in 0..windows {
        let lo = (rng.next() % (n as u64 * 2)) as i64;
        let hi = lo + width;
        let o = run(cust.as_mut(), &cs, "SELECT CUST-ID, CUST-NAME FROM CUSTOMERS WHERE CUST-ID BETWEEN ? AND ?", &[num(lo), num(hi)]);
        let want = loop_range(cust.as_mut(), &cs, lo, hi);
        let got: Vec<(i64, String)> = o
            .items
            .iter()
            .map(|i| (match &i.values[0] { Value::Num(n) => n.mantissa as i64, _ => -1 }, i.values[1].display()))
            .collect();
        rows += got.len();
        b.check(o.error.is_none() && got == want, format!("range {lo}..{hi}: {} vs {}", got.len(), want.len()));
        b.check(o.class == Some(cobolt_runtime::partiql::plan::ReadClass::Range), "range class");
        let d = run(cust.as_mut(), &cs, "SELECT CUST-ID FROM CUSTOMERS WHERE CUST-ID BETWEEN ? AND ? ORDER BY CUST-ID DESC", &[num(lo), num(hi)]);
        let mut want_desc: Vec<i64> = want.iter().map(|x| x.0).collect();
        want_desc.reverse();
        let got_desc: Vec<i64> = d.items.iter().map(|i| match &i.values[0] { Value::Num(n) => n.mantissa as i64, _ => -1 }).collect();
        b.check(got_desc == want_desc, "DESC is the reverse");
    }
    b.phase("SELECT range", t, per_sec(rows, t, "records"));

    // ── AC1: the scan equals the loop with an IF. ──
    b.form("SELECT scan (filter on a signed decimal)");
    let t = Instant::now();
    let scans = 3;
    let mut scanned = 0usize;
    for _ in 0..scans {
        let o = run(cust.as_mut(), &cs, "SELECT CUST-ID FROM CUSTOMERS WHERE CUST-BALANCE > 1000.50", &[]);
        let f = |nm: &str| &cs.fields.iter().find(|f| f.name == nm).unwrap().occ[0];
        cust.set_key_of_reference(0);
        assert_eq!(cust.start(StartOp::Ge, &[0u8; 8]), "00");
        let mut want = 0usize;
        while let (Some(rec), "00" | "02") = cust.read_seq(ReadDir::Next) {
            if let Value::Num(v) = decode_field(f("CUST-BALANCE"), &rec) {
                if v.cmp(&CobolNumeric::new(100050, 2)) == std::cmp::Ordering::Greater {
                    want += 1;
                }
            }
        }
        b.check(o.error.is_none() && o.count == want && o.class == Some(cobolt_runtime::partiql::plan::ReadClass::Scan), format!("scan {} vs {want}", o.count));
        scanned += n;
    }
    b.phase("SELECT scan", t, per_sec(scanned, t, "records"));

    // ── AC1: duplicates through an alternate key. ──
    b.form("SELECT through an alternate key (duplicates)");
    let o = run(cust.as_mut(), &cs, "SELECT CUST-ID FROM CUSTOMERS.\"CUST-NAME\" WHERE CUST-NAME = 'Ana'", &[]);
    let expect = (0..n).filter(|i| i % 2000 == 0).count();
    b.check(o.error.is_none() && o.count == expect, format!("duplicates: {} vs {expect}", o.count));
    b.check(run(cust.as_mut(), &cs, "SELECT * FROM CUSTOMERS WHERE CUST-ID = 1", &[]).count == 0 && run(cust.as_mut(), &cs, "SELECT * FROM CUSTOMERS WHERE CUST-ID = 1", &[]).status == "00", "no match: 0 with 00");
    // IN, in key order, a duplicate value and an absent one.
    b.form("SELECT ... IN [..]");
    let o = run(cust.as_mut(), &cs, "SELECT CUST-ID FROM CUSTOMERS WHERE CUST-ID IN [?, ?, ?, ?]", &[num(40), num(2), num(41), num(40)]);
    b.check(texts(&o) == vec!["2".to_string(), "40".to_string()], format!("IN: {:?}", texts(&o)));

    // ── AC2-style projection: Number with its decimal point, a group path. ──
    b.form("SELECT group path, signed decimal");
    cust.write(&customer(&cs, 99_999_991, "Marta", -12345, "Recife"));
    let o = run(cust.as_mut(), &cs, "SELECT CUST-ID, CUST-BALANCE, BILLING.CITY FROM CUSTOMERS WHERE CUST-ID = 99999991", &[]);
    b.check(texts(&o) == vec!["99999991\t-123.45\tRecife".to_string()], format!("projection {:?}", texts(&o)));
    let o = run(cust.as_mut(), &cs, "SELECT * FROM CUSTOMERS WHERE CUST-ID = 99999991", &[]);
    b.check(o.items[0].cols.len() == 4 && o.items[0].cols[3].name == "CITY", "star columns");
    cust.delete(Some(b"99999991"));

    // ── AC3: INSERT / UPDATE / DELETE. ──
    b.form("INSERT");
    b.form("UPDATE ... SET (arithmetic) ... RETURNING");
    b.form("DELETE ... RETURNING");
    let o = run(cust.as_mut(), &cs, "INSERT INTO CUSTOMERS VALUE {'CUST-ID': ?, 'CUST-NAME': 'Ana'}", &[num(1)]);
    b.check(o.status == "00" && o.count == 1, "insert");
    let o = run(cust.as_mut(), &cs, "SELECT CUST-BALANCE, CITY FROM CUSTOMERS WHERE CUST-ID = 1", &[]);
    b.check(texts(&o) == vec!["0.00\t".to_string()], format!("initial values {:?}", texts(&o)));
    let o = run(cust.as_mut(), &cs, "INSERT INTO CUSTOMERS VALUE {'CUST-ID': ?, 'CUST-NAME': 'Ana'}", &[num(1)]);
    b.check(o.status == "22" && o.count == 0 && o.last_error().starts_with("DuplicateItemException"), format!("dup {:?}", o.last_error()));
    let o = run(cust.as_mut(), &cs, "UPDATE CUSTOMERS SET CUST-BALANCE = CUST-BALANCE + ? WHERE CUST-ID = ?", &[dec(5050), num(1)]);
    b.check(o.status == "00" && o.count == 1, "update");
    let o = run(cust.as_mut(), &cs, "UPDATE CUSTOMERS SET CUST-BALANCE = CUST-BALANCE + ? WHERE CUST-ID = ? AND CUST-BALANCE >= ?", &[dec(100), num(1), dec(999999)]);
    b.check(o.status == "23" && o.count == 0 && o.last_error().starts_with("ConditionalCheckFailedException"), format!("cond {:?}", o.last_error()));
    let o = run(cust.as_mut(), &cs, "SELECT CUST-BALANCE FROM CUSTOMERS WHERE CUST-ID = 1", &[]);
    b.check(texts(&o) == vec!["50.50".to_string()], "failed update changed nothing");
    let o = run(cust.as_mut(), &cs, "UPDATE CUSTOMERS SET CUST-BALANCE = 1 WHERE CUST-ID = ?", &[num(3)]);
    b.check(o.status == "23" && o.count == 0, "update of an absent key is 23");
    let o = run(cust.as_mut(), &cs, "UPDATE CUSTOMERS SET CUST-NAME = 'Beta' REMOVE CITY WHERE CUST-ID = 1 RETURNING MODIFIED OLD *", &[]);
    b.check(o.count == 1 && texts(&o) == vec!["Ana\t".to_string()], format!("returning {:?}", texts(&o)));
    let o = run(cust.as_mut(), &cs, "UPDATE CUSTOMERS SET CUST-NAME = 'Gama' WHERE CUST-ID = 1 RETURNING ALL NEW *", &[]);
    b.check(o.items.len() == 1 && o.items[0].values[1] == Value::text("Gama"), "returning all new");
    let o = run(cust.as_mut(), &cs, "DELETE FROM CUSTOMERS WHERE CUST-ID = 99999999", &[]);
    b.check(o.status == "00" && o.count == 0, "absent delete is 0 with 00");
    let o = run(cust.as_mut(), &cs, "DELETE FROM CUSTOMERS WHERE CUST-ID = 1 AND CUST-BALANCE > 1000", &[]);
    b.check(o.status == "23", "delete with a failed condition");
    let o = run(cust.as_mut(), &cs, "DELETE FROM CUSTOMERS WHERE CUST-ID = 1 RETURNING ALL OLD *", &[]);
    b.check(o.count == 1 && o.items[0].values[1] == Value::text("Gama"), "delete returning");
    // R4: a Number that does not fit refuses and changes nothing.
    let o = run(cust.as_mut(), &cs, "INSERT INTO CUSTOMERS VALUE {'CUST-ID': 12345678901}", &[]);
    b.check(o.status == "94" && o.last_error().contains("integer digits"), format!("overflow {:?}", o.last_error()));
    // A string longer than a key field never matches a truncated record.
    let long = "x".repeat(60);
    let o = run(cust.as_mut(), &cs, "UPDATE CUSTOMERS SET CITY = 'x' WHERE CUST-ID = ?", &[s(&long)]);
    b.check(o.status == "94" || o.status == "23", "a mistyped key");
    // R29: no parameter value in an error.
    let o = run(cust.as_mut(), &cs, "SELECT * FROM CUSTOMERS WHERE CUST-ID = ?", &[s("SECRET-42")]);
    b.check(o.status == "94" && !o.last_error().contains("SECRET-42"), "secret stays out of LastError");
    let o = run(cust.as_mut(), &cs, "UPDATE CUSTOMERS SET CUST-NAME = ? WHERE CUST-ID = 2 AND CUST-NAME = ?", &[s("a"), num(5)]);
    b.check(o.status == "94" && !o.last_error().contains('5'), "no number in LastError");

    // ── AC5 on ORDERS. ──
    b.form("SELECT ITEM-QTY[1], size(ITEM-QTY)");
    b.form("SELECT through \"ORDERS\".\"BY-DATE\" (begins_with)");
    b.form("SELECT on a split key (= and BETWEEN)");
    b.form("REMOVE ITEM-QTY[2]");
    let o = run(ord.as_mut(), &os, "SELECT ITEM-QTY[1], size(ITEM-QTY) FROM ORDERS WHERE CUST-ID = ? AND ORDER-NO = ?", &[num(2), num(1)]);
    b.check(texts(&o) == vec!["1\t5".to_string()] && o.class == Some(cobolt_runtime::partiql::plan::ReadClass::Keyed), format!("occurs {:?}", texts(&o)));
    let o = run(ord.as_mut(), &os, "SELECT ORDER-NO FROM ORDERS WHERE CUST-ID = ? AND ORDER-NO BETWEEN ? AND ?", &[num(2), num(2), num(4)]);
    b.check(o.class == Some(cobolt_runtime::partiql::plan::ReadClass::Range) && o.count == 3 && texts(&o) == vec!["2", "3", "4"], format!("split key {:?} {:?}", o.class, texts(&o)));
    let o = run(ord.as_mut(), &os, "SELECT ORDER-NO FROM ORDERS WHERE CUST-ID = ? ORDER BY ORDER-NO DESC", &[num(2)]);
    let want = n.div_ceil((n / 10).max(1));
    b.check(o.count >= want.min(10) && first_int(&o) as usize == o.count, format!("desc {} {}", o.count, first_int(&o)));
    let o = run(ord.as_mut(), &os, "SELECT ORDER-NO FROM ORDERS.\"BY-DATE\" WHERE begins_with(ORDER-DATE, '2026-10')", &[]);
    let by_loop = (0..n).filter(|i| 1 + (i % 12) == 10).count();
    b.check(o.error.is_none() && o.class == Some(cobolt_runtime::partiql::plan::ReadClass::Range) && o.count == by_loop, format!("by-date {} vs {by_loop}", o.count));
    let o = run(ord.as_mut(), &os, "UPDATE ORDERS REMOVE ITEM-QTY[2] WHERE CUST-ID = 2 AND ORDER-NO = 1", &[]);
    b.check(o.status == "00", "remove");
    let o = run(ord.as_mut(), &os, "SELECT ITEM-QTY[1], ITEM-QTY[2], ITEM-QTY[3] FROM ORDERS WHERE CUST-ID = 2 AND ORDER-NO = 1", &[]);
    b.check(texts(&o) == vec!["1\t0\t3".to_string()], format!("remove leaves the others {:?}", texts(&o)));
    let o = run(ord.as_mut(), &os, "SELECT * FROM ORDERS WHERE CUST-ID = 2 AND ORDER-NO = 1", &[]);
    b.check(o.items[0].cols.len() == 4 + 5 && o.items[0].cols[4].name == "ITEM-QTY[1]", "star over OCCURS: one column per occurrence");
    b.form("contains, attribute_type, size on a scan");
    let o = run(cust.as_mut(), &cs, "SELECT CUST-ID FROM CUSTOMERS WHERE contains(CUST-NAME, 'Cliente 0001') AND attribute_type(CUST-BALANCE, 'N') AND size(CUST-NAME) = 12", &[]);
    b.check(o.error.is_none() && o.count == (0..n).filter(|i| i % 2000 == 1).count(), format!("contains {}", o.count));

    // ── AC6: units. ──
    b.form("ExecuteTransaction: EXISTS + UPDATE + UPDATE");
    let acc = |id: i64, cents: i64| customer(&cs, id, "Conta", cents, "Recife");
    cust.write(&acc(90_000_001, 10_000));
    cust.write(&acc(90_000_003, 10_000));
    let unit = "EXISTS(SELECT * FROM CUSTOMERS WHERE CUST-ID = ? AND CUST-BALANCE >= ?); UPDATE CUSTOMERS SET CUST-BALANCE = CUST-BALANCE - ? WHERE CUST-ID = ?; UPDATE CUSTOMERS SET CUST-BALANCE = CUST-BALANCE + ? WHERE CUST-ID = ?";
    let o = run_unit(cust.as_mut(), &cs, unit, &[num(90_000_001), dec(3000), dec(3000), num(90_000_001), dec(3000), num(90_000_003)]);
    b.check(o.status == "00" && o.count == 3, format!("unit {:?}", o.last_error()));
    let bal = |st: &mut dyn IndexedStore, id: i64| texts(&run(st, &cs, "SELECT CUST-BALANCE FROM CUSTOMERS WHERE CUST-ID = ?", &[num(id)]))[0].clone();
    b.check(bal(cust.as_mut(), 90_000_001) == "70.00" && bal(cust.as_mut(), 90_000_003) == "130.00", "unit moved the amount");
    let o = run_unit(cust.as_mut(), &cs, unit, &[num(90_000_001), dec(999_999), dec(3000), num(90_000_001), dec(3000), num(90_000_003)]);
    b.check(o.status == "23" && o.count == 0 && o.last_error().starts_with("TransactionCanceledException: statement 1:"), format!("insufficient {:?}", o.last_error()));
    b.check(bal(cust.as_mut(), 90_000_001) == "70.00" && bal(cust.as_mut(), 90_000_003) == "130.00", "neither record changed");
    // A third statement on an absent key: the first two undone; a WRITE before the unit stays, and ROLLBACK takes it away.
    cust.commit();
    cust.write(&acc(90_000_005, 500));
    let o = run_unit(
        cust.as_mut(),
        &cs,
        "UPDATE CUSTOMERS SET CUST-BALANCE = 1 WHERE CUST-ID = ?; UPDATE CUSTOMERS SET CUST-BALANCE = 2 WHERE CUST-ID = ?; UPDATE CUSTOMERS SET CUST-BALANCE = 3 WHERE CUST-ID = ?",
        &[num(90_000_001), num(90_000_003), num(90_000_009)],
    );
    b.check(o.status == "23" && o.count == 0 && o.items.is_empty() && o.last_error().contains("statement 3"), format!("mid-unit {:?}", o.last_error()));
    b.check(bal(cust.as_mut(), 90_000_001) == "70.00" && bal(cust.as_mut(), 90_000_003) == "130.00", "first two undone");
    b.check(bal(cust.as_mut(), 90_000_005) == "5.00", "the WRITE before the unit is still there");
    // R26: the same record twice is refused before anything runs.
    let o = run_unit(cust.as_mut(), &cs, "UPDATE CUSTOMERS SET CUST-BALANCE = 1 WHERE CUST-ID = ?; UPDATE CUSTOMERS SET CUST-BALANCE = 2 WHERE CUST-ID = ?", &[num(90_000_001), num(90_000_001)]);
    b.check(o.status == "94" && bal(cust.as_mut(), 90_000_001) == "70.00", format!("same record twice {:?}", o.last_error()));
    // RETURNING and SELECT items of a unit, in order.
    let o = run_unit(cust.as_mut(), &cs, "SELECT CUST-ID FROM CUSTOMERS WHERE CUST-ID = ?; UPDATE CUSTOMERS SET CITY = 'Olinda' WHERE CUST-ID = ? RETURNING ALL NEW *", &[num(90_000_001), num(90_000_003)]);
    b.check(o.count == 2 && o.items.len() == 2, format!("unit items {}", o.items.len()));
    // EXISTS outside a unit is refused.
    let o = run(cust.as_mut(), &cs, "EXISTS(SELECT * FROM CUSTOMERS WHERE CUST-ID = 1)", &[]);
    b.check(o.status == "94" && o.last_error().contains("transaction"), "EXISTS outside a unit");
    cust.rollback();

    // ── Throughput of writes. ──
    let writes = (n / 10).clamp(100, 5000);
    let t = Instant::now();
    for i in 0..writes {
        let o = run(cust.as_mut(), &cs, "INSERT INTO CUSTOMERS VALUE {'CUST-ID': ?, 'CUST-NAME': ?, 'CUST-BALANCE': ?}", &[num(5_000_001 + 2 * i as i64), s("Novo"), dec(1000)]);
        assert_eq!(o.status, "00", "insert {i}: {:?}", o.last_error());
    }
    b.phase("INSERT", t, per_sec(writes, t, "records"));
    let t = Instant::now();
    for i in 0..writes {
        let o = run(cust.as_mut(), &cs, "UPDATE CUSTOMERS SET CUST-BALANCE = CUST-BALANCE + ? WHERE CUST-ID = ?", &[dec(1), num(5_000_001 + 2 * i as i64)]);
        assert_eq!(o.status, "00", "update {i}");
    }
    b.phase("UPDATE", t, per_sec(writes, t, "records"));
    let t = Instant::now();
    let units = (writes / 5).max(20);
    for i in 0..units {
        let a = 5_000_001 + 2 * (2 * i as i64);
        let c = a + 2;
        let o = run_unit(cust.as_mut(), &cs, unit, &[num(a), dec(0), dec(5), num(a), dec(5), num(c)]);
        assert_eq!(o.count, 3, "unit {i}: {:?}", o.last_error());
    }
    b.phase("units (EXISTS + 2 UPDATE)", t, per_sec(units, t, "units"));
    let t = Instant::now();
    for i in 0..writes {
        let o = run(cust.as_mut(), &cs, "DELETE FROM CUSTOMERS WHERE CUST-ID = ?", &[num(5_000_001 + 2 * i as i64)]);
        assert_eq!((o.status, o.count), ("00", 1), "delete {i}");
    }
    b.phase("DELETE", t, per_sec(writes, t, "records"));
    cust.commit();

    // ── A file open for INPUT: 48 / 49. ──
    cust.close();
    ord.close();
    drop(ord);
    // A MEMORY file keeps its records in the object: reopen the same one.
    let mut ro = if engine == Engine::Memory {
        assert_eq!(cust.open(OpenMode::Input), "00");
        cust
    } else {
        open_store(engine, &dir, "customers", CUSTOMERS_LEN, customers_keys(), OpenMode::Input)
    };
    let o = run(ro.as_mut(), &cs, "INSERT INTO CUSTOMERS VALUE {'CUST-ID': 7}", &[]);
    b.check(o.status == "48", format!("INSERT on INPUT: {}", o.status));
    let o = run(ro.as_mut(), &cs, "UPDATE CUSTOMERS SET CITY = 'x' WHERE CUST-ID = 2", &[]);
    b.check(o.status == "49", format!("UPDATE on INPUT: {}", o.status));
    let o = run(ro.as_mut(), &cs, "SELECT CUST-ID FROM CUSTOMERS WHERE CUST-ID = 2", &[]);
    b.check(o.count == 1, "SELECT on INPUT");
    ro.close();

    println!("\n=== test_partiql_executor — {} — {n} records in each file ===", engine.name());
    println!("forms exercised: {}", b.forms.join("; "));
    println!("PICTUREs: 9(8) key, X(40) alternate key WITH DUPLICATES, S9(6)V99, 9(7)V99, 9(3) OCCURS 5, split key 9(8)+9(6), group BILLING.CITY X(20)");
    for (name, ms, rate) in &b.phases {
        println!("  {name:<36} {ms:>8} ms   {rate}");
    }
    println!("checks: {} passed, {} failed; total {} ms", b.checks - b.failures.len(), b.failures.len(), started.elapsed().as_millis());
    for f in &b.failures {
        println!("  FAIL {f}");
    }
    assert!(b.failures.is_empty(), "{} failures", b.failures.len());
}

#[test]
fn partiql_executor_prcidxd1() {
    run_engine(Engine::Prcidxd1);
}

#[test]
fn partiql_executor_redb() {
    run_engine(Engine::Redb);
}

#[test]
fn partiql_executor_memory() {
    run_engine(Engine::Memory);
}

/// An engine that delegates to another and fails the Nth `rewrite` with status 90.
struct FaultStore {
    inner: Box<dyn IndexedStore>,
    rewrites: usize,
    fail_from: usize,
}

impl IndexedStore for FaultStore {
    fn open(&mut self, mode: OpenMode) -> &'static str {
        self.inner.open(mode)
    }
    fn close(&mut self) -> &'static str {
        self.inner.close()
    }
    fn write(&mut self, rec: &[u8]) -> &'static str {
        self.inner.write(rec)
    }
    fn read_key(&mut self, key: &[u8]) -> (Option<Vec<u8>>, &'static str) {
        self.inner.read_key(key)
    }
    fn read_seq(&mut self, dir: ReadDir) -> (Option<Vec<u8>>, &'static str) {
        self.inner.read_seq(dir)
    }
    fn start(&mut self, op: StartOp, key: &[u8]) -> &'static str {
        self.inner.start(op, key)
    }
    fn rewrite(&mut self, rec: &[u8], random_key: Option<&[u8]>) -> &'static str {
        self.rewrites += 1;
        if self.rewrites >= self.fail_from {
            return "90";
        }
        self.inner.rewrite(rec, random_key)
    }
    fn delete(&mut self, random_key: Option<&[u8]>) -> &'static str {
        self.inner.delete(random_key)
    }
    fn set_key_of_reference(&mut self, kor: usize) {
        self.inner.set_key_of_reference(kor)
    }
    fn is_open(&self) -> bool {
        self.inner.is_open()
    }
}

/// T9: when undoing a unit itself fails, the answer is 90 naming the statement — never silence.
#[test]
fn partiql_unit_compensation_failure_is_90() {
    let t0 = Instant::now();
    let dir = Scratch::new("fault");
    let cs = customers_schema();
    let mut store = FaultStore {
        inner: open_store(Engine::Prcidxd1, &dir, "customers", CUSTOMERS_LEN, customers_keys(), OpenMode::Io),
        rewrites: 0,
        fail_from: 3,
    };
    for id in [2, 4] {
        assert_eq!(store.write(&customer(&cs, id, "x", 1000, "Recife")), "00");
    }
    let o = run_unit(
        &mut store,
        &cs,
        "UPDATE CUSTOMERS SET CUST-BALANCE = 1 WHERE CUST-ID = ?; UPDATE CUSTOMERS SET CUST-BALANCE = 2 WHERE CUST-ID = ?; UPDATE CUSTOMERS SET CUST-BALANCE = 3 WHERE CUST-ID = ?",
        &[num(2), num(4), num(6)],
    );
    let text = o.last_error();
    println!("\n=== test_partiql_executor — compensation failure ===");
    println!("forms exercised: ExecuteTransaction with a failing third statement and a failing undo (engine fault injected on the 3rd rewrite)");
    println!("answer: status {} count {} LastError {text:?}; elapsed {} ms", o.status, o.count, t0.elapsed().as_millis());
    assert_eq!(o.status, "90");
    assert_eq!(o.count, 0);
    assert!(text.starts_with("TransactionCanceledException: statement 3:"), "{text}");
    assert!(text.contains("undoing the unit failed"), "{text}");
}
