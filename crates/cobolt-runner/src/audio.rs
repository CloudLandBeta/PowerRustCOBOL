// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The desktop's sound player (DESKTOP-WASM-001 T49, ABI A36): a form's `COBOL::"AUDIO-PLAY"`,
//! `"AUDIO-PAUSE"`, `"AUDIO-RESUME"`, `"AUDIO-STOP"` and `"AUDIO-STATE"`, played by the runtime's
//! player (`cobolt_runtime::audio_runtime`) exactly as the interpreter plays them.
//!
//! Each form instance has its own door to the application's one player, as each form's interpreter
//! has: a sound one form started plays on while another form reads its progress, and the instance
//! that started it stops it when it closes. A relative file is read from the folder the program's
//! assets come from, as the interpreter reads it.

use cobolt_abi_host::host::AudioPort;
use cobolt_runtime::audio_runtime::AudioHandle;
use std::collections::HashMap;

/// The player's doors, one per form instance that has used it.
#[derive(Default)]
pub(crate) struct RunnerAudio {
    doors: HashMap<u32, AudioHandle>,
}

impl AudioPort for RunnerAudio {
    fn command(&mut self, inst: u32, verb: &str, file: &str) -> String {
        let door = self.doors.entry(inst).or_default();
        let status = match verb {
            "AUDIO-PLAY" => door.play(&cobolt_runtime::interpreter::anchor_relative_path(file.trim())),
            "AUDIO-PAUSE" => door.pause(),
            "AUDIO-RESUME" => door.resume(),
            _ => door.stop(),
        };
        status.as_word().to_owned()
    }

    fn state(&mut self, inst: u32) -> (String, u64, u64) {
        let snap = self.doors.entry(inst).or_default().state();
        (snap.state.as_word().to_owned(), snap.position_ms, snap.duration_ms)
    }

    fn release(&mut self, inst: u32) {
        self.doors.remove(&inst);
    }
}
