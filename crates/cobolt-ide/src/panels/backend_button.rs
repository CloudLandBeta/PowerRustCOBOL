// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Backend button of the top bar: grey while no Backend runs, green while
//! it runs as the page expects, red when something is wrong (it serves another
//! project or another page origin, or it ended with an error). In every state
//! but grey, a click stops the Backend.

use egui::{Color32, RichText};

use crate::i18n::Tr;

/// What the button says about the Backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    /// No Backend: the button is disabled.
    Idle,
    /// Running, for this project and for the page being served.
    Ok,
    /// Running, but not for what the developer has open: a Backend of another
    /// project, or one that accepts another page origin than the Preview serves.
    Stale,
    /// It ended by itself with an error; the click dismisses the mark.
    Failed,
}

/// The answer from the facts of a frame. `run_port` is the page port the
/// Backend was told to accept; `page_port` the port the Preview serves on.
pub fn health(running: bool, failed: bool, same_project: bool, run_port: Option<u16>, preview_active: bool, page_port: Option<u16>) -> Health {
    if failed {
        return Health::Failed;
    }
    if !running {
        return Health::Idle;
    }
    let origin_matches = match (run_port, preview_active) {
        (Some(want), true) => page_port == Some(want),
        _ => true,
    };
    if same_project && origin_matches {
        Health::Ok
    } else {
        Health::Stale
    }
}

const GREEN: Color32 = Color32::from_rgb(0x2E, 0xC2, 0x7E);
const RED: Color32 = Color32::from_rgb(0xE5, 0x48, 0x4D);
const GREY: Color32 = Color32::from_rgb(0x8A, 0x8F, 0x98);

/// Draw the button at the right of the top bar; `true` when it was clicked.
pub fn show(ui: &mut egui::Ui, health: Health, tr: &Tr) -> bool {
    let (colour, tip) = match health {
        Health::Idle => (GREY, tr.backend_btn_idle),
        Health::Ok => (GREEN, tr.backend_btn_ok),
        Health::Stale => (RED, tr.backend_btn_stale),
        Health::Failed => (RED, tr.backend_btn_failed),
    };
    let label = RichText::new(format!("● {}", tr.backend_window_title)).color(colour).strong();
    ui.add_enabled(health != Health::Idle, egui::Button::new(label)).on_hover_text(tip).on_disabled_hover_text(tip).clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_button_is_grey_green_or_red_by_what_the_backend_is_doing() {
        // No Backend: disabled.
        assert_eq!(health(false, false, true, None, false, None), Health::Idle);
        // Running for this project, no page yet, or the page on the port it accepts: green.
        assert_eq!(health(true, false, true, Some(8765), false, None), Health::Ok);
        assert_eq!(health(true, false, true, Some(8765), true, Some(8765)), Health::Ok);
        assert_eq!(health(true, false, true, None, true, Some(49623)), Health::Ok, "no origin was set: any page");
        // The page is on 49623 while the Backend accepts 8765: stale, red.
        assert_eq!(health(true, false, true, Some(8765), true, Some(49623)), Health::Stale);
        // A Backend of another project: red.
        assert_eq!(health(true, false, false, None, false, None), Health::Stale);
        // It ended with an error: red until dismissed, whatever else is true.
        assert_eq!(health(false, true, true, None, false, None), Health::Failed);
    }
}
