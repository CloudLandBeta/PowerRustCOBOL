// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A form as the player draws it, as one static page** (WASM-016 R14, T17): the measuring end of
//! the look-and-feel parity between the desktop and the web.
//!
//! The player's view layer is plain data (`view/`): the elements each control becomes and the
//! property-layer stylesheet its properties write. This example runs that layer natively — with no
//! program module and no browser — and writes the result as an HTML page that a browser draws as the
//! player would: the same shadow root, the same base sheet, the same property layer, the same
//! elements in the same order. A headless browser then takes the picture the desktop's own picture
//! is compared with.
//!
//! ```text
//! cargo run --manifest-path crates/cobolt-web-player/Cargo.toml --example parity_page -- \
//!     FORM.wcfrm --out page.html [--width 1440] [--height 900] [--state state.json]
//! ```
//!
//! `--geometry` adds a script to the page that writes every control's box into the document
//! (`<pre id="geom">`), and `--compare DOM.html` reads that back from a headless browser's dumped DOM
//! (`chrome --headless --dump-dom PAGE`) and compares each box with the one the desktop's own layout
//! solver (`cobolt_forms::layout::solve`) gives the control: the measure of the layout half of the
//! parity, to the pixel.
//!
//! `--state` is a `{"CONTROL": {"visible": …, "enabled": …, "props": {…}}}` dump, as the headless
//! `run_form` script's `dump` step answers: the properties a running program had written are written
//! onto the tree the way the program writes them (`FormTree::set`), so the page shows the form as the
//! desktop showed it once its handlers had run. Only what differs from the design is written.
//!
//! What this page leaves out, on purpose: the loader's progress bar, the event listeners and
//! everything that moves. What it keeps is what the picture shows.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::PathBuf;

use cobolt_wasm_abi::manifest::{FormEntry, Limits, Manifest, VariantInfo};
use cobolt_web_player::base_css;
use cobolt_web_player::tree::FormTree;
use cobolt_web_player::value::truthy;
use cobolt_web_player::view::{controls, style, Empty, Prop, Slot, VNode};

/// The colour behind a see-through form theme: `cobolt_forms::theme::SEE_THROUGH_BACKDROP_RGB`.
const BACKDROP: &str = "#4A4642";

struct Args {
    form: PathBuf,
    out: PathBuf,
    width: Option<u32>,
    height: Option<u32>,
    state: Option<PathBuf>,
    backdrop: String,
    geometry: bool,
    compare: Option<PathBuf>,
}

fn args() -> Args {
    let mut it = std::env::args().skip(1);
    let mut a = Args { form: PathBuf::new(), out: PathBuf::from("page.html"), width: None, height: None, state: None, backdrop: BACKDROP.into(), geometry: false, compare: None };
    while let Some(k) = it.next() {
        match k.as_str() {
            "--out" => a.out = it.next().expect("--out FILE").into(),
            "--width" => a.width = it.next().and_then(|v| v.parse().ok()),
            "--height" => a.height = it.next().and_then(|v| v.parse().ok()),
            "--state" => a.state = it.next().map(PathBuf::from),
            "--backdrop" => a.backdrop = it.next().expect("--backdrop COLOUR"),
            "--geometry" => a.geometry = true,
            "--compare" => a.compare = it.next().map(PathBuf::from),
            _ if a.form.as_os_str().is_empty() => a.form = k.into(),
            other => panic!("unknown argument {other}"),
        }
    }
    assert!(!a.form.as_os_str().is_empty(), "usage: parity_page FORM --out page.html [--width W] [--height H] [--state state.json]");
    a
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn esc_attr(s: &str) -> String {
    esc(s).replace('"', "&quot;")
}

const VOID: &[&str] = &["input", "img", "br", "hr", "meta", "link", "source", "col"];

/// The first element (preorder) of `v` that takes the children of slot `s`, as the path of child
/// indexes from `v`; `None` when the node has no such element (the control's root takes them).
fn slot_path(v: &VNode, s: Slot, path: &mut Vec<usize>) -> Option<Vec<usize>> {
    if v.slot == Some(s) {
        return Some(path.clone());
    }
    for (i, c) in v.children.iter().enumerate() {
        path.push(i);
        if let Some(p) = slot_path(c, s, path) {
            path.pop();
            return Some(p);
        }
        path.pop();
    }
    None
}

struct Page<'a> {
    vnodes: &'a [Option<VNode>],
    kids: &'a BTreeMap<(usize, Slot), Vec<usize>>,
}

impl Page<'_> {
    /// The element of control `i`, with the controls mounted inside it.
    fn control(&self, i: usize, out: &mut String) {
        let Some(v) = &self.vnodes[i] else { return };
        let slots: Vec<(Slot, Vec<usize>)> = [Slot::Children, Slot::Page(1), Slot::Page(2), Slot::Page(3), Slot::Page(4), Slot::Page(5), Slot::Page(6), Slot::Page(7), Slot::Page(8), Slot::Pane(1), Slot::Pane(2)]
            .into_iter()
            .filter(|s| self.kids.contains_key(&(i, *s)))
            .map(|s| (s, slot_path(v, s, &mut Vec::new()).unwrap_or_default()))
            .collect();
        self.node(v, i, &mut Vec::new(), &slots, out);
    }

    fn node(&self, v: &VNode, ctl: usize, path: &mut Vec<usize>, slots: &[(Slot, Vec<usize>)], out: &mut String) {
        let _ = write!(out, "<{}", v.tag);
        if !v.classes.is_empty() {
            let _ = write!(out, " class=\"{}\"", esc_attr(&v.classes.join(" ")));
        }
        for (k, val) in &v.attrs {
            if val.is_empty() {
                let _ = write!(out, " {k}");
            } else {
                let _ = write!(out, " {k}=\"{}\"", esc_attr(val));
            }
        }
        let mut text_value: Option<&str> = None;
        for (k, p) in &v.props {
            match (*k, p) {
                ("value", Prop::Str(s)) if v.tag == "textarea" => text_value = Some(s),
                ("value", Prop::Str(s)) if v.tag == "input" => {
                    let _ = write!(out, " value=\"{}\"", esc_attr(s));
                }
                ("value", Prop::Num(n)) if v.tag == "input" || v.tag == "progress" => {
                    let _ = write!(out, " value=\"{n}\"");
                }
                ("checked" | "disabled" | "selected" | "hidden" | "open", Prop::Bool(true)) => {
                    let _ = write!(out, " {k}");
                }
                _ => {}
            }
        }
        if !v.styles.is_empty() {
            let mut s = String::new();
            for (k, val) in &v.styles {
                let _ = write!(s, "{k}: {val}; ");
            }
            let _ = write!(out, " style=\"{}\"", esc_attr(s.trim_end()));
        }
        out.push('>');
        if VOID.contains(&v.tag) {
            return;
        }
        if let Some(t) = v.text.as_deref().or(text_value) {
            out.push_str(&esc(t));
        }
        for (k, c) in v.children.iter().enumerate() {
            path.push(k);
            self.node(c, ctl, path, slots, out);
            path.pop();
        }
        for (slot, at) in slots {
            if at == path {
                for &kid in &self.kids[&(ctl, *slot)] {
                    self.control(kid, out);
                }
            }
        }
        let _ = write!(out, "</{}>", v.tag);
    }
}

/// Write onto `tree` what a running program had written: every property of the dump that is not what
/// the design already says, through the program's own door (`FormTree::set`).
fn apply_state(tree: &mut FormTree, state: &serde_json::Value) -> (usize, usize) {
    let (mut written, mut skipped) = (0, 0);
    let Some(map) = state.as_object() else { return (0, 0) };
    for (name, s) in map {
        let Some(i) = tree.by_name(name) else {
            skipped += 1;
            continue;
        };
        if i == 0 {
            continue;
        }
        let was_visible = tree.get(i, "Visible");
        let visible = if s["visible"].as_bool().unwrap_or(true) { "1" } else { "0" };
        if was_visible != visible {
            tree.set(i, "Visible", visible);
            written += 1;
        }
        let Some(props) = s["props"].as_object() else { continue };
        for (k, v) in props {
            let Some(v) = v.as_str() else { continue };
            if k.eq_ignore_ascii_case("visible") || k.eq_ignore_ascii_case("name") {
                continue;
            }
            // A value the design already holds is no program's write.
            let current = tree.get(i, k);
            if current == v || (current.is_empty() && v.is_empty()) {
                continue;
            }
            // Numbers: "10" and "10.0" are one value.
            if let (Ok(a), Ok(b)) = (current.trim().parse::<f64>(), v.trim().parse::<f64>()) {
                if (a - b).abs() < 1e-9 {
                    continue;
                }
            }
            tree.set(i, k, v);
            written += 1;
        }
    }
    (written, skipped)
}

/// The script `--geometry` puts in the page: every control's box, in page pixels, as JSON.
const GEOMETRY_SCRIPT: &str = r#"<script>
(function () {
  function run() {
    var root = document.getElementById('pr-app').shadowRoot, out = {};
    root.querySelectorAll('[data-name]').forEach(function (e) {
      var r = e.getBoundingClientRect(), cs = getComputedStyle(e);
      out[e.getAttribute('data-name')] = [r.left, r.top, r.width, r.height, cs.display === 'none' ? 0 : 1];
    });
    var pre = document.createElement('pre'); pre.id = 'geom'; pre.textContent = JSON.stringify(out);
    document.body.appendChild(pre);
  }
  if (document.readyState === 'complete') run(); else window.addEventListener('load', run);
})();
</script>"#;

/// The boxes a browser reported: `{"NAME": [x, y, w, h, shown]}` from the dumped DOM's `<pre id="geom">`.
fn read_geometry(dom: &str) -> BTreeMap<String, [f64; 5]> {
    let start = dom.find("<pre id=\"geom\">").expect("the dumped DOM has no <pre id=\"geom\">: was the page made with --geometry?") + "<pre id=\"geom\">".len();
    let end = dom[start..].find("</pre>").expect("an unclosed <pre id=\"geom\">") + start;
    let text = dom[start..end].replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&");
    serde_json::from_str(&text).expect("the geometry is JSON")
}

/// Compare the browser's boxes with the desktop solver's, and say where they differ.
fn compare_geometry(form: &cobolt_forms::Form, dom: &str, w: u32, h: u32) {
    let web = read_geometry(dom);
    let input = cobolt_forms::layout::LayoutInput::new(&form.controls, (form.width as f32, form.height as f32), (w as f32, h as f32), &form.layout, &form.breakpoints);
    let out = cobolt_forms::layout::solve(&input);
    struct Row {
        id: String,
        kind: String,
        desk: [f64; 4],
        web: Option<[f64; 4]>,
        worst: f64,
    }
    let mut rows: Vec<Row> = Vec::new();
    let by_id: BTreeMap<&str, &cobolt_forms::Control> = form.controls.iter().map(|c| (c.id.as_str(), c)).collect();
    // Drawn on the desktop only when the control and every container above it is visible.
    let drawn = |c: &cobolt_forms::Control| {
        let mut cur = Some(c);
        while let Some(x) = cur {
            if !x.visible {
                return false;
            }
            cur = x.parent.as_deref().and_then(|p| by_id.get(p).copied());
        }
        true
    };
    for c in &form.controls {
        if !drawn(c) || !cobolt_forms::web_class::available_on_web(&c.control_type) {
            continue;
        }
        if c.control_type.is_non_visual() || out.hidden.contains(&c.id) {
            continue;
        }
        let Some(r) = out.rects.get(&c.id) else { continue };
        let desk = [r.x as f64, r.y as f64, r.w as f64, r.h as f64];
        let web_box = web.get(&c.id).filter(|b| b[4] > 0.5).map(|b| [b[0], b[1], b[2], b[3]]);
        let worst = match web_box {
            Some(b) => (0..4).map(|i| (b[i] - desk[i]).abs()).fold(0.0, f64::max),
            None => f64::INFINITY,
        };
        rows.push(Row { id: c.id.clone(), kind: c.control_type.as_str().to_owned(), desk, web: web_box, worst });
    }
    let total = rows.len();
    let exact = rows.iter().filter(|r| r.worst <= 0.51).count();
    let near = rows.iter().filter(|r| r.worst > 0.51 && r.worst <= 2.0).count();
    let far = rows.iter().filter(|r| r.worst > 2.0 && r.worst.is_finite()).count();
    let missing = rows.iter().filter(|r| r.web.is_none()).count();
    println!("geometry of {total} visible controls at {w}x{h} (breakpoint {:?}): within 0.5 px {exact}, within 2 px {near}, further {far}, no box in the page {missing}", out.breakpoint);
    rows.sort_by(|a, b| b.worst.partial_cmp(&a.worst).unwrap_or(std::cmp::Ordering::Equal));
    for r in rows.iter().filter(|r| r.worst > 0.51).take(40) {
        match r.web {
            Some(b) => println!(
                "  {:<22} {:<12} desktop {:>7.1} {:>7.1} {:>7.1} {:>7.1}   web {:>7.1} {:>7.1} {:>7.1} {:>7.1}   off by {:>6.1}",
                r.id, r.kind, r.desk[0], r.desk[1], r.desk[2], r.desk[3], b[0], b[1], b[2], b[3], r.worst
            ),
            None => println!("  {:<22} {:<12} desktop {:>7.1} {:>7.1} {:>7.1} {:>7.1}   web: no box", r.id, r.kind, r.desk[0], r.desk[1], r.desk[2], r.desk[3]),
        }
    }
}

fn main() {
    let a = args();
    let form = cobolt_forms::xml::load_form(&a.form).unwrap_or_else(|e| panic!("{}: {e}", a.form.display()));
    if let Some(dom) = &a.compare {
        let text = std::fs::read_to_string(dom).unwrap_or_else(|e| panic!("{}: {e}", dom.display()));
        compare_geometry(&form, &text, a.width.unwrap_or(form.width), a.height.unwrap_or(form.height));
        return;
    }
    let manifest = Manifest {
        abi: 1,
        generator: "parity_page".into(),
        project: "parity".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: vec![],
        events: vec![],
        forms: vec![FormEntry { id: 0, name: form.name.clone(), main: true, instances: 1, region_size: 64, controls: vec![], handlers: vec![], layers: vec![] }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut tree = FormTree::build(&form, &manifest.forms[0], &manifest, 0, None).expect("the form pairs with an empty manifest");
    if let Some(p) = &a.state {
        let text = std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let state: serde_json::Value = serde_json::from_str(&text).expect("the state is JSON");
        let (written, skipped) = apply_state(&mut tree, &state);
        eprintln!("state: {written} writes, {skipped} entries for no control");
    }
    let (w, h) = (a.width.unwrap_or(form.width), a.height.unwrap_or(form.height));

    // The breakpoint the page's width selects (`dom/instance.rs` `on_resize`) and what it changes.
    let bp = (!tree.breakpoints.is_empty())
        .then(|| cobolt_forms::layout::breakpoints::select(&tree.breakpoints, w as f32, None).cloned())
        .flatten();
    let ctls = tree.effective_ctls(bp.as_ref());
    let responsive = truthy(&tree.get(0, "Responsive"));
    let assets = BTreeMap::new();
    let empty = Empty::default();
    let cx = empty.cx(&tree);

    let n = ctls.len();
    let mut vnodes: Vec<Option<VNode>> = vec![None; n];
    let mut css: Vec<String> = vec![String::new(); n];
    css[0] = style::form_css(0, &ctls, responsive, &assets);
    for i in 1..n {
        if style::has_a_box(&ctls[i]) {
            vnodes[i] = Some(controls::view(&cx, i));
            css[i] = style::control_css(0, &ctls, i, responsive);
        }
    }
    // Controls in a hidden layer are not drawn (spec 091); the player hides them with the layer.
    let mut kids: BTreeMap<(usize, Slot), Vec<usize>> = BTreeMap::new();
    for i in style::mount_order(&ctls) {
        if vnodes[i].is_some() {
            kids.entry(style::mount_of(&ctls, i)).or_default().push(i);
        }
    }
    let page = Page { vnodes: &vnodes, kids: &kids };
    let mut surface = String::new();
    for &i in kids.get(&(0, Slot::Children)).map(Vec::as_slice).unwrap_or(&[]) {
        page.control(i, &mut surface);
    }

    let title = tree.get(0, "Title");
    let mut doc = String::new();
    let _ = write!(
        doc,
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><title>{t}</title>\n<style>\nhtml, body {{ margin: 0; padding: 0; }}\n#pr-app {{ {frame} overflow: hidden; color: rgb(255, 255, 255); background: {bd}; font: 14px/1.4 system-ui, sans-serif; }}\n</style></head>\n<body class=\"pr-window\"><div id=\"pr-app\" class=\"pr-window\"><template shadowrootmode=\"open\">\n<style>\n:host {{ display: block; }}\n.pr-root {{ width: 100%; height: 100%; }}\n{base}\n{props}\n</style>\n<div class=\"pr-root\"><div class=\"pr-banner-host\"></div><div class=\"pr-main\"><section class=\"pr-form\" data-pr-form=\"{f}\" data-pr-inst=\"0\" aria-label=\"{t}\"{resp}><div class=\"pr-surface\">{surface}</div></section></div></div>\n</template></div>{geom}</body></html>\n",
        t = esc_attr(&title),
        bd = a.backdrop,
        // The window the host page gives the application, or — to measure — a box of the page's own size
        // whatever the browser's viewport is (a headless browser's DOM dump has a shorter one).
        frame = if a.geometry { format!("position: absolute; left: 0; top: 0; width: {w}px; height: {h}px;") } else { "position: fixed; inset: 0; width: 100vw; height: 100vh;".to_owned() },
        base = base_css::base_layer(),
        props = style::wrap_layer(&css),
        f = esc_attr(&tree.form_name),
        resp = if responsive { " data-responsive" } else { "" },
        surface = surface,
        geom = if a.geometry { GEOMETRY_SCRIPT } else { "" },
    );
    std::fs::write(&a.out, &doc).unwrap_or_else(|e| panic!("{}: {e}", a.out.display()));
    let drawn = vnodes.iter().filter(|v| v.is_some()).count();
    let bp_name = bp.map(|b| b.name).unwrap_or_else(|| "(none)".into());
    let _ = HashMap::<(), ()>::new();
    eprintln!("wrote {} ({} bytes): {drawn} of {} controls drawn, breakpoint {bp_name}, page {w}x{h}", a.out.display(), doc.len(), n - 1);
}
