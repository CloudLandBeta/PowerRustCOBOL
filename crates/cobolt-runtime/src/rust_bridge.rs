// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Curated Rust-FFI bridge (spec 005, Phase 2).
//!
//! `REPOSITORY` binds a COBOL class name to a Rust type by its external name
//! (e.g. `RUST-STRING IS "Rust.String"`). A `USAGE OBJECT REFERENCE` item then
//! holds a **handle** — an id into [`RustBridge`]'s object table. The bridge owns
//! the live Rust objects: constructing one returns a handle, methods are
//! dispatched by `(type, method)`, and dropping a handle (or the whole bridge)
//! runs the Rust destructor, so there is a defined RAII lifecycle and no leak.
//!
//! This is a *curated* surface, not auto-generated bindings: a realistic first
//! cut covering `i32`/`i64`/`f64`/`bool`, `String`, and `Vec`. Method calls run
//! inside [`std::panic::catch_unwind`] so a panic in bridge code becomes a
//! [`BridgeError`] instead of unwinding through the interpreter.

use std::any::Any;
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// A value marshaled across the COBOL ↔ Rust boundary. The interpreter converts
/// to/from [`crate::value::CobolValue`] when calling the bridge (see T10).
#[derive(Debug, Clone, PartialEq)]
pub enum BridgeValue {
    /// No value (a `()`-like result, or an absent argument).
    Null,
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    /// A handle into the object table (an `OBJECT REFERENCE`).
    Handle(i64),
}

/// Why a bridge call failed.
#[derive(Debug, Clone, PartialEq)]
pub enum BridgeError {
    /// `REPOSITORY` named a Rust type the curated bridge does not implement.
    UnknownType(String),
    /// The type has no such method.
    UnknownMethod { type_name: String, method: String },
    /// Argument count/kind mismatch.
    BadArgs { method: String, detail: String },
    /// The handle id is not (or no longer) live.
    NoSuchHandle(i64),
    /// A method panicked; the unwinding was caught here.
    Panicked(String),
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeError::UnknownType(t) => write!(f, "unknown Rust type '{t}'"),
            BridgeError::UnknownMethod { type_name, method } => {
                write!(f, "'{type_name}' has no method '{method}'")
            }
            BridgeError::BadArgs { method, detail } => {
                write!(f, "bad arguments to '{method}': {detail}")
            }
            BridgeError::NoSuchHandle(id) => write!(f, "no such Rust handle {id}"),
            BridgeError::Panicked(what) => write!(f, "Rust call '{what}' panicked"),
        }
    }
}

/// One live Rust object plus the type it was created as (so methods dispatch).
struct BridgeObject {
    type_name: String,
    value: Box<dyn Any>,
}

/// The run-unit's table of live Rust objects referenced from COBOL.
///
/// Handles are stable ids; dropping one removes the object (running its `Drop`),
/// and dropping the `RustBridge` drops every remaining object.
#[derive(Default)]
pub struct RustBridge {
    objects: HashMap<i64, BridgeObject>,
    next_id: i64,
}

impl RustBridge {
    pub fn new() -> Self {
        Self { objects: HashMap::new(), next_id: 1 }
    }

    /// Number of live object handles. `0` ⇒ everything created has been dropped.
    pub fn live_count(&self) -> usize {
        self.objects.len()
    }

    /// Store a pre-built object under a fresh handle id.
    fn store(&mut self, type_name: &str, value: Box<dyn Any>) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        self.objects.insert(id, BridgeObject { type_name: type_name.to_owned(), value });
        id
    }

    /// Drop a handle, running the Rust destructor. Returns whether it existed.
    pub fn drop_handle(&mut self, id: i64) -> bool {
        self.objects.remove(&id).is_some()
    }

    /// Construct a curated Rust object. `type_name` is the `REPOSITORY` external
    /// name (e.g. `"Rust.String"`). Returns a [`BridgeValue::Handle`].
    pub fn create(&mut self, type_name: &str, args: &[BridgeValue]) -> Result<BridgeValue, BridgeError> {
        let value: Box<dyn Any> = match type_name {
            "Rust.String" => Box::new(string_new(args)?),
            "Rust.i8" | "Rust.i16" | "Rust.i32" | "Rust.i64" | "Rust.i128"
            | "Rust.isize" | "Rust.u8" | "Rust.u16" | "Rust.u32" | "Rust.u64"
            | "Rust.u128" | "Rust.usize" => Box::new(int_new(args)?),
            "Rust.f32" | "Rust.f64" => Box::new(float_new(args)?),
            "Rust.bool" => Box::new(bool_new(args)?),
            "Rust.Vec" => Box::new(Vec::<BridgeValue>::new()),
            other => return Err(BridgeError::UnknownType(other.to_owned())),
        };
        Ok(BridgeValue::Handle(self.store(type_name, value)))
    }

    /// Invoke a method on a live handle, marshaling `args` in and the result out.
    /// Runs inside `catch_unwind`, so a panic surfaces as [`BridgeError::Panicked`]
    /// rather than tearing down the interpreter.
    pub fn invoke(&mut self, id: i64, method: &str, args: &[BridgeValue]) -> Result<BridgeValue, BridgeError> {
        let obj = self.objects.get_mut(&id).ok_or(BridgeError::NoSuchHandle(id))?;
        let type_name = obj.type_name.clone();
        let value = &mut obj.value;
        let called = catch_unwind(AssertUnwindSafe(|| dispatch(&type_name, value, method, args)));
        match called {
            Ok(result) => result,
            Err(_) => Err(BridgeError::Panicked(format!("{type_name}::{method}"))),
        }
    }
}

/// Dispatch `method` to the concrete curated type behind `value`. Methods mutate
/// in place or return a plain [`BridgeValue`]; constructing *new* objects is done
/// through [`RustBridge::create`] (a method returning a fresh handle is future
/// work, to keep the borrow here simple).
fn dispatch(
    type_name: &str,
    value: &mut Box<dyn Any>,
    method: &str,
    args: &[BridgeValue],
) -> Result<BridgeValue, BridgeError> {
    match type_name {
        "Rust.String" => string_method(value.downcast_mut::<String>().unwrap(), method, args),
        "Rust.Vec" => {
            vec_method(value.downcast_mut::<Vec<BridgeValue>>().unwrap(), method, args)
        }
        "Rust.f32" | "Rust.f64" => float_method(value.downcast_mut::<f64>().unwrap(), method, args),
        "Rust.bool" => bool_method(value.downcast_mut::<bool>().unwrap(), method, args),
        // every integer width is stored as i64
        _ if type_name.starts_with("Rust.i") || type_name.starts_with("Rust.u") => {
            int_method(value.downcast_mut::<i64>().unwrap(), method, args)
        }
        other => Err(BridgeError::UnknownType(other.to_owned())),
    }
}

// ── Argument helpers ────────────────────────────────────────────────────────

fn arg_str(args: &[BridgeValue], i: usize, method: &str) -> Result<String, BridgeError> {
    match args.get(i) {
        Some(BridgeValue::Str(s)) => Ok(s.clone()),
        Some(other) => Ok(coerce_string(other)),
        None => Err(BridgeError::BadArgs { method: method.to_owned(), detail: format!("missing argument {i}") }),
    }
}

fn arg_int(args: &[BridgeValue], i: usize, method: &str) -> Result<i64, BridgeError> {
    match args.get(i) {
        Some(BridgeValue::Int(n)) => Ok(*n),
        Some(BridgeValue::Float(x)) => Ok(*x as i64),
        Some(BridgeValue::Bool(b)) => Ok(*b as i64),
        Some(other) => Err(BridgeError::BadArgs {
            method: method.to_owned(),
            detail: format!("argument {i} expected an integer, got {other:?}"),
        }),
        None => Err(BridgeError::BadArgs { method: method.to_owned(), detail: format!("missing argument {i}") }),
    }
}

fn coerce_string(v: &BridgeValue) -> String {
    match v {
        BridgeValue::Null => String::new(),
        BridgeValue::Int(n) => n.to_string(),
        BridgeValue::Float(x) => x.to_string(),
        BridgeValue::Bool(b) => b.to_string(),
        BridgeValue::Str(s) => s.clone(),
        BridgeValue::Handle(id) => format!("<handle {id}>"),
    }
}

// ── Constructors ──────────────────────────────────────────────────────────────

fn string_new(args: &[BridgeValue]) -> Result<String, BridgeError> {
    Ok(match args.first() {
        None | Some(BridgeValue::Null) => String::new(),
        Some(v) => coerce_string(v),
    })
}

fn int_new(args: &[BridgeValue]) -> Result<i64, BridgeError> {
    Ok(match args.first() {
        None | Some(BridgeValue::Null) => 0,
        Some(BridgeValue::Int(n)) => *n,
        Some(BridgeValue::Float(x)) => *x as i64,
        Some(BridgeValue::Bool(b)) => *b as i64,
        // A COBOL `VALUE "10"` arrives as a string literal — parse it.
        Some(BridgeValue::Str(s)) => s.trim().parse().unwrap_or(0),
        Some(BridgeValue::Handle(_)) => 0,
    })
}

fn float_new(args: &[BridgeValue]) -> Result<f64, BridgeError> {
    Ok(match args.first() {
        None | Some(BridgeValue::Null) => 0.0,
        Some(BridgeValue::Float(x)) => *x,
        Some(BridgeValue::Int(n)) => *n as f64,
        Some(BridgeValue::Str(s)) => s.trim().parse().unwrap_or(0.0),
        Some(BridgeValue::Bool(b)) => *b as i64 as f64,
        Some(BridgeValue::Handle(_)) => 0.0,
    })
}

fn bool_new(args: &[BridgeValue]) -> Result<bool, BridgeError> {
    Ok(match args.first() {
        None | Some(BridgeValue::Null) => false,
        Some(BridgeValue::Bool(b)) => *b,
        Some(BridgeValue::Int(n)) => *n != 0,
        Some(BridgeValue::Str(s)) => matches!(s.trim(), "1" | "true" | "TRUE" | "T" | "Y"),
        _ => false,
    })
}

// ── Method tables ──────────────────────────────────────────────────────────────

fn string_method(s: &mut String, method: &str, args: &[BridgeValue]) -> Result<BridgeValue, BridgeError> {
    Ok(match method {
        "len" => BridgeValue::Int(s.len() as i64), // bytes, like Rust's String::len
        "char_count" => BridgeValue::Int(s.chars().count() as i64),
        "is_empty" => BridgeValue::Bool(s.is_empty()),
        "as_str" | "to_string" | "clone" => BridgeValue::Str(s.clone()),
        "to_uppercase" => BridgeValue::Str(s.to_uppercase()),
        "to_lowercase" => BridgeValue::Str(s.to_lowercase()),
        "trim" => BridgeValue::Str(s.trim().to_owned()),
        "push_str" => { s.push_str(&arg_str(args, 0, method)?); BridgeValue::Null }
        "clear" => { s.clear(); BridgeValue::Null }
        "contains" => BridgeValue::Bool(s.contains(&arg_str(args, 0, method)?)),
        "starts_with" => BridgeValue::Bool(s.starts_with(&arg_str(args, 0, method)?)),
        "ends_with" => BridgeValue::Bool(s.ends_with(&arg_str(args, 0, method)?)),
        "replace" => BridgeValue::Str(s.replace(&arg_str(args, 0, method)?, &arg_str(args, 1, method)?)),
        _ => return Err(BridgeError::UnknownMethod { type_name: "Rust.String".into(), method: method.into() }),
    })
}

fn int_method(n: &mut i64, method: &str, args: &[BridgeValue]) -> Result<BridgeValue, BridgeError> {
    Ok(match method {
        "value" => BridgeValue::Int(*n),
        "to_string" => BridgeValue::Str(n.to_string()),
        "abs" => BridgeValue::Int(n.abs()),
        "add" => { *n += arg_int(args, 0, method)?; BridgeValue::Int(*n) }
        "sub" => { *n -= arg_int(args, 0, method)?; BridgeValue::Int(*n) }
        "mul" => { *n *= arg_int(args, 0, method)?; BridgeValue::Int(*n) }
        "div" => { *n /= arg_int(args, 0, method)?; BridgeValue::Int(*n) } // div-by-0 panics → caught
        "rem" => { *n %= arg_int(args, 0, method)?; BridgeValue::Int(*n) }
        "pow" => BridgeValue::Int(n.pow(arg_int(args, 0, method)?.max(0) as u32)),
        _ => return Err(BridgeError::UnknownMethod { type_name: "Rust.int".into(), method: method.into() }),
    })
}

fn float_method(x: &mut f64, method: &str, args: &[BridgeValue]) -> Result<BridgeValue, BridgeError> {
    Ok(match method {
        "value" => BridgeValue::Float(*x),
        "to_string" => BridgeValue::Str(x.to_string()),
        "abs" => BridgeValue::Float(x.abs()),
        "sqrt" => BridgeValue::Float(x.sqrt()),
        "floor" => BridgeValue::Float(x.floor()),
        "ceil" => BridgeValue::Float(x.ceil()),
        "round" => BridgeValue::Float(x.round()),
        "add" => { *x += arg_int(args, 0, method)? as f64; BridgeValue::Float(*x) }
        _ => return Err(BridgeError::UnknownMethod { type_name: "Rust.f64".into(), method: method.into() }),
    })
}

fn bool_method(b: &mut bool, method: &str, _args: &[BridgeValue]) -> Result<BridgeValue, BridgeError> {
    Ok(match method {
        "value" => BridgeValue::Bool(*b),
        "not" => { *b = !*b; BridgeValue::Bool(*b) }
        "to_string" => BridgeValue::Str(b.to_string()),
        _ => return Err(BridgeError::UnknownMethod { type_name: "Rust.bool".into(), method: method.into() }),
    })
}

fn vec_method(v: &mut Vec<BridgeValue>, method: &str, args: &[BridgeValue]) -> Result<BridgeValue, BridgeError> {
    Ok(match method {
        "len" => BridgeValue::Int(v.len() as i64),
        "is_empty" => BridgeValue::Bool(v.is_empty()),
        "clear" => { v.clear(); BridgeValue::Null }
        "push" => {
            v.push(args.first().cloned().unwrap_or(BridgeValue::Null));
            BridgeValue::Null
        }
        "pop" => v.pop().unwrap_or(BridgeValue::Null),
        "get" => {
            let i = arg_int(args, 0, method)?;
            usize::try_from(i).ok().and_then(|i| v.get(i)).cloned().unwrap_or(BridgeValue::Null)
        }
        _ => return Err(BridgeError::UnknownMethod { type_name: "Rust.Vec".into(), method: method.into() }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Increments a shared counter on construction, decrements on drop, so the
    /// test can prove the handle table actually runs destructors (no leak).
    struct DropProbe(Arc<AtomicUsize>);
    impl DropProbe {
        fn new(c: &Arc<AtomicUsize>) -> Self {
            c.fetch_add(1, Ordering::SeqCst);
            Self(c.clone())
        }
    }
    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn string_create_invoke_len() {
        let mut b = RustBridge::new();
        let h = b.create("Rust.String", &[BridgeValue::Str("hello".into())]).unwrap();
        let BridgeValue::Handle(id) = h else { panic!("expected handle") };
        assert_eq!(b.invoke(id, "len", &[]).unwrap(), BridgeValue::Int(5));
        assert_eq!(b.invoke(id, "to_uppercase", &[]).unwrap(), BridgeValue::Str("HELLO".into()));
        b.invoke(id, "push_str", &[BridgeValue::Str("!".into())]).unwrap();
        assert_eq!(b.invoke(id, "len", &[]).unwrap(), BridgeValue::Int(6));
    }

    #[test]
    fn handle_drop_has_no_leak() {
        let live = Arc::new(AtomicUsize::new(0));
        let mut b = RustBridge::new();
        let id = b.store("Test.Probe", Box::new(DropProbe::new(&live)));
        assert_eq!(live.load(Ordering::SeqCst), 1);
        assert_eq!(b.live_count(), 1);
        assert!(b.drop_handle(id));
        assert_eq!(live.load(Ordering::SeqCst), 0, "destructor did not run");
        assert_eq!(b.live_count(), 0);
    }

    #[test]
    fn dropping_bridge_drops_remaining_objects() {
        let live = Arc::new(AtomicUsize::new(0));
        {
            let mut b = RustBridge::new();
            b.store("Test.Probe", Box::new(DropProbe::new(&live)));
            b.store("Test.Probe", Box::new(DropProbe::new(&live)));
            assert_eq!(live.load(Ordering::SeqCst), 2);
        }
        assert_eq!(live.load(Ordering::SeqCst), 0, "objects leaked when bridge dropped");
    }

    #[test]
    fn method_panic_is_caught() {
        let mut b = RustBridge::new();
        let BridgeValue::Handle(id) = b.create("Rust.i64", &[BridgeValue::Int(10)]).unwrap() else {
            panic!("expected handle")
        };
        // Integer divide-by-zero panics inside the bridge; catch_unwind turns it
        // into an error and the interpreter keeps running.
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {})); // silence the panic message
        let r = b.invoke(id, "div", &[BridgeValue::Int(0)]);
        std::panic::set_hook(prev);
        assert!(matches!(r, Err(BridgeError::Panicked(_))), "got {r:?}");
        // The handle is still live and usable after a caught panic.
        assert_eq!(b.invoke(id, "value", &[]).unwrap(), BridgeValue::Int(10));
    }

    #[test]
    fn int_constructed_from_string_value() {
        // A COBOL `VALUE "10"` reaches the bridge as a string; the integer
        // constructor must parse it (regression: it used to fail to construct).
        let mut b = RustBridge::new();
        let BridgeValue::Handle(id) = b.create("Rust.i64", &[BridgeValue::Str("10".into())]).unwrap()
        else {
            panic!("expected handle")
        };
        assert_eq!(b.invoke(id, "add", &[BridgeValue::Int(5)]).unwrap(), BridgeValue::Int(15));
    }

    #[test]
    fn unknown_type_and_method_error() {
        let mut b = RustBridge::new();
        assert!(matches!(b.create("Rust.Nope", &[]), Err(BridgeError::UnknownType(_))));
        let BridgeValue::Handle(id) = b.create("Rust.bool", &[BridgeValue::Bool(true)]).unwrap() else {
            panic!()
        };
        assert!(matches!(
            b.invoke(id, "frobnicate", &[]),
            Err(BridgeError::UnknownMethod { .. })
        ));
        assert!(matches!(b.invoke(9999, "len", &[]), Err(BridgeError::NoSuchHandle(9999))));
    }
}
