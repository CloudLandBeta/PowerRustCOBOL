// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `/.well-known/security.txt` (RFC 9116) from the developer's own parameters (WASM-013 Q4).
//!
//! One text for two places: the Web Build writes it into the bundle, and the backend's HTTPS surface answers
//! with it at the same path. Both read the values from a `[security]` table (the project manifest, and
//! `backend.toml`) and call [`SecurityTxt::render`], so the two cannot drift apart. The values are the
//! developer's: the product repository's own security policy is a different file.
//!
//! `contact` (a `mailto:` or `https:` URI) and `expires` (a date, `YYYY-MM-DD`, read as the end of that day in
//! UTC, or a full `YYYY-MM-DDTHH:MM:SSZ`) are required, because RFC 9116 requires them. `policy` (an `https:`
//! URI) and `preferred_languages` are optional. A value that spans lines is refused: it would write a
//! field the developer never wrote.

/// The `[security]` table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SecurityTxt {
    pub contact: String,
    pub policy: String,
    pub expires: String,
    pub preferred_languages: String,
}

/// The days from 1970-01-01 to a civil date (proleptic Gregorian).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn month_days(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    }
}

/// `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SSZ` as (text for the file, unix seconds).
fn parse_expires(text: &str) -> Option<(String, i64)> {
    let t = text.trim();
    let (date, time) = match t.split_once('T') {
        Some((d, rest)) => (d, Some(rest)),
        None => (t, None),
    };
    let mut it = date.split('-');
    let (y, m, d) = (it.next()?, it.next()?, it.next()?);
    if it.next().is_some() || y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    let (y, m, d): (i64, i64, i64) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    if !(1..=12).contains(&m) || d < 1 || d > month_days(y, m) {
        return None;
    }
    let (hh, mm, ss) = match time {
        None => (23, 59, 59),
        Some(rest) => {
            let body = rest.strip_suffix('Z')?;
            let mut p = body.split(':');
            let (h, mi, s) = (p.next()?, p.next()?, p.next()?);
            if p.next().is_some() || h.len() != 2 || mi.len() != 2 || s.len() != 2 {
                return None;
            }
            let (h, mi, s): (i64, i64, i64) = (h.parse().ok()?, mi.parse().ok()?, s.parse().ok()?);
            if h > 23 || mi > 59 || s > 59 {
                return None;
            }
            (h, mi, s)
        }
    };
    let shown = format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z");
    Some((shown, days_from_civil(y, m, d) * 86_400 + hh * 3600 + mm * 60 + ss))
}

/// The moment a date setting expires, in unix seconds: `YYYY-MM-DD` is read as the end of that day in UTC, or a full
/// `YYYY-MM-DDTHH:MM:SSZ`. `None` when it is neither (a month 13, a 30th of February, a missing part). The manifest's
/// `[mobile.pinning] expires` reads dates the same way (WASM-013 T21).
pub fn expiry_unix(text: &str) -> Option<i64> {
    parse_expires(text).map(|(_, at)| at)
}

impl SecurityTxt {
    /// Is any key set? A table with nothing in it is the same as no table.
    pub fn is_empty(&self) -> bool {
        self.contact.trim().is_empty() && self.policy.trim().is_empty() && self.expires.trim().is_empty() && self.preferred_languages.trim().is_empty()
    }

    /// The file, or every key that is wrong, each named (`[security] contact: …`). `now_unix` is the clock:
    /// an `expires` that is not after it is refused (RFC 9116 asks for a date that has not passed).
    pub fn render(&self, now_unix: i64) -> Result<String, Vec<String>> {
        let mut errs = Vec::new();
        for (key, v) in [("contact", &self.contact), ("policy", &self.policy), ("expires", &self.expires), ("preferred_languages", &self.preferred_languages)] {
            if v.chars().any(|c| c.is_control()) {
                errs.push(format!("[security] {key}: must be one line of text"));
            }
        }
        let contact = self.contact.trim();
        if contact.is_empty() {
            errs.push("[security] contact: required (a mailto: or https: address) once [security] is present".into());
        } else if !(contact.starts_with("mailto:") && contact.len() > "mailto:".len() || contact.starts_with("https://") && contact.len() > "https://".len()) {
            errs.push("[security] contact: must be a mailto: or https: address".into());
        }
        let policy = self.policy.trim();
        if !policy.is_empty() && !(policy.starts_with("https://") && policy.len() > "https://".len()) {
            errs.push("[security] policy: must be an https: address".into());
        }
        let mut expires = None;
        if self.expires.trim().is_empty() {
            errs.push("[security] expires: required (a date, YYYY-MM-DD) once [security] is present".into());
        } else {
            match parse_expires(&self.expires) {
                None => errs.push("[security] expires: must be a date, YYYY-MM-DD (or YYYY-MM-DDTHH:MM:SSZ)".into()),
                Some((shown, at)) if at <= now_unix => errs.push(format!("[security] expires: {shown} has passed; set a later date")),
                Some((shown, _)) => expires = Some(shown),
            }
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        let mut out = format!("Contact: {contact}\nExpires: {}\n", expires.expect("checked"));
        if !policy.is_empty() {
            out.push_str(&format!("Policy: {policy}\n"));
        }
        let langs = self.preferred_languages.trim();
        if !langs.is_empty() {
            out.push_str(&format!("Preferred-Languages: {langs}\n"));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-11-06T00:00:00Z
    const NOW: i64 = 1_793_923_200;

    fn full() -> SecurityTxt {
        SecurityTxt { contact: "mailto:security@example.com".into(), policy: "https://example.com/security".into(), expires: "2027-01-31".into(), preferred_languages: "en, pt".into() }
    }

    #[test]
    fn the_clock_arithmetic_agrees_with_known_dates() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(days_from_civil(2026, 11, 6) * 86_400, NOW);
    }

    #[test]
    fn a_complete_table_writes_the_four_fields_in_order() {
        assert_eq!(full().render(NOW).unwrap(), "Contact: mailto:security@example.com\nExpires: 2027-01-31T23:59:59Z\nPolicy: https://example.com/security\nPreferred-Languages: en, pt\n");
        let min = SecurityTxt { contact: "https://example.com/report".into(), expires: "2027-01-31T10:00:00Z".into(), ..Default::default() };
        assert_eq!(min.render(NOW).unwrap(), "Contact: https://example.com/report\nExpires: 2027-01-31T10:00:00Z\n");
    }

    #[test]
    fn a_missing_wrong_or_past_key_is_refused_and_named() {
        let e = SecurityTxt::default().render(NOW).unwrap_err();
        assert!(e.iter().any(|m| m.contains("contact")) && e.iter().any(|m| m.contains("expires")), "{e:?}");
        let e = SecurityTxt { contact: "security@example.com".into(), ..full() }.render(NOW).unwrap_err();
        assert!(e.len() == 1 && e[0].contains("contact"), "{e:?}");
        let e = SecurityTxt { expires: "2026-11-05".into(), ..full() }.render(NOW).unwrap_err();
        assert!(e.len() == 1 && e[0].contains("expires") && e[0].contains("passed"), "{e:?}");
        let e = SecurityTxt { expires: "2027-02-30".into(), ..full() }.render(NOW).unwrap_err();
        assert!(e[0].contains("expires"), "{e:?}");
        let e = SecurityTxt { policy: "http://example.com".into(), ..full() }.render(NOW).unwrap_err();
        assert!(e[0].contains("policy"), "{e:?}");
        // the end of the day named is still ahead of a clock that is on that day
        assert!(SecurityTxt { expires: "2026-11-06".into(), ..full() }.render(NOW).is_ok());
    }

    #[test]
    fn a_value_cannot_write_a_field_of_its_own() {
        let e = SecurityTxt { contact: "mailto:a@example.com\nPolicy: https://evil.example".into(), ..full() }.render(NOW).unwrap_err();
        assert!(e.iter().any(|m| m.contains("contact") && m.contains("one line")), "{e:?}");
        let e = SecurityTxt { preferred_languages: "en\r\nExpires: 2999-01-01".into(), ..full() }.render(NOW).unwrap_err();
        assert!(e.iter().any(|m| m.contains("preferred_languages")), "{e:?}");
    }
}
