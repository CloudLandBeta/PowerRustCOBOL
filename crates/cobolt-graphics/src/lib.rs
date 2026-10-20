// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Placeholder while the crate is assembled.

pub mod canvas;
pub mod color;
pub(crate) mod detmath;
pub mod display_list;
pub mod error;
pub mod handles;
#[cfg(feature = "imagebuf")]
pub mod imagebuf;
pub mod limits;
pub mod par;
pub mod pixels;
#[cfg(feature = "plot")]
pub mod plot;
pub mod resources;
#[cfg(feature = "text")]
pub mod text;
pub mod textapi;

pub use color::Rgba;
pub use error::{GraphicsError, Result};
pub use limits::Limits;
pub use pixels::PixelBuffer;
