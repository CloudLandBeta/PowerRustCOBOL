// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Canvas2D object: a recording list in front of a [`Surface`] (WASM-018 R6, R7).
//!
//! A program's drawing methods **record** commands into the canvas's display list;
//! nothing is drawn until the list is **flushed** - by `Present`, by `Flush`, at the
//! end of an `onFrame` handler with `AutoPresent`, or by a method that reads the
//! picture back (`ToImage`, `GetPixel`, `MeasureText`, `IsPointInPath`). A flush is
//! one call into the rasteriser however many shapes were recorded: ten thousand
//! `FillRect`s are one host call ([`Canvas2D::host_calls`] counts them, which is what
//! AC2 measures).

use crate::canvas::{Outcome, ReadBack, Surface, SurfaceConfig};
use crate::color::Rgba;
use crate::display_list::DisplayListBuilder;
use crate::error::{GraphicsError, Result};
use crate::limits::Limits;
use crate::pixels::PixelBuffer;
use crate::resources::Resources;
use crate::textapi::TextMetrics;
use crate::value::Value;

/// What a flush did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlushReport {
    /// Commands executed.
    pub commands: u64,
    /// Whether the list contained `Present`.
    pub presented: bool,
    /// Things skipped with a reason (a missing image, no fonts ...).
    pub diagnostics: Vec<String>,
}

/// A canvas.
pub struct Canvas2D {
    surface: Surface,
    list: DisplayListBuilder,
    limits: Limits,
    width: u32,
    height: u32,
    ratio: f32,
    host_calls: u64,
    diagnostics: Vec<String>,
    presented_since_read: bool,
}

impl std::fmt::Debug for Canvas2D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Canvas2D({}x{} @{}, {} recorded)", self.width, self.height, self.ratio, self.list.command_count())
    }
}

impl Canvas2D {
    /// A transparent canvas; the size is checked against the limits.
    pub fn new(width: u32, height: u32, pixel_ratio: f32, limits: Limits) -> Result<Canvas2D> {
        let surface = Surface::new(&SurfaceConfig { pixel_ratio, ..SurfaceConfig::new(width, height) }, &limits)?;
        Ok(Canvas2D { surface, list: DisplayListBuilder::with_limits(limits.clone()), limits, width, height, ratio: pixel_ratio, host_calls: 0, diagnostics: Vec::new(), presented_since_read: false })
    }

    /// Size in logical pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Size in device pixels (what the pixel accessors use).
    pub fn device_size(&self) -> (u32, u32) {
        self.surface.device_size()
    }

    /// Device pixels per logical pixel.
    pub fn pixel_ratio(&self) -> f32 {
        self.ratio
    }

    /// `Resize`: a new size (and ratio). The picture and the drawing state are cleared and what
    /// was recorded is thrown away, as when an HTML canvas is resized.
    pub fn resize(&mut self, width: u32, height: u32, pixel_ratio: f32) -> Result<()> {
        let surface = Surface::new(&SurfaceConfig { pixel_ratio, ..SurfaceConfig::new(width, height) }, &self.limits)?;
        self.surface = surface;
        self.width = width;
        self.height = height;
        self.ratio = pixel_ratio;
        self.list.discard();
        Ok(())
    }

    /// The memory the canvas holds.
    pub fn memory_bytes(&self) -> u64 {
        self.surface.memory_bytes() + self.list.payload_len() as u64
    }

    /// How many commands are recorded and not yet drawn.
    pub fn pending_commands(&self) -> u32 {
        self.list.command_count()
    }

    /// How many times the rasteriser was called (a flush with something to do is one call).
    pub fn host_calls(&self) -> u64 {
        self.host_calls
    }

    /// Record a command by name (see [`DisplayListBuilder::push_dynamic`]).
    pub fn record(&mut self, command: &str, args: &[Value]) -> Result<()> {
        self.list.push_dynamic(command, args)
    }

    /// The list being recorded, for typed recording from Rust (the egui host, tests, `Plot::render_into`).
    pub fn list(&mut self) -> &mut DisplayListBuilder {
        &mut self.list
    }

    /// `Discard`: forget what was recorded without drawing it.
    pub fn discard(&mut self) {
        self.list.discard();
    }

    /// Diagnostics collected since the last call (a missing image, no fonts ...).
    pub fn take_diagnostics(&mut self) -> Vec<String> {
        std::mem::take(&mut self.diagnostics)
    }

    /// Draw everything recorded.
    pub fn flush(&mut self, res: &dyn Resources) -> Result<FlushReport> {
        if self.list.is_empty() && self.list.error().is_none() {
            return Ok(FlushReport::default());
        }
        let out = self.run(res)?;
        Ok(FlushReport { commands: out.commands_executed, presented: out.presented, diagnostics: out.diagnostics })
    }

    fn run(&mut self, res: &dyn Resources) -> Result<Outcome> {
        let bytes = self.list.take()?;
        self.host_calls += 1;
        let out = self.surface.execute(&bytes, res)?;
        if out.presented {
            self.presented_since_read = true;
        }
        for d in &out.diagnostics {
            if self.diagnostics.len() < 100 {
                self.diagnostics.push(d.clone());
            }
        }
        Ok(out)
    }

    /// Whether `Present` ran since the host last took the picture (for the host's repaint decision).
    pub fn take_presented(&mut self) -> bool {
        std::mem::take(&mut self.presented_since_read)
    }

    /// `GetPixel`: the colour at a canvas point (logical pixels), or `None` outside the canvas.
    pub fn get_pixel(&mut self, res: &dyn Resources, x: f32, y: f32) -> Result<Option<Rgba>> {
        self.list.read_pixel(0, x, y);
        let out = self.run(res)?;
        out.readbacks
            .into_iter()
            .find_map(|r| match r {
                ReadBack::Pixel { slot: 0, color } => Some(color),
                _ => None,
            })
            .ok_or_else(|| GraphicsError::invalid("the canvas gave no answer for GetPixel"))
    }

    /// `IsPointInPath`: whether a canvas point is inside the current path (`even_odd` picks the fill rule).
    pub fn hit_path(&mut self, res: &dyn Resources, x: f32, y: f32, even_odd: bool) -> Result<bool> {
        self.list.hit_path(0, x, y, even_odd as u8);
        let out = self.run(res)?;
        out.readbacks
            .into_iter()
            .find_map(|r| match r {
                ReadBack::Hit { slot: 0, inside } => Some(inside),
                _ => None,
            })
            .ok_or_else(|| GraphicsError::invalid("the canvas gave no answer for IsPointInPath"))
    }

    /// `IsPointInStroke`: whether a canvas point is on the stroke of the current path.
    pub fn hit_stroke(&mut self, res: &dyn Resources, x: f32, y: f32) -> Result<bool> {
        self.list.hit_stroke(0, x, y);
        let out = self.run(res)?;
        out.readbacks
            .into_iter()
            .find_map(|r| match r {
                ReadBack::Hit { slot: 0, inside } => Some(inside),
                _ => None,
            })
            .ok_or_else(|| GraphicsError::invalid("the canvas gave no answer for IsPointInStroke"))
    }

    /// `MeasureText`: the metrics of a text in the current font.
    pub fn measure_text(&mut self, res: &dyn Resources, text: &str) -> Result<TextMetrics> {
        self.limits.check_text(text.len())?;
        self.list.measure_text(0, text);
        let out = self.run(res)?;
        out.readbacks
            .into_iter()
            .find_map(|r| match r {
                ReadBack::Text { slot: 0, metrics } => Some(metrics),
                _ => None,
            })
            .ok_or_else(|| GraphicsError::invalid("there is no font to measure with (load one with LoadFont)"))
    }

    /// `Capture`: a rectangle of the canvas as an image (device resolution).
    pub fn capture(&mut self, res: &dyn Resources, x: f32, y: f32, w: f32, h: f32) -> Result<Option<PixelBuffer>> {
        self.list.capture(0, x, y, w, h);
        let out = self.run(res)?;
        out.readbacks
            .into_iter()
            .find_map(|r| match r {
                ReadBack::Image { slot: 0, image } => Some(image),
                _ => None,
            })
            .ok_or_else(|| GraphicsError::invalid("the canvas gave no answer for Capture"))
    }

    /// `ToImage`: draw what is recorded and return the whole picture (device resolution, straight alpha).
    pub fn to_image(&mut self, res: &dyn Resources) -> Result<PixelBuffer> {
        self.flush(res)?;
        Ok(self.surface.to_image())
    }

    /// The picture without drawing what is pending, as straight-alpha RGBA bytes (`width * height * 4` of device size).
    pub fn write_rgba(&self, out: &mut [u8]) {
        self.surface.write_rgba(out);
    }

    /// The picture as premultiplied RGBA bytes (what egui and most GPU uploads take).
    pub fn write_rgba_premultiplied(&self, out: &mut [u8]) {
        self.surface.write_rgba_premultiplied(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::NoResources;

    #[test]
    fn ten_thousand_shapes_are_one_host_call() {
        let mut c = Canvas2D::new(200, 100, 1.0, Limits::default()).unwrap();
        c.record("SetFillColor", &[Value::Text("#2060C0".into())]).unwrap();
        for i in 0..10_000 {
            c.record("FillRect", &[Value::Num((i % 190) as f64), Value::Num((i % 90) as f64), 4.into(), 4.into()]).unwrap();
        }
        assert_eq!(c.pending_commands(), 10_001);
        assert_eq!(c.host_calls(), 0, "nothing drawn yet");
        c.record("Present", &[]).unwrap();
        let r = c.flush(&NoResources).unwrap();
        assert!(r.presented && r.commands == 10_002);
        assert_eq!(c.host_calls(), 1, "one call for ten thousand rectangles (AC2)");
        assert!(c.take_presented());
        assert!(!c.take_presented());
        assert_eq!(c.pending_commands(), 0);
        // An empty flush is not a host call.
        c.flush(&NoResources).unwrap();
        assert_eq!(c.host_calls(), 1);
    }

    #[test]
    fn reading_back_draws_first_and_answers_what_was_drawn() {
        let mut c = Canvas2D::new(50, 50, 1.0, Limits::default()).unwrap();
        c.record("SetFillColor", &[Value::Int(0xFF0000FF)]).unwrap();
        c.record("FillRect", &[10.into(), 10.into(), 20.into(), 20.into()]).unwrap();
        // The read draws the pending list, then reads.
        assert_eq!(c.get_pixel(&NoResources, 15.0, 15.0).unwrap(), Some(Rgba::rgb(255, 0, 0)));
        assert_eq!(c.get_pixel(&NoResources, 5.0, 5.0).unwrap(), Some(Rgba::TRANSPARENT));
        assert_eq!(c.get_pixel(&NoResources, 500.0, 5.0).unwrap(), None);
        assert_eq!(c.host_calls(), 3);
        c.record("BeginPath", &[]).unwrap();
        c.record("Rect", &[0.into(), 0.into(), 10.into(), 10.into()]).unwrap();
        assert!(c.hit_path(&NoResources, 5.0, 5.0, false).unwrap());
        assert!(!c.hit_path(&NoResources, 15.0, 5.0, false).unwrap());
        let img = c.to_image(&NoResources).unwrap();
        assert_eq!(img.get(15, 15), Some(Rgba::rgb(255, 0, 0)));
        let cap = c.capture(&NoResources, 10.0, 10.0, 5.0, 5.0).unwrap().unwrap();
        assert_eq!((cap.width(), cap.height()), (5, 5));
        assert_eq!(cap.get(0, 0), Some(Rgba::rgb(255, 0, 0)));
    }

    #[test]
    fn resize_clears_and_checks_the_limits() {
        let mut c = Canvas2D::new(20, 20, 1.0, Limits::default()).unwrap();
        c.record("SetFillColor", &[Value::Int(0x000000FF)]).unwrap();
        c.record("FillRect", &[0.into(), 0.into(), 20.into(), 20.into()]).unwrap();
        c.flush(&NoResources).unwrap();
        c.record("Save", &[]).unwrap();
        c.resize(30, 10, 2.0).unwrap();
        assert_eq!(c.size(), (30, 10));
        assert_eq!(c.device_size(), (60, 20));
        assert_eq!(c.pending_commands(), 0);
        assert_eq!(c.to_image(&NoResources).unwrap().get(5, 5), Some(Rgba::TRANSPARENT));
        assert!(c.resize(100_000, 10, 1.0).is_err());
        assert_eq!(c.size(), (30, 10), "a refused resize changes nothing");
        assert!(Canvas2D::new(0, 10, 1.0, Limits::default()).is_err());
    }

    #[test]
    fn the_command_limit_is_reported_at_the_flush() {
        let mut c = Canvas2D::new(10, 10, 1.0, Limits { max_display_list_commands: 5, ..Limits::default() }).unwrap();
        for _ in 0..10 {
            c.record("Save", &[]).unwrap();
        }
        let e = c.flush(&NoResources).unwrap_err();
        assert!(e.to_string().contains("max_display_list_commands"), "{e}");
        assert_eq!(c.pending_commands(), 0, "and the list starts over");
        c.record("Save", &[]).unwrap();
        c.flush(&NoResources).unwrap();
    }

    #[test]
    fn measure_without_a_font_says_how_to_fix_it() {
        let mut c = Canvas2D::new(10, 10, 1.0, Limits::default()).unwrap();
        let e = c.measure_text(&NoResources, "hello").unwrap_err();
        assert!(e.to_string().contains("LoadFont"), "{e}");
    }
}
