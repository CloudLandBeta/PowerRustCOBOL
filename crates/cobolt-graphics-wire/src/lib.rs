// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Display-list version 1 **on the wire** (WASM-018 R6, T11): the `PRGD` header,
//! the command table, the operand kinds and the encoder.
//!
//! `no_std`, no `alloc`, no dependencies: the generated WebAssembly module links
//! this crate to build its display list in its transport arena, and the
//! interpreter's `cobolt-graphics` builds the very same bytes with it. The
//! decoder, the builder and the executor stay in `cobolt-graphics`.
//!
//! # One definition
//!
//! [`with_command_table!`] hands the command table (opcode, name, builder method
//! name, operands) to a macro. This crate uses it for [`COMMANDS`] and the
//! [`encode`] functions; `cobolt-graphics` uses it for its `Command` enum, its
//! decoder and its builder methods. The table is written once, here.
//!
//! The encoding is documented in `cobolt_graphics::display_list`.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(test)]
extern crate std;

pub mod consts;

/// The four magic bytes of every list.
pub const MAGIC: [u8; 4] = *b"PRGD";
/// The encoding version.
pub const VERSION: u16 = 1;
/// Bytes before the payload.
pub const HEADER_LEN: usize = 16;

/// The output had no room left (only a fixed buffer can say this).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Full;

/// Where encoded bytes go.
pub trait Sink {
    /// Append `bytes`, or fail without appending anything.
    fn put(&mut self, bytes: &[u8]) -> Result<(), Full>;
}

/// A [`Sink`] over a caller's byte slice (the generated module's arena).
#[derive(Debug)]
pub struct Writer<'a> {
    buf: &'a mut [u8],
    len: usize,
}

impl<'a> Writer<'a> {
    /// A writer that fills `buf` from its start.
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, len: 0 }
    }

    /// Bytes written so far.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether nothing was written.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The bytes written so far.
    pub fn written(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

impl Sink for Writer<'_> {
    fn put(&mut self, bytes: &[u8]) -> Result<(), Full> {
        let end = self.len.checked_add(bytes.len()).ok_or(Full)?;
        if end > self.buf.len() {
            return Err(Full);
        }
        self.buf[self.len..end].copy_from_slice(bytes);
        self.len = end;
        Ok(())
    }
}

/// Write the 16-byte header for a list of `commands` commands whose payload is
/// `payload_len` bytes.
pub fn write_header<S: Sink + ?Sized>(s: &mut S, commands: u32, payload_len: u32) -> Result<(), Full> {
    s.put(&MAGIC)?;
    s.put(&VERSION.to_le_bytes())?;
    s.put(&0u16.to_le_bytes())?;
    s.put(&commands.to_le_bytes())?;
    s.put(&payload_len.to_le_bytes())
}

/// Append one byte.
pub fn put_u8<S: Sink + ?Sized>(s: &mut S, v: u8) -> Result<(), Full> {
    s.put(&[v])
}

/// Append a little-endian `u32`.
pub fn put_u32<S: Sink + ?Sized>(s: &mut S, v: u32) -> Result<(), Full> {
    s.put(&v.to_le_bytes())
}

/// Append an `f32` as its IEEE-754 binary32 bits, little-endian.
pub fn put_f32<S: Sink + ?Sized>(s: &mut S, v: f32) -> Result<(), Full> {
    s.put(&v.to_bits().to_le_bytes())
}

/// Append a `str` operand: a `u32` byte length then the UTF-8 bytes.
pub fn put_str<S: Sink + ?Sized>(s: &mut S, v: &str) -> Result<(), Full> {
    put_u32(s, v.len() as u32)?;
    s.put(v.as_bytes())
}

/// A run of `(x, y)` points, however the caller holds them.
pub trait PointSeq {
    /// Number of points.
    fn len(&self) -> usize;
    /// Point `i` (`i < len()`).
    fn get(&self, i: usize) -> (f32, f32);
}

/// A run of `f32` values.
pub trait FloatSeq {
    /// Number of values.
    fn len(&self) -> usize;
    /// Value `i`.
    fn get(&self, i: usize) -> f32;
}

/// A run of gradient stops, colours as `0xRRGGBBAA`.
pub trait StopSeq {
    /// Number of stops.
    fn len(&self) -> usize;
    /// Stop `i`: offset and colour.
    fn get(&self, i: usize) -> (f32, u32);
}

impl PointSeq for &[(f32, f32)] {
    fn len(&self) -> usize {
        <[_]>::len(self)
    }
    fn get(&self, i: usize) -> (f32, f32) {
        self[i]
    }
}

impl<const N: usize> PointSeq for [(f32, f32); N] {
    fn len(&self) -> usize {
        N
    }
    fn get(&self, i: usize) -> (f32, f32) {
        self[i]
    }
}

impl FloatSeq for &[f32] {
    fn len(&self) -> usize {
        <[_]>::len(self)
    }
    fn get(&self, i: usize) -> f32 {
        self[i]
    }
}

impl<const N: usize> FloatSeq for [f32; N] {
    fn len(&self) -> usize {
        N
    }
    fn get(&self, i: usize) -> f32 {
        self[i]
    }
}

impl StopSeq for &[(f32, u32)] {
    fn len(&self) -> usize {
        <[_]>::len(self)
    }
    fn get(&self, i: usize) -> (f32, u32) {
        self[i]
    }
}

impl<const N: usize> StopSeq for [(f32, u32); N] {
    fn len(&self) -> usize {
        N
    }
    fn get(&self, i: usize) -> (f32, u32) {
        self[i]
    }
}

/// Append a `points` operand.
pub fn put_points<S: Sink + ?Sized>(s: &mut S, v: &dyn PointSeq) -> Result<(), Full> {
    put_u32(s, v.len() as u32)?;
    for i in 0..v.len() {
        let (x, y) = v.get(i);
        put_f32(s, x)?;
        put_f32(s, y)?;
    }
    Ok(())
}

/// Append a `floats` operand.
pub fn put_floats<S: Sink + ?Sized>(s: &mut S, v: &dyn FloatSeq) -> Result<(), Full> {
    put_u32(s, v.len() as u32)?;
    for i in 0..v.len() {
        put_f32(s, v.get(i))?;
    }
    Ok(())
}

/// Append a `stops` operand.
pub fn put_stops<S: Sink + ?Sized>(s: &mut S, v: &dyn StopSeq) -> Result<(), Full> {
    put_u32(s, v.len() as u32)?;
    for i in 0..v.len() {
        let (o, c) = v.get(i);
        put_f32(s, o)?;
        put_u32(s, c)?;
    }
    Ok(())
}

/// How one kind of operand is written. The kinds are the names of the table.
pub trait WireKind {
    /// The name used in [`COMMANDS`] (`u8`, `u32`, `f32`, `rgba`, `str`, `points`, `floats`, `stops`).
    const KIND: &'static str;
    /// What the encoder takes.
    type Arg<'a>;
    /// Append the wire form.
    fn put<S: Sink + ?Sized>(s: &mut S, v: Self::Arg<'_>) -> Result<(), Full>;
}

macro_rules! kind {
    ($name:ident, $kind:literal, $ty:ty, $put:expr) => {
        #[doc = concat!("The `", $kind, "` operand.")]
        pub struct $name;
        impl WireKind for $name {
            const KIND: &'static str = $kind;
            type Arg<'a> = $ty;
            fn put<S: Sink + ?Sized>(s: &mut S, v: Self::Arg<'_>) -> Result<(), Full> {
                ($put)(s, v)
            }
        }
    };
}

kind!(U8, "u8", u8, put_u8::<S>);
kind!(U32, "u32", u32, put_u32::<S>);
kind!(F32, "f32", f32, put_f32::<S>);
kind!(Color, "rgba", u32, put_u32::<S>);
kind!(Str, "str", &'a str, put_str::<S>);
kind!(PointsK, "points", &'a dyn PointSeq, put_points::<S>);
kind!(FloatsK, "floats", &'a dyn FloatSeq, put_floats::<S>);
kind!(StopsK, "stops", &'a dyn StopSeq, put_stops::<S>);

/// One row of the command table, for tools that need it as data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandInfo {
    /// The opcode byte.
    pub opcode: u8,
    /// The command name, as in the COBOL method (`FillRect`).
    pub name: &'static str,
    /// The builder method name (`fill_rect`).
    pub method: &'static str,
    /// The operands in wire order: `(name, kind)`.
    pub operands: &'static [(&'static str, &'static str)],
    /// What it does, in one line.
    pub doc: &'static str,
}

/// Hand the command table to the macro `$m`, as
/// `$( $(#[doc = "..."])* 0xNN Name method { field: Kind, ... } )*`.
/// The kinds are the type names `U8 U32 F32 Color Str PointsK FloatsK StopsK`,
/// resolved where `$m` expands.
#[macro_export]
macro_rules! with_command_table {
    ($m:ident) => {
        $m! {
            // ---- state -----------------------------------------------------------
            /// Push the graphics state (transform, styles, clip, text settings).
            0x01 Save save {}
            /// Pop the graphics state.
            0x02 Restore restore {}
            /// Back to the default state: empties the stack, the clip and the path (pixels stay).
            0x03 Reset reset {}
            /// Fill the whole canvas with a colour, ignoring transform, clip and blend mode.
            0x04 Clear clear { color: Color }
            /// A frame boundary: the host may show the picture now.
            0x05 Present present {}
            /// Image resampling quality: 0 off, 1 low, 2 high.
            0x06 SetSmoothing set_smoothing { quality: U8 }

            // ---- paths -----------------------------------------------------------
            /// Start a new empty path.
            0x10 BeginPath begin_path {}
            /// Start a sub-path.
            0x11 MoveTo move_to { x: F32, y: F32 }
            /// A line to the point.
            0x12 LineTo line_to { x: F32, y: F32 }
            /// A quadratic curve.
            0x13 QuadTo quad_to { cx: F32, cy: F32, x: F32, y: F32 }
            /// A cubic curve.
            0x14 CubicTo cubic_to { c1x: F32, c1y: F32, c2x: F32, c2y: F32, x: F32, y: F32 }
            /// A rounded corner between two lines (HTML `arcTo`).
            0x15 ArcTo arc_to { x1: F32, y1: F32, x2: F32, y2: F32, radius: F32 }
            /// A circular arc; angles in radians, `ccw` 1 for counter-clockwise.
            0x16 Arc arc { x: F32, y: F32, radius: F32, start: F32, end: F32, ccw: U8 }
            /// An elliptical arc, rotated by `rotation` radians.
            0x17 Ellipse ellipse { x: F32, y: F32, rx: F32, ry: F32, rotation: F32, start: F32, end: F32, ccw: U8 }
            /// A closed rectangle sub-path.
            0x18 Rect rect { x: F32, y: F32, w: F32, h: F32 }
            /// A closed rounded rectangle; the four radii are top-left, top-right, bottom-right, bottom-left.
            0x19 RoundRect round_rect { x: F32, y: F32, w: F32, h: F32, tl: F32, tr: F32, br: F32, bl: F32 }
            /// Close the current sub-path.
            0x1A ClosePath close_path {}
            /// Fill the current path with the fill style; `rule` 0 non-zero, 1 even-odd.
            0x1B Fill fill { rule: U8 }
            /// Stroke the current path with the stroke style.
            0x1C Stroke stroke {}
            /// Intersect the clip with the current path.
            0x1D Clip clip { rule: U8 }

            // ---- shapes ----------------------------------------------------------
            /// Fill a rectangle (does not touch the current path).
            0x20 FillRect fill_rect { x: F32, y: F32, w: F32, h: F32 }
            /// Stroke a rectangle.
            0x21 StrokeRect stroke_rect { x: F32, y: F32, w: F32, h: F32 }
            /// Make a rectangle transparent.
            0x22 ClearRect clear_rect { x: F32, y: F32, w: F32, h: F32 }
            /// Stroke a line.
            0x23 DrawLine draw_line { x1: F32, y1: F32, x2: F32, y2: F32 }
            /// A circle; `mode` bit 0 fills, bit 1 strokes.
            0x24 DrawCircle draw_circle { x: F32, y: F32, radius: F32, mode: U8 }
            /// A polygon or polyline; `mode` bit 0 fills, bit 1 strokes, bit 2 closes.
            0x25 DrawPolygon draw_polygon { points: PointsK, mode: U8 }

            // ---- style -----------------------------------------------------------
            /// Fill with a solid colour.
            0x30 SetFillColor set_fill_color { color: Color }
            /// Stroke with a solid colour.
            0x31 SetStrokeColor set_stroke_color { color: Color }
            /// Stroke width.
            0x32 SetLineWidth set_line_width { width: F32 }
            /// 0 butt, 1 round, 2 square.
            0x33 SetLineCap set_line_cap { cap: U8 }
            /// 0 miter, 1 round, 2 bevel.
            0x34 SetLineJoin set_line_join { join: U8 }
            /// Miter limit.
            0x35 SetMiterLimit set_miter_limit { limit: F32 }
            /// Dash pattern (empty for solid) and its offset.
            0x36 SetDash set_dash { intervals: FloatsK, offset: F32 }
            /// Global alpha, 0 to 1.
            0x37 SetAlpha set_alpha { alpha: F32 }
            /// Compositing operation (see `BlendMode`).
            0x38 SetBlendMode set_blend_mode { mode: U8 }
            /// A linear gradient as fill (target 0) or stroke (target 1) style.
            0x39 SetLinearGradient set_linear_gradient { target: U8, x0: F32, y0: F32, x1: F32, y1: F32, stops: StopsK }
            /// A radial gradient between two circles.
            0x3A SetRadialGradient set_radial_gradient { target: U8, x0: F32, y0: F32, r0: F32, x1: F32, y1: F32, r1: F32, stops: StopsK }
            /// A conic (sweep) gradient starting at `angle` radians.
            0x3B SetConicGradient set_conic_gradient { target: U8, x: F32, y: F32, angle: F32, stops: StopsK }
            /// An image pattern; `repeat` 0 repeat, 1 repeat-x, 2 repeat-y, 3 no-repeat.
            0x3C SetPattern set_pattern { target: U8, image: U32, repeat: U8 }
            /// Shadow colour, blur, and offset (alpha 0 turns it off).
            0x3D SetShadow set_shadow { color: Color, blur: F32, dx: F32, dy: F32 }

            // ---- transforms ------------------------------------------------------
            /// Move the origin.
            0x50 Translate translate { x: F32, y: F32 }
            /// Rotate by radians.
            0x51 Rotate rotate { angle: F32 }
            /// Scale.
            0x52 Scale scale { x: F32, y: F32 }
            /// Multiply the transform by a matrix.
            0x53 Transform transform { a: F32, b: F32, c: F32, d: F32, e: F32, f: F32 }
            /// Replace the transform by a matrix.
            0x54 SetTransform set_transform { a: F32, b: F32, c: F32, d: F32, e: F32, f: F32 }
            /// Back to the identity transform.
            0x55 ResetTransform reset_transform {}

            // ---- text ------------------------------------------------------------
            /// The font, as a CSS font shorthand ("italic 700 16px Arial, sans-serif").
            0x60 SetFont set_font { css: Str }
            /// 0 start, 1 end, 2 left, 3 right, 4 center.
            0x61 SetTextAlign set_text_align { align: U8 }
            /// 0 alphabetic, 1 top, 2 hanging, 3 middle, 4 ideographic, 5 bottom.
            0x62 SetTextBaseline set_text_baseline { baseline: U8 }
            /// 0 auto, 1 left-to-right, 2 right-to-left.
            0x63 SetTextDirection set_text_direction { direction: U8 }
            /// One line of text; `max_width` 0 means no limit; `mode` bit 0 fills, bit 1 strokes.
            0x64 DrawText draw_text { text: Str, x: F32, y: F32, max_width: F32, mode: U8 }
            /// Text wrapped to `width` with `line_height` between lines.
            0x65 DrawTextBox draw_text_box { text: Str, x: F32, y: F32, width: F32, line_height: F32, mode: U8 }

            // ---- images ----------------------------------------------------------
            /// An image at its natural size.
            0x70 DrawImage draw_image { image: U32, x: F32, y: F32 }
            /// An image scaled into a rectangle.
            0x71 DrawImageScaled draw_image_scaled { image: U32, x: F32, y: F32, w: F32, h: F32 }
            /// A source rectangle of an image into a destination rectangle.
            0x72 DrawImageRegion draw_image_region { image: U32, sx: F32, sy: F32, sw: F32, sh: F32, dx: F32, dy: F32, dw: F32, dh: F32 }
            /// One frame of a sprite sheet (frames are laid out left to right, top to bottom).
            0x73 DrawSprite draw_sprite { image: U32, frame: U32, frame_w: F32, frame_h: F32, dx: F32, dy: F32, dw: F32, dh: F32 }

            // ---- read-back -------------------------------------------------------
            /// Read the pixel under a canvas point into result `slot`.
            0x80 ReadPixel read_pixel { slot: U32, x: F32, y: F32 }
            /// Copy a canvas rectangle into an image delivered as result `slot`.
            0x81 Capture capture { slot: U32, x: F32, y: F32, w: F32, h: F32 }
            /// Whether a canvas point is inside the current path (result `slot`).
            0x82 HitPath hit_path { slot: U32, x: F32, y: F32, rule: U8 }
            /// Whether a canvas point is on the stroke of the current path (result `slot`).
            0x83 HitStroke hit_stroke { slot: U32, x: F32, y: F32 }
            /// Measure a text with the current font (result `slot`).
            0x84 MeasureText measure_text { slot: U32, text: Str }
        }
    };
}

macro_rules! table {
    (
        $(
            $(#[doc = $doc:literal])*
            $op:literal $name:ident $snake:ident { $( $f:ident : $t:ty ),* $(,)? }
        )*
    ) => {
        /// Every command of version 1, in opcode order.
        pub const COMMANDS: &[CommandInfo] = &[
            $( CommandInfo {
                opcode: $op,
                name: stringify!($name),
                method: stringify!($snake),
                operands: &[ $( (stringify!($f), <$t as WireKind>::KIND) ),* ],
                doc: concat!($($doc),*),
            } ),*
        ];

        /// One encoder function per command: the opcode byte, then the operands. A
        /// command that does not fit returns `Full` and may have written its first
        /// bytes; the caller truncates to the length it had before the call.
        pub mod encode {
            use super::*;
            $(
                #[doc = concat!("Encode `", stringify!($name), "` (opcode ", stringify!($op), ").")]
                #[allow(clippy::too_many_arguments)]
                pub fn $snake<S: Sink + ?Sized>(s: &mut S $( , $f: <$t as WireKind>::Arg<'_> )* ) -> Result<(), Full> {
                    s.put(&[$op])?;
                    $( <$t as WireKind>::put(s, $f)?; )*
                    Ok(())
                }
            )*
        }
    };
}

with_command_table!(table);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_writer_fills_its_buffer_and_refuses_one_byte_more() {
        let mut buf = [0u8; 5];
        let mut w = Writer::new(&mut buf);
        assert_eq!(encode::move_to(&mut w, 1.0, 2.0), Err(Full));
        // A command that does not fit may have written its first operands: the
        // caller keeps `len()` from before the call and truncates to it.
        assert_eq!(w.len(), 5);
        let mut buf = [0u8; 9];
        let mut w = Writer::new(&mut buf);
        encode::move_to(&mut w, 1.0, 2.0).unwrap();
        assert_eq!(w.len(), 9);
        assert_eq!(w.written()[0], 0x11);
        encode::save(&mut w).unwrap_err();
    }

    #[test]
    fn the_header_is_sixteen_bytes_in_the_documented_order() {
        let mut buf = [0u8; HEADER_LEN];
        let mut w = Writer::new(&mut buf);
        write_header(&mut w, 3, 40).unwrap();
        assert_eq!(&buf, b"PRGD\x01\x00\x00\x00\x03\x00\x00\x00\x28\x00\x00\x00");
    }

    #[test]
    fn opcodes_are_unique_and_ascending() {
        let mut prev = 0u8;
        for c in COMMANDS {
            assert!(c.opcode > prev, "{} is not above the previous opcode", c.name);
            prev = c.opcode;
        }
    }

    // ---- every command against the bytes recorded before the move (T11) -------

    static POINTS: [(f32, f32); 2] = [(1.0, -2.5), (3.25, 4.0)];
    static FLOATS: [f32; 3] = [4.0, 2.0, 0.5];
    static STOPS: [(f32, u32); 2] = [(0.0, 0xff00_00ff), (1.0, 0x0000_ffff)];

    /// The sample operand of each kind: the same formulas as
    /// `cobolt-graphics/tests/wire_bytes.rs`, which recorded the file.
    trait Sample: WireKind {
        fn sample(n: u32) -> Self::Arg<'static>;
    }
    impl Sample for U8 {
        fn sample(n: u32) -> u8 {
            (n % 3) as u8
        }
    }
    impl Sample for U32 {
        fn sample(n: u32) -> u32 {
            n * 3 + 1
        }
    }
    impl Sample for F32 {
        fn sample(n: u32) -> f32 {
            1.5f32 * (n + 1) as f32
        }
    }
    impl Sample for Color {
        fn sample(n: u32) -> u32 {
            0x1122_3344u32 + n
        }
    }
    impl Sample for Str {
        fn sample(_: u32) -> &'static str {
            "h\u{e9}llo \u{65e5}\u{672c}"
        }
    }
    impl Sample for PointsK {
        fn sample(_: u32) -> &'static dyn PointSeq {
            &POINTS
        }
    }
    impl Sample for FloatsK {
        fn sample(_: u32) -> &'static dyn FloatSeq {
            &FLOATS
        }
    }
    impl Sample for StopsK {
        fn sample(_: u32) -> &'static dyn StopSeq {
            &STOPS
        }
    }

    macro_rules! check_all {
        (
            $(
                $(#[doc = $doc:literal])*
                $op:literal $name:ident $snake:ident { $( $f:ident : $t:ty ),* $(,)? }
            )*
        ) => {
            fn encoded_lines() -> std::vec::Vec<std::string::String> {
                use std::fmt::Write as _;
                let mut lines = std::vec::Vec::new();
                let mut i = 0u32;
                $(
                    {
                        let mut buf = [0u8; 256];
                        let mut w = Writer::new(&mut buf);
                        let mut _k = 0u32;
                        encode::$snake(&mut w $( , { let a = <$t as Sample>::sample(i + _k); _k += 1; let _ = stringify!($f); a } )*).unwrap();
                        let mut line = std::format!("0x{:02X} {} ", $op, stringify!($name));
                        for b in w.written() {
                            write!(line, "{b:02x}").unwrap();
                        }
                        lines.push(line);
                        i += 1;
                    }
                )*
                let _ = i;
                lines
            }
        };
    }
    with_command_table!(check_all);

    #[test]
    fn every_command_encodes_to_the_bytes_recorded_before_the_move() {
        let recorded = include_str!("../../cobolt-graphics/tests/golden/display-list-v1-commands.txt");
        let lines = encoded_lines();
        assert_eq!(lines.len(), COMMANDS.len());
        for (got, want) in lines.iter().zip(recorded.lines()) {
            assert_eq!(got, want);
        }
        assert_eq!(recorded.lines().count(), lines.len(), "the recorded file and the table differ in length");
        std::println!("cobolt-graphics-wire: {} commands encode to the recorded bytes", lines.len());
    }
}
