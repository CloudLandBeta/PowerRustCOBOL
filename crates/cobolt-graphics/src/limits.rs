// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Configurable limits (WASM-018 R15).
//!
//! "The graphics objects shall have limits - canvas size, decoded-image pixels,
//! model size, texture size, number of nodes and bodies, display-list length -
//! set by the project with safe defaults, and shall refuse past them with a
//! readable error."
//!
//! A [`Limits`] value travels with every object that creates something sized by
//! its input. Each `check_*` method returns the readable
//! [`GraphicsError::LimitExceeded`] naming the setting that raises the limit, so
//! the message a developer sees says what to change.
//!
//! The defaults are chosen so a phone browser survives them: they allow a 24
//! megapixel photograph and a 4096 x 4096 canvas, and nothing beyond what a
//! 32-bit WebAssembly memory could hold anyway.

use crate::error::{GraphicsError, Result};

/// All the limits of R15 in one place. `Limits::default()` is the safe default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Largest canvas width in logical pixels.
    pub max_canvas_width: u32,
    /// Largest canvas height in logical pixels.
    pub max_canvas_height: u32,
    /// Largest canvas area in pixels (width x height, device pixels).
    pub max_canvas_pixels: u64,
    /// Largest decoded image, in pixels. Checked from the file header
    /// *before* any pixel memory is allocated.
    pub max_image_pixels: u64,
    /// Largest encoded input (an image or a font file) accepted for decoding.
    pub max_encoded_bytes: u64,
    /// Largest model file (glTF / GLB), in bytes.
    pub max_model_bytes: u64,
    /// Largest texture edge, in pixels.
    pub max_texture_size: u32,
    /// Most nodes in one scene.
    pub max_nodes: u32,
    /// Most triangles in one scene.
    pub max_triangles: u64,
    /// Most bodies in one physics world.
    pub max_bodies: u32,
    /// Longest display list, in bytes.
    pub max_display_list_bytes: u64,
    /// Most commands in one display list.
    pub max_display_list_commands: u64,
    /// Deepest `Save` nesting.
    pub max_save_depth: u32,
    /// Most points in one path or polygon.
    pub max_path_points: u32,
    /// Longest text passed to one draw or measure call, in bytes.
    pub max_text_bytes: u32,
    /// Largest single font file, in bytes.
    pub max_font_bytes: u64,
    /// Most points in one plot series.
    pub max_series_points: u64,
    /// Most bytes of live pixel/vertex memory the registry may account for.
    pub max_total_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_canvas_width: 8192,
            max_canvas_height: 8192,
            max_canvas_pixels: 16_777_216, // 4096 x 4096
            max_image_pixels: 50_000_000,
            max_encoded_bytes: 128 * 1024 * 1024,
            max_model_bytes: 64 * 1024 * 1024,
            max_texture_size: 8192,
            max_nodes: 100_000,
            max_triangles: 5_000_000,
            max_bodies: 100_000,
            max_display_list_bytes: 32 * 1024 * 1024,
            max_display_list_commands: 2_000_000,
            max_save_depth: 1024,
            max_path_points: 1_000_000,
            max_text_bytes: 1024 * 1024,
            max_font_bytes: 64 * 1024 * 1024,
            max_series_points: 10_000_000,
            max_total_bytes: 1024 * 1024 * 1024,
        }
    }
}

fn over(what: &'static str, requested: u64, limit: u64, setting: &'static str) -> Result<()> {
    if requested > limit {
        Err(GraphicsError::LimitExceeded { what, requested, limit, setting })
    } else {
        Ok(())
    }
}

impl Limits {
    /// These limits with the texture edge held to what the graphics device creates
    /// (`max_texture_dimension_2d`): a project may ask for more than the machine has.
    pub fn clamped_to_device(&self, max_texture_dimension_2d: u32) -> Limits {
        Limits { max_texture_size: self.max_texture_size.min(max_texture_dimension_2d), ..self.clone() }
    }

    /// A canvas of `width` x `height` logical pixels at `pixel_ratio`.
    pub fn check_canvas(&self, width: u32, height: u32, pixel_ratio: f32) -> Result<()> {
        if width == 0 || height == 0 {
            return Err(GraphicsError::invalid("a canvas needs a width and a height of at least 1 pixel"));
        }
        if !(pixel_ratio.is_finite() && pixel_ratio > 0.0 && pixel_ratio <= 8.0) {
            return Err(GraphicsError::invalid("PixelRatio must be a number from above 0 up to 8"));
        }
        over("canvas width", width as u64, self.max_canvas_width as u64, "max_canvas_width")?;
        over("canvas height", height as u64, self.max_canvas_height as u64, "max_canvas_height")?;
        let dw = (width as f64 * pixel_ratio as f64).ceil() as u64;
        let dh = (height as f64 * pixel_ratio as f64).ceil() as u64;
        over("canvas pixels", dw.saturating_mul(dh), self.max_canvas_pixels, "max_canvas_pixels")
    }

    /// A decoded or created image of `width` x `height` pixels.
    pub fn check_image(&self, width: u32, height: u32) -> Result<()> {
        if width == 0 || height == 0 {
            return Err(GraphicsError::invalid("an image needs a width and a height of at least 1 pixel"));
        }
        over(
            "image pixels",
            (width as u64).saturating_mul(height as u64),
            self.max_image_pixels,
            "max_image_pixels",
        )
    }

    /// An encoded input of `len` bytes (image or font) about to be decoded.
    pub fn check_encoded(&self, len: usize) -> Result<()> {
        over("encoded file size in bytes", len as u64, self.max_encoded_bytes, "max_encoded_bytes")
    }

    /// A model file of `len` bytes.
    pub fn check_model_bytes(&self, len: usize) -> Result<()> {
        over("model file size in bytes", len as u64, self.max_model_bytes, "max_model_bytes")
    }

    /// A font file of `len` bytes.
    pub fn check_font_bytes(&self, len: usize) -> Result<()> {
        over("font file size in bytes", len as u64, self.max_font_bytes, "max_font_bytes")
    }

    /// A texture edge.
    pub fn check_texture(&self, width: u32, height: u32) -> Result<()> {
        let edge = width.max(height);
        over("texture size", edge as u64, self.max_texture_size as u64, "max_texture_size")
    }

    /// A scene reaching `nodes` nodes.
    pub fn check_nodes(&self, nodes: u64) -> Result<()> {
        over("scene nodes", nodes, self.max_nodes as u64, "max_nodes")
    }

    /// A scene reaching `triangles` triangles.
    pub fn check_triangles(&self, triangles: u64) -> Result<()> {
        over("scene triangles", triangles, self.max_triangles, "max_triangles")
    }

    /// One mesh of `vertices` vertices (bounded by the scene's triangle limit: a mesh cannot
    /// usefully have more vertices than the scene may hold triangles).
    pub fn check_vertices(&self, vertices: u64) -> Result<()> {
        over("vertices in one mesh", vertices, self.max_triangles, "max_triangles")
    }

    /// A world reaching `bodies` bodies.
    pub fn check_bodies(&self, bodies: u64) -> Result<()> {
        over("physics bodies", bodies, self.max_bodies as u64, "max_bodies")
    }

    /// A display list of `bytes` bytes.
    pub fn check_display_list_bytes(&self, bytes: u64) -> Result<()> {
        over("display list bytes", bytes, self.max_display_list_bytes, "max_display_list_bytes")
    }

    /// A display list of `commands` commands.
    pub fn check_display_list_commands(&self, commands: u64) -> Result<()> {
        over(
            "display list commands",
            commands,
            self.max_display_list_commands,
            "max_display_list_commands",
        )
    }

    /// A `Save` nested `depth` deep.
    pub fn check_save_depth(&self, depth: u64) -> Result<()> {
        over("Save nesting depth", depth, self.max_save_depth as u64, "max_save_depth")
    }

    /// A path or polygon with `points` points.
    pub fn check_path_points(&self, points: u64) -> Result<()> {
        over("path points", points, self.max_path_points as u64, "max_path_points")
    }

    /// A text of `len` bytes passed to one call.
    pub fn check_text(&self, len: usize) -> Result<()> {
        over("text length in bytes", len as u64, self.max_text_bytes as u64, "max_text_bytes")
    }

    /// A series of `points` points.
    pub fn check_series_points(&self, points: u64) -> Result<()> {
        over("plot series points", points, self.max_series_points, "max_series_points")
    }

    /// Total accounted memory of `bytes` bytes.
    pub fn check_total(&self, bytes: u64) -> Result<()> {
        over("graphics memory in bytes", bytes, self.max_total_bytes, "max_total_bytes")
    }
}

impl Limits {
    /// The names a project may set, in the order of the fields.
    pub const SETTINGS: &'static [&'static str] = &[
        "max_canvas_width",
        "max_canvas_height",
        "max_canvas_pixels",
        "max_image_pixels",
        "max_encoded_bytes",
        "max_model_bytes",
        "max_texture_size",
        "max_nodes",
        "max_triangles",
        "max_bodies",
        "max_display_list_bytes",
        "max_display_list_commands",
        "max_save_depth",
        "max_path_points",
        "max_text_bytes",
        "max_font_bytes",
        "max_series_points",
        "max_total_bytes",
    ];

    /// These limits with one project setting changed (R15: "set by the project"). The name is the one an error
    /// message tells a developer to raise; an unknown name or a value that does not fit is refused, not ignored.
    pub fn with_setting(&self, name: &str, value: u64) -> Result<Limits> {
        let mut l = self.clone();
        let small = |v: u64| u32::try_from(v).map_err(|_| GraphicsError::invalid(format!("{name} = {value} does not fit: the largest is {}", u32::MAX)));
        match name.trim().to_ascii_lowercase().as_str() {
            "max_canvas_width" => l.max_canvas_width = small(value)?,
            "max_canvas_height" => l.max_canvas_height = small(value)?,
            "max_canvas_pixels" => l.max_canvas_pixels = value,
            "max_image_pixels" => l.max_image_pixels = value,
            "max_encoded_bytes" => l.max_encoded_bytes = value,
            "max_model_bytes" => l.max_model_bytes = value,
            "max_texture_size" => l.max_texture_size = small(value)?,
            "max_nodes" => l.max_nodes = small(value)?,
            "max_triangles" => l.max_triangles = value,
            "max_bodies" => l.max_bodies = small(value)?,
            "max_display_list_bytes" => l.max_display_list_bytes = value,
            "max_display_list_commands" => l.max_display_list_commands = value,
            "max_save_depth" => l.max_save_depth = small(value)?,
            "max_path_points" => l.max_path_points = small(value)?,
            "max_text_bytes" => l.max_text_bytes = small(value)?,
            "max_font_bytes" => l.max_font_bytes = value,
            "max_series_points" => l.max_series_points = value,
            "max_total_bytes" => l.max_total_bytes = value,
            other => {
                return Err(GraphicsError::invalid(format!("`{other}` is not a graphics limit; the settings are: {}", Limits::SETTINGS.join(", "))));
            }
        }
        Ok(l)
    }

    /// The defaults with every `(name, value)` of a project applied, in order.
    pub fn from_settings(settings: &[(String, u64)]) -> Result<Limits> {
        settings.iter().try_fold(Limits::default(), |l, (k, v)| l.with_setting(k, *v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_allow_a_24_megapixel_photo_and_a_4k_square_canvas() {
        let l = Limits::default();
        l.check_image(6000, 4000).unwrap();
        l.check_canvas(4096, 4096, 1.0).unwrap();
    }

    #[test]
    fn oversized_canvas_is_refused_with_the_setting_named() {
        let l = Limits::default();
        let e = l.check_canvas(8000, 8000, 1.0).unwrap_err();
        assert!(e.to_string().contains("max_canvas_pixels"), "{e}");
        let e = l.check_canvas(9000, 10, 1.0).unwrap_err();
        assert!(e.to_string().contains("max_canvas_width"), "{e}");
    }

    #[test]
    fn pixel_ratio_counts_towards_the_area() {
        let l = Limits::default();
        l.check_canvas(2048, 2048, 2.0).unwrap();
        assert!(l.check_canvas(2049, 2049, 2.0).is_err());
        assert!(l.check_canvas(10, 10, 0.0).is_err());
        assert!(l.check_canvas(10, 10, f32::NAN).is_err());
    }

    #[test]
    fn zero_sizes_are_refused() {
        let l = Limits::default();
        assert!(l.check_canvas(0, 5, 1.0).is_err());
        assert!(l.check_image(5, 0).is_err());
    }

    #[test]
    fn a_project_setting_changes_one_limit_and_a_bad_one_is_refused() {
        let l = Limits::from_settings(&[("max_canvas_width".into(), 16384), ("MAX_BODIES".into(), 50)]).unwrap();
        assert_eq!(l.max_canvas_width, 16384);
        assert_eq!(l.max_bodies, 50);
        assert_eq!(l.max_nodes, Limits::default().max_nodes);
        let e = Limits::default().with_setting("max_canvas_wdith", 1).unwrap_err().to_string();
        assert!(e.contains("max_canvas_width"), "the message lists the real names: {e}");
        assert!(Limits::default().with_setting("max_nodes", u64::MAX).is_err());
        // every name the settings list accepts
        for n in Limits::SETTINGS {
            Limits::default().with_setting(n, 7).unwrap();
        }
        println!("limits: {} settings, all accepted; unknown and oversized values refused", Limits::SETTINGS.len());
    }
}
