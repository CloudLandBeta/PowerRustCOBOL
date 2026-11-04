// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The command table: **the** single definition of display-list version 1.
//!
//! The table itself is written once, in `cobolt-graphics-wire`
//! (`with_command_table!`), with the operand encodings. Here it is expanded a
//! second time to generate the [`Command`] enum, the decoder, the encoder
//! (which calls the wire crate's operand writers) and the builder methods; the
//! wire crate expands it to [`COMMANDS`] and its own no-alloc encoder. Nothing
//! about the wire format is written anywhere else, so the encoder, the decoder
//! and the executor cannot drift apart.

use crate::color::Rgba;
use crate::error::{GraphicsError, Result};
use cobolt_graphics_wire as wire;
use wire::{FloatSeq, PointSeq, Sink, StopSeq};

pub use wire::{CommandInfo, COMMANDS};

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

/// A growable buffer as the wire crate's byte sink (it never runs out of room).
pub(crate) struct VecSink<'a>(pub(crate) &'a mut Vec<u8>);

impl Sink for VecSink<'_> {
    fn put(&mut self, bytes: &[u8]) -> std::result::Result<(), wire::Full> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}

/// Append helpers: the wire crate writes, the buffer grows.
pub(crate) fn put_u32(out: &mut Vec<u8>, v: u32) {
    let _ = wire::put_u32(&mut VecSink(out), v);
}

pub(crate) fn put_f32(out: &mut Vec<u8>, v: f32) {
    let _ = wire::put_f32(&mut VecSink(out), v);
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

impl PointSeq for Points<'_> {
    fn len(&self) -> usize {
        Points::len(self)
    }
    fn get(&self, i: usize) -> (f32, f32) {
        Points::get(self, i)
    }
}

impl FloatSeq for Floats<'_> {
    fn len(&self) -> usize {
        Floats::len(self)
    }
    fn get(&self, i: usize) -> f32 {
        Floats::get(self, i)
    }
}

impl StopSeq for Stops<'_> {
    fn len(&self) -> usize {
        Stops::len(self)
    }
    fn get(&self, i: usize) -> (f32, u32) {
        let (o, c) = Stops::get(self, i);
        (o, c.to_u32())
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

scalar_kind!(U8, "u8", u8, |o: &mut Vec<u8>, v: u8| { let _ = wire::put_u8(&mut VecSink(o), v); }, |r: &mut Reader<'_>| r.u8());
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
        let _ = wire::put_u32(&mut VecSink(out), v.to_u32());
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
        let _ = wire::put_str(&mut VecSink(out), v);
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
        let _ = wire::put_points(&mut VecSink(out), v);
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
        let _ = wire::put_floats(&mut VecSink(out), v);
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
        let _ = wire::put_stops(&mut VecSink(out), v);
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

wire::with_command_table!(commands);

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
