// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Mermaid part of the player** (WASM-016 R13).
//!
//! One function crosses the boundary: [`render`] takes the source of a flowchart or a sequence
//! diagram and answers the SVG the desktop Viewer would draw, as text. The player reads that text
//! with its allow-listed SVG reader and builds DOM nodes from it — nothing here, and nothing the
//! player does with the answer, inserts markup into the page.

use wasm_bindgen::prelude::*;

/// The diagram as SVG text, or the reason it cannot be drawn.
#[wasm_bindgen]
pub fn render(source: &str) -> Result<String, JsError> {
    cobolt_forms::viewer::render_mermaid_svg(source).map_err(|reason| JsError::new(&reason))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_flowchart_and_a_sequence_diagram_render_and_anything_else_says_why() {
        let svg = cobolt_forms::viewer::render_mermaid_svg("flowchart LR\n  A[Start] --> B[Done]\n").expect("a flowchart");
        assert!(svg.starts_with("<svg") && svg.contains("Start"), "{}", &svg[..svg.len().min(200)]);
        let seq = cobolt_forms::viewer::render_mermaid_svg("sequenceDiagram\n  A->>B: hello\n").expect("a sequence diagram");
        assert!(seq.contains("<svg"));
        let class = cobolt_forms::viewer::render_mermaid_svg("classDiagram\n  A <|-- B\n").unwrap_err();
        assert!(class.contains("not supported"), "{class}");
    }
}
