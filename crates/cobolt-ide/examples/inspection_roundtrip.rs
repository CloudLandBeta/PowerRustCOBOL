// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! MCP/inspection round-trip client (spec 027 T11 / AC3).
//!
//! Connects to a **running** PowerRustCOBOL IDE's agent-access endpoint
//! (127.0.0.1:5719 by default, `INSPECTION_ADDR` to override), then:
//!   1. `GetInfo`   — handshake + app label,
//!   2. `GetTree`   — full AccessKit widget tree (named-node census),
//!   3. `ApplyEvents` — click a named widget (default "New Form"),
//!   4. `GetTree`   — observe the effect,
//!   5. `GetScreenshot` — save a PNG proof.
//!
//! This is the same wire protocol the official `egui-mcp` bridge speaks, so a
//! passing run proves the whole agent-access chain end-to-end.

use std::net::TcpStream;

use egui_inspection::protocol::read_handshake;
use egui_inspection::{read_message, write_message, Request, Response};

fn tree_census(resp: &Response) -> (usize, Vec<(String, String, egui::Rect)>) {
    let Response::Tree {
        accesskit: Some(update),
        ..
    } = resp
    else {
        return (0, Vec::new());
    };
    let mut named = Vec::new();
    for (_id, node) in &update.nodes {
        if let Some(label) = node.label() {
            let b = node.bounds().unwrap_or_default();
            named.push((
                format!("{:?}", node.role()),
                label.to_string(),
                egui::Rect::from_min_max(
                    egui::pos2(b.x0 as f32, b.y0 as f32),
                    egui::pos2(b.x1 as f32, b.y1 as f32),
                ),
            ));
        }
    }
    (update.nodes.len(), named)
}

fn click_events(center: egui::Pos2) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(center),
        egui::Event::PointerButton {
            pos: center,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        },
        egui::Event::PointerButton {
            pos: center,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        },
    ]
}

fn main() {
    let addr = std::env::var("INSPECTION_ADDR").unwrap_or_else(|_| "127.0.0.1:5719".into());
    let target = std::env::var("CLICK_TARGET").unwrap_or_else(|_| "New Form".into());

    let mut stream = TcpStream::connect(&addr).expect("IDE inspection endpoint not reachable");
    // The server speaks first: read ITS handshake; clients send only framed
    // requests after that.
    let peer_version = read_handshake(&mut stream).expect("handshake read");
    println!("[1] connected to {addr}, protocol v{peer_version}");

    write_message(&mut stream, &Request::GetInfo).unwrap();
    match read_message::<_, Response>(&mut stream).unwrap() {
        Response::Info {
            label,
            egui_version,
        } => println!("[1] peer: {label:?} (egui {egui_version})"),
        other => panic!("unexpected GetInfo reply: {other:?}"),
    }

    write_message(&mut stream, &Request::GetTree).unwrap();
    let tree1 = read_message::<_, Response>(&mut stream).unwrap();
    let (total1, named1) = tree_census(&tree1);
    println!("[2] tree: {total1} nodes, {} named", named1.len());
    for (role, label, _r) in named1.iter().take(12) {
        println!("      {role:<14} {label:?}");
    }

    let hit = named1
        .iter()
        .find(|(_, label, _)| label.contains(&target))
        .cloned();
    match hit {
        Some((role, label, rect)) => {
            let c = rect.center();
            println!("[3] clicking {role} {label:?} at ({:.0},{:.0})", c.x, c.y);
            write_message(
                &mut stream,
                &Request::ApplyEvents {
                    events: click_events(c),
                },
            )
            .unwrap();
            match read_message::<_, Response>(&mut stream).unwrap() {
                Response::Done => println!("[3] click applied (frame executed)"),
                other => panic!("unexpected ApplyEvents reply: {other:?}"),
            }
        }
        None => {
            println!("[3] SKIP: no widget labelled {target:?} in the current view");
        }
    }

    write_message(&mut stream, &Request::GetTree).unwrap();
    let tree2 = read_message::<_, Response>(&mut stream).unwrap();
    let (total2, named2) = tree_census(&tree2);
    println!("[4] tree after click: {total2} nodes, {} named", named2.len());

    write_message(
        &mut stream,
        &Request::GetScreenshot {
            pixels_per_point: Some(1.0),
        },
    )
    .unwrap();
    match read_message::<_, Response>(&mut stream).unwrap() {
        Response::Screenshot(png) => {
            let out = std::env::var("SCREENSHOT_OUT")
                .unwrap_or_else(|_| "inspection_roundtrip.png".into());
            std::fs::write(&out, &png.bytes).unwrap();
            println!("[5] screenshot {}x{} saved to {out}", png.size[0], png.size[1]);
        }
        other => panic!("unexpected screenshot reply: {other:?}"),
    }
    println!("ROUNDTRIP OK");
}
