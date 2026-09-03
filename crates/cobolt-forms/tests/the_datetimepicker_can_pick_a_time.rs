#![cfg(feature = "render")]
//! A DateTimePicker can pick a **time**, and `Format` decides what it edits.
//!
//! The popup was calendar-only: `parse_ymd`, a month grid, day cells. The field
//! could SHOW `09:30` because `Value` happened to hold it, and nothing anywhere
//! could change it (operator, 2026-09-03). `Format` was dead too — `Short`,
//! `Long`, `Time` and `Custom` all printed `Value` verbatim.
//!
//! The value stays canonical ISO whatever the format says (`YYYY-MM-DD`,
//! `HH:MM`, or the two space-separated), so a COBOL handler reading `Value`
//! gets one shape. `Format` decides what is editable and what is displayed; it
//! never decides how the value is stored.

use cobolt_forms::model::{Control, ControlType, PropValue, Rect as MRect};
use cobolt_forms::paint::{display_dt, dt_parts, format_dt_value, parse_hm, parse_ymd, DtParts};

fn picker(format: &str, custom: &str, value: &str) -> Control {
    let mut c = Control::new("DTP-1", ControlType::DateTimePicker, 20, 20);
    c.rect = MRect::new(20, 20, 180, 24);
    c.set_prop("Format", PropValue::String(format.to_owned()));
    c.set_prop("CustomFormat", PropValue::String(custom.to_owned()));
    c.set_prop("Value", PropValue::String(value.to_owned()));
    c
}

#[test]
fn a_value_carrying_a_time_still_reads_as_a_date() {
    // The bug in the way of the feature: splitting the whole string on `-` gave
    // three parts for "2026-09-03 09:30" too, but the last was "03 09:30",
    // which does not parse — so a value with a time read as NO date, and the
    // calendar opened on its hardcoded default month instead of the value's.
    assert_eq!(parse_ymd("2026-09-03"), Some((2026, 9, 3)));
    assert_eq!(parse_ymd("2026-09-03 09:30"), Some((2026, 9, 3)));
    assert_eq!(parse_ymd("2026-09-03T09:30"), Some((2026, 9, 3)));
    assert_eq!(parse_ymd("09:30"), None);
}

#[test]
fn the_time_half_is_read_wherever_it_sits() {
    assert_eq!(parse_hm("2026-09-03 09:30"), Some((9, 30)));
    assert_eq!(parse_hm("2026-09-03T23:59"), Some((23, 59)));
    assert_eq!(parse_hm("09:30"), Some((9, 30)));
    assert_eq!(parse_hm("2026-09-03"), None);
    // Out of range is not a time.
    assert_eq!(parse_hm("24:00"), None);
    assert_eq!(parse_hm("12:60"), None);
}

#[test]
fn format_says_which_halves_the_picker_edits() {
    let d = DtParts { date: true, time: false };
    let t = DtParts { date: false, time: true };
    let both = DtParts { date: true, time: true };

    assert_eq!(dt_parts(&picker("Short", "", "")), d);
    assert_eq!(dt_parts(&picker("Long", "", "")), d);
    assert_eq!(dt_parts(&picker("Time", "", "")), t);
    // Custom is decided by its own letters — the ones its property hint shows.
    assert_eq!(dt_parts(&picker("Custom", "dd/MM/yyyy HH:mm", "")), both);
    assert_eq!(dt_parts(&picker("Custom", "dd/MM/yyyy", "")), d);
    assert_eq!(dt_parts(&picker("Custom", "HH:mm", "")), t);
    // `M` is the month and `m` the minute: the one place case matters.
    assert_eq!(dt_parts(&picker("Custom", "MM", "")), d);
    assert_eq!(dt_parts(&picker("Custom", "mm", "")), t);
    // A pattern naming neither is not a picker that edits nothing.
    assert_eq!(dt_parts(&picker("Custom", "???", "")), d);
    // An unrecognised preset falls back to a date, as it always did.
    assert_eq!(dt_parts(&picker("Nonsense", "", "")), d);
}

#[test]
fn the_written_value_carries_only_the_halves_the_format_asks_for() {
    let date = Some((2026, 9, 3));
    let time = Some((9, 30));
    let d = DtParts { date: true, time: false };
    let t = DtParts { date: false, time: true };
    let both = DtParts { date: true, time: true };

    assert_eq!(format_dt_value(date, time, d), "2026-09-03");
    assert_eq!(format_dt_value(date, time, t), "09:30");
    assert_eq!(format_dt_value(date, time, both), "2026-09-03 09:30");
    // Zero-padded on both halves, so the value sorts and parses the same way
    // whatever month or hour it holds.
    assert_eq!(
        format_dt_value(Some((2026, 1, 5)), Some((0, 7)), both),
        "2026-01-05 00:07"
    );
    // A half the format asks for but the picker has no value for is simply
    // absent — never a placeholder written into the data.
    assert_eq!(format_dt_value(None, time, both), "09:30");
    assert_eq!(format_dt_value(None, None, both), "");
}

#[test]
fn the_field_shows_the_halves_its_format_asks_for() {
    // `Format` used to do nothing at all: the field printed `Value` verbatim,
    // so a Short picker holding a time showed it and a Time picker showed the
    // date.
    let both = DtParts { date: true, time: true };
    let d = DtParts { date: true, time: false };
    let t = DtParts { date: false, time: true };

    assert_eq!(display_dt("2026-09-03 09:30", d), "2026-09-03");
    assert_eq!(display_dt("2026-09-03 09:30", t), "09:30");
    assert_eq!(display_dt("2026-09-03 09:30", both), "2026-09-03 09:30");
    // A date-only value in a picker that also edits time shows the date alone
    // rather than inventing a midnight.
    assert_eq!(display_dt("2026-09-03", both), "2026-09-03");
    assert_eq!(display_dt("", d), "");
}

#[test]
fn a_value_the_control_cannot_parse_is_shown_as_it_stands() {
    // User data is never hidden: text that is neither a date nor a time is the
    // developer's, and blanking it would read as the control losing it.
    let d = DtParts { date: true, time: false };
    assert_eq!(display_dt("next Tuesday", d), "next Tuesday");
    assert_eq!(display_dt("  ", d), "");
}
