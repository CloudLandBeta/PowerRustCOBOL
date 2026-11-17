// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Graphics on the runner** (DESKTOP-WASM-001 T50; R11, R27).
//!
//! The seven graphics objects (Canvas2D, Canvas3D, ImageBuffer, Scene3D, Plot, Physics2D,
//! Physics3D) are drawn by the library the interpreter draws them with, `cobolt-graphics`, in a
//! registry made as the interpreter makes one for each form it runs
//! (`cobolt_runtime::interpreter::graphics::desktop_registry`): the project's limits and fonts, and
//! `LoadAsset` from the project's `Assets/` folder. So a program built on the WebAssembly path
//! draws the pictures a program built on the Cargo path draws.
//!
//! What reaches the registry is what reaches the browser's Canvas part:
//!
//! * a **drawing method** of a Canvas2D, and the **display lists** of ABI A23, are commands of the
//!   form's tree (`canvas-call`, `canvas-draw`); they run, in order, when the program thread hands
//!   the tree's changes to the windows;
//! * a **read-back** of a Canvas2D (`GetPixel`, `MeasureText`, `ToImage` ...) and **every method**
//!   of the other six answer at once, through the ABI host's graphics hook (`Host::gfx_sync`),
//!   after the drawing that was waiting.
//!
//! What the interpreter does after a call, this module does after it: an answer clears
//! `LastError`, a refusal writes `"<method> refused: <why>"` there; a Canvas2D that presented
//! publishes its picture (`canvas_frames`, a `_CanvasFrame` update), a Canvas3D that rendered its
//! render target (`gpu_share`, a `_Canvas3DFrame` update), and the window shows them as it shows the
//! interpreter's. A Canvas3D draws on the window's own device, and follows it when it is replaced.
//! A form's objects are released when its instance is gone.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::rc::Rc;

use cobolt_abi_host::host::{GfxAnswer, Host};
use cobolt_abi_host::kinds::{player_kind, PlayerKind};
use cobolt_abi_host::tree::{Change, GfxItem};
use cobolt_graphics::handles::Handle;
use cobolt_graphics::{ObjectKind, Registry};
use cobolt_runtime::interpreter::graphics::{answer_text, desktop_registry, values_for};
use cobolt_runtime::{canvas_frames, gpu_share, StateUpdate};

/// The owner of every object a form instance makes (one registry per instance).
const OWNER: u32 = 1;

/// The drawing kept for a canvas whose object cannot be made yet, at most (the page's bound).
const MAX_PENDING: usize = 100_000;

/// The graphics objects of one form instance.
struct Session {
    registry: Registry,
    /// Control index -> its object and kind.
    handles: HashMap<usize, (Handle, ObjectKind)>,
    /// Drawing for a canvas whose object is not made yet (only a read-back of another canvas runs
    /// before the window's pump has seen it): it runs, first, when the object is made.
    pending: HashMap<usize, VecDeque<GfxItem>>,
    /// This instance's slot in `canvas_frames` and `gpu_share`.
    slot: u64,
    /// Pictures published so far (the generation of the next one).
    frames: u64,
    /// The generation of the window's device the registry draws on (0: none yet).
    gpu_gen: u64,
}

impl Session {
    fn new() -> Session {
        Session { registry: desktop_registry(), handles: HashMap::new(), pending: HashMap::new(), slot: canvas_frames::next_instance(), frames: 0, gpu_gen: 0 }
    }

    /// The object of control `ctl`, made on first use with the interpreter's arguments: a Canvas2D
    /// `Width`, `Height` (320 x 240 when not above zero) and `PixelRatio` (1), a Canvas3D its size.
    fn handle(&mut self, ctl: usize, kind: ObjectKind, size: (f64, f64), ratio: f64) -> Result<Handle, String> {
        if let Some((h, _)) = self.handles.get(&ctl) {
            return Ok(*h);
        }
        let dim = |v: f64, default: i64| if v >= 1.0 { v as i64 } else { default };
        use cobolt_graphics::Value;
        let args: Vec<Value> = match kind {
            ObjectKind::Canvas2D => vec![Value::Int(dim(size.0, 320)), Value::Int(dim(size.1, 240)), Value::Num(if ratio > 0.0 { ratio } else { 1.0 })],
            ObjectKind::Canvas3D => vec![Value::Int(dim(size.0, 320)), Value::Int(dim(size.1, 240))],
            _ => Vec::new(),
        };
        let h = self.registry.create(kind, OWNER, &args).map_err(|e| e.to_string())?;
        self.handles.insert(ctl, (h, kind));
        Ok(h)
    }
}

/// What a call left for the windows: a picture, or a message for a control's `LastError`. The
/// graphics hook cannot reach the form's tree or its window (the ABI host is busy with the call),
/// so these wait for the pump, which names the control.
enum Out {
    Picture { inst: u32, ctl: usize, frame: canvas_frames::CanvasFrame },
    Picture3d { inst: u32, ctl: usize, frame: gpu_share::Canvas3dFrame },
    LastError { inst: u32, ctl: usize, text: String },
}

/// The graphics objects of every form instance of the run, shared by the ABI host's graphics hook
/// and the program thread's pump.
#[derive(Default)]
pub(crate) struct Graphics {
    sessions: HashMap<u32, Session>,
    out: Vec<Out>,
    /// Pictures for an instance whose window is not joined to it yet.
    waiting: Vec<(u32, StateUpdate)>,
}

/// The graphics of a run, as the ABI host's hook and the program thread hold them.
pub(crate) type Shared = Rc<RefCell<Graphics>>;

/// Give `host` the graphics hook: a read-back of a Canvas2D, and every method of the other six,
/// answered from the instance's registry after the drawing the instance has waiting.
pub(crate) fn install(host: &mut Host) -> Shared {
    let shared: Shared = Rc::new(RefCell::new(Graphics::default()));
    let g = Rc::clone(&shared);
    host.gfx_sync = Some(Rc::new(move |inst, ctl, _part, kind, size, method, args, queued| g.borrow_mut().sync(inst, ctl, kind, size, method, args, queued)));
    shared
}

impl Graphics {
    /// A method that answers at once (the ABI host's hook).
    #[allow(clippy::too_many_arguments)]
    fn sync(&mut self, inst: u32, ctl: usize, kind: &str, size: (f64, f64), method: &str, args: &[String], queued: Vec<(usize, GfxItem)>) -> GfxAnswer {
        let Some(kind) = ObjectKind::from_name(kind) else {
            return GfxAnswer::Refused(format!("{kind} is not a graphics object"));
        };
        let start = self.out.len();
        let session = self.sessions.entry(inst).or_insert_with(Session::new);
        // What the program drew before this call goes ahead of it, in order.
        for (c, item) in queued {
            let q = session.pending.entry(c).or_default();
            if q.len() < MAX_PENDING {
                q.push_back(item);
            }
        }
        // A 3D view takes the window's device before it is made or called, as the interpreter's does.
        if kind == ObjectKind::Canvas3D {
            self.sync_gpu(inst);
        }
        // What `sync_gpu` noted for this call (a replaced device) is `start..noted`.
        let noted = self.out.len();
        let Some(session) = self.sessions.get_mut(&inst) else { return GfxAnswer::NotLoaded };
        if let Err(why) = session.handle(ctl, kind, size, 1.0) {
            return GfxAnswer::Refused(why);
        }
        self.run_pending(inst);
        let session = self.sessions.get_mut(&inst).expect("made above");
        let (h, _) = session.handles[&ctl];
        let values = values_for(kind, method, &args.iter().map(|a| (a.to_string(), false)).collect::<Vec<_>>());
        let answer = match session.registry.invoke(h, method, &values) {
            Ok(v) => GfxAnswer::Answer(answer_text(&v)),
            Err(e) => GfxAnswer::Refused(e.to_string()),
        };
        // The call's answer is the last word on its control's `LastError`, as in the interpreter:
        // what earlier calls and the drawing before it left there is superseded. The ABI host writes
        // a refusal itself; an answer leaves "" (the interpreter writes it after every call that
        // answers, so it is in the form's state even when it was empty), unless the call found a new
        // graphics device, whose note stays.
        let own = |o: &Out| matches!(o, Out::LastError { inst: i, ctl: c, .. } if *i == inst && *c == ctl);
        let device_note = self.out[start..noted].iter().any(own);
        let mut at = 0;
        self.out.retain(|o| {
            let keep = (start..noted).contains(&at) || !own(o);
            at += 1;
            keep
        });
        if matches!(answer, GfxAnswer::Answer(_)) && !device_note {
            self.out.push(Out::LastError { inst, ctl, text: String::new() });
        }
        if matches!(answer, GfxAnswer::Answer(_)) && kind == ObjectKind::Canvas3D && method.trim().eq_ignore_ascii_case("Render") {
            self.publish_3d(inst, ctl);
        }
        self.collect_presented(inst);
        answer
    }

    /// Run the drawing waiting for every canvas of `inst` whose object is made.
    fn run_pending(&mut self, inst: u32) {
        let Some(session) = self.sessions.get(&inst) else { return };
        let mut ready: Vec<usize> = session.pending.iter().filter(|(c, q)| !q.is_empty() && session.handles.contains_key(c)).map(|(c, _)| *c).collect();
        // In control order, so two canvases' LastError writes land the same way on every run.
        ready.sort_unstable();
        for c in ready {
            let items: Vec<GfxItem> = self.sessions.get_mut(&inst).and_then(|s| s.pending.get_mut(&c)).map(|q| q.drain(..).collect()).unwrap_or_default();
            for item in items {
                self.run_item(inst, c, item);
            }
        }
    }

    /// One drawing command (or display list) of the Canvas2D `ctl`, whose object is made. A refusal
    /// is the program's mistake, written to `LastError` as the interpreter writes it; the rest still
    /// draws.
    fn run_item(&mut self, inst: u32, ctl: usize, item: GfxItem) {
        let Some(session) = self.sessions.get_mut(&inst) else { return };
        let Some((h, _)) = session.handles.get(&ctl).copied() else { return };
        let outcome = match &item {
            GfxItem::Call(c) => {
                let values = values_for(ObjectKind::Canvas2D, &c[0], &c[1..].iter().map(|a| (a.to_string(), false)).collect::<Vec<_>>());
                session.registry.invoke(h, &c[0], &values).map(|_| ()).map_err(|e| format!("{} refused: {e}", c[0]))
            }
            GfxItem::List(bytes) => session.registry.draw_list(h, bytes).map(|_| ()).map_err(|e| format!("the drawing was refused: {e}")),
        };
        // The interpreter clears `LastError` after every call that answers; the pump does it once,
        // after the commands, unless one was refused.
        self.out.push(Out::LastError { inst, ctl, text: outcome.err().unwrap_or_default() });
        self.collect_presented(inst);
    }

    /// Every Canvas2D of `inst` that a `Present` ran on hands its picture over.
    fn collect_presented(&mut self, inst: u32) {
        let Some(session) = self.sessions.get_mut(&inst) else { return };
        let canvases: Vec<(usize, Handle)> = session.handles.iter().filter(|(_, (_, k))| *k == ObjectKind::Canvas2D).map(|(c, (h, _))| (*c, *h)).collect();
        for (ctl, h) in canvases {
            let picture = session
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
            if let Some((width, height, rgba_premultiplied)) = picture {
                session.frames += 1;
                self.out.push(Out::Picture { inst, ctl, frame: canvas_frames::CanvasFrame { width, height, rgba_premultiplied, generation: session.frames } });
            }
        }
    }

    /// A Canvas3D that has just rendered hands its render target over; a message it raised (no
    /// adapter, a lost device) goes to `LastError`.
    fn publish_3d(&mut self, inst: u32, ctl: usize) {
        let Some(session) = self.sessions.get_mut(&inst) else { return };
        let Some((h, _)) = session.handles.get(&ctl).copied() else { return };
        let seen = session
            .registry
            .with_canvas3d(h, |c, _| Ok((c.renderer().and_then(|r| r.color_texture().cloned().map(|t| (t, r.gpu().serial()))), c.size(), c.take_event())))
            .ok();
        let Some((target, (width, height), event)) = seen else { return };
        if let Some(text) = event {
            self.out.push(Out::LastError { inst, ctl, text });
        }
        let Some((texture, device)) = target else { return };
        session.frames += 1;
        let generation = session.frames;
        self.out.push(Out::Picture3d { inst, ctl, frame: gpu_share::Canvas3dFrame { texture, width, height, generation, device } });
    }

    /// The window published its device, or a new one: the registry of `inst` takes it, and every
    /// Canvas3D that was connected is connected again there and, with a scene, draws again, as the
    /// interpreter's do. Only a replaced device is reported, in `LastError` (written after the call
    /// that found it, so it stays there as the interpreter leaves it).
    fn sync_gpu(&mut self, inst: u32) {
        let Some((generation, gpu)) = gpu_share::current_gpu() else { return };
        let Some(session) = self.sessions.get_mut(&inst) else { return };
        if session.gpu_gen == generation {
            return;
        }
        let replaced = session.gpu_gen != 0;
        session.gpu_gen = generation;
        session.registry.set_gpu(gpu);
        let mut views: Vec<(usize, Handle, bool)> = Vec::new();
        for (ctl, (h, k)) in &session.handles {
            if *k != ObjectKind::Canvas3D {
                continue;
            }
            if let Ok((true, has_scene)) = session.registry.with_canvas3d(*h, |c, _| Ok((c.renderer().is_some(), c.scene().is_some()))) {
                views.push((*ctl, *h, has_scene));
            }
        }
        views.sort_by_key(|(c, _, _)| *c);
        for (ctl, h, has_scene) in views {
            let Some(session) = self.sessions.get_mut(&inst) else { break };
            let _ = session.registry.invoke(h, "Connect", &[]);
            if has_scene && session.registry.invoke(h, "Render", &[]).is_ok() {
                self.publish_3d(inst, ctl);
            }
            if replaced {
                self.out.push(Out::LastError { inst, ctl, text: "the graphics device was replaced: the 3D view was connected to the new one".into() });
            }
        }
    }

    /// Run the drawing among `list` (the tree's `canvas-call` and `canvas-draw` commands), in order,
    /// leaving every other change in it; publish the pictures the drawing and the calls made, and
    /// write the `LastError`s they left; release the objects of instances that are gone. Answers the
    /// state updates for the windows: `(instance, update)`.
    pub(crate) fn pump(&mut self, host: &mut Host, list: &mut Vec<(u32, Change)>) -> Vec<(u32, StateUpdate)> {
        let mut kept = Vec::with_capacity(list.len());
        for (inst, change) in std::mem::take(list) {
            let item = match &change {
                Change::Command { ctl, name, args } if name == "canvas-call" && !args.is_empty() => Some((*ctl, GfxItem::Call(args.clone()))),
                Change::Command { ctl, name, args } if name == "canvas-draw" => {
                    let seq = args.first().and_then(|s| s.parse::<u64>().ok());
                    match seq.and_then(|seq| host.trees.get_mut(&inst).and_then(|t| t.take_gfx(seq))) {
                        Some(bytes) => Some((*ctl, GfxItem::List(bytes))),
                        None => continue,
                    }
                }
                _ => None,
            };
            let Some((ctl, item)) = item else {
                kept.push((inst, change));
                continue;
            };
            let Some(tree) = host.trees.get(&inst) else { continue };
            // Drawing is recorded only for a Canvas2D (`canvas-call`); its object is made with the
            // size the program sees now.
            let num = |p: &str| tree.get(ctl, p).trim().parse::<f64>().unwrap_or(0.0);
            let (size, ratio) = ((num("Width"), num("Height")), num("PixelRatio"));
            let session = self.sessions.entry(inst).or_insert_with(Session::new);
            match session.handle(ctl, ObjectKind::Canvas2D, size, ratio) {
                Ok(_) => {
                    let q = session.pending.entry(ctl).or_default();
                    if q.len() < MAX_PENDING {
                        q.push_back(item);
                    }
                    self.run_pending(inst);
                }
                Err(why) => self.out.push(Out::LastError { inst, ctl, text: format!("the canvas could not be made: {why}") }),
            }
        }
        *list = kept;
        // A Canvas3D follows the window's device as soon as it is published (the interpreter looks at
        // every event).
        let insts: Vec<u32> = self.sessions.keys().copied().collect();
        for inst in insts {
            if self.sessions[&inst].handles.values().any(|(_, k)| *k == ObjectKind::Canvas3D) {
                self.sync_gpu(inst);
            }
        }
        // What the drawing and the calls left.
        let mut updates = std::mem::take(&mut self.waiting);
        // The last word on each control's `LastError` in this pump: a refusal, or "" from a call that answered.
        let mut errors: BTreeMap<(u32, usize), String> = BTreeMap::new();
        for out in std::mem::take(&mut self.out) {
            match out {
                Out::Picture { inst, ctl, frame } => {
                    let (Some(tree), Some(session)) = (host.trees.get(&inst), self.sessions.get(&inst)) else { continue };
                    let name = tree.ctls[ctl].name.clone();
                    let token = canvas_frames::token(session.slot, frame.generation);
                    canvas_frames::publish(session.slot, &name, frame);
                    updates.push((inst, StateUpdate::new(name, canvas_frames::FRAME_PROP, token)));
                }
                Out::Picture3d { inst, ctl, frame } => {
                    let (Some(tree), Some(session)) = (host.trees.get(&inst), self.sessions.get(&inst)) else { continue };
                    let name = tree.ctls[ctl].name.clone();
                    let token = format!("{}:{}", session.slot, frame.generation);
                    gpu_share::publish_frame(session.slot, &name, frame);
                    updates.push((inst, StateUpdate::new(name, gpu_share::FRAME_PROP, token)));
                }
                Out::LastError { inst, ctl, text } => {
                    errors.insert((inst, ctl), text);
                }
            }
        }
        // Written even when it does not change: the interpreter writes `LastError` after every call,
        // so it is in the form's state from the first call on.
        for ((inst, ctl), text) in errors {
            if let Some(tree) = host.trees.get_mut(&inst) {
                let _ = tree.set(ctl, "LastError", &text);
            }
        }
        // A form that is gone lets go of its objects and its pictures.
        let gone: Vec<u32> = self.sessions.keys().copied().filter(|i| !host.trees.contains_key(i)).collect();
        for inst in gone {
            if let Some(mut s) = self.sessions.remove(&inst) {
                s.handles.clear();
                canvas_frames::forget(s.slot);
                gpu_share::forget_frames(s.slot);
                s.registry.release_owner(OWNER);
            }
            updates.retain(|(i, _)| *i != inst);
        }
        updates
    }

    /// Pictures for an instance whose window is not joined to it yet wait for the next pump.
    pub(crate) fn wait(&mut self, inst: u32, update: StateUpdate) {
        self.waiting.push((inst, update));
    }
}

/// Whether the event `event` of control `ctl` of instance `inst` is a Canvas2D's `onFrame` whose
/// picture is presented when the handler returns: `AutoPresent` on, the default (the interpreter's
/// rule, R6 of WASM-018).
pub(crate) fn auto_presents(host: &Host, inst: u32, ctl: usize, event: &str) -> bool {
    let Some(tree) = host.trees.get(&inst) else { return false };
    let Some(c) = tree.ctls.get(ctl) else { return false };
    if !event.trim().eq_ignore_ascii_case("onFrame") || player_kind(c) != Some(PlayerKind::Canvas2D) {
        return false;
    }
    let auto = tree.get(ctl, "AutoPresent");
    !matches!(auto.trim().to_ascii_lowercase().as_str(), "0" | "false" | "no" | "off")
}
