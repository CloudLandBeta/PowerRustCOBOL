// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The script of `run_form` as one type (WASM-015 T1, plan §3.1): the steps the desktop host runs, and the
//! read-only steps the browser helper adds. A step is a JSON object with ONE key, the step's name; the JSON of
//! the existing steps is unchanged, so every script an agent already wrote parses as before.
//!
//! The three engines (desktop, browser, module) take the same [`Step`]; an engine that cannot do a step says so
//! by its name ([`Step::desktop_unsupported`]) instead of ignoring it.

use serde_json::{json, Map, Value};

/// One step of a script.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// `{"set":{"control","property","value"}}`
    Set { control: String, property: String, value: String },
    /// `{"event":{"control","name"}}`, with `"wait": false` beside it to go on without settling.
    Event { control: String, name: String, wait: bool },
    /// `{"click":{"control"}}`, a real pointer click.
    Click { control: String },
    /// `{"wait_ms":n}`
    WaitMs(u64),
    /// `{"read":{"control","property"}}`
    Read { control: String, property: String },
    /// `{"dump":{}}`
    Dump,
    /// `{"open_form":"NAME"}`
    OpenForm(String),
    /// `{"text":{"control"}}`: an element's text (browser).
    Text { control: String },
    /// `{"style":{"control","property"}}`: one computed property (browser).
    Style { control: String, property: String },
    /// `{"rect":{"control"}}`: the border box in CSS pixels (browser).
    Rect { control: String },
    /// `{"snapshot":{}}`: the player's snapshot JSON (browser).
    Snapshot,
    /// `{"focused":{}}`: the focused control.
    Focused,
    /// `{"key":{"key","shift"}}`: a key press.
    Key { key: String, shift: bool },
    /// `{"page":{"property","frame"?}}`: a window property of a fixed list (browser).
    Page { property: String, frame: Option<String> },
}

impl Step {
    /// The step's name, as it is written in the JSON.
    pub fn name(&self) -> &'static str {
        match self {
            Step::Set { .. } => "set",
            Step::Event { .. } => "event",
            Step::Click { .. } => "click",
            Step::WaitMs(_) => "wait_ms",
            Step::Read { .. } => "read",
            Step::Dump => "dump",
            Step::OpenForm(_) => "open_form",
            Step::Text { .. } => "text",
            Step::Style { .. } => "style",
            Step::Rect { .. } => "rect",
            Step::Snapshot => "snapshot",
            Step::Focused => "focused",
            Step::Key { .. } => "key",
            Step::Page { .. } => "page",
        }
    }

    /// Parse one step; an unknown one is refused by name.
    pub fn parse(v: &Value) -> Result<Step, String> {
        let obj = v.as_object().ok_or("a step is an object")?;
        let (name, arg) = obj.iter().find(|(k, _)| k.as_str() != "wait").or_else(|| obj.iter().next()).ok_or("a step is an object with one key")?;
        let known = [
            "set", "event", "click", "wait_ms", "read", "dump", "open_form", "text", "style", "rect", "snapshot", "focused", "key", "page",
        ];
        if !known.contains(&name.as_str()) {
            return Err(format!("`{name}` is not a step (use {})", known.join(", ")));
        }
        let extra = obj.keys().filter(|k| k.as_str() != name.as_str()).count();
        if extra > 0 && !(name == "event" && extra == 1 && obj.contains_key("wait")) {
            return Err("a step is an object with one key".into());
        }
        let text = |k: &str| arg.get(k).and_then(Value::as_str).map(str::to_owned);
        let need = |k: &str| text(k).ok_or_else(|| format!("`{name}` needs `{k}`"));
        Ok(match name.as_str() {
            "set" => Step::Set { control: need("control")?, property: need("property")?, value: text("value").unwrap_or_default() },
            "event" => Step::Event { control: need("control")?, name: need("name")?, wait: obj.get("wait").and_then(Value::as_bool) != Some(false) },
            "click" => Step::Click { control: need("control")? },
            "wait_ms" => Step::WaitMs(arg.as_u64().ok_or("`wait_ms` is a number of milliseconds")?),
            "read" => Step::Read { control: need("control")?, property: need("property")? },
            "dump" => Step::Dump,
            "open_form" => Step::OpenForm(arg.as_str().ok_or("`open_form` is a form name")?.to_owned()),
            "text" => Step::Text { control: need("control")? },
            "style" => Step::Style { control: need("control")?, property: need("property")? },
            "rect" => Step::Rect { control: need("control")? },
            "snapshot" => Step::Snapshot,
            "focused" => Step::Focused,
            "key" => Step::Key { key: need("key")?, shift: arg.get("shift").and_then(Value::as_bool).unwrap_or(false) },
            "page" => Step::Page { property: need("property")?, frame: text("frame") },
            _ => unreachable!("checked against the list"),
        })
    }

    /// Parse a whole script; the first wrong step is the error, numbered from 1.
    pub fn parse_all(steps: &[Value]) -> Result<Vec<Step>, String> {
        steps.iter().enumerate().map(|(i, s)| Step::parse(s).map_err(|e| format!("step {}: {e}", i + 1))).collect()
    }

    /// The step's JSON; [`Step::parse`] of it gives the step back.
    pub fn to_json(&self) -> Value {
        let one = |name: &str, arg: Value| {
            let mut m = Map::new();
            m.insert(name.to_owned(), arg);
            Value::Object(m)
        };
        match self {
            Step::Set { control, property, value } => one("set", json!({"control": control, "property": property, "value": value})),
            Step::Event { control, name, wait } => {
                let mut v = one("event", json!({"control": control, "name": name}));
                if !wait {
                    v["wait"] = json!(false);
                }
                v
            }
            Step::Click { control } => one("click", json!({"control": control})),
            Step::WaitMs(ms) => one("wait_ms", json!(ms)),
            Step::Read { control, property } => one("read", json!({"control": control, "property": property})),
            Step::Dump => one("dump", json!({})),
            Step::OpenForm(name) => one("open_form", json!(name)),
            Step::Text { control } => one("text", json!({"control": control})),
            Step::Style { control, property } => one("style", json!({"control": control, "property": property})),
            Step::Rect { control } => one("rect", json!({"control": control})),
            Step::Snapshot => one("snapshot", json!({})),
            Step::Focused => one("focused", json!({})),
            Step::Key { key, shift } => one("key", json!({"key": key, "shift": shift})),
            Step::Page { property, frame } => {
                let mut a = json!({"property": property});
                if let Some(f) = frame {
                    a["frame"] = json!(f);
                }
                one("page", a)
            }
        }
    }

    /// Does this step need the browser's DOM, so the desktop host cannot run it?
    pub fn desktop_unsupported(&self) -> bool {
        matches!(self, Step::Text { .. } | Step::Style { .. } | Step::Rect { .. } | Step::Snapshot | Step::Focused | Step::Key { .. } | Step::Page { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<Value> {
        vec![
            json!({"set": {"control": "TXT-A", "property": "Text", "value": "Ana"}}),
            json!({"event": {"control": "BTN-GO", "name": "onClick"}}),
            json!({"event": {"control": "BTN-GO", "name": "onClick"}, "wait": false}),
            json!({"click": {"control": "BTN-GO"}}),
            json!({"wait_ms": 250}),
            json!({"read": {"control": "LBL-OUT", "property": "Text"}}),
            json!({"dump": {}}),
            json!({"open_form": "CUSTOMERS"}),
            json!({"text": {"control": "LBL-OUT"}}),
            json!({"style": {"control": "LBL-OUT", "property": "color"}}),
            json!({"rect": {"control": "BTN-GO"}}),
            json!({"snapshot": {}}),
            json!({"focused": {}}),
            json!({"key": {"key": "Tab", "shift": true}}),
            json!({"page": {"property": "innerWidth", "frame": "phone-frame"}}),
        ]
    }

    /// WASM-015 T1: each step's JSON parses and gives the same JSON back, so no script an agent wrote changes.
    #[test]
    fn every_step_round_trips_its_json() {
        for v in all() {
            let step = Step::parse(&v).unwrap_or_else(|e| panic!("{v}: {e}"));
            assert_eq!(step.to_json(), v, "{}", step.name());
            assert_eq!(Step::parse(&step.to_json()), Ok(step));
        }
        assert_eq!(Step::parse_all(&all()).unwrap().len(), 15);
    }

    #[test]
    fn an_unknown_or_malformed_step_is_refused_by_name() {
        let e = Step::parse(&json!({"eval": "alert(1)"})).unwrap_err();
        assert!(e.contains("`eval` is not a step"), "{e}");
        assert!(Step::parse(&json!({"click": {}})).unwrap_err().contains("`click` needs `control`"));
        assert!(Step::parse(&json!({"wait_ms": "soon"})).is_err());
        assert!(Step::parse(&json!({"click": {"control": "A"}, "dump": {}})).is_err(), "two keys are not a step");
        let e = Step::parse_all(&[json!({"dump": {}}), json!({"nope": 1})]).unwrap_err();
        assert!(e.starts_with("step 2:"), "{e}");
    }

    #[test]
    fn the_browser_steps_are_the_ones_the_desktop_cannot_run() {
        let unsupported: Vec<&str> = Step::parse_all(&all()).unwrap().iter().filter(|s| s.desktop_unsupported()).map(Step::name).collect();
        assert_eq!(unsupported, ["text", "style", "rect", "snapshot", "focused", "key", "page"]);
    }
}
