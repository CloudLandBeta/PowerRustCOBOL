// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Property test for the ProgressBar widget.
//!     cargo test --manifest-path tests/widgets/Cargo.toml --test progressbar -- --nocapture

use cobolt_forms::ControlType;
use widget_tests::assert_widget;

#[test]
fn progressbar_all_properties() {
    assert_widget(ControlType::ProgressBar, "ProgressBar");
}
