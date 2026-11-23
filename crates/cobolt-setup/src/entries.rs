// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! The other entry points the `.msi` names as custom actions (task T12): the three small ones around the
//! supervisor's `PrcRun`.
//!
//! * `PrcTicks`, immediate, before the work: makes the progress bar's room.
//! * `PrcRollback`, a rollback action scheduled before the work: removes what the setup made if the install is
//!   cancelled.
//! * `PrcReadResult`, immediate, after the work: turns the result file into the finish page's text.
//!
//! None of them may fail the install by accident. The only answer to the installer that is not success is
//! `ERROR_INSTALL_USEREXIT`, and only because the developer pressed Cancel.

use crate::facts::system::set_property;
use crate::result;
use crate::rollback;
use crate::supervisor::{guarded, Session, IDCANCEL};
use crate::ticks;
use std::path::Path;
use windows_sys::Win32::Foundation::{ERROR_INSTALL_USEREXIT, ERROR_SUCCESS};
use windows_sys::Win32::System::ApplicationInstallationAndServicing::MSIHANDLE;

/// A property that holds a path, or `None` when it is not set.
fn path_property(session: &Session, name: &str) -> Option<String> {
    session.property(name).filter(|v| !v.trim().is_empty())
}

/// The entry point the `.msi` names (`DllEntry="PrcTicks"`, an immediate action scheduled only when the setup
/// prepares): add the preparation's share to the progress bar's total, give the two steps their allotments
/// (`PRC_TICKS_RUST`, `PRC_TICKS_COMPILE`, which the `CustomActionData` of each step carries) and remove the result
/// and cancel files a run that was interrupted may have left.
#[no_mangle]
pub extern "system" fn PrcTicks(handle: MSIHANDLE) -> u32 {
    guarded(handle, |session| {
        for name in ["PRC_RESULT_FILE", "PRC_CANCEL_FILE"] {
            if let Some(file) = path_property(session, name) {
                let _ = std::fs::remove_file(file);
            }
        }
        set_property(session, "PRC_TICKS_RUST", &ticks::RUST.to_string());
        set_property(session, "PRC_TICKS_COMPILE", &ticks::COMPILE.to_string());
        let answer = session.add_total(ticks::TOTAL);
        session.info(&format!(
            "PRC: {} ticks added to the progress bar (Rust {}, compile {}); the installer answered {answer}",
            ticks::TOTAL,
            ticks::RUST,
            ticks::COMPILE
        ));
        if answer == IDCANCEL {
            ERROR_INSTALL_USEREXIT
        } else {
            ERROR_SUCCESS
        }
    })
}

/// The entry point the `.msi` names (`DllEntry="PrcRollback"`, a rollback action; its `CustomActionData` is the
/// build folder): remove the compiled-crate cache if the setup created it (R14). A cache the developer already had
/// stays.
#[no_mangle]
pub extern "system" fn PrcRollback(handle: MSIHANDLE) -> u32 {
    guarded(handle, |session| {
        let Some(folder) = path_property(session, "CustomActionData") else {
            session.info("PRC: rollback: no build folder was given; nothing was touched");
            return ERROR_SUCCESS;
        };
        let removal = rollback::remove_if_created_by_setup(Path::new(folder.trim()));
        session.info(&format!("PRC: rollback: {}", removal.describe()));
        ERROR_SUCCESS
    })
}

/// The entry point the `.msi` names (`DllEntry="PrcReadResult"`, an immediate action after the work and before the
/// finish page): set `PRC_RESULT` from the result file. The page says nothing when the developer did not ask for
/// the preparation.
#[no_mangle]
pub extern "system" fn PrcReadResult(handle: MSIHANDLE) -> u32 {
    guarded(handle, |session| {
        let asked = session.property("PRC_PREPARE").as_deref() == Some("1");
        let file = path_property(session, "PRC_RESULT_FILE");
        let linker = path_property(session, "PRC_LINKER").is_some();
        let text = result::finish_text_from_file(asked, file.as_deref().map(Path::new), linker);
        // The whole text goes to the log as well: the finish page has room for a few lines, the log for all of it.
        session.info(&format!(
            "PRC: finish page: asked {asked}, result file {}, linker {linker}: {} characters: {}",
            file.as_deref().unwrap_or("(none)"),
            text.len(),
            text.replace("\r\n", " | ")
        ));
        set_property(session, "PRC_RESULT", &text);
        // What the steps wrote is read; the file is not kept.
        if let Some(file) = file {
            let _ = std::fs::remove_file(file);
        }
        ERROR_SUCCESS
    })
}
