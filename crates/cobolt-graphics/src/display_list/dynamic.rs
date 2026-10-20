// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Recording a command by *name* from [`Value`]s - what a COBOL call
//! `CANVAS::"FillRect"(10 20 30 40)` becomes.
//!
//! The command table ([`COMMANDS`]) says each command's opcode and the kind of each
//! operand, so one generic encoder serves all of them; the typed builder methods
//! (`fill_rect(...)`) and this one write the same bytes, and a test proves it for
//! every command.

use super::builder::DisplayListBuilder;
use super::ops::{put_f32, put_u32, COMMANDS};
use crate::error::{GraphicsError, Result};
use crate::value::{Args, Value};

/// The commands a program may record directly: all but the read-back ones, which the
/// canvas offers as methods that answer (`GetPixel`, `MeasureText` ...).
pub fn is_recordable(opcode: u8) -> bool {
    opcode < 0x80
}

impl DisplayListBuilder {
    /// Record the command called `name` (case-insensitive) with these operands.
    ///
    /// A whole number or a number gives a `u8`, `u32` or `f32` operand, a colour (a number or
    /// text) an `rgba`, text a `str`, and lists of numbers `points` (x, y pairs), `floats` and
    /// `stops` (offset, colour pairs). Wrong counts and kinds are refused with a sentence.
    pub fn push_dynamic(&mut self, name: &str, values: &[Value]) -> Result<()> {
        let info = COMMANDS
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(name) && is_recordable(c.opcode))
            .ok_or_else(|| GraphicsError::UnknownMethod { object: "Canvas2D", method: name.to_string() })?;
        if values.len() != info.operands.len() {
            return Err(GraphicsError::ArgumentCount { method: format!("Canvas2D.{}", info.name), expected: info.operands.len(), got: values.len() });
        }
        let args = Args::new("Canvas2D", info.name, values);
        let mut out = vec![info.opcode];
        for (i, (_, kind)) in info.operands.iter().enumerate() {
            match *kind {
                "u8" => out.push(args.int_in(i, 0, 255)? as u8),
                "u32" => put_u32(&mut out, args.u32(i)?),
                "f32" => put_f32(&mut out, args.f32(i)?),
                "rgba" => put_u32(&mut out, args.color(i)?.to_u32()),
                "str" => {
                    let s = args.text(i)?;
                    if s.len() > u32::MAX as usize / 2 {
                        return Err(args.refuse(i, "is too long"));
                    }
                    put_u32(&mut out, s.len() as u32);
                    out.extend_from_slice(s.as_bytes());
                }
                "points" => {
                    let v = args.floats(i)?;
                    if v.len() % 2 != 0 {
                        return Err(args.refuse(i, "needs x and y for every point, so an even number of values"));
                    }
                    put_u32(&mut out, (v.len() / 2) as u32);
                    for x in v.iter() {
                        put_f32(&mut out, *x as f32);
                    }
                }
                "floats" => {
                    let v = args.floats(i)?;
                    put_u32(&mut out, v.len() as u32);
                    for x in v.iter() {
                        put_f32(&mut out, *x as f32);
                    }
                }
                "stops" => {
                    let v = args.floats(i)?;
                    if v.len() % 2 != 0 || v.iter().skip(1).step_by(2).any(|c| !(0.0..=u32::MAX as f64).contains(c) || c.fract() != 0.0) {
                        return Err(args.refuse(i, "needs an offset and a colour (a whole number 0xRRGGBBAA) for every stop"));
                    }
                    put_u32(&mut out, (v.len() / 2) as u32);
                    for pair in v.chunks_exact(2) {
                        put_f32(&mut out, pair[0] as f32);
                        put_u32(&mut out, pair[1] as u32);
                    }
                }
                other => unreachable!("the command table has an operand kind {other} the dynamic encoder does not know"),
            }
        }
        self.push_encoded(&out);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgba;

    #[test]
    fn a_command_recorded_by_name_is_byte_for_byte_the_typed_one() {
        let mut typed = DisplayListBuilder::new();
        typed
            .save()
            .set_fill_color(Rgba::new(10, 20, 30, 40))
            .fill_rect(1.0, 2.0, 3.0, 4.0)
            .draw_polygon(&[(0.0, 0.0), (5.0, 1.0), (2.0, 7.0)], 3)
            .set_dash(&[3.0, 1.0], 0.5)
            .draw_text("héllo", 1.0, 2.0, 0.0, 1)
            .set_linear_gradient(0, 0.0, 0.0, 10.0, 0.0, &[(0.0, Rgba::BLACK), (1.0, Rgba::WHITE)])
            .set_line_cap(1)
            .draw_image(7, 4.0, 5.0)
            .present();
        let mut dynamic = DisplayListBuilder::new();
        dynamic.push_dynamic("Save", &[]).unwrap();
        dynamic.push_dynamic("SetFillColor", &[Value::Text("#0A141E28".into())]).unwrap();
        dynamic.push_dynamic("fillrect", &[1.into(), 2.0.into(), 3.into(), 4.0.into()]).unwrap();
        dynamic.push_dynamic("DrawPolygon", &[vec![0.0, 0.0, 5.0, 1.0, 2.0, 7.0].into(), 3.into()]).unwrap();
        dynamic.push_dynamic("SetDash", &[vec![3.0, 1.0].into(), 0.5.into()]).unwrap();
        dynamic.push_dynamic("DrawText", &["héllo".into(), 1.into(), 2.into(), 0.into(), 1.into()]).unwrap();
        dynamic.push_dynamic("SetLinearGradient", &[0.into(), 0.into(), 0.into(), 10.into(), 0.into(), Value::Floats(vec![0.0, 0xFF as f64, 1.0, 0xFFFFFFFFu32 as f64])]).unwrap();
        dynamic.push_dynamic("SetLineCap", &[1.into()]).unwrap();
        dynamic.push_dynamic("DrawImage", &[7.into(), 4.into(), 5.into()]).unwrap();
        dynamic.push_dynamic("Present", &[]).unwrap();
        assert_eq!(typed.take().unwrap(), dynamic.take().unwrap());
    }

    #[test]
    fn every_recordable_command_can_be_recorded_by_name_with_plausible_arguments() {
        for c in COMMANDS.iter().filter(|c| is_recordable(c.opcode)) {
            let args: Vec<Value> = c
                .operands
                .iter()
                .map(|(_, kind)| match *kind {
                    "u8" | "u32" => Value::Int(1),
                    "f32" => Value::Num(2.5),
                    "rgba" => Value::Int(0x336699FF),
                    "str" => Value::Text("x".into()),
                    "points" => Value::Floats(vec![0.0, 0.0, 1.0, 1.0]),
                    "floats" => Value::Floats(vec![1.0, 2.0]),
                    "stops" => Value::Floats(vec![0.0, 255.0, 1.0, 65535.0]),
                    other => panic!("{other}"),
                })
                .collect();
            let mut b = DisplayListBuilder::new();
            b.push_dynamic(c.name, &args).unwrap_or_else(|e| panic!("{}: {e}", c.name));
            let bytes = b.take().unwrap();
            let list = crate::display_list::DisplayList::parse(&bytes, &crate::limits::Limits::default()).unwrap();
            list.validate().unwrap_or_else(|e| panic!("{}: {e}", c.name));
            assert_eq!(list.command_count(), 1, "{}", c.name);
        }
    }

    #[test]
    fn mistakes_are_refused_with_the_command_and_the_position() {
        let mut b = DisplayListBuilder::new();
        let e = b.push_dynamic("FillRect", &[1.into(), 2.into()]).unwrap_err();
        assert!(matches!(e, GraphicsError::ArgumentCount { expected: 4, got: 2, .. }), "{e:?}");
        let e = b.push_dynamic("FillRect", &["a".into(), 2.into(), 3.into(), 4.into()]).unwrap_err();
        assert!(e.to_string().contains("Canvas2D.FillRect") && e.to_string().contains("number"), "{e}");
        assert!(matches!(b.push_dynamic("Nope", &[]), Err(GraphicsError::UnknownMethod { .. })));
        assert!(matches!(b.push_dynamic("ReadPixel", &[1.into(), 1.into(), 1.into()]), Err(GraphicsError::UnknownMethod { .. })), "read-back commands are methods that answer, not recordable");
        assert!(b.push_dynamic("SetLineCap", &[300.into()]).is_err(), "a u8 operand is 0 to 255");
        assert!(b.push_dynamic("DrawPolygon", &[vec![1.0, 2.0, 3.0].into(), 1.into()]).is_err(), "odd number of coordinates");
        assert!(b.is_empty(), "nothing was recorded by the failed calls");
    }

    #[test]
    fn the_list_limits_apply_to_dynamic_commands_too() {
        let mut b = DisplayListBuilder::with_limits(crate::limits::Limits { max_display_list_commands: 2, ..Default::default() });
        for _ in 0..5 {
            b.push_dynamic("Save", &[]).unwrap();
        }
        assert!(b.take().unwrap_err().to_string().contains("max_display_list_commands"));
    }
}
