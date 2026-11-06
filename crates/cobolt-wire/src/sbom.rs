// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The one writer of a software bill of materials (WASM-013 R10, T17): CycloneDX 1.5, JSON.
//!
//! The Web Build writes one beside the bundle and the Backend build writes one beside the binary; both
//! go through [`cyclonedx`], so the two cannot drift in shape. This module hashes nothing: the caller
//! hands in the SHA-256 it already has, which keeps the crate free of a hashing dependency.

use serde_json::{json, Value};

/// A file the build produced, with its SHA-256 (lower-case hex).
pub fn file_component(path: &str, sha256_hex: &str) -> Value {
    json!({"type": "file", "name": path, "hashes": [{"alg": "SHA-256", "content": sha256_hex}]})
}

/// A library the product links, with its version and licence.
pub fn library_component(name: &str, version: &str, license: &str) -> Value {
    json!({"type": "library", "name": name, "version": version, "licenses": [{"license": {"name": license}}]})
}

/// A bundled data file (a font, say) with its licence and no version.
pub fn data_component(name: &str, license: &str) -> Value {
    json!({"type": "data", "name": name, "licenses": [{"license": {"name": license}}]})
}

/// The document: `name` is the application, `components` come in the order given, `build_info` (when it
/// parses as JSON) is recorded as the `player-build` property.
pub fn cyclonedx(name: &str, components: Vec<Value>, build_info: Option<&str>) -> Value {
    let mut bom = json!({
        "bomFormat": "CycloneDX", "specVersion": "1.5", "version": 1,
        "metadata": {"component": {"type": "application", "name": name}},
        "components": components,
    });
    if let Some(info) = build_info {
        if let Ok(v) = serde_json::from_str::<Value>(info) {
            bom["metadata"]["properties"] = json!([{"name": "player-build", "value": v.to_string()}]);
        }
    }
    bom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_document_has_the_cyclonedx_shape_and_keeps_the_order_given() {
        let bom = cyclonedx("demo", vec![file_component("a.js", "ab"), library_component("serde", "1.0", "MIT"), data_component("Font", "OFL")], Some(r#"{"rust":"1.95"}"#));
        assert_eq!(bom["bomFormat"], "CycloneDX");
        assert_eq!(bom["specVersion"], "1.5");
        let names: Vec<&str> = bom["components"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["a.js", "serde", "Font"]);
        assert_eq!(bom["components"][0]["hashes"][0]["alg"], "SHA-256");
        assert_eq!(bom["metadata"]["properties"][0]["name"], "player-build");
    }

    #[test]
    fn a_build_info_that_is_not_json_is_left_out() {
        let bom = cyclonedx("demo", Vec::new(), Some("not json"));
        assert!(bom["metadata"].get("properties").is_none());
    }
}
