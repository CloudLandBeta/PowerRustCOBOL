// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the Backend needs from its process to run as a service (spec 113 R8 to R10): its secrets
//! from an environment file, a stop signal that is not lost while it starts, and the readiness
//! protocol of systemd. Nothing here changes how the Backend runs from a terminal.

use std::path::Path;

/// One `NAME=value` line of an environment file.
fn parse_env_line(line: &str) -> Option<Result<(String, String), ()>> {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return None;
    }
    let Some((name, value)) = t.split_once('=') else { return Some(Err(())) };
    let name = name.trim();
    let valid = !name.is_empty() && !name.starts_with(|c: char| c.is_ascii_digit()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid {
        return Some(Err(()));
    }
    let value = value.trim();
    // A value in matching quotes loses them; nothing else is interpreted (no escapes, no expansion, no `#` comment: a password may hold one).
    let unquoted = match value.as_bytes() {
        [q @ (b'"' | b'\''), .., last] if q == last && value.len() >= 2 => &value[1..value.len() - 1],
        _ => value,
    };
    Some(Ok((name.to_owned(), unquoted.to_owned())))
}

/// The variables an environment file names, in order. The answer's error names the file and the line,
/// never a value. On Unix a file that other users can read is refused: it holds the Backend's secrets.
pub fn read_env_file(path: &Path) -> Result<Vec<(String, String)>, String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(path).map_err(|e| format!("the environment file {} cannot be read: {e}", path.display()))?;
        let mode = meta.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(format!("the environment file {} can be read by other users (mode {mode:04o}); it holds secrets: run chmod 600 on it", path.display()));
        }
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("the environment file {} cannot be read: {e}", path.display()))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        match parse_env_line(line) {
            None => {}
            Some(Ok(pair)) => out.push(pair),
            Some(Err(())) => return Err(format!("the environment file {} line {} is not NAME=value", path.display(), i + 1)),
        }
    }
    Ok(out)
}

/// Set the variables of an environment file that the process does not already have, so a value given
/// by the operator's own environment wins. Returns how many it set. Call it before the configuration is read.
pub fn apply_env_file(path: &Path) -> Result<usize, String> {
    let mut set = 0;
    for (name, value) in read_env_file(path)? {
        if std::env::var_os(&name).is_none() {
            std::env::set_var(&name, value);
            set += 1;
        }
    }
    Ok(set)
}

/// The signals that stop the Backend, registered **at once** so that one sent while the Backend is
/// still starting is kept and seen when it asks: `SIGTERM` (systemd, launchd, `kill`) and `SIGINT`
/// on Unix, Ctrl-C elsewhere. (A Windows service is stopped by the Service Control Manager, not here.)
pub struct StopSignal {
    rt: tokio::runtime::Runtime,
    #[cfg(unix)]
    term: tokio::signal::unix::Signal,
    #[cfg(unix)]
    int: tokio::signal::unix::Signal,
}

impl StopSignal {
    pub fn install() -> std::io::Result<StopSignal> {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let _guard = rt.enter();
            let term = signal(SignalKind::terminate())?;
            let int = signal(SignalKind::interrupt())?;
            Ok(StopSignal { rt, term, int })
        }
        #[cfg(not(unix))]
        {
            Ok(StopSignal { rt })
        }
    }

    /// Block until the Backend is asked to stop.
    #[cfg(unix)]
    pub fn wait(mut self) {
        let (term, int) = (&mut self.term, &mut self.int);
        self.rt.block_on(async {
            tokio::select! {
                _ = term.recv() => {}
                _ = int.recv() => {}
            }
        });
    }

    /// Block until the Backend is asked to stop.
    #[cfg(not(unix))]
    pub fn wait(self) {
        self.rt.block_on(async {
            let _ = tokio::signal::ctrl_c().await;
        });
    }
}

/// Tell systemd the Backend is ready (`READY=1`): it listens. A no-op when `NOTIFY_SOCKET` is not set.
pub fn notify_ready() -> bool {
    notify("READY=1")
}

/// Tell systemd the Backend is stopping (`STOPPING=1`). A no-op when `NOTIFY_SOCKET` is not set.
pub fn notify_stopping() -> bool {
    notify("STOPPING=1")
}

/// Tell systemd what the Backend is doing, for `systemctl status` (`STATUS=`).
pub fn notify_status(text: &str) -> bool {
    notify(&format!("STATUS={}", text.replace('\n', " ")))
}

/// One datagram to the socket systemd gave (`NOTIFY_SOCKET`): a path, or `@name` for an abstract
/// address on Linux. The protocol is that one datagram; no library.
#[cfg(unix)]
fn notify(message: &str) -> bool {
    use std::os::unix::net::UnixDatagram;
    let Some(target) = std::env::var_os("NOTIFY_SOCKET").filter(|s| !s.is_empty()) else { return false };
    let Ok(sock) = UnixDatagram::unbound() else { return false };
    let target = target.to_string_lossy().into_owned();
    #[cfg(target_os = "linux")]
    if let Some(name) = target.strip_prefix('@') {
        use std::os::linux::net::SocketAddrExt;
        let Ok(addr) = std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes()) else { return false };
        return sock.send_to_addr(message.as_bytes(), &addr).is_ok();
    }
    sock.send_to(message.as_bytes(), &target).is_ok()
}

#[cfg(not(unix))]
fn notify(_message: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("prc-service-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[cfg(unix)]
    fn private(path: &Path, text: &str) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(path, text).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    #[cfg(not(unix))]
    fn private(path: &Path, text: &str) {
        std::fs::write(path, text).unwrap();
    }

    /// The environment file reader, the stop signal and the readiness datagram, with one result block.
    #[test]
    fn service_plumbing_reports_what_it_checked() {
        let started = Instant::now();
        let mut cases = 0;
        let mut ok = |what: &str, cond: bool| {
            assert!(cond, "{what}");
            cases += 1;
        };
        let dir = temp("env");

        // 1. The environment file.
        let f = dir.join("a.env");
        private(&f, "\u{feff}# secrets\n\nDB_PASSWORD=p#ss=word\nQUOTED=\"two words\"\nSINGLE='x'\r\nEMPTY=\n  SPACED = padded  \n");
        let got = read_env_file(&f).unwrap();
        let get = |n: &str| got.iter().find(|(k, _)| k == n).map(|(_, v)| v.as_str());
        ok("a value keeps `#` and `=`", get("DB_PASSWORD") == Some("p#ss=word"));
        ok("double quotes are removed", get("QUOTED") == Some("two words"));
        ok("single quotes are removed", get("SINGLE") == Some("x"));
        ok("an empty value is empty", get("EMPTY") == Some(""));
        ok("spaces around the name and value are trimmed", get("SPACED") == Some("padded"));
        ok("comments and blank lines are skipped", got.len() == 5);

        let bad = dir.join("bad.env");
        private(&bad, "FINE=1\nthis is not a pair\n");
        let e = read_env_file(&bad).unwrap_err();
        ok("a bad line is named by its number", e.contains("line 2"));
        ok("the error never holds a value", !e.contains("FINE") && !e.contains("this is not a pair"));
        private(&bad, "1BAD=x\n");
        ok("a name may not start with a digit", read_env_file(&bad).is_err());
        ok("a missing file is an error that names it", read_env_file(&dir.join("absent.env")).unwrap_err().contains("absent.env"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let open = dir.join("open.env");
            std::fs::write(&open, "X=1\n").unwrap();
            std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o644)).unwrap();
            let e = read_env_file(&open).unwrap_err();
            ok("a file other users can read is refused", e.contains("0644") && e.contains("chmod 600"));
            ok("the refusal does not print the content", !e.contains("X=1"));
        }

        let kept = format!("PRC_TEST_KEPT_{}", std::process::id());
        let fresh = format!("PRC_TEST_FRESH_{}", std::process::id());
        std::env::set_var(&kept, "from the environment");
        private(&dir.join("b.env"), &format!("{kept}=from the file\n{fresh}=from the file\n"));
        let set = apply_env_file(&dir.join("b.env")).unwrap();
        ok("a variable already set is kept", std::env::var(&kept).unwrap() == "from the environment");
        ok("a variable not set is set", std::env::var(&fresh).unwrap() == "from the file");
        ok("only the new one is counted", set == 1);

        // 2. The readiness datagram.
        #[cfg(unix)]
        {
            use std::os::unix::net::UnixDatagram;
            let sock_path = dir.join("notify.sock");
            let server = UnixDatagram::bind(&sock_path).unwrap();
            server.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            ok("without NOTIFY_SOCKET nothing is sent", {
                std::env::remove_var("NOTIFY_SOCKET");
                !notify_ready()
            });
            std::env::set_var("NOTIFY_SOCKET", &sock_path);
            let received = || {
                let mut buf = [0u8; 128];
                let n = server.recv(&mut buf).unwrap();
                buf[..n].to_vec()
            };
            ok("READY=1 is sent", notify_ready() && received() == b"READY=1");
            ok("STOPPING=1 is sent", notify_stopping() && received() == b"STOPPING=1");
            ok("STATUS= is one line", notify_status("a\nb") && received() == b"STATUS=a b");
            std::env::remove_var("NOTIFY_SOCKET");
        }

        // 3. The stop signal, registered before it is asked for: one sent early is kept.
        #[cfg(unix)]
        {
            let stop = StopSignal::install().unwrap();
            let sent = Instant::now();
            let pid = std::process::id().to_string();
            ok("kill -TERM is sent", std::process::Command::new("kill").args(["-TERM", &pid]).status().unwrap().success());
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                stop.wait();
                let _ = tx.send(());
            });
            ok("SIGTERM sent before wait() is seen", rx.recv_timeout(Duration::from_secs(5)).is_ok());
            let seen_ms = sent.elapsed().as_millis();
            println!("stop signal seen {seen_ms} ms after SIGTERM");
            ok("and seen within a second", seen_ms < 1000);
        }

        let _ = std::fs::remove_dir_all(&dir);
        println!("cobolt-server service: {cases} checks, 0 failed, {} ms", started.elapsed().as_millis());
    }
}
