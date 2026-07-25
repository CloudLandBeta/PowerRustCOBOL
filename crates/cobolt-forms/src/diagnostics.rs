// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Per-control diagnostics dump.
//!
//! When any diagnostic setting is enabled, the running form writes a detailed,
//! human-readable record of the whole form — every control, its geometry, all of
//! its properties, its events, animations, data bindings, and (for a DataGrid)
//! its advanced column configuration — to
//! `/tmp/<project>_diagnostics_dump.log`. It is meant to be opened alongside the
//! frame / component overlays so a problem seen on screen can be traced back to
//! the exact control state that produced it.
//!
//! Deliberately egui-free: it works off the [`Form`] model alone, so the dump is
//! identical whether produced by the IDE preview or a standalone `rcrun run-form`.

use std::fmt::Write as _;

use crate::model::{DataGridAdvanced, DATAGRID_ADVANCED_PROP};
use crate::{Control, Form};

/// Build the full diagnostics dump for `form`. `project` names the project (used
/// in the header); `enabled` lists each diagnostic flag and whether it is on, so
/// the reader knows which overlays were active for this run.
pub fn dump_form_diagnostics(form: &Form, project: &str, enabled: &[(&str, bool)]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "===== PowerRustCOBOL diagnostics dump =====");
    let _ = writeln!(out, "project    : {project}");
    let _ = writeln!(
        out,
        "form       : {:?}  title={:?}  {}x{}",
        form.name, form.title, form.width, form.height
    );
    let _ = writeln!(out, "generated  : {}", timestamp());
    let flags: Vec<String> = enabled
        .iter()
        .map(|(name, on)| format!("{name}={}", if *on { "on" } else { "off" }))
        .collect();
    let _ = writeln!(out, "diagnostics: {}", flags.join("  "));
    let _ = writeln!(
        out,
        "background : color={:?} gradient={} transparency={} image={:?}",
        form.background_color,
        form.background_gradient_enabled,
        form.transparency,
        form.background_image
    );
    if !form.data_bindings.is_empty() {
        let _ = writeln!(out, "form data bindings: {}", form.data_bindings.len());
    }
    let total = count_controls(&form.controls);
    let _ = writeln!(out, "controls   : {total} total\n");

    let mut index = 0usize;
    for ctrl in &form.controls {
        dump_control(&mut out, ctrl, 0, &mut index);
    }
    out
}

/// Total control count including nested children.
fn count_controls(controls: &[Control]) -> usize {
    controls
        .iter()
        .map(|c| 1 + count_controls(&c.children))
        .sum()
}

fn dump_control(out: &mut String, ctrl: &Control, depth: usize, index: &mut usize) {
    *index += 1;
    let pad = "  ".repeat(depth);
    let _ = writeln!(
        out,
        "{pad}[{index}] {} ({:?})",
        ctrl.id, ctrl.control_type
    );
    let _ = writeln!(
        out,
        "{pad}    rect   : x={} y={} w={} h={}",
        ctrl.rect.x, ctrl.rect.y, ctrl.rect.w, ctrl.rect.h
    );
    let _ = writeln!(
        out,
        "{pad}    order  : z={} tab={}   visible={} enabled={}",
        ctrl.z_order, ctrl.tab_order, ctrl.visible, ctrl.enabled
    );
    if let Some(parent) = &ctrl.parent {
        let _ = write!(out, "{pad}    parent : {parent}");
        if let Some(tab) = ctrl.tab {
            let _ = write!(out, "   tab-page={tab}");
        }
        let _ = writeln!(out);
    } else if let Some(tab) = ctrl.tab {
        let _ = writeln!(out, "{pad}    tab-page: {tab}");
    }

    // Properties, in the control's own declaration order.
    if ctrl.properties.is_empty() {
        let _ = writeln!(out, "{pad}    properties: (none)");
    } else {
        let _ = writeln!(out, "{pad}    properties ({}):", ctrl.properties.len());
        for (key, value) in &ctrl.properties {
            // Skip the packed DataGrid blob here; it is expanded below.
            if key == DATAGRID_ADVANCED_PROP {
                continue;
            }
            let _ = writeln!(out, "{pad}      {key} = {value}");
        }
    }

    // Events (handlers). Report presence + size, not the whole CDATA body.
    if !ctrl.events.is_empty() {
        let _ = writeln!(out, "{pad}    events ({}):", ctrl.events.len());
        for ev in &ctrl.events {
            let _ = writeln!(
                out,
                "{pad}      {} -> {} ({} bytes of code)",
                ev.event,
                ev.paragraph,
                ev.code.len()
            );
        }
    }

    if !ctrl.animations.is_empty() {
        let _ = writeln!(out, "{pad}    animations: {}", ctrl.animations.len());
    }

    // Expand the DataGrid's advanced configuration so its per-column setup is
    // legible next to the component-frame overlay.
    if ctrl.get_prop(DATAGRID_ADVANCED_PROP).is_some() {
        let adv = DataGridAdvanced::from_control(ctrl);
        let _ = writeln!(
            out,
            "{pad}    datagrid: {} column(s), grid-line={:?}, {} filter(s), {} row-height override(s)",
            adv.columns.len(),
            adv.grid_line_style,
            adv.filters.len(),
            adv.row_overrides.len()
        );
        for (i, col) in adv.columns.iter().enumerate() {
            let _ = writeln!(
                out,
                "{pad}      col[{i}] title={:?} width={} frozen={} bg={:?} rules={} gauge={}",
                col.title,
                col.width,
                col.frozen,
                col.background_color,
                col.value_style_rules.len(),
                col.gauge.is_some()
            );
        }
    }

    if !ctrl.children.is_empty() {
        let ids: Vec<&str> = ctrl.children.iter().map(|c| c.id.as_str()).collect();
        let _ = writeln!(
            out,
            "{pad}    children ({}): {}",
            ctrl.children.len(),
            ids.join(", ")
        );
    }
    let _ = writeln!(out);

    for child in &ctrl.children {
        dump_control(out, child, depth + 1, index);
    }
}

/// Best-effort wall-clock stamp without pulling in a date crate: ISO-ish UTC from
/// the Unix epoch. Enough to order dumps across runs.
fn timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => {
            let secs = d.as_secs();
            let (days, rem) = (secs / 86_400, secs % 86_400);
            let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
            // Civil date from days since 1970 (Howard Hinnant's algorithm).
            let z = days as i64 + 719_468;
            let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
            let doe = z - era * 146_097;
            let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
            let y = yoe + era * 400;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let day = doy - (153 * mp + 2) / 5 + 1;
            let month = if mp < 10 { mp + 3 } else { mp - 9 };
            let year = if month <= 2 { y + 1 } else { y };
            format!(
                "{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z (epoch {secs})"
            )
        }
        Err(_) => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ControlType, PropValue};

    #[test]
    fn dump_lists_every_control_with_geometry_and_props() {
        let mut form = Form::new("My App", "Main", 800, 600);
        let mut panel = Control::new("PNL", ControlType::Panel, 10, 20);
        panel.rect = crate::model::Rect::new(10, 20, 400, 300);
        let mut label = Control::new("LBL", ControlType::Label, 12, 22);
        label.set_prop("Text", PropValue::String("Hello".into()));
        label.parent = Some("PNL".into());
        panel.children.push(label);
        form.controls.push(panel);

        let dump = dump_form_diagnostics(
            &form,
            "My App",
            &[("frame_diagnostics", true), ("datagrid_diagnostics", false)],
        );

        assert!(dump.contains("project    : My App"));
        assert!(dump.contains("controls   : 2 total"));
        assert!(dump.contains("PNL (Panel)"));
        assert!(dump.contains("rect   : x=10 y=20 w=400 h=300"));
        assert!(dump.contains("LBL (Label)"));
        assert!(dump.contains("Text = Hello"));
        assert!(dump.contains("parent : PNL"));
        assert!(dump.contains("frame_diagnostics=on"));
        assert!(dump.contains("datagrid_diagnostics=off"));
    }
}
