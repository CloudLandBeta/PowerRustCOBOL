// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The command table: **the** single definition of display-list version 1.
//!
//! One `commands!` invocation (at the bottom) lists every command with its
//! opcode, its name, the Rust builder method name and its operands. From that
//! one list the macro generates the [`Command`] enum, the encoder, the decoder,
//! the builder methods and the [`COMMANDS`] description table that the
//! generator, the documentation and the tests read. Nothing about the wire
//! format is written anywhere else, so the encoder, the decoder and the
//! executor cannot drift apart.

use crate::color::Rgba;
use crate::error::{GraphicsError, Result};

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// A bounds-checked cursor over the payload. Every read that would run past the
/// end is an error - never a panic, never an allocation sized by the input.
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// A reader over `buf`.
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// Bytes not yet read.
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    /// Offset of the next byte.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Take `n` bytes.
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if n > self.remaining() {
            return Err(GraphicsError::bad_list(format!(
                "truncated: {n} byte(s) wanted at offset {}, {} left",
                self.pos,
                self.remaining()
            )));
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    /// One byte.
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    /// A little-endian `u32`.
    pub fn u32(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// A little-endian `i32`.
    pub fn i32(&mut self) -> Result<i32> {
        Ok(self.u32()? as i32)
    }

    /// A little-endian `f32`.
    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
}

/// Append helpers (kept next to the reader so the pairing is obvious).
pub(crate) fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

pub(crate) fn put_f32(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&v.to_bits().to_le_bytes());
}

fn le_f32(b: &[u8]) -> f32 {
    f32::from_bits(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

// ---------------------------------------------------------------------------
// Array operands (zero-copy when decoded, plain slices when built)
// ---------------------------------------------------------------------------

/// A run of `(x, y)` points.
#[derive(Clone, Copy)]
pub enum Points<'a> {
    /// Straight from the wire: `8 * n` bytes of `f32` pairs.
    Wire(&'a [u8]),
    /// From the caller's slice (the builder).
    Pairs(&'a [(f32, f32)]),
}

impl<'a> Points<'a> {
    /// Number of points.
    pub fn len(&self) -> usize {
        match self {
            Self::Wire(b) => b.len() / 8,
            Self::Pairs(p) => p.len(),
        }
    }

    /// Whether there are no points.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Point `i` (`i < len()`).
    pub fn get(&self, i: usize) -> (f32, f32) {
        match self {
            Self::Wire(b) => (le_f32(&b[i * 8..]), le_f32(&b[i * 8 + 4..])),
            Self::Pairs(p) => p[i],
        }
    }

    /// The points in order.
    pub fn iter(&self) -> impl Iterator<Item = (f32, f32)> + '_ {
        (0..self.len()).map(move |i| self.get(i))
    }
}

impl std::fmt::Debug for Points<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl PartialEq for Points<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self.iter().zip(other.iter()).all(|(a, b)| a.0.to_bits() == b.0.to_bits() && a.1.to_bits() == b.1.to_bits())
    }
}

/// A run of `f32` values (dash intervals).
#[derive(Clone, Copy)]
pub enum Floats<'a> {
    /// From the wire: `4 * n` bytes.
    Wire(&'a [u8]),
    /// From the caller's slice.
    Values(&'a [f32]),
}

impl<'a> Floats<'a> {
    /// Number of values.
    pub fn len(&self) -> usize {
        match self {
            Self::Wire(b) => b.len() / 4,
            Self::Values(v) => v.len(),
        }
    }

    /// Whether there are no values.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Value `i`.
    pub fn get(&self, i: usize) -> f32 {
        match self {
            Self::Wire(b) => le_f32(&b[i * 4..]),
            Self::Values(v) => v[i],
        }
    }

    /// The values in order.
    pub fn iter(&self) -> impl Iterator<Item = f32> + '_ {
        (0..self.len()).map(move |i| self.get(i))
    }
}

impl std::fmt::Debug for Floats<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl PartialEq for Floats<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().zip(other.iter()).all(|(a, b)| a.to_bits() == b.to_bits())
    }
}

/// A run of gradient stops `(offset, colour)`.
#[derive(Clone, Copy)]
pub enum Stops<'a> {
    /// From the wire: `8 * n` bytes, `f32` offset then `u32` colour.
    Wire(&'a [u8]),
    /// From the caller's slice.
    Values(&'a [(f32, Rgba)]),
}

impl<'a> Stops<'a> {
    /// Number of stops.
    pub fn len(&self) -> usize {
        match self {
            Self::Wire(b) => b.len() / 8,
            Self::Values(v) => v.len(),
        }
    }

    /// Whether there are no stops.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Stop `i`.
    pub fn get(&self, i: usize) -> (f32, Rgba) {
        match self {
            Self::Wire(b) => {
                let o = le_f32(&b[i * 8..]);
                let c = u32::from_le_bytes([b[i * 8 + 4], b[i * 8 + 5], b[i * 8 + 6], b[i * 8 + 7]]);
                (o, Rgba::from_u32(c))
            }
            Self::Values(v) => v[i],
        }
    }

    /// The stops in order.
    pub fn iter(&self) -> impl Iterator<Item = (f32, Rgba)> + '_ {
        (0..self.len()).map(move |i| self.get(i))
    }
}

impl std::fmt::Debug for Stops<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl PartialEq for Stops<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self.iter().zip(other.iter()).all(|(a, b)| a.0.to_bits() == b.0.to_bits() && a.1 == b.1)
    }
}

// ---------------------------------------------------------------------------
// Operand kinds
// ---------------------------------------------------------------------------

/// How one kind of operand is written, read and handed to the builder.
pub trait OperandKind {
    /// The name used in [`COMMANDS`] and in the documentation table.
    const KIND: &'static str;
    /// What the decoded command holds.
    type Value<'a>;
    /// What the builder method takes.
    type Arg<'a>;
    /// From a builder argument to the held value.
    fn from_arg<'a>(a: Self::Arg<'a>) -> Self::Value<'a>;
    /// Append the wire form.
    fn encode(v: &Self::Value<'_>, out: &mut Vec<u8>);
    /// Read the wire form.
    fn decode<'a>(r: &mut Reader<'a>) -> Result<Self::Value<'a>>;
}

macro_rules! scalar_kind {
    ($name:ident, $kind:literal, $ty:ty, $put:expr, $get:expr) => {
        #[doc = concat!("The `", $kind, "` operand.")]
        pub struct $name;
        impl OperandKind for $name {
            const KIND: &'static str = $kind;
            type Value<'a> = $ty;
            type Arg<'a> = $ty;
            fn from_arg<'a>(a: Self::Arg<'a>) -> Self::Value<'a> {
                a
            }
            fn encode(v: &Self::Value<'_>, out: &mut Vec<u8>) {
                ($put)(out, *v)
            }
            fn decode<'a>(r: &mut Reader<'a>) -> Result<Self::Value<'a>> {
                ($get)(r)
            }
        }
    };
}

scalar_kind!(U8, "u8", u8, |o: &mut Vec<u8>, v: u8| o.push(v), |r: &mut Reader<'_>| r.u8());
scalar_kind!(U32, "u32", u32, put_u32, |r: &mut Reader<'_>| r.u32());
scalar_kind!(F32, "f32", f32, put_f32, |r: &mut Reader<'_>| r.f32());

/// The `rgba` operand: a `u32` `0xRRGGBBAA`.
pub struct Color;
impl OperandKind for Color {
    const KIND: &'static str = "rgba";
    type Value<'a> = Rgba;
    type Arg<'a> = Rgba;
    fn from_arg<'a>(a: Self::Arg<'a>) -> Self::Value<'a> {
        a
    }
    fn encode(v: &Self::Value<'_>, out: &mut Vec<u8>) {
        put_u32(out, v.to_u32())
    }
    fn decode<'a>(r: &mut Reader<'a>) -> Result<Self::Value<'a>> {
        Ok(Rgba::from_u32(r.u32()?))
    }
}

/// The `str` operand: a `u32` byte length then that many UTF-8 bytes.
pub struct Str;
impl OperandKind for Str {
    const KIND: &'static str = "str";
    type Value<'a> = &'a str;
    type Arg<'a> = &'a str;
    fn from_arg<'a>(a: Self::Arg<'a>) -> Self::Value<'a> {
        a
    }
    fn encode(v: &Self::Value<'_>, out: &mut Vec<u8>) {
        put_u32(out, v.len() as u32);
        out.extend_from_slice(v.as_bytes());
    }
    fn decode<'a>(r: &mut Reader<'a>) -> Result<Self::Value<'a>> {
        let n = r.u32()? as usize;
        let b = r.take(n)?;
        std::str::from_utf8(b).map_err(|_| GraphicsError::bad_list("a text operand is not valid UTF-8"))
    }
}

/// The `points` operand: a `u32` count then `count` pairs of `f32`.
pub struct PointsK;
impl OperandKind for PointsK {
    const KIND: &'static str = "points";
    type Value<'a> = Points<'a>;
    type Arg<'a> = &'a [(f32, f32)];
    fn from_arg<'a>(a: Self::Arg<'a>) -> Self::Value<'a> {
        Points::Pairs(a)
    }
    fn encode(v: &Self::Value<'_>, out: &mut Vec<u8>) {
        put_u32(out, v.len() as u32);
        for (x, y) in v.iter() {
            put_f32(out, x);
            put_f32(out, y);
        }
    }
    fn decode<'a>(r: &mut Reader<'a>) -> Result<Self::Value<'a>> {
        let n = r.u32()? as usize;
        let bytes = n.checked_mul(8).ok_or_else(|| GraphicsError::bad_list("point count overflows"))?;
        Ok(Points::Wire(r.take(bytes)?))
    }
}

/// The `floats` operand: a `u32` count then `count` `f32`.
pub struct FloatsK;
impl OperandKind for FloatsK {
    const KIND: &'static str = "floats";
    type Value<'a> = Floats<'a>;
    type Arg<'a> = &'a [f32];
    fn from_arg<'a>(a: Self::Arg<'a>) -> Self::Value<'a> {
        Floats::Values(a)
    }
    fn encode(v: &Self::Value<'_>, out: &mut Vec<u8>) {
        put_u32(out, v.len() as u32);
        for x in v.iter() {
            put_f32(out, x);
        }
    }
    fn decode<'a>(r: &mut Reader<'a>) -> Result<Self::Value<'a>> {
        let n = r.u32()? as usize;
        let bytes = n.checked_mul(4).ok_or_else(|| GraphicsError::bad_list("value count overflows"))?;
        Ok(Floats::Wire(r.take(bytes)?))
    }
}

/// The `stops` operand: a `u32` count then `count` of (`f32` offset, `u32` rgba).
pub struct StopsK;
impl OperandKind for StopsK {
    const KIND: &'static str = "stops";
    type Value<'a> = Stops<'a>;
    type Arg<'a> = &'a [(f32, Rgba)];
    fn from_arg<'a>(a: Self::Arg<'a>) -> Self::Value<'a> {
        Stops::Values(a)
    }
    fn encode(v: &Self::Value<'_>, out: &mut Vec<u8>) {
        put_u32(out, v.len() as u32);
        for (o, c) in v.iter() {
            put_f32(out, o);
            put_u32(out, c.to_u32());
        }
    }
    fn decode<'a>(r: &mut Reader<'a>) -> Result<Self::Value<'a>> {
        let n = r.u32()? as usize;
        let bytes = n.checked_mul(8).ok_or_else(|| GraphicsError::bad_list("stop count overflows"))?;
        Ok(Stops::Wire(r.take(bytes)?))
    }
}

// ---------------------------------------------------------------------------
// The table
// ---------------------------------------------------------------------------

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

macro_rules! commands {
    (
        $(
            $(#[doc = $doc:literal])*
            $op:literal $name:ident $snake:ident { $( $f:ident : $t:ty ),* $(,)? }
        )*
    ) => {
        /// One decoded drawing command. See the module documentation of
        /// [`display_list`](crate::display_list) for the table.
        #[derive(Debug, Clone, PartialEq)]
        #[allow(missing_docs)]
        pub enum Command<'a> {
            $( $(#[doc = $doc])* $name { $( $f: <$t as OperandKind>::Value<'a> ),* } ),*
        }

        impl<'a> Command<'a> {
            /// The opcode byte of this command.
            pub fn opcode(&self) -> u8 {
                match self { $( Command::$name { .. } => $op ),* }
            }

            /// The command's name.
            pub fn name(&self) -> &'static str {
                match self { $( Command::$name { .. } => stringify!($name) ),* }
            }

            /// Append the wire form (opcode and operands).
            pub fn encode(&self, out: &mut Vec<u8>) {
                match self {
                    $( Command::$name { $( $f ),* } => {
                        out.push($op);
                        $( <$t as OperandKind>::encode($f, out); )*
                    } ),*
                }
            }

            /// Read one command.
            pub fn decode(r: &mut Reader<'a>) -> Result<Command<'a>> {
                let op = r.u8()?;
                match op {
                    $( $op => Ok(Command::$name { $( $f: <$t as OperandKind>::decode(r)? ),* }), )*
                    other => Err(GraphicsError::bad_list(format!(
                        "unknown command 0x{other:02X} at offset {}", r.position().saturating_sub(1)
                    ))),
                }
            }
        }

        /// Every command of version 1, in opcode order.
        pub const COMMANDS: &[CommandInfo] = &[
            $( CommandInfo {
                opcode: $op,
                name: stringify!($name),
                method: stringify!($snake),
                operands: &[ $( (stringify!($f), <$t as OperandKind>::KIND) ),* ],
                doc: concat!($($doc),*),
            } ),*
        ];

        impl super::DisplayListBuilder {
            $(
                #[doc = concat!("Record `", stringify!($name), "` (opcode ", stringify!($op), ").")]
                #[allow(clippy::too_many_arguments)]
                pub fn $snake(&mut self $( , $f: <$t as OperandKind>::Arg<'_> )* ) -> &mut Self {
                    let cmd = Command::$name { $( $f: <$t as OperandKind>::from_arg($f) ),* };
                    self.push(&cmd);
                    self
                }
            )*
        }
    };
}

commands! {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opcodes_are_unique_and_ascending() {
        let mut prev = 0u8;
        for c in COMMANDS {
            assert!(c.opcode > prev, "{} (0x{:02X}) is not above the previous opcode", c.name, c.opcode);
            prev = c.opcode;
        }
    }

    #[test]
    fn names_and_methods_are_unique() {
        let mut names: Vec<_> = COMMANDS.iter().map(|c| c.name).collect();
        names.sort();
        let n = names.len();
        names.dedup();
        assert_eq!(n, names.len());
        let mut methods: Vec<_> = COMMANDS.iter().map(|c| c.method).collect();
        methods.sort();
        let n = methods.len();
        methods.dedup();
        assert_eq!(n, methods.len());
    }

    #[test]
    fn a_command_round_trips() {
        let pts = [(1.0f32, 2.0f32), (3.5, -4.25)];
        let stops = [(0.0f32, Rgba::BLACK), (1.0, Rgba::WHITE)];
        let cmds = vec![
            Command::Save {},
            Command::FillRect { x: 1.0, y: 2.0, w: 3.0, h: 4.0 },
            Command::DrawPolygon { points: Points::Pairs(&pts), mode: 7 },
            Command::SetLinearGradient { target: 0, x0: 0.0, y0: 0.0, x1: 10.0, y1: 5.0, stops: Stops::Values(&stops) },
            Command::DrawText { text: "héllo 日本語", x: 1.0, y: 2.0, max_width: 0.0, mode: 1 },
            Command::SetDash { intervals: Floats::Values(&[4.0, 2.0]), offset: 1.0 },
        ];
        let mut bytes = Vec::new();
        for c in &cmds {
            c.encode(&mut bytes);
        }
        let mut r = Reader::new(&bytes);
        for c in &cmds {
            let back = Command::decode(&mut r).unwrap();
            assert_eq!(&back, c);
        }
        assert_eq!(r.remaining(), 0);
    }
}
