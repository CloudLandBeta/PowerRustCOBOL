// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **SVG text to nodes, allow-listed** (WASM-013 R5).
//!
//! The icon catalogue writes its drawings as SVG text and the Mermaid renderer writes diagrams the
//! same way. The page may not insert text as markup, so this module *reads* that SVG and builds
//! [`VNode`]s from it: an element is kept only if it is in [`ELEMENTS`], an attribute only if it is in
//! [`ATTRIBUTES`] (and never `on*`, `href`, `style` or anything that names a resource outside the
//! document), and a `<style>` or `<script>` is not an element this reader knows, so it is not kept.
//! What it does not recognise is dropped, never passed through.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use super::VNode;

/// Elements an SVG may contain here.
pub const ELEMENTS: &[&str] = &[
    "svg", "g", "path", "circle", "ellipse", "rect", "line", "polyline", "polygon", "text", "tspan", "defs",
    "linearGradient", "radialGradient", "stop", "clipPath", "marker", "title", "desc", "filter", "feDropShadow",
    "feGaussianBlur", "feOffset", "feFlood", "feMerge", "feMergeNode", "mask", "pattern", "symbol",
];

/// Attributes kept (presentation and geometry). `style`, `class`, `id` are handled separately.
pub const ATTRIBUTES: &[&str] = &[
    "viewBox", "width", "height", "x", "y", "x1", "y1", "x2", "y2", "cx", "cy", "r", "rx", "ry", "d", "points",
    "transform", "fill", "stroke", "stroke-width", "stroke-linecap", "stroke-linejoin", "stroke-dasharray",
    "stroke-dashoffset", "stroke-opacity", "fill-opacity", "opacity", "fill-rule", "clip-rule", "font-family",
    "font-size", "font-weight", "font-style", "text-anchor", "dominant-baseline", "alignment-baseline",
    "letter-spacing", "text-decoration", "offset", "stop-color", "stop-opacity", "gradientUnits",
    "gradientTransform", "spreadMethod", "fx", "fy", "preserveAspectRatio", "marker-end", "marker-start",
    "marker-mid", "markerWidth", "markerHeight", "markerUnits", "refX", "refY", "orient", "clip-path", "mask",
    "filter", "dx", "dy", "stdDeviation", "flood-color", "flood-opacity", "in", "result", "patternUnits",
    "patternTransform", "maskUnits", "clipPathUnits", "xmlns", "role", "aria-label", "aria-hidden", "focusable",
    "vector-effect", "paint-order", "visibility", "display", "rotate", "textLength", "lengthAdjust", "color",
];

/// A value that would reach outside the document: `url(...)` to anything but a fragment, a script
/// URL, an external reference.
fn value_is_safe(v: &str) -> bool {
    let l = v.to_ascii_lowercase();
    if l.contains("javascript:") || l.contains("data:") || l.contains("expression(") || l.contains("@import") {
        return false;
    }
    let mut rest = l.as_str();
    while let Some(i) = rest.find("url(") {
        let after = rest[i + 4..].trim_start_matches([' ', '"', '\'']);
        if !after.starts_with('#') {
            return false;
        }
        rest = &after[1..];
    }
    true
}

/// SVG names are case-sensitive (`linearGradient`), and so is the match.
fn canonical_element(name: &str) -> Option<&'static str> {
    ELEMENTS.iter().find(|e| **e == name).copied()
}

fn canonical_attr(name: &str) -> Option<&'static str> {
    ATTRIBUTES.iter().find(|a| **a == name).copied()
}

fn element_of(e: &BytesStart<'_>) -> Option<VNode> {
    let name = std::str::from_utf8(e.local_name().as_ref()).ok()?.to_owned();
    let tag = canonical_element(&name)?;
    let mut n = VNode::svg(tag);
    for a in e.attributes().with_checks(false).flatten() {
        let key = match std::str::from_utf8(a.key.as_ref()) {
            Ok(k) => k.to_owned(),
            Err(_) => continue,
        };
        let Ok(val) = a.unescape_value() else { continue };
        let val = val.into_owned();
        // Event handlers and every namespaced attribute (`xlink:href`, `xml:base`, …) are refused.
        if key.starts_with("on") || key.contains(':') {
            continue;
        }
        if key == "style" {
            // Inline style: keep the declarations that are safe, through the CSSOM (never the attribute).
            for decl in val.split(';') {
                if let Some((k, v)) = decl.split_once(':') {
                    let (k, v) = (k.trim(), v.trim());
                    if !k.is_empty() && !v.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && value_is_safe(v) {
                        n = n.style(k.to_owned(), v.to_owned());
                    }
                }
            }
            continue;
        }
        if key == "class" {
            for c in val.split_whitespace() {
                if c.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                    n = n.class(c.to_owned());
                }
            }
            continue;
        }
        if key == "id" {
            if val.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':' || c == '.') {
                n = n.attr("id", val);
            }
            continue;
        }
        if let Some(k) = canonical_attr(&key) {
            if value_is_safe(&val) {
                n = n.attr(k, val);
            }
        }
    }
    Some(n)
}

/// Read `svg` and answer the `<svg>` root as a node, or `None` when it is not SVG, not well formed,
/// or nothing in it is on the allow-list. `max_nodes` bounds the work on hostile input.
pub fn svg_to_vnode(svg: &str, max_nodes: usize) -> Option<VNode> {
    let mut reader = Reader::from_str(svg);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<Option<VNode>> = Vec::new();
    let mut root: Option<VNode> = None;
    let mut skipped_depth = 0usize;
    let mut count = 0usize;
    loop {
        let ev = reader.read_event().ok()?;
        match ev {
            Event::Start(e) => {
                if skipped_depth > 0 {
                    skipped_depth += 1;
                    continue;
                }
                count += 1;
                if count > max_nodes {
                    return None;
                }
                match element_of(&e) {
                    Some(n) => stack.push(Some(n)),
                    None => skipped_depth = 1,
                }
            }
            Event::Empty(e) => {
                if skipped_depth > 0 {
                    continue;
                }
                count += 1;
                if count > max_nodes {
                    return None;
                }
                if let Some(n) = element_of(&e) {
                    match stack.last_mut() {
                        Some(Some(parent)) => parent.children.push(n),
                        _ => root = root.or(Some(n)),
                    }
                }
            }
            Event::End(_) => {
                if skipped_depth > 0 {
                    skipped_depth -= 1;
                    continue;
                }
                if let Some(Some(n)) = stack.pop() {
                    match stack.last_mut() {
                        Some(Some(parent)) => parent.children.push(n),
                        _ => root = root.or(Some(n)),
                    }
                }
            }
            Event::Text(t) => {
                if skipped_depth == 0 {
                    if let (Some(Some(parent)), Ok(txt)) = (stack.last_mut(), t.unescape()) {
                        if !txt.trim().is_empty() && matches!(parent.tag, "text" | "tspan" | "title" | "desc") {
                            parent.text = Some(match parent.text.take() {
                                Some(p) => p + &txt,
                                None => txt.into_owned(),
                            });
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    root.filter(|r| r.tag == "svg")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_catalogue_svg_becomes_nodes() {
        let svg = cobolt_forms::icons::icon_svg("doc-open", "#336699").expect("the catalogue has doc-open");
        let n = svg_to_vnode(&svg, 5000).expect("it reads");
        assert_eq!(n.tag, "svg");
        assert_eq!(n.attr_of("viewBox"), Some("0 0 24 24"));
        assert!(n.count("path") > 0, "{}", n.describe());
        let mut all = Vec::new();
        n.find_all(&|_| true, &mut all);
        assert!(all.iter().all(|x| ELEMENTS.contains(&x.tag)));
    }

    #[test]
    fn script_events_references_and_style_elements_are_dropped_not_passed_through() {
        let hostile = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10" onload="alert(1)">
            <script>alert(1)</script>
            <style>rect { fill: red }</style>
            <foreignObject><div>x</div></foreignObject>
            <a href="javascript:alert(1)"><rect width="5" height="5" fill="url(https://evil.example/x)" onclick="x()"/></a>
            <rect id="ok" width="5" height="5" fill="url(#g)" style="fill: red; background: url(https://evil.example/x)"/>
            <image href="https://evil.example/x.png"/>
            <use href="#ok"/>
        </svg>"##;
        let n = svg_to_vnode(hostile, 100).expect("the root survives");
        let text = n.describe();
        for bad in ["script", "style>", "foreignObject", "onload", "onclick", "javascript", "evil.example", "image", "<use", "href"] {
            assert!(!text.contains(bad), "{bad} survived: {text}");
        }
        let rect = n.find_tag("rect").expect("the rect is allowed");
        assert_eq!(rect.attr_of("fill"), Some("url(#g)"), "a fragment reference is kept");
        assert_eq!(rect.styles, vec![("fill".to_owned(), "red".to_owned())], "the safe declaration goes to the CSSOM, the one that loads a URL is dropped");
    }

    #[test]
    fn a_document_that_is_not_svg_or_is_too_big_is_refused() {
        assert!(svg_to_vnode("<div>hello</div>", 10).is_none());
        assert!(svg_to_vnode("not xml at all <", 10).is_none());
        let many = format!("<svg>{}</svg>", "<g/>".repeat(50));
        assert!(svg_to_vnode(&many, 10).is_none(), "more nodes than the bound");
        assert!(svg_to_vnode(&many, 100).is_some());
    }
}
