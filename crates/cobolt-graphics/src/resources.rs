// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What a canvas reads from the host while it executes a display list.
//!
//! The crate opens no file and makes no connection (WASM-018 R15): everything a
//! drawing refers to by number - an image, a font - is looked up through
//! [`Resources`], which the host implements over whatever it has (the
//! interpreter's registry, the player's tables). Assets by *name* come through
//! [`AssetHost`], which is the only way bytes enter or leave: the desktop reads
//! and writes files behind it, the web player hands bytes to the browser.

use std::collections::HashMap;
use std::sync::Arc;

use crate::pixels::PixelBuffer;
use crate::textapi::TextEngine;

/// Numbered resources a display list may refer to.
pub trait Resources: Sync {
    /// The image with this id, if there is one.
    fn image(&self, id: u32) -> Option<Arc<PixelBuffer>>;

    /// The text engine, if the host has fonts. Without one `DrawText` and
    /// `MeasureText` report a diagnostic and draw nothing.
    fn text_engine(&self) -> Option<&dyn TextEngine>;
}

/// Named assets in and bytes out: the host's side of `LoadAsset` and `Save`.
pub trait AssetHost {
    /// The bytes of the bundled asset `name` (a path inside the project's
    /// `Assets/`), or `None` if there is no such asset.
    fn asset(&self, name: &str) -> Option<Vec<u8>>;

    /// Deliver `bytes` under `name` (a file on the desktop, a download in the
    /// browser, a request to the backend). The host decides what "save" means.
    fn save(&self, name: &str, bytes: &[u8]) -> Result<(), String>;
}

/// A [`Resources`] made of a map, for tests and simple hosts.
#[derive(Default)]
pub struct MapResources {
    images: HashMap<u32, Arc<PixelBuffer>>,
    engine: Option<Arc<dyn TextEngine>>,
}

impl MapResources {
    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add (or replace) an image.
    pub fn with_image(mut self, id: u32, image: PixelBuffer) -> Self {
        self.images.insert(id, Arc::new(image));
        self
    }

    /// Insert an image in place.
    pub fn set_image(&mut self, id: u32, image: Arc<PixelBuffer>) {
        self.images.insert(id, image);
    }

    /// Use this text engine.
    pub fn with_text_engine(mut self, engine: Arc<dyn TextEngine>) -> Self {
        self.engine = Some(engine);
        self
    }
}

impl Resources for MapResources {
    fn image(&self, id: u32) -> Option<Arc<PixelBuffer>> {
        self.images.get(&id).cloned()
    }

    fn text_engine(&self) -> Option<&dyn TextEngine> {
        self.engine.as_deref()
    }
}

/// No resources at all.
pub struct NoResources;

impl Resources for NoResources {
    fn image(&self, _id: u32) -> Option<Arc<PixelBuffer>> {
        None
    }

    fn text_engine(&self) -> Option<&dyn TextEngine> {
        None
    }
}

/// An [`AssetHost`] over a map of names to bytes; `save` keeps what it is given.
#[derive(Default)]
pub struct MapAssets {
    assets: HashMap<String, Vec<u8>>,
    saved: std::sync::Mutex<Vec<(String, Vec<u8>)>>,
}

impl MapAssets {
    /// An empty host.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an asset.
    pub fn with_asset(mut self, name: &str, bytes: Vec<u8>) -> Self {
        self.assets.insert(name.to_string(), bytes);
        self
    }

    /// What `save` was given, in order.
    pub fn saved(&self) -> Vec<(String, Vec<u8>)> {
        self.saved.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

impl AssetHost for MapAssets {
    fn asset(&self, name: &str) -> Option<Vec<u8>> {
        self.assets.get(name).cloned()
    }

    fn save(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        self.saved.lock().map_err(|_| "poisoned".to_string())?.push((name.to_string(), bytes.to_vec()));
        Ok(())
    }
}
