// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Canvas2D, Canvas3D and the five graphics objects** (WASM-018 R1): the vocabulary, in one place.
//!
//! Two visual controls - Canvas2D and Canvas3D - and five non-visual objects - ImageBuffer, Plot,
//! Scene3D, Physics2D, Physics3D. Their methods live in `cobolt-graphics` (`Registry::methods`); this
//! module is pure data, no egui and no I/O, so the player builds it for `wasm32`.
//!
//! `Width` and `Height` of a canvas are the control's own geometry (logical pixels); every other
//! property of R1 is seeded here.

use crate::model::ControlType;

/// The seven type names, as a form file spells them.
pub const NAMES: &[&str] = &["Canvas2D", "Canvas3D", "ImageBuffer", "Plot", "Scene3D", "Physics2D", "Physics3D"];

/// The events of both canvases (R1): the frame, the pointer, the wheel, the keys, the touches and the
/// size. A handler is raised only when one is bound.
pub const CANVAS_EVENTS: &[&str] = &[
    "onFrame",
    "onPointerDown",
    "onPointerMove",
    "onPointerUp",
    "onPointerEnter",
    "onPointerLeave",
    "onWheel",
    "onKeyDown",
    "onKeyUp",
    "onTouchStart",
    "onTouchMove",
    "onTouchEnd",
    "onResize",
];

/// Canvas3D's events: the canvas ones, and `onError` for a lost device or no adapter (T18).
pub const CANVAS3D_EVENTS: &[&str] = &[
    "onFrame",
    "onPointerDown",
    "onPointerMove",
    "onPointerUp",
    "onPointerEnter",
    "onPointerLeave",
    "onWheel",
    "onKeyDown",
    "onKeyUp",
    "onTouchStart",
    "onTouchMove",
    "onTouchEnd",
    "onResize",
    "onError",
];

/// The objects raise no events of their own yet.
pub const OBJECT_EVENTS: &[&str] = &[];

/// What `Control::new` seeds on a Canvas2D: every value a string, as the properties of every other
/// control that holds a number are (`PixelRatio` is a decimal).
pub const CANVAS2D_SEEDS: &[(&str, &str)] = &[
    ("PixelRatio", "1"),
    ("Smoothing", "Low"),
    ("BackgroundColor", "#00000000"),
    ("TargetFps", "0"),
    ("AutoPresent", "True"),
    ("AccessibleLabel", ""),
    ("FixedStep", "False"),
    ("PointerX", "0"),
    ("PointerY", "0"),
];

/// What `Control::new` seeds on a Canvas3D: the canvas properties and `ClearColor`, `Msaa`.
pub const CANVAS3D_SEEDS: &[(&str, &str)] = &[
    ("PixelRatio", "1"),
    ("Smoothing", "Low"),
    ("BackgroundColor", "#00000000"),
    ("TargetFps", "0"),
    ("AutoPresent", "True"),
    ("AccessibleLabel", ""),
    ("FixedStep", "False"),
    ("PointerX", "0"),
    ("PointerY", "0"),
    ("ClearColor", "#000000FF"),
    ("Msaa", "4"),
];

/// The values of `Smoothing` (display-list `SetSmoothing` 0, 1, 2).
pub const SMOOTHING: &[&str] = &["Off", "Low", "High"];

/// The values of Canvas3D's read-only `Backend` (R1 and plan 1.8: `OpenGL` is what the Windows rule can produce;
/// `None` before it connects).
pub const BACKENDS: &[&str] = &["WebGPU", "WebGL2", "Vulkan", "Metal", "Dx12", "OpenGL", "Software", "None"];

/// Read-only properties a Canvas3D carries at run time.
pub const CANVAS3D_RUNTIME_PROPS: &[&str] = &["Backend"];

/// Is `t` one of the seven?
pub fn is_graphics(t: &ControlType) -> bool {
    matches!(
        t,
        ControlType::Canvas2D
            | ControlType::Canvas3D
            | ControlType::ImageBuffer
            | ControlType::Plot
            | ControlType::Scene3D
            | ControlType::Physics2D
            | ControlType::Physics3D
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names_are_the_types_and_every_type_round_trips_its_name() {
        assert_eq!(NAMES.len(), 7);
        for n in NAMES {
            let t = ControlType::from_str(n);
            assert!(is_graphics(&t), "{n} is not a model type");
            assert_eq!(t.as_str(), *n);
        }
    }
}
