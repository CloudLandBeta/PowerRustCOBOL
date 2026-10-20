// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The methods of Canvas2D (WASM-018 R7): the drawing commands of the display list by
//! name, and the control methods around them.

use super::{dispatch, done, method};
use crate::canvas2d::Canvas2D;
use crate::display_list::{is_recordable, COMMANDS};
use crate::error::{GraphicsError, Result};
use crate::registry::{MethodInfo, ObjectKind, Registry};
use crate::value::Value;

/// The rows for the methods that are not display-list commands.
#[allow(unused_variables)]
pub(crate) static METHODS: &[crate::value::Method<Canvas2D>] = &[
    method!(Canvas2D, "Resize", [width: Int, height: Int], [pixel_ratio: Num], Nothing, "New size in logical pixels (and device pixel ratio); clears the picture and the recorded list.", |o, r, a| {
        let ratio = a.num_or(2, o.pixel_ratio() as f64)? as f32;
        o.resize(a.u32(0)?, a.u32(1)?, ratio)?;
        done()
    }),
    method!(Canvas2D, "SetPixelRatio", [ratio: Num], [], Nothing, "Change the device pixel ratio (clears the picture, like Resize).", |o, r, a| {
        let (w, h) = o.size();
        o.resize(w, h, a.f32(0)?)?;
        done()
    }),
    method!(Canvas2D, "GetWidth", [], [], Int, "The width in logical pixels.", |o, r, a| Ok(o.size().0.into())),
    method!(Canvas2D, "GetHeight", [], [], Int, "The height in logical pixels.", |o, r, a| Ok(o.size().1.into())),
    method!(Canvas2D, "GetPixelRatio", [], [], Num, "Device pixels per logical pixel.", |o, r, a| Ok(o.pixel_ratio().into())),
    method!(Canvas2D, "Flush", [], [], Int, "Draw everything recorded so far; the answer is how many commands ran.", |o, r, a| {
        let res = r.resources();
        Ok(o.flush(&res)?.commands.into())
    }),
    method!(Canvas2D, "Discard", [], [], Nothing, "Forget what was recorded without drawing it.", |o, r, a| {
        o.discard();
        done()
    }),
    method!(Canvas2D, "PendingCommands", [], [], Int, "How many commands are recorded and not yet drawn.", |o, r, a| Ok(o.pending_commands().into())),
    method!(Canvas2D, "HostCalls", [], [], Int, "How many times the rasteriser has been called (a flush is one call, however long the list).", |o, r, a| Ok((o.host_calls() as i64).into())),
    method!(Canvas2D, "TakePresented", [], [], Int, "1 if Present ran since the last call, else 0 (the host repaints when it is 1).", |o, r, a| Ok(o.take_presented().into())),
    method!(Canvas2D, "ToImage", [], [], Handle, "Draw what is recorded and return the whole picture as a new ImageBuffer.", |o, r, a| {
        let res = r.resources();
        let px = o.to_image(&res)?;
        Ok(r.add_image(px)?.into())
    }),
    method!(Canvas2D, "Capture", [x: Num, y: Num, width: Num, height: Num], [], Handle, "A rectangle of the picture as a new ImageBuffer (0 when the rectangle is empty).", |o, r, a| {
        let res = r.resources();
        match o.capture(&res, a.f32(0)?, a.f32(1)?, a.f32(2)?, a.f32(3)?)? {
            Some(px) => Ok(r.add_image(px)?.into()),
            None => Ok(0u32.into()),
        }
    }),
    method!(Canvas2D, "GetPixel", [x: Num, y: Num], [], Color, "The colour at a point as 0xRRGGBBAA, or -1 outside the canvas.", |o, r, a| {
        let res = r.resources();
        Ok(match o.get_pixel(&res, a.f32(0)?, a.f32(1)?)? {
            Some(c) => c.into(),
            None => Value::Int(-1),
        })
    }),
    method!(Canvas2D, "IsPointInPath", [x: Num, y: Num], [even_odd: Flag], Int, "1 if the point is inside the current path (0 non-zero rule, 1 even-odd).", |o, r, a| {
        let res = r.resources();
        let eo = if a.has(2) { a.flag(2)? } else { false };
        Ok(o.hit_path(&res, a.f32(0)?, a.f32(1)?, eo)?.into())
    }),
    method!(Canvas2D, "IsPointInStroke", [x: Num, y: Num], [], Int, "1 if the point is on the stroke of the current path.", |o, r, a| {
        let res = r.resources();
        Ok(o.hit_stroke(&res, a.f32(0)?, a.f32(1)?)?.into())
    }),
    method!(Canvas2D, "MeasureText", [text: Text], [], Floats, "Metrics of a text in the current font: width, ink left, ink right, ink ascent, ink descent, font ascent, font descent, lines.", |o, r, a| {
        let res = r.resources();
        let m = o.measure_text(&res, a.text(0)?)?;
        Ok(vec![m.width, m.actual_left, m.actual_right, m.actual_ascent, m.actual_descent, m.font_ascent, m.font_descent, m.lines as f32].into())
    }),
    method!(Canvas2D, "LoadFont", [bytes: Bytes], [], Int, "Register the faces of a font file (TTF, OTF, TTC); the answer is how many were added.", |o, r, a| load_font(r, a.bytes(0)?.to_vec())),
    method!(Canvas2D, "LoadFontAsset", [name: Text], [], Int, "Register the font in an asset by name.", |o, r, a| {
        let name = a.text(0)?;
        let bytes = r.assets().and_then(|h| h.asset(name)).ok_or_else(|| GraphicsError::Font(format!("there is no asset named {name}")))?;
        load_font(r, bytes)
    }),
    method!(Canvas2D, "GetDiagnostics", [], [], Text, "What was skipped since the last call (a missing image, no fonts ...), one per line.", |o, r, a| Ok(o.take_diagnostics().join("\n").into())),
];

fn load_font(r: &mut Registry, bytes: Vec<u8>) -> Result<Value> {
    #[cfg(feature = "text")]
    {
        let faces = r.fonts().add_font_data(bytes)?;
        Ok(faces.len().into())
    }
    #[cfg(not(feature = "text"))]
    {
        let _ = (r, bytes);
        Err(GraphicsError::Unsupported("text is not in this build".into()))
    }
}

pub(crate) fn call(o: &mut Canvas2D, reg: &mut Registry, method: &str, args: &[Value]) -> Result<Value> {
    if METHODS.iter().any(|m| m.name.eq_ignore_ascii_case(method)) {
        return dispatch(METHODS, "Canvas2D", o, reg, method, args);
    }
    // Every other name is a drawing command of the display list.
    o.record(method, args)?;
    if COMMANDS.iter().find(|c| c.name.eq_ignore_ascii_case(method)).map(|c| c.name == "Present").unwrap_or(false) {
        // `Present` ends a frame: draw it now, in one call.
        let res = reg.resources();
        o.flush(&res)?;
    }
    done()
}

fn kind_word(k: &str) -> &'static str {
    match k {
        "u8" | "u32" => "int",
        "f32" => "num",
        "rgba" => "color",
        "str" => "text",
        _ => "numbers",
    }
}

pub(crate) fn infos() -> Vec<MethodInfo> {
    let mut v = super::infos(ObjectKind::Canvas2D, METHODS);
    for c in COMMANDS.iter().filter(|c| is_recordable(c.opcode)) {
        v.push(MethodInfo {
            object: ObjectKind::Canvas2D,
            name: c.name.to_string(),
            params: c.operands.iter().map(|(n, k)| (n.to_string(), kind_word(k), false)).collect(),
            returns: "-",
            doc: c.doc.trim().to_string(),
        });
    }
    v
}
