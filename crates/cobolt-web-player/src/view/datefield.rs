// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A date field that is typed into, and its calendar** (2.0.432): the elements of a DateTimePicker that edits a date
//! alone (`Format` `Short`, `Long`, or a `Custom` pattern with no time in it; not `ShowUpDown`).
//!
//! * The field is a text box: the date is typed into it, in the order it is shown (`crate::calendar::Typed`), and is
//!   committed when the box is left or Enter is pressed. At rest it shows the date in the field's own format.
//! * Beside it a button with the calendar's icon opens the calendar: a modal `<dialog>`, which the browser draws in its top
//!   layer, so no Panel clips it and the page behind it takes no press. The page places it under the field
//!   (`dom::h::show_popup`).
//! * The calendar shows one month, six weeks from a Sunday. Two lists jump to any month and year, two buttons step a month.
//!   Today is ringed, the day the field holds is filled, a day outside `MinimumDate`..`MaximumDate` cannot be pressed, and a
//!   holy day that is a holiday for the whole country of the page's locale carries a dot and is named under the month.
//! * Its colours are two, the ground and the ink, so every text on it has the contrast of that pair: the field's own
//!   `BackgroundColor` when it is opaque, with black or white ink, whichever reads better on it; else the theme's surface
//!   and text (`--pr-surface`, `--pr-text`).
//!
//! A picker with a time in it, and a `ShowUpDown` one, are the browser's own date and time fields as before.

use cobolt_forms::fieldtext::{date_in_bounds, display_dt_for, dt_parts, parse_ymd};
use cobolt_forms::model::ControlType;

use super::controls::{field_glyph, model_for, root};
use super::{CalendarUi, VNode, ViewCtx};
use crate::calendar::{country, day_label, holiday_on, iso, month_cells, texts, year_range, Typed, Ymd};
use crate::tree::Ctl;
use crate::value::{css_color, parse_color, truthy};

/// Is control `i` a date field that is typed into: a DateTimePicker that edits a date and no time, without `ShowUpDown`?
pub fn is_typed(cx: &ViewCtx, i: usize) -> bool {
    if !matches!(cx.tree.ctl(i).control_type(), Some(ControlType::DateTimePicker)) {
        return false;
    }
    let parts = dt_parts(&model_for(cx, i, &["Format", "CustomFormat"]));
    parts.date && !parts.time && !truthy(&cx.tree.get(i, "ShowUpDown"))
}

/// The dates the field takes: `MinimumDate` and `MaximumDate`.
pub fn bounds(cx: &ViewCtx, i: usize) -> (Option<Ymd>, Option<Ymd>) {
    (parse_ymd(&cx.tree.get(i, "MinimumDate")), parse_ymd(&cx.tree.get(i, "MaximumDate")))
}

/// How the field's date is typed.
pub fn typed(cx: &ViewCtx, i: usize) -> Typed {
    Typed::of(&cx.tree.get(i, "Format"), &cx.tree.get(i, "CustomFormat"))
}

pub fn date_field(cx: &ViewCtx, i: usize) -> VNode {
    let c = cx.tree.ctl(i);
    let value = cx.tree.get(i, "Value");
    let held = parse_ymd(&value);
    let typed = typed(cx, i);
    let t = texts(cx.locale);
    // Under the caret, the date as it is typed; at rest, as the field's format shows it. A `Value` that is no date is the
    // developer's text and is shown as it stands.
    let shown = if cx.ui.focused == Some(i) {
        held.map(|d| typed.show(d)).unwrap_or_else(|| value.trim().to_owned())
    } else {
        display_dt_for(&model_for(cx, i, &["Value", "Format", "CustomFormat"]), &value)
    };
    let open = cx.ui.calendar.filter(|k| k.ctl == i);
    let input = VNode::new("input")
        .key("input")
        .attr("type", "text")
        .attr("inputmode", "numeric")
        .attr("autocomplete", "off")
        .attr("spellcheck", "false")
        .attr("placeholder", typed.pattern())
        .prop_str("value", shown)
        .prop_bool("disabled", !c.enabled())
        .on("change")
        .on("focusin")
        .on("focusout");
    let button = VNode::new("button")
        .key("open")
        .class("pr-dt-open")
        .attr("type", "button")
        .attr("data-part", "open")
        .attr("aria-label", t.open)
        .attr("title", t.open)
        .attr("aria-haspopup", "dialog")
        .attr("aria-expanded", if open.is_some() { "true" } else { "false" })
        .prop_bool("disabled", !c.enabled())
        .on("click")
        .child(field_glyph(true));
    let mut n = root(cx, i, "div").class("pr-dt-typed").child(input).child(button);
    if let Some(k) = open {
        n = n.child(calendar(cx, i, k, held));
    }
    n
}

/// The relative luminance of a colour (WCAG 2).
fn luminance(c: [u8; 4]) -> f64 {
    let lin = |v: u8| {
        let s = v as f64 / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// The contrast ratio of two colours (WCAG 2): 1 for the same colour, 21 for black on white.
pub fn contrast(a: [u8; 4], b: [u8; 4]) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// The calendar's ground and ink when the field has an opaque `BackgroundColor` of its own: that colour, and black or white,
/// whichever stands out more against it (never under 4.5 to 1, on any ground: the two meet at 4.58 on a mid grey). `None` leaves both to the theme.
pub fn calendar_colors(c: &Ctl) -> Option<([u8; 4], [u8; 4])> {
    let ground = c.raw("backgroundcolor").and_then(parse_color).filter(|g| g[3] >= 230).map(|g| [g[0], g[1], g[2], 255])?;
    let (black, white) = ([0, 0, 0, 255], [255, 255, 255, 255]);
    Some((ground, if contrast(ground, black) >= contrast(ground, white) { black } else { white }))
}

fn calendar(cx: &ViewCtx, i: usize, k: CalendarUi, held: Option<Ymd>) -> VNode {
    let t = texts(cx.locale);
    let land = country(cx.locale);
    let bounds = bounds(cx, i);
    let step = |part: &'static str, label: &'static str, glyph: &'static str| {
        VNode::new("button").key(part).class("pr-cal-step").attr("type", "button").attr("data-part", part).attr("aria-label", label).attr("title", label).text(glyph)
    };
    let months = VNode::new("select").key("month").attr("data-part", "month").attr("aria-label", t.month).prop_str("value", k.month.to_string()).children(
        (1..=12u32).map(|m| VNode::new("option").key(format!("m{m}")).prop_str("value", m.to_string()).prop_bool("selected", m == k.month).text(t.months[(m - 1) as usize])),
    );
    let (first, last) = year_range(k.year, k.today.0, bounds);
    let years = VNode::new("select").key("year").attr("data-part", "year").attr("aria-label", t.year).prop_str("value", k.year.to_string()).children(
        (first..=last).map(|y| VNode::new("option").key(format!("y{y}")).prop_str("value", y.to_string()).prop_bool("selected", y == k.year).text(y.to_string())),
    );
    let head = VNode::new("div").key("head").class("pr-cal-head").child(step("previous", t.previous, "‹")).child(months).child(years).child(step("next", t.next, "›"));
    let week = VNode::new("div")
        .key("week")
        .class("pr-cal-week")
        .attr("aria-hidden", "true")
        .children(t.weekdays.iter().enumerate().map(|(n, w)| VNode::new("span").key(format!("w{n}")).text(*w)));
    let mut named = Vec::new();
    let mut grid = VNode::new("div").key("grid").class("pr-cal-grid");
    for (n, d) in month_cells(k.year, k.month).into_iter().enumerate() {
        let holiday = holiday_on(&land, d);
        let of_month = (d.0, d.1) == (k.year, k.month);
        let mut b = VNode::new("button")
            .key(format!("d{n}"))
            .class("pr-cal-day")
            .attr("type", "button")
            .attr("data-part", "day")
            .attr("data-id", iso(d))
            .attr("aria-label", day_label(cx.locale, d, holiday))
            .text(d.2.to_string());
        if !of_month {
            b = b.class("pr-cal-out");
        }
        if d == k.today {
            b = b.class("pr-cal-today").attr("aria-current", "date");
        }
        if Some(d) == held {
            b = b.class("pr-cal-held").attr("aria-pressed", "true");
        }
        if let Some(h) = holiday {
            b = b.class("pr-cal-holy").attr("title", h.name(cx.locale));
            if of_month {
                named.push((d.2, h));
            }
        }
        if !date_in_bounds(d, bounds) {
            b = b.prop_bool("disabled", true);
        }
        grid = grid.child(b);
    }
    let mut panel = VNode::new("div").key("panel").class("pr-cal-panel").attr("data-part", "panel").child(head).child(week).child(grid);
    if !named.is_empty() {
        panel = panel.child(
            VNode::new("ul")
                .key("holy")
                .class("pr-cal-names")
                .children(named.iter().map(|(day, h)| VNode::new("li").key(format!("h{day}")).text(format!("{day} · {}", h.name(cx.locale))))),
        );
    }
    let act = |part: &'static str, label: &'static str| VNode::new("button").key(part).attr("type", "button").attr("data-part", part).text(label);
    panel = panel.child(VNode::new("div").key("foot").class("pr-cal-foot").child(act("today", t.today)).child(act("clear", t.clear)).child(act("close", t.close)));
    // `data-part="backdrop"` is what a press outside the panel finds: the dialog itself, whose backdrop is the rest of the page.
    let mut dialog = VNode::new("dialog")
        .key("cal")
        .class("pr-cal")
        .attr("data-pr-popup", "1")
        .attr("data-part", "backdrop")
        .attr("aria-label", format!("{} {}", t.months[(k.month.clamp(1, 12) - 1) as usize], k.year))
        .on("click")
        .on("change")
        .on("keydown")
        .on("cancel")
        .child(panel);
    if let Some((ground, ink)) = calendar_colors(cx.tree.ctl(i)) {
        dialog = dialog.style("--pr-cal-ground", css_color(ground)).style("--pr-cal-ink", css_color(ink));
    }
    dialog
}

#[cfg(test)]
mod tests {
    use super::super::controls::view;
    use super::super::{Empty, Prop};
    use super::*;
    use crate::tree::testing::tree_with;

    fn open_on(e: &mut Empty, ctl: usize, year: i32, month: u32, today: Ymd) {
        e.ui.calendar = Some(CalendarUi { ctl, year, month, today });
    }

    #[test]
    fn a_date_only_picker_is_a_text_box_with_a_calendar_button_and_the_others_are_the_browsers_own_fields() {
        let e = Empty::default();
        for (props, typed) in [
            (&[("Format", "Short")][..], true),
            (&[("Format", "Long")][..], true),
            (&[][..], true),
            (&[("Format", "Custom"), ("CustomFormat", "dd/MM/yyyy")][..], true),
            (&[("Format", "Time")][..], false),
            (&[("Format", "Custom"), ("CustomFormat", "dd/MM/yyyy HH:mm")][..], false),
            (&[("Format", "Short"), ("ShowUpDown", "1")][..], false),
        ] {
            let (t, i) = tree_with("DateTimePicker", "DT-1", props);
            let cx = e.cx(&t);
            assert_eq!(is_typed(&cx, i), typed, "{props:?}");
            let n = view(&cx, i);
            let input = n.find(&|x| x.tag == "input").unwrap();
            assert_eq!(input.attr_of("type") == Some("text"), typed, "{props:?}: {}", n.describe());
            assert_eq!(n.find(&|x| x.attr_of("data-part") == Some("open")).is_some(), typed, "{props:?}: the calendar button");
            assert_eq!(n.has_class("pr-dt-typed"), typed);
        }
        assert!(!is_typed(&e.cx(&tree_with("TextBox", "T-1", &[]).0), 1), "another control is no date field");
    }

    #[test]
    fn the_field_shows_the_date_in_its_format_at_rest_and_as_it_is_typed_under_the_caret() {
        let field = |props: &[(&str, &str)], focused: bool| {
            let (t, i) = tree_with("DateTimePicker", "DT-1", props);
            let mut e = Empty::default();
            if focused {
                e.ui.focused = Some(i);
            }
            let n = view(&e.cx(&t), i);
            let input = n.find(&|x| x.tag == "input").unwrap().clone();
            (input.prop_of("value").cloned(), input.attr_of("placeholder").map(str::to_owned))
        };
        let s = |v: &str| Some(Prop::Str(v.to_owned()));
        // A custom pattern of numbers: one text at rest and under the caret, and the pattern as the empty field's hint.
        let br = [("Format", "Custom"), ("CustomFormat", "dd/MM/yyyy"), ("Value", "2026-11-03")];
        assert_eq!(field(&br, false), (s("03/11/2026"), Some("dd/MM/yyyy".to_owned())));
        assert_eq!(field(&br, true), (s("03/11/2026"), Some("dd/MM/yyyy".to_owned())));
        // `Short` shows and is typed as ISO.
        assert_eq!(field(&[("Value", "2026-11-03")], false), (s("2026-11-03"), Some("yyyy-MM-dd".to_owned())));
        // `Long` shows the long date at rest and the date to type under the caret.
        let long = [("Format", "Long"), ("Value", "2026-11-03")];
        assert_eq!(field(&long, false).0, s("Tuesday, 3 November 2026"));
        assert_eq!(field(&long, true).0, s("2026-11-03"));
        // Empty, and a value that is no date (the developer's text, shown as it stands).
        assert_eq!(field(&br[..2], false).0, s(""));
        assert_eq!(field(&[("Value", "soon")], true).0, s("soon"));
        // A disabled field takes neither typing nor a press on its button.
        let (t, i) = tree_with("DateTimePicker", "DT-1", &[("Enabled", "0")]);
        let n = view(&Empty::default().cx(&t), i);
        let off = |tag: &str| n.find(&|x| x.tag == tag).unwrap().prop_of("disabled") == Some(&Prop::Bool(true));
        assert!(off("input") && off("button"), "{}", n.describe());
    }

    #[test]
    fn the_open_calendar_shows_six_weeks_marks_today_the_held_day_and_the_holy_days_and_offers_every_month_and_year() {
        let (t, i) = tree_with("DateTimePicker", "DT-1", &[("Value", "2026-12-08"), ("MinimumDate", "2026-12-03"), ("MaximumDate", "2027-06-30")]);
        let mut e = Empty::default();
        // Closed: no dialog at all.
        let closed = view(&e.cx(&t), i);
        assert!(closed.find(&|x| x.tag == "dialog").is_none());
        assert_eq!(closed.find(&|x| x.attr_of("data-part") == Some("open")).unwrap().attr_of("aria-expanded"), Some("false"));
        open_on(&mut e, i, 2026, 12, (2026, 12, 11));
        let cx = ViewCtx { locale: "pt-BR", ..e.cx(&t) };
        let n = view(&cx, i);
        assert_eq!(n.find(&|x| x.attr_of("data-part") == Some("open")).unwrap().attr_of("aria-expanded"), Some("true"));
        let d = n.find(&|x| x.tag == "dialog").unwrap_or_else(|| panic!("no dialog: {}", n.describe()));
        assert_eq!((d.attr_of("data-pr-popup"), d.attr_of("data-part"), d.attr_of("aria-label")), (Some("1"), Some("backdrop"), Some("dezembro 2026")));
        for kind in ["click", "change", "keydown", "cancel"] {
            assert!(d.on.contains(&kind), "the dialog hears {kind}");
        }
        let mut days = Vec::new();
        d.find_all(&|x| x.attr_of("data-part") == Some("day"), &mut days);
        assert_eq!(days.len(), 42, "six weeks");
        // December 2026 begins on a Tuesday: two days of November lead it, nine of January follow.
        assert_eq!((days[0].attr_of("data-id"), days[2].attr_of("data-id"), days[41].attr_of("data-id")), (Some("2026-11-29"), Some("2026-12-01"), Some("2027-01-09")));
        let day = |id: &str| *days.iter().find(|x| x.attr_of("data-id") == Some(id)).unwrap();
        assert!(day("2026-11-30").has_class("pr-cal-out") && !day("2026-12-01").has_class("pr-cal-out"));
        assert!(day("2026-12-11").has_class("pr-cal-today") && day("2026-12-11").attr_of("aria-current") == Some("date"));
        assert!(day("2026-12-08").has_class("pr-cal-held") && day("2026-12-08").attr_of("aria-pressed") == Some("true"));
        // Brazil keeps Christmas and not the Immaculate Conception; the day says its name, and the month's holy days are listed.
        assert!(day("2026-12-25").has_class("pr-cal-holy") && !day("2026-12-08").has_class("pr-cal-holy"));
        assert_eq!((day("2026-12-25").attr_of("title"), day("2026-12-25").attr_of("aria-label")), (Some("Natal"), Some("25 dezembro 2026, Natal")));
        let names = d.find(&|x| x.has_class("pr-cal-names")).unwrap();
        assert_eq!(names.all_text(), "25 · Natal");
        // A day before `MinimumDate` cannot be pressed; one inside the bounds can.
        assert_eq!(day("2026-12-02").prop_of("disabled"), Some(&Prop::Bool(true)));
        assert_eq!(day("2026-12-03").prop_of("disabled"), None);
        // The lists: twelve months in the page's language, and the years of the bounds.
        let list = |part: &str| d.find(&|x| x.tag == "select" && x.attr_of("data-part") == Some(part)).unwrap();
        assert_eq!((list("month").children.len(), list("month").prop_of("value"), list("month").children[11].all_text()), (12, Some(&Prop::Str("12".into())), "dezembro".to_owned()));
        assert_eq!(list("year").children.iter().map(|o| o.all_text()).collect::<Vec<_>>(), ["2026", "2027"]);
        assert_eq!(d.find(&|x| x.has_class("pr-cal-week")).unwrap().all_text(), "domsegterquaquisexsáb");
        for (part, text) in [("today", "Hoje"), ("clear", "Limpar"), ("close", "Fechar")] {
            assert_eq!(d.find(&|x| x.attr_of("data-part") == Some(part)).unwrap().all_text(), text);
        }
        // Spain keeps the 8th of December; a page in Japanese marks no day; an unbounded field offers a century of years.
        let (t, i) = tree_with("DateTimePicker", "DT-1", &[]);
        open_on(&mut e, i, 2026, 12, (2026, 12, 11));
        // (The grid of December 2026 runs to the 9th of January: Spain's Epiphany is on it, marked, though named under January only.)
        for (locale, holy) in [("es", vec!["2026-12-08", "2026-12-25", "2027-01-06"]), ("ja", vec![]), ("en", vec!["2026-12-25"]), ("fr", vec!["2026-12-25"])] {
            let n = view(&ViewCtx { locale, ..e.cx(&t) }, i);
            let mut marked = Vec::new();
            n.find_all(&|x| x.has_class("pr-cal-holy"), &mut marked);
            assert_eq!(marked.iter().map(|x| x.attr_of("data-id").unwrap()).collect::<Vec<_>>(), holy, "{locale}");
            assert_eq!(n.find(&|x| x.has_class("pr-cal-names")).is_some(), !holy.is_empty(), "{locale}: the list is there only for a month that has one");
        }
        let n = view(&e.cx(&t), i);
        let years = n.find(&|x| x.tag == "select" && x.attr_of("data-part") == Some("year")).unwrap();
        assert_eq!((years.children.len(), years.children[0].all_text(), years.children[110].all_text()), (111, "1926".to_owned(), "2036".to_owned()));
    }

    #[test]
    fn the_calendar_has_one_ground_and_one_ink_and_the_pair_always_reads() {
        // The field's own opaque ground, with the ink that reads better on it.
        let pair = |bg: &str| {
            let (t, i) = tree_with("DateTimePicker", "DT-1", &[("BackgroundColor", bg)]);
            calendar_colors(t.ctl(i))
        };
        assert_eq!(pair("#FFFFFFFF"), Some(([255, 255, 255, 255], [0, 0, 0, 255])));
        assert_eq!(pair("#1C223C"), Some(([28, 34, 60, 255], [255, 255, 255, 255])));
        // A see-through ground is no ground to read on: the theme's surface and text stand.
        assert_eq!(pair("#FFFFFF40"), None);
        assert_eq!(pair(""), None);
        // Whatever the ground, the ink chosen clears 4.5 to 1 (WCAG AA): every grey from black to white, and the six primaries.
        let mut worst = f64::MAX;
        let mut grounds: Vec<[u8; 4]> = (0..=255u8).map(|v| [v, v, v, 255]).collect();
        grounds.extend([[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 0, 255], [0, 255, 255, 255], [255, 0, 255, 255]]);
        for g in &grounds {
            let (black, white) = (contrast(*g, [0, 0, 0, 255]), contrast(*g, [255, 255, 255, 255]));
            worst = worst.min(black.max(white));
        }
        assert!(worst >= 4.5, "the worst ground still reads at {worst:.2} to 1");
        assert!((contrast([0, 0, 0, 255], [255, 255, 255, 255]) - 21.0).abs() < 0.01);
        // The dialog carries the pair as its two custom properties, and none when the theme decides.
        let (t, i) = tree_with("DateTimePicker", "DT-1", &[("BackgroundColor", "#FFFFFFFF")]);
        let mut e = Empty::default();
        open_on(&mut e, i, 2026, 11, (2026, 11, 11));
        let n = view(&e.cx(&t), i);
        let d = n.find(&|x| x.tag == "dialog").unwrap();
        assert_eq!(d.styles, [("--pr-cal-ground".to_owned(), "rgb(255 255 255)".to_owned()), ("--pr-cal-ink".to_owned(), "rgb(0 0 0)".to_owned())]);
        println!("{} grounds: the ink chosen never reads under {worst:.2} to 1", grounds.len());
    }
}
