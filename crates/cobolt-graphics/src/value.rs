// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What crosses the line between a COBOL program and this crate.
//!
//! A host (the interpreter, the generated module's glue, the player) turns a
//! method call `OBJ::"FillRect"(10 20 30 40)` into
//! [`Registry::invoke`](crate::registry::Registry::invoke) with a handle, the
//! method name and a slice of [`Value`]s, and turns the [`Value`] that comes back
//! into a data item. Every method is described once by a [`Method`] row - its name,
//! its parameters and what it returns - and that row is both the documentation
//! (the table in the crate docs is generated from the rows and a test keeps the two
//! equal) and the check the call goes through: a wrong number of arguments or an
//! argument of the wrong kind is refused with a sentence before any method runs.

use std::borrow::Cow;

use crate::color::Rgba;
use crate::error::{GraphicsError, Result};
use crate::handles::Handle;
use crate::registry::Registry;

/// One value passed to or returned by a method.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// Nothing (a method that returns no answer; an omitted optional argument).
    Nothing,
    /// A whole number: a handle, a count, a flag (0 or 1), a colour as `0xRRGGBBAA`.
    Int(i64),
    /// A number with decimals.
    Num(f64),
    /// Text.
    Text(String),
    /// A block of bytes: a file, an encoded image, a COBOL table.
    Bytes(Vec<u8>),
    /// A run of numbers: points, matrices, vertex data, the answer of a read-back.
    Floats(Vec<f64>),
    /// A run of whole numbers: triangle indices, handles, histogram counts.
    Ints(Vec<i64>),
}

impl Value {
    /// The kind, for messages.
    pub fn kind(&self) -> &'static str {
        match self {
            Value::Nothing => "nothing",
            Value::Int(_) => "a whole number",
            Value::Num(_) => "a number",
            Value::Text(_) => "text",
            Value::Bytes(_) => "a block of bytes",
            Value::Floats(_) => "a list of numbers",
            Value::Ints(_) => "a list of whole numbers",
        }
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}
impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::Int(v as i64)
    }
}
impl From<u64> for Value {
    fn from(v: u64) -> Self {
        Value::Int(v.min(i64::MAX as u64) as i64)
    }
}
impl From<u32> for Value {
    fn from(v: u32) -> Self {
        Value::Int(v as i64)
    }
}
impl From<usize> for Value {
    fn from(v: usize) -> Self {
        Value::Int(v as i64)
    }
}
impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Int(v as i64)
    }
}
impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Num(v)
    }
}
impl From<f32> for Value {
    fn from(v: f32) -> Self {
        Value::Num(v as f64)
    }
}
impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Text(v.to_string())
    }
}
impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Text(v)
    }
}
impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Value::Bytes(v)
    }
}
impl From<Vec<f64>> for Value {
    fn from(v: Vec<f64>) -> Self {
        Value::Floats(v)
    }
}
impl From<Vec<f32>> for Value {
    fn from(v: Vec<f32>) -> Self {
        Value::Floats(v.into_iter().map(|x| x as f64).collect())
    }
}
impl From<Vec<i64>> for Value {
    fn from(v: Vec<i64>) -> Self {
        Value::Ints(v)
    }
}
impl From<Vec<u32>> for Value {
    fn from(v: Vec<u32>) -> Self {
        Value::Ints(v.into_iter().map(|x| x as i64).collect())
    }
}
impl From<Rgba> for Value {
    fn from(c: Rgba) -> Self {
        Value::Int(c.to_u32() as i64)
    }
}

/// What kind of value a parameter takes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    /// A whole number.
    Int,
    /// A number (a whole number is accepted).
    Num,
    /// Text.
    Text,
    /// A block of bytes.
    Bytes,
    /// A list of numbers (a list of whole numbers is accepted).
    Floats,
    /// A list of whole numbers.
    Ints,
    /// A handle: a whole number from 1.
    Handle,
    /// A colour: a whole number `0xRRGGBBAA`, or text such as `#336699`, `#33669980` or `red`.
    Color,
    /// A 0 or 1 (any non-zero number counts as 1).
    Flag,
}

impl Ty {
    /// The word the documentation table uses.
    pub fn word(self) -> &'static str {
        match self {
            Ty::Int => "int",
            Ty::Num => "num",
            Ty::Text => "text",
            Ty::Bytes => "bytes",
            Ty::Floats => "numbers",
            Ty::Ints => "ints",
            Ty::Handle => "handle",
            Ty::Color => "color",
            Ty::Flag => "flag",
        }
    }
}

/// What a method gives back.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Ret {
    /// Nothing.
    Nothing,
    /// A whole number.
    Int,
    /// A number.
    Num,
    /// Text.
    Text,
    /// A block of bytes.
    Bytes,
    /// A list of numbers.
    Floats,
    /// A list of whole numbers.
    Ints,
    /// A handle of something new (0 where there is none).
    Handle,
    /// A colour as `0xRRGGBBAA`.
    Color,
}

impl Ret {
    /// The word the documentation table uses.
    pub fn word(self) -> &'static str {
        match self {
            Ret::Nothing => "-",
            Ret::Int => "int",
            Ret::Num => "num",
            Ret::Text => "text",
            Ret::Bytes => "bytes",
            Ret::Floats => "numbers",
            Ret::Ints => "ints",
            Ret::Handle => "handle",
            Ret::Color => "color",
        }
    }
}

/// One parameter of a method.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Param {
    /// Its name.
    pub name: &'static str,
    /// What it takes.
    pub ty: Ty,
    /// Whether it may be left out (optional parameters come last).
    pub optional: bool,
}

/// One row of an object's method table.
pub struct Method<T> {
    /// The COBOL method name (`FillRect`). Matched without regard to case.
    pub name: &'static str,
    /// The parameters in order.
    pub params: &'static [Param],
    /// What comes back.
    pub returns: Ret,
    /// One line saying what it does.
    pub doc: &'static str,
    /// The implementation; the arguments have already been checked against `params`.
    pub call: fn(&mut T, &mut Registry, &Args<'_>) -> Result<Value>,
}

impl<T> std::fmt::Debug for Method<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Method({})", self.name)
    }
}

/// The checked arguments of one call, with typed accessors that name the method and the position when something is wrong.
pub struct Args<'a> {
    pub(crate) object: &'static str,
    pub(crate) method: &'a str,
    pub(crate) values: &'a [Value],
}

impl<'a> Args<'a> {
    /// Arguments for `object.method` (the registry builds these; tests may too).
    pub fn new(object: &'static str, method: &'a str, values: &'a [Value]) -> Self {
        Self { object, method, values }
    }

    /// How many were given.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether none were given.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    fn bad(&self, i: usize, message: impl Into<String>) -> GraphicsError {
        GraphicsError::BadArgument { method: format!("{}.{}", self.object, self.method), index: i, message: message.into() }
    }

    fn present(&self, i: usize) -> Result<&'a Value> {
        match self.values.get(i) {
            Some(Value::Nothing) | None => Err(self.bad(i, "this argument is required")),
            Some(v) => Ok(v),
        }
    }

    /// Whether argument `i` was given (and is not `Nothing`).
    pub fn has(&self, i: usize) -> bool {
        !matches!(self.values.get(i), Some(Value::Nothing) | None)
    }

    /// A whole number. A number with decimals is refused rather than rounded.
    pub fn int(&self, i: usize) -> Result<i64> {
        match self.present(i)? {
            Value::Int(v) => Ok(*v),
            Value::Num(v) if v.is_finite() && v.fract() == 0.0 && v.abs() < 9.0e15 => Ok(*v as i64),
            other => Err(self.bad(i, format!("needs a whole number, not {}", other.kind()))),
        }
    }

    /// A whole number in `[lo, hi]`.
    pub fn int_in(&self, i: usize, lo: i64, hi: i64) -> Result<i64> {
        let v = self.int(i)?;
        if v < lo || v > hi {
            return Err(self.bad(i, format!("must be from {lo} to {hi}, not {v}")));
        }
        Ok(v)
    }

    /// A whole number in `[lo, hi]`, or the default when omitted.
    pub fn int_in_or(&self, i: usize, default: i64, lo: i64, hi: i64) -> Result<i64> {
        if self.has(i) {
            self.int_in(i, lo, hi)
        } else {
            Ok(default)
        }
    }

    /// A `u32`.
    pub fn u32(&self, i: usize) -> Result<u32> {
        self.int_in(i, 0, u32::MAX as i64).map(|v| v as u32)
    }

    /// A count or size.
    pub fn usize(&self, i: usize) -> Result<usize> {
        self.int_in(i, 0, u32::MAX as i64).map(|v| v as usize)
    }

    /// A handle (1 or more).
    pub fn handle(&self, i: usize) -> Result<Handle> {
        self.int_in(i, 1, u32::MAX as i64).map(|v| v as u32)
    }

    /// A handle, or `None` when it is 0 or omitted ("no such thing").
    pub fn opt_handle(&self, i: usize) -> Result<Option<Handle>> {
        if !self.has(i) {
            return Ok(None);
        }
        match self.int_in(i, 0, u32::MAX as i64)? {
            0 => Ok(None),
            h => Ok(Some(h as u32)),
        }
    }

    /// A number.
    pub fn num(&self, i: usize) -> Result<f64> {
        let v = match self.present(i)? {
            Value::Int(v) => *v as f64,
            Value::Num(v) => *v,
            other => return Err(self.bad(i, format!("needs a number, not {}", other.kind()))),
        };
        if v.is_finite() {
            Ok(v)
        } else {
            Err(self.bad(i, "is not a finite number"))
        }
    }

    /// A number as `f32`.
    pub fn f32(&self, i: usize) -> Result<f32> {
        self.num(i).map(|v| v as f32)
    }

    /// A number or the default when omitted.
    pub fn num_or(&self, i: usize, default: f64) -> Result<f64> {
        if self.has(i) {
            self.num(i)
        } else {
            Ok(default)
        }
    }

    /// A whole number or the default when omitted.
    pub fn int_or(&self, i: usize, default: i64) -> Result<i64> {
        if self.has(i) {
            self.int(i)
        } else {
            Ok(default)
        }
    }

    /// A 0 or 1 (any non-zero number counts as 1).
    pub fn flag(&self, i: usize) -> Result<bool> {
        match self.present(i)? {
            Value::Int(v) => Ok(*v != 0),
            Value::Num(v) => Ok(*v != 0.0),
            other => Err(self.bad(i, format!("needs 0 or 1, not {}", other.kind()))),
        }
    }

    /// Text.
    pub fn text(&self, i: usize) -> Result<&'a str> {
        match self.present(i)? {
            Value::Text(s) => Ok(s),
            other => Err(self.bad(i, format!("needs text, not {}", other.kind()))),
        }
    }

    /// Text or the default when omitted.
    pub fn text_or(&self, i: usize, default: &'a str) -> Result<&'a str> {
        if self.has(i) {
            self.text(i)
        } else {
            Ok(default)
        }
    }

    /// A block of bytes.
    pub fn bytes(&self, i: usize) -> Result<&'a [u8]> {
        match self.present(i)? {
            Value::Bytes(b) => Ok(b),
            Value::Text(s) => Ok(s.as_bytes()),
            other => Err(self.bad(i, format!("needs a block of bytes, not {}", other.kind()))),
        }
    }

    /// A list of numbers (a list of whole numbers is converted).
    pub fn floats(&self, i: usize) -> Result<Cow<'a, [f64]>> {
        match self.present(i)? {
            Value::Floats(v) => {
                if v.iter().all(|x| x.is_finite()) {
                    Ok(Cow::Borrowed(v))
                } else {
                    Err(self.bad(i, "holds a number that is not finite"))
                }
            }
            Value::Ints(v) => Ok(Cow::Owned(v.iter().map(|x| *x as f64).collect())),
            other => Err(self.bad(i, format!("needs a list of numbers, not {}", other.kind()))),
        }
    }

    /// A list of numbers as `f32`.
    pub fn floats32(&self, i: usize) -> Result<Vec<f32>> {
        Ok(self.floats(i)?.iter().map(|x| *x as f32).collect())
    }

    /// A list of whole numbers.
    pub fn ints(&self, i: usize) -> Result<Cow<'a, [i64]>> {
        match self.present(i)? {
            Value::Ints(v) => Ok(Cow::Borrowed(v)),
            Value::Floats(v) if v.iter().all(|x| x.fract() == 0.0 && x.abs() < 9.0e15) => Ok(Cow::Owned(v.iter().map(|x| *x as i64).collect())),
            other => Err(self.bad(i, format!("needs a list of whole numbers, not {}", other.kind()))),
        }
    }

    /// A colour: `0xRRGGBBAA` as a whole number, or text.
    pub fn color(&self, i: usize) -> Result<Rgba> {
        match self.present(i)? {
            Value::Int(v) if (0..=u32::MAX as i64).contains(v) => Ok(Rgba::from_u32(*v as u32)),
            Value::Num(v) if v.fract() == 0.0 && (0.0..=u32::MAX as f64).contains(v) => Ok(Rgba::from_u32(*v as u32)),
            Value::Text(s) => Rgba::parse(s).ok_or_else(|| self.bad(i, format!("\"{s}\" is not a colour (try #RRGGBB, #RRGGBBAA or a name such as red)"))),
            other => Err(self.bad(i, format!("needs a colour (0xRRGGBBAA or text), not {}", other.kind()))),
        }
    }

    /// A colour, or the default when omitted.
    pub fn color_or(&self, i: usize, default: Rgba) -> Result<Rgba> {
        if self.has(i) {
            self.color(i)
        } else {
            Ok(default)
        }
    }

    /// An error for a value that is of the right kind but not an allowed one.
    pub fn refuse(&self, i: usize, message: impl Into<String>) -> GraphicsError {
        self.bad(i, message)
    }
}

/// Check `values` against `params`: the count, then each kind. Returns the method's own arguments.
pub fn check_args(object: &'static str, method: &str, params: &[Param], values: &[Value]) -> Result<()> {
    let required = params.iter().filter(|p| !p.optional).count();
    if values.len() < required || values.len() > params.len() {
        return Err(GraphicsError::ArgumentCount {
            method: format!("{object}.{method}"),
            expected: if values.len() < required { required } else { params.len() },
            got: values.len(),
        });
    }
    let args = Args::new(object, method, values);
    for (i, p) in params.iter().enumerate() {
        if i >= values.len() || (p.optional && !args.has(i)) {
            continue;
        }
        let ok = match p.ty {
            Ty::Int => args.int(i).is_ok(),
            Ty::Num => args.num(i).is_ok(),
            Ty::Text => args.text(i).is_ok(),
            Ty::Bytes => args.bytes(i).is_ok(),
            Ty::Floats => args.floats(i).is_ok(),
            Ty::Ints => args.ints(i).is_ok(),
            Ty::Handle => args.handle(i).is_ok(),
            Ty::Color => args.color(i).is_ok(),
            Ty::Flag => args.flag(i).is_ok(),
        };
        if !ok {
            // Ask the accessor again for its sentence.
            let e = match p.ty {
                Ty::Int => args.int(i).err(),
                Ty::Num => args.num(i).err(),
                Ty::Text => args.text(i).err(),
                Ty::Bytes => args.bytes(i).err(),
                Ty::Floats => args.floats(i).err(),
                Ty::Ints => args.ints(i).err(),
                Ty::Handle => args.handle(i).err(),
                Ty::Color => args.color(i).err(),
                Ty::Flag => args.flag(i).err(),
            };
            return Err(e.unwrap_or_else(|| args.bad(i, format!("{} is not {}", p.name, p.ty.word()))));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARAMS: &[Param] = &[
        Param { name: "x", ty: Ty::Num, optional: false },
        Param { name: "name", ty: Ty::Text, optional: false },
        Param { name: "color", ty: Ty::Color, optional: true },
    ];

    #[test]
    fn counts_are_checked_before_kinds() {
        let ok = [Value::Int(3), Value::Text("a".into())];
        assert!(check_args("Obj", "M", PARAMS, &ok).is_ok());
        let e = check_args("Obj", "M", PARAMS, &[Value::Int(3)]).unwrap_err();
        assert!(matches!(e, GraphicsError::ArgumentCount { expected: 2, got: 1, .. }), "{e:?}");
        let many = [Value::Int(1), Value::Text("a".into()), Value::Nothing, Value::Int(4)];
        assert!(matches!(check_args("Obj", "M", PARAMS, &many), Err(GraphicsError::ArgumentCount { .. })));
    }

    #[test]
    fn kinds_are_checked_with_a_sentence_naming_method_and_position() {
        let e = check_args("Obj", "M", PARAMS, &[Value::Text("x".into()), Value::Text("a".into())]).unwrap_err();
        assert_eq!(e.to_string(), GraphicsError::BadArgument { method: "Obj.M".into(), index: 0, message: "needs a number, not text".into() }.to_string());
        let e = check_args("Obj", "M", PARAMS, &[Value::Int(1), Value::Int(2)]).unwrap_err();
        assert!(e.to_string().contains("needs text"), "{e}");
        assert!(check_args("Obj", "M", PARAMS, &[Value::Num(f64::NAN), Value::Text("a".into())]).is_err(), "NaN is not a number a method may be given");
    }

    #[test]
    fn colours_come_as_numbers_or_text() {
        let vals = [Value::Int(0xFF8000FF), Value::Text("#336699".into()), Value::Text("red".into()), Value::Text("nonsense".into()), Value::Int(-1)];
        let a = Args::new("O", "M", &vals);
        assert_eq!(a.color(0).unwrap(), Rgba::new(255, 128, 0, 255));
        assert_eq!(a.color(1).unwrap(), Rgba::rgb(0x33, 0x66, 0x99));
        assert_eq!(a.color(2).unwrap(), Rgba::rgb(255, 0, 0));
        assert!(a.color(3).is_err());
        assert!(a.color(4).is_err());
        assert_eq!(a.color_or(9, Rgba::BLACK).unwrap(), Rgba::BLACK);
    }

    #[test]
    fn whole_numbers_do_not_silently_round() {
        let vals = [Value::Num(3.0), Value::Num(3.5), Value::Int(-2), Value::Int(i64::MAX)];
        let a = Args::new("O", "M", &vals);
        assert_eq!(a.int(0).unwrap(), 3);
        assert!(a.int(1).is_err());
        assert!(a.u32(2).is_err());
        assert!(a.handle(2).is_err());
        assert!(a.u32(3).is_err());
        assert!(a.opt_handle(7).unwrap().is_none());
    }

    #[test]
    fn lists_convert_between_whole_and_decimal() {
        let vals = [Value::Ints(vec![1, 2, 3]), Value::Floats(vec![1.0, 2.0]), Value::Floats(vec![1.5])];
        let a = Args::new("O", "M", &vals);
        assert_eq!(&*a.floats(0).unwrap(), &[1.0, 2.0, 3.0]);
        assert_eq!(&*a.ints(1).unwrap(), &[1, 2]);
        assert!(a.ints(2).is_err());
    }
}
