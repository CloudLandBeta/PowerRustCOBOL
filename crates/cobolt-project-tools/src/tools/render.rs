// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **`render_form`** (spec 084 R30): a picture of a form, as Run Form draws
//! it, for a coding agent to look at what it built.
//!
//! The picture itself is made by `cobolt_forms::snapshot`, which needs the
//! render engine; this crate does not link it, so the host injects a
//! [`Renderer`] (the IDE and `rcrun` both do) — as `build` injects its builder.

use std::path::Path;
use std::sync::Arc;

use serde_json::{json, Value};

use crate::host::ProjectHost;
use crate::root::ProjectRoot;

/// How a form is to be pictured.
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    /// The project's default form theme (`[forms] theme`).
    pub theme_default: Option<String>,
    /// 1 = one pixel per designed point.
    pub scale: f32,
    /// The window to lay the form out in, `[width, height]` in points;
    /// `None` is its designed size.
    pub window: Option<[f32; 2]>,
}

/// Picture the form at `cfrm`, assets resolved against `project`, as
/// `picture` says: `(png, [width, height])` in pixels.
pub type Renderer = Arc<dyn Fn(&Path, &Path, &Picture) -> Result<(Vec<u8>, [usize; 2]), String> + Send + Sync>;

/// The project's default form theme: `[forms] theme` in the manifest.
fn theme_default(root: &ProjectRoot) -> Option<String> {
    let text = std::fs::read_to_string(root.manifest()).ok()?;
    let doc: toml_edit::DocumentMut = text.parse().ok()?;
    doc.get("forms")?
        .get("theme")?
        .as_str()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
}

/// Run `render_form`: the PNG and what the agent is told about it.
pub fn run(
    host: &impl ProjectHost,
    root: &ProjectRoot,
    renderer: Option<&Renderer>,
    rel: &str,
    scale: f32,
    window: Option<[f32; 2]>,
) -> Result<(Vec<u8>, Value), String> {
    let renderer = renderer.ok_or("render_form is not available in this server")?;
    if !rel.to_ascii_lowercase().ends_with(".cfrm") {
        return Err("render_form takes a form (.cfrm)".to_owned());
    }
    let abs = root.resolve(rel)?;
    if !abs.is_file() {
        return Err(format!("no form at {rel}"));
    }
    let picture = Picture { theme_default: theme_default(root), scale, window };
    let (png, [w, h]) = renderer(&abs, root.dir(), &picture)?;
    let mut answer = json!({
        "form": rel,
        "width": w,
        "height": h,
        "scale": scale,
        "note": "As Run Form draws it when the form opens — before any event handler has run.",
    });
    if let Some([ww, wh]) = window {
        // The window the form was laid out in, in points: smaller than asked
        // when the form's minimum held it, as a running window is held.
        let (pw, ph) = ((w as f32 / scale).round(), (h as f32 / scale).round());
        answer["window"] = json!([pw, ph]);
        if pw > ww.round() || ph > wh.round() {
            answer["held_at_minimum"] =
                json!(format!("the form cannot lay out smaller than {pw}x{ph}: a window asked {ww}x{wh} opens at that"));
        }
    }
    if host.unsaved(&abs) {
        answer["unsaved"] = json!("the IDE holds unsaved edits to this form: the picture shows the saved file");
    }
    Ok((png, answer))
}
