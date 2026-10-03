// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Opening a project in PowerRustCOBOL AI from `rcrun mcp`** (spec 084
//! R20a). After a coding agent created or opened a project through the stdio
//! server, the IDE is started beside `rcrun` with `--open <manifest>` and
//! waited for; the agent is told what happened and which server to use next.
//!
//! It only ever CONNECTS (to see whether the IDE's tool server answers) — it
//! opens no listener, which `rcrun`'s own tests hold it to.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Where the IDE is and how to reach its tool server.
#[derive(Debug, Clone)]
pub struct IdeEnv {
    /// Whether the IDE may be started at all (`PRC_NO_IDE_LAUNCH` says no).
    pub launch: bool,
    /// The IDE's executable, when it sits beside `rcrun`.
    pub ide: Option<PathBuf>,
    /// The port its tool server uses (`LlmConfig::mcp_port`, default 5720).
    pub port: u16,
    /// How long to wait for a started IDE to answer.
    pub wait: Duration,
}

/// The IDE executable's names, by platform and build (installed, then dev).
const IDE_NAMES: [&str; 5] = ["PowerRustCOBOL", "PowerRustCOBOL.exe", "cobolt-ide", "cobolt-ide.exe", "powerrustcobol"];

impl IdeEnv {
    /// The machine's: the IDE beside `beside` (rcrun's folder), its configured
    /// port, a minute's wait, and `PRC_NO_IDE_LAUNCH` honoured.
    pub fn from_machine(beside: Option<&Path>) -> Self {
        let ide = beside.and_then(|dir| IDE_NAMES.iter().map(|n| dir.join(n)).find(|p| p.is_file()));
        let off = std::env::var("PRC_NO_IDE_LAUNCH")
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);
        Self { launch: !off, ide, port: machine_mcp_port(), wait: Duration::from_secs(60) }
    }
}

/// The tool server's port from the IDE's machine configuration
/// (`<data dir>/cobolt/llm_config.json`, `mcp_port`), default 5720.
pub fn machine_mcp_port() -> u16 {
    dirs::data_dir()
        .map(|d| d.join("cobolt").join("llm_config.json"))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("mcp_port").and_then(serde_json::Value::as_u64))
        .and_then(|p| u16::try_from(p).ok())
        .unwrap_or(5720)
}

fn ide_answers(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(300),
    )
    .is_ok()
}

/// Open `manifest` in PowerRustCOBOL AI and say what happened. The project
/// itself is already created/opened for the stdio server either way.
pub fn open_in_ide(manifest: &Path, env: &IdeEnv) -> String {
    if !env.launch {
        return "PowerRustCOBOL AI was not opened (PRC_NO_IDE_LAUNCH); these tools work on the project".to_owned();
    }
    if ide_answers(env.port) {
        return "PowerRustCOBOL AI is already running: open this project there with the \
                powerrustcobol-ide server's open_project, and use that server from now on"
            .to_owned();
    }
    let Some(ide) = &env.ide else {
        return "PowerRustCOBOL AI was not found beside rcrun, so it was not opened; these tools \
                work on the project without it"
            .to_owned();
    };
    let spawned = std::process::Command::new(ide)
        .arg("--open")
        .arg(manifest)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    if let Err(e) = spawned {
        return format!("PowerRustCOBOL AI could not be started ({e}); these tools work on the project without it");
    }
    let deadline = Instant::now() + env.wait;
    while Instant::now() < deadline {
        if ide_answers(env.port) {
            return "opened in PowerRustCOBOL AI: use the powerrustcobol-ide server from now on, \
                    and not this one"
                .to_owned();
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    format!(
        "PowerRustCOBOL AI was started but its tool server did not answer within {} s; \
         these tools still work on the project",
        env.wait.as_secs()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn free_port() -> u16 {
        TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
    }

    fn env(ide: Option<PathBuf>, port: u16, wait_ms: u64) -> IdeEnv {
        IdeEnv { launch: true, ide, port, wait: Duration::from_millis(wait_ms) }
    }

    /// Spec 084 AC15 / R20a, with a stand-in IDE (this test binary, which exits
    /// at once) and a port the test controls: switched off, no IDE beside
    /// rcrun, an IDE already running, one that comes up, one that never does.
    #[test]
    fn open_in_ide_says_what_happened() {
        let manifest = Path::new("/abs/Demo/Demo.project.toml");
        let stand_in = std::env::current_exe().unwrap();

        let running = TcpListener::bind("127.0.0.1:0").unwrap();
        let off = IdeEnv { launch: false, ..env(Some(stand_in.clone()), running.local_addr().unwrap().port(), 100) };
        assert!(open_in_ide(manifest, &off).contains("PRC_NO_IDE_LAUNCH"), "switched off wins over a running IDE");
        let up = env(Some(stand_in.clone()), running.local_addr().unwrap().port(), 1000);
        assert!(open_in_ide(manifest, &up).contains("already running"));
        drop(running);

        assert!(open_in_ide(manifest, &env(None, free_port(), 100)).contains("not found"));

        let port = free_port();
        let later = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(500));
            let l = TcpListener::bind(("127.0.0.1", port)).unwrap();
            let _ = l.accept();
        });
        assert!(open_in_ide(manifest, &env(Some(stand_in.clone()), port, 10_000)).contains("opened in PowerRustCOBOL AI"));
        later.join().unwrap();

        assert!(open_in_ide(manifest, &env(Some(stand_in), free_port(), 800)).contains("did not answer"));
        println!("open_in_ide: switched off / already running / not found / started and answered / started, silent — 5 outcomes");
    }
}
