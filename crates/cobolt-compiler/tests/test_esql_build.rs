// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 087 T25 (AC10, AC14 binary part) — a built application takes its SQL
//! connections from the `sql-connections.toml` Build wrote beside it, the
//! environment overrides it, a password in it is refused, and the end of the
//! run commits or rolls back. The `COBOL::"OPEN-DB"` built-in reaches the
//! same SQL connection by name (R40, AC17 binary part).
//!
//! Gated: it builds a real binary. Run it with
//! `cargo test -p cobolt-compiler --test test_esql_build -- --ignored`.

use std::path::{Path, PathBuf};
use std::time::Instant;

use cobolt_compiler::{build_project, BuildOptions, SQL_CONNECTIONS_FILE};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap().to_path_buf()
}

const MANIFEST: &str = r#"[project]
name = "Crm"
version = "1.0.0"
main = "src/deployed.cbl"
debug_compilation = true

[[sql-connections]]
name = "SALES"
path = "data/sales.db"
default = true
"#;

/// Connects to SALES, adds a row, shows the count; with `SALES_MODE=FAIL` it
/// then ends in a runtime error.
const PROGRAM: &str = "IDENTIFICATION DIVISION.
PROGRAM-ID. DEPLOYED.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 SQLSTATE PIC X(5).
01 SQLMSG PIC X(300).
01 WS-N PIC 9(3).
01 WS-MODE PIC X(10).
01 WS-H PIC 9(9).
01 WS-ERR PIC X(200).
PROCEDURE DIVISION.
MAIN-PARA.
    DISPLAY \"SALES_MODE\" UPON ENVIRONMENT-NAME
    ACCEPT WS-MODE FROM ENVIRONMENT-VALUE
    IF WS-MODE = \"OPENDB\"
        COBOL::\"OPEN-DB\" ( \"sql-connection:SALES\" WS-H WS-ERR )
        IF WS-H > 0
            DISPLAY \"OPEN-DB OK\"
        ELSE
            DISPLAY \"OPEN-DB \" WS-ERR
        END-IF
        STOP RUN
    END-IF
    EXEC SQL CONNECT TO 'SALES' END-EXEC
    DISPLAY \"CONNECT \" SQLSTATE
    IF SQLSTATE NOT = \"00000\"
        DISPLAY \"MSG \" SQLMSG
        STOP RUN
    END-IF
    EXEC SQL CREATE TABLE IF NOT EXISTS ORDERS (ID INTEGER) END-EXEC
    EXEC SQL INSERT INTO ORDERS VALUES (7) END-EXEC
    EXEC SQL SELECT COUNT(*) INTO :WS-N FROM ORDERS END-EXEC
    DISPLAY \"ROWS \" WS-N
    IF WS-MODE = \"FAIL\"
        MOVE \"X\" TO super::Title
    END-IF
    STOP RUN.
";

/// Run the binary; its stdout lines and whether it succeeded.
fn run(bin: &Path, env: &[(&str, &str)]) -> (Vec<String>, bool) {
    let out = std::process::Command::new(bin)
        .env_remove("CRM_SQL_SALES_URL")
        .env_remove("SALES_MODE")
        .envs(env.iter().copied())
        .output()
        .unwrap_or_else(|e| panic!("cannot run {}: {e}", bin.display()));
    let lines = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim_end().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    (lines, out.status.success())
}

#[test]
#[ignore = "builds a real binary; run with --ignored"]
fn a_built_binary_reads_its_sql_connections_file() {
    let t_all = Instant::now();
    let project = std::env::temp_dir().join(format!("prc087-build-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(project.join("crm.project.toml"), MANIFEST).unwrap();
    std::fs::write(project.join("src/deployed.cbl"), PROGRAM).unwrap();
    // Build resolves the project folder, symlinks and all (`/var` is
    // `/private/var` on macOS), and so must the expected paths.
    let project = project.canonicalize().unwrap();
    let opts = BuildOptions { verbose: false, workspace_root: Some(workspace_root()), ..Default::default() };

    // ── Build: the starting file, beside the binary and in the hand-over ────
    let t = Instant::now();
    let built = build_project(&project.join("crm.project.toml"), &opts).unwrap_or_else(|e| panic!("build failed: {e}"));
    let build_s = t.elapsed().as_secs_f32();
    let bin = built.binary_path.clone();
    let beside = bin.parent().unwrap().join(SQL_CONNECTIONS_FILE);
    let starting = std::fs::read_to_string(&beside).expect("Build writes the file beside the binary");
    assert!(project.join("dist").join(SQL_CONNECTIONS_FILE).is_file(), "and in the destination folder");
    let db = project.join("data/sales.db");
    assert!(starting.contains(&format!("path = \"{}\"", db.display())), "an absolute path:\n{starting}");
    assert!(!starting.contains("user =") && !starting.contains("password ="), "no credentials:\n{starting}");
    assert!(starting.contains("CRM_SQL_SALES_PASSWORD"), "the header names the password variable");

    // The project's file does not exist and is never created behind its back.
    let (out, ok) = run(&bin, &[]);
    assert!(ok, "{out:?}");
    assert_eq!(out[0], "CONNECT 08001", "{out:?}");
    assert!(!db.exists());

    // ── The operator points SALES elsewhere; a rebuild keeps the edit ───────
    let other = project.join("elsewhere.db");
    let edited = format!(
        "[[connection]]\nname = \"SALES\"\npath = \"{}\"\ncreate-if-missing = true\n",
        other.display()
    );
    std::fs::write(&beside, &edited).unwrap();
    let t = Instant::now();
    build_project(&project.join("crm.project.toml"), &opts).unwrap_or_else(|e| panic!("rebuild failed: {e}"));
    let rebuild_s = t.elapsed().as_secs_f32();
    assert_eq!(std::fs::read_to_string(&beside).unwrap(), edited, "a rebuild leaves the edited file alone");

    assert_eq!(run(&bin, &[]).0, ["CONNECT 00000", "ROWS 001"]);
    assert_eq!(run(&bin, &[]).0, ["CONNECT 00000", "ROWS 002"], "the first run's row was committed at STOP RUN");
    // R40, binary part: the built-in reaches the same SQL connection by name.
    assert_eq!(run(&bin, &[("SALES_MODE", "OPENDB")]).0, ["OPEN-DB OK"]);

    // ── A runtime error rolls the open work back ─────────────────────────────
    let (out, ok) = run(&bin, &[("SALES_MODE", "FAIL")]);
    assert!(!ok, "the run ends in an error: {out:?}");
    assert_eq!(out, ["CONNECT 00000", "ROWS 003"]);
    assert_eq!(run(&bin, &[]).0, ["CONNECT 00000", "ROWS 003"], "the failed run's row was rolled back");

    // ── The environment overrides the file ───────────────────────────────────
    let third = project.join("third.db");
    let url = format!("sqlite:{}", third.display());
    assert_eq!(run(&bin, &[("CRM_SQL_SALES_URL", &url)]).0, ["CONNECT 00000", "ROWS 001"]);
    assert!(third.is_file());

    // ── A password in the file is refused, naming the file and the variable ─
    std::fs::write(&beside, format!("{edited}password = \"secret\"\n")).unwrap();
    let (out, _) = run(&bin, &[]);
    assert_eq!(out[0], "CONNECT 28000", "{out:?}");
    assert!(out[1].contains(SQL_CONNECTIONS_FILE) && out[1].contains("CRM_SQL_SALES_PASSWORD"), "{out:?}");
    assert!(!out.join("\n").contains("secret"), "the password is never shown");

    let _ = std::fs::remove_dir_all(&project);
    println!(
        "087 AC10: build {build_s:.1} s, rebuild {rebuild_s:.1} s; starting file absolute and credential-free; \
         missing file 08001; edited file kept; rows 1, 2 committed; OPEN-DB 'sql-connection:SALES' resolved; FAIL run's row rolled back (3 stays 3); \
         CRM_SQL_SALES_URL → third.db; password in file → 28000; total {:.1} s",
        t_all.elapsed().as_secs_f32()
    );
}
