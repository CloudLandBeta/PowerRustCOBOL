// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 109 AC4: fixed expected outputs for pt-BR, en-US and fr-FR, explicit
//! BRL, signs, zero, rounding boundaries, compact thresholds and the percent
//! convention. The expected texts are written out character by character.

use std::time::Instant;

use cobolt_format::{format_decimal, FormatError};

const NB: &str = "\u{a0}";
const NN: &str = "\u{202f}";

fn f(m: i128, s: u8, fmt: &str, loc: &str, cur: Option<&str>) -> String {
    format_decimal(m, s, fmt, loc, cur).expect("formats")
}

#[test]
fn the_formats_print_the_same_characters_on_every_host() {
    let t0 = Instant::now();
    let brl = Some("BRL");
    let cases: Vec<(&str, String, String)> = vec![
        // number: own scale, grouped
        ("pt-BR number", f(123456789, 2, "number", "pt-BR", None), "1.234.567,89".into()),
        ("en-US number", f(123456789, 2, "number", "en-US", None), "1,234,567.89".into()),
        ("fr-FR number", f(123456789, 2, "number", "fr-FR", None), format!("1{NN}234{NN}567,89")),
        ("count, no decimals", f(1234, 0, "number", "pt-BR", None), "1.234".into()),
        ("zero", f(0, 0, "number", "en-US", None), "0".into()),
        ("negative", f(-150, 2, "number", "pt-BR", None), "-1,50".into()),
        // currency
        ("pt-BR BRL", f(123456, 2, "currency", "pt-BR", brl), format!("R${NB}1.234,56")),
        ("en-US BRL", f(123456, 2, "currency", "en-US", brl), "R$1,234.56".into()),
        ("fr-FR EUR", f(123456, 2, "currency", "fr-FR", Some("eur")), format!("1{NN}234,56{NB}€")),
        ("JPY has no decimals", f(1234, 0, "currency", "en-US", Some("JPY")), "¥1,234".into()),
        ("negative currency", f(-5, 0, "currency", "en-US", Some("USD")), "-$5.00".into()),
        ("scale is rounded half away: 0.005", f(5, 3, "currency", "en-US", Some("USD")), "$0.01".into()),
        ("scale is rounded half away: -0.005", f(-5, 3, "currency", "en-US", Some("USD")), "-$0.01".into()),
        ("below the boundary: 0.0049", f(49, 4, "currency", "en-US", Some("USD")), "$0.00".into()),
        // compact thresholds
        ("999 is not compact", f(999, 0, "compact", "en-US", None), "999".into()),
        ("1000", f(1000, 0, "compact", "en-US", None), "1K".into()),
        ("1250", f(1250, 0, "compact", "en-US", None), "1.3K".into()),
        ("999950 promotes to M", f(999950, 0, "compact", "en-US", None), "1M".into()),
        ("pt-BR milhões", f(1_200_000, 0, "compact", "pt-BR", None), format!("1,2{NB}mi")),
        ("pt-BR mil", f(-4500, 0, "compact", "pt-BR", None), format!("-4,5{NB}mil")),
        ("fr-FR milliards", f(3_000_000_000, 0, "compact", "fr-FR", None), format!("3{NB}Md")),
        ("trillions", f(2_500_000_000_000, 0, "compact", "en-US", None), "2.5T".into()),
        ("compact with scale", f(150_000, 2, "compact", "en-US", None), "1.5K".into()),
        // currency-short
        ("pt-BR short", f(1_200_000, 0, "currency-short", "pt-BR", brl), format!("R${NB}1,2{NB}mi")),
        ("en-US short", f(1_200_000, 0, "currency-short", "en-US", Some("USD")), "$1.2M".into()),
        // percent: the value is a ratio
        ("percent 0.425", f(425, 3, "percent", "pt-BR", None), "42,5%".into()),
        ("percent 1", f(1, 0, "percent", "en-US", None), "100%".into()),
        ("percent fr", f(425, 3, "percent", "fr-FR", None), format!("42,5{NN}%")),
        ("percent of zero", f(0, 2, "percent", "pt-BR", None), "0%".into()),
        // names and tags are case-insensitive, a locale's blanks are not part of it
        ("format name case", f(1234, 0, "NUMBER", "pt-BR", None), "1.234".into()),
        ("Currency-Short case", f(1500, 0, "Currency-Short", "en-US", Some("usd")), "$1.5K".into()),
        ("locale case and blanks", f(1234, 0, "number", "PT-br      ", None), "1.234".into()),
    ];
    let mut failed = 0;
    for (name, got, want) in &cases {
        if got != want {
            failed += 1;
            eprintln!("FAIL {name}: got [{got}] want [{want}]");
        }
    }
    // What is refused (R8).
    let refused: Vec<(&str, Result<String, FormatError>)> = vec![
        ("unknown format", format_decimal(1, 0, "scientific", "pt-BR", None)),
        ("unknown locale", format_decimal(1, 0, "number", "de-DE", None)),
        ("currency without a code", format_decimal(1, 0, "currency", "pt-BR", None)),
        ("currency-short without a code", format_decimal(1, 0, "currency-short", "pt-BR", None)),
        ("invalid currency", format_decimal(1, 0, "currency", "pt-BR", Some("REAL"))),
    ];
    for (name, got) in &refused {
        if got.is_ok() {
            failed += 1;
            eprintln!("FAIL {name}: should have been refused");
        }
    }
    let total = cases.len() + refused.len();
    println!("================ cobolt-format formats ================");
    println!("locales: pt-BR, en-US, fr-FR; formats: number, currency, compact, currency-short, percent");
    println!("cases: signs, zero, rounding boundaries (0.0049, 0.005), compact thresholds (999, 1000, 999950), percent ratio, case-insensitive names, 5 refusals");
    println!("run: {total} cases in {} us", t0.elapsed().as_micros());
    println!("tally: {} passed, {failed} failed", total - failed);
    assert_eq!(failed, 0);
}
