// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The COBOL-facing method tables: one module per object kind.
//!
//! Each module has a `static METHODS` of [`Method`](crate::value::Method) rows
//! (name, parameters, answer, one line of documentation, implementation) and two
//! functions: `call`, which finds the row, checks the arguments against it and runs
//! it, and `infos`, which lists the rows for the documentation. Because the row *is*
//! the dispatch, a method cannot be documented and missing, or working and
//! undocumented - and a test checks the table in the crate docs against `infos`.

use crate::error::{GraphicsError, Result};
use crate::registry::{MethodInfo, ObjectKind, Registry};
use crate::value::{check_args, Args, Method, Value};

pub(crate) mod canvas2d;
#[cfg(feature = "imagebuf")]
pub(crate) mod image;
#[cfg(feature = "plot")]
pub(crate) mod plot;
#[cfg(feature = "physics")]
pub(crate) mod physics2d;
#[cfg(feature = "physics")]
pub(crate) mod physics3d;
#[cfg(feature = "scene3d-core")]
pub(crate) mod scene3d;
#[cfg(feature = "scene3d")]
pub(crate) mod canvas3d;

/// One row of a table, written compactly. `[required]` and `[optional]` parameters are
/// `name: Kind` with the kinds of [`Ty`](crate::value::Ty); the answer is a [`Ret`](crate::value::Ret).
macro_rules! method {
    ($T:ty, $name:literal, [$($p:ident : $t:ident),* $(,)?], [$($op:ident : $ot:ident),* $(,)?], $ret:ident, $doc:literal, |$o:ident, $r:ident, $a:ident| $body:expr) => {
        $crate::value::Method::<$T> {
            name: $name,
            params: &[
                $($crate::value::Param { name: stringify!($p), ty: $crate::value::Ty::$t, optional: false },)*
                $($crate::value::Param { name: stringify!($op), ty: $crate::value::Ty::$ot, optional: true },)*
            ],
            returns: $crate::value::Ret::$ret,
            doc: $doc,
            call: |$o: &mut $T, $r: &mut $crate::registry::Registry, $a: &$crate::value::Args<'_>| -> $crate::error::Result<$crate::value::Value> { $body },
        }
    };
}
pub(crate) use method;

/// Find a row by name (case-insensitive), check the arguments, run it.
pub(crate) fn dispatch<T>(table: &[Method<T>], object: &'static str, obj: &mut T, reg: &mut Registry, name: &str, values: &[Value]) -> Result<Value> {
    let m = table.iter().find(|m| m.name.eq_ignore_ascii_case(name)).ok_or_else(|| GraphicsError::UnknownMethod { object, method: name.to_string() })?;
    check_args(object, m.name, m.params, values)?;
    (m.call)(obj, reg, &Args::new(object, m.name, values))
}

/// Describe a table's rows.
pub(crate) fn infos<T>(kind: ObjectKind, table: &[Method<T>]) -> Vec<MethodInfo> {
    table
        .iter()
        .map(|m| MethodInfo {
            object: kind,
            name: m.name.to_string(),
            params: m.params.iter().map(|p| (p.name.to_string(), p.ty.word(), p.optional)).collect(),
            returns: m.returns.word(),
            doc: m.doc.to_string(),
        })
        .collect()
}

/// A method that answers with nothing.
pub(crate) fn done() -> Result<Value> {
    Ok(Value::Nothing)
}
