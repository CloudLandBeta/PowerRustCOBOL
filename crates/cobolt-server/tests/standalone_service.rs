// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 113 (R8 to R11, A3): the options a built backend takes to run as a service, proved on a real
//! process: `--env-file` supplies a secret, `--log-file` receives the lines, `SIGTERM` stops it cleanly
//! (exit 0, `READY=1` only after it listens, `STOPPING=1` as it stops), a refused configuration is
//! reported with its reason and never says ready, and a run without the options is as before.

#![cfg(unix)]

use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use cobolt_server::bundle::Bundle;
use cobolt_server::testkit::backend_dir;

const EXPORTS: &str = "[contract]\nversion = \"1.0.0\"\n\n[[export]]\nprogram = \"PING\"\npublic = true\n";
const PING: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. PING.\nDATA DIVISION.\nLINKAGE SECTION.\n01 LK-PONG.\n    05 REPLY PIC X(4).\nPROCEDURE DIVISION RETURNING LK-PONG.\nMAIN.\n    MOVE \"pong\" TO REPLY\n    GOBACK.\n";

fn probe() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let p = exe.parent().unwrap().parent().unwrap().join("examples").join("standalone_probe");
    if !p.exists() {
        // `cargo test --test …` does not build examples; the stand-in is built here, once.
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let st = Command::new(cargo).args(["build", "--example", "standalone_probe"]).current_dir(env!("CARGO_MANIFEST_DIR")).status().unwrap();
        assert!(st.success(), "building the standalone_probe example failed");
    }
    assert!(p.exists(), "the example {} is not built", p.display());
    p
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

struct Rig {
    dir: PathBuf,
    bundle: PathBuf,
}

fn rig(tag: &str) -> Rig {
    let dir = backend_dir(tag, EXPORTS, &[("ping.cbl", PING)]);
    let bytes = Bundle::from_dir(&dir).expect("the stand-in backend loads").to_bytes().expect("it packs");
    let bundle = dir.join("backend.bin");
    std::fs::write(&bundle, bytes).unwrap();
    Rig { dir, bundle }
}

fn config(rig: &Rig, port: u16, secret_var: &str, origins: &str) -> PathBuf {
    let path = rig.dir.join(format!("backend-{port}.toml"));
    std::fs::write(
        &path,
        format!("[server]\nlisten = \"127.0.0.1:{port}\"\nallowed_origins = {origins}\n[auth]\nissuer = [\"https://idp.example.test/pool\"]\naudiences = [\"prc-api\"]\n[[configuration]]\nname = \"CRM\"\nAuthToken = \"env:{secret_var}\"\n"),
    )
    .unwrap();
    path
}

fn env_file(rig: &Rig, name: &str, secret_var: &str, mode: u32) -> PathBuf {
    let path = rig.dir.join(name);
    std::fs::write(&path, format!("# the Backend's secrets\n{secret_var}=s3cr3t-value\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
    path
}

struct Ran {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run(rig: &Rig, args: &[&str], secret_var: &str) -> Ran {
    let mut cmd = Command::new(probe());
    cmd.args(args).env("PRC_PROBE_BUNDLE", &rig.bundle).env_remove(secret_var).env_remove("NOTIFY_SOCKET").current_dir(&rig.dir);
    let out = cmd.output().unwrap();
    Ran { code: out.status.code(), stdout: String::from_utf8_lossy(&out.stdout).into_owned(), stderr: String::from_utf8_lossy(&out.stderr).into_owned() }
}

fn spawn(rig: &Rig, args: &[&str], notify: &Path, secret_var: &str) -> Child {
    Command::new(probe())
        .args(args)
        .env("PRC_PROBE_BUNDLE", &rig.bundle)
        .env("NOTIFY_SOCKET", notify)
        .env_remove(secret_var)
        .current_dir(&rig.dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

fn datagram(sock: &UnixDatagram, within: Duration) -> Option<String> {
    sock.set_read_timeout(Some(within)).unwrap();
    let mut buf = [0u8; 256];
    sock.recv(&mut buf).ok().map(|n| String::from_utf8_lossy(&buf[..n]).into_owned())
}

fn wait_exit(child: &mut Child, within: Duration) -> Option<std::process::ExitStatus> {
    let end = Instant::now() + within;
    while Instant::now() < end {
        if let Some(s) = child.try_wait().unwrap() {
            return Some(s);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

#[test]
fn a_backend_runs_as_a_service_and_reports_what_it_did() {
    let started = Instant::now();
    let mut checks = 0;
    let mut ok = |what: &str, cond: bool| {
        assert!(cond, "{what}");
        checks += 1;
    };
    let rig = rig("svc");
    let var = format!("PRC_PROBE_SECRET_{}", std::process::id());
    let port = free_port();
    let cfg = config(&rig, port, &var, "[\"https://app.example.test\"]");
    let cfg_s = cfg.to_str().unwrap();
    let good_env = env_file(&rig, "secrets.env", &var, 0o600);
    let open_env = env_file(&rig, "open.env", &var, 0o644);

    // Without the secret the configuration is refused, naming the variable; with --env-file it is accepted.
    let r = run(&rig, &["--check", "--config", cfg_s], &var);
    ok("a missing secret refuses the configuration (exit 2)", r.code == Some(2));
    ok("and names the variable, not a value", r.stderr.contains(&var) && !r.stderr.contains("s3cr3t"));
    let r = run(&rig, &["--check", "--config", cfg_s, "--env-file", good_env.to_str().unwrap()], &var);
    ok("--env-file supplies it (exit 0)", r.code == Some(0));
    ok("--check prints the line it always printed", r.stdout.starts_with("configuration ok: contract ") && r.stdout.contains(&format!("127.0.0.1:{port}")) && r.stdout.contains("(1 exported program(s))"));
    let r = run(&rig, &["--check", "--config", cfg_s, "--env-file", open_env.to_str().unwrap()], &var);
    ok("an environment file other users can read is refused (exit 2)", r.code == Some(2) && r.stderr.contains("0644"));
    ok("and its content is not printed", !r.stderr.contains("s3cr3t"));
    let r = run(&rig, &["--check", "--config", cfg_s, "--env-file", rig.dir.join("nope.env").to_str().unwrap()], &var);
    ok("a missing environment file is refused (exit 2)", r.code == Some(2) && r.stderr.contains("nope.env"));
    let r = run(&rig, &["--version"], &var);
    ok("--version is as before", r.code == Some(0) && r.stdout.starts_with("backend (cobolt-server "));
    let r = run(&rig, &["--describe"], &var);
    ok("--describe is as before", r.code == Some(0) && r.stdout.contains("\"PING\""));
    let r = run(&rig, &["--service", "--config", cfg_s], &var);
    ok("--service is refused where there is no Service Control Manager (exit 2)", r.code == Some(2));

    // A run as a service: ready only after it listens, a log file, SIGTERM stops it cleanly.
    let notify_path = rig.dir.join("notify.sock");
    let notify = UnixDatagram::bind(&notify_path).unwrap();
    let log_path = rig.dir.join("logs").join("backend.log");
    let t_start = Instant::now();
    let mut child = spawn(&rig, &["--config", cfg_s, "--env-file", good_env.to_str().unwrap(), "--log-file", log_path.to_str().unwrap()], &notify_path, &var);
    let first = datagram(&notify, Duration::from_secs(60));
    let ready_ms = t_start.elapsed().as_millis();
    ok("READY=1 is sent", first.as_deref() == Some("READY=1"));
    ok("and the port answers when it is", std::net::TcpStream::connect_timeout(&format!("127.0.0.1:{port}").parse().unwrap(), Duration::from_secs(2)).is_ok());
    let log = std::fs::read_to_string(&log_path).unwrap_or_default();
    ok("the log file was made in a folder that did not exist", log_path.exists());
    ok("it holds the listening line with a UTC stamp", log.lines().any(|l| l.contains("backend listening on 127.0.0.1:") && l.len() > 21 && l.as_bytes()[4] == b'-' && l.as_bytes()[10] == b'T'));
    let t_stop = Instant::now();
    ok("kill -TERM is sent", Command::new("kill").args(["-TERM", &child.id().to_string()]).status().unwrap().success());
    let status = wait_exit(&mut child, Duration::from_secs(30));
    let stop_ms = t_stop.elapsed().as_millis();
    ok("it exits within 30 s of SIGTERM", status.is_some());
    ok("with exit code 0", status.and_then(|s| s.code()) == Some(0));
    ok("STOPPING=1 was sent", datagram(&notify, Duration::from_secs(2)).as_deref() == Some("STOPPING=1"));
    ok("the log records it stopped", std::fs::read_to_string(&log_path).unwrap_or_default().contains("stopping"));
    let mut child_err = String::new();
    child.stderr.take().unwrap().read_to_string(&mut child_err).unwrap();
    ok("a service with a log file leaves standard error empty of the lifecycle lines", !child_err.contains("backend listening"));

    // A refused configuration: never ready, the reason in the log file, exit non-zero.
    let bad = config(&rig, free_port(), &var, "[]");
    let bad_log = rig.dir.join("logs").join("refused.log");
    let mut child = spawn(&rig, &["--config", bad.to_str().unwrap(), "--env-file", good_env.to_str().unwrap(), "--log-file", bad_log.to_str().unwrap()], &notify_path, &var);
    let status = wait_exit(&mut child, Duration::from_secs(30));
    ok("a refused configuration ends the process", status.is_some());
    ok("with a non-zero code (2)", status.and_then(|s| s.code()) == Some(2));
    ok("READY=1 was never sent", datagram(&notify, Duration::from_millis(300)).is_none());
    let refused = std::fs::read_to_string(&bad_log).unwrap_or_default();
    ok("the reason is in the log file", refused.contains("allowed_origins") || refused.contains("origin"));
    // The same refusal with no log file: the reason is on standard error, where the journal reads it.
    let r = run(&rig, &["--config", bad.to_str().unwrap(), "--env-file", good_env.to_str().unwrap()], &var);
    ok("without a log file the reason is on standard error (exit 2)", r.code == Some(2) && (r.stderr.contains("allowed_origins") || r.stderr.contains("origin")));

    let _ = std::fs::remove_dir_all(&rig.dir);
    println!(
        "standalone service: {checks} checks, 0 failed; ready {ready_ms} ms after start (debug build), stopped {stop_ms} ms after SIGTERM; total {} ms",
        started.elapsed().as_millis()
    );
}
