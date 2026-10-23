// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Interpreter / binary parity for the backend call (WASM-005, WASM-014): the
//! program that `rcrun run` runs against a backend (`cobolt-cli`'s
//! `backend_cli`) is built into a native binary and runs the same way. The
//! binary links the backend client only because the program has a `WITH
//! WEBSOCKET LINKAGE` call, and starts it from the environment before anything
//! else runs.

use std::path::{Path, PathBuf};
use std::time::Instant;

use cobolt_compiler::{build_project, BuildOptions};
use cobolt_server::dev::{DevOptions, DevServer};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap().to_path_buf()
}

const EXPORTS: &str = "[contract]\nversion = \"1.0.0\"\n\n[[export]]\nprogram = \"PING\"\npublic = true\n\n[[export]]\nprogram = \"WHOAMI\"\n";
const PING: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. PING.\nDATA DIVISION.\nLINKAGE SECTION.\n01 LK-PONG.\n    05 REPLY PIC X(4).\nPROCEDURE DIVISION RETURNING LK-PONG.\nMAIN.\n    MOVE \"pong\" TO REPLY\n    GOBACK.\n";
const WHOAMI: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. WHOAMI.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-SUB PIC X(40).\nLINKAGE SECTION.\n01 LK-ME.\n    05 WHO PIC X(40).\nPROCEDURE DIVISION RETURNING LK-ME.\nMAIN.\n    COBOL::\"WS-CLAIM\" ( \"sub\" WS-SUB )\n    MOVE WS-SUB TO WHO\n    GOBACK.\n";

const CALLER: &str = "       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALLER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 PONG.
          05 REPLY PIC X(4).
       01 ME.
          05 WHO PIC X(40).
       01 WS-CLASS PIC X(16).
       01 WS-CODE  PIC 9(5).
       01 WS-TEXT  PIC X(80).
       PROCEDURE DIVISION.
       MAIN.
           CALL \"PING\" WITH WEBSOCKET LINKAGE RETURNING PONG
               ON EXCEPTION
                   COBOL::\"WS-LAST-ERROR\" ( WS-CLASS WS-CODE WS-TEXT )
                   DISPLAY \"ping failed \" WS-CLASS WS-CODE
               NOT ON EXCEPTION
                   DISPLAY \"ping=\" REPLY
           END-CALL
           CALL \"WHOAMI\" WITH WEBSOCKET LINKAGE RETURNING ME
               ON EXCEPTION
                   COBOL::\"WS-LAST-ERROR\" ( WS-CLASS WS-CODE WS-TEXT )
                   DISPLAY \"whoami failed \" WS-CLASS WS-CODE
               NOT ON EXCEPTION
                   DISPLAY \"who=\" WHO
           END-CALL
           STOP RUN.
";

#[test]
#[ignore = "builds a real binary; run with --ignored"]
fn a_built_application_calls_its_backend_as_rcrun_run_does() {
    // The backend, served the way `rcrun backend run` serves it.
    let backend = std::env::temp_dir().join(format!("prc014-parity-backend-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&backend);
    std::fs::create_dir_all(backend.join("src")).unwrap();
    std::fs::write(backend.join("exports.toml"), EXPORTS).unwrap();
    std::fs::write(backend.join("src").join("ping.cbl"), PING).unwrap();
    std::fs::write(backend.join("src").join("whoami.cbl"), WHOAMI).unwrap();
    let server = DevServer::start(&DevOptions { backend_dir: backend.clone(), listen: None, ..DevOptions::default() }).expect("the backend starts");
    let token = server.dev_token("tester", &["user"]);

    // The application: a console program with a remote call.
    let project = std::env::temp_dir().join(format!("prc014-parity-app-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(project.join("src")).unwrap();
    let project = project.canonicalize().unwrap();
    std::fs::write(project.join("src").join("caller.cbl"), CALLER).unwrap();
    std::fs::write(project.join("caller.project.toml"), "[project]\nname = \"Caller\"\nversion = \"1.0.0\"\nmain = \"src/caller.cbl\"\n").unwrap();
    let opts = BuildOptions { verbose: false, workspace_root: Some(workspace_root()), debug: true, ..Default::default() };
    let t = Instant::now();
    let built = build_project(&project.join("caller.project.toml"), &opts).unwrap_or_else(|e| panic!("build failed: {e}"));
    let build_s = t.elapsed().as_secs_f32();

    let run = |with_backend: bool| {
        let mut cmd = std::process::Command::new(&built.binary_path);
        cmd.current_dir(&project).env_remove("COBOLT_BACKEND_URL").env_remove("COBOLT_BACKEND_TOKEN");
        if with_backend {
            cmd.env("COBOLT_BACKEND_URL", &server.url).env("COBOLT_BACKEND_TOKEN", &token);
        }
        let out = cmd.output().expect("runs");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let with = run(true);
    assert!(with.contains("ping=pong") && with.contains("who=tester"), "the built binary reaches the backend: {with}");
    let without = run(false);
    assert!(without.contains("ping failed TRANSPORT") && without.contains("01003"), "and fails visibly when none is configured: {without}");

    server.stop();
    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(&backend);
    println!("WASM-014 parity: the compiled application calls its backend (ping=pong, who=tester) and fails with TRANSPORT 1003 without one; build {build_s:.1} s (debug)");
}
