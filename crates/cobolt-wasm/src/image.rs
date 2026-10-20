// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The initial image of a region: every `VALUE`, and the defaults the standard
//! leaves to the implementation (spaces for characters, zero for numbers).
//!
//! The image is built **by running the module's own routines natively** — the
//! `MOVE` of each value into its item, against a scratch memory — so the bytes
//! are exactly the ones a `MOVE` at run time would write. It is then stored as
//! runs: long stretches of one byte become a `memory.fill`, the rest a
//! `memory.copy` from a small literal blob, so a table of ten thousand blank
//! rows costs a few bytes in the module, not ten thousand.

use crate::error::{GenError, GenResult};
use crate::layout::{Item, Layout};
use crate::pool::{Pool, Sub};
use cobolt_ast::expr::{FigurativeConstant, Literal};
use cobolt_wasm_rt::fd::*;
use cobolt_wasm_rt::mem::TestMem;
use cobolt_wasm_rt::opd::{FIG_SPACE, FIG_ZERO};

/// One step of region initialisation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitOp {
    /// `memory.fill` of `len` bytes at region offset `off`.
    Fill { off: u32, len: u32, byte: u8 },
    /// `memory.copy` of `len` bytes from the literal blob at `blob_off`.
    Copy { off: u32, blob_off: u32, len: u32 },
}

pub struct Image {
    pub ops: Vec<InitOp>,
    pub blob: Vec<u8>,
    /// The raw bytes, for tests.
    pub bytes: Vec<u8>,
}

/// Fills shorter than this are cheaper as part of a copy.
const MIN_FILL: usize = 32;

fn lit_operand(pool: &mut Pool, lit: &Literal, to_numeric: bool, line: u32) -> GenResult<u32> {
    Ok(match lit {
        Literal::Integer(n) | Literal::IntegerDigits(n, _) => {
            if to_numeric {
                pool.opd_lit_num(*n as i128, 0)
            } else {
                // A numeric literal into a character item: its digits as written.
                let d = lit.integer_digits().unwrap();
                pool.opd_lit_alnum(d.as_bytes())
            }
        }
        Literal::Decimal(m, s) => pool.opd_lit_num(*m, *s),
        Literal::Float(f) => {
            let (m, s) = crate::lower::float_to_decimal_pub(*f);
            pool.opd_lit_num(m, s)
        }
        Literal::String(s) => pool.opd_lit_alnum(s.as_bytes()),
        Literal::Figurative(fc) => match fc {
            FigurativeConstant::Zero => pool.opd_fig(FIG_ZERO),
            FigurativeConstant::Space => pool.opd_fig(FIG_SPACE),
            FigurativeConstant::HighValue => pool.opd_fig(cobolt_wasm_rt::opd::FIG_HIGH),
            FigurativeConstant::LowValue => pool.opd_fig(cobolt_wasm_rt::opd::FIG_LOW),
            FigurativeConstant::Quote => pool.opd_fig(cobolt_wasm_rt::opd::FIG_QUOTE),
            FigurativeConstant::Null => pool.opd_fig(cobolt_wasm_rt::opd::FIG_NULL),
            FigurativeConstant::All(inner) => {
                let bytes: Vec<u8> = match inner.as_ref() {
                    Literal::String(s) => s.as_bytes().to_vec(),
                    other => other.integer_digits().unwrap_or_default().into_bytes(),
                };
                if bytes.is_empty() {
                    return Err(GenError::new(line, "ALL with an empty literal"));
                }
                pool.opd_all(&bytes)
            }
        },
        Literal::National(_) | Literal::Utf8(_) => {
            return Err(GenError::new(line, "national and UTF-8 VALUE clauses are not supported yet"))
        }
    })
}

/// Is `idx` or one of its ancestors a `REDEFINES` (storage shared with
/// another item, which owns the initial value)?
fn shares_storage(layout: &Layout, idx: usize) -> bool {
    let mut cur = Some(idx);
    while let Some(i) = cur {
        if layout.items[i].redefines.is_some() {
            return true;
        }
        cur = layout.items[i].parent;
    }
    false
}

/// Build the initial image of the whole region.
pub fn build(layout: &Layout, decimal_comma: bool, currency: char) -> GenResult<Image> {
    let size = layout.total as usize;
    let region: u32 = 64;
    let pool_base = region + ((size as u32 + 63) & !63) + 64;
    let mut pool = Pool::new(pool_base);
    // (sender operand, receiver operand)
    let mut jobs: Vec<(u32, u32)> = Vec::new();

    for (idx, item) in layout.items.iter().enumerate() {
        if item.level == 88 || item.level == 66 || item.hidden || item.is_group && item.value.is_none() {
            continue;
        }
        if item.external || shares_storage(layout, idx) {
            continue;
        }
        // An item inside a group that has its own VALUE is initialised by the
        // group's move first; its own VALUE (if any) then overrides.
        let chain = layout.occurs_chain(idx);
        let total: u64 = chain.iter().map(|c| c.1 as u64).product::<u64>().max(1);
        if total > 5_000_000 {
            return Err(GenError::new(item.span_line, "a table of more than five million entries cannot be initialised"));
        }
        let (src_for, numeric) = (default_or_value(&mut pool, item)?, item.fd.is_numeric());
        let _ = numeric;
        let mut counters = vec![0u32; chain.len()];
        'occ: loop {
            let mut off = item.off;
            for (k, c) in counters.iter().enumerate() {
                off += c * chain[k].0;
            }
            let fd = pool.fd_with_spec(&item.fd, item.pic_template.as_deref(), decimal_comma, currency);
            let dst = pool.opd_item(off, &fd, &[] as &[Sub], None);
            jobs.push((src_for, dst));
            // Advance the odometer (innermost level fastest).
            let mut k = chain.len();
            loop {
                if k == 0 {
                    break 'occ;
                }
                k -= 1;
                counters[k] += 1;
                if counters[k] < chain[k].1 {
                    break;
                }
                counters[k] = 0;
            }
        }
    }

    // Run the moves natively.
    let mem_size = pool_base as usize + pool.bytes.len() + 256;
    let mut mem = TestMem::new(mem_size);
    mem.put(pool_base, &pool.bytes);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // The safe entry points, not the exported ones: a panic must unwind.
        for (s, d) in &jobs {
            let snd = cobolt_wasm_rt::mv::sender(region, &cobolt_wasm_rt::opd::parse(*s));
            cobolt_wasm_rt::mv::move_to(region, &snd, &cobolt_wasm_rt::opd::parse(*d));
        }
    }));
    if outcome.is_err() {
        return Err(GenError::new(0, "internal error: building the initial image failed"));
    }
    let bytes = mem.get(region, size as u32).to_vec();
    drop(mem);

    // Compress into runs.
    let mut ops = Vec::new();
    let mut blob = Vec::new();
    let mut i = 0usize;
    let mut lit_start: Option<usize> = None;
    let flush = |ops: &mut Vec<InitOp>, blob: &mut Vec<u8>, start: usize, end: usize, bytes: &[u8]| {
        if end > start {
            let blob_off = blob.len() as u32;
            blob.extend_from_slice(&bytes[start..end]);
            ops.push(InitOp::Copy { off: start as u32, blob_off, len: (end - start) as u32 });
        }
    };
    while i < bytes.len() {
        let b = bytes[i];
        let mut j = i + 1;
        while j < bytes.len() && bytes[j] == b {
            j += 1;
        }
        if j - i >= MIN_FILL {
            if let Some(s) = lit_start.take() {
                flush(&mut ops, &mut blob, s, i, &bytes);
            }
            // Zero bytes need no fill: memory starts zeroed, but an instance
            // region is reused, so the fill is still required.
            ops.push(InitOp::Fill { off: i as u32, len: (j - i) as u32, byte: b });
        } else if lit_start.is_none() {
            lit_start = Some(i);
        }
        i = j;
    }
    if let Some(s) = lit_start.take() {
        flush(&mut ops, &mut blob, s, bytes.len(), &bytes);
    }
    Ok(Image { ops, blob, bytes })
}

/// The sender that initialises `item`: its `VALUE`, or the default.
fn default_or_value(pool: &mut Pool, item: &Item) -> GenResult<u32> {
    if let Some(v) = &item.value {
        return lit_operand(pool, v, item.fd.is_numeric(), item.span_line);
    }
    Ok(match item.fd.kind {
        K_NUM_DISPLAY | K_BINARY | K_COMP5 | K_PACKED | K_FLOAT32 | K_FLOAT64 | K_INDEX => pool.opd_fig(FIG_ZERO),
        _ => pool.opd_fig(FIG_SPACE),
    })
}
