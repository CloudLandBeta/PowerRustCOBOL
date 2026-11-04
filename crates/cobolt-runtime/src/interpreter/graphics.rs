// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Canvas2D, Canvas3D and the five graphics objects in the interpreter (WASM-018 T17, R2/R6/R7/R15/R16).
//!
//! One [`cobolt_graphics::Registry`] per running application. A control or object on the form owns one
//! handle, made the first time a method is called on it (a canvas takes its size from the control's
//! `Width` and `Height`, and `PixelRatio`). Methods are routed **by the object's class**, before the
//! general dispatch, straight to [`Registry::invoke`]: the drawing methods record into the canvas's
//! display list and the library executes it in one call on `Present`, `Flush` and the reads
//! (`ToImage`, `GetPixel`, `MeasureText`, `IsPointInPath`). A handle a method makes (a series, a node,
//! a body) comes back as a whole number. A refused call leaves the objects as they were and writes the
//! reason to the control's `LastError`.

use std::collections::HashMap;

use cobolt_graphics::{Limits, ObjectKind, Registry, Value};

use super::Interpreter;
use crate::value::CobolValue;

/// The owner every object of this interpreter carries: one interpreter is one form instance.
const OWNER: u32 = 1;

static PROJECT_LIMITS: std::sync::OnceLock<Limits> = std::sync::OnceLock::new();

/// Publish the project's graphics limits (R15), once per process; a later call cannot replace them.
/// A host that read the project's settings calls this before the first form runs
/// (`Limits::from_settings`); a process that never does keeps `Limits::default()`.
pub fn publish_graphics_limits(limits: Limits) {
    let _ = PROJECT_LIMITS.set(limits);
}

/// The limits every graphics registry of this process starts with.
pub fn project_limits() -> Limits {
    PROJECT_LIMITS.get().cloned().unwrap_or_default()
}

/// What the interpreter keeps for the graphics objects of one running form.
pub(crate) struct GraphicsState {
    pub registry: Registry,
    /// Control id (upper-cased) -> its handle.
    pub handles: HashMap<String, cobolt_graphics::handles::Handle>,
    /// The canvas whose `onFrame` handler is running; `AutoPresent` presents it when the handler returns.
    pub frame_canvas: Option<String>,
    /// This form's slot number in `canvas_frames`.
    pub instance: u64,
    /// Pictures published so far (the generation of the next one).
    pub frames: u64,
    /// The generation of the window's device the registry uses (0: none yet).
    pub gpu_gen: u64,
}

impl GraphicsState {
    pub(crate) fn with_limits(limits: Limits) -> GraphicsState {
        GraphicsState { registry: Registry::new(limits), handles: HashMap::new(), frame_canvas: None, instance: crate::canvas_frames::next_instance(), frames: 0, gpu_gen: 0 }
    }

    pub(crate) fn new() -> GraphicsState {
        GraphicsState { registry: Registry::new(project_limits()), handles: HashMap::new(), frame_canvas: None, instance: crate::canvas_frames::next_instance(), frames: 0, gpu_gen: 0 }
    }
}

/// A COBOL argument as the library's value: a number stays a number, anything else is text.
fn to_value(v: &CobolValue) -> Value {
    let text = v.as_display_string();
    let t = text.trim();
    if v.is_numeric() || t.parse::<f64>().is_ok() {
        if let Ok(n) = t.parse::<i64>() {
            return Value::Int(n);
        }
        if let Ok(f) = t.parse::<f64>() {
            return Value::Num(f);
        }
    }
    Value::Text(text)
}

/// The library's answer as the text a COBOL program receives.
fn to_text(v: &Value) -> String {
    match v {
        Value::Nothing | Value::Bytes(_) => String::new(),
        Value::Int(n) => n.to_string(),
        Value::Num(f) => {
            if f.fract() == 0.0 && f.abs() < 1e15 {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        Value::Text(s) => s.clone(),
        Value::Floats(xs) => xs.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" "),
        Value::Ints(xs) => xs.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" "),
    }
}

impl Interpreter {
    /// The graphics kind of a control or object, by its class.
    pub(crate) fn graphics_kind(&self, obj: &str) -> Option<ObjectKind> {
        self.object_class(obj.trim()).and_then(|c| ObjectKind::from_name(&c))
    }

    /// The handle of `obj`, made on first use.
    fn graphics_handle(&mut self, obj: &str, kind: ObjectKind) -> Result<cobolt_graphics::handles::Handle, String> {
        let key = obj.trim().to_ascii_uppercase();
        if let Some(h) = self.graphics.as_ref().and_then(|g| g.handles.get(&key)) {
            return Ok(*h);
        }
        let num = |me: &Self, p: &str, default: i64| me.obj_get(obj, p).trim().parse::<i64>().ok().filter(|n| *n > 0).unwrap_or(default);
        let args: Vec<Value> = match kind {
            ObjectKind::Canvas2D => {
                let ratio = self.obj_get(obj, "PixelRatio").trim().parse::<f64>().ok().filter(|r| *r > 0.0).unwrap_or(1.0);
                vec![Value::Int(num(self, "Width", 320)), Value::Int(num(self, "Height", 240)), Value::Num(ratio)]
            }
            ObjectKind::Canvas3D => vec![Value::Int(num(self, "Width", 320)), Value::Int(num(self, "Height", 240))],
            _ => Vec::new(),
        };
        let state = self.graphics.get_or_insert_with(GraphicsState::new);
        let h = state.registry.create(kind, OWNER, &args).map_err(|e| e.to_string())?;
        state.handles.insert(key, h);
        Ok(h)
    }

    /// Run `m` on a graphics control. `None` when the class is not one of the seven.
    pub(crate) fn graphics_method(&mut self, obj: &str, m: &str, args: &[CobolValue]) -> Option<String> {
        let kind = self.graphics_kind(obj)?;
        let told = if kind == ObjectKind::Canvas3D { self.graphics_sync_gpu() } else { Vec::new() };
        let values: Vec<Value> = args.iter().map(to_value).collect();
        let outcome = self.graphics_handle(obj, kind).and_then(|h| {
            let state = self.graphics.as_mut().expect("made with the handle");
            state.registry.invoke(h, m, &values).map_err(|e| e.to_string())
        });
        match outcome {
            Ok(v) => {
                self.graphics_publish_frame(obj, kind, m);
                if !told.iter().any(|n| n.eq_ignore_ascii_case(obj.trim())) {
                    self.obj_set(obj, "LastError", String::new());
                }
                Some(to_text(&v))
            }
            Err(why) => {
                self.obj_set(obj, "LastError", format!("{m} refused: {why}"));
                Some(String::new())
            }
        }
    }

    /// A Canvas2D that has just been presented hands its picture to the host (R16): one slot per canvas,
    /// and a `_CanvasFrame` update that names it.
    fn graphics_publish_frame(&mut self, obj: &str, kind: ObjectKind, method: &str) {
        if kind == ObjectKind::Canvas3D {
            if method.eq_ignore_ascii_case("Render") {
                self.graphics_publish_3d(obj);
            }
            return;
        }
        if kind != ObjectKind::Canvas2D {
            return;
        }
        let key = obj.trim().to_ascii_uppercase();
        let Some(state) = self.graphics.as_mut() else { return };
        let Some(h) = state.handles.get(&key).copied() else { return };
        let picture = state
            .registry
            .with_canvas2d(h, |c, _| {
                if !c.take_presented() {
                    return Ok(None);
                }
                let (w, hgt) = c.device_size();
                let mut px = vec![0u8; (w as usize) * (hgt as usize) * 4];
                c.write_rgba_premultiplied(&mut px);
                Ok(Some((w, hgt, px)))
            })
            .ok()
            .flatten();
        let Some((width, height, rgba_premultiplied)) = picture else { return };
        state.frames += 1;
        let (instance, generation) = (state.instance, state.frames);
        crate::canvas_frames::publish(instance, obj, crate::canvas_frames::CanvasFrame { width, height, rgba_premultiplied, generation });
        let spelled = self.form_spelling(obj);
        if let Some(tx) = &self.state_tx {
            let _ = tx.send(crate::StateUpdate::new(spelled, crate::canvas_frames::FRAME_PROP, crate::canvas_frames::token(instance, generation)));
        }
    }

    /// Canvas3D draws on the window's own device (R16). When the host published one - or a new one, because
    /// the window's device was replaced - the registry takes it, and a Canvas3D that already had a renderer
    /// is connected again on the new device, with the reason in its `LastError`.
    fn graphics_sync_gpu(&mut self) -> Vec<String> {
        let Some((generation, gpu)) = crate::gpu_share::current_gpu() else { return Vec::new() };
        let state = self.graphics.get_or_insert_with(GraphicsState::new);
        if state.gpu_gen == generation {
            return Vec::new();
        }
        let replaced = state.gpu_gen != 0;
        state.gpu_gen = generation;
        state.registry.set_gpu(gpu);
        if !replaced {
            return Vec::new();
        }
        let views: Vec<(String, cobolt_graphics::handles::Handle)> = state
            .handles
            .iter()
            .filter(|(_, h)| state.registry.kind_of(**h).ok() == Some(ObjectKind::Canvas3D))
            .map(|(n, h)| (n.clone(), *h))
            .collect();
        let mut told = Vec::new();
        for (name, h) in views {
            if let Some(state) = self.graphics.as_mut() {
                let _ = state.registry.invoke(h, "Connect", &[]);
            }
            self.obj_set(&name, "LastError", "the graphics device was replaced: the 3D view was connected to the new one".to_string());
            told.push(name);
        }
        told
    }

    /// A Canvas3D that has just rendered hands its render target to the host: the texture itself is shared
    /// (nothing is copied or read back), with a `_Canvas3DFrame` update that names it. A message the canvas
    /// raised (no adapter, a lost device) goes to `LastError`.
    fn graphics_publish_3d(&mut self, obj: &str) {
        let key = obj.trim().to_ascii_uppercase();
        let Some(state) = self.graphics.as_mut() else { return };
        let Some(h) = state.handles.get(&key).copied() else { return };
        let seen = state
            .registry
            .with_canvas3d(h, |c, _| Ok((c.renderer().and_then(|r| r.color_texture().cloned()), c.size(), c.take_event())))
            .ok();
        let Some((texture, (width, height), event)) = seen else { return };
        if let Some(message) = event {
            self.obj_set(obj, "LastError", message);
        }
        let Some(texture) = texture else { return };
        let state = self.graphics.as_mut().expect("checked above");
        state.frames += 1;
        let (instance, generation) = (state.instance, state.frames);
        crate::gpu_share::publish_frame(instance, obj, crate::gpu_share::Canvas3dFrame { texture, width, height, generation });
        let spelled = self.form_spelling(obj);
        if let Some(tx) = &self.state_tx {
            let _ = tx.send(crate::StateUpdate::new(spelled, crate::gpu_share::FRAME_PROP, format!("{instance}:{generation}")));
        }
    }

    /// An event left the queue for a handler: remember a canvas's `onFrame`.
    pub(crate) fn graphics_note_event(&mut self, ctrl: &str, event: &str) {
        if event.eq_ignore_ascii_case("onFrame") && matches!(self.graphics_kind(ctrl), Some(ObjectKind::Canvas2D | ObjectKind::Canvas3D)) {
            let state = self.graphics.get_or_insert_with(GraphicsState::new);
            state.frame_canvas = Some(ctrl.trim().to_string());
        }
    }

    /// The handler of the last event has returned: a canvas whose `onFrame` it was, with `AutoPresent` on
    /// (the default), shows what the handler drew (R6).
    pub(crate) fn graphics_end_of_frame(&mut self) {
        let Some(ctrl) = self.graphics.as_mut().and_then(|g| g.frame_canvas.take()) else { return };
        if self.graphics_kind(&ctrl) == Some(ObjectKind::Canvas3D) {
            return; // a 3D view draws when the program says Render
        }
        let auto = self.obj_get(&ctrl, "AutoPresent");
        let off = matches!(auto.trim().to_ascii_lowercase().as_str(), "0" | "false" | "no" | "off");
        if !off {
            let _ = self.graphics_method(&ctrl, "Present", &[]);
        }
    }

    /// The form closed: release every object it made. Returns how many.
    pub(crate) fn release_graphics(&mut self) -> usize {
        match self.graphics.as_mut() {
            Some(g) => {
                g.handles.clear();
                crate::canvas_frames::forget(g.instance);
                crate::gpu_share::forget_frames(g.instance);
                g.registry.release_owner(OWNER)
            }
            None => 0,
        }
    }

    /// Live graphics objects (for tests and the limits report).
    pub fn graphics_object_count(&self) -> usize {
        self.graphics.as_ref().map_or(0, |g| g.registry.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_lexer::{tokenize, SourceFormat};
    use cobolt_parser::parse;
    use std::time::Instant;

    fn interp(objects: &[(&str, &str, &[(&str, &str)])]) -> Interpreter {
        let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. GFX-T17.\nPROCEDURE DIVISION.\nMAIN.\n    STOP RUN.\n";
        let parsed = parse(tokenize(src, SourceFormat::Free));
        let mut i = Interpreter::new(parsed.program.expect("program should parse"));
        i.seed_objects(objects.iter().map(|(id, class, props)| {
            (id.to_string(), class.to_string(), props.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<Vec<_>>())
        }));
        i
    }

    fn call(i: &mut Interpreter, obj: &str, m: &str, args: &[&str]) -> String {
        let a: Vec<CobolValue> = args.iter().map(|s| CobolValue::from_str(s, s.len())).collect();
        i.exec_method(obj, &m.to_ascii_uppercase(), &a).as_display_string().trim().to_string()
    }

    /// Every one of the seven kinds is created on first use, answers a method, and goes with the form.
    #[test]
    fn each_of_the_seven_kinds_is_made_called_and_released_with_the_form() {
        let kinds: [(&str, &str, &str, &[&str]); 7] = [
            ("CANVAS-1", "Canvas2D", "FillRect", &["1", "1", "4", "4"]),
            ("IMAGE-1", "ImageBuffer", "GetWidth", &[]),
            ("PLOT-1", "Plot", "GetSeriesCount", &[]),
            ("PHYS2-1", "Physics2D", "GetWorldCount", &[]),
            ("PHYS3-1", "Physics3D", "GetWorldCount", &[]),
            ("SCENE-1", "Scene3D", "GetNodeCount", &[]),
            ("CANVAS3-1", "Canvas3D", "GetBackend", &[]),
        ];
        let objs: Vec<(&str, &str, &[(&str, &str)])> = kinds.iter().map(|(id, c, _, _)| (*id, *c, &[("Width", "64"), ("Height", "48")][..])).collect();
        let mut i = interp(&objs);
        let before = i.graphics_object_count();
        let mut made = 0;
        for (id, class, m, args) in kinds {
            let _ = call(&mut i, id, m, args);
            let err = i.obj_get(id, "LastError");
            // Canvas3D may have no graphics adapter on this machine: that is a refusal with a reason, not a crash.
            println!("{class}: {m} -> LastError {err:?}");
            if class != "Canvas3D" {
                assert_eq!(err, "", "{class}.{m}");
            }
            if i.graphics_object_count() > made {
                made += 1;
            }
        }
        let live = i.graphics_object_count();
        let released = i.release_graphics();
        let after = i.graphics_object_count();
        println!("T17: objects before {before}, live after the calls {live}, released with the form {released}, after {after}");
        assert_eq!(before, 0);
        assert!(live >= 6, "at least the six that need no adapter were made, got {live}");
        assert_eq!(released, live);
        assert_eq!(after, 0);
    }

    /// 10,000 shapes then one `Present`: the library ran the list once.
    #[test]
    fn ten_thousand_shapes_then_present_execute_one_list() {
        let mut i = interp(&[("CANVAS-1", "Canvas2D", &[("Width", "200"), ("Height", "100")])]);
        let t = Instant::now();
        for n in 0..10_000 {
            let x = (n % 190).to_string();
            let y = (n % 90).to_string();
            call(&mut i, "CANVAS-1", "FillRect", &[&x, &y, "4", "4"]);
        }
        let recorded = t.elapsed();
        let h = *i.graphics.as_ref().unwrap().handles.get("CANVAS-1").unwrap();
        let pending = i.graphics.as_mut().unwrap().registry.with_canvas2d(h, |c, _| Ok(c.pending_commands())).unwrap();
        let calls_before = i.graphics.as_mut().unwrap().registry.with_canvas2d(h, |c, _| Ok(c.host_calls())).unwrap();
        let t = Instant::now();
        call(&mut i, "CANVAS-1", "Present", &[]);
        let present = t.elapsed();
        let calls_after = i.graphics.as_mut().unwrap().registry.with_canvas2d(h, |c, _| Ok(c.host_calls())).unwrap();
        println!(
            "T17: 10000 FillRect recorded in {:.1} ms ({} pending), Present {:.1} ms, executions {}",
            recorded.as_secs_f64() * 1e3,
            pending,
            present.as_secs_f64() * 1e3,
            calls_after - calls_before
        );
        assert!(pending >= 10_000);
        assert_eq!(calls_after - calls_before, 1, "one list, one execution");
        assert_eq!(i.obj_get("CANVAS-1", "LastError"), "");
    }

    /// The drawn pixels come back: `GetPixel` after `Present`.
    #[test]
    fn get_pixel_returns_what_was_drawn() {
        let mut i = interp(&[("CANVAS-1", "Canvas2D", &[("Width", "100"), ("Height", "50")])]);
        call(&mut i, "CANVAS-1", "SetFillColor", &["#2060C0"]);
        call(&mut i, "CANVAS-1", "FillRect", &["10", "10", "40", "20"]);
        let p = call(&mut i, "CANVAS-1", "GetPixel", &["20", "20"]);
        println!("T17: GetPixel(20 20) = {p} (expected {})", 0x2060C0FFu32);
        assert_eq!(p, 0x2060C0FFu32.to_string());
        assert_eq!(i.obj_get("CANVAS-1", "LastError"), "");
    }

    /// A canvas past the limit is refused with a readable reason, and nothing is made.
    #[test]
    fn an_oversized_canvas_is_refused_with_a_readable_error() {
        let mut i = interp(&[("BIG-1", "Canvas2D", &[("Width", "100000"), ("Height", "100000")])]);
        call(&mut i, "BIG-1", "FillRect", &["0", "0", "1", "1"]);
        let err = i.obj_get("BIG-1", "LastError");
        println!("T17: oversized canvas -> {err:?}, live objects {}", i.graphics_object_count());
        assert!(err.contains("refused") && err.len() > 20, "{err}");
        assert_eq!(i.graphics_object_count(), 0);
    }

    /// A thing an object makes is a whole-number handle that the object's other methods take.
    #[test]
    fn a_made_thing_comes_back_as_a_handle_the_next_call_takes() {
        let mut i = interp(&[("PHYS2-1", "Physics2D", &[])]);
        let world = call(&mut i, "PHYS2-1", "CreateWorld", &["0", "-9.81"]);
        let bodies = call(&mut i, "PHYS2-1", "GetBodyCount", &[&world]);
        println!("T17: CreateWorld -> handle {world}, GetBodyCount({world}) = {bodies}");
        assert!(world.parse::<i64>().is_ok(), "{world}");
        assert_eq!(bodies, "0");
        assert_eq!(i.obj_get("PHYS2-1", "LastError"), "");
    }

    /// R6: with `AutoPresent` on (the default) the end of an `onFrame` handler presents the canvas; off, it does not.
    #[test]
    fn auto_present_runs_the_list_when_the_frame_handler_returns() {
        for (auto, expect) in [("True", 1u64), ("False", 0u64)] {
            let mut i = interp(&[("CANVAS-1", "Canvas2D", &[("Width", "50"), ("Height", "50"), ("AutoPresent", auto)])]);
            i.graphics_note_event("CANVAS-1", "onFrame");
            call(&mut i, "CANVAS-1", "FillRect", &["0", "0", "10", "10"]);
            let h = *i.graphics.as_ref().unwrap().handles.get("CANVAS-1").unwrap();
            let before = i.graphics.as_mut().unwrap().registry.with_canvas2d(h, |c, _| Ok(c.host_calls())).unwrap();
            i.graphics_end_of_frame();
            let after = i.graphics.as_mut().unwrap().registry.with_canvas2d(h, |c, _| Ok(c.host_calls())).unwrap();
            println!("T17: AutoPresent={auto}: executions at the end of onFrame {}", after - before);
            assert_eq!(after - before, expect);
        }
        // an event that is not a canvas's onFrame presents nothing
        let mut i = interp(&[("BTN-1", "Button", &[])]);
        i.graphics_note_event("BTN-1", "onFrame");
        assert!(i.graphics.is_none());
    }

    /// R15: the registry starts with the project's limits.
    #[test]
    fn the_projects_limits_apply_to_the_registry() {
        let mut i = interp(&[("CANVAS-1", "Canvas2D", &[("Width", "300"), ("Height", "100")])]);
        i.graphics = Some(GraphicsState::with_limits(Limits::from_settings(&[("max_canvas_width".into(), 200)]).unwrap()));
        call(&mut i, "CANVAS-1", "FillRect", &["0", "0", "1", "1"]);
        let err = i.obj_get("CANVAS-1", "LastError");
        println!("T17: project limit max_canvas_width=200 -> {err:?}");
        assert!(err.contains("max_canvas_width") && err.contains("200"), "{err}");
        assert_eq!(i.graphics_object_count(), 0);
    }

    /// `ToImage` after drawing: the handle's pixels are the drawn ones.
    #[test]
    fn to_image_after_drawing_returns_the_drawn_pixels() {
        let mut i = interp(&[("CANVAS-1", "Canvas2D", &[("Width", "40"), ("Height", "40")])]);
        call(&mut i, "CANVAS-1", "SetFillColor", &["#FF0000"]);
        call(&mut i, "CANVAS-1", "FillRect", &["0", "0", "20", "40"]);
        let img: u32 = call(&mut i, "CANVAS-1", "ToImage", &[]).parse().expect("a handle");
        assert_eq!(i.obj_get("CANVAS-1", "LastError"), "");
        let g = i.graphics.as_mut().unwrap();
        let inside = g.registry.invoke(img, "GetPixel", &[5.into(), 5.into()]).unwrap();
        let outside = g.registry.invoke(img, "GetPixel", &[30.into(), 5.into()]).unwrap();
        println!("T17: ToImage handle {img}: pixel (5 5) = {inside:?}, (30 5) = {outside:?}");
        assert_eq!(inside, Value::Int(0xFF0000FFu32 as i64));
        assert_eq!(outside, Value::Int(0));
    }

    /// R16: a `Present` leaves the picture for the host and sends one `_CanvasFrame` update; a drawing method alone sends none.
    #[test]
    fn a_present_publishes_the_picture_and_tells_the_host() {
        let mut i = interp(&[("CANVAS-1", "Canvas2D", &[("Width", "20"), ("Height", "10")])]);
        let (tx, rx) = std::sync::mpsc::channel();
        i.state_tx = Some(tx);
        call(&mut i, "CANVAS-1", "SetFillColor", &["#FF0000"]);
        call(&mut i, "CANVAS-1", "FillRect", &["0", "0", "20", "10"]);
        let before: Vec<_> = rx.try_iter().filter(|u| u.prop == crate::canvas_frames::FRAME_PROP).collect();
        call(&mut i, "CANVAS-1", "Present", &[]);
        let ups: Vec<_> = rx.try_iter().filter(|u| u.prop == crate::canvas_frames::FRAME_PROP).collect();
        println!("T18: updates before Present {}, after {}", before.len(), ups.len());
        assert_eq!((before.len(), ups.len()), (0, 1));
        let f = crate::canvas_frames::take("canvas-1", &ups[0].value).expect("the picture");
        println!("T18: published {}x{} picture, first pixel {:?}", f.width, f.height, &f.rgba_premultiplied[..4]);
        assert_eq!((f.width, f.height), (20, 10));
        assert_eq!(&f.rgba_premultiplied[..4], &[255, 0, 0, 255]);
        i.release_graphics();
        assert!(crate::canvas_frames::take("canvas-1", &ups[0].value).is_none(), "the form's end frees the slot");
    }

    /// R16 (Canvas3D): a Canvas3D draws on the device the window published; a `Render` shares its target with
    /// the host (one `_Canvas3DFrame` update, no read-back); a device published again (the window's was
    /// replaced) is picked up, the view is connected to it and says so in `LastError`. SKIPS without an adapter.
    #[test]
    fn a_canvas3d_draws_on_the_published_device_and_follows_a_replaced_one() {
        let Ok(first) = cobolt_graphics::scene3d::Gpu::new() else {
            assert!(std::env::var("GRAPHICS_REQUIRE_GPU").map(|v| v != "1").unwrap_or(true), "GRAPHICS_REQUIRE_GPU=1 but there is no graphics adapter");
            eprintln!("SKIPPED (no graphics adapter)");
            return;
        };
        let started = Instant::now();
        let mut i = interp(&[("VIEW-1", "Canvas3D", &[("Width", "64"), ("Height", "48")]), ("SCENE-1", "Scene3D", &[])]);
        let (tx, rx) = std::sync::mpsc::channel();
        i.state_tx = Some(tx);
        crate::gpu_share::publish_gpu(first.clone());
        let ball = call(&mut i, "SCENE-1", "AddSphere", &["1.5"]);
        call(&mut i, "SCENE-1", "SetAmbient", &["1", "1", "1", "1"]);
        let scene = i.graphics.as_ref().unwrap().handles["SCENE-1"];
        call(&mut i, "VIEW-1", "SetScene", &[&scene.to_string()]);
        let backend = call(&mut i, "VIEW-1", "Connect", &[]);
        let draws = call(&mut i, "VIEW-1", "Render", &[]);
        let frames: Vec<_> = rx.try_iter().filter(|u| u.prop == crate::gpu_share::FRAME_PROP).collect();
        let err_after_render = i.obj_get("VIEW-1", "LastError");
        println!("T18 Canvas3D: sphere node {ball}, backend {backend}, Render drew {draws} call(s), {} frame update(s), LastError {err_after_render:?}", frames.len());
        assert_ne!(backend, "None");
        assert_eq!(frames.len(), 1, "one Render, one update");
        let frame = crate::gpu_share::take_frame("view-1", &frames[0].value).expect("the shared target");
        assert_eq!((frame.width, frame.height), (64, 48));
        assert!(err_after_render.is_empty(), "{err_after_render}");

        // The window's device is replaced: the next call connects the view to the new one and reports it.
        crate::gpu_share::publish_gpu(first);
        call(&mut i, "VIEW-1", "GetBackend", &[]);
        let err = i.obj_get("VIEW-1", "LastError");
        println!("T18 Canvas3D: after a replaced device LastError = {err:?}; {} ms", started.elapsed().as_millis());
        assert!(err.contains("replaced"), "{err}");
        assert_eq!(call(&mut i, "VIEW-1", "GetBackend", &[]), backend);
        i.release_graphics();
        assert!(crate::gpu_share::take_frame("view-1", &frames[0].value).is_none());
        crate::gpu_share::withdraw_gpu();
    }
}
