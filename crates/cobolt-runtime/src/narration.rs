// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! How a control narrates what it did, on wherever the program's `DISPLAY` goes: `[agent X]`,
//! `[search X]`, `[aws X]`. One function, so the interpreter and the desktop runner write the
//! same lines (DESKTOP-WASM-001 T48, plan C5).

/// A block of text under one label, one line per line of the text: `[kind control] label: text`;
/// `(empty)` for no text; a text of several lines under `[kind control] label:`, each line indented
/// by two spaces after the tag.
///
/// Verbatim: no clipping, no flattening, no summary. A payload or a reply is evidence, and evidence
/// that has been shortened cannot be compared against what the provider's own documentation says
/// should be there (operator, 2026-09-07).
pub fn block(kind: &str, control: &str, label: &str, text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec![format!("[{kind} {control}] {label}: (empty)")];
    }
    let lines: Vec<&str> = text.lines().collect();
    if let [one] = lines.as_slice() {
        return vec![format!("[{kind} {control}] {label}: {one}")];
    }
    let mut out = Vec::with_capacity(lines.len() + 1);
    out.push(format!("[{kind} {control}] {label}:"));
    out.extend(lines.iter().map(|line| format!("[{kind} {control}]   {line}")));
    out
}

#[cfg(test)]
mod tests {
    use super::block;

    #[test]
    fn a_block_is_one_line_or_a_label_over_its_lines() {
        let cases: [(&str, Vec<&str>); 3] = [
            ("", vec!["[aws S3-1] request: (empty)"]),
            ("PutObject {}", vec!["[aws S3-1] request: PutObject {}"]),
            ("a\nb", vec!["[aws S3-1] request:", "[aws S3-1]   a", "[aws S3-1]   b"]),
        ];
        for (text, want) in &cases {
            assert_eq!(block("aws", "S3-1", "request", text), *want, "{text:?}");
        }
        println!("── a_block_is_one_line_or_a_label_over_its_lines ──");
        println!("  texts: empty, one line, two lines; checks: {} of {} passed", cases.len(), cases.len());
    }
}
