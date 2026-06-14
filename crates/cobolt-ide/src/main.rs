// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! PowerRustCOBOL — entry point.
//!
//! Launches the egui/eframe window and hands control to `CoboltApp`.
//!
//! # Running
//!
//! ```sh
//! cargo run -p cobolt-ide
//! ```

mod app;
pub mod docs_embed;
pub mod file_dialog;
pub mod pdf_export;
pub mod fonts;
pub mod form_runtime;
pub mod i18n;
pub mod llm;
mod panels;
mod project_model;
mod runner;
pub mod theme;
pub mod version;
pub mod welcome;

use app::CoboltApp;
use version::VERSION;

fn main() -> eframe::Result<()> {
    // Initialise logging.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_env("COBOLT_LOG")
                .add_directive(tracing::Level::WARN.into()),
        )
        .with_target(false)
        .init();

    let ide_title = format!("PowerRustCOBOL  v{VERSION}");

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(&ide_title)
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0])
            .with_transparent(true)          // let desktop wallpaper bleed through
            .with_icon(load_icon()),
        ..Default::default()
    };

    eframe::run_native(
        &ide_title,
        native_options,
        Box::new(|cc| Ok(Box::new(CoboltApp::new(cc)))),
    )
}

/// Generate a 256×256 PowerRustCOBOL dock icon programmatically.
///
/// Visual: dark-navy rounded-rect background + a light-blue "C" arc (thick ring
/// with a gap on the right), plus two horizontal serifs at the gap ends.
fn load_icon() -> egui::IconData {
    const SZ: usize = 256;
    let mut px = vec![0u8; SZ * SZ * 4];

    let cx = SZ as f32 / 2.0;
    let cy = SZ as f32 / 2.0;

    // Rounded-square parameters
    let half   = SZ as f32 * 0.46;
    let bevel  = SZ as f32 * 0.18; // corner radius

    // "C" arc parameters
    let outer_r = SZ as f32 * 0.36;
    let inner_r = SZ as f32 * 0.22;
    let gap_deg = 42.0_f32;        // half-angle of the open gap (right side)
    let serif_h = SZ as f32 * 0.055; // vertical half-height of the terminal serifs
    let serif_w = SZ as f32 * 0.10;  // horizontal depth of the serifs

    for y in 0..SZ {
        for x in 0..SZ {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let r  = (dx * dx + dy * dy).sqrt();
            let i  = (y * SZ + x) * 4;

            // ── Rounded-square background ─────────────────────────────────
            let rx = dx.abs() - (half - bevel);
            let ry = dy.abs() - (half - bevel);
            let corner_dist = if rx > 0.0 && ry > 0.0 {
                (rx * rx + ry * ry).sqrt()
            } else {
                rx.max(ry).max(0.0)
            };
            let in_bg = corner_dist < bevel;
            if !in_bg { continue; }

            // Dark navy background
            px[i]   = 14;
            px[i+1] = 18;
            px[i+2] = 48;
            px[i+3] = 255;

            // Subtle inner highlight (top-left quadrant)
            if dx < 0.0 && dy < 0.0 && r < half * 0.85 {
                let blend = (1.0 - r / (half * 0.85)).powf(2.0) * 0.18;
                px[i]   = (px[i]   as f32 + blend * 255.0) as u8;
                px[i+1] = (px[i+1] as f32 + blend * 255.0) as u8;
                px[i+2] = (px[i+2] as f32 + blend * 255.0) as u8;
            }

            // ── "C" arc body ──────────────────────────────────────────────
            let angle = dy.atan2(dx).to_degrees(); // −180 … +180
            let in_ring = r >= inner_r && r <= outer_r;
            let in_gap  = angle.abs() < gap_deg;

            // Anti-aliased ring edge
            let edge_aa = {
                let d_out = (r - outer_r).abs();
                let d_in  = (r - inner_r).abs();
                let edge  = d_out.min(d_in);
                (1.0 - (edge - 0.0).max(0.0)).clamp(0.0, 1.0)
            };

            if in_ring && !in_gap {
                let t = ((r - inner_r) / (outer_r - inner_r)).clamp(0.0, 1.0);
                let blue_r = (60.0  + t * 50.0) as u8;
                let blue_g = (130.0 + t * 25.0) as u8;
                let blue_b = 255u8;
                let a = (edge_aa * 255.0) as u8;
                blend_pixel(&mut px[i..i+4], blue_r, blue_g, blue_b, a);
            }

            // ── Terminal serifs (top and bottom of the gap) ───────────────
            // Top serif: centred at angle = +gap_deg on the ring midpoint
            let mid_r  = (inner_r + outer_r) / 2.0;
            let gap_rad = gap_deg.to_radians();
            let serif_cy_top = cy + gap_rad.sin() * mid_r;
            let serif_cy_bot = cy - gap_rad.sin() * mid_r;
            let serif_cx     = cx + gap_rad.cos() * mid_r;

            // Top serif box
            if (dy - (serif_cy_top - cy)).abs() < serif_h
                && (dx - (serif_cx - cx)).abs() < outer_r * 0.5
                && dx > (serif_cx - cx) - serif_w
            {
                blend_pixel(&mut px[i..i+4], 90, 155, 255, 230);
            }
            // Bottom serif box
            if (dy - (serif_cy_bot - cy)).abs() < serif_h
                && (dx - (serif_cx - cx)).abs() < outer_r * 0.5
                && dx > (serif_cx - cx) - serif_w
            {
                blend_pixel(&mut px[i..i+4], 90, 155, 255, 230);
            }
        }
    }

    egui::IconData { rgba: px, width: SZ as u32, height: SZ as u32 }
}

/// Alpha-blend a colour onto a pixel (straight-alpha over dark background).
#[inline]
fn blend_pixel(dst: &mut [u8], r: u8, g: u8, b: u8, a: u8) {
    let af = a as f32 / 255.0;
    let bf = 1.0 - af;
    dst[0] = (r as f32 * af + dst[0] as f32 * bf) as u8;
    dst[1] = (g as f32 * af + dst[1] as f32 * bf) as u8;
    dst[2] = (b as f32 * af + dst[2] as f32 * bf) as u8;
    dst[3] = 255;
}
