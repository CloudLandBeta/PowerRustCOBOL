// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Canvas 2D rasteriser: executes display lists on `tiny-skia`
//! (WASM-018 R7).
//!
//! A [`Surface`] owns the pixels and the graphics state (transform, styles,
//! clip, font ...) of one canvas. [`Surface::execute`] takes the bytes of a
//! display list, validates all of it before drawing anything, runs it and returns
//! an [`Outcome`] with the answers to the read-back commands.
//!
//! Rasterisation is CPU, anti-aliased and **deterministic**: no GPU, no
//! platform text, no platform `libm` in the pixel path (the series used for the
//! shadow kernel is explicit), so the picture is the same on the desktop, in
//! the browser and on a phone. A large canvas (2 megapixels or more) is cut
//! into bands of 256 rows - a function of the canvas size alone - that run in
//! parallel when threads exist and give the same bytes with one thread or many.
//!
//! # Differences from the HTML Canvas 2D API
//!
//! | HTML | here |
//! |---|---|
//! | `clearRect`, `fillRect` ... in CSS pixels, scaled by `devicePixelRatio` by the page | `PixelRatio` is a property of the canvas; all coordinates are logical pixels |
//! | `createLinearGradient()` and `addColorStop()` objects | `SetLinearGradient` takes the stops in one call |
//! | `fillStyle` may hold a gradient object | `SetFillColor`, `SetLinearGradient`, `SetRadialGradient`, `SetConicGradient`, `SetPattern` each replace the fill (or stroke) style |
//! | Invalid arguments throw or are ignored | A call with a non-finite number, a negative radius or an unknown enumeration value is **ignored**, as in HTML; nothing throws |
//! | `fillText` newline handling | `DrawText` turns line breaks into spaces; `DrawTextBox` wraps and honours them |
//! | `measureText()` returns an object | `MeasureText` returns the metrics through a read-back slot |
//! | `globalCompositeOperation` applies to the whole clip region | The same - operations that clear pixels outside the shape (`source-in`, `copy`, ...) are computed on a layer |
//! | `drawImage` of any CanvasImageSource | `DrawImage` of an `ImageBuffer` only |
//! | Shadows blur in CSS pixels | Shadow offsets and blur are scaled by `PixelRatio`; the offset is rounded to whole device pixels |
//! | `isPointInPath(x, y)` | `HitPath` takes the point in canvas (logical) coordinates; the path is stored in device space as in HTML |
//! | `Path2D` objects | not provided; there is one current path |
//! | `filter`, `imageSmoothingQuality` strings | `SetSmoothing` 0, 1, 2; no CSS filters (use `ImageBuffer` filters) |
//! | A `roundRect` with a `DOMPointInit` radius | four numbers, one per corner |

mod band;
mod blur;
mod paint;
mod path;
mod state;

use std::collections::HashMap;
use std::sync::Arc;

use kurbo::Affine;
use tiny_skia::Pixmap;

use crate::color::Rgba;
use crate::display_list::{Command, DisplayList};
use crate::error::{GraphicsError, Result};
use crate::limits::Limits;
use crate::par;
use crate::pixels::PixelBuffer;
use crate::resources::Resources;
use crate::textapi::TextMetrics;

use band::{merge_reads, Band, BandOutcome, RunCtx};

/// Canvases with at least this many device pixels are cut into bands.
pub const TILE_MIN_PIXELS: u64 = 2_000_000;
/// Rows per band when a canvas is tiled automatically.
pub const TILE_ROWS: u32 = 256;

/// How to create a [`Surface`].
#[derive(Debug, Clone)]
pub struct SurfaceConfig {
    /// Width in logical pixels.
    pub width: u32,
    /// Height in logical pixels.
    pub height: u32,
    /// Device pixels per logical pixel (`PixelRatio`).
    pub pixel_ratio: f32,
    /// Force bands of this many rows (at least 8). `None` lets the canvas size
    /// decide ([`TILE_MIN_PIXELS`], [`TILE_ROWS`]). Tests use this to compare
    /// the banded picture with the unbanded one.
    pub tile_rows: Option<u32>,
}

impl SurfaceConfig {
    /// A canvas of this size at ratio 1.
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height, pixel_ratio: 1.0, tile_rows: None }
    }
}

/// The answer to one read-back command.
#[derive(Debug, Clone, PartialEq)]
pub enum ReadBack {
    /// `ReadPixel`: the colour, or `None` outside the canvas.
    Pixel {
        /// The command's slot.
        slot: u32,
        /// The pixel (straight alpha).
        color: Option<Rgba>,
    },
    /// `HitPath` / `HitStroke`.
    Hit {
        /// The command's slot.
        slot: u32,
        /// Whether the point is inside.
        inside: bool,
    },
    /// `Capture`: the rectangle as an image (`None` when the rectangle is empty).
    Image {
        /// The command's slot.
        slot: u32,
        /// The pixels, at device resolution.
        image: Option<PixelBuffer>,
    },
    /// `MeasureText`.
    Text {
        /// The command's slot.
        slot: u32,
        /// The metrics.
        metrics: TextMetrics,
    },
}

/// What executing a display list produced besides pixels.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outcome {
    /// Whether the list contained `Present`.
    pub presented: bool,
    /// Answers to the read-back commands, in command order.
    pub readbacks: Vec<ReadBack>,
    /// Things that were skipped with a reason (a missing image, no fonts ...).
    /// Not errors: the rest of the list ran.
    pub diagnostics: Vec<String>,
    /// How many commands ran.
    pub commands_executed: u64,
}

/// The pixels and state of one Canvas 2D.
pub struct Surface {
    logical: (u32, u32),
    device: (u32, u32),
    ratio: f32,
    bands: Vec<Band>,
    limits: Limits,
    image_cache: HashMap<u32, (Arc<PixelBuffer>, Arc<Pixmap>)>,
}

impl std::fmt::Debug for Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Surface({}x{} @{}, {} band(s))", self.logical.0, self.logical.1, self.ratio, self.bands.len())
    }
}

impl Surface {
    /// A transparent canvas. The size is checked against `limits`.
    pub fn new(cfg: &SurfaceConfig, limits: &Limits) -> Result<Surface> {
        limits.check_canvas(cfg.width, cfg.height, cfg.pixel_ratio)?;
        let dw = ((cfg.width as f64) * cfg.pixel_ratio as f64).ceil().max(1.0) as u32;
        let dh = ((cfg.height as f64) * cfg.pixel_ratio as f64).ceil().max(1.0) as u32;
        let base = Affine::scale(cfg.pixel_ratio as f64);
        let pixels = dw as u64 * dh as u64;
        let rows = match cfg.tile_rows {
            Some(r) => r.max(8),
            None if pixels >= TILE_MIN_PIXELS => TILE_ROWS,
            None => dh,
        };
        let mut bands = Vec::new();
        let mut y = 0;
        while y < dh {
            let h = rows.min(dh - y);
            let b = Band::new(y, dw, h, base, y == 0)
                .ok_or_else(|| GraphicsError::invalid("the canvas could not be allocated"))?;
            bands.push(b);
            y += h;
        }
        Ok(Surface {
            logical: (cfg.width, cfg.height),
            device: (dw, dh),
            ratio: cfg.pixel_ratio,
            bands,
            limits: limits.clone(),
            image_cache: HashMap::new(),
        })
    }

    /// Size in logical pixels.
    pub fn logical_size(&self) -> (u32, u32) {
        self.logical
    }

    /// Size in device pixels (what the pixel accessors use).
    pub fn device_size(&self) -> (u32, u32) {
        self.device
    }

    /// Device pixels per logical pixel.
    pub fn pixel_ratio(&self) -> f32 {
        self.ratio
    }

    /// How many bands the canvas is cut into (1 when it is not tiled).
    pub fn band_count(&self) -> usize {
        self.bands.len()
    }

    /// The memory the canvas holds, for the registry's accounting.
    pub fn memory_bytes(&self) -> u64 {
        self.device.0 as u64 * self.device.1 as u64 * 4
    }

    /// Collect the images a list refers to and make premultiplied pixmaps of
    /// them (once each, cached while the image is unchanged).
    fn prepare_images(&mut self, list: &DisplayList<'_>, res: &dyn Resources) -> HashMap<u32, Arc<Pixmap>> {
        let mut ids: Vec<u32> = Vec::new();
        for c in list.commands().flatten() {
            match c {
                Command::DrawImage { image, .. }
                | Command::DrawImageScaled { image, .. }
                | Command::DrawImageRegion { image, .. }
                | Command::DrawSprite { image, .. }
                | Command::SetPattern { image, .. } => {
                    if !ids.contains(&image) {
                        ids.push(image);
                    }
                }
                _ => {}
            }
        }
        let mut out = HashMap::new();
        for id in ids {
            let Some(img) = res.image(id) else { continue };
            let hit = self.image_cache.get(&id).filter(|(old, _)| Arc::ptr_eq(old, &img)).map(|(_, pm)| pm.clone());
            let pm = match hit {
                Some(pm) => pm,
                None => match img.to_pixmap() {
                    Some(pm) => {
                        let pm = Arc::new(pm);
                        if self.image_cache.len() >= 128 {
                            self.image_cache.clear();
                        }
                        self.image_cache.insert(id, (img, pm.clone()));
                        pm
                    }
                    None => continue,
                },
            };
            out.insert(id, pm);
        }
        out
    }

    /// Execute a display list.
    ///
    /// The whole list is parsed and validated before the first pixel changes,
    /// so a damaged or oversized list leaves the canvas as it was and returns
    /// the error.
    pub fn execute(&mut self, bytes: &[u8], res: &dyn Resources) -> Result<Outcome> {
        let list = DisplayList::parse(bytes, &self.limits)?;
        list.validate()?;
        let images = self.prepare_images(&list, res);
        let ctx = RunCtx { res, images: &images, limits: &self.limits, ratio: self.ratio, canvas: self.device };

        struct Work<'b> {
            band: &'b mut Band,
            result: Option<Result<BandOutcome>>,
        }
        let mut work: Vec<Work<'_>> = self.bands.iter_mut().map(|band| Work { band, result: None }).collect();
        par::for_each_chunk_mut(&mut work, 1, |_, chunk| {
            let w = &mut chunk[0];
            w.result = Some(w.band.run(&list, &ctx));
        });
        let mut outcomes = Vec::with_capacity(work.len());
        for w in work {
            outcomes.push(w.result.expect("every band ran")?);
        }
        let n_reads = outcomes.first().map(|o| o.reads.len()).unwrap_or(0);
        let mut outcomes = outcomes;
        let primary = std::mem::take(&mut outcomes[0]);
        let mut all = vec![primary];
        all.extend(outcomes.into_iter().skip(1));
        let presented = all[0].presented;
        let diagnostics = std::mem::take(&mut all[0].diagnostics);
        let commands_executed = all[0].executed;
        let readbacks = merge_reads(all, n_reads);
        Ok(Outcome { presented, readbacks, diagnostics, commands_executed })
    }

    /// The whole picture as straight-alpha RGBA at device resolution (`ToImage`).
    pub fn to_image(&self) -> PixelBuffer {
        let (w, h) = self.device;
        let mut data = vec![0u8; w as usize * h as usize * 4];
        self.write_rgba(&mut data);
        PixelBuffer::from_rgba(w, h, data).expect("sizes match")
    }

    /// Write the picture as **straight** alpha RGBA (what a browser `ImageData`
    /// wants). `out` must be `width * height * 4` bytes of device size.
    pub fn write_rgba(&self, out: &mut [u8]) {
        let stride = self.device.0 as usize * 4;
        for b in &self.bands {
            let off = b.y0 as usize * stride;
            let dst = &mut out[off..off + b.pm.pixels().len() * 4];
            for (d, p) in dst.chunks_exact_mut(4).zip(b.pm.pixels()) {
                let c = p.demultiply();
                d.copy_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
            }
        }
    }

    /// Write the picture as **premultiplied** alpha RGBA (what egui's
    /// `ColorImage::from_rgba_premultiplied` and most GPU uploads want); this
    /// is the rasteriser's own format, so it is a plain copy.
    pub fn write_rgba_premultiplied(&self, out: &mut [u8]) {
        let stride = self.device.0 as usize * 4;
        for b in &self.bands {
            let off = b.y0 as usize * stride;
            let src = b.pm.data();
            out[off..off + src.len()].copy_from_slice(src);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display_list::DisplayListBuilder;
    use crate::resources::NoResources;

    fn run(surface: &mut Surface, f: impl FnOnce(&mut DisplayListBuilder)) -> Outcome {
        let mut b = DisplayListBuilder::new();
        f(&mut b);
        surface.execute(&b.take().unwrap(), &NoResources).unwrap()
    }

    #[test]
    fn a_filled_rectangle_lands_where_it_should() {
        let mut s = Surface::new(&SurfaceConfig::new(20, 10), &Limits::default()).unwrap();
        run(&mut s, |b| {
            b.set_fill_color(Rgba::rgb(255, 0, 0)).fill_rect(2.0, 3.0, 5.0, 4.0);
        });
        let img = s.to_image();
        assert_eq!(img.get(2, 3), Some(Rgba::rgb(255, 0, 0)));
        assert_eq!(img.get(6, 6), Some(Rgba::rgb(255, 0, 0)));
        assert_eq!(img.get(7, 6), Some(Rgba::TRANSPARENT));
        assert_eq!(img.get(1, 3), Some(Rgba::TRANSPARENT));
        assert_eq!(img.get(2, 7), Some(Rgba::TRANSPARENT));
    }

    #[test]
    fn a_damaged_list_changes_nothing() {
        let mut s = Surface::new(&SurfaceConfig::new(8, 8), &Limits::default()).unwrap();
        let mut b = DisplayListBuilder::new();
        b.set_fill_color(Rgba::WHITE).fill_rect(0.0, 0.0, 8.0, 8.0);
        let mut bytes = b.take().unwrap();
        bytes.pop(); // truncate the last operand
        assert!(s.execute(&bytes, &NoResources).is_err());
        assert_eq!(s.to_image().get(3, 3), Some(Rgba::TRANSPARENT));
    }

    #[test]
    fn pixel_ratio_scales_the_drawing_and_the_backing_store() {
        let cfg = SurfaceConfig { pixel_ratio: 2.0, ..SurfaceConfig::new(10, 10) };
        let mut s = Surface::new(&cfg, &Limits::default()).unwrap();
        assert_eq!(s.device_size(), (20, 20));
        run(&mut s, |b| {
            b.set_fill_color(Rgba::BLACK).fill_rect(1.0, 1.0, 2.0, 2.0);
        });
        let img = s.to_image();
        assert_eq!(img.get(2, 2), Some(Rgba::BLACK));
        assert_eq!(img.get(5, 5), Some(Rgba::BLACK));
        assert_eq!(img.get(6, 6), Some(Rgba::TRANSPARENT));
    }

    #[test]
    fn oversized_canvas_is_refused() {
        let e = Surface::new(&SurfaceConfig::new(5000, 5000), &Limits::default()).unwrap_err();
        assert!(e.to_string().contains("max_canvas_pixels"), "{e}");
    }

    #[test]
    fn present_and_readbacks_come_back_in_order() {
        let mut s = Surface::new(&SurfaceConfig::new(10, 10), &Limits::default()).unwrap();
        let o = run(&mut s, |b| {
            b.set_fill_color(Rgba::rgb(10, 20, 30)).fill_rect(0.0, 0.0, 10.0, 10.0).read_pixel(7, 3.0, 4.0).read_pixel(8, 99.0, 99.0).present();
        });
        assert!(o.presented);
        assert_eq!(o.readbacks[0], ReadBack::Pixel { slot: 7, color: Some(Rgba::rgb(10, 20, 30)) });
        assert_eq!(o.readbacks[1], ReadBack::Pixel { slot: 8, color: None });
    }
}
