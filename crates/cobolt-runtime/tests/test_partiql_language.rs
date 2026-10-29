// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 108 — the language core: the parser against a table derived from the
//! PartiQL grammar (reference only), the analysis errors of AC4 with no file,
//! the schema, and the read classes of AC1 checked against a brute-force filter.
//!
//! One result block at the end (GOLDEN RULE #7).

#[path = "support/partiql_fixtures.rs"]
mod fx;

use std::time::Instant;

use cobolt_runtime::indexed::{IndexedStore, OpenMode, ReadDir, StartOp};
use cobolt_runtime::partiql::analyze::{analyze, RStmt};
use cobolt_runtime::partiql::exec::{bind, execute, Arg};
use cobolt_runtime::partiql::parser::{parse_script, parse_statement, Statement};
use cobolt_runtime::partiql::plan::{classify, ReadClass};
use cobolt_runtime::partiql::schema::{KeyInput, Schema};
use cobolt_runtime::partiql::value::Value;
use fx::*;

#[derive(Default)]
struct Tally {
    rows: Vec<(String, usize, usize)>,
    failures: Vec<String>,
}

impl Tally {
    fn section(&mut self, name: &str, cases: Vec<Result<(), String>>) {
        let ok = cases.iter().filter(|c| c.is_ok()).count();
        let bad = cases.len() - ok;
        for c in cases {
            if let Err(e) = c {
                self.failures.push(format!("{name}: {e}"));
            }
        }
        self.rows.push((name.to_string(), ok, bad));
    }
}

/// Cases derived, in our own words and examples, from the official PartiQL
/// grammar (reference only): each accepted form of `SELECT`, `INSERT`,
/// `UPDATE`, `DELETE` and `REMOVE`, each operator, and the precedence of §5.1.
/// `Ok(grouping)` is the grouping the grammar gives the WHERE; `Err(note)` is
/// a deliberate departure of our subset.
fn grammar_table() -> Vec<(&'static str, Result<&'static str, &'static str>)> {
    vec![
        // Comparison binds tighter than NOT, NOT than AND, AND than OR.
        ("SELECT * FROM T WHERE A = 1 AND B = 2 OR C = 3", Ok("(or (and (= A 1) (= B 2)) (= C 3))")),
        ("SELECT * FROM T WHERE A = 1 OR B = 2 AND C = 3", Ok("(or (= A 1) (and (= B 2) (= C 3)))")),
        ("SELECT * FROM T WHERE NOT A = 1 AND B = 2", Ok("(and (not (= A 1)) (= B 2))")),
        ("SELECT * FROM T WHERE NOT NOT A = 1", Ok("(not (not (= A 1)))")),
        ("SELECT * FROM T WHERE (A = 1 OR B = 2) AND C = 3", Ok("(and (or (= A 1) (= B 2)) (= C 3))")),
        ("SELECT * FROM T WHERE A = 1 AND (B = 2 OR (C = 3 AND D = 4))", Ok("(and (= A 1) (or (= B 2) (and (= C 3) (= D 4))))")),
        // Every comparison operator, both spellings of "not equal".
        ("SELECT * FROM T WHERE A < 1", Ok("(< A 1)")),
        ("SELECT * FROM T WHERE A <= 1", Ok("(<= A 1)")),
        ("SELECT * FROM T WHERE A > 1", Ok("(> A 1)")),
        ("SELECT * FROM T WHERE A >= 1", Ok("(>= A 1)")),
        ("SELECT * FROM T WHERE A <> 1", Ok("(<> A 1)")),
        ("SELECT * FROM T WHERE A != 1", Ok("(<> A 1)")),
        // BETWEEN owns the AND that follows its lower bound.
        ("SELECT * FROM T WHERE A BETWEEN 1 AND 2", Ok("(between A 1 2)")),
        ("SELECT * FROM T WHERE A BETWEEN 1 AND 2 AND B = 3", Ok("(and (between A 1 2) (= B 3))")),
        ("SELECT * FROM T WHERE B = 3 AND A BETWEEN 1 AND 2 OR C = 4", Ok("(or (and (= B 3) (between A 1 2)) (= C 4))")),
        ("SELECT * FROM T WHERE NOT A BETWEEN 1 AND 2", Ok("(not (between A 1 2))")),
        // IN takes a bracketed list; ours also takes parentheses (departure 8).
        ("SELECT * FROM T WHERE A IN [1, 2]", Ok("(in A [1 2])")),
        ("SELECT * FROM T WHERE A IN (1, 2)", Ok("(in A [1 2])")),
        ("SELECT * FROM T WHERE A IN [1] OR B IN ['x', 'y']", Ok("(or (in A [1]) (in B ['x' 'y']))")),
        // Functions are conditions on their own.
        ("SELECT * FROM T WHERE begins_with(A, 'x') AND B = 1", Ok("(and (begins_with A 'x') (= B 1))")),
        ("SELECT * FROM T WHERE contains(A, 'x') OR NOT contains(B, 'y')", Ok("(or (contains A 'x') (not (contains B 'y')))")),
        ("SELECT * FROM T WHERE attribute_type(A, 'N')", Ok("(attribute_type A 'N')")),
        ("SELECT * FROM T WHERE size(A) > 3", Ok("(> (size A) 3)")),
        ("SELECT * FROM T WHERE A IS MISSING", Ok("(missing A)")),
        ("SELECT * FROM T WHERE A IS NOT MISSING AND B = 1", Ok("(and (present A) (= B 1))")),
        // Arithmetic is left associative and binds tighter than a comparison.
        ("SELECT * FROM T WHERE A - 1 + 2 > 0", Ok("(> (+ (- A 1) 2) 0)")),
        ("SELECT * FROM T WHERE (A + 1) > 2", Ok("(> (+ A 1) 2)")),
        // Paths.
        ("SELECT * FROM T WHERE G.F = 1", Ok("(= G.F 1)")),
        ("SELECT * FROM T WHERE ITEM[2] = 1", Ok("(= ITEM[2] 1)")),
        ("SELECT * FROM T WHERE G.ITEM[2] = 1", Ok("(= G.ITEM[2] 1)")),
        // Our departures, stated.
        ("SELECT * FROM T WHERE QTY-1 = 1", Ok("(= QTY-1 1)")), // a hyphen is part of a name (departure 1)
        ("SELECT * FROM T WHERE QTY - 1 = 1", Ok("(= (- QTY 1) 1)")),
        ("SELECT * FROM T WHERE A LIKE 'x%'", Err("LIKE")),
        ("SELECT * FROM T WHERE A = TRUE", Err("Boolean")),
        ("SELECT * FROM T WHERE A = NULL", Err("NULL")),
        ("SELECT * FROM T WHERE A IN <<1, 2>>", Err("set value")),
        // The accepted forms of each statement (their WHERE, when they have one).
        ("SELECT A, B FROM T", Ok("")),
        ("SELECT A + 1, size(B), G.F FROM T ORDER BY A DESC", Ok("")),
        ("SELECT * FROM \"T\".\"I\" WHERE A = 1 ORDER BY A ASC", Ok("(= A 1)")),
        ("INSERT INTO T VALUE {'A': 1, 'B': 'x'}", Ok("")),
        ("UPDATE T SET A = 1 WHERE K = 1", Ok("(= K 1)")),
        ("UPDATE T SET A = A + 1 SET B = 'x' WHERE K = 1 RETURNING ALL OLD *", Ok("(= K 1)")),
        ("UPDATE T REMOVE A WHERE K = 1", Ok("(= K 1)")),
        ("UPDATE T SET A = 1 REMOVE B SET C = 2 WHERE K = 1 RETURNING MODIFIED NEW *", Ok("(= K 1)")),
        ("DELETE FROM T WHERE K = 1", Ok("(= K 1)")),
        ("DELETE FROM T WHERE K = 1 RETURNING ALL OLD *", Ok("(= K 1)")),
    ]
}

fn where_sexp(text: &str) -> Result<String, String> {
    let p = parse_statement(text).map_err(|e| e.text())?;
    Ok(match p.statement {
        Statement::Select(s) => s.filter.map(|w| w.cond.sexp()).unwrap_or_default(),
        Statement::Update(u) => u.filter.cond.sexp(),
        Statement::Delete(d) => d.filter.cond.sexp(),
        Statement::Insert(_) | Statement::Exists(_) => String::new(),
    })
}

fn analyse(sql: &str, schema: &Schema) -> Result<cobolt_runtime::partiql::analyze::Analyzed, cobolt_runtime::partiql::PartiqlError> {
    analyze(&parse_statement(sql)?, schema, false)
}

/// AC4: each statement is refused with 94 and a message that names the construct.
fn ac4(schema: &Schema, orders: &Schema) -> Vec<Result<(), String>> {
    let cases: Vec<(&Schema, &str, &str, &str)> = vec![
        (schema, "SELECT COUNT(*) FROM CUSTOMERS", "ValidationException", "COUNT"),
        (schema, "SELECT * FROM CUSTOMERS LIMIT 10", "ValidationException", "LIMIT"),
        (schema, "SELECT * FROM CUSTOMERS WHERE CUST-BALANCE = 'ten'", "ValidationException", "Number"),
        (schema, "SELECT * FROM CUSTOMERS ORDER BY CUST-BALANCE", "ValidationException", "ORDER BY CUST-BALANCE"),
        (schema, "UPDATE CUSTOMERS SET CUST-NAME = 'x' WHERE CUST-NAME = 'Ana'", "ValidationException", "RECORD KEY"),
        (schema, "UPDATE CUSTOMERS SET CUST-ID = 1 WHERE CUST-ID = 2", "ValidationException", "RECORD KEY"),
        (schema, "SELECT * FROM CUSTOMERS WHERE NO-SUCH-FIELD = 1", "ValidationException", "NO-SUCH-FIELD"),
        (schema, "SELECT * FROM INVOICES", "ResourceNotFoundException", "INVOICES"),
        (schema, "INSERT INTO CUSTOMERS VALUE {'CUST-NAME': 'x'}", "ValidationException", "CUST-ID"),
        (orders, "UPDATE ORDERS SET ITEM-QTY = list_append(ITEM-QTY, [1]) WHERE CUST-ID = 1 AND ORDER-NO = 1", "ValidationException", "list_append"),
        (schema, "EXISTS(SELECT * FROM CUSTOMERS WHERE CUST-ID = 1)", "ValidationException", "transaction"),
        (orders, "SELECT ITEM-QTY[0] FROM ORDERS", "ValidationException", "[0]"),
        (orders, "SELECT ITEM-QTY[6] FROM ORDERS", "ValidationException", "past the last"),
        (schema, "SELECT * FROM CUSTOMERS WHERE CUST-ID = 'a'", "ValidationException", "Number"),
        (schema, "SELECT * FROM CUSTOMERS WHERE begins_with(CUST-ID, '1')", "ValidationException", "begins_with"),
        (schema, "SELECT size(CUST-BALANCE) FROM CUSTOMERS", "ValidationException", "size"),
        (schema, "SELECT * FROM CUSTOMERS WHERE CUST-NAME = 5", "ValidationException", "String"),
        (schema, "SELECT * FROM CUSTOMERS.\"NO-INDEX\"", "ResourceNotFoundException", "NO-INDEX"),
    ];
    let mut out = Vec::new();
    for (sc, sql, exc, needle) in cases {
        out.push(match parse_statement(sql).and_then(|p| analyze(&p, sc, false)) {
            Ok(_) => Err(format!("{sql}: accepted")),
            Err(e) if e.status != "94" => Err(format!("{sql}: status {}", e.status)),
            Err(e) if e.exception != exc => Err(format!("{sql}: {}", e.text())),
            Err(e) if !e.text().contains(needle) => Err(format!("{sql}: {} lacks {needle}", e.text())),
            Err(_) => Ok(()),
        });
    }
    // Two `?` and one argument: refused when the arguments are bound.
    let a = analyse("SELECT * FROM CUSTOMERS WHERE CUST-ID = ? AND CUST-NAME = ?", schema).unwrap();
    out.push(match bind(&[&a], &[Arg::Value(Value::int(1))]) {
        Err(e) if e.status == "94" && e.text().contains("2 ? parameter") => Ok(()),
        o => Err(format!("parameter count: {o:?}")),
    });
    // A mismatched argument names the argument, never its value (R29).
    let a = analyse("SELECT * FROM CUSTOMERS WHERE CUST-ID = ?", schema).unwrap();
    out.push(match bind(&[&a], &[Arg::Value(Value::text("SECRET-42"))]) {
        Err(e) if !e.text().contains("SECRET-42") && e.text().contains("argument 1") => Ok(()),
        o => Err(format!("secret: {o:?}")),
    });
    // A declared field is never missing (a name that is not a field is refused above).
    out.push(match analyse("SELECT * FROM CUSTOMERS WHERE CUST-ID IS MISSING", schema) {
        Ok(_) => Ok(()),
        Err(e) => Err(e.text()),
    });
    out
}

/// The scale of `PIC S9(6)V99` survives; a `.cidx` and an FD text of the same file give one schema.
fn schemas() -> Vec<Result<(), String>> {
    let mut out = Vec::new();
    let s = customers_schema();
    let bal = s.fields.iter().find(|f| f.name == "CUST-BALANCE").unwrap();
    out.push(match &bal.class {
        cobolt_runtime::partiql::schema::Class::Number { decimals: 2, signed: true } => Ok(()),
        c => Err(format!("balance class {c:?}")),
    });
    let city = s.fields.iter().find(|f| f.name == "CITY").unwrap();
    out.push(if city.path() == "BILLING.CITY" && city.occ[0].offset == 56 { Ok(()) } else { Err(format!("{:?}", city)) });
    let o = orders_schema();
    let q = o.fields.iter().find(|f| f.name == "ITEM-QTY").unwrap();
    out.push(if q.occ.len() == 5 && q.is_table() { Ok(()) } else { Err("ITEM-QTY".into()) });
    out.push(if o.primary.parts.len() == 2 && o.primary.parts.iter().all(|p| p.field.is_some()) { Ok(()) } else { Err("split key".into()) });
    out.push(if o.alternates[0].name.as_deref() == Some("BY-DATE") && o.alternates[0].duplicates { Ok(()) } else { Err("alternate".into()) });

    // The same file through a `.cidx`.
    let cidx = r#"<?xml version="1.0" encoding="UTF-8"?><IndexedFile name="CUSTOMERS" finalized="true" version="1.0"><assign-path>data/customers.idx</assign-path><access-mode>dynamic</access-mode><record-format fixed-length="76"/><storage mode="disk" compression="false" persistence="false"/><keys><primary duplicates="false" ordering="ascending"><part field="CUST-ID" offset="0" length="8" encoding="bytes"/></primary><alternate name="CUST-NAME" duplicates="true" ordering="ascending"><part field="CUST-NAME" offset="8" length="40" encoding="bytes"/></alternate></keys><fields><Field level="1" name="CUST-REC" usage="display"><Field level="5" name="CUST-ID" pic="9(8)" usage="display" offset="0" length="8"></Field><Field level="5" name="CUST-NAME" pic="X(40)" usage="display" offset="8" length="40"></Field><Field level="5" name="CUST-BALANCE" pic="S9(6)V99" usage="display" offset="48" length="8"></Field><Field level="5" name="BILLING" usage="display"><Field level="10" name="CITY" pic="X(20)" usage="display" offset="56" length="20"></Field></Field></Field></fields></IndexedFile>"#;
    out.push(match cobolt_indexed::load_indexed_from_str(cidx) {
        Err(e) => Err(format!("cidx: {e:?}")),
        Ok(def) => match Schema::from_definition("customers", &def) {
            Err(e) => Err(e),
            Ok(from_cidx) => {
                let from_fd = Schema::from_fd_text(
                    "CUSTOMERS",
                    &cobolt_runtime::indexed_ide::definition_text_of(&def),
                    KeyInput { name: None, parts: vec![("CUST-ID".into(), 0, 8)], duplicates: false },
                    vec![KeyInput { name: Some("CUST-NAME".into()), parts: vec![("CUST-NAME".into(), 8, 40)], duplicates: true }],
                )
                .unwrap();
                if from_cidx == from_fd && from_cidx.table == "CUSTOMERS" && from_cidx == s {
                    Ok(())
                } else {
                    Err(format!("schemas differ:\n{from_cidx:#?}\nvs\n{s:#?}"))
                }
            }
        },
    });
    out
}

/// AC1's four statements are keyed, range, scan, keyed.
fn classes(schema: &Schema) -> Vec<Result<(), String>> {
    let want = [
        ("SELECT CUST-ID, CUST-NAME FROM CUSTOMERS WHERE CUST-ID = ?", ReadClass::Keyed),
        ("SELECT * FROM CUSTOMERS WHERE CUST-ID BETWEEN ? AND ?", ReadClass::Range),
        ("SELECT * FROM CUSTOMERS WHERE CUST-BALANCE > 1000.50", ReadClass::Scan),
        ("SELECT * FROM CUSTOMERS.\"CUST-NAME\" WHERE CUST-NAME = 'Ana'", ReadClass::Keyed),
    ];
    want.iter()
        .map(|(sql, class)| match analyse(sql, schema) {
            Ok(a) => {
                let RStmt::Select(sel) = a.stmt else { return Err("not a select".into()) };
                let got = classify(&sel, schema);
                if got == *class {
                    Ok(())
                } else {
                    Err(format!("{sql}: {got:?}"))
                }
            }
            Err(e) => Err(e.text()),
        })
        .collect()
}

/// Reads every record in key order and keeps what `keep` accepts.
fn brute(store: &mut dyn IndexedStore, schema: &Schema, keep: &dyn Fn(i64, &str, i64) -> bool) -> Vec<i64> {
    use cobolt_runtime::files::decode_field;
    store.set_key_of_reference(0);
    assert_eq!(store.start(StartOp::Ge, &[0u8; 8]), "00");
    let f = |n: &str| &schema.fields.iter().find(|f| f.name == n).unwrap().occ[0];
    let mut out = Vec::new();
    while let (Some(rec), "00" | "02") = store.read_seq(ReadDir::Next) {
        let Value::Num(id) = decode_field(f("CUST-ID"), &rec) else { panic!() };
        let Value::Str(name) = decode_field(f("CUST-NAME"), &rec) else { panic!() };
        let Value::Num(bal) = decode_field(f("CUST-BALANCE"), &rec) else { panic!() };
        if keep(id.mantissa as i64, &name, bal.mantissa as i64) {
            out.push(id.mantissa as i64);
        }
    }
    out
}

/// Bounds generated on unsigned, decimal, signed and alphanumeric parts; each
/// statement's rows equal a brute-force filter's. Returns (cases, scans, ranges, keyed).
fn plan_property(schema: &Schema) -> (Vec<Result<(), String>>, [usize; 3]) {
    let dir = Scratch::new("lang");
    let mut store = open_store(Engine::Memory, &dir, "c", CUSTOMERS_LEN, customers_keys(), OpenMode::Io);
    let names = ["Ana", "Bruno", "Carla", "Ana Maria", "Zé", "bia", "", "Anabela"];
    let mut rng = Lcg(7);
    let mut seen = std::collections::BTreeSet::new();
    while seen.len() < 400 {
        let id = (rng.next() % 3000) as i64;
        if seen.insert(id) {
            let name = names[(rng.next() % names.len() as u64) as usize];
            let bal = (rng.next() % 2_000_000) as i64 - 1_000_000;
            assert_eq!(store.write(&customer(schema, id, name, bal, "Recife")), "00");
        }
    }
    let ops = ["=", "<", "<=", ">", ">=", "<>"];
    let mut out = Vec::new();
    let mut classes = [0usize; 3];
    let mut run = |sql: String, keep: &dyn Fn(i64, &str, i64) -> bool, out: &mut Vec<Result<(), String>>, store: &mut Box<dyn IndexedStore>| {
        let a = analyse(&sql, schema).unwrap();
        let RStmt::Select(_) = &a.stmt else { panic!() };
        let params = bind(&[&a], &[]).unwrap();
        let o = execute(store.as_mut(), schema, &a, &params);
        if let Some(e) = &o.error {
            out.push(Err(format!("{sql}: {}", e.text())));
            return;
        }
        match o.class {
            Some(ReadClass::Keyed) => classes[0] += 1,
            Some(ReadClass::Range) => classes[1] += 1,
            _ => classes[2] += 1,
        }
        let got: Vec<i64> = o.items.iter().map(|i| match &i.values[0] { Value::Num(n) => n.mantissa as i64, _ => -1 }).collect();
        let mut want = brute(store.as_mut(), schema, keep);
        // Through an alternate key the order is the key's (duplicates in join
        // order); the brute force reads by the RECORD KEY, so compare as sets.
        let mut got = got;
        if sql.contains("\"CUST-NAME\"") {
            got.sort();
            want.sort();
        }
        out.push(if got == want { Ok(()) } else { Err(format!("{sql}: {} rows vs {}", got.len(), want.len())) });
    };
    let cmp = |op: &str, a: i128, b: i128| match op {
        "=" => a == b,
        "<" => a < b,
        "<=" => a <= b,
        ">" => a > b,
        ">=" => a >= b,
        _ => a != b,
    };
    for _ in 0..120 {
        let op = ops[(rng.next() % 6) as usize];
        // Unsigned part, whole and fractional literals; negative and over-wide ones too.
        let lit = (rng.next() % 3200) as i64 - 100;
        let id = move |i: i64, _: &str, _: i64| cmp(op, i as i128, lit as i128);
        run(format!("SELECT CUST-ID FROM CUSTOMERS WHERE CUST-ID {op} {lit}"), &id, &mut out, &mut store);
        let (m, s) = ((rng.next() % 32000) as i128 - 1000, 1u32 + (rng.next() % 2) as u32);
        let frac = format!("{}.{:0w$}", m / 10i128.pow(s), (m % 10i128.pow(s)).abs(), w = s as usize);
        let neg = m < 0 && m / 10i128.pow(s) == 0;
        let frac = if neg { format!("-{frac}") } else { frac };
        let id2 = move |i: i64, _: &str, _: i64| cmp(op, i as i128 * 10i128.pow(s), m);
        run(format!("SELECT CUST-ID FROM CUSTOMERS WHERE CUST-ID {op} {frac}"), &id2, &mut out, &mut store);
        // Signed decimal part: always a scan.
        let b = (rng.next() % 2_000_000) as i128 - 1_000_000;
        let bl = format!("{}{}.{:02}", if b < 0 { "-" } else { "" }, b.abs() / 100, b.abs() % 100);
        let bal = move |_: i64, _: &str, v: i64| cmp(op, v as i128, b);
        run(format!("SELECT CUST-ID FROM CUSTOMERS WHERE CUST-BALANCE {op} {bl}"), &bal, &mut out, &mut store);
        // Between, with the bounds either way round.
        let (lo, hi) = ((rng.next() % 3000) as i64, (rng.next() % 3000) as i64);
        let btw = move |i: i64, _: &str, _: i64| i >= lo && i <= hi;
        run(format!("SELECT CUST-ID FROM CUSTOMERS WHERE CUST-ID BETWEEN {lo} AND {hi}"), &btw, &mut out, &mut store);
        // IN, with a duplicate and an absent value.
        let (x, y) = ((rng.next() % 3000) as i64, (rng.next() % 3000) as i64);
        let inn = move |i: i64, _: &str, _: i64| i == x || i == y || i == x;
        run(format!("SELECT CUST-ID FROM CUSTOMERS WHERE CUST-ID IN [{x}, {y}, {x}]"), &inn, &mut out, &mut store);
        // An alphanumeric part: prefix and comparison on the name.
        let n = names[(rng.next() % names.len() as u64) as usize];
        let nm = n.to_string();
        let pre = move |_: i64, s: &str, _: i64| s.starts_with(&nm);
        run(format!("SELECT CUST-ID FROM CUSTOMERS.\"CUST-NAME\" WHERE begins_with(CUST-NAME, '{n}')"), &pre, &mut out, &mut store);
    }
    (out, classes)
}

#[test]
fn partiql_language() {
    let mut t = Tally::default();
    let customers = customers_schema();
    let orders = orders_schema();

    // The grammar-derived table.
    let started = Instant::now();
    let mut cases: Vec<Result<(), String>> = Vec::new();
    for (sql, want) in grammar_table() {
        cases.push(match (where_sexp(sql), want) {
            (Ok(got), Ok(w)) if got == w => Ok(()),
            (Ok(got), Ok(w)) => Err(format!("{sql}: {got} vs {w}")),
            (Err(e), Err(note)) if e.contains(note) => Ok(()),
            (got, want) => Err(format!("{sql}: {got:?} vs {want:?}")),
        });
    }
    t.section("grammar-derived cases", cases);

    // Every §5.1 form parses; a corpus parsed in a loop gives the rate.
    let corpus = [
        "SELECT CUST-ID, CUST-NAME FROM CUSTOMERS WHERE CUST-ID BETWEEN ? AND ? AND NOT begins_with(CUST-NAME, 'A') ORDER BY CUST-ID DESC",
        "INSERT INTO CUSTOMERS VALUE {'CUST-ID': ?, 'CUST-NAME': 'Ana', 'CUST-BALANCE': 12.50}",
        "UPDATE CUSTOMERS SET CUST-BALANCE = CUST-BALANCE - ? REMOVE CITY WHERE CUST-ID = ? AND CUST-BALANCE >= ? RETURNING MODIFIED NEW *",
        "DELETE FROM CUSTOMERS WHERE CUST-ID = ? RETURNING ALL OLD *",
        "EXISTS(SELECT * FROM CUSTOMERS WHERE CUST-ID = ? AND CUST-BALANCE >= ?); UPDATE CUSTOMERS SET CUST-BALANCE = CUST-BALANCE - ? WHERE CUST-ID = ?",
    ];
    let loops = 4000usize;
    let t0 = Instant::now();
    let mut parsed = 0usize;
    for _ in 0..loops {
        for s in corpus {
            parsed += parse_script(s).map(|v| v.len()).unwrap_or(0);
        }
    }
    let parse_secs = t0.elapsed().as_secs_f64();
    let forms_ok: Vec<Result<(), String>> = corpus.iter().map(|s| parse_script(s).map(|_| ()).map_err(|e| e.text())).collect();
    t.section("section 5.1 forms", forms_ok);

    t.section("AC4 refusals with no file", ac4(&customers, &orders));
    t.section("schema (scale, groups, OCCURS, keys, .cidx = FD text)", schemas());
    t.section("AC1 read classes", classes(&customers));
    let (prop, c) = plan_property(&customers);
    t.section("plan vs brute-force filter", prop);
    let total_ms = started.elapsed().as_millis();

    println!("\n=== test_partiql_language ===");
    println!("forms: SELECT INSERT UPDATE DELETE EXISTS, WHERE (=, <>, !=, <, <=, >, >=, BETWEEN, IN [..], IN (..), AND, OR, NOT, IS [NOT] MISSING, begins_with, contains, size, attribute_type), paths, ?, arithmetic");
    println!("PICTUREs: 9(8) unsigned key, X(40) key, S9(6)V99 signed decimal, 9(7)V99, 9(3) OCCURS 5, split key 9(8)+9(6); engine: STORAGE IS MEMORY (400 records)");
    for (name, ok, bad) in &t.rows {
        println!("  {name:<58} pass {ok:>4}  fail {bad:>3}");
    }
    println!("plan property: {} statements — {} keyed, {} range, {} scan, each equal to a brute-force filter", c[0] + c[1] + c[2], c[0], c[1], c[2]);
    println!("parse rate: {parsed} statements in {parse_secs:.3}s = {:.0} statements/s", parsed as f64 / parse_secs);
    println!("total {total_ms} ms; failures: {}", t.failures.len());
    for f in &t.failures {
        println!("  FAIL {f}");
    }
    assert!(t.failures.is_empty(), "{} failures", t.failures.len());
}
