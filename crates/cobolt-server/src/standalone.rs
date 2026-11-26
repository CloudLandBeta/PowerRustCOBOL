// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The `main` of the built backend binary (WASM-003 R2, R20; spec 113 for the service options).
//!
//! ```text
//! <backend> [--config backend.toml] [--env-file FILE] [--log-file FILE]
//!           [--service [--service-name NAME]]   (Windows only)
//!           [--check] [--describe] [--version]
//! ```
//!
//! `--check` validates the configuration and prints what it would do;
//! `--describe` prints the interface description; with neither, the backend
//! starts and runs until it is asked to stop (Ctrl-C, `SIGTERM`, or the Windows
//! Service Control Manager). A built binary is a **release** run: any `unsafe_*`
//! setting stops it (WASM-013 R11).
//!
//! As a service it needs three things a terminal run does not: its secrets from a
//! file only its account can read (`--env-file`), a log file that rotates where the
//! system keeps no journal (`--log-file`), and, on Windows, to be the service
//! itself (`--service`). None of them changes a run without them.

use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::Arc;

use crate::bundle::Bundle;
use crate::config::{RunMode, ServerConfig};
use crate::log::{LogSink, StdoutSink};
use crate::logfile::{LogWriter, RotatingLog};
use crate::serve::Options;

const USAGE: &str = "usage: backend [--config backend.toml] [--env-file FILE] [--log-file FILE] [--service [--service-name NAME]] [--check] [--describe] [--version]";

/// The arguments of a run.
#[derive(Debug, Default, Clone)]
pub struct Args {
    pub config_path: PathBuf,
    pub env_file: Option<PathBuf>,
    pub log_file: Option<PathBuf>,
    pub service: bool,
    pub service_name: String,
    pub check: bool,
    pub describe: bool,
}

enum Parsed {
    Run(Args),
    /// Print this and leave with this code.
    Leave(i32),
}

fn parse(args: &[String]) -> Parsed {
    let mut a = Args { config_path: PathBuf::from("backend.toml"), service_name: "backend".into(), ..Args::default() };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = |what: &str| -> Result<String, ()> {
            match it.next() {
                Some(v) => Ok(v.clone()),
                None => {
                    eprintln!("{what} needs a value");
                    Err(())
                }
            }
        };
        match arg.as_str() {
            "--config" => match value("--config") {
                Ok(v) => a.config_path = PathBuf::from(v),
                Err(()) => return Parsed::Leave(2),
            },
            "--env-file" => match value("--env-file") {
                Ok(v) => a.env_file = Some(PathBuf::from(v)),
                Err(()) => return Parsed::Leave(2),
            },
            "--log-file" => match value("--log-file") {
                Ok(v) => a.log_file = Some(PathBuf::from(v)),
                Err(()) => return Parsed::Leave(2),
            },
            "--service-name" => match value("--service-name") {
                Ok(v) => a.service_name = v,
                Err(()) => return Parsed::Leave(2),
            },
            "--service" => a.service = true,
            "--check" => a.check = true,
            "--describe" => a.describe = true,
            "--version" => {
                println!("backend (cobolt-server {})", env!("CARGO_PKG_VERSION"));
                return Parsed::Leave(0);
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return Parsed::Leave(0);
            }
            other => {
                eprintln!("unknown argument '{other}'");
                return Parsed::Leave(2);
            }
        }
    }
    Parsed::Run(a)
}

/// Where the backend says things: standard error, and the log file when there is one. A service has
/// no terminal, so with a log file the lines go there and to standard error only when someone is looking.
pub(crate) struct Out {
    file: Option<Arc<RotatingLog>>,
}

impl Out {
    pub(crate) fn line(&self, text: &str) {
        if self.file.is_none() || std::io::stderr().is_terminal() {
            eprintln!("{text}");
        }
        if let Some(f) = &self.file {
            f.line(&format!("{} {text}", crate::logfile::stamp()));
        }
    }
}

/// What differs between a run from a terminal, under systemd or launchd, and as a Windows service.
pub(crate) struct Hooks<'a> {
    /// Called once the backend listens.
    pub ready: Box<dyn Fn() + 'a>,
    /// Blocks until the backend is asked to stop.
    pub wait: Box<dyn FnOnce() + 'a>,
    /// Called as the backend starts to stop.
    pub stopping: Box<dyn Fn() + 'a>,
}

/// Run the packed backend with the process arguments. Returns the exit code.
pub fn main(bundle: &[u8], args: Vec<String>) -> i32 {
    let a = match parse(&args) {
        Parsed::Run(a) => a,
        Parsed::Leave(code) => return code,
    };
    // A log file, when asked for, is where everything goes: the records, the runtime's warnings and the lines below.
    let file = match &a.log_file {
        Some(p) => match RotatingLog::open_default(p) {
            Ok(l) => Some(Arc::new(l)),
            Err(e) => {
                eprintln!("the log file {} cannot be opened: {e}", p.display());
                return 2;
            }
        },
        None => None,
    };
    // Warnings and errors of the runtime go to standard error (or the log file); `COBOLT_LOG` widens it.
    let filter = tracing_subscriber::EnvFilter::from_env("COBOLT_LOG").add_directive(tracing::Level::WARN.into());
    let _ = match &file {
        Some(f) => tracing_subscriber::fmt().with_env_filter(filter).with_target(false).with_ansi(false).with_writer(LogWriter(f.clone())).try_init(),
        None => tracing_subscriber::fmt().with_env_filter(filter).with_target(false).with_writer(std::io::stderr).try_init(),
    };
    let out = Out { file: file.clone() };
    let sink: Arc<dyn LogSink> = match &file {
        Some(f) => f.clone(),
        None => Arc::new(StdoutSink),
    };
    // The secrets, before the configuration that names them is read; one the environment already has wins.
    if let Some(p) = &a.env_file {
        if let Err(e) = crate::service::apply_env_file(p) {
            out.line(&format!("error: {e}"));
            return 2;
        }
    }
    if a.service {
        #[cfg(windows)]
        {
            return crate::service_windows::run(&a.service_name, |hooks| run(&a, bundle, &out, sink.clone(), hooks));
        }
        #[cfg(not(windows))]
        {
            out.line("--service is for Windows: under systemd or launchd the backend is started as an ordinary program");
            return 2;
        }
    }
    // A signal that arrives while the backend is starting is kept, not lost.
    let stop = crate::service::StopSignal::install().ok();
    let hooks = Hooks {
        ready: Box::new(|| {
            crate::service::notify_ready();
        }),
        wait: Box::new(move || match stop {
            Some(s) => s.wait(),
            None => wait_for_interrupt(),
        }),
        stopping: Box::new(|| {
            crate::service::notify_stopping();
        }),
    };
    run(&a, bundle, &out, sink, hooks)
}

/// Load, check, start, wait, stop. The exit code: 0 clean, 1 could not load or start, 2 refused configuration.
pub(crate) fn run(a: &Args, bundle: &[u8], out: &Out, sink: Arc<dyn LogSink>, hooks: Hooks<'_>) -> i32 {
    let backend = match Bundle::from_bytes(bundle).and_then(|b| b.into_backend().map_err(|e| e.join("\n"))) {
        Ok(b) => b,
        Err(e) => {
            out.line(&e);
            return 1;
        }
    };
    if a.describe {
        println!("{}", backend.interface.to_json());
        return 0;
    }
    let cfg = match ServerConfig::load(&a.config_path) {
        Ok(c) => c,
        Err(e) => {
            out.line(&e.to_string());
            out.line("The backend needs its configuration: address, TLS identity, the issuer of the tokens it accepts. See backend.toml in the project's backend folder.");
            return 2;
        }
    };
    let base = a.config_path.parent().map(|p| p.to_path_buf()).filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| PathBuf::from("."));
    let resolved = match cfg.resolve(RunMode::Release, &base) {
        Ok(r) => r,
        Err(errs) => {
            for e in errs {
                out.line(&format!("error: {e}"));
            }
            return 2;
        }
    };
    if a.check {
        println!("configuration ok: contract {} on {} ({} exported program(s))", backend.interface.contract, resolved.listen, backend.interface.programs.len());
        return 0;
    }
    let handle = match crate::serve::start(resolved, backend, Options { sink: Some(sink), fetcher: None, identity_http: None }) {
        Ok(h) => h,
        Err(e) => {
            out.line(&format!("error: {e}"));
            return 1;
        }
    };
    out.line(&format!("backend listening on {}", handle.addr));
    (hooks.ready)();
    (hooks.wait)();
    (hooks.stopping)();
    out.line("stopping");
    handle.stop();
    0
}

/// Block until the process is interrupted (Ctrl-C).
pub fn wait_for_interrupt() {
    match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt.block_on(async {
            let _ = tokio::signal::ctrl_c().await;
        }),
        Err(_) => loop {
            std::thread::park();
        },
    }
}
