// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 087 T23 — `rcrun run` gives a program in a project that project's SQL
//! connections (AC9, runtime part; R33, R38, R39).

use std::process::Command;

const PROGRAM: &str = "IDENTIFICATION DIVISION.
PROGRAM-ID. COUNTER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 SQLSTATE PIC X(5).
01 WS-N PIC 9(3).
PROCEDURE DIVISION.
MAIN-PARA.
    EXEC SQL CONNECT TO 'SALES' END-EXEC
    DISPLAY \"CONNECT \" SQLSTATE
    EXEC SQL CREATE TABLE IF NOT EXISTS ORDERS (ID INTEGER) END-EXEC
    EXEC SQL INSERT INTO ORDERS VALUES (7) END-EXEC
    EXEC SQL SELECT COUNT(*) INTO :WS-N FROM ORDERS END-EXEC
    DISPLAY \"ROWS \" WS-N
    STOP RUN.
";

const MANIFEST: &str = "[project]
name = \"Shop\"
version = \"1.0.0\"
main = \"src/counter.cbl\"

[[sql-connections]]
name = \"SALES\"
path = \"data/sales.db\"
create-if-missing = true
";

fn rcrun_run(program: &std::path::Path, env: &[(&str, &str)]) -> Vec<String> {
    let out = Command::new(env!("CARGO_BIN_EXE_rcrun"))
        .args(["run", program.to_str().unwrap(), "--source-format", "free"])
        .env_remove("SHOP_SQL_SALES_URL")
        .envs(env.iter().copied())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "rcrun run failed:\n{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    stdout.lines().map(|l| l.trim_end().to_string()).filter(|l| !l.is_empty()).collect()
}

/// A scratch folder, removed when the test ends.
struct Dir(std::path::PathBuf);
impl Dir {
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn rcrun_run_connects_to_the_projects_sql_connection_by_name() {
    let root = std::env::temp_dir().join(format!("esql-rcrun-run-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let dir = Dir(root);
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::create_dir_all(dir.path().join("data")).unwrap();
    std::fs::write(dir.path().join("shop.project.toml"), MANIFEST).unwrap();
    let program = dir.path().join("src/counter.cbl");
    std::fs::write(&program, PROGRAM).unwrap();

    // Twice: the first run's row was committed at its STOP RUN (R38).
    assert_eq!(rcrun_run(&program, &[]), ["CONNECT 00000", "ROWS 001"]);
    assert_eq!(rcrun_run(&program, &[]), ["CONNECT 00000", "ROWS 002"]);
    assert!(dir.path().join("data/sales.db").is_file(), "the path is relative to the project folder");

    // `SHOP_SQL_SALES_URL` points the same name at another database (R39).
    let other = dir.path().join("other.db");
    let url = format!("sqlite:{}", other.display());
    assert_eq!(rcrun_run(&program, &[("SHOP_SQL_SALES_URL", &url)]), ["CONNECT 00000", "ROWS 001"]);
    assert!(other.is_file());
    println!(
        "rcrun run: CONNECT TO 'SALES' reached data/sales.db (rows 1, then 2 — committed at STOP RUN); \
         SHOP_SQL_SALES_URL redirected it to other.db (rows 1)"
    );
}
