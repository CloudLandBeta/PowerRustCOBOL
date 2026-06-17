// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Property test for the Timer control.
//!     cargo test --manifest-path tests/controls/Cargo.toml --test timer -- --nocapture

use cobolt_forms::ControlType;
use control_tests::assert_control;

#[test]
fn timer_all_properties() {
    assert_control(ControlType::Timer, "Timer");
}
