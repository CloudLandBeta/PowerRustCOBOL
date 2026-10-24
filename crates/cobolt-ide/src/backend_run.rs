// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The project's backend started from the IDE (WASM-003 R20, AC9; T10).
//!
//! The IDE runs `rcrun backend run <project>` as a child process — the
//! operator's decision of 2026-10-23 — so what runs is exactly the standalone
//! development server, and the IDE links no server, socket, TLS or token code.
//! Its standard output and standard error (the address, the development token,
//! one JSON log line per record) are read on two threads and handed to the
//! Output panel; Stop, a second Start and the IDE's exit kill the child.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};

/// What `rcrun backend run` prints first, then the address.
const LISTENING: &str = "backend listening on ";

/// Something the running backend did since the last [`BackendRun::poll`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendEvent {
    /// A line the child printed, as printed.
    Line(String),
    /// It listens on this address (`ws://127.0.0.1:PORT/ws`).
    Listening(String),
    /// It ended by itself, with this exit code (`None`: killed by a signal).
    Exited(Option<i32>),
}

pub struct BackendRun {
    /// The project folder the backend was started for.
    pub project_dir: PathBuf,
    child: Child,
    lines: Receiver<String>,
    url: Option<String>,
    exit_status: Option<ExitStatus>,
}

/// The `rcrun` to run: beside the IDE's executable, else the one on `PATH`.
pub fn rcrun() -> PathBuf {
    crate::project_model::find_cobolt_binary().unwrap_or_else(|| PathBuf::from(format!("rcrun{}", std::env::consts::EXE_SUFFIX)))
}

fn forward(from: impl Read + Send + 'static, to: Sender<String>) {
    std::thread::spawn(move || {
        for line in BufReader::new(from).lines() {
            let Ok(line) = line else { break };
            if to.send(line).is_err() {
                break;
            }
        }
    });
}

impl BackendRun {
    /// Start `<rcrun> backend run <project_dir>`.
    pub fn spawn(rcrun: &Path, project_dir: &Path) -> std::io::Result<Self> {
        let mut child = Command::new(rcrun)
            .arg("backend")
            .arg("run")
            .arg(project_dir)
            .current_dir(project_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let (tx, lines) = channel();
        if let Some(out) = child.stdout.take() {
            forward(out, tx.clone());
        }
        if let Some(err) = child.stderr.take() {
            forward(err, tx);
        }
        Ok(BackendRun { project_dir: project_dir.to_path_buf(), child, lines, url: None, exit_status: None })
    }

    /// The address, once the child has printed it.
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    pub fn is_running(&self) -> bool {
        self.exit_status.is_none()
    }

    /// The lines printed since the last call, the address when it appears,
    /// and the exit when the child ended by itself (after its last lines).
    pub fn poll(&mut self) -> Vec<BackendEvent> {
        let mut events = Vec::new();
        while let Ok(line) = self.lines.try_recv() {
            if self.url.is_none() {
                if let Some(url) = line.strip_prefix(LISTENING) {
                    self.url = Some(url.trim().to_string());
                    events.push(BackendEvent::Listening(url.trim().to_string()));
                    continue;
                }
            }
            events.push(BackendEvent::Line(line));
        }
        if self.exit_status.is_none() {
            if let Ok(Some(status)) = self.child.try_wait() {
                self.exit_status = Some(status);
                // What it printed while dying: the reader threads end at EOF.
                while let Ok(line) = self.lines.recv_timeout(std::time::Duration::from_millis(200)) {
                    events.push(BackendEvent::Line(line));
                }
                events.push(BackendEvent::Exited(status.code()));
            }
        }
        events
    }

    /// Kill the child and wait for it (the Stop action).
    pub fn stop(&mut self) {
        if self.exit_status.is_none() {
            let _ = self.child.kill();
            if let Ok(status) = self.child.wait() {
                self.exit_status = Some(status);
            }
        }
    }
}

impl Drop for BackendRun {
    fn drop(&mut self) {
        // Never leave a backend listening after the IDE exits.
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{Duration, Instant};

    /// `target/<profile>/rcrun`, beside the `deps/` folder this test runs from.
    /// `cargo test -p cobolt-ide --bin cobolt-ide` does not build it.
    fn built_rcrun() -> PathBuf {
        let exe = std::env::current_exe().unwrap();
        let profile = exe.parent().and_then(|d| d.parent()).unwrap();
        let rcrun = profile.join(format!("rcrun{}", std::env::consts::EXE_SUFFIX));
        assert!(rcrun.exists(), "{} is missing: run `cargo build -p cobolt-cli --bin rcrun` first", rcrun.display());
        rcrun
    }

    fn get_health(port: u16) -> std::io::Result<String> {
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port))?;
        s.set_read_timeout(Some(Duration::from_secs(5)))?;
        write!(s, "GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
        let mut raw = String::new();
        let _ = s.read_to_string(&mut raw);
        Ok(raw)
    }

    /// AC9, the IDE half: Start runs the project's backend, its address and log
    /// reach the IDE, `/health` answers; Stop ends the process and frees the port.
    #[test]
    fn the_ide_starts_the_backend_reads_its_log_and_stops_it() {
        let rcrun = built_rcrun();
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("Shop");
        use cobolt_project_tools::create::{create_project_with, CreateRequest, BACKEND_CONFIG};
        create_project_with(&dir, &CreateRequest::new("Shop").template("web")).expect("a Web project");
        // The scaffold listens on a fixed loopback port; a free one keeps the test
        // from colliding with a backend the developer is running.
        std::fs::write(dir.join(BACKEND_CONFIG), "[server]\nlisten = \"127.0.0.1:0\"\n").unwrap();

        let started = Instant::now();
        let mut run = BackendRun::spawn(&rcrun, &dir).expect("rcrun starts");
        let mut log = Vec::new();
        while run.url().is_none() && started.elapsed() < Duration::from_secs(60) {
            log.extend(run.poll());
            std::thread::sleep(Duration::from_millis(20));
        }
        let url = run.url().map(str::to_owned).unwrap_or_else(|| panic!("no address after 60 s: {log:?}"));
        println!("listening line after {} ms: {url}", started.elapsed().as_millis());
        assert!(url.starts_with("ws://127.0.0.1:"), "{url}");
        let port: u16 = url["ws://127.0.0.1:".len()..].split('/').next().unwrap().parse().unwrap();

        let health = get_health(port).unwrap();
        assert!(health.starts_with("HTTP/1.1 200"), "{health}");
        println!("GET /health: {}", health.lines().next().unwrap_or_default());

        // The server's own log record of its start arrives as a line, live.
        let deadline = Instant::now() + Duration::from_secs(10);
        while !log.iter().any(|e| matches!(e, BackendEvent::Line(l) if l.contains("\"event\":\"start\""))) && Instant::now() < deadline {
            log.extend(run.poll());
            std::thread::sleep(Duration::from_millis(20));
        }
        let lines: Vec<&String> = log.iter().filter_map(|e| if let BackendEvent::Line(l) = e { Some(l) } else { None }).collect();
        println!("captured {} line(s):", lines.len());
        for l in &lines {
            println!("  {}", if l.split('.').count() == 3 && !l.contains(' ') { "<development token>" } else { l });
        }
        assert!(lines.iter().any(|l| l.contains("\"event\":\"start\"")), "the start record: {lines:?}");

        let stopping = Instant::now();
        run.stop();
        assert!(!run.is_running());
        println!("stopped in {} ms", stopping.elapsed().as_millis());
        let pid = run.child.id();
        drop(run);
        // The port is free again: nothing answers, and it can be bound.
        let mut freed = false;
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
                freed = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(freed, "port {port} still taken");
        assert!(get_health(port).is_err(), "nothing answers on {port} after Stop");
        println!("process {pid} gone, port {port} free after {} ms", stopping.elapsed().as_millis());
    }

    /// A start that cannot run (no backend in the folder) ends by itself, and
    /// says why on the way out.
    #[test]
    fn a_folder_without_a_backend_exits_and_says_why() {
        let rcrun = built_rcrun();
        let root = tempfile::tempdir().unwrap();
        let mut run = BackendRun::spawn(&rcrun, root.path()).expect("rcrun starts");
        let mut log = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(30);
        while run.is_running() && Instant::now() < deadline {
            log.extend(run.poll());
            std::thread::sleep(Duration::from_millis(20));
        }
        println!("{log:?}");
        assert!(log.contains(&BackendEvent::Exited(Some(1))), "{log:?}");
        assert!(log.iter().any(|e| matches!(e, BackendEvent::Line(l) if l.contains("exports.toml"))), "{log:?}");
    }
}
