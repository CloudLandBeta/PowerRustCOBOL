// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The methods of the controls: `LST-1::AddItem("x")`, `TXT::GetText()`, `NUD-1::Increment()`.
//!
//! Each arm is the interpreter's (`cobolt-runtime/src/interpreter.rs`, `exec_method`) carried
//! over without change of meaning, because a program must do the same thing in a browser as on
//! the desktop (WASM-002 R4). Where the interpreter writes a *pseudo-property* the desktop host
//! reads back as a command (`_SetFocus`, `_SelectAll`, `_ShowSnackbar`), the player records a
//! [`Change::Command`] for the renderer instead — the same intent, with no state left over.
//!
//! A method name the closed vocabulary (`cobolt-ast/src/methods.rs`) knows but the browser
//! cannot do (`LoadFromFile`, `Print`, the AWS and database verbs) answers [`MethodError::Unsupported`]
//! with a message that says why; the host reports it as a readable run-time error and carries on.

use cobolt_forms::model::{ControlType, SELECTED_PROP};

use crate::tree::{Ctl, EventReq, FormTree, Kind};
use crate::value::{b01, fmt_number, index1, parse_f64, parse_i64, truthy};

/// What a method did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The answer, for `CALLR`; `None` for a method with no result.
    pub value: Option<String>,
    /// Observer events the writes it made raised.
    pub events: Vec<EventReq>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodError {
    /// The method is not available on this control in a browser.
    Unsupported(String),
    /// The host refuses it (a policy: a forbidden URL scheme, …).
    Refused(String),
}

impl std::fmt::Display for MethodError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MethodError::Unsupported(m) | MethodError::Refused(m) => f.write_str(m),
        }
    }
}

fn done() -> Result<Outcome, MethodError> {
    Ok(Outcome::default())
}

fn answer(v: impl Into<String>) -> Result<Outcome, MethodError> {
    Ok(Outcome { value: Some(v.into()), events: Vec::new() })
}

fn is_list(c: &Ctl) -> bool {
    matches!(c.kind, Kind::Control(ControlType::ListBox | ControlType::ComboBox))
}

fn is_class(c: &Ctl, t: ControlType) -> bool {
    matches!(&c.kind, Kind::Control(x) if *x == t)
}

/// Apply `set` and gather its events into `out`.
fn put(tree: &mut FormTree, ctl: usize, name: &str, value: &str, out: &mut Outcome) {
    out.events.extend(tree.set(ctl, name, value));
}

fn lines(items: &str) -> Vec<String> {
    items.lines().map(str::to_owned).collect()
}

/// The count of items in a newline-separated list (`""` is none).
fn count(items: &str) -> usize {
    if items.trim().is_empty() {
        0
    } else {
        items.lines().count()
    }
}

/// `Name::Prop` / `Name.Prop` → (control index, property); a bare name → (this control, name).
fn resolve_ref(tree: &FormTree, ctl: usize, p: &str) -> (usize, String) {
    for sep in ["::", "."] {
        if let Some((c, prop)) = p.split_once(sep) {
            if let Some(i) = tree.by_name(c) {
                return (i, prop.trim().to_owned());
            }
        }
    }
    (ctl, p.trim().to_owned())
}

/// Call method `method` (any case) on `ctl` with `args`.
pub fn call(tree: &mut FormTree, ctl: usize, method: &str, args: &[&str]) -> Result<Outcome, MethodError> {
    let m = method.trim().to_ascii_uppercase();
    let arg = |i: usize| args.get(i).map(|s| s.trim().to_owned()).unwrap_or_default();
    let mut out = Outcome::default();
    let kind = tree.ctls[ctl].kind.clone();
    let is_snackbar = matches!(kind, Kind::Control(ControlType::Snackbar));
    let is_chart = matches!(
        &kind,
        Kind::Control(
            ControlType::BarChart
                | ControlType::LineChart
                | ControlType::PieChart
                | ControlType::AreaChart
                | ControlType::ScatterChart
                | ControlType::DonutChart
                | ControlType::RadarChart
        )
    );

    // `GET-x` / `SET-x` are accessors (spec 010).
    if let Some(prop) = m.strip_prefix("GET-") {
        return answer(tree.get(ctl, prop));
    }
    if let Some(prop) = m.strip_prefix("SET-") {
        put(tree, ctl, prop, &arg(0), &mut out);
        return Ok(out);
    }

    match m.as_str() {
        // ── universal lifecycle / visibility ────────────────────────────────
        "SHOW" => {
            if is_snackbar {
                tree.command(ctl, "show", Vec::new());
            } else {
                put(tree, ctl, "Visible", "1", &mut out);
            }
        }
        "HIDE" => put(tree, ctl, "Visible", "0", &mut out),
        "ENABLE" => put(tree, ctl, "Enabled", "1", &mut out),
        "DISABLE" => put(tree, ctl, "Enabled", "0", &mut out),
        "SETFOCUS" | "FOCUS" => tree.command(ctl, "focus", Vec::new()),
        "BRINGTOFRONT" => put(tree, ctl, "ZOrder", "10000", &mut out),
        "SENDTOBACK" => put(tree, ctl, "ZOrder", "-10000", &mut out),
        "REFRESH" | "VALIDATE" => tree.command(ctl, "refresh", Vec::new()),
        // ── geometry ────────────────────────────────────────────────────────
        "MOVETO" => {
            put(tree, ctl, "X", &arg(0), &mut out);
            put(tree, ctl, "Y", &arg(1), &mut out);
        }
        "RESIZE" => {
            put(tree, ctl, "Width", &arg(0), &mut out);
            put(tree, ctl, "Height", &arg(1), &mut out);
        }
        // ── generic property access ─────────────────────────────────────────
        "SETPROPERTY" => {
            let (target, key) = resolve_ref(tree, ctl, &arg(0));
            put(tree, target, &key, &arg(1), &mut out);
        }
        "GETPROPERTY" => {
            let (target, key) = resolve_ref(tree, ctl, &arg(0));
            return answer(tree.get(target, &key));
        }
        // ── text / caption ──────────────────────────────────────────────────
        "SETCAPTION" => put(tree, ctl, "Caption", &arg(0), &mut out),
        "SETTEXT" => put(tree, ctl, "Text", &arg(0), &mut out),
        "GETCAPTION" => return answer(tree.get(ctl, "Caption")),
        "GETTEXT" => return answer(tree.get(ctl, "Text")),
        "APPENDTEXT" => {
            let cur = tree.get(ctl, "Text");
            put(tree, ctl, "Text", &format!("{cur}{}", args.first().copied().unwrap_or("")), &mut out);
        }
        "SETCOLOR" => put(tree, ctl, "ForegroundColor", &arg(0), &mut out),
        "SELECTALL" => tree.command(ctl, "select-all", Vec::new()),
        "CLEAR" => {
            if is_snackbar {
                put(tree, ctl, "Buttons", "", &mut out);
                return Ok(out);
            }
            if is_chart {
                put(tree, ctl, "__ChartData", "", &mut out);
                return Ok(out);
            }
            put(tree, ctl, "Text", "", &mut out);
            put(tree, ctl, "Items", "", &mut out);
            if is_list(&tree.ctls[ctl]) {
                put(tree, ctl, "SelectedIndex", "0", &mut out);
                put(tree, ctl, "Value", "", &mut out);
            }
        }
        // ── charts ──────────────────────────────────────────────────────────
        "ADDPOINT" | "ADD-POINT" if is_chart => {
            let mut line = vec![arg(0), fmt_number(parse_f64(&arg(1)).unwrap_or(0.0))];
            line.extend(args.iter().skip(2).map(|v| fmt_number(parse_f64(v).unwrap_or(0.0))));
            let cur = tree.get(ctl, "__ChartData");
            let next = if cur.is_empty() { line.join("\t") } else { format!("{cur}\n{}", line.join("\t")) };
            put(tree, ctl, "__ChartData", &next, &mut out);
        }
        // ── snackbar ────────────────────────────────────────────────────────
        "DISMISSALL" if is_snackbar => tree.command(ctl, "dismiss-all", Vec::new()),
        "ADDBUTTON" | "ADD-BUTTON" if is_snackbar => {
            let existing = tree.get(ctl, "Buttons");
            match cobolt_forms::snackbar::add_button(&existing, &arg(0)) {
                Some(next) => put(tree, ctl, "Buttons", &next, &mut out),
                None => {
                    return Err(MethodError::Unsupported(format!(
                        "AddButton(\"{}\") declares no id — nothing added",
                        arg(0)
                    )))
                }
            }
        }
        // ── checkbox / radio ────────────────────────────────────────────────
        "ISCHECKED" | "ISSELECTED" => {
            let key = if matches!(kind, Kind::Control(ControlType::RadioButton)) { SELECTED_PROP } else { "Checked" };
            return answer(b01(&tree.get(ctl, key)));
        }
        "SETCHECKED" | "SETSELECTED" => put(tree, ctl, "Checked", &b01(&arg(0)), &mut out),
        "SELECT" => put(tree, ctl, "Checked", "1", &mut out),
        "TOGGLE" => {
            let cur = tree.get(ctl, "Checked");
            put(tree, ctl, "Checked", if truthy(&cur) { "0" } else { "1" }, &mut out);
        }
        // ── numeric value ───────────────────────────────────────────────────
        "SETVALUE" => put(tree, ctl, "Value", &arg(0), &mut out),
        "GETVALUE" => return answer(tree.get(ctl, "Value")),
        "INCREMENT" | "DECREMENT" => {
            let step = parse_f64(&tree.get(ctl, "Step")).filter(|s| *s != 0.0).map(f64::abs).unwrap_or(1.0);
            let v = parse_f64(&tree.get(ctl, "Value")).unwrap_or(0.0);
            let mut n = if m == "INCREMENT" { v + step } else { v - step };
            if let Some(lo) = parse_f64(&tree.get(ctl, "Minimum")) {
                n = n.max(lo);
            }
            if let Some(hi) = parse_f64(&tree.get(ctl, "Maximum")) {
                n = n.min(hi);
            }
            put(tree, ctl, "Value", &fmt_number(n), &mut out);
        }
        "RESET" => {
            let default = tree.get(ctl, "DefaultValue");
            let back = if !default.trim().is_empty() {
                default
            } else {
                let min = tree.get(ctl, "Minimum");
                if min.trim().is_empty() { "0".to_owned() } else { min }
            };
            put(tree, ctl, "Value", &back, &mut out);
        }
        // ── items (list / combo) ────────────────────────────────────────────
        "ADDITEM" => {
            let cur = tree.get(ctl, "Items");
            let nv = if cur.is_empty() { arg(0) } else { format!("{cur}\n{}", arg(0)) };
            put(tree, ctl, "Items", &nv, &mut out);
        }
        "REMOVEITEM" | "REMOVEAT" => {
            let cur = tree.get(ctl, "Items");
            let mut ls = lines(&cur);
            let at = if m == "REMOVEAT" {
                index1(&arg(0))
            } else {
                let wanted = args.first().copied().unwrap_or("").trim_end().to_owned();
                ls.iter().position(|l| l.trim_end() == wanted)
            };
            if let Some(i) = at.filter(|i| *i < ls.len()) {
                let removed = ls.remove(i);
                put(tree, ctl, "Items", &ls.join("\n"), &mut out);
                if is_list(&tree.ctls[ctl]) && tree.get(ctl, "Value") == removed {
                    put(tree, ctl, "SelectedIndex", "0", &mut out);
                    put(tree, ctl, "Value", "", &mut out);
                }
            }
        }
        "GETSELECTED" => return answer(tree.get(ctl, "Value")),
        "GETSELECTEDINDEX" | "GETINDEX" => return answer(tree.get(ctl, "SelectedIndex")),
        "SETSELECTEDINDEX" | "SETINDEX" => {
            put(tree, ctl, "SelectedIndex", &arg(0), &mut out);
            // Keep `Value` in step with the index, as the control does when the user picks.
            if let Some(i) = index1(&arg(0)) {
                let items = tree.get(ctl, "Items");
                if let Some(l) = items.lines().nth(i) {
                    let l = l.to_owned();
                    put(tree, ctl, "Value", &l, &mut out);
                }
            }
        }
        "GETCOUNT" => return answer(count(&tree.get(ctl, "Items")).to_string()),
        "LOADFROMFILE" => {
            return Err(MethodError::Unsupported(
                "LoadFromFile reads a file of the machine, which a browser application has none of: load the lines from the backend and AddItem them".into(),
            ))
        }
        // ── TreeView ────────────────────────────────────────────────────────
        "ADDNODE" => {
            let level = parse_i64(&arg(0)).max(1) as usize - 1;
            let text = arg(1);
            if !text.is_empty() {
                let mut line = format!("{}{text}", "  ".repeat(level));
                let fields = [arg(2), arg(3), arg(4)];
                if let Some(last) = fields.iter().rposition(|f| !f.is_empty()) {
                    for f in &fields[..=last] {
                        line.push('\t');
                        line.push_str(f);
                    }
                }
                let cur = tree.get(ctl, "Items");
                let nv = if cur.is_empty() { line } else { format!("{cur}\n{line}") };
                put(tree, ctl, "Items", &nv, &mut out);
            }
        }
        "NODECOUNT" => return answer(cobolt_forms::treenodes::nodes(&tree.get(ctl, "Items")).len().to_string()),
        "NODEINDEXOF" => {
            let r = cobolt_forms::treenodes::index_of(&tree.get(ctl, "Items"), &arg(0)).map(|i| i + 1).unwrap_or(0);
            return answer(r.to_string());
        }
        "NODETEXT" | "NODENAME" => return answer(node_field(tree, ctl, &arg(0), |n| n.text.clone())),
        "NODEPATH" => return answer(node_field(tree, ctl, &arg(0), |n| n.path.clone())),
        "NODELEVEL" => return answer(node_field(tree, ctl, &arg(0), |n| (n.level + 1).to_string())),
        "NODEICON" => return answer(node_field(tree, ctl, &arg(0), |n| n.icon.clone().unwrap_or_default())),
        "NODECOLOR" | "NODECOLOUR" => return answer(node_field(tree, ctl, &arg(0), |n| n.color.clone().unwrap_or_default())),
        "NODEBACKCOLOR" | "NODEBACKGROUND" => {
            return answer(node_field(tree, ctl, &arg(0), |n| n.background.clone().unwrap_or_default()))
        }
        "NODECHILDCOUNT" => return answer(node_field(tree, ctl, &arg(0), |n| n.child_count.to_string())),
        "NODEHASCHILDREN" => {
            return answer(node_field(tree, ctl, &arg(0), |n| if n.child_count > 0 { "1" } else { "0" }.to_owned()))
        }
        "NODEPARENT" => return answer(node_link(tree, ctl, &arg(0), |n| n.parent)),
        "NODEFIRSTCHILD" => return answer(node_link(tree, ctl, &arg(0), |n| n.first_child)),
        "NODELASTCHILD" => return answer(node_link(tree, ctl, &arg(0), |n| n.last_child)),
        "NODENEXTSIBLING" => return answer(node_link(tree, ctl, &arg(0), |n| n.next_sibling)),
        "NODEPREVSIBLING" | "NODEPREVIOUSSIBLING" => return answer(node_link(tree, ctl, &arg(0), |n| n.prev_sibling)),
        "NODECHECKED" => {
            let text = node_field(tree, ctl, &arg(0), |n| n.text.clone());
            return answer(list_holds(&tree.get(ctl, "CheckedNodes"), &text));
        }
        "NODECOLLAPSED" => {
            let text = node_field(tree, ctl, &arg(0), |n| n.text.clone());
            return answer(list_holds(&tree.get(ctl, "CollapsedNodes"), &text));
        }
        "REMOVENODE" => {
            let items = tree.get(ctl, "Items");
            return match tree_without(&items, &arg(0)) {
                Some(rest) => {
                    put(tree, ctl, "Items", &rest, &mut out);
                    out.value = Some("1".into());
                    Ok(out)
                }
                None => answer("0"),
            };
        }
        "EXPANDALL" => put(tree, ctl, "CollapsedNodes", "", &mut out),
        "COLLAPSEALL" => {
            let parents: Vec<String> = cobolt_forms::treenodes::nodes(&tree.get(ctl, "Items"))
                .into_iter()
                .filter(|n| n.child_count > 0)
                .map(|n| n.text)
                .collect();
            put(tree, ctl, "CollapsedNodes", &parents.join("\n"), &mut out);
        }
        "GETSELECTEDNODE" => return answer(tree.get(ctl, "SelectedNode")),
        "SETSELECTEDNODE" => put(tree, ctl, "SelectedNode", &arg(0), &mut out),
        // ── DataGrid (in-memory rows) ───────────────────────────────────────
        "GETROWCOUNT" => return answer(rows_of(tree, ctl).len().to_string()),
        "GETCELLVALUE" => {
            let v = index1(&arg(0))
                .zip(index1(&arg(1)))
                .and_then(|(r, c)| rows_of(tree, ctl).get(r).and_then(|cells| cells.get(c)).cloned())
                .unwrap_or_default();
            return answer(v);
        }
        "SETCELLVALUE" => {
            if let (Some(row), Some(col)) = (index1(&arg(0)), index1(&arg(1))) {
                let mut rows = rows_of(tree, ctl);
                let width = column_count(tree, ctl).max(col + 1);
                while rows.len() <= row {
                    rows.push(vec![String::new(); width]);
                }
                if rows[row].len() <= col {
                    rows[row].resize(col + 1, String::new());
                }
                rows[row][col] = arg(2);
                set_rows(tree, ctl, &rows, &mut out);
            }
        }
        "ADDROW" => {
            let mut rows = rows_of(tree, ctl);
            let row = if args.is_empty() {
                vec![String::new(); column_count(tree, ctl)]
            } else {
                args[0].trim().split('\t').map(str::to_owned).collect()
            };
            rows.push(row);
            set_rows(tree, ctl, &rows, &mut out);
        }
        "DELETEROW" => {
            if let Some(row) = index1(&arg(0)) {
                let mut rows = rows_of(tree, ctl);
                if row < rows.len() {
                    rows.remove(row);
                    set_rows(tree, ctl, &rows, &mut out);
                }
            }
        }
        "CLEARROWS" => put(tree, ctl, "Rows", "", &mut out),
        "SORT" => {
            if let Some(col) = index1(&arg(0)) {
                let mut rows = rows_of(tree, ctl);
                crate::par::sort_rows(&mut rows, col);
                set_rows(tree, ctl, &rows, &mut out);
            }
        }
        "SETFILTER" | "CLEARFILTERS" | "FREEZECOLUMNS" | "FREEZEROWS" | "SETROWHEIGHT" | "SETCOLUMNWIDTH" | "SETCOLUMNTITLE" => {
            tree.command(ctl, &m.to_ascii_lowercase(), args.iter().map(|s| (*s).to_owned()).collect());
        }
        "REFRESHBINDING" | "COMMITEDIT" | "DISCARDEDIT" | "SAVE" if is_class(&tree.ctls[ctl], ControlType::DataGrid) => {
            return Err(MethodError::Unsupported(format!(
                "{method} works through the backend (class C control): the application's remote data layer performs it"
            )))
        }
        // ── Timer ───────────────────────────────────────────────────────────
        "START" => {
            put(tree, ctl, "Enabled", "1", &mut out);
            tree.command(ctl, "timer-start", Vec::new());
        }
        "STOP" => {
            put(tree, ctl, "Enabled", "0", &mut out);
            tree.command(ctl, "timer-stop", Vec::new());
        }
        "SETINTERVAL" => put(tree, ctl, "Interval", &arg(0), &mut out),
        "ISENABLED" => return answer(b01(&tree.get(ctl, "Enabled"))),
        "ISBUSY" => return answer(b01(&tree.get(ctl, "Busy"))),
        // ── Panel / GroupBox (spec 090) ─────────────────────────────────────
        "EXPAND" => put(tree, ctl, "Expanded", "1", &mut out),
        "COLLAPSE" => put(tree, ctl, "Expanded", "0", &mut out),
        // ── animation ───────────────────────────────────────────────────────
        "PLAYANIMATION" | "PLAY" | "STOPANIMATION" | "PAUSE" => {
            tree.command(ctl, &m.to_ascii_lowercase(), args.iter().map(|s| (*s).to_owned()).collect());
        }
        // ── Viewer: the conversation (spec 058 §8), kept in the tree like the interpreter keeps it ──
        "APPENDHTML" | "APPEND-HTML" | "APPENDMARKDOWN" | "APPEND-MARKDOWN" | "APPENDRAW" | "APPEND-RAW"
            if is_class(&tree.ctls[ctl], ControlType::Viewer) =>
        {
            use cobolt_forms::viewer::{AppendMode, MessageRole};
            let mode = if m.contains("HTML") {
                AppendMode::Html
            } else if m.contains("MARKDOWN") {
                AppendMode::Markdown
            } else {
                AppendMode::Raw
            };
            let content = args.first().copied().unwrap_or("");
            let role = MessageRole::from_str(&arg(1));
            let render = truthy(&tree.get(ctl, "RenderAsHtml"));
            let conv = tree.conversation_mut(ctl);
            conv.set_render_as_html(render);
            // The message's id comes back, so a streamed reply can extend it with `AppendToMessage`.
            let id = conv.append_as(mode, content, role);
            tree.changed(ctl, "__conversation");
            return answer(id);
        }
        "APPENDTOMESSAGE" | "APPEND-TO-MESSAGE" | "REPLACEMESSAGE" | "REPLACE-MESSAGE" | "REMOVEMESSAGE" | "REMOVE-MESSAGE"
            if is_class(&tree.ctls[ctl], ControlType::Viewer) =>
        {
            use cobolt_forms::viewer::AppendMode;
            let id = arg(0);
            let content = args.get(1).copied().unwrap_or("");
            let mode = AppendMode::from_str(&arg(2));
            let render = truthy(&tree.get(ctl, "RenderAsHtml"));
            let conv = tree.conversation_mut(ctl);
            conv.set_render_as_html(render);
            let ok = if m.contains("REMOVE") {
                conv.remove_message(&id)
            } else if m.contains("REPLACE") {
                conv.replace_message(&id, mode, content)
            } else {
                conv.append_to_message(&id, mode, content)
            };
            if ok {
                tree.changed(ctl, "__conversation");
            } else {
                // A chunk for a message that never started is a caller error worth reporting.
                put(tree, ctl, "LastError", &format!("no conversation message with id {id:?}"), &mut out);
                out.events.push(EventReq { ctl, event: "onError" });
            }
        }
        "NEWCONVERSATION" | "NEW-CONVERSATION" if is_class(&tree.ctls[ctl], ControlType::Viewer) => {
            let was_empty = tree.conversation_mut(ctl).is_empty();
            if !was_empty {
                tree.conversation_mut(ctl).clear();
                tree.changed(ctl, "__conversation");
                put(tree, ctl, "ConversationId", "", &mut out);
                out.events.push(EventReq { ctl, event: "onConversationCreated" });
            }
        }
        "SELECTCONVERSATION" | "SELECT-CONVERSATION" if is_class(&tree.ctls[ctl], ControlType::Viewer) => {
            tree.conversation_mut(ctl).clear();
            tree.changed(ctl, "__conversation");
            put(tree, ctl, "ConversationId", &arg(0), &mut out);
            out.events.push(EventReq { ctl, event: "onConversationSelected" });
        }
        "JUMPTOLATEST" | "JUMP-TO-LATEST" if is_class(&tree.ctls[ctl], ControlType::Viewer) => {
            tree.command(ctl, "jump-to-latest", Vec::new());
        }
        "FIND" | "FINDNEXT" | "FIND-NEXT" | "FINDPREVIOUS" | "FIND-PREVIOUS" | "FINDCLOSE" | "FIND-CLOSE"
            if is_class(&tree.ctls[ctl], ControlType::Viewer) =>
        {
            // The find bar's state is the control's properties, as on the desktop; the page highlights
            // nothing in this version (see the plan: Find is state only).
            match m.as_str() {
                "FIND" => {
                    if !args.is_empty() {
                        put(tree, ctl, "View1SearchText", &arg(0), &mut out);
                        put(tree, ctl, "View1SearchCurrentMatch", "0", &mut out);
                    }
                    if !truthy(&tree.get(ctl, "View1FindOpen")) {
                        put(tree, ctl, "View1FindOpen", "1", &mut out);
                        out.events.push(EventReq { ctl, event: "onFindOpened" });
                    }
                }
                "FINDCLOSE" | "FIND-CLOSE" => {
                    if truthy(&tree.get(ctl, "View1FindOpen")) {
                        put(tree, ctl, "View1FindOpen", "0", &mut out);
                        out.events.push(EventReq { ctl, event: "onFindClosed" });
                    }
                }
                _ => {}
            }
        }
        "PRINT" | "SHARE" | "SAVEAS" | "SAVEASPDF" | "SAVE-AS-PDF" if is_class(&tree.ctls[ctl], ControlType::Viewer) => {
            return Err(MethodError::Unsupported(format!(
                "{method} needs the operating system (class D); use the backend to produce the document and download it"
            )))
        }
        // ── WebLink (WASM-006): a player-level control (Authentication is the host's: it holds the session) ──
        "SETURL" | "GETURL" | "OPEN" | "OPENIN"
            if crate::kinds::player_kind(&tree.ctls[ctl]) == Some(crate::kinds::PlayerKind::WebLink) =>
        {
            return crate::weblink::method(tree, ctl, &m, args);
        }
        _ => {
            return Err(MethodError::Unsupported(format!(
                "the method {method} is not available on a {} in the browser",
                kind.type_name()
            )))
        }
    }
    Ok(out)
}

fn rows_of(tree: &FormTree, ctl: usize) -> Vec<Vec<String>> {
    cobolt_forms::datagrid::parse_rows(&tree.get(ctl, "Rows"))
}

fn set_rows(tree: &mut FormTree, ctl: usize, rows: &[Vec<String>], out: &mut Outcome) {
    let value = rows.iter().map(|r| r.join("\t")).collect::<Vec<_>>().join("\n");
    put(tree, ctl, "Rows", &value, out);
}

/// How many columns the grid declares (`Columns` is `Name:Type` per line or comma).
fn column_count(tree: &FormTree, ctl: usize) -> usize {
    let cols = tree.get(ctl, "Columns");
    if cols.trim().is_empty() {
        return 0;
    }
    cols.split(|c| c == '\n' || c == ',').filter(|s| !s.trim().is_empty()).count()
}

fn node_field(tree: &FormTree, ctl: usize, handle: &str, f: impl Fn(&cobolt_forms::treenodes::NodeInfo) -> String) -> String {
    index1(handle)
        .and_then(|i| cobolt_forms::treenodes::node_at(&tree.get(ctl, "Items"), i))
        .map(|n| f(&n))
        .unwrap_or_default()
}

fn node_link(
    tree: &FormTree,
    ctl: usize,
    handle: &str,
    f: impl Fn(&cobolt_forms::treenodes::NodeInfo) -> Option<usize>,
) -> String {
    index1(handle)
        .and_then(|i| cobolt_forms::treenodes::node_at(&tree.get(ctl, "Items"), i))
        .and_then(|n| f(&n))
        .map(|l| (l + 1).to_string())
        .unwrap_or_else(|| "0".into())
}

fn list_holds(list: &str, text: &str) -> String {
    let needle = text.trim();
    let held = !needle.is_empty() && list.lines().map(str::trim).any(|l| !l.is_empty() && l == needle);
    if held { "1" } else { "0" }.to_owned()
}

fn tree_without(items: &str, handle: &str) -> Option<String> {
    let line = index1(handle)?;
    let all = cobolt_forms::treenodes::nodes(items);
    let node = all.iter().find(|n| n.index == line)?;
    let ls: Vec<&str> = items.lines().collect();
    let end = all.iter().find(|n| n.index > line && n.level <= node.level).map_or(ls.len(), |n| n.index);
    Some(ls[..line].iter().chain(ls[end..].iter()).copied().collect::<Vec<_>>().join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::testing::tree;

    fn t() -> (FormTree, usize, usize, usize) {
        let t = tree();
        let lst = t.by_name("LST-1").unwrap();
        let nud = t.by_name("NUD-1").unwrap();
        let txt = t.by_name("TXT-IN").unwrap();
        (t, lst, nud, txt)
    }

    fn val(o: Result<Outcome, MethodError>) -> String {
        o.unwrap().value.unwrap_or_default()
    }

    #[test]
    fn list_methods_follow_the_interpreters_semantics() {
        let (mut t, lst, ..) = t();
        assert_eq!(val(call(&mut t, lst, "GetCount", &[])), "2");
        call(&mut t, lst, "AddItem", &["three"]).unwrap();
        assert_eq!(t.get(lst, "Items"), "one\ntwo\nthree");
        assert_eq!(val(call(&mut t, lst, "getcount", &[])), "3");
        call(&mut t, lst, "SetSelectedIndex", &["2"]).unwrap();
        assert_eq!(val(call(&mut t, lst, "GetSelected", &[])), "two");
        // Removing the selected item takes the selection with it.
        call(&mut t, lst, "RemoveItem", &["two"]).unwrap();
        assert_eq!(t.get(lst, "Items"), "one\nthree");
        assert_eq!(val(call(&mut t, lst, "GetSelectedIndex", &[])), "0");
        call(&mut t, lst, "RemoveAt", &["1"]).unwrap();
        assert_eq!(t.get(lst, "Items"), "three");
        call(&mut t, lst, "Clear", &[]).unwrap();
        assert_eq!(val(call(&mut t, lst, "GetCount", &[])), "0");
    }

    #[test]
    fn increment_steps_and_clamps_and_raises_the_observer_event_once_per_change() {
        let (mut t, _, nud, _) = t();
        let o = call(&mut t, nud, "Increment", &[]).unwrap();
        assert_eq!(t.get(nud, "Value"), "7");
        assert_eq!(o.events.len(), 1);
        call(&mut t, nud, "Increment", &[]).unwrap();
        call(&mut t, nud, "Increment", &[]).unwrap();
        assert_eq!(t.get(nud, "Value"), "10", "clamped to Maximum");
        let o = call(&mut t, nud, "Increment", &[]).unwrap();
        assert!(o.events.is_empty(), "a write that changes nothing raises nothing");
        call(&mut t, nud, "Reset", &[]).unwrap();
        assert_eq!(t.get(nud, "Value"), "0", "Reset goes to Minimum when there is no DefaultValue");
    }

    #[test]
    fn text_methods_and_the_generic_accessors() {
        let (mut t, _, _, txt) = t();
        call(&mut t, txt, "AppendText", &[" world"]).unwrap();
        assert_eq!(val(call(&mut t, txt, "GetText", &[])), "hello world");
        call(&mut t, txt, "SetProperty", &["Text", "x"]).unwrap();
        assert_eq!(val(call(&mut t, txt, "GetProperty", &["text"])), "x");
        call(&mut t, txt, "SetProperty", &["LST-1.Items", "a\nb"]).unwrap();
        let lst = t.by_name("LST-1").unwrap();
        assert_eq!(t.get(lst, "Items"), "a\nb", "Name.Prop addresses another control");
        call(&mut t, txt, "Set-Width", &["300"]).unwrap();
        assert_eq!(val(call(&mut t, txt, "Get-Width", &[])), "300");
        call(&mut t, txt, "Hide", &[]).unwrap();
        assert_eq!(t.get(txt, "Visible"), "false");
        call(&mut t, txt, "SetFocus", &[]).unwrap();
        assert!(t.take_changes().iter().any(|c| matches!(c, crate::tree::Change::Command { name, .. } if name == "focus")));
    }

    #[test]
    fn checkbox_and_radio_methods_use_the_state_property() {
        let mut t = tree();
        let chk = t.by_name("CHK-1").unwrap();
        let a = t.by_name("RAD-A").unwrap();
        call(&mut t, chk, "Toggle", &[]).unwrap();
        assert_eq!(val(call(&mut t, chk, "IsChecked", &[])), "1");
        call(&mut t, chk, "Toggle", &[]).unwrap();
        assert_eq!(val(call(&mut t, chk, "IsChecked", &[])), "0");
        call(&mut t, a, "Select", &[]).unwrap();
        assert_eq!(val(call(&mut t, a, "IsSelected", &[])), "1");
    }

    #[test]
    fn tree_nodes_are_one_based_handles_and_traversal_chains() {
        let mut t = tree();
        // reuse the ListBox as a TreeView stand-in for the Items text
        let lst = t.by_name("LST-1").unwrap();
        call(&mut t, lst, "Clear", &[]).unwrap();
        call(&mut t, lst, "AddNode", &["1", "Root"]).unwrap();
        call(&mut t, lst, "AddNode", &["2", "Child A"]).unwrap();
        call(&mut t, lst, "AddNode", &["2", "Child B", "star"]).unwrap();
        assert_eq!(val(call(&mut t, lst, "NodeCount", &[])), "3");
        assert_eq!(val(call(&mut t, lst, "NodeParent", &["2"])), "1");
        assert_eq!(val(call(&mut t, lst, "NodeNextSibling", &["2"])), "3");
        assert_eq!(val(call(&mut t, lst, "NodeNextSibling", &["3"])), "0");
        assert_eq!(val(call(&mut t, lst, "NodeIcon", &["3"])), "star");
        assert_eq!(val(call(&mut t, lst, "NodePath", &["3"])), "Root/Child B");
        assert_eq!(val(call(&mut t, lst, "RemoveNode", &["1"])), "1", "takes the subtree with it");
        assert_eq!(val(call(&mut t, lst, "NodeCount", &[])), "0");
    }

    #[test]
    fn datagrid_rows_are_tab_separated_lines_and_an_empty_line_is_a_row() {
        let mut t = tree();
        let lst = t.by_name("LST-1").unwrap(); // any control carries the Rows text here
        call(&mut t, lst, "AddRow", &["a\tb"]).unwrap();
        call(&mut t, lst, "AddRow", &["c\td"]).unwrap();
        assert_eq!(val(call(&mut t, lst, "GetRowCount", &[])), "2");
        assert_eq!(val(call(&mut t, lst, "GetCellValue", &["2", "2"])), "d");
        call(&mut t, lst, "SetCellValue", &["1", "2", "B"]).unwrap();
        assert_eq!(val(call(&mut t, lst, "GetCellValue", &["1", "2"])), "B");
        call(&mut t, lst, "Sort", &["1"]).unwrap();
        call(&mut t, lst, "DeleteRow", &["1"]).unwrap();
        assert_eq!(val(call(&mut t, lst, "GetRowCount", &[])), "1");
    }

    #[test]
    fn what_a_browser_cannot_do_is_a_readable_unsupported_not_a_silent_nothing() {
        let (mut t, lst, ..) = t();
        let e = call(&mut t, lst, "LoadFromFile", &["x.txt"]).unwrap_err();
        assert!(e.to_string().contains("backend"), "{e}");
        let e = call(&mut t, lst, "NoSuchMethod", &[]).unwrap_err();
        assert!(e.to_string().contains("not available"), "{e}");
    }
}
