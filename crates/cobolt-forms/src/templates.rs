// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 079 — forms to start from instead of a blank canvas.
//!
//! Every template is a responsive form in the modern style
//! ([`crate::style`]): its controls are anchored, or placed by a flex row, so
//! it follows its window from the first run. Layout and look only — no event
//! handler is written; the developer's code is theirs to write.
//!
//! The captions come in through [`Texts`], so the IDE hands them over in its
//! own language; [`Texts::default`] is English.

use crate::model::{Control, ControlType, Form, PropValue};
use crate::style;

/// What the New Form dialog offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormTemplate {
    Blank,
    RecordEntry,
    ListDetails,
    Dashboard,
}

impl FormTemplate {
    pub const ALL: [FormTemplate; 4] = [Self::Blank, Self::RecordEntry, Self::ListDetails, Self::Dashboard];

    /// The size the template is drawn at.
    pub fn size(self) -> (u32, u32) {
        match self {
            Self::Blank => (640, 480),
            Self::RecordEntry => (720, 480),
            Self::ListDetails => (960, 560),
            Self::Dashboard => (1024, 640),
        }
    }
}

/// The captions a template writes, in the IDE's language.
#[derive(Debug, Clone)]
pub struct Texts {
    pub code: String,
    pub name: String,
    pub email: String,
    pub phone: String,
    pub new: String,
    pub save: String,
    pub delete: String,
    pub search: String,
    pub details: String,
    pub kpis: [String; 3],
    pub charts: [String; 2],
}

impl Default for Texts {
    fn default() -> Self {
        Self {
            code: "Code".into(),
            name: "Name".into(),
            email: "E-mail".into(),
            phone: "Phone".into(),
            new: "New".into(),
            save: "Save".into(),
            delete: "Delete".into(),
            search: "Search...".into(),
            details: "Details".into(),
            kpis: ["Sales".into(), "Orders".into(), "Customers".into()],
            charts: ["Monthly revenue".into(), "Orders by week".into()],
        }
    }
}

struct B<'a> {
    form: &'a mut Form,
    z: i32,
    tab: u32,
}

impl B<'_> {
    fn add(&mut self, ct: ControlType, id: &str, (x, y, w, h): (i32, i32, i32, i32), parent: Option<&str>) -> &mut Control {
        let mut c = Control::new(id, ct, x, y);
        c.rect.w = w;
        c.rect.h = h;
        c.parent = parent.map(str::to_owned);
        c.z_order = self.z;
        self.z += 1;
        if !matches!(c.control_type, ControlType::Label | ControlType::Panel | ControlType::GroupBox) {
            self.tab += 1;
            c.tab_order = self.tab;
        }
        style::style_control(&mut c);
        self.form.controls.push(c);
        self.form.controls.last_mut().unwrap()
    }
    fn label(&mut self, id: &str, text: &str, r: (i32, i32, i32, i32), parent: Option<&str>) -> &mut Control {
        let c = self.add(ControlType::Label, id, r, parent);
        c.set_prop("Caption", PropValue::String(text.into()));
        c
    }
    fn button(&mut self, id: &str, text: &str, r: (i32, i32, i32, i32), parent: Option<&str>, primary: bool) -> &mut Control {
        let c = self.add(ControlType::Button, id, r, parent);
        if !primary {
            style::style_secondary_button(c);
        }
        c.set_prop("Caption", PropValue::String(text.into()));
        c
    }
}

fn anchor(c: &mut Control, edges: &str) {
    c.set_prop("Anchor", PropValue::String(edges.into()));
}

/// A new form from `template`: named, titled, responsive and modern. The
/// size is the template's own; `Blank` takes `size` when given.
pub fn build(template: FormTemplate, name: &str, title: &str, size: Option<(u32, u32)>, t: &Texts) -> Form {
    let (w, h) = match template {
        FormTemplate::Blank => size.unwrap_or_else(|| template.size()),
        other => other.size(),
    };
    let mut form = Form::new(name, title, w, h);
    form.responsive = true;
    style::style_form(&mut form);
    let mut b = B { form: &mut form, z: 0, tab: 0 };
    let (w, h) = (w as i32, h as i32);
    match template {
        FormTemplate::Blank => {}
        FormTemplate::RecordEntry => {
            let title_c = b.label("LBL-TITLE", title, (24, 20, w - 48, 32), None);
            style::style_title(title_c);
            anchor(title_c, "Top,Left,Right");
            let card = b.add(ControlType::Panel, "PNL-RECORD", (24, 68, w - 48, 316), None);
            anchor(card, "Top,Left,Right");
            card.set_prop("MinWidth", PropValue::Int((w - 48) as i64));
            let fields: [(&str, &str, i32, bool); 4] = [
                ("CODE", &t.code, 140, false),
                ("NAME", &t.name, w - 88, true),
                ("EMAIL", &t.email, w - 88, true),
                ("PHONE", &t.phone, 220, false),
            ];
            for (i, (key, caption, fw, stretch)) in fields.into_iter().enumerate() {
                let y = 84 + i as i32 * 72;
                b.label(&format!("LBL-{key}"), caption, (44, y, 300, 20), Some("PNL-RECORD"));
                let tb = b.add(ControlType::TextBox, &format!("TXT-{key}"), (44, y + 24, fw, 32), Some("PNL-RECORD"));
                if stretch {
                    anchor(tb, "Top,Left,Right");
                    tb.set_prop("MinWidth", PropValue::Int(fw as i64));
                }
            }
            let y = h - 72;
            for (i, (id, text, primary)) in [("BTN-NEW", &t.new, false), ("BTN-DELETE", &t.delete, false), ("BTN-SAVE", &t.save, true)]
                .into_iter()
                .enumerate()
            {
                let x = w - 24 - 112 * (3 - i as i32) + 8;
                let c = b.button(id, text, (x, y, 104, 36), None, primary);
                anchor(c, "Bottom,Right");
            }
        }
        FormTemplate::ListDetails => {
            let title_c = b.label("LBL-TITLE", title, (24, 20, w - 48, 32), None);
            style::style_title(title_c);
            anchor(title_c, "Top,Left,Right");
            let s = b.add(ControlType::TextBox, "TXT-SEARCH", (24, 64, 316, 32), None);
            s.set_prop("HintText", PropValue::String(t.search.clone()));
            let n = b.button("BTN-NEW", &t.new, (352, 64, 92, 32), None, false);
            anchor(n, "Top,Left");
            let g = b.add(ControlType::DataGrid, "GRD-LIST", (24, 108, 420, h - 132), None);
            anchor(g, "Top,Bottom,Left");
            g.set_prop("Columns", PropValue::String(format!("{}:string\n{}:string", t.code, t.name)));
            let pw = w - 488;
            let card = b.add(ControlType::Panel, "PNL-DETAILS", (464, 64, pw, h - 88), None);
            anchor(card, "Top,Bottom,Left,Right");
            card.set_prop("MinWidth", PropValue::Int(pw as i64));
            let dt = b.label("LBL-DETAILS", &t.details, (484, 80, pw - 40, 24), Some("PNL-DETAILS"));
            style::style_title(dt);
            dt.set_prop("FontSize", PropValue::Int(16));
            anchor(dt, "Top,Left,Right");
            for (i, (key, caption)) in [("NAME", &t.name), ("EMAIL", &t.email), ("PHONE", &t.phone)].into_iter().enumerate() {
                let y = 120 + i as i32 * 72;
                b.label(&format!("LBL-{key}"), caption, (484, y, 300, 20), Some("PNL-DETAILS"));
                let tb = b.add(ControlType::TextBox, &format!("TXT-{key}"), (484, y + 24, pw - 40, 32), Some("PNL-DETAILS"));
                anchor(tb, "Top,Left,Right");
                tb.set_prop("MinWidth", PropValue::Int((pw - 40) as i64));
            }
            let y = h - 24 - 16 - 36;
            let d = b.button("BTN-DELETE", &t.delete, (464 + pw - 20 - 104 - 112, y, 104, 36), Some("PNL-DETAILS"), false);
            anchor(d, "Bottom,Right");
            let sv = b.button("BTN-SAVE", &t.save, (464 + pw - 20 - 104, y, 104, 36), Some("PNL-DETAILS"), true);
            anchor(sv, "Bottom,Right");
        }
        FormTemplate::Dashboard => {
            let title_c = b.label("LBL-TITLE", title, (24, 20, w - 48, 32), None);
            style::style_title(title_c);
            anchor(title_c, "Top,Left,Right");
            // Three indicator cards sharing the width (a flex row).
            let row = b.add(ControlType::Panel, "PNL-KPIS", (24, 68, w - 48, 112), None);
            anchor(row, "Top,Left,Right");
            row.set_prop("LayoutMode", PropValue::String("Flex".into()));
            row.set_prop("Gap", PropValue::Int(16));
            row.set_prop("MinWidth", PropValue::Int((w - 48) as i64));
            for p in ["BackgroundColor"] {
                row.set_prop(p, PropValue::String(style::palette::CLEAR.into()));
            }
            row.set_prop("BorderStyle", PropValue::String("None".into()));
            row.set_prop("ShadowEnabled", PropValue::Bool(false));
            let cw = (w - 48 - 32) / 3;
            for i in 0..3 {
                let x = 24 + i * (cw + 16);
                let id = format!("PNL-KPI{}", i + 1);
                let card = b.add(ControlType::Panel, &id, (x, 68, cw, 112), Some("PNL-KPIS"));
                card.set_prop("FlexGrow", PropValue::String("1".into()));
                let cap = b.label(&format!("LBL-KPI{}", i + 1), &t.kpis[i as usize], (x + 20, 84, cw - 40, 20), Some(&id));
                style::style_muted(cap);
                let v = b.label(&format!("LBL-KPI{}-VALUE", i + 1), "0", (x + 20, 112, cw - 40, 44), Some(&id));
                style::style_title(v);
                v.set_prop("FontSize", PropValue::Int(30));
            }
            // Two charts sharing the rest of the window.
            let row = b.add(ControlType::Panel, "PNL-CHARTS", (24, 200, w - 48, h - 224), None);
            anchor(row, "Top,Bottom,Left,Right");
            row.set_prop("LayoutMode", PropValue::String("Flex".into()));
            row.set_prop("Gap", PropValue::Int(16));
            row.set_prop("MinWidth", PropValue::Int((w - 48) as i64));
            row.set_prop("BackgroundColor", PropValue::String(style::palette::CLEAR.into()));
            row.set_prop("BorderStyle", PropValue::String("None".into()));
            row.set_prop("ShadowEnabled", PropValue::Bool(false));
            let chw = (w - 48 - 16) / 2;
            for (i, (ct, id)) in [(ControlType::BarChart, "CHT-REVENUE"), (ControlType::LineChart, "CHT-ORDERS")].into_iter().enumerate() {
                let c = b.add(ct, id, (24 + i as i32 * (chw + 16), 200, chw, h - 224), Some("PNL-CHARTS"));
                c.set_prop("FlexGrow", PropValue::String("1".into()));
                c.set_prop("Title", PropValue::String(t.charts[i].clone()));
            }
        }
    }
    form
}
