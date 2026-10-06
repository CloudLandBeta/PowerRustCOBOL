// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 078 T-A10: the AWS controls in the form model — non-visual, their
//! design-time properties and events, their runtime-only answers, and a
//! `.cfrm` round trip.

use cobolt_forms::{load_form_from_str, model::runtime_property_names_for, Control, ControlType, Form, PropValue};

#[test]
fn aws_controls_are_non_visual_and_round_trip_a_cfrm() {
    let mut form = Form::new("AWS-FORM", "AWS", 400, 300);
    let mut lambda = Control::new("AwsLambda-1", ControlType::AwsLambda, 20, 20);
    lambda.set_prop("Connection", PropValue::String("prod".into()));
    lambda.set_prop("FunctionName", PropValue::String("order-sync".into()));
    lambda.set_prop("AllowWrite", PropValue::Bool(true));
    let mcp = Control::new("AwsMcp-1", ControlType::AwsMcp, 90, 20);
    form.controls = vec![lambda, mcp];

    for ct in [ControlType::AwsLambda, ControlType::AwsMcp] {
        assert!(ct.is_non_visual(), "{}", ct.as_str());
        assert_eq!(ct.default_size(), (56, 56));
        assert_eq!(ControlType::from_str(ct.as_str()), ct);
        assert!(ControlType::ALL.contains(&ct));
        let events = ct.supported_events();
        for e in ["onComplete", "onError", "onTimeout", "onCancelled"] {
            assert!(events.contains(&e), "{} lacks {e}", ct.as_str());
        }
        assert!(events.contains(&ct.primary_event()));
        let c = Control::new("x", ct.clone(), 0, 0);
        for p in ["Connection", "Mode", "Busy", "TimeoutMs", "StartTimeoutMs", "AllowWrite", "Verbose"] {
            assert!(c.get_prop(p).is_some(), "{} does not seed {p}", ct.as_str());
        }
        assert_eq!(c.get_prop("AllowWrite").map(|v| v.as_bool()), Some(false), "read-only by default (R25)");
        // What an operation returns is runtime-only, never a design-time seed.
        for p in ["ResponseBody", "ResultJson", "RowCount", "LastError"] {
            assert!(runtime_property_names_for(ct.as_str()).contains(&p));
            assert!(c.get_prop(p).is_none(), "{} seeds the runtime answer {p}", ct.as_str());
        }
    }
    assert_eq!(ControlType::AwsLambda.primary_event(), "onInvoked");
    assert!(runtime_property_names_for("AwsLambda").contains(&"FunctionError"));

    let xml = cobolt_forms::xml::form_to_string(&form).unwrap();
    let back = load_form_from_str(&xml).unwrap();
    assert_eq!(back.controls.len(), 2);
    assert_eq!(back.controls[0].control_type, ControlType::AwsLambda);
    assert_eq!(back.controls[0].get_prop("FunctionName").map(|v| v.to_string()).as_deref(), Some("order-sync"));
    assert_eq!(back.controls[0].get_prop("Connection").map(|v| v.to_string()).as_deref(), Some("prod"));
    assert_eq!(back.controls[0].get_prop("AllowWrite").map(|v| v.as_bool()), Some(true));
    assert_eq!(back.controls[1].control_type, ControlType::AwsMcp);
    // R11: no credential field is ever written.
    for word in ["AccessKey", "SecretKey", "SessionToken", "Password"] {
        assert!(!xml.contains(word), "the .cfrm holds {word}");
    }
    println!("AWS model: 2 types non-visual, 7 seeds each, 4+ runtime answers, .cfrm round trip, no credential field ({} bytes)", xml.len());
}

/// Amendment A5: each AWS control has a hand-drawn AWS-style SVG that
/// rasterises — a service-coloured tile with a white glyph. Writes PNG
/// previews to `PRC_ICON_PREVIEW_DIR` when that is set.
#[cfg(feature = "render")]
#[test]
fn aws_icons_render_as_service_tiles() {
    use cobolt_forms::paint::{aws_icon_svg, rasterize_svg_square};
    let expect = [(ControlType::AwsLambda, (0xE0u8, 0x70u8, 0x10u8)), (ControlType::AwsMcp, (0xD8, 0x2A, 0x6C))];
    for (ct, (r, g, b)) in expect {
        let svg = aws_icon_svg(&ct).expect("an AWS control has an icon");
        assert!(svg.contains("Not an AWS file"), "{}: the file says it is hand-drawn", ct.as_str());
        let img = rasterize_svg_square(svg, 128).expect("it rasterises");
        let at = |x: usize, y: usize| img.pixels[y * 128 + x];
        // The tile's middle band, away from the glyph: the service colour.
        let tile = at(64, 6);
        let near = |a: u8, b: u8| (a as i32 - b as i32).abs() < 48;
        assert!(near(tile.r(), r) && near(tile.g(), g) && near(tile.b(), b), "{}: tile colour {tile:?}", ct.as_str());
        let white = img.pixels.iter().filter(|p| p.r() > 240 && p.g() > 240 && p.b() > 240).count();
        assert!(white > 200, "{}: the white glyph is drawn ({white} px)", ct.as_str());
        assert_eq!(at(0, 0).a(), 0, "{}: the rounded corner is transparent", ct.as_str());
        if let Ok(dir) = std::env::var("PRC_ICON_PREVIEW_DIR") {
            let bytes: Vec<u8> = img.pixels.iter().flat_map(|p| p.to_srgba_unmultiplied()).collect();
            let path = std::path::Path::new(&dir).join(format!("{}.png", ct.as_str()));
            image::save_buffer(&path, &bytes, 128, 128, image::ExtendedColorType::Rgba8).unwrap();
        }
    }
    println!("A5 icons: AwsLambda and AwsMcp rasterise at 128 px as service-coloured tiles with white glyphs");
}
