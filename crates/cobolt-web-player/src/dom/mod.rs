// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The DOM projection (placeholder while the renderer is written).

use std::rc::Rc;

use crate::api::{Inner, Surface};

pub(crate) fn create(_inner: &Rc<Inner>, _root: web_sys::Element) -> Result<Box<dyn Surface>, String> {
    Err("the page renderer is not built into this player".into())
}
