// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The graphics state a `Save` pushes and a `Restore` pops.

use std::sync::Arc;

use kurbo::Affine;
use tiny_skia::Mask;

use super::path::PathAcc;
use crate::color::Rgba;
use crate::display_list::consts::*;
use crate::textapi::FontSpec;

/// Where a fill or stroke gets its colour.
#[derive(Clone)]
pub(crate) enum PaintSrc {
    Solid(Rgba),
    Linear { p0: (f32, f32), p1: (f32, f32), stops: Arc<Vec<(f32, Rgba)>> },
    Radial { c0: (f32, f32), r0: f32, c1: (f32, f32), r1: f32, stops: Arc<Vec<(f32, Rgba)>> },
    Conic { c: (f32, f32), angle: f32, stops: Arc<Vec<(f32, Rgba)>> },
    Pattern { pixmap: Arc<tiny_skia::Pixmap>, repeat: PatternRepeat },
    /// An image being drawn (`DrawImage`): sampled once, edges padded.
    Image(Arc<tiny_skia::Pixmap>),
}

/// The shadow settings (zero alpha means none).
#[derive(Clone, Copy)]
pub(crate) struct Shadow {
    pub color: Rgba,
    pub blur: f32,
    pub dx: f32,
    pub dy: f32,
}

/// Everything `Save` keeps.
#[derive(Clone)]
pub(crate) struct GState {
    /// User space to device space, pixel ratio included.
    pub ctm: Affine,
    pub fill: PaintSrc,
    pub stroke: PaintSrc,
    pub line_width: f32,
    pub cap: LineCap,
    pub join: LineJoin,
    pub miter_limit: f32,
    pub dash: Arc<Vec<f32>>,
    pub dash_offset: f32,
    pub alpha: f32,
    pub blend: BlendMode,
    pub shadow: Shadow,
    /// The clip, in the coordinates of the band that owns this state.
    pub clip: Option<Arc<Mask>>,
    pub font: Arc<FontSpec>,
    pub align: TextAlign,
    pub baseline: TextBaseline,
    pub direction: TextDirection,
    pub smoothing: Smoothing,
}

impl GState {
    pub fn new(base: Affine) -> Self {
        Self {
            ctm: base,
            fill: PaintSrc::Solid(Rgba::BLACK),
            stroke: PaintSrc::Solid(Rgba::BLACK),
            line_width: 1.0,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            miter_limit: 10.0,
            dash: Arc::new(Vec::new()),
            dash_offset: 0.0,
            alpha: 1.0,
            blend: BlendMode::SourceOver,
            shadow: Shadow { color: Rgba::TRANSPARENT, blur: 0.0, dx: 0.0, dy: 0.0 },
            clip: None,
            font: Arc::new(FontSpec::default()),
            align: TextAlign::Start,
            baseline: TextBaseline::Alphabetic,
            direction: TextDirection::Auto,
            smoothing: Smoothing::Low,
        }
    }
}

/// A band's whole mutable state: the current graphics state, the saved ones
/// and the path.
pub(crate) struct ExecState {
    pub cur: GState,
    pub stack: Vec<GState>,
    pub path: PathAcc,
    pub base: Affine,
}

impl ExecState {
    pub fn new(base: Affine) -> Self {
        Self { cur: GState::new(base), stack: Vec::new(), path: PathAcc::default(), base }
    }

    /// `Reset`: default state, empty stack, clip and path.
    pub fn reset(&mut self) {
        self.cur = GState::new(self.base);
        self.stack.clear();
        self.path.clear();
    }
}
