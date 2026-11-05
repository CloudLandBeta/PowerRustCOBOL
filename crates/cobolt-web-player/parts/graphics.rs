// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

// **The graphics part of the player** (WASM-016 R13, WASM-018 R7/R16/R18), shared source.
//
// Three crates compile this file with `include!`, each with the `cobolt-graphics` features its part needs:
// `canvas` (`text` + `imagebuf`: Canvas2D and ImageBuffer), `graphics` (+ `plot` + `physics`) and `graphics3d`
// (+ `scene3d`: Scene3D, Canvas3D, Physics3D bound to nodes). They are *supersets*, so that one form instance's
// objects live in **one `Registry`** and a handle one object makes is the next object's argument (a Plot drawn
// into a Canvas2D, a body bound to a node). The player loads the smallest part that has every kind the form
// uses, and loads no heavier one for a form that does not.
//
// The methods are `cobolt-graphics`' own, called by name with their arguments as text, exactly as the
// interpreter routes them (`cobolt-runtime`'s `interpreter/graphics.rs`); an object is made the first time it is
// used, from the control's `Width`, `Height` and `PixelRatio`. Nothing here touches the page: the player puts the
// pixels of a canvas on its own `<canvas>` element.

use std::collections::HashMap;

use cobolt_graphics::handles::Handle;
use cobolt_graphics::{Limits, ObjectKind, Registry, Value};
use wasm_bindgen::prelude::*;

/// An argument as the library's value, by the kind word of the method's parameter.
fn convert(kind: &str, text: &str) -> Value {
    let t = text.trim();
    let list = || t.split([',', ' ', ';']).filter(|s| !s.is_empty());
    match kind {
        "int" | "handle" | "flag" => t.parse::<i64>().map(Value::Int).or_else(|_| t.parse::<f64>().map(|f| Value::Int(f as i64))).unwrap_or_else(|_| Value::Text(text.to_owned())),
        "num" => t.parse::<f64>().map(Value::Num).unwrap_or_else(|_| Value::Text(text.to_owned())),
        // A colour is a whole number `0xRRGGBBAA`, or text such as `#336699` or `red`.
        "color" => t.parse::<i64>().map(Value::Int).unwrap_or_else(|_| Value::Text(text.to_owned())),
        "numbers" => Value::Floats(list().filter_map(|s| s.parse().ok()).collect()),
        "ints" => Value::Ints(list().filter_map(|s| s.parse().ok()).collect()),
        "bytes" => Value::Bytes(text.as_bytes().to_vec()),
        _ => Value::Text(text.to_owned()),
    }
}

/// The library's answer as the text a COBOL program receives (the interpreter's `to_text`).
fn show(v: &Value) -> String {
    match v {
        Value::Nothing => String::new(),
        Value::Int(n) => n.to_string(),
        Value::Num(f) => {
            if f.fract() == 0.0 && f.abs() < 1e15 {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        Value::Text(t) => t.clone(),
        Value::Bytes(b) => format!("{} bytes", b.len()),
        Value::Floats(f) => f.iter().map(f64::to_string).collect::<Vec<_>>().join(" "),
        Value::Ints(i) => i.iter().map(i64::to_string).collect::<Vec<_>>().join(" "),
    }
}

/// `["a","b"]` as texts: the one shape the player sends (no number, no nesting).
fn parse_args(json: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut chars = json.trim().chars().peekable();
    if chars.next() != Some('[') {
        return Err("the arguments are not a list".into());
    }
    loop {
        while chars.peek().is_some_and(|c| c.is_whitespace() || *c == ',') {
            chars.next();
        }
        match chars.next() {
            Some(']') => return Ok(out),
            Some('"') => {
                let mut s = String::new();
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some('r') => s.push('\r'),
                            Some('u') => {
                                let hex: String = chars.by_ref().take(4).collect();
                                s.push(u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32).unwrap_or('\u{fffd}'));
                            }
                            Some(c) => s.push(c),
                            None => return Err("the arguments end inside a text".into()),
                        },
                        Some(c) => s.push(c),
                        None => return Err("the arguments end inside a text".into()),
                    }
                }
                out.push(s);
            }
            _ => return Err("the arguments are not a list of texts".into()),
        }
    }
}

/// One object of a form instance: the control it belongs to, by the id the player gives it.
struct Obj {
    handle: Handle,
    kind: ObjectKind,
    /// The last picture of a Canvas2D at device resolution (straight alpha), reused between reads.
    pixels: Vec<u8>,
}

/// The owner every object of one `Graphics` carries: one `Graphics` is one form instance.
const OWNER: u32 = 1;

/// The objects of one form instance, in one registry.
#[wasm_bindgen]
pub struct Graphics {
    reg: Registry,
    objs: HashMap<u32, Obj>,
    /// The kind word of each parameter of each method, by `KIND.METHOD` in upper case.
    kinds: HashMap<String, Vec<&'static str>>,
}

impl Graphics {
    fn obj(&self, ctl: u32) -> Result<(Handle, ObjectKind), String> {
        self.objs.get(&ctl).map(|o| (o.handle, o.kind)).ok_or_else(|| format!("control {ctl} has no graphics object yet"))
    }

    fn add_font_inner(&mut self, bytes: &[u8]) -> Result<u32, String> {
        self.reg.fonts().add_font_data(bytes.to_vec()).map(|faces| faces.len() as u32).map_err(|e| e.to_string())
    }

    fn make_inner(&mut self, ctl: u32, kind: &str, width: u32, height: u32, ratio: f32) -> Result<u32, String> {
        if let Some(o) = self.objs.get(&ctl) {
            return Ok(o.handle);
        }
        // A kind the library knows but this part was built without is named, so the message says what to load instead.
        const EVERY_KIND: [&str; 7] = ["Canvas2D", "Canvas3D", "ImageBuffer", "Plot", "Scene3D", "Physics2D", "Physics3D"];
        let Some(kind) = ObjectKind::from_name(kind) else {
            return Err(if EVERY_KIND.iter().any(|n| n.eq_ignore_ascii_case(kind.trim())) { format!("this part of the player does not carry {}", kind.trim()) } else { format!("`{kind}` is not a graphics object") });
        };
        if !ObjectKind::available().contains(&kind) {
            return Err(format!("this part of the player does not carry {}", kind.name()));
        }
        let args: Vec<Value> = match kind {
            ObjectKind::Canvas2D => vec![Value::Int(width.max(1) as i64), Value::Int(height.max(1) as i64), Value::Num(ratio as f64)],
            ObjectKind::Canvas3D => vec![Value::Int(width.max(1) as i64), Value::Int(height.max(1) as i64)],
            _ => Vec::new(),
        };
        let handle = self.reg.create(kind, OWNER, &args).map_err(|e| e.to_string())?;
        for m in Registry::methods(kind) {
            self.kinds.entry(format!("{}.{}", kind.name(), m.name).to_ascii_uppercase()).or_insert_with(|| m.params.iter().map(|p| p.1).collect());
        }
        self.objs.insert(ctl, Obj { handle, kind, pixels: Vec::new() });
        Ok(handle)
    }

    fn call_inner(&mut self, ctl: u32, method: &str, args: &[String]) -> Result<String, String> {
        let (handle, kind) = self.obj(ctl)?;
        let key = format!("{}.{}", kind.name(), method.trim()).to_ascii_uppercase();
        let kinds = self.kinds.get(&key);
        let values: Vec<Value> = args.iter().enumerate().map(|(i, a)| convert(kinds.and_then(|k| k.get(i)).copied().unwrap_or("text"), a)).collect();
        self.reg.invoke(handle, method, &values).map(|v| show(&v)).map_err(|e| e.to_string())
    }

    fn canvas2d<R>(&mut self, ctl: u32, f: impl FnOnce(&mut cobolt_graphics::Canvas2D) -> cobolt_graphics::Result<R>) -> Result<R, String> {
        let (handle, kind) = self.obj(ctl)?;
        if kind != ObjectKind::Canvas2D {
            return Err("this control is not a Canvas2D".into());
        }
        self.reg.with_canvas2d(handle, |c, _| f(c)).map_err(|e| e.to_string())
    }
}

#[wasm_bindgen]
impl Graphics {
    /// A registry with the library's default limits.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Graphics {
        Graphics { reg: Registry::new(Limits::default()), objs: HashMap::new(), kinds: HashMap::new() }
    }

    /// The kinds this part carries, as a comma list (the player checks it against the form's controls).
    pub fn kinds(&self) -> String {
        ObjectKind::available().iter().map(|k| k.name()).collect::<Vec<_>>().join(",")
    }

    /// Register the faces of a font file (TTF, OTF, TTC) the project ships (WASM-018 T27, Q7): the text of every canvas and
    /// plot of this registry is lettered with them, the same files the desktop registers. The answer is how many faces
    /// the file added; a file that is not a font says so.
    pub fn add_font(&mut self, bytes: &[u8]) -> Result<u32, JsError> {
        self.add_font_inner(bytes).map_err(|e| JsError::new(&e))
    }

    /// Make the object of control `ctl` (idempotent); the answer is its handle. A canvas takes its size from
    /// `width` x `height` logical pixels and `pixel_ratio`; the other kinds ignore them.
    pub fn make(&mut self, ctl: u32, kind: &str, width: u32, height: u32, pixel_ratio: f32) -> Result<u32, JsError> {
        self.make_inner(ctl, kind, width, height, pixel_ratio).map_err(|e| JsError::new(&e))
    }

    /// Call `method` of control `ctl` with `args_json` (a JSON array of texts); the answer is the method's
    /// answer as text, or the reason it was refused.
    pub fn call(&mut self, ctl: u32, method: &str, args_json: &str) -> Result<String, JsError> {
        let args = parse_args(args_json).map_err(|e| JsError::new(&e))?;
        self.call_inner(ctl, method, &args).map_err(|e| JsError::new(&e))
    }

    /// Execute one display list the program's module encoded (ABI A23, a `DRAW`) on the Canvas2D `ctl`, after what
    /// it had recorded; the answer is how many commands ran. A damaged list changes nothing and says why.
    pub fn draw_list(&mut self, ctl: u32, list: &[u8]) -> Result<u32, JsError> {
        let (handle, _) = self.obj(ctl).map_err(|e| JsError::new(&e))?;
        self.reg.draw_list(handle, list).map(|n| n.min(u32::MAX as u64) as u32).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Whether a `Present` has run on the Canvas2D `ctl` since the last call of this.
    pub fn take_presented(&mut self, ctl: u32) -> bool {
        self.canvas2d(ctl, |c| Ok(c.take_presented())).unwrap_or(false)
    }

    /// The picture's width in device pixels.
    pub fn device_width(&mut self, ctl: u32) -> u32 {
        self.canvas2d(ctl, |c| Ok(c.device_size().0)).unwrap_or(0)
    }

    /// The picture's height in device pixels.
    pub fn device_height(&mut self, ctl: u32) -> u32 {
        self.canvas2d(ctl, |c| Ok(c.device_size().1)).unwrap_or(0)
    }

    /// The picture of the Canvas2D `ctl` as straight-alpha RGBA.
    pub fn rgba(&mut self, ctl: u32) -> Vec<u8> {
        let mut px = self.objs.get_mut(&ctl).map(|o| std::mem::take(&mut o.pixels)).unwrap_or_default();
        let _ = self.canvas2d(ctl, |c| {
            let (w, h) = c.device_size();
            px.resize(w as usize * h as usize * 4, 0);
            c.write_rgba(&mut px);
            Ok(())
        });
        let out = px.clone();
        if let Some(o) = self.objs.get_mut(&ctl) {
            o.pixels = px;
        }
        out
    }

    /// Release control `ctl`'s object (and what it made that nothing else holds).
    pub fn release(&mut self, ctl: u32) {
        if let Some(o) = self.objs.remove(&ctl) {
            let _ = self.reg.release(o.handle);
        }
    }

    /// How many objects the registry holds (tests and the limits report).
    pub fn object_count(&self) -> u32 {
        self.reg.len() as u32
    }
}

impl Default for Graphics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "scene3d")]
mod gpu3d {
    use super::*;
    use cobolt_graphics::scene3d::{Gpu, PendingRead};

    /// A graphics device opened for the page (WebGPU, else WebGL2), not yet given to a registry.
    #[wasm_bindgen]
    pub struct GpuDevice {
        gpu: Option<Gpu>,
    }

    /// Open the best adapter the browser offers. The answer is the device or the reason there is none.
    #[wasm_bindgen]
    pub async fn open_gpu() -> Result<GpuDevice, JsError> {
        Gpu::new_async().await.map(|g| GpuDevice { gpu: Some(g) }).map_err(|e| JsError::new(&e.to_string()))
    }

    #[wasm_bindgen]
    impl GpuDevice {
        /// The backend's name (`WebGPU`, `WebGL2`, `Software`).
        pub fn backend(&self) -> String {
            self.gpu.as_ref().map(|g| g.info().backend.name().to_owned()).unwrap_or_default()
        }
    }

    /// The picture a Canvas3D has just rendered, being copied out of the device.
    #[wasm_bindgen]
    pub struct Picture3D {
        read: Option<PendingRead>,
        width: u32,
        height: u32,
    }

    #[wasm_bindgen]
    impl Picture3D {
        pub fn width(&self) -> u32 {
            self.width
        }
        pub fn height(&self) -> u32 {
            self.height
        }
        /// Wait for the copy: the picture as straight-alpha RGBA.
        pub async fn finish(mut self) -> Result<Vec<u8>, JsError> {
            let read = self.read.take().ok_or_else(|| JsError::new("the picture was already taken"))?;
            read.finish().await.map(|p| p.into_bytes()).map_err(|e| JsError::new(&e.to_string()))
        }
    }

    #[wasm_bindgen]
    impl Graphics {
        /// Give the registry the device `open_gpu` made; every Canvas3D then connects to it.
        pub fn set_gpu(&mut self, device: &GpuDevice) -> Result<(), JsError> {
            let gpu = device.gpu.clone().ok_or_else(|| JsError::new("the device is not open"))?;
            self.reg.set_gpu(gpu);
            Ok(())
        }

        /// Start reading the picture the Canvas3D `ctl` rendered last.
        pub fn picture3d(&mut self, ctl: u32) -> Result<Picture3D, JsError> {
            let (handle, kind) = self.obj(ctl).map_err(|e| JsError::new(&e))?;
            if kind != ObjectKind::Canvas3D {
                return Err(JsError::new("this control is not a Canvas3D"));
            }
            self.reg
                .with_canvas3d(handle, |c, _| {
                    let (width, height) = c.size();
                    let r = c.renderer().ok_or_else(|| cobolt_graphics::GraphicsError::Gpu("nothing has been rendered yet: call Render first".into()))?;
                    Ok(Picture3D { read: Some(r.begin_read()?), width, height })
                })
                .map_err(|e| JsError::new(&e.to_string()))
        }
    }
}

#[cfg(feature = "scene3d")]
pub use gpu3d::{open_gpu, GpuDevice, Picture3D};

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_graphics::color::Rgba;
    use cobolt_graphics::display_list::DisplayListBuilder;

    #[test]
    fn methods_called_by_name_with_text_arguments_draw_what_the_documentation_says() {
        let mut g = Graphics::new();
        g.make_inner(7, "Canvas2D", 20, 10, 1.0).expect("a canvas");
        g.call_inner(7, "SetFillColor", &["#FF0000".to_owned()]).expect("a colour as text");
        g.call_inner(7, "FillRect", &["0".into(), "0".into(), "10".into(), "5".into()]).expect("numbers as text");
        assert!(!g.canvas2d(7, |c| Ok(c.take_presented())).unwrap(), "nothing is presented until Present runs");
        g.call_inner(7, "Present", &[]).expect("present");
        assert!(g.take_presented(7), "Present ran");
        assert!(!g.take_presented(7), "and it is told once");
        let px = g.rgba(7);
        assert_eq!(&px[0..4], &[255, 0, 0, 255]);
        let outside = (9 * 20 + 19) * 4;
        assert_eq!(&px[outside..outside + 4], &[0, 0, 0, 0]);
        assert_eq!((g.device_width(7), g.device_height(7)), (20, 10));
        assert!(g.call_inner(7, "NoSuchMethod", &[]).is_err(), "an unknown method says so");
        assert!(g.call_inner(7, "FillRect", &["1".into()]).is_err(), "too few arguments are refused with a sentence");
        g.call_inner(7, "SetFillColor", &["4278190335".into()]).expect("0xFF0000FF as a number");
        assert!(g.call_inner(8, "Present", &[]).is_err(), "a control with no object is refused");
    }

    #[test]
    fn a_shipped_font_letters_the_text_of_a_canvas_and_a_text_with_no_font_says_so() {
        let font = include_bytes!("../../../assets/web-fonts/Ubuntu-Light.ttf");
        let mut g = Graphics::new();
        g.make_inner(1, "Canvas2D", 80, 24, 1.0).unwrap();
        g.call_inner(1, "SetFillColor", &["#FFFFFF".into()]).unwrap();
        g.call_inner(1, "SetFont", &["16px Ubuntu".into()]).unwrap();
        g.call_inner(1, "DrawText", &["Hi".into(), "2".into(), "18".into(), "0".into(), "1".into()]).unwrap();
        let _ = g.call_inner(1, "Present", &[]);
        let diag = g.call_inner(1, "GetDiagnostics", &[]).unwrap();
        assert!(diag.contains("font"), "no font is registered yet: {diag}");
        assert!(g.add_font_inner(b"not a font").is_err(), "a file that is no font is refused");
        let faces = g.add_font_inner(font).expect("the project's font");
        assert!(faces >= 1);
        g.call_inner(1, "DrawText", &["Hi".into(), "2".into(), "18".into(), "0".into(), "1".into()]).unwrap();
        g.call_inner(1, "Present", &[]).unwrap();
        let lit = g.rgba(1).chunks(4).filter(|p| p[3] != 0).count();
        assert!(lit > 20, "the text was drawn with the shipped font: {lit} pixels");
    }

    #[test]
    fn the_argument_list_is_read_with_its_escapes_and_anything_else_is_refused() {
        assert_eq!(parse_args(r#"["a", "b\"c", "x\ny", "é"]"#).unwrap(), vec!["a", "b\"c", "x\ny", "é"]);
        assert_eq!(parse_args("[]").unwrap(), Vec::<String>::new());
        assert!(parse_args("[1]").is_err());
        assert!(parse_args("{}").is_err());
        assert!(parse_args(r#"["open"#).is_err());
    }

    #[test]
    fn a_list_from_the_module_is_drawn_after_the_recorded_calls_and_get_pixel_answers_what_was_drawn() {
        let mut g = Graphics::new();
        g.make_inner(1, "Canvas2D", 20, 10, 1.0).expect("a canvas");
        g.call_inner(1, "SetFillColor", &["#0000FF".to_owned()]).unwrap();
        g.call_inner(1, "FillRect", &["0".into(), "0".into(), "20".into(), "10".into()]).unwrap();
        let mut b = DisplayListBuilder::new();
        b.set_fill_color(Rgba::new(255, 0, 0, 255));
        b.fill_rect(0.0, 0.0, 10.0, 10.0);
        b.present();
        let list = b.take().expect("a list");
        let (h, _) = g.obj(1).unwrap();
        assert_eq!(g.reg.draw_list(h, &list).unwrap(), 3, "the list's three commands ran");
        assert!(g.take_presented(1), "the list's Present is told");
        assert_eq!(g.call_inner(1, "GetPixel", &["2".into(), "2".into()]).unwrap(), "4278190335", "red, 0xFF0000FF, inside the list's rectangle");
        assert_eq!(g.call_inner(1, "GetPixel", &["15".into(), "5".into()]).unwrap(), "65535", "blue 0x0000FFFF from the earlier call");
        assert!(g.reg.draw_list(h, b"nope").is_err(), "a damaged list is refused with the reason");
    }

    #[test]
    fn two_controls_share_one_registry_so_a_handle_one_makes_the_other_takes() {
        let mut g = Graphics::new();
        g.make_inner(1, "Canvas2D", 40, 20, 1.0).unwrap();
        g.make_inner(2, "ImageBuffer", 0, 0, 1.0).unwrap();
        assert_eq!(g.object_count(), 2);
        assert!(g.make_inner(3, "Frobnicator", 0, 0, 1.0).is_err(), "an unknown kind is refused with its name");
        assert_eq!(g.make_inner(1, "Canvas2D", 99, 99, 1.0).unwrap(), g.obj(1).unwrap().0, "making twice is the same object");
        g.release(2);
        assert_eq!(g.object_count(), 1, "release gives the object back");
    }
}
