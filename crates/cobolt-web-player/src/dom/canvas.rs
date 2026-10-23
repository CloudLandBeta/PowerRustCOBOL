// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Canvas 2D in the page** (WASM-018 R7, R16, WASM-016 R13): the browser half.
//!
//! The program's drawing methods reach the player as ordinary method calls on the control (`methods.rs`
//! turns each into a `canvas-call` command, in the order made). The drawing itself is `cobolt-graphics`'
//! `Canvas2D`, which lives in the **Canvas part** — a module of its own the loader fetches the first time a
//! canvas is drawn on, so a form with no canvas never pays for it. This file feeds the part: it keeps the calls
//! of a control until the part has loaded, replays them in order, and when the part says a `Present` has run,
//! puts the picture on the control's `<canvas>` with `putImageData`.

use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use js_sys::{Array, Function, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::{Clamped, JsCast};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData};

use super::Shared;
use crate::api::Inner;

/// The most calls kept for a canvas whose part has not loaded yet: a program that draws in a loop at
/// start-up cannot fill the page's memory while the part is being fetched.
const MAX_PENDING: usize = 200_000;

/// What the page holds for one canvas.
#[derive(Default)]
pub(crate) struct CanvasRec {
    /// The part's `Surface2D` object, once the part has loaded.
    obj: Option<JsValue>,
    /// Calls made, in order, not yet run.
    pending: VecDeque<Vec<String>>,
    loading: bool,
    /// The part could not load or build the surface: the reason, shown once, and the calls are dropped.
    failed: Option<String>,
}

pub(crate) type CanvasMap = HashMap<(u32, usize), CanvasRec>;

fn message(e: &JsValue) -> String {
    e.as_string().or_else(|| Reflect::get(e, &"message".into()).ok().and_then(|m| m.as_string())).unwrap_or_else(|| format!("{e:?}"))
}

fn method(obj: &JsValue, name: &str) -> Option<Function> {
    Reflect::get(obj, &name.into()).ok().and_then(|f| f.dyn_into::<Function>().ok())
}

impl Shared {
    /// `args[0]` is the method, the rest its arguments.
    pub(crate) fn canvas_call(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, ctl: usize, args: &[String]) {
        if args.is_empty() {
            return;
        }
        {
            let mut map = self.canvas.borrow_mut();
            let rec = map.entry((inst, ctl)).or_default();
            if rec.failed.is_some() {
                return;
            }
            if rec.pending.len() < MAX_PENDING {
                rec.pending.push_back(args.to_vec());
            }
        }
        self.canvas_pump(inner, inst, ctl);
    }

    /// Make sure the part is loading, or run what is waiting if it has loaded.
    fn canvas_pump(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, ctl: usize) {
        let key = (inst, ctl);
        let have_obj = self.canvas.borrow().get(&key).is_some_and(|r| r.obj.is_some());
        if have_obj {
            self.canvas_run(inst, ctl);
            return;
        }
        {
            let mut map = self.canvas.borrow_mut();
            let Some(rec) = map.get_mut(&key) else { return };
            if rec.loading || rec.failed.is_some() {
                return;
            }
            rec.loading = true;
        }
        let (w, h) = inner
            .host
            .try_borrow()
            .ok()
            .and_then(|host| host.trees.get(&inst).map(|t| (t.get(ctl, "Width").trim().parse::<f64>().unwrap_or(300.0), t.get(ctl, "Height").trim().parse::<f64>().unwrap_or(150.0))))
            .unwrap_or((300.0, 150.0));
        let weak = Rc::downgrade(self);
        let inner = inner.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let built: Result<JsValue, String> = async {
                let module = inner.part_module("canvas").await?;
                let class = Reflect::get(&module, &"Surface2D".into()).ok().and_then(|c| c.dyn_into::<Function>().ok()).ok_or("the canvas part has no Surface2D")?;
                let args = Array::of3(&w.max(1.0).into(), &h.max(1.0).into(), &1.0.into());
                Reflect::construct(&class, &args).map_err(|e| message(&e))
            }
            .await;
            let Some(sh) = weak.upgrade() else { return };
            {
                let mut map = sh.canvas.borrow_mut();
                let Some(rec) = map.get_mut(&key) else {
                    // The instance closed while the part loaded: nothing to keep.
                    if let Ok(o) = &built {
                        if let Some(f) = method(o, "free") {
                            let _ = f.call0(o);
                        }
                    }
                    return;
                };
                rec.loading = false;
                match built {
                    Ok(obj) => rec.obj = Some(obj),
                    Err(e) => {
                        web_sys::console::error_1(&format!("[pr] the canvas could not be made: {e}").into());
                        rec.failed = Some(e);
                        rec.pending.clear();
                    }
                }
            }
            sh.canvas_run(key.0, key.1);
        });
    }

    /// Run the waiting calls in order, then paint when a `Present` ran.
    fn canvas_run(self: &Rc<Self>, inst: u32, ctl: usize) {
        let key = (inst, ctl);
        let (obj, calls) = {
            let mut map = self.canvas.borrow_mut();
            let Some(rec) = map.get_mut(&key) else { return };
            let Some(obj) = rec.obj.clone() else { return };
            (obj, std::mem::take(&mut rec.pending))
        };
        let Some(call) = method(&obj, "call") else { return };
        for c in calls {
            let args = serde_json::to_string(&c[1..]).unwrap_or_else(|_| "[]".into());
            if let Err(e) = call.call2(&obj, &c[0].as_str().into(), &args.as_str().into()) {
                // A refused call is the program's mistake, said once and not fatal: the rest still draws.
                web_sys::console::warn_1(&format!("[pr] Canvas2D {}: {}", c[0], message(&e)).into());
            }
        }
        let presented = method(&obj, "take_presented").and_then(|f| f.call0(&obj).ok()).and_then(|v| v.as_bool()).unwrap_or(false);
        if presented {
            self.canvas_paint(inst, ctl, &obj);
        }
    }

    /// Put the part's picture on the control's `<canvas>`.
    fn canvas_paint(&self, inst: u32, ctl: usize, obj: &JsValue) {
        let Some(el) = self.control_element(inst, ctl).and_then(|c| c.query_selector("canvas").ok().flatten()) else { return };
        let Ok(canvas) = el.dyn_into::<HtmlCanvasElement>() else { return };
        let dim = |name: &str| method(obj, name).and_then(|f| f.call0(obj).ok()).and_then(|v| v.as_f64()).unwrap_or(0.0) as u32;
        let (w, h) = (dim("device_width"), dim("device_height"));
        if w == 0 || h == 0 {
            return;
        }
        let Some(bytes) = method(obj, "rgba").and_then(|f| f.call0(obj).ok()).map(|v| js_sys::Uint8Array::new(&v).to_vec()) else { return };
        let Ok(image) = ImageData::new_with_u8_clamped_array_and_sh(Clamped(&bytes), w, h) else { return };
        let ctx = canvas.get_context("2d").ok().flatten().and_then(|c| c.dyn_into::<CanvasRenderingContext2d>().ok());
        if let Some(ctx) = ctx {
            let _ = ctx.put_image_data(&image, 0.0, 0.0);
        }
    }

    /// The page goes away (WASM-017 R14): every canvas's `Surface2D` is freed, whatever instance
    /// it belonged to.
    pub(crate) fn clear_all_canvases(&self) {
        let dropped: Vec<CanvasRec> = self.canvas.borrow_mut().drain().map(|(_, r)| r).collect();
        for r in dropped {
            if let Some(o) = r.obj {
                if let Some(f) = method(&o, "free") {
                    let _ = f.call0(&o);
                }
            }
        }
    }

    /// An instance went away: its canvases are freed with it.
    pub(crate) fn clear_canvas(&self, inst: u32) {
        let dropped: Vec<CanvasRec> = {
            let mut map = self.canvas.borrow_mut();
            let keys: Vec<(u32, usize)> = map.keys().filter(|(i, _)| *i == inst).copied().collect();
            keys.into_iter().filter_map(|k| map.remove(&k)).collect()
        };
        for r in dropped {
            if let Some(o) = r.obj {
                if let Some(f) = method(&o, "free") {
                    let _ = f.call0(&o);
                }
            }
        }
    }
}
