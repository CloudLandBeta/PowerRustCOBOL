// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! The Windows half of the Build Tools step: `prc-buildtools.exe` (task T19).
//!
//! The bundle's Burn engine runs this program elevated, before the `.msi`. It asks the machine whether the C++ build
//! tools are there (the same `vswhere` question the `.msi`'s facts ask), downloads Microsoft's installer when they
//! are not, checks that Windows trusts its signature and that Microsoft signed it, runs it quietly and waits, and
//! hands what happened to [`decide`], whose exit code is the program's. Everything it does is written to
//! `%TEMP%\prc-buildtools.log` as well as to standard output, so a developer who is told "the C++ build tools were
//! not installed" can read why.

use super::{
    accept, arguments, decide, needed, Decision, Run, Signature, DOWNLOAD_URL, LIMIT_SECONDS,
};
use crate::facts::{linker, system::Here};
use crate::supervisor::wide;
use std::ffi::c_void;
use std::io::Write as _;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{
    HANDLE, HWND, TRUST_E_NOSIGNATURE, TRUST_E_PROVIDER_UNKNOWN, TRUST_E_SUBJECT_FORM_UNKNOWN,
};
use windows_sys::Win32::Security::Cryptography::{
    CertGetNameStringW, CERT_NAME_SIMPLE_DISPLAY_TYPE,
};
use windows_sys::Win32::Security::WinTrust::{
    WTHelperGetProvCertFromChain, WTHelperGetProvSignerFromChain, WTHelperProvDataFromStateData,
    WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_FILE_INFO,
    WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY, WTD_UI_NONE,
};
use windows_sys::Win32::System::Com::Urlmon::URLDownloadToFileW;
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

/// Where the program writes what it does: the installing user's temporary folder.
fn log_path() -> PathBuf {
    std::env::temp_dir().join("prc-buildtools.log")
}

/// One line to standard output and to the log file (best effort: a log that cannot be written never stops the step).
fn say(line: &str) {
    println!("PRC buildtools: {line}");
    let _ = std::io::stdout().flush();
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
    {
        let _ = writeln!(file, "PRC buildtools: {line}");
    }
}

/// Download Microsoft's installer to `target`. Uses the system's own downloader, which follows the machine's proxy
/// settings, as a developer's browser would.
fn download(target: &Path) -> Result<(), String> {
    let _ = std::fs::remove_file(target);
    let url = wide(DOWNLOAD_URL);
    let file = wide(&target.to_string_lossy());
    // SAFETY: both strings are NUL-terminated and outlive the call; no caller object and no callback are passed.
    let hr = unsafe {
        URLDownloadToFileW(
            std::ptr::null_mut(),
            url.as_ptr(),
            file.as_ptr(),
            0,
            std::ptr::null_mut(),
        )
    };
    if hr == 0 && target.is_file() {
        Ok(())
    } else {
        Err(format!(
            "The download of Microsoft's installer for the C++ build tools failed (code 0x{:08X}).",
            hr as u32
        ))
    }
}

/// What Windows says about the file's signature, and who signed it.
fn verify(file: &Path) -> Signature {
    let path = wide(&file.to_string_lossy());
    let mut info = WINTRUST_FILE_INFO {
        cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: path.as_ptr(),
        hFile: std::ptr::null_mut() as HANDLE,
        pgKnownSubject: std::ptr::null_mut(),
    };
    // SAFETY: an all-zero WINTRUST_DATA is the documented starting point; the fields set below are the ones the
    // file check reads.
    let mut data: WINTRUST_DATA = unsafe { std::mem::zeroed() };
    data.cbStruct = std::mem::size_of::<WINTRUST_DATA>() as u32;
    data.dwUIChoice = WTD_UI_NONE;
    // Revocation lists are not asked for: a locked-down network that cannot reach them would refuse a genuine file.
    // The file came over HTTPS from Microsoft's own address, and the chain and the publisher are still checked.
    data.fdwRevocationChecks = WTD_REVOKE_NONE;
    data.dwUnionChoice = WTD_CHOICE_FILE;
    data.Anonymous.pFile = &mut info;
    data.dwStateAction = WTD_STATEACTION_VERIFY;
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    // "No window" is the documented INVALID_HANDLE_VALUE.
    let no_window = -1isize as HWND;

    // SAFETY: `data` and `info` live to the end of the function, the action GUID is valid, and the state opened here
    // is closed below before anything returns.
    let status = unsafe {
        WinVerifyTrust(
            no_window,
            &mut action,
            std::ptr::addr_of_mut!(data).cast::<c_void>(),
        )
    };
    let verdict = if status == 0 {
        match signer_name(&data) {
            Some(publisher) => Signature::Valid { publisher },
            None => Signature::Untrusted {
                reason: "the signer's name could not be read".to_owned(),
            },
        }
    } else if matches!(
        status,
        TRUST_E_NOSIGNATURE | TRUST_E_PROVIDER_UNKNOWN | TRUST_E_SUBJECT_FORM_UNKNOWN
    ) {
        Signature::Unsigned
    } else {
        Signature::Untrusted {
            reason: format!("Windows answered 0x{:08X}", status as u32),
        }
    };

    data.dwStateAction = WTD_STATEACTION_CLOSE;
    // SAFETY: closes the state the verify call opened, with the same data.
    unsafe {
        WinVerifyTrust(
            no_window,
            &mut action,
            std::ptr::addr_of_mut!(data).cast::<c_void>(),
        );
    }
    verdict
}

/// The display name of the certificate that signed the file the open state is about.
fn signer_name(data: &WINTRUST_DATA) -> Option<String> {
    // SAFETY: the state handle came from a successful `WinVerifyTrust`, and every pointer taken from the provider's
    // data is checked for null before it is read.
    unsafe {
        let provider = WTHelperProvDataFromStateData(data.hWVTStateData);
        if provider.is_null() {
            return None;
        }
        let signer = WTHelperGetProvSignerFromChain(provider, 0, 0, 0);
        if signer.is_null() {
            return None;
        }
        let cert = WTHelperGetProvCertFromChain(signer, 0);
        if cert.is_null() || (*cert).pCert.is_null() {
            return None;
        }
        let mut name = [0u16; 256];
        let len = CertGetNameStringW(
            (*cert).pCert,
            CERT_NAME_SIMPLE_DISPLAY_TYPE,
            0,
            std::ptr::null(),
            name.as_mut_ptr(),
            name.len() as u32,
        );
        // `len` counts the terminating NUL; 1 means there was no name.
        (len > 1).then(|| String::from_utf16_lossy(&name[..(len as usize - 1)]))
    }
}

/// Run Microsoft's installer and wait for it, up to [`LIMIT_SECONDS`].
fn run_installer(file: &Path) -> Run {
    let mut child = match Command::new(file)
        .args(arguments())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            return Run::NotRun(format!(
                "Microsoft's installer for the C++ build tools could not be started ({e})."
            ))
        }
    };
    let began = Instant::now();
    let mut last_note = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return Run::Exited(status.code().map_or(u32::MAX, |c| c as u32));
            }
            Ok(None) if began.elapsed() < Duration::from_secs(LIMIT_SECONDS) => {
                // A line every half minute, so the log shows the step is alive and how long it has been going.
                if last_note.elapsed() >= Duration::from_secs(30) {
                    say(&format!(
                        "Microsoft's installer is still running ({} s)",
                        began.elapsed().as_secs()
                    ));
                    last_note = Instant::now();
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Run::TimedOut;
            }
            Err(e) => {
                return Run::NotRun(format!(
                    "Microsoft's installer for the C++ build tools could not be waited for ({e})."
                ))
            }
        }
    }
}

/// The program's whole work. Returns its exit code (see [`decide`]).
pub fn main() -> u32 {
    say(&format!("start; log {}", log_path().display()));
    let mut here = Here;
    if !needed(linker(&mut here)) {
        say("the C++ build tools are installed already; nothing to do");
        return 0;
    }

    let installer = std::env::temp_dir().join("prc-vs_BuildTools.exe");
    say(&format!("downloading {DOWNLOAD_URL}"));
    let run = match download(&installer) {
        Err(sentence) => Run::NotRun(sentence),
        Ok(()) => {
            let signature = verify(&installer);
            say(&format!("signature: {signature:?}"));
            match accept(&signature) {
                Err(sentence) => Run::NotRun(sentence),
                Ok(()) => {
                    say(&format!("running it: {}", arguments().join(" ")));
                    run_installer(&installer)
                }
            }
        }
    };
    let _ = std::fs::remove_file(&installer);

    let Decision { exit, ready, text } = decide(&run, linker(&mut here));
    say(&format!("{run:?}; ready {ready}; exit {exit}: {text}"));
    exit
}
