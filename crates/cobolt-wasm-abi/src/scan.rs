// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Reading a program module without running it: its imports, exports, memories
//! and custom sections, and the checks WASM-002 AC4b, WASM-017 AC5/AC6 and the
//! loader's baseline rest on.
//!
//! This is what the bundle assembler uses to refuse a module that imports
//! anything beyond ABI v1 §3, that contains `memory.grow`, or whose single-memory
//! variant uses a multi-memory instruction. The browser loader performs the same
//! import/export checks in JavaScript (`WebAssembly.Module.imports/exports`);
//! this is the build-time twin, so a bad module never reaches a page.

use wasmparser::{Operator, Parser, Payload, TypeRef, Validator, WasmFeatures};

use crate::codes::{CONFORMANCE_IMPORTS, DEBUG_EXPORTS, IMPORT_MODULE, PRODUCTION_IMPORTS, REQUIRED_EXPORTS};
use crate::manifest::{Manifest, MANIFEST_SECTION};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportInfo {
    pub module: String,
    pub name: String,
    pub kind: ExternKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternKind {
    Func,
    Table,
    Memory,
    Global,
    Tag,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportInfo {
    pub name: String,
    pub kind: ExternKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryInfo {
    pub initial: u64,
    pub maximum: Option<u64>,
    pub shared: bool,
    pub memory64: bool,
}

/// Everything a build or a loader needs to know about a module before it runs.
#[derive(Debug, Clone, Default)]
pub struct ModuleScan {
    pub imports: Vec<ImportInfo>,
    pub exports: Vec<ExportInfo>,
    /// Memories the module defines (not imported ones).
    pub memories: Vec<MemoryInfo>,
    pub customs: Vec<(String, Vec<u8>)>,
    /// Does any function contain `memory.grow`? (WASM-017 R7: it must not.)
    pub has_memory_grow: bool,
    pub size: usize,
}

impl ModuleScan {
    pub fn custom(&self, name: &str) -> Option<&[u8]> {
        self.customs.iter().find(|(n, _)| n == name).map(|(_, d)| d.as_slice())
    }
    pub fn has_export(&self, name: &str) -> bool {
        self.exports.iter().any(|e| e.name == name)
    }
    /// The manifest embedded as the custom section `prmanifest`, parsed and validated.
    pub fn embedded_manifest(&self) -> Result<Manifest, String> {
        let bytes = self.custom(MANIFEST_SECTION).ok_or_else(|| format!("the module has no `{MANIFEST_SECTION}` custom section"))?;
        let text = core::str::from_utf8(bytes).map_err(|_| "the embedded manifest is not UTF-8".to_owned())?;
        Manifest::parse(text).map_err(|e| e.to_string())
    }
}

fn kind_of(t: &TypeRef) -> ExternKind {
    match t {
        TypeRef::Func(_) | TypeRef::FuncExact(_) => ExternKind::Func,
        TypeRef::Table(_) => ExternKind::Table,
        TypeRef::Memory(_) => ExternKind::Memory,
        TypeRef::Global(_) => ExternKind::Global,
        TypeRef::Tag(_) => ExternKind::Tag,
    }
}

fn ekind(k: wasmparser::ExternalKind) -> ExternKind {
    use wasmparser::ExternalKind as K;
    match k {
        K::Func | K::FuncExact => ExternKind::Func,
        K::Table => ExternKind::Table,
        K::Memory => ExternKind::Memory,
        K::Global => ExternKind::Global,
        K::Tag => ExternKind::Tag,
    }
}

/// Read a module's structure. Fails with a readable message on malformed bytes.
pub fn scan_module(bytes: &[u8]) -> Result<ModuleScan, String> {
    let mut out = ModuleScan { size: bytes.len(), ..Default::default() };
    for payload in Parser::new(0).parse_all(bytes) {
        let payload = payload.map_err(|e| format!("not a WebAssembly module: {e}"))?;
        match payload {
            Payload::ImportSection(r) => {
                for imp in r.into_imports() {
                    let imp = imp.map_err(|e| e.to_string())?;
                    out.imports.push(ImportInfo { module: imp.module.to_owned(), name: imp.name.to_owned(), kind: kind_of(&imp.ty) });
                }
            }
            Payload::ExportSection(r) => {
                for e in r {
                    let e = e.map_err(|e| e.to_string())?;
                    out.exports.push(ExportInfo { name: e.name.to_owned(), kind: ekind(e.kind) });
                }
            }
            Payload::MemorySection(r) => {
                for m in r {
                    let m = m.map_err(|e| e.to_string())?;
                    out.memories.push(MemoryInfo { initial: m.initial, maximum: m.maximum, shared: m.shared, memory64: m.memory64 });
                }
            }
            Payload::CustomSection(c) => out.customs.push((c.name().to_owned(), c.data().to_vec())),
            Payload::CodeSectionEntry(body) => {
                if !out.has_memory_grow {
                    let mut ops = body.get_operators_reader().map_err(|e| e.to_string())?;
                    while !ops.eof() {
                        if let Operator::MemoryGrow { .. } = ops.read().map_err(|e| e.to_string())? {
                            out.has_memory_grow = true;
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// The feature set the single-memory variant and the plain player stand on
/// (WASM-017 R18): WebAssembly 2.0 without SIMD — mutable globals, sign
/// extension, saturating float-to-int, multi-value, bulk memory, reference
/// types. Chrome 91+, Firefox 79+ and Safari 15+ have all of it.
pub fn baseline_features() -> WasmFeatures {
    WasmFeatures::WASM1
        | WasmFeatures::BULK_MEMORY
        | WasmFeatures::REFERENCE_TYPES
        | WasmFeatures::SIGN_EXTENSION
        | WasmFeatures::SATURATING_FLOAT_TO_INT
        | WasmFeatures::MULTI_VALUE
}

/// Does the module validate under exactly `features`?
pub fn validates_with(bytes: &[u8], features: WasmFeatures) -> Result<(), String> {
    Validator::new_with_features(features).validate_all(bytes).map(|_| ()).map_err(|e| e.to_string())
}

/// A module that validates under the baseline uses no multi-memory instruction,
/// no second memory and nothing beyond WebAssembly 2.0 without SIMD.
pub fn validates_on_the_baseline(bytes: &[u8]) -> Result<(), String> {
    validates_with(bytes, baseline_features())
}

/// A module that validates under the baseline plus multi-memory.
pub fn validates_with_multi_memory(bytes: &[u8]) -> Result<(), String> {
    validates_with(bytes, baseline_features() | WasmFeatures::MULTI_MEMORY)
}

/// Everything wrong with a program module as a *bundle member*, in words; empty
/// when it is acceptable. `single_memory` selects which variant it must be.
pub fn bundle_problems(bytes: &[u8], single_memory: bool) -> Vec<String> {
    let mut problems = Vec::new();
    let scan = match scan_module(bytes) {
        Ok(s) => s,
        Err(e) => return vec![e],
    };
    for imp in &scan.imports {
        let allowed = imp.module == IMPORT_MODULE
            && imp.kind == ExternKind::Func
            && PRODUCTION_IMPORTS.contains(&imp.name.as_str());
        if allowed {
            continue;
        }
        if imp.module == IMPORT_MODULE && CONFORMANCE_IMPORTS.contains(&imp.name.as_str()) {
            problems.push(format!(
                "the module imports `{}.{}`, a conformance-build import (ABI §8): a conformance module is never bundled",
                imp.module, imp.name
            ));
        } else {
            problems.push(format!("the module imports `{}.{}`, which ABI v1 §3 does not allow", imp.module, imp.name));
        }
    }
    for req in REQUIRED_EXPORTS {
        if !scan.has_export(req) {
            problems.push(format!("the module does not export `{req}` (ABI v1 §4)"));
        }
    }
    for want in ["memory", "arena"] {
        if !scan.exports.iter().any(|e| e.name == want && e.kind == ExternKind::Memory) {
            problems.push(format!("the module does not export a memory named `{want}`"));
        }
    }
    for e in &scan.exports {
        let known = REQUIRED_EXPORTS.contains(&e.name.as_str())
            || DEBUG_EXPORTS.contains(&e.name.as_str())
            || e.name == "memory"
            || e.name == "arena";
        if !known {
            problems.push(format!("the module exports `{}`, which ABI v1 §4 does not list", e.name));
        }
    }
    if scan.has_memory_grow {
        problems.push("the module contains `memory.grow`: the data memory is fixed (WASM-017 R7)".into());
    }
    for (i, m) in scan.memories.iter().enumerate() {
        if m.maximum != Some(m.initial) {
            problems.push(format!(
                "memory {i} is not fixed: minimum {} pages, maximum {:?} (WASM-017 R7 wants them equal)",
                m.initial, m.maximum
            ));
        }
        if m.shared || m.memory64 {
            problems.push(format!("memory {i} is shared or 64-bit; ABI v1 uses plain 32-bit memories"));
        }
    }
    if single_memory {
        if scan.memories.len() != 1 {
            problems.push(format!("the single-memory variant defines {} memories, not 1", scan.memories.len()));
        }
        if let Err(e) = validates_on_the_baseline(bytes) {
            problems.push(format!("the single-memory variant uses something beyond the baseline (e.g. a multi-memory instruction): {e}"));
        }
    } else {
        if scan.memories.len() != 2 {
            problems.push(format!("the multi-memory variant defines {} memories, not 2", scan.memories.len()));
        }
        if let Err(e) = validates_with_multi_memory(bytes) {
            problems.push(format!("the multi-memory variant does not validate: {e}"));
        }
    }
    match scan.embedded_manifest() {
        Ok(m) => {
            if m.variant.conformance {
                problems.push("the manifest marks a conformance build; it is never bundled".into());
            }
            if m.variant.multi_memory == single_memory {
                problems.push(format!(
                    "the manifest says multi_memory = {} but the file is the {} variant",
                    m.variant.multi_memory,
                    if single_memory { "single-memory" } else { "multi-memory" }
                ));
            }
        }
        Err(e) => problems.push(e),
    }
    problems
}
