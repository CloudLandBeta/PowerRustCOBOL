// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Container/containment helpers — moved to `cobolt-forms` (spec 017) so the
//! unified render engine and the designer share one implementation. This module
//! re-exports them so existing `super::containers::*` call sites keep working.

pub use cobolt_forms::containers::{
    ancestor_opacity, clip_rect, collect_descendants, is_descendant, is_visible,
    render_order, resolve_drop_target, ActiveTabs, DropTarget,
};
