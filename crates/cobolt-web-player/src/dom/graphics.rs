// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The graphics objects in the page** (WASM-018 R7, R16, R18; WASM-016 R13): the browser half.
//!
//! A form instance's graphics objects — Canvas2D, Canvas3D, ImageBuffer, Plot, Scene3D, Physics2D/3D — live in one
//! `Graphics` registry inside a **part** of the player: a module of its own that the loader fetches when a form
//! that uses it opens (`canvas`, `graphics` or `graphics3d`, the smallest that has every kind the form uses; a form with
//! no graphics object fetches none). This file feeds the part:
//!
//! * a drawing method of a Canvas2D arrives as a `canvas-call` command and a display list as a `canvas-draw`; both are kept
//!   per canvas, in the order made, until the part can run them, and when a `Present` has run the picture is put on the
//!   control's `<canvas>` with `putImageData`;
//! * every other method answers something the program reads next (a handle, a count, a pixel), so the host asks the
//!   part at once ([`Shared::gfx_sync`]): the pending drawing of the instance runs first, so the answer is of the
//!   picture at that point of the program, and whatever the call drew (a Plot into a canvas) is painted after it;
//! * a Canvas3D's `Render` is a draw on the GPU the part opened for the page; the picture is copied out of it and painted
//!   when the copy completes (the next turn of the page).

use std::collections::{HashMap, HashSet, VecDeque};
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

/// Something a canvas has been asked to do, kept in the order made until the part can run it.
pub(crate) enum Item {
    /// A drawing method by name with its arguments as texts (the first is the method).
    Call(Vec<String>),
    /// A display list the module encoded (ABI A23, a `DRAW`).
    List(Vec<u8>),
}

/// The most bytes of display lists kept for a canvas whose part has not loaded yet.
const MAX_PENDING_BYTES: usize = 64 * 1024 * 1024;

/// What the page holds for one Canvas2D.
#[derive(Default)]
pub(crate) struct CanvasRec {
    /// Calls and lists made, in order, not yet run.
    pending: VecDeque<Item>,
    /// The bytes of the lists in `pending`.
    pending_bytes: usize,
    /// The part could not load or build the surface: the reason, shown once, and the calls are dropped.
    failed: Option<String>,
}

/// Everything the page holds for graphics: one `Graphics` per form instance, and the state of each canvas.
#[derive(Default)]
pub(crate) struct Gfx {
    /// The registry of each instance (a `Graphics` object of the part).
    objs: HashMap<u32, JsValue>,
    /// The Canvas2D controls the page has drawn on or read.
    canvases: HashMap<(u32, usize), CanvasRec>,
    /// Instances whose part is being fetched (a page whose loader has not preloaded it).
    loading: HashSet<u32>,
    /// Controls whose objects the part has made.
    made: HashSet<(u32, usize)>,
}

fn message(e: &JsValue) -> String {
    e.as_string().or_else(|| Reflect::get(e, &"message".into()).ok().and_then(|m| m.as_string())).unwrap_or_else(|| format!("{e:?}"))
}

fn method(obj: &JsValue, name: &str) -> Option<Function> {
    Reflect::get(obj, &name.into()).ok().and_then(|f| f.dyn_into::<Function>().ok())
}

/// A `Graphics` object from the loaded part `module`, given the page's GPU when there is one.
fn build_graphics(module: &JsValue, gpu: Option<&JsValue>) -> Result<JsValue, String> {
    let class = Reflect::get(module, &"Graphics".into()).ok().and_then(|c| c.dyn_into::<Function>().ok()).ok_or("the graphics part has no Graphics")?;
    let g = Reflect::construct(&class, &Array::new()).map_err(|e| message(&e))?;
    if let (Some(dev), Some(set)) = (gpu, method(&g, "set_gpu")) {
        set.call1(&g, dev).map_err(|e| message(&e))?;
    }
    Ok(g)
}

fn free(g: &JsValue) {
    if let Some(f) = method(g, "free") {
        let _ = f.call0(g);
    }
}

impl Shared {
    /// The part's object for instance `inst`, made now from the part the page preloaded; `None` before it has loaded.
    fn graphics_of(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32) -> Option<JsValue> {
        if let Some(g) = self.canvas.borrow().objs.get(&inst) {
            return Some(g.clone());
        }
        let part = {
            let host = inner.host.try_borrow().ok()?;
            host.graphics_part(host.trees.get(&inst)?.form_idx)?
        };
        let module = inner.parts.borrow().get(part).cloned()?;
        let gpu = inner.gpu_device.borrow().clone();
        match build_graphics(&module, gpu.as_ref()) {
            Ok(g) => {
                // The project's fonts: every text this registry draws is lettered with them.
                let fonts = inner.fonts.borrow().clone();
                if let (Some(fonts), Some(add)) = (fonts, method(&g, "add_font")) {
                    for bytes in fonts.iter() {
                        if let Err(e) = add.call1(&g, &js_sys::Uint8Array::from(bytes.as_slice())) {
                            web_sys::console::warn_1(&format!("[pr] a project font was refused: {}", message(&e)).into());
                        }
                    }
                }
                self.canvas.borrow_mut().objs.insert(inst, g.clone());
                Some(g)
            }
            Err(e) => {
                web_sys::console::error_1(&format!("[pr] the graphics part could not be used: {e}").into());
                None
            }
        }
    }

    /// `args[0]` is the method, the rest its arguments.
    pub(crate) fn canvas_call(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, ctl: usize, args: &[String]) {
        if args.is_empty() {
            return;
        }
        {
            let mut map = self.canvas.borrow_mut();
            let rec = map.canvases.entry((inst, ctl)).or_default();
            if rec.failed.is_some() {
                return;
            }
            if rec.pending.len() < MAX_PENDING {
                rec.pending.push_back(Item::Call(args.to_vec()));
            }
        }
        self.canvas_pump(inner, inst, ctl);
    }

    /// A display list for the canvas: `args[0]` names it in the form's tree (ABI A23).
    pub(crate) fn canvas_draw(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, ctl: usize, args: &[String]) {
        let Some(seq) = args.first().and_then(|s| s.parse::<u64>().ok()) else { return };
        let list = inner.host.try_borrow_mut().ok().and_then(|mut h| h.trees.get_mut(&inst).and_then(|t| t.take_gfx(seq)));
        let Some(list) = list else { return };
        {
            let mut map = self.canvas.borrow_mut();
            let rec = map.canvases.entry((inst, ctl)).or_default();
            if rec.failed.is_some() {
                return;
            }
            if rec.pending.len() < MAX_PENDING && rec.pending_bytes + list.len() <= MAX_PENDING_BYTES {
                rec.pending_bytes += list.len();
                rec.pending.push_back(Item::List(list));
            }
        }
        self.canvas_pump(inner, inst, ctl);
    }

    /// Answer a method of a graphics object at once (ABI A23, WASM-018 T25/T26): the part makes the object on first use,
    /// the pending drawing of the instance runs first, and what the call drew is painted afterwards.
    pub(crate) fn gfx_sync(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, ctl: usize, kind: &str, size: (f64, f64), method_name: &str, args: &[String]) -> crate::host::GfxAnswer {
        use crate::host::GfxAnswer;
        let Some(g) = self.graphics_of(inner, inst) else { return GfxAnswer::NotLoaded };
        if kind == "Canvas2D" {
            self.canvas.borrow_mut().canvases.entry((inst, ctl)).or_default();
        }
        if let Err(why) = self.make_object(&g, inst, ctl, kind, size) {
            return GfxAnswer::Refused(why);
        }
        self.canvas_run_all(inner, inst);
        let Some(call) = method(&g, "call") else { return GfxAnswer::Refused("the graphics part has no call".into()) };
        let json = serde_json::to_string(args).unwrap_or_else(|_| "[]".into());
        let answer = match call.call3(&g, &(ctl as u32).into(), &method_name.into(), &json.as_str().into()) {
            Ok(v) => GfxAnswer::Answer(v.as_string().unwrap_or_default()),
            Err(e) => GfxAnswer::Refused(message(&e)),
        };
        if kind == "Canvas3D" && method_name.trim().eq_ignore_ascii_case("Render") && matches!(answer, GfxAnswer::Answer(_)) {
            self.picture3d(&g, inst, ctl);
        }
        self.paint_presented(inst, &g);
        answer
    }

    /// `Graphics.make` for the control, once.
    fn make_object(self: &Rc<Self>, g: &JsValue, inst: u32, ctl: usize, kind: &str, size: (f64, f64)) -> Result<(), String> {
        if self.canvas.borrow().made.contains(&(inst, ctl)) {
            return Ok(());
        }
        let make = method(g, "make").ok_or("the graphics part has no make")?;
        let args = Array::of5(&(ctl as u32).into(), &kind.into(), &(size.0.max(1.0) as u32).into(), &(size.1.max(1.0) as u32).into(), &1.0.into());
        make.apply(g, &args).map_err(|e| message(&e))?;
        self.canvas.borrow_mut().made.insert((inst, ctl));
        Ok(())
    }

    /// Make sure the part is loading (a page whose loader did not preload it), or run what is waiting if it has loaded.
    fn canvas_pump(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, ctl: usize) {
        if self.graphics_of(inner, inst).is_some() {
            self.canvas_run(inner, inst, ctl);
            return;
        }
        {
            let mut map = self.canvas.borrow_mut();
            if !map.loading.insert(inst) {
                return;
            }
        }
        let part = inner.host.try_borrow().ok().and_then(|h| h.trees.get(&inst).map(|t| t.form_idx).and_then(|f| h.graphics_part(f))).unwrap_or("canvas");
        let weak = Rc::downgrade(self);
        let inner = inner.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let loaded = inner.load_part(part).await;
            let Some(sh) = weak.upgrade() else { return };
            sh.canvas.borrow_mut().loading.remove(&inst);
            match loaded {
                Ok(()) => {
                    let ctls: Vec<usize> = sh.canvas.borrow().canvases.keys().filter(|(i, _)| *i == inst).map(|(_, c)| *c).collect();
                    for c in ctls {
                        sh.canvas_run(&inner, inst, c);
                    }
                }
                Err(e) => {
                    web_sys::console::error_1(&format!("[pr] the graphics part could not be loaded: {e}").into());
                    let mut map = sh.canvas.borrow_mut();
                    for ((i, _), rec) in map.canvases.iter_mut() {
                        if *i == inst {
                            rec.failed = Some(e.clone());
                            rec.pending.clear();
                        }
                    }
                }
            }
        });
    }

    /// Run what every canvas of the instance has waiting (before a method that answers at once).
    fn canvas_run_all(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32) {
        let ctls: Vec<usize> = self.canvas.borrow().canvases.iter().filter(|((i, _), r)| *i == inst && !r.pending.is_empty()).map(|((_, c), _)| *c).collect();
        for c in ctls {
            self.canvas_run(inner, inst, c);
        }
    }

    /// Run the waiting items of one canvas in order, then paint when a `Present` ran.
    fn canvas_run(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, ctl: usize) {
        let Some(g) = self.graphics_of(inner, inst) else { return };
        let size = inner
            .host
            .try_borrow()
            .ok()
            .and_then(|host| host.trees.get(&inst).map(|t| (t.get(ctl, "Width").trim().parse::<f64>().unwrap_or(300.0), t.get(ctl, "Height").trim().parse::<f64>().unwrap_or(150.0))))
            .unwrap_or((300.0, 150.0));
        if let Err(e) = self.make_object(&g, inst, ctl, "Canvas2D", size) {
            web_sys::console::error_1(&format!("[pr] the canvas could not be made: {e}").into());
            if let Some(rec) = self.canvas.borrow_mut().canvases.get_mut(&(inst, ctl)) {
                rec.failed = Some(e);
                rec.pending.clear();
            }
            return;
        }
        let items = {
            let mut map = self.canvas.borrow_mut();
            let Some(rec) = map.canvases.get_mut(&(inst, ctl)) else { return };
            rec.pending_bytes = 0;
            std::mem::take(&mut rec.pending)
        };
        let id: JsValue = (ctl as u32).into();
        let Some(call) = method(&g, "call") else { return };
        let draw = method(&g, "draw_list");
        for item in items {
            match item {
                Item::Call(c) => {
                    let args = serde_json::to_string(&c[1..]).unwrap_or_else(|_| "[]".into());
                    if let Err(e) = call.call3(&g, &id, &c[0].as_str().into(), &args.as_str().into()) {
                        // A refused call is the program's mistake, said once and not fatal: the rest still draws.
                        web_sys::console::warn_1(&format!("[pr] Canvas2D {}: {}", c[0], message(&e)).into());
                    }
                }
                Item::List(bytes) => {
                    if let Some(draw) = &draw {
                        let view = js_sys::Uint8Array::from(bytes.as_slice());
                        if let Err(e) = draw.call2(&g, &id, &view) {
                            web_sys::console::warn_1(&format!("[pr] Canvas2D display list: {}", message(&e)).into());
                        }
                    }
                }
            }
        }
        self.paint_if_presented(inst, ctl, &g);
    }

    /// Paint every canvas of the instance that a `Present` has run on (a Plot drawn into one, say).
    fn paint_presented(self: &Rc<Self>, inst: u32, g: &JsValue) {
        let ctls: Vec<usize> = self.canvas.borrow().canvases.keys().filter(|(i, _)| *i == inst).map(|(_, c)| *c).collect();
        for c in ctls {
            self.paint_if_presented(inst, c, g);
        }
    }

    fn paint_if_presented(&self, inst: u32, ctl: usize, g: &JsValue) {
        let id: JsValue = (ctl as u32).into();
        let presented = method(g, "take_presented").and_then(|f| f.call1(g, &id).ok()).and_then(|v| v.as_bool()).unwrap_or(false);
        if !presented {
            return;
        }
        let dim = |name: &str| method(g, name).and_then(|f| f.call1(g, &id).ok()).and_then(|v| v.as_f64()).unwrap_or(0.0) as u32;
        let (w, h) = (dim("device_width"), dim("device_height"));
        let Some(bytes) = method(g, "rgba").and_then(|f| f.call1(g, &id).ok()).map(|v| js_sys::Uint8Array::new(&v).to_vec()) else { return };
        self.put_pixels(inst, ctl, &bytes, w, h);
    }

    /// Put straight-alpha RGBA of `w` x `h` on the control's `<canvas>`.
    fn put_pixels(&self, inst: u32, ctl: usize, bytes: &[u8], w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        let Some(el) = self.control_element(inst, ctl).and_then(|c| c.query_selector("canvas").ok().flatten()) else { return };
        let Ok(canvas) = el.dyn_into::<HtmlCanvasElement>() else { return };
        let Ok(image) = ImageData::new_with_u8_clamped_array_and_sh(Clamped(bytes), w, h) else { return };
        let ctx = canvas.get_context("2d").ok().flatten().and_then(|c| c.dyn_into::<CanvasRenderingContext2d>().ok());
        if let Some(ctx) = ctx {
            let _ = ctx.put_image_data(&image, 0.0, 0.0);
        }
    }

    /// A Canvas3D has rendered: copy its picture out of the GPU and paint it when the copy completes.
    fn picture3d(self: &Rc<Self>, g: &JsValue, inst: u32, ctl: usize) {
        let Some(begin) = method(g, "picture3d") else { return };
        let pic = match begin.call1(g, &(ctl as u32).into()) {
            Ok(p) => p,
            Err(e) => {
                web_sys::console::warn_1(&format!("[pr] Canvas3D picture: {}", message(&e)).into());
                return;
            }
        };
        let dim = |name: &str| method(&pic, name).and_then(|f| f.call0(&pic).ok()).and_then(|v| v.as_f64()).unwrap_or(0.0) as u32;
        let (w, h) = (dim("width"), dim("height"));
        let weak = Rc::downgrade(self);
        wasm_bindgen_futures::spawn_local(async move {
            let Some(finish) = method(&pic, "finish") else { return };
            let Ok(promise) = finish.call0(&pic) else { return };
            match wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&promise)).await {
                Ok(v) => {
                    if let Some(sh) = weak.upgrade() {
                        sh.put_pixels(inst, ctl, &js_sys::Uint8Array::new(&v).to_vec(), w, h);
                    }
                }
                Err(e) => web_sys::console::warn_1(&format!("[pr] Canvas3D picture: {}", message(&e)).into()),
            }
        });
    }

    /// The page goes away (WASM-017 R14): every instance's registry is freed.
    pub(crate) fn clear_all_canvases(&self) {
        let all = std::mem::take(&mut *self.canvas.borrow_mut());
        for (_, g) in all.objs {
            free(&g);
        }
    }

    /// An instance went away: its registry, and every object in it, is freed with it.
    pub(crate) fn clear_canvas(&self, inst: u32) {
        let g = {
            let mut map = self.canvas.borrow_mut();
            map.canvases.retain(|(i, _), _| *i != inst);
            map.made.retain(|(i, _)| *i != inst);
            map.loading.remove(&inst);
            map.objs.remove(&inst)
        };
        if let Some(g) = g {
            free(&g);
        }
    }
}
