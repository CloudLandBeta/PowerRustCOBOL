// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Canvas 2D part of the player** (WASM-016 R13, WASM-018 R6/R18).
//!
//! A [`Canvas`] is one drawing surface. The program's drawing methods are recorded as a **display
//! list** (the versioned binary encoding of `cobolt-graphics`); [`Canvas::draw`] executes one list and
//! [`Canvas::rgba`] answers the picture as straight-alpha RGBA at device resolution, which is what a
//! browser `ImageData` takes. Nothing here touches the page: the player puts the pixels on its own
//! `<canvas>` element.

use cobolt_graphics::canvas::{Surface, SurfaceConfig};
use cobolt_graphics::resources::NoResources;
use cobolt_graphics::Limits;
use wasm_bindgen::prelude::*;

/// One Canvas 2D surface.
#[wasm_bindgen]
pub struct Canvas {
    surface: Surface,
    pixels: Vec<u8>,
}

impl Canvas {
    fn create(width: u32, height: u32, pixel_ratio: f32) -> Result<Canvas, String> {
        let mut cfg = SurfaceConfig::new(width, height);
        cfg.pixel_ratio = pixel_ratio;
        let surface = Surface::new(&cfg, &Limits::default()).map_err(|e| e.to_string())?;
        let (w, h) = surface.device_size();
        Ok(Canvas { surface, pixels: vec![0; w as usize * h as usize * 4] })
    }

    fn run(&mut self, list: &[u8]) -> Result<u32, String> {
        let out = self.surface.execute(list, &NoResources).map_err(|e| e.to_string())?;
        Ok(out.commands_executed.min(u32::MAX as u64) as u32)
    }
}

#[wasm_bindgen]
impl Canvas {
    /// A transparent canvas of `width` x `height` logical pixels at `pixel_ratio` device pixels each;
    /// refused when it exceeds the crate's limits.
    #[wasm_bindgen(constructor)]
    pub fn new(width: u32, height: u32, pixel_ratio: f32) -> Result<Canvas, JsError> {
        Canvas::create(width, height, pixel_ratio).map_err(|e| JsError::new(&e))
    }

    /// Execute one display list. A damaged list changes nothing and says why. The answer is how many
    /// commands ran.
    pub fn draw(&mut self, list: &[u8]) -> Result<u32, JsError> {
        self.run(list).map_err(|e| JsError::new(&e))
    }

    /// The picture's width in device pixels.
    pub fn device_width(&self) -> u32 {
        self.surface.device_size().0
    }

    /// The picture's height in device pixels.
    pub fn device_height(&self) -> u32 {
        self.surface.device_size().1
    }

    /// The picture as straight-alpha RGBA, `device_width * device_height * 4` bytes.
    pub fn rgba(&mut self) -> Vec<u8> {
        self.surface.write_rgba(&mut self.pixels);
        self.pixels.clone()
    }
}

/// One **Canvas2D object** as a COBOL program uses it (WASM-018 R7): methods called by name with their
/// arguments as text, the way the player receives them from the program. It is the crate's own `Registry`
/// with one `Canvas2D` in it, so every method of the documented vocabulary (`FillRect`, `SetFillColor`,
/// `DrawText`, `Present`, ...) means exactly what it means on the desktop.
#[wasm_bindgen]
pub struct Surface2D {
    reg: cobolt_graphics::Registry,
    handle: u32,
    /// The kind word of each parameter of each method, by upper-cased method name.
    kinds: std::collections::HashMap<String, Vec<&'static str>>,
    pixels: Vec<u8>,
}

fn convert(kind: &str, text: &str) -> cobolt_graphics::Value {
    use cobolt_graphics::Value;
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

fn show(v: &cobolt_graphics::Value) -> String {
    use cobolt_graphics::Value;
    match v {
        Value::Nothing => String::new(),
        Value::Int(n) => n.to_string(),
        Value::Num(n) => n.to_string(),
        Value::Text(t) => t.clone(),
        Value::Bytes(b) => format!("{} bytes", b.len()),
        Value::Floats(f) => f.iter().map(f64::to_string).collect::<Vec<_>>().join(","),
        Value::Ints(i) => i.iter().map(i64::to_string).collect::<Vec<_>>().join(","),
    }
}

impl Surface2D {
    fn create(width: u32, height: u32, ratio: f32) -> Result<Surface2D, String> {
        use cobolt_graphics::{ObjectKind, Registry, Value};
        let mut reg = Registry::new(Limits::default());
        let handle = reg.create(ObjectKind::Canvas2D, 0, &[Value::Int(width as i64), Value::Int(height as i64), Value::Num(ratio as f64)]).map_err(|e| e.to_string())?;
        let kinds = Registry::methods(ObjectKind::Canvas2D).into_iter().map(|m| (m.name.to_ascii_uppercase(), m.params.iter().map(|p| p.1).collect())).collect();
        let (w, h) = reg.with_canvas2d(handle, |c, _| Ok(c.device_size())).map_err(|e| e.to_string())?;
        Ok(Surface2D { reg, handle, kinds, pixels: vec![0; w as usize * h as usize * 4] })
    }

    fn call_inner(&mut self, method: &str, args: &[String]) -> Result<String, String> {
        let kinds = self.kinds.get(&method.trim().to_ascii_uppercase());
        let values: Vec<cobolt_graphics::Value> = args.iter().enumerate().map(|(i, a)| convert(kinds.and_then(|k| k.get(i)).copied().unwrap_or("text"), a)).collect();
        self.reg.invoke(self.handle, method, &values).map(|v| show(&v)).map_err(|e| e.to_string())
    }

    fn take_presented_inner(&mut self) -> bool {
        self.call_inner("TakePresented", &[]).map(|v| v == "1").unwrap_or(false)
    }

    fn rgba_inner(&mut self) -> Vec<u8> {
        let pixels = &mut self.pixels;
        let _ = self.reg.with_canvas2d(self.handle, |c, _| {
            c.write_rgba(pixels);
            Ok(())
        });
        self.pixels.clone()
    }

    fn size_inner(&mut self) -> (u32, u32) {
        self.reg.with_canvas2d(self.handle, |c, _| Ok(c.device_size())).unwrap_or((0, 0))
    }
}

#[wasm_bindgen]
impl Surface2D {
    /// A canvas of `width` x `height` logical pixels at `pixel_ratio`.
    #[wasm_bindgen(constructor)]
    pub fn new(width: u32, height: u32, pixel_ratio: f32) -> Result<Surface2D, JsError> {
        Surface2D::create(width, height, pixel_ratio).map_err(|e| JsError::new(&e))
    }

    /// Call method `method` with `args_json` (a JSON array of texts); the answer is the method's answer as text,
    /// or the reason it was refused.
    pub fn call(&mut self, method: &str, args_json: &str) -> Result<String, JsError> {
        let args: Vec<String> = parse_args(args_json).map_err(|e| JsError::new(&e))?;
        self.call_inner(method, &args).map_err(|e| JsError::new(&e))
    }

    /// Whether a `Present` has run since the last call of this: the page repaints when it has.
    pub fn take_presented(&mut self) -> bool {
        self.take_presented_inner()
    }

    /// The picture's width in device pixels.
    pub fn device_width(&mut self) -> u32 {
        self.size_inner().0
    }

    /// The picture's height in device pixels.
    pub fn device_height(&mut self) -> u32 {
        self.size_inner().1
    }

    /// The picture as straight-alpha RGBA.
    pub fn rgba(&mut self) -> Vec<u8> {
        self.rgba_inner()
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

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_graphics::color::Rgba;
    use cobolt_graphics::display_list::DisplayListBuilder;

    #[test]
    fn methods_called_by_name_with_text_arguments_draw_what_the_documentation_says() {
        let mut s = Surface2D::create(20, 10, 1.0).expect("a canvas");
        s.call_inner("SetFillColor", &["#FF0000".to_owned()]).expect("a colour as text");
        s.call_inner("FillRect", &["0".into(), "0".into(), "10".into(), "5".into()]).expect("numbers as text");
        assert!(!s.take_presented_inner(), "nothing is presented until Present runs");
        s.call_inner("Present", &[]).expect("present");
        assert!(s.take_presented_inner(), "Present ran");
        assert!(!s.take_presented_inner(), "and it is told once");
        let px = s.rgba_inner();
        assert_eq!(&px[0..4], &[255, 0, 0, 255]);
        let outside = (9 * 20 + 19) * 4;
        assert_eq!(&px[outside..outside + 4], &[0, 0, 0, 0]);
        assert_eq!(s.size_inner(), (20, 10));
        assert!(s.call_inner("NoSuchMethod", &[]).is_err(), "an unknown method says so");
        assert!(s.call_inner("FillRect", &["1".into()]).is_err(), "too few arguments are refused with a sentence");
        // A colour may also be the whole number the model uses, and a list of numbers is read from text.
        s.call_inner("SetFillColor", &["4278190335".into()]).expect("0xFF0000FF as a number");
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
    fn a_recorded_list_is_executed_and_the_picture_read_back() {
        let mut b = DisplayListBuilder::new();
        b.set_fill_color(Rgba::new(255, 0, 0, 255));
        b.fill_rect(0.0, 0.0, 4.0, 4.0);
        b.present();
        let list = b.take().expect("a list");
        let mut c = Canvas::create(8, 8, 1.0).expect("a canvas");
        assert_eq!(c.run(&list), Ok(3));
        let px = c.rgba();
        assert_eq!(&px[0..4], &[255, 0, 0, 255], "inside the rectangle");
        let last = (7 * 8 + 7) * 4;
        assert_eq!(&px[last..last + 4], &[0, 0, 0, 0], "outside it stays transparent");
        assert!(c.run(b"nope").is_err(), "a damaged list is refused");
    }
}
