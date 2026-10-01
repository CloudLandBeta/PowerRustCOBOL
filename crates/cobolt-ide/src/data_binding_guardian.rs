// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Deterministic validation for form data-binding metadata.
//!
//! The guardian lives in `cobolt-project-tools` (spec 080), so the coding-agent
//! tools' `check` applies exactly the gate the IDE's Build/Check applies. This
//! module re-exports it, which keeps every IDE call site unchanged.

pub use cobolt_project_tools::binding_guardian::*;
