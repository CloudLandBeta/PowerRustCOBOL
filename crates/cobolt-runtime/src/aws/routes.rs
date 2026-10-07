// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The AWS route table (spec 078 R14–R17): which MCP server and tool serve
//! each control operation, and how COBOL arguments become the tool's input
//! and its answer becomes properties — all of it DATA, in `routes.toml`.
//!
//! This file knows the table's grammar (placeholders, filters, extractors)
//! and nothing about any service: no server, tool or launch command is named
//! in code (R15; a test greps for it). The grammar is documented at the top
//! of `routes.toml`, next to the data it describes.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use cobolt_mcp::{Content, ToolResult};
use serde::Deserialize;
use serde_json::Value;

use super::process::Launch;

/// The table as shipped with the product (R16).
pub const SHIPPED: &str = include_str!("routes.toml");

/// How a server is spoken to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    #[default]
    Handshake,
    Stateless,
}

/// How to start one server.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ServerDef {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub readonly_args: Vec<String>,
    #[serde(default)]
    pub write_args: Vec<String>,
    #[serde(default)]
    pub protocol: Protocol,
}

/// Whether an operation changes AWS state (R25, R26).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutating {
    Yes,
    No,
    /// Decided by the tool's own `readOnlyHint`.
    ByTool,
}

impl<'de> Deserialize<'de> for Mutating {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match toml::Value::deserialize(d)? {
            toml::Value::Boolean(true) => Ok(Mutating::Yes),
            toml::Value::Boolean(false) => Ok(Mutating::No),
            toml::Value::String(s) if s == "by-tool" => Ok(Mutating::ByTool),
            other => Err(serde::de::Error::custom(format!("mutating must be true, false or \"by-tool\", not {other}"))),
        }
    }
}

/// One control operation.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OpDef {
    pub server: String,
    /// A tool name template, or `$list` for "list the server's tools".
    pub tool: String,
    #[serde(default)]
    pub input: Option<toml::Value>,
    pub mutating: Mutating,
    #[serde(default)]
    pub result: BTreeMap<String, String>,
    /// The control's own completion event, raised before `onComplete`.
    pub event: String,
    /// How the answer becomes the control's rows, when the route shapes them.
    #[serde(default)]
    pub rows: Option<RowsDef>,
    /// When an answer the server did not flag as an error is one anyway.
    #[serde(default)]
    pub fail: Option<FailDef>,
}

/// How an answer becomes rows.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RowsDef {
    /// An extractor giving an array (one row per item), an object (one row
    /// per entry; `$key` names it) or, with `columns`, an array of arrays.
    pub from: String,
    /// A pointer into the whole answer to the column names, when each row is
    /// an array of values.
    #[serde(default)]
    pub columns: Option<String>,
    /// Field = pointer into the row; `a|b` takes the first one present.
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
    /// Pointer into the row = template; a row is kept when the value there
    /// equals the expanded template. An empty template keeps every row.
    #[serde(default, rename = "where")]
    pub filter: BTreeMap<String, String>,
}

/// An answer that reports failure in its own data.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FailDef {
    /// An extractor; the answer failed when it gives `equals` — or, without
    /// `equals`, anything at all.
    pub when: String,
    #[serde(default)]
    pub equals: Option<String>,
    /// An extractor giving the failure's text.
    pub text: String,
}

/// The whole table.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct Routes {
    #[serde(default)]
    pub servers: BTreeMap<String, ServerDef>,
    #[serde(default)]
    pub ops: BTreeMap<String, OpDef>,
}

impl Routes {
    pub fn parse(text: &str) -> Result<Routes, String> {
        toml::from_str(text).map_err(|e| format!("the AWS route table is not valid: {e}"))
    }

    /// The shipped table, parsed once.
    pub fn shipped() -> &'static Routes {
        static ROUTES: OnceLock<Routes> = OnceLock::new();
        ROUTES.get_or_init(|| Routes::parse(SHIPPED).expect("the shipped route table parses (a test pins it)"))
    }

    /// The table with a connection's override applied (R16): each server or
    /// operation the override names replaces the shipped one of that key.
    pub fn with_override(&self, override_toml: &str) -> Result<Routes, String> {
        if override_toml.trim().is_empty() {
            return Ok(self.clone());
        }
        let o = Routes::parse(override_toml).map_err(|e| format!("the connection's route override: {e}"))?;
        let mut merged = self.clone();
        merged.servers.extend(o.servers);
        merged.ops.extend(o.ops);
        Ok(merged)
    }

    pub fn op(&self, control_type: &str, method: &str) -> Option<&OpDef> {
        self.ops.get(&format!("{control_type}.{method}"))
    }
}

/// Where placeholders find their values.
pub struct Context<'a> {
    /// The COBOL arguments, 1-based in templates.
    pub args: &'a [String],
    pub prop: &'a dyn Fn(&str) -> String,
    pub connection: &'a dyn Fn(&str) -> String,
}

/// Why a template could not be expanded.
#[derive(Debug, Clone, PartialEq)]
pub enum ExpandError {
    /// A JSON argument was not valid JSON (R21): which argument, and why.
    InvalidJson { arg: usize, message: String },
    /// The template itself is wrong — a route-table bug.
    Template(String),
    /// A value is not what the tool takes (`|int` on text).
    BadValue(String),
}

impl std::fmt::Display for ExpandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExpandError::InvalidJson { arg, message } => {
                write!(f, "argument {arg} is not valid JSON: {message}")
            }
            ExpandError::Template(e) => write!(f, "the AWS route table is wrong: {e}"),
            ExpandError::BadValue(e) => write!(f, "{e}"),
        }
    }
}

/// A placeholder's value: text, a JSON value (after `|json`), or nothing at
/// all (an empty value after `|opt`: the input leaves the key out).
enum Piece {
    Text(String),
    Json(Value),
    Absent,
}

impl Piece {
    fn into_text(self) -> String {
        match self {
            Piece::Text(t) => t,
            Piece::Json(Value::String(s)) => s,
            Piece::Json(v) => v.to_string(),
            Piece::Absent => String::new(),
        }
    }
    fn into_json(self) -> Value {
        match self {
            Piece::Json(v) => v,
            other => Value::String(other.into_text()),
        }
    }
}

/// The index of the `}` closing the `{` at `open`.
fn closing(s: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Split at top-level `|` (not inside a nested placeholder).
fn split_filters(s: &str) -> Vec<&str> {
    let (mut parts, mut depth, mut start) = (Vec::new(), 0usize, 0usize);
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            '|' if depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

/// `[^A-Za-z0-9_]` -> `_`, and a leading `_` before a digit.
fn ident(s: &str) -> String {
    let mut out: String = s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// Evaluate one placeholder body (without its braces).
fn placeholder(body: &str, ctx: &Context) -> Result<Piece, ExpandError> {
    let parts = split_filters(body);
    let source = parts[0].trim();
    let mut arg_no = 0usize;
    let mut piece = if let Some(n) = source.strip_prefix("arg:") {
        arg_no = n.trim().parse().map_err(|_| ExpandError::Template(format!("bad argument number in {{{body}}}")))?;
        if arg_no == 0 {
            return Err(ExpandError::Template("arguments count from 1".into()));
        }
        Piece::Text(ctx.args.get(arg_no - 1).cloned().unwrap_or_default())
    } else if let Some(name) = source.strip_prefix("prop:") {
        Piece::Text((ctx.prop)(name.trim()))
    } else if let Some(field) = source.strip_prefix("Connection.") {
        Piece::Text((ctx.connection)(field.trim()))
    } else {
        return Err(ExpandError::Template(format!("unknown placeholder {{{body}}}")));
    };
    for filter in &parts[1..] {
        if matches!(piece, Piece::Absent) {
            break;
        }
        let filter = filter.trim();
        piece = if filter == "json" || filter.starts_with("json:") {
            let mut text = piece.into_text();
            if let (true, Some(default)) = (text.trim().is_empty(), filter.strip_prefix("json:")) {
                text = default.to_owned();
            }
            match serde_json::from_str::<Value>(text.trim()) {
                Ok(v) => Piece::Json(v),
                Err(e) => return Err(ExpandError::InvalidJson { arg: arg_no, message: e.to_string() }),
            }
        } else if filter == "opt" {
            let text = piece.into_text();
            if text.trim().is_empty() {
                Piece::Absent
            } else {
                Piece::Text(text)
            }
        } else if let Some(fallback) = filter.strip_prefix("or:") {
            let text = piece.into_text();
            Piece::Text(if text.trim().is_empty() { expand(fallback, ctx)? } else { text })
        } else if filter == "int" {
            let text = piece.into_text();
            match text.trim().parse::<i64>() {
                Ok(n) => Piece::Json(Value::from(n)),
                Err(_) => return Err(ExpandError::BadValue(format!("\"{}\" is not a whole number ({source})", text.trim()))),
            }
        } else if filter == "quote" {
            Piece::Text(Value::String(piece.into_text()).to_string())
        } else if let Some(key) = filter.strip_prefix("obj:") {
            let mut map = serde_json::Map::new();
            map.insert(key.trim().to_owned(), piece.into_json());
            Piece::Json(Value::Object(map))
        } else if filter == "text" {
            Piece::Text(piece.into_text())
        } else if filter == "ident" {
            Piece::Text(ident(&piece.into_text()))
        } else if let Some(prefix) = filter.strip_prefix("strip:") {
            let prefix = expand(prefix, ctx)?;
            let text = piece.into_text();
            Piece::Text(match text.strip_prefix(prefix.as_str()) {
                Some(rest) if !prefix.is_empty() => rest.to_owned(),
                _ => text,
            })
        } else {
            return Err(ExpandError::Template(format!("unknown filter |{filter}")));
        };
    }
    Ok(piece)
}

/// Expand every placeholder in `template` to text.
pub fn expand(template: &str, ctx: &Context) -> Result<String, ExpandError> {
    let mut out = String::new();
    let mut i = 0;
    while let Some(rel) = template[i..].find('{') {
        let open = i + rel;
        out.push_str(&template[i..open]);
        let close = closing(template, open)
            .ok_or_else(|| ExpandError::Template(format!("unclosed placeholder in {template}")))?;
        out.push_str(&placeholder(&template[open + 1..close], ctx)?.into_text());
        i = close + 1;
    }
    out.push_str(&template[i..]);
    Ok(out)
}

/// Expand a tool-input template into JSON. A string that is exactly one
/// placeholder ending in `|json` becomes that JSON value; other strings are
/// expanded as text; tables and arrays are walked.
pub fn expand_input(template: &toml::Value, ctx: &Context) -> Result<Value, ExpandError> {
    Ok(input_value(template, ctx)?.unwrap_or(Value::Null))
}

/// One input leaf; `None` when it is an empty `|opt` value, which leaves its
/// key out.
fn input_value(template: &toml::Value, ctx: &Context) -> Result<Option<Value>, ExpandError> {
    Ok(Some(match template {
        toml::Value::String(s) => {
            let t = s.trim();
            let whole = t.starts_with('{') && closing(t, 0) == Some(t.len() - 1);
            match whole.then(|| placeholder(&t[1..t.len() - 1], ctx)).transpose()? {
                Some(Piece::Json(v)) => v,
                Some(Piece::Text(text)) => Value::String(text),
                Some(Piece::Absent) => return Ok(None),
                None => Value::String(expand(s, ctx)?),
            }
        }
        toml::Value::Table(t) => {
            let mut map = serde_json::Map::new();
            for (k, v) in t {
                if let Some(v) = input_value(v, ctx)? {
                    map.insert(k.clone(), v);
                }
            }
            Value::Object(map)
        }
        toml::Value::Array(a) => {
            let mut out = Vec::new();
            for v in a {
                out.extend(input_value(v, ctx)?);
            }
            Value::Array(out)
        }
        toml::Value::Integer(n) => Value::from(*n),
        toml::Value::Float(f) => Value::from(*f),
        toml::Value::Boolean(b) => Value::from(*b),
        toml::Value::Datetime(d) => Value::String(d.to_string()),
    }))
}

/// How to start `server` for a connection (R25: read-only unless a control
/// on the connection allows writes).
pub fn launch_for(server: &ServerDef, ctx: &Context, allow_write: bool, profile: &str, region: &str) -> Result<Launch, ExpandError> {
    let mut args = server.args.iter().map(|a| expand(a, ctx)).collect::<Result<Vec<_>, _>>()?;
    let extra = if allow_write { &server.write_args } else { &server.readonly_args };
    for a in extra {
        args.push(expand(a, ctx)?);
    }
    let mut env = Vec::new();
    for (k, v) in &server.env {
        let v = expand(v, ctx)?;
        // An empty value means "not set" — e.g. no Lambda function list.
        if !v.trim().is_empty() {
            env.push((k.clone(), v));
        }
    }
    let opt = |s: &str| (!s.trim().is_empty()).then(|| s.trim().to_owned());
    Ok(Launch { command: expand(&server.command, ctx)?, args, env, profile: opt(profile), region: opt(region) })
}

/// Every text block of a result, joined.
pub fn result_text(r: &ToolResult) -> String {
    r.content.iter().filter_map(Content::as_text).collect::<Vec<_>>().join("\n")
}

/// A JSON value as a property's text: a string as itself, `null` as empty.
pub fn value_text(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(v) => v.to_string(),
    }
}

/// The answer as one JSON value: its structured content, else its text read
/// as JSON — the whole text, or else the last text block that is JSON (a
/// server that sends a message and then the data).
fn answer_json(r: &ToolResult) -> Option<Value> {
    if let Some(v) = &r.structured_content {
        return Some(v.clone());
    }
    if let Ok(v) = serde_json::from_str::<Value>(result_text(r).trim()) {
        return Some(v);
    }
    r.content.iter().rev().filter_map(Content::as_text).find_map(|t| serde_json::from_str::<Value>(t.trim()).ok())
}

/// The first of `a|b|…` present (and not `null`) under `v`.
fn first_pointer<'v>(v: &'v Value, alternatives: &str) -> Option<&'v Value> {
    alternatives.split('|').find_map(|p| v.pointer(p.trim()).filter(|x| !x.is_null()))
}

/// An extractor's JSON: `$json:/ptr` (alternatives allowed), or `$jsonseq:`
/// — a text of JSON values one after another, as an array.
pub fn extract_value(r: &ToolResult, expr: &str) -> Option<Value> {
    if let Some(ptr) = expr.strip_prefix("$json:") {
        let whole = answer_json(r)?;
        return if ptr.is_empty() { Some(whole) } else { first_pointer(&whole, ptr).cloned() };
    }
    if expr == "$jsonseq:" {
        let text = result_text(r);
        let values: Result<Vec<Value>, _> = serde_json::Deserializer::from_str(&text).into_iter::<Value>().collect();
        return values.ok().map(Value::Array);
    }
    None
}

/// The rows a route's `rows` shapes from an answer. `filter` holds the
/// `where` templates already expanded.
pub fn shape_rows(r: &ToolResult, def: &RowsDef, filter: &BTreeMap<String, String>) -> Vec<Value> {
    let Some(source) = extract_value(r, &def.from) else { return Vec::new() };
    let mut items: Vec<(Option<String>, Value)> = match source {
        Value::Array(a) => a.into_iter().map(|v| (None, v)).collect(),
        Value::Object(o) => o.into_iter().map(|(k, v)| (Some(k), v)).collect(),
        _ => Vec::new(),
    };
    if let Some(cols) = &def.columns {
        let names: Vec<String> = answer_json(r)
            .as_ref()
            .and_then(|w| first_pointer(w, cols))
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|c| value_text(Some(c))).collect())
            .unwrap_or_default();
        for (_, v) in &mut items {
            if let Value::Array(cells) = v {
                let row: serde_json::Map<String, Value> = names.iter().cloned().zip(cells.iter().cloned()).collect();
                *v = Value::Object(row);
            }
        }
    }
    items.retain(|(_, v)| filter.iter().all(|(ptr, want)| want.is_empty() || value_text(v.pointer(ptr)) == *want));
    if def.fields.is_empty() {
        return items.into_iter().map(|(_, v)| v).collect();
    }
    items
        .into_iter()
        .map(|(key, v)| {
            let row: serde_json::Map<String, Value> = def
                .fields
                .iter()
                .map(|(name, spec)| {
                    let value = if spec.trim() == "$key" {
                        key.clone().map(Value::String)
                    } else {
                        first_pointer(&v, spec).cloned()
                    };
                    (name.clone(), value.unwrap_or(Value::String(String::new())))
                })
                .collect();
            Value::Object(row)
        })
        .collect()
}

/// Apply one result extractor.
pub fn extract(r: &ToolResult, expr: &str) -> String {
    if expr == "$text" {
        return result_text(r);
    }
    if expr == "$isError" {
        return if r.is_error == Some(true) { "1" } else { "0" }.into();
    }
    if expr.starts_with("$json:") || expr == "$jsonseq:" {
        return value_text(extract_value(r, expr).as_ref());
    }
    if let Some(markers) = expr.strip_prefix("$after:") {
        let text = result_text(r);
        for m in markers.split('|') {
            if let Some(at) = text.find(m) {
                return text[at + m.len()..].trim().to_owned();
            }
        }
        return String::new();
    }
    String::new()
}

/// Check one operation against a server's recorded `tools/list` (R17): the
/// tool exists — or, for a tool named by a placeholder (a Lambda function),
/// the server's tools have one shared shape — and every input the tool's
/// schema requires is one the route fills.
pub fn check_route(op_name: &str, op: &OpDef, tools: &[cobolt_mcp::Tool]) -> Result<(), String> {
    if op.tool == "$list" {
        return Ok(());
    }
    let schema = if op.tool.contains('{') {
        // A dynamic tool: every tool of this server is one of them.
        let first = tools.first().ok_or_else(|| format!("{op_name}: the recorded server lists no tool"))?;
        // The generator titles each schema after its tool; the shape is the rest.
        let shape = |v: &Value| {
            let mut v = v.clone();
            if let Some(o) = v.as_object_mut() {
                o.remove("title");
            }
            v
        };
        if let Some(odd) = tools.iter().find(|t| shape(&t.input_schema) != shape(&first.input_schema)) {
            return Err(format!("{op_name}: the server's tools do not share one shape ({} differs)", odd.name));
        }
        &first.input_schema
    } else {
        &tools
            .iter()
            .find(|t| t.name == op.tool)
            .ok_or_else(|| format!("{op_name}: the server has no tool named {}", op.tool))?
            .input_schema
    };
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let filled: Vec<String> = match &op.input {
        Some(toml::Value::Table(t)) => t.keys().cloned().collect(),
        // The whole input is one JSON argument: the caller fills it.
        Some(toml::Value::String(_)) => return Ok(()),
        _ => Vec::new(),
    };
    // Every input the route fills must be one the tool takes: a server that
    // renamed `parameters` to `payload` would otherwise pass this check and
    // fail every call.
    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        if let Some(unknown) = filled.iter().find(|f| !props.contains_key(f.as_str())) {
            return Err(format!("{op_name}: the route fills \"{unknown}\", which the server's tool does not take"));
        }
    }
    let missing: Vec<&str> = required.into_iter().filter(|r| !filled.iter().any(|f| f == r)).collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("{op_name}: the tool requires {missing:?}, which the route does not fill"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx<'a>(args: &'a [String], prop: &'a dyn Fn(&str) -> String, conn: &'a dyn Fn(&str) -> String) -> Context<'a> {
        Context { args, prop, connection: conn }
    }

    #[test]
    fn the_shipped_table_parses_and_every_op_names_a_server() {
        let r = Routes::shipped();
        assert!(!r.ops.is_empty());
        for (name, op) in &r.ops {
            assert!(
                op.server.starts_with('{') || r.servers.contains_key(&op.server),
                "{name} names an unknown server {}",
                op.server
            );
        }
        for (id, s) in &r.servers {
            for a in &s.args {
                if a.contains('@') {
                    assert!(!a.ends_with("@latest"), "{id} is not pinned: {a}");
                }
            }
        }
        println!("route table: {} servers, {} operations", r.servers.len(), r.ops.len());
    }

    #[test]
    fn placeholders_and_filters_expand() {
        let args = vec!["app-order-sync".to_string(), r#"{"id": 7}"#.to_string()];
        let prop = |n: &str| if n == "ServerId" { "lambda".into() } else { String::new() };
        let conn = |f: &str| if f == "FunctionPrefix" { "app-".into() } else { String::new() };
        let c = ctx(&args, &prop, &conn);
        assert_eq!(expand("{arg:1|strip:{Connection.FunctionPrefix}|ident}", &c).unwrap(), "order_sync");
        assert_eq!(expand("{arg:1|ident}", &c).unwrap(), "app_order_sync");
        assert_eq!(expand("x-{prop:ServerId}-y", &c).unwrap(), "x-lambda-y");
        let input: toml::Value = toml::from_str(r#"parameters = "{arg:2|json}""#).unwrap();
        assert_eq!(expand_input(&input, &c).unwrap(), json!({"parameters": {"id": 7}}));
        let numeric = vec!["9lives".to_string()];
        assert_eq!(expand("{arg:1|ident}", &ctx(&numeric, &prop, &conn)).unwrap(), "_9lives");
    }

    /// R21: an invalid JSON argument names the argument.
    #[test]
    fn invalid_json_names_the_argument() {
        let args = vec!["f".to_string(), "{not json".to_string()];
        let none = |_: &str| String::new();
        let input: toml::Value = toml::from_str(r#"parameters = "{arg:2|json}""#).unwrap();
        match expand_input(&input, &ctx(&args, &none, &none)) {
            Err(ExpandError::InvalidJson { arg: 2, .. }) => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_override_replaces_one_key() {
        let shipped = Routes::shipped();
        let o = r#"
[servers.lambda]
command = "/opt/fake"
args = ["script.json"]
"#;
        let merged = shipped.with_override(o).unwrap();
        assert_eq!(merged.servers["lambda"].command, "/opt/fake");
        assert_eq!(merged.ops, shipped.ops, "operations untouched");
        assert!(shipped.with_override("not = [valid").is_err());
    }

    #[test]
    fn extractors_read_text_json_and_markers() {
        let r = ToolResult::ok(vec![Content::text("Function app-f returned: {\n  \"total\": 3\n}")]);
        assert_eq!(extract(&r, "$after:returned: |returned payload: "), "{\n  \"total\": 3\n}");
        assert_eq!(extract(&r, "$after:returned with error: "), "");
        let e = ToolResult::ok(vec![Content::text("Function app-f returned with error: Unhandled")]);
        assert_eq!(extract(&e, "$after:returned with error: "), "Unhandled");
        let j = ToolResult::ok(vec![Content::text(r#"{"rows":[{"a":1}],"name":"x"}"#)]);
        assert_eq!(extract(&j, "$json:/name"), "x");
        assert_eq!(extract(&j, "$json:/rows/0"), r#"{"a":1}"#);
        assert_eq!(extract(&ToolResult::failed("no"), "$isError"), "1");
    }

    #[test]
    fn launch_is_readonly_unless_writes_are_allowed() {
        let s = ServerDef {
            command: "uvx".into(),
            args: vec!["pkg@1.0.0".into()],
            env: BTreeMap::from([("FUNCTION_PREFIX".into(), "{Connection.FunctionPrefix}".into()), ("FUNCTION_LIST".into(), "{Connection.FunctionList}".into())]),
            readonly_args: vec!["--read-only".into()],
            write_args: vec!["--allow-write".into()],
            protocol: Protocol::Handshake,
        };
        let none = |_: &str| String::new();
        let conn = |f: &str| if f == "FunctionPrefix" { "app-".into() } else { String::new() };
        let c = ctx(&[], &none, &conn);
        let ro = launch_for(&s, &c, false, "dev", "eu-west-1").unwrap();
        assert_eq!(ro.args, ["pkg@1.0.0", "--read-only"]);
        assert_eq!(ro.env, [("FUNCTION_PREFIX".to_string(), "app-".to_string())], "an empty variable is not set");
        assert_eq!(launch_for(&s, &c, true, "dev", "eu-west-1").unwrap().args, ["pkg@1.0.0", "--allow-write"]);
    }

    /// The Delivery B filters: defaults, optional keys, numbers, and JSON
    /// inside a string.
    #[test]
    fn filters_default_omit_and_shape_values() {
        let args = vec![String::new(), "say \"hi\"".to_string()];
        let prop = |n: &str| match n {
            "RuntimeArn" => "arn:x".into(),
            "MaxResults" => "5".into(),
            _ => String::new(),
        };
        let none = |_: &str| String::new();
        let c = ctx(&args, &prop, &none);
        assert_eq!(expand("{arg:1|or:{prop:RuntimeArn}}", &c).unwrap(), "arn:x");
        assert_eq!(expand("{arg:2|or:{prop:RuntimeArn}}", &c).unwrap(), "say \"hi\"");
        assert_eq!(expand("{arg:2|quote}", &c).unwrap(), r#""say \"hi\"""#);
        let input: toml::Value = toml::from_str(
            r#"
a = "{arg:1|or:{prop:RuntimeArn}}"
payload = "{arg:2|obj:prompt|text}"
session = "{arg:3|opt}"
n = "{prop:MaxResults|opt|int}"
gone = "{prop:Nothing|opt|int}"
p = "{arg:4|json:{}}"
list = [{ text = "{arg:2}", role = "{arg:5|or:USER}" }]
"#,
        )
        .unwrap();
        assert_eq!(
            expand_input(&input, &c).unwrap(),
            json!({"a": "arn:x", "payload": r#"{"prompt":"say \"hi\""}"#, "n": 5, "p": {}, "list": [{"text": "say \"hi\"", "role": "USER"}]})
        );
        let bad = vec!["seven".to_string()];
        let e = expand("{arg:1|int}", &ctx(&bad, &none, &none)).unwrap_err();
        assert!(e.to_string().contains("not a whole number"), "{e}");
    }

    #[test]
    fn rows_are_shaped_from_arrays_objects_sequences_and_columns() {
        let def = |t: &str| -> RowsDef { toml::from_str(t).unwrap() };
        let none = BTreeMap::new();
        // A sequence of JSON objects, one after another, with alternatives.
        let seq = ToolResult::ok(vec![Content::text(
            "{\"content\":{\"text\":\"A\"},\"location\":{\"s3Location\":{\"uri\":\"s3://b/a\"}},\"score\":0.9}\n\n{\"content\":{\"text\":\"B\"},\"location\":{\"type\":\"WEB\",\"webLocation\":{\"url\":\"https://x\"}},\"score\":0.5}",
        )]);
        let rows = shape_rows(&seq, &def(r#"from = "$jsonseq:"
fields = { Text = "/content/text", Source = "/location/s3Location/uri|/location/webLocation/url", Score = "/score" }"#), &none);
        assert_eq!(rows, vec![json!({"Text": "A", "Source": "s3://b/a", "Score": 0.9}), json!({"Text": "B", "Source": "https://x", "Score": 0.5})]);
        // An object: a row per entry, `$key` its key.
        let obj = ToolResult::ok(vec![Content::text(r#"{"KB1":{"name":"Docs"},"KB2":{"name":"FAQ"}}"#)]);
        let rows = shape_rows(&obj, &def(r#"from = "$json:"
fields = { Id = "$key", Name = "/name" }"#), &none);
        assert_eq!(rows, vec![json!({"Id": "KB1", "Name": "Docs"}), json!({"Id": "KB2", "Name": "FAQ"})]);
        // Arrays of values named by a column list.
        let tab = ToolResult::ok(vec![Content::text(r#"{"columns":["id","city"],"rows":[[1,"Lisboa"],[2,"Porto"]]}"#)]);
        let rows = shape_rows(&tab, &def(r#"from = "$json:/rows"
columns = "/columns""#), &none);
        assert_eq!(rows, vec![json!({"id": 1, "city": "Lisboa"}), json!({"id": 2, "city": "Porto"})]);
        // `where`: an expanded value keeps matching rows; empty keeps all.
        let listed = ToolResult::ok(vec![Content::text(r#"{"tables":[{"namespace":["sales"],"name":"a"},{"namespace":["ops"],"name":"b"}]}"#)]);
        let d = def(r#"from = "$json:/tables"
fields = { Name = "/name" }"#);
        let only = BTreeMap::from([("/namespace/0".to_string(), "ops".to_string())]);
        assert_eq!(shape_rows(&listed, &d, &only), vec![json!({"Name": "b"})]);
        let all = BTreeMap::from([("/namespace/0".to_string(), String::new())]);
        assert_eq!(shape_rows(&listed, &d, &all).len(), 2);
        // A message block, then the data: `$json:` finds the data.
        let two = ToolResult::ok(vec![Content::text("Successfully retrieved table"), Content::text(r#"{"job_run_id":"jr_1"}"#)]);
        assert_eq!(extract(&two, "$json:/job_run_id"), "jr_1");
    }

    /// R15: no Rust source names a server package or launcher; they live in
    /// the route table only.
    #[test]
    fn no_server_or_tool_named_in_code() {
        let src = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
        let mut offenders = Vec::new();
        let mut stack = vec![std::path::PathBuf::from(src)];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    let text = std::fs::read_to_string(&p).unwrap();
                    for banned in ["awslabs.", "mcp-proxy-for-aws", "lambda-tool-mcp-server"] {
                        // This test names them to look for them.
                        if text.contains(banned) && !p.ends_with("routes.rs") {
                            offenders.push(format!("{} names {banned}", p.display()));
                        }
                    }
                }
            }
        }
        assert!(offenders.is_empty(), "{offenders:#?}");
    }
}
