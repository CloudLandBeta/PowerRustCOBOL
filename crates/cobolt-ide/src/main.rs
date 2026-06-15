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

/// The IDE window / dock / taskbar icon. Uses the bundled PowerRustCOBOL samurai
/// icon as-is; users can override it by dropping an `app-icon.png` into the
/// PowerRustCOBOL config directory.
fn load_icon() -> egui::IconData {
    // Configurable override: <config-dir>/app-icon.png, when present and valid.
    if let Some(icon) = std::fs::read(crate::llm::base_dir().join("app-icon.png"))
        .ok()
        .and_then(|bytes| decode_icon(&bytes))
    {
        return icon;
    }
    // Default: the bundled PowerRustCOBOL icon.
    decode_icon(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/images/powerrustcobol-icon.png"
    )))
    .unwrap_or(egui::IconData { rgba: vec![0, 0, 0, 0], width: 1, height: 1 })
}

/// Decode image bytes into an `egui::IconData` (RGBA, resized to 256×256).
fn decode_icon(bytes: &[u8]) -> Option<egui::IconData> {
    let img = image::load_from_memory(bytes)
        .ok()?
        .resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData { rgba: img.into_raw(), width: w, height: h })
}
