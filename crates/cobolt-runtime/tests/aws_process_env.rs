// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 078 AC7 (R11): a started AWS server never receives an access key, a
//! secret key or a session token — even when the application's own
//! environment holds all three. Its own test binary, because it plants the
//! secrets in this process's environment.

#![cfg(feature = "aws")]

use cobolt_runtime::aws::process::{spawn, Launch, SECRET_VARS};
use serde_json::json;

#[test]
fn a_started_server_never_receives_aws_secrets() {
    let dir = std::env::temp_dir().join(format!("prc-078-env-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dump = dir.join("env.txt");
    let script = dir.join("script.json");
    std::fs::write(&script, json!({"env_dump": dump.to_string_lossy()}).to_string()).unwrap();

    let planted = [
        ("AWS_ACCESS_KEY_ID", "AKIAPLANTEDKEY000001"),
        ("AWS_SECRET_ACCESS_KEY", "PlantedSecretKeyValue0000000000000000abcd"),
        ("AWS_SESSION_TOKEN", "PlantedSessionToken0000000000000000000000000000000000000000Zz9"),
        ("AWS_SECURITY_TOKEN", "PlantedSecurityToken"),
    ];
    for (k, v) in planted {
        // This test binary runs this single test, so nothing else reads the
        // environment while it changes.
        std::env::set_var(k, v);
    }
    let launch = Launch {
        command: env!("CARGO_BIN_EXE_fake_mcp").into(),
        args: vec![script.to_string_lossy().into_owned()],
        profile: Some("dev".into()),
        region: Some("us-east-1".into()),
        env: vec![("FUNCTION_PREFIX".into(), "app-".into())],
    };
    let mut s = spawn(&launch).expect("the fake starts");
    drop(s.stdin); // EOF: the fake ends
    let _ = s.child.wait();
    let env = std::fs::read_to_string(&dump).unwrap();
    for (k, v) in planted {
        assert!(!env.contains(v), "the value of {k} reached the server");
    }
    for name in SECRET_VARS {
        assert!(!env.lines().any(|l| l.starts_with(&format!("{name}="))), "{name} reached the server");
    }
    assert!(env.lines().any(|l| l == "AWS_PROFILE=dev"), "{env}");
    assert!(env.lines().any(|l| l == "AWS_REGION=us-east-1"), "{env}");
    assert!(env.lines().any(|l| l == "FUNCTION_PREFIX=app-"), "{env}");
    let _ = std::fs::remove_dir_all(&dir);
    println!(
        "AC7: {} planted secrets, 0 reached the server; it received {} variables, AWS_PROFILE and AWS_REGION among them",
        planted.len(),
        env.lines().count()
    );
}
