// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Small value rules shared by the state, the methods and the renderer.
//!
//! They mirror what the interpreter and the desktop form host do, so a value means the
//! same thing on every surface: a boolean written as `1`, `true`, `yes` or `on` is on;
//! a number is printed without a trailing `.0`; a colour is `#RRGGBB` or `#RRGGBBAA`.

/// The interpreter's `truthy`: `1`, `true`, `yes`, `on` in any case.
pub fn truthy(s: &str) -> bool {
    let t = s.trim();
    t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes") || t.eq_ignore_ascii_case("on")
}

/// The host's `turned_on` (a *write* of a state property): anything but empty, `0`, `false`.
pub fn turned_on(s: &str) -> bool {
    !matches!(s.trim(), "" | "0" | "false" | "FALSE")
}

/// `"1"` or `"0"`.
pub fn b01(s: &str) -> String {
    if truthy(s) { "1" } else { "0" }.to_owned()
}

pub fn parse_f64(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

pub fn parse_i64(s: &str) -> i64 {
    s.trim().parse::<i64>().unwrap_or(0)
}

/// The interpreter's `format_step_number`: whole numbers without a fraction, others to six
/// decimals with trailing zeros trimmed.
pub fn fmt_number(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        let s = format!("{n:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

/// A 1-based COBOL index to a 0-based one (`None` for 0, negative or non-numeric).
pub fn index1(value: &str) -> Option<usize> {
    let n = value.trim().parse::<usize>().ok()?;
    n.checked_sub(1)
}

/// Parse `#RGB`, `#RRGGBB` or `#RRGGBBAA` (the `.cfrm` colour spelling) to `[r, g, b, a]`.
pub fn parse_color(s: &str) -> Option<[u8; 4]> {
    let t = s.trim().trim_start_matches('#');
    let hex = |i: usize| u8::from_str_radix(t.get(i..i + 2)?, 16).ok();
    match t.len() {
        3 => {
            let d = |i: usize| u8::from_str_radix(&t[i..i + 1], 16).ok().map(|v| v * 17);
            Some([d(0)?, d(1)?, d(2)?, 255])
        }
        6 => Some([hex(0)?, hex(2)?, hex(4)?, 255]),
        8 => Some([hex(0)?, hex(2)?, hex(4)?, hex(6)?]),
        _ => None,
    }
}

/// A colour as a CSS `rgb()` / `rgba()` value. An alpha of 255 prints as `rgb`, anything else
/// as `rgba` with the alpha to three decimals (so `B3` is `0.702`).
pub fn css_color(c: [u8; 4]) -> String {
    if c[3] == 255 {
        format!("rgb({} {} {})", c[0], c[1], c[2])
    } else {
        let a = (c[3] as f64 / 255.0 * 1000.0).round() / 1000.0;
        format!("rgb({} {} {} / {})", c[0], c[1], c[2], fmt_number(a))
    }
}

/// `css_color(parse_color(s))`, or `None` for an empty or unreadable colour.
pub fn css_color_of(s: &str) -> Option<String> {
    parse_color(s).map(css_color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truthy_and_turned_on_follow_the_two_host_rules() {
        for on in ["1", "true", "TRUE", "yes", "On"] {
            assert!(truthy(on), "{on}");
        }
        for off in ["", "0", "false", "no", "off", "2"] {
            assert!(!truthy(off), "{off}");
        }
        // A write of `2` turns a state property on (the host's rule), though `2` is not `truthy`.
        assert!(turned_on("2") && turned_on("yes") && !turned_on("0") && !turned_on("false") && !turned_on(""));
        assert_eq!(b01("TRUE"), "1");
        assert_eq!(b01("nope"), "0");
    }

    #[test]
    fn numbers_print_the_way_the_interpreter_prints_them() {
        assert_eq!(fmt_number(5.0), "5");
        assert_eq!(fmt_number(0.5), "0.5");
        assert_eq!(fmt_number(1.0 / 3.0), "0.333333");
        assert_eq!(fmt_number(-2.0), "-2");
        assert_eq!(index1("3"), Some(2));
        assert_eq!(index1("0"), None);
        assert_eq!(index1("x"), None);
    }

    #[test]
    fn colours_parse_in_the_three_spellings_and_print_as_css() {
        assert_eq!(parse_color("#36383EB3"), Some([0x36, 0x38, 0x3e, 0xb3]));
        assert_eq!(parse_color("#fff"), Some([255, 255, 255, 255]));
        assert_eq!(parse_color("#F0F0F0"), Some([240, 240, 240, 255]));
        assert_eq!(parse_color("#12"), None);
        assert_eq!(parse_color("red"), None);
        assert_eq!(css_color([240, 240, 240, 255]), "rgb(240 240 240)");
        assert_eq!(css_color([0x36, 0x38, 0x3e, 0xb3]), "rgb(54 56 62 / 0.702)");
        assert_eq!(css_color_of(""), None);
    }
}
