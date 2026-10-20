// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The display list: one versioned binary encoding of Canvas 2D drawing
//! (WASM-018 R6), defined once.
//!
//! A program's drawing methods **record commands**; the control **executes the
//! recorded list in one call** - on `Present` or `Flush`, at the end of an
//! `onFrame` handler, and before any read-back (`ToImage`, `GetPixel`,
//! `MeasureText`, `IsPointInPath`). In the interpreter the list is built in host
//! memory by [`DisplayListBuilder`]; the generated WebAssembly module builds the
//! very same bytes in its transport arena and hands them over with one host
//! call (WASM-017 R11). Every host decodes them with this module, so a command
//! means the same thing everywhere.
//!
//! # Encoding, version 1
//!
//! All numbers are little-endian. A list is a 16-byte header and a payload.
//!
//! | offset | size | field |
//! |---|---|---|
//! | 0 | 4 | magic `PRGD` |
//! | 4 | 2 | version, `1` |
//! | 6 | 2 | flags, must be `0` |
//! | 8 | 4 | number of commands |
//! | 12 | 4 | payload length in bytes (what follows the header, exactly) |
//!
//! The payload is that many commands back to back. A command is one **opcode**
//! byte followed by its operands, in the order of the table below, with no
//! padding:
//!
//! | kind | wire form |
//! |---|---|
//! | `u8` | one byte |
//! | `u32` | 4 bytes |
//! | `f32` | 4 bytes, IEEE-754 binary32 |
//! | `rgba` | a `u32` holding `0xRRGGBBAA` |
//! | `str` | a `u32` byte length, then that many bytes of UTF-8 (no terminator) |
//! | `points` | a `u32` count, then count pairs of `f32` (x, y) |
//! | `floats` | a `u32` count, then count `f32` |
//! | `stops` | a `u32` count, then count of (`f32` offset, `rgba`) |
//!
//! Coordinates are **logical pixels** of the canvas: the executor multiplies by
//! the canvas `PixelRatio`. Angles are radians. A decoder refuses a list whose
//! header is wrong, whose payload length is not exact, that has more commands
//! or bytes than the [`Limits`](crate::limits::Limits) allow, that holds an
//! unknown opcode, that is truncated, or that has bytes left over after its
//! last command - and never panics on any input (the fuzz test in
//! `tests/display_list_robustness.rs` feeds it random bytes).
//!
//! # Commands
//!
//! | opcode | command | operands | meaning |
//! |---|---|---|---|
//! | 0x01 | Save | | push the graphics state |
//! | 0x02 | Restore | | pop the graphics state |
//! | 0x03 | Reset | | default state, empty stack, clip and path (pixels stay) |
//! | 0x04 | Clear | color:rgba | fill the whole canvas, ignoring transform, clip and blend mode |
//! | 0x05 | Present | | frame boundary |
//! | 0x06 | SetSmoothing | quality:u8 | image resampling 0 off, 1 low, 2 high |
//! | 0x10 | BeginPath | | new empty path |
//! | 0x11 | MoveTo | x:f32 y:f32 | start a sub-path |
//! | 0x12 | LineTo | x:f32 y:f32 | line |
//! | 0x13 | QuadTo | cx:f32 cy:f32 x:f32 y:f32 | quadratic curve |
//! | 0x14 | CubicTo | c1x:f32 c1y:f32 c2x:f32 c2y:f32 x:f32 y:f32 | cubic curve |
//! | 0x15 | ArcTo | x1:f32 y1:f32 x2:f32 y2:f32 radius:f32 | rounded corner (HTML `arcTo`) |
//! | 0x16 | Arc | x:f32 y:f32 radius:f32 start:f32 end:f32 ccw:u8 | circular arc |
//! | 0x17 | Ellipse | x:f32 y:f32 rx:f32 ry:f32 rotation:f32 start:f32 end:f32 ccw:u8 | elliptical arc |
//! | 0x18 | Rect | x:f32 y:f32 w:f32 h:f32 | rectangle sub-path |
//! | 0x19 | RoundRect | x:f32 y:f32 w:f32 h:f32 tl:f32 tr:f32 br:f32 bl:f32 | rounded rectangle sub-path |
//! | 0x1A | ClosePath | | close the sub-path |
//! | 0x1B | Fill | rule:u8 | fill the path (0 non-zero, 1 even-odd) |
//! | 0x1C | Stroke | | stroke the path |
//! | 0x1D | Clip | rule:u8 | intersect the clip with the path |
//! | 0x20 | FillRect | x:f32 y:f32 w:f32 h:f32 | fill a rectangle |
//! | 0x21 | StrokeRect | x:f32 y:f32 w:f32 h:f32 | stroke a rectangle |
//! | 0x22 | ClearRect | x:f32 y:f32 w:f32 h:f32 | make a rectangle transparent |
//! | 0x23 | DrawLine | x1:f32 y1:f32 x2:f32 y2:f32 | stroke a line |
//! | 0x24 | DrawCircle | x:f32 y:f32 radius:f32 mode:u8 | circle, mode bit 0 fill, bit 1 stroke |
//! | 0x25 | DrawPolygon | points:points mode:u8 | polygon, mode bit 0 fill, bit 1 stroke, bit 2 close |
//! | 0x30 | SetFillColor | color:rgba | solid fill |
//! | 0x31 | SetStrokeColor | color:rgba | solid stroke |
//! | 0x32 | SetLineWidth | width:f32 | stroke width |
//! | 0x33 | SetLineCap | cap:u8 | 0 butt, 1 round, 2 square |
//! | 0x34 | SetLineJoin | join:u8 | 0 miter, 1 round, 2 bevel |
//! | 0x35 | SetMiterLimit | limit:f32 | miter limit |
//! | 0x36 | SetDash | intervals:floats offset:f32 | dash pattern, empty for solid |
//! | 0x37 | SetAlpha | alpha:f32 | global alpha 0 to 1 |
//! | 0x38 | SetBlendMode | mode:u8 | compositing operation |
//! | 0x39 | SetLinearGradient | target:u8 x0:f32 y0:f32 x1:f32 y1:f32 stops:stops | linear gradient style |
//! | 0x3A | SetRadialGradient | target:u8 x0:f32 y0:f32 r0:f32 x1:f32 y1:f32 r1:f32 stops:stops | radial gradient style |
//! | 0x3B | SetConicGradient | target:u8 x:f32 y:f32 angle:f32 stops:stops | conic gradient style |
//! | 0x3C | SetPattern | target:u8 image:u32 repeat:u8 | image pattern style |
//! | 0x3D | SetShadow | color:rgba blur:f32 dx:f32 dy:f32 | shadow |
//! | 0x50 | Translate | x:f32 y:f32 | move the origin |
//! | 0x51 | Rotate | angle:f32 | rotate |
//! | 0x52 | Scale | x:f32 y:f32 | scale |
//! | 0x53 | Transform | a:f32 b:f32 c:f32 d:f32 e:f32 f:f32 | multiply by a matrix |
//! | 0x54 | SetTransform | a:f32 b:f32 c:f32 d:f32 e:f32 f:f32 | replace by a matrix |
//! | 0x55 | ResetTransform | | identity |
//! | 0x60 | SetFont | css:str | CSS font shorthand |
//! | 0x61 | SetTextAlign | align:u8 | 0 start, 1 end, 2 left, 3 right, 4 center |
//! | 0x62 | SetTextBaseline | baseline:u8 | 0 alphabetic, 1 top, 2 hanging, 3 middle, 4 ideographic, 5 bottom |
//! | 0x63 | SetTextDirection | direction:u8 | 0 auto, 1 ltr, 2 rtl |
//! | 0x64 | DrawText | text:str x:f32 y:f32 max_width:f32 mode:u8 | one line of text |
//! | 0x65 | DrawTextBox | text:str x:f32 y:f32 width:f32 line_height:f32 mode:u8 | wrapped text |
//! | 0x70 | DrawImage | image:u32 x:f32 y:f32 | image at natural size |
//! | 0x71 | DrawImageScaled | image:u32 x:f32 y:f32 w:f32 h:f32 | image scaled into a rectangle |
//! | 0x72 | DrawImageRegion | image:u32 sx:f32 sy:f32 sw:f32 sh:f32 dx:f32 dy:f32 dw:f32 dh:f32 | source rectangle into destination rectangle |
//! | 0x73 | DrawSprite | image:u32 frame:u32 frame_w:f32 frame_h:f32 dx:f32 dy:f32 dw:f32 dh:f32 | one frame of a sprite sheet |
//! | 0x80 | ReadPixel | slot:u32 x:f32 y:f32 | read-back: a pixel |
//! | 0x81 | Capture | slot:u32 x:f32 y:f32 w:f32 h:f32 | read-back: a rectangle as an image |
//! | 0x82 | HitPath | slot:u32 x:f32 y:f32 rule:u8 | read-back: point in path |
//! | 0x83 | HitStroke | slot:u32 x:f32 y:f32 | read-back: point on stroke |
//! | 0x84 | MeasureText | slot:u32 text:str | read-back: text metrics |
//!
//! Read-back commands do not draw: the executor answers them in the
//! [`Outcome`](crate::canvas::Outcome) under their `slot`, in the state the
//! commands before them left behind.
//!
//! # Compatibility
//!
//! Version 1 never changes meaning. A new command takes the next free opcode of
//! its group and a list that uses it declares the version that introduced it; a
//! decoder that does not know a version refuses the list instead of guessing.

mod builder;
pub mod consts;
mod list;
pub mod ops;

pub use builder::DisplayListBuilder;
pub use consts::*;
pub use list::{Commands, DisplayList, HEADER_LEN, MAGIC, VERSION};
pub use ops::{Command, CommandInfo, Floats, Points, Reader, Stops, COMMANDS};

#[cfg(test)]
mod tests {
    use super::*;

    /// The table in the module documentation above is hand-written for the
    /// reader; this keeps it equal to the macro-generated table.
    #[test]
    fn the_documentation_table_matches_the_command_table() {
        let src = include_str!("mod.rs");
        for c in COMMANDS {
            let operands: Vec<String> = c.operands.iter().map(|(n, k)| format!("{n}:{k}")).collect();
            let row = if operands.is_empty() {
                format!("//! | 0x{:02X} | {} | |", c.opcode, c.name)
            } else {
                format!("//! | 0x{:02X} | {} | {} |", c.opcode, c.name, operands.join(" "))
            };
            assert!(src.contains(&row), "the documentation table has no row {row:?}");
        }
        let rows = src.lines().filter(|l| l.starts_with("//! | 0x")).count();
        assert_eq!(rows, COMMANDS.len(), "the documentation table has rows the command table lacks");
    }
}
