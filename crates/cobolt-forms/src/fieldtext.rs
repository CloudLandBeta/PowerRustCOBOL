// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **What a date field and a number field show** — the text of a DateTimePicker (`Format`, `CustomFormat`) and of a
//! NumericUpDown (`DecimalPlaces`, `ThousandsSeparator`) as the desktop's painter lays it out, with no egui in it, so that the
//! Web player shows the same text whatever the browser's own locale would have printed. `paint` re-exports these names.

use crate::model::Control;

pub const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Which halves of a date-time a DateTimePicker's `Format` asks for.
///
/// A picker with no time half shows no time strip and writes no time into
/// `Value`; one with no date half shows no calendar. Both false is impossible —
/// `dt_parts` falls back to the date, because a picker that can pick nothing is
/// never what a `CustomFormat` typo meant.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DtParts {
    pub date: bool,
    pub time: bool,
}

/// Read `Format` (and `CustomFormat`) and say which halves the control edits.
///
/// `Short`/`Long` are date only, `Time` is time only. `Custom` is decided by
/// the pattern itself: `y`/`M`/`d` ask for a date, `H`/`h`/`m` for a time — the
/// same letters the property's own hint shows (`dd/MM/yyyy HH:mm`). A pattern
/// naming neither falls back to a date.
pub fn dt_parts(ctrl: &Control) -> DtParts {
    let format = ctrl
        .get_prop("Format")
        .map(|v| v.as_str().to_owned())
        .unwrap_or_default();
    match format.trim() {
        f if f.eq_ignore_ascii_case("Time") => DtParts { date: false, time: true },
        f if f.eq_ignore_ascii_case("Custom") => {
            let pat = ctrl
                .get_prop("CustomFormat")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_default();
            // `M` is the month and `m` the minute — case matters, and it is the
            // one place in this control where it does.
            let date = pat.contains('y') || pat.contains('M') || pat.contains('d');
            let time = pat.contains('H') || pat.contains('h') || pat.contains('m');
            if date || time {
                DtParts { date, time }
            } else {
                DtParts { date: true, time: false }
            }
        }
        // Short, Long, anything unrecognised.
        _ => DtParts { date: true, time: false },
    }
}

/// The `HH:MM` in a value, wherever it sits in the string.
///
/// Separate from [`parse_ymd`] so each half of a value can be missing: a
/// `Format = Time` picker holds `09:30` and no date at all.
pub fn parse_hm(s: &str) -> Option<(u32, u32)> {
    let token = s
        .split(|c: char| c.is_whitespace() || c == 'T')
        .find(|t| t.contains(':'))?;
    let mut it = token.split(':');
    let h: u32 = it.next()?.trim().parse().ok()?;
    let m: u32 = it.next()?.trim().parse().ok()?;
    (h < 24 && m < 60).then_some((h, m))
}

/// What a DateTimePicker's field shows for `raw`, given the halves its `Format`
/// asks for.
///
/// `Format` used to do nothing whatever: the field printed `Value` verbatim,
/// so a `Short` picker holding `2026-09-03 09:30` showed the time and a `Time`
/// picker showed the date. The value is canonical ISO and stays that way — this
/// only decides what is put on screen.
///
/// A `Value` that carries neither half readably is shown as it stands rather
/// than blanked: it is the developer's text, and hiding it would look like the
/// control had lost it.
pub fn display_dt(raw: &str, parts: DtParts) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return String::new();
    }
    let date = parse_ymd(raw);
    let time = parse_hm(raw);
    if date.is_none() && time.is_none() {
        return raw.to_owned();
    }
    format_dt_value(date, time, parts)
}

/// How many fractional digits a NumericUpDown shows and writes: its
/// `DecimalPlaces` (0-6), and never fewer than its `Step` carries — a Step of
/// 0.25 on a 0-place field would otherwise step invisibly.
pub fn numeric_updown_places(ctrl: &Control) -> usize {
    let declared = ctrl.get_prop("DecimalPlaces").map(|v| v.as_i64()).unwrap_or(0).clamp(0, 6) as usize;
    let step = ctrl.get_prop("Step").map(|v| v.as_str().trim().to_owned()).unwrap_or_default();
    let step_places = step.split_once('.').map(|(_, f)| f.trim_end_matches('0').len()).unwrap_or(0);
    declared.max(step_places.min(6))
}

/// What a NumericUpDown's field shows: `Value` with `DecimalPlaces` digits,
/// and grouped in thousands when `ThousandsSeparator` is on. The value was
/// read as a whole number — so 2.5 showed as 0 — and neither property was
/// read at all (property audit, 2026-09-26).
pub fn numeric_updown_text(ctrl: &Control) -> String {
    let raw = ctrl.get_prop("Value").map(|v| v.as_str().trim().to_owned()).unwrap_or_default();
    let v = raw.parse::<f64>().unwrap_or(0.0);
    let text = format!("{v:.*}", numeric_updown_places(ctrl));
    if !ctrl.get_prop("ThousandsSeparator").map(|v| v.as_bool()).unwrap_or(false) {
        return text;
    }
    let (sign, digits) = text.strip_prefix('-').map_or(("", text.as_str()), |d| ("-", d));
    let (int, frac) = digits.split_once('.').map_or((digits, None), |(i, f)| (i, Some(f)));
    let mut grouped = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    match frac {
        Some(f) => format!("{sign}{grouped}.{f}"),
        None => format!("{sign}{grouped}"),
    }
}

/// A DateTimePicker's `MinimumDate` and `MaximumDate`, as dates. Empty or
/// unreadable is no bound.
pub fn dt_bounds(ctrl: &Control) -> (Option<(i32, u32, u32)>, Option<(i32, u32, u32)>) {
    let read = |k: &str| ctrl.get_prop(k).and_then(|v| parse_ymd(v.as_str()));
    (read("MinimumDate"), read("MaximumDate"))
}

/// Whether `d` lies within the bounds (inclusive). Dates compare as
/// (year, month, day) tuples.
pub fn date_in_bounds(
    d: (i32, u32, u32),
    bounds: (Option<(i32, u32, u32)>, Option<(i32, u32, u32)>),
) -> bool {
    bounds.0.is_none_or(|lo| d >= lo) && bounds.1.is_none_or(|hi| d <= hi)
}

/// `d` moved into the bounds, if it lies outside them.
pub fn clamp_date(
    d: (i32, u32, u32),
    bounds: (Option<(i32, u32, u32)>, Option<(i32, u32, u32)>),
) -> (i32, u32, u32) {
    let d = bounds.0.map_or(d, |lo| d.max(lo));
    bounds.1.map_or(d, |hi| d.min(hi))
}

/// `d` moved by `delta` days, across months and years.
pub fn step_date(d: (i32, u32, u32), delta: i32) -> (i32, u32, u32) {
    let (mut y, mut m, mut day) = (d.0, d.1.clamp(1, 12), d.2.max(1) as i32);
    day = day.min(days_in_month(y, m) as i32) + delta;
    while day < 1 {
        if m == 1 {
            m = 12;
            y -= 1;
        } else {
            m -= 1;
        }
        day += days_in_month(y, m) as i32;
    }
    while day > days_in_month(y, m) as i32 {
        day -= days_in_month(y, m) as i32;
        if m == 12 {
            m = 1;
            y += 1;
        } else {
            m += 1;
        }
    }
    (y, m, day as u32)
}

const WEEKDAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/// Lay a date and time out through a pattern: `yyyy` `yy` `MMMM` `MMM` `MM`
/// `M` `dddd` `ddd` `dd` `d` `HH` `H` `hh` `h` `mm` `m` `tt`, anything else
/// as written. A token whose half is missing is left out.
pub fn format_dt_pattern(pattern: &str, date: Option<(i32, u32, u32)>, time: Option<(u32, u32)>) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let mut n = 1;
        while i + n < chars.len() && chars[i + n] == c {
            n += 1;
        }
        let piece = match (c, date, time) {
            ('y', Some((y, _, _)), _) => Some(if n <= 2 { format!("{:02}", y.rem_euclid(100)) } else { format!("{y:04}") }),
            ('M', Some((_, m, _)), _) => Some(match n {
                1 => m.to_string(),
                2 => format!("{m:02}"),
                3 => MONTHS[(m.clamp(1, 12) - 1) as usize][..3].to_owned(),
                _ => MONTHS[(m.clamp(1, 12) - 1) as usize].to_owned(),
            }),
            ('d', Some((y, m, d)), _) => Some(match n {
                1 => d.to_string(),
                2 => format!("{d:02}"),
                3 => WEEKDAYS[day_of_week(y, m, d) as usize][..3].to_owned(),
                _ => WEEKDAYS[day_of_week(y, m, d) as usize].to_owned(),
            }),
            ('H', _, Some((h, _))) => Some(if n == 1 { h.to_string() } else { format!("{h:02}") }),
            ('h', _, Some((h, _))) => {
                let h12 = if h % 12 == 0 { 12 } else { h % 12 };
                Some(if n == 1 { h12.to_string() } else { format!("{h12:02}") })
            }
            ('m', _, Some((_, mi))) => Some(if n == 1 { mi.to_string() } else { format!("{mi:02}") }),
            ('t', _, Some((h, _))) => Some(if h < 12 { "AM" } else { "PM" }.to_owned()),
            ('y' | 'M' | 'd' | 'H' | 'h' | 'm' | 't', _, _) => Some(String::new()),
            _ => None,
        };
        match piece {
            Some(p) => out.push_str(&p),
            None => out.extend(std::iter::repeat_n(c, n)),
        }
        i += n;
    }
    out.trim().to_owned()
}

/// What a DateTimePicker's field shows: [`display_dt`] for `Short` and
/// `Time`, a long date for `Long` (`Thursday, 3 September 2026`), and the
/// value laid out through `CustomFormat` for `Custom`. `Format` used to decide
/// only which halves were editable — the field always printed ISO, so `Long`
/// read exactly like `Short` and a custom pattern changed nothing on screen
/// (property audit, 2026-09-26). `Value` itself stays ISO.
pub fn display_dt_for(ctrl: &Control, raw: &str) -> String {
    let parts = dt_parts(ctrl);
    let format = ctrl.get_prop("Format").map(|v| v.as_str().trim().to_owned()).unwrap_or_default();
    let pattern = ctrl.get_prop("CustomFormat").map(|v| v.as_str().trim().to_owned()).unwrap_or_default();
    let (date, time) = (parse_ymd(raw.trim()), parse_hm(raw.trim()));
    if raw.trim().is_empty() || (date.is_none() && time.is_none()) {
        return display_dt(raw, parts);
    }
    if format.eq_ignore_ascii_case("Custom") && !pattern.is_empty() {
        return format_dt_pattern(&pattern, date.filter(|_| parts.date), time.filter(|_| parts.time));
    }
    if format.eq_ignore_ascii_case("Long") && date.is_some() {
        return format_dt_pattern("dddd, d MMMM yyyy", date, None);
    }
    display_dt(raw, parts)
}

///
/// Canonical is ISO — `YYYY-MM-DD`, `HH:MM`, or the two space-separated — so a
/// COBOL handler reading `Value` gets one shape whatever the developer chose to
/// *display*. `Format` decides what is editable and what is shown; it never
/// changes how the value is stored.
pub fn format_dt_value(date: Option<(i32, u32, u32)>, time: Option<(u32, u32)>, parts: DtParts) -> String {
    let d = (parts.date)
        .then(|| date.map(|(y, m, d)| format!("{y:04}-{m:02}-{d:02}")))
        .flatten();
    let t = (parts.time)
        .then(|| time.map(|(h, mi)| format!("{h:02}:{mi:02}")))
        .flatten();
    match (d, t) {
        (Some(d), Some(t)) => format!("{d} {t}"),
        (Some(d), None) => d,
        (None, Some(t)) => t,
        (None, None) => String::new(),
    }
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Day of week for a date, 0 = Sunday (Sakamoto's algorithm).
pub fn day_of_week(y: i32, m: u32, d: u32) -> u32 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let yy = if m < 3 { y - 1 } else { y };
    let v = (yy + yy / 4 - yy / 100 + yy / 400 + t[(m.clamp(1, 12) - 1) as usize] + d as i32) % 7;
    ((v + 7) % 7) as u32
}

/// The `YYYY-MM-DD` in a value, ignoring any time that follows it.
///
/// The date half is taken off the front FIRST. Splitting the whole string on
/// `-` gave three parts for `2026-09-03` and three for `2026-09-03 09:30` too —
/// but the last of those was `03 09:30`, which does not parse, so a value
/// carrying a time read as no date at all and the calendar opened on its
/// hardcoded default month instead of the value's.
pub fn parse_ymd(s: &str) -> Option<(i32, u32, u32)> {
    let head = s
        .trim()
        .split(|c: char| c.is_whitespace() || c == 'T')
        .next()?;
    let p: Vec<&str> = head.split('-').collect();
    if p.len() == 3 {
        Some((p[0].parse().ok()?, p[1].parse().ok()?, p[2].parse().ok()?))
    } else {
        None
    }
}

// ── DataGrid cell text ────────────────────────────────────────────────────────
//
// What a grid cell shows for the raw value it holds: shared, like the rest of this module, so that the
// desktop painter and the web player draw the same text.

/// Short month names for DataGrid date cells and the DateTimePicker field.
pub const MONTH_ABBR: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Format a DataGrid cell value by its declared column type.
/// Returns `(display_text, right_aligned)`.
pub fn format_cell(raw: &str, ty: &str) -> (String, bool) {
    match ty {
        "number" | "num" | "int" | "integer" | "float" | "decimal" => {
            match raw.trim().parse::<f64>() {
                Ok(n) if n.fract() == 0.0 => (format!("{}", n as i64), true),
                Ok(n) => (format!("{n}"), true),
                Err(_) => (raw.to_owned(), true),
            }
        }
        "datetime" | "date" => match parse_ymd(raw.trim()) {
            Some((y, m, d)) => (
                format!(
                    "{:02} {} {}",
                    d,
                    MONTH_ABBR[(m.clamp(1, 12) - 1) as usize],
                    y
                ),
                false,
            ),
            None => (raw.to_owned(), false),
        },
        _ => (raw.to_owned(), false),
    }
}

/// Format a DataGrid cell using a COBOL display mask when one is available.
/// The mask support is intentionally conservative and covers the common grid
/// binding masks: `9(n)`, `S9(n)V99`, `PIC ...`, and text `X(n)`.
pub fn format_cell_with_cobol_mask(raw: &str, ty: &str, cobol_mask: &str) -> (String, bool) {
    let mask = normalize_cobol_mask(cobol_mask);
    if mask.is_empty() || mask == "-" || mask == "—" {
        return format_cell(raw, ty);
    }
    if mask.contains('X') || mask.contains('A') {
        return (raw.to_owned(), false);
    }
    // COBOL *edited* pictures — zero-suppression (`Z`), check protection (`*`),
    // digit-group (`,`) and displayed decimal (`.`) insertion, e.g.
    // `ZZZ,ZZZ,ZZ9.99` → `3,000.00`. Handled before the plain-numeric path so a
    // bound `S9(9)V99` value renders through the column's display mask.
    if is_edited_picture(&mask) {
        if let Some(edited) = format_edited_numeric(raw, &mask) {
            return (edited, true);
        }
    }
    if !mask.contains('9') {
        return format_cell(raw, ty);
    }
    let Some((integer_digits, decimal_digits, signed)) = parse_numeric_cobol_mask(&mask) else {
        return format_cell(raw, ty);
    };
    let Ok(value) = raw.trim().parse::<f64>() else {
        return (raw.to_owned(), true);
    };
    let negative = value.is_sign_negative();
    let abs_value = value.abs();
    let should_zero_pad = !signed;
    let formatted = if decimal_digits == 0 {
        let mut integer = format!("{:.0}", abs_value);
        if should_zero_pad && integer_digits > integer.len() {
            integer = format!("{}{}", "0".repeat(integer_digits - integer.len()), integer);
        }
        integer
    } else {
        let fixed = format!("{abs_value:.decimal_digits$}");
        let mut parts = fixed.splitn(2, '.');
        let mut integer = parts.next().unwrap_or_default().to_owned();
        let decimal = parts.next().unwrap_or_default();
        if should_zero_pad && integer_digits > integer.len() {
            integer = format!("{}{}", "0".repeat(integer_digits - integer.len()), integer);
        }
        format!("{integer}.{decimal}")
    };
    let sign = if negative {
        "-"
    } else if signed && mask.starts_with('+') {
        "+"
    } else {
        ""
    };
    (format!("{sign}{formatted}"), true)
}

/// A normalized picture is an *edited* numeric picture when it carries any
/// insertion/suppression symbol (`Z` `*` `,` `$` `B`) or a displayed decimal
/// point (`.`). Plain `9`/`S`/`V` pictures keep the simpler legacy formatting.
fn is_edited_picture(mask: &str) -> bool {
    mask.contains('Z')
        || mask.contains('*')
        || mask.contains(',')
        || mask.contains('$')
        || mask.contains('B')
        || mask.contains('.')
}

/// Expand `9(3)` / `Z(4)` / `*(2)` repetition groups into individual symbols so
/// the picture can be walked one position at a time.
fn expand_picture(mask: &str) -> String {
    let chars: Vec<char> = mask.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if chars.get(i + 1) == Some(&'(') {
            if let Some(end) = chars[i + 2..].iter().position(|c| *c == ')') {
                let end = i + 2 + end;
                let count: usize = chars[i + 2..end]
                    .iter()
                    .collect::<String>()
                    .parse()
                    .unwrap_or(1);
                for _ in 0..count {
                    out.push(ch);
                }
                i = end + 1;
                continue;
            }
        }
        out.push(ch);
        i += 1;
    }
    out
}

fn count_digit_positions(pic: &str) -> usize {
    pic.chars().filter(|c| matches!(c, '9' | 'Z' | '*')).count()
}

/// Format `raw` through a COBOL edited numeric picture (`ZZZ,ZZZ,ZZ9.99`,
/// `**,**9.99`, `9(6)`, …). Zero-suppresses leading zeros, honours grouping and
/// the displayed decimal point, and prefixes a sign for negatives / `+` floats.
/// Returns `None` when `raw` isn't numeric (caller falls back).
fn format_edited_numeric(raw: &str, mask: &str) -> Option<String> {
    let pic = expand_picture(mask);
    let value: f64 = raw.trim().parse().ok()?;
    let negative = value.is_sign_negative() && value != 0.0;
    let plus = pic.contains('+');
    let abs = value.abs();

    // Split integer / fraction at the displayed decimal point or implied `V`.
    let (int_pic, dec_pic) = if let Some(idx) = pic.find('.') {
        (pic[..idx].to_string(), pic[idx + 1..].to_string())
    } else if let Some(idx) = pic.find('V') {
        (pic[..idx].to_string(), pic[idx + 1..].to_string())
    } else {
        (pic.clone(), String::new())
    };
    let dec_digits = count_digit_positions(&dec_pic);
    let int_positions = count_digit_positions(&int_pic);

    // Digit strings, scaled to the fraction width and padded to the picture.
    let scaled = format!("{abs:.dec_digits$}");
    let (int_str, dec_str) = scaled.split_once('.').unwrap_or((scaled.as_str(), ""));
    let mut int_digits = int_str.to_string();
    while int_digits.len() < int_positions {
        int_digits.insert(0, '0');
    }
    if int_digits.len() > int_positions {
        int_digits = int_digits[int_digits.len() - int_positions..].to_string();
    }
    let int_digits: Vec<char> = int_digits.chars().collect();

    let fill = if pic.contains('*') { '*' } else { ' ' };
    let mut out = String::new();
    let mut di = 0usize;
    let mut suppressing = true;
    for ch in int_pic.chars() {
        match ch {
            '9' => {
                out.push(int_digits[di]);
                di += 1;
                suppressing = false;
            }
            'Z' | '*' => {
                let d = int_digits[di];
                di += 1;
                if suppressing && d == '0' {
                    out.push(fill);
                } else {
                    out.push(d);
                    suppressing = false;
                }
            }
            ',' => out.push(if suppressing { fill } else { ',' }),
            'B' => out.push(' '),
            'S' | 'V' | '+' | '-' => {}
            other => out.push(other),
        }
    }

    if dec_digits > 0 {
        out.push('.');
        let mut dec_chars = dec_str.to_string();
        while dec_chars.len() < dec_digits {
            dec_chars.push('0');
        }
        out.push_str(&dec_chars[..dec_digits]);
    }

    // Leading fill is alignment padding for a fixed field; a grid cell reads
    // cleaner trimmed. Asterisk (check-protection) fill is meaningful — keep it.
    if fill == ' ' {
        out = out.trim_start().to_string();
    }
    if negative {
        out = format!("-{out}");
    } else if plus {
        out = format!("+{out}");
    }
    Some(out)
}

fn normalize_cobol_mask(mask: &str) -> String {
    let mut normalized = mask.trim().to_ascii_uppercase();
    if let Some(rest) = normalized.strip_prefix("PIC ") {
        normalized = rest.trim().to_owned();
    } else if let Some(rest) = normalized.strip_prefix("PICTURE ") {
        normalized = rest.trim().to_owned();
    }
    normalized.retain(|ch| !ch.is_whitespace());
    normalized
}

fn parse_numeric_cobol_mask(mask: &str) -> Option<(usize, usize, bool)> {
    let signed = mask.starts_with('S') || mask.starts_with('+') || mask.starts_with('-');
    let unsigned = mask.trim_start_matches(['S', '+', '-']);
    let (integer_mask, decimal_mask) = unsigned
        .split_once('V')
        .or_else(|| unsigned.split_once('.'))
        .map(|(left, right)| (left, right))
        .unwrap_or((unsigned, ""));
    let integer_digits = count_cobol_digit_positions(integer_mask)?;
    let decimal_digits = if decimal_mask.is_empty() {
        0
    } else {
        count_cobol_digit_positions(decimal_mask)?
    };
    Some((integer_digits, decimal_digits, signed))
}

fn count_cobol_digit_positions(mask: &str) -> Option<usize> {
    let chars: Vec<char> = mask.chars().collect();
    let mut index = 0;
    let mut count = 0;
    while index < chars.len() {
        match chars[index] {
            '9' | 'Z' | '*' => {
                if chars.get(index + 1) == Some(&'(') {
                    let mut end = index + 2;
                    while end < chars.len() && chars[end] != ')' {
                        end += 1;
                    }
                    if end >= chars.len() {
                        return None;
                    }
                    let repeat = chars[index + 2..end]
                        .iter()
                        .collect::<String>()
                        .parse::<usize>()
                        .ok()?;
                    count += repeat;
                    index = end + 1;
                } else {
                    count += 1;
                    index += 1;
                }
            }
            ',' | '.' | '-' | '+' | '$' | '/' => index += 1,
            _ => return None,
        }
    }
    Some(count)
}
