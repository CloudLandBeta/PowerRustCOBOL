// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Built-in vector icon catalogue for menu items (spec 018 R29/R30).
//!
//! Every icon is a hand-drawn egui path/stroke — no external image files.
//! Icons are tinted with the caller's colour (typically the menu item's
//! foreground).

#[cfg(feature = "render")]
use egui::{Color32, Pos2, Rect, Stroke, Vec2};

/// Full catalogue of available icon names, grouped by category.
pub const MENU_ICON_NAMES: &[&str] = &[
    // Document (10)
    "doc-new", "doc-open", "doc-save", "doc-save-as", "doc-copy",
    "doc-blank", "doc-text", "doc-pdf", "doc-spreadsheet", "doc-stack",
    // Edit (12)
    "scissors", "clipboard-copy", "clipboard-paste", "pencil", "eraser",
    "pen", "brush", "type-text", "bold", "italic", "underline", "strikethrough",
    // Navigation (10)
    "arrow-left", "arrow-right", "arrow-up", "arrow-down",
    "chevron-left", "chevron-right", "chevron-up", "chevron-down",
    "home", "external-link",
    // Action (12)
    "plus", "minus", "check", "x-mark", "refresh", "sync",
    "download", "upload", "share", "export", "import", "link",
    // UI/View (10)
    "eye", "eye-off", "magnifier", "zoom-in", "zoom-out",
    "fullscreen", "collapse", "expand", "grid-view", "list-view",
    // Communication (10)
    "mail", "mail-open", "send", "inbox", "chat",
    "phone", "video", "bell", "bell-off", "at-sign",
    // Social (6)
    "heart", "star", "thumbs-up", "thumbs-down", "bookmark", "flag",
    // People/User (6)
    "user", "users", "user-plus", "user-minus", "user-check", "user-circle",
    // Media (8)
    "play", "pause", "stop", "skip-forward", "skip-back",
    "volume", "volume-off", "music",
    // Data (8)
    "database", "chart-bar", "chart-line", "chart-pie",
    "table", "filter", "sort-asc", "sort-desc",
    // System (10)
    "gear", "wrench", "shield", "lock", "unlock", "key",
    "terminal", "code", "bug", "cpu",
    // Status (8)
    "info-circle", "warning-triangle", "error-circle", "help-circle",
    "check-circle", "x-circle", "clock", "calendar",
    // Commerce (6)
    "cart", "credit-card", "wallet", "receipt", "tag", "percent",
    // File/Folder (6)
    "folder", "folder-open", "folder-plus", "archive", "trash", "printer",
];

#[cfg(feature = "render")]
pub fn draw_menu_icon(painter: &egui::Painter, rect: Rect, name: &str, color: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height()) * 0.5;
    let st = Stroke::new((s * 0.14).max(1.0), color);
    let sth = Stroke::new((s * 0.18).max(1.2), color);
    let ls = |p1: Pos2, p2: Pos2| painter.line_segment([p1, p2], st);
    let lsh = |p1: Pos2, p2: Pos2| painter.line_segment([p1, p2], sth);

    match name {
        // ── Document ────────────────────────────────────────────────────────
        "doc-new" => {
            let r = icon_doc_outline(painter, c, s, st);
            ls(Pos2::new(c.x, c.y + s*0.1), Pos2::new(c.x, c.y + s*0.5));
            ls(Pos2::new(c.x - s*0.2, c.y + s*0.3), Pos2::new(c.x + s*0.2, c.y + s*0.3));
        }
        "doc-open" => {
            icon_doc_outline(painter, c, s, st);
            ls(Pos2::new(c.x - s*0.3, c.y + s*0.5), Pos2::new(c.x + s*0.5, c.y + s*0.5));
            ls(Pos2::new(c.x + s*0.3, c.y + s*0.3), Pos2::new(c.x + s*0.5, c.y + s*0.5));
            ls(Pos2::new(c.x + s*0.3, c.y + s*0.7), Pos2::new(c.x + s*0.5, c.y + s*0.5));
        }
        "doc-save" => {
            let tl = Pos2::new(c.x - s*0.5, c.y - s*0.6);
            let br = Pos2::new(c.x + s*0.5, c.y + s*0.6);
            painter.rect_stroke(Rect::from_min_max(tl, br), s*0.1, st);
            ls(Pos2::new(c.x - s*0.2, c.y - s*0.6), Pos2::new(c.x - s*0.2, c.y - s*0.25));
            ls(Pos2::new(c.x + s*0.2, c.y - s*0.6), Pos2::new(c.x + s*0.2, c.y - s*0.25));
            ls(Pos2::new(c.x - s*0.2, c.y - s*0.25), Pos2::new(c.x + s*0.2, c.y - s*0.25));
            painter.rect_stroke(
                Rect::from_min_max(Pos2::new(c.x - s*0.3, c.y + s*0.1), Pos2::new(c.x + s*0.3, c.y + s*0.6)),
                s*0.05, st);
        }
        "doc-save-as" => {
            draw_menu_icon(painter, rect.shrink(s*0.15), "doc-save", color);
            painter.text(Pos2::new(rect.max.x - s*0.15, rect.max.y - s*0.1),
                egui::Align2::RIGHT_BOTTOM, "…", egui::FontId::proportional(s*0.6), color);
        }
        "doc-copy" => {
            let off = s * 0.15;
            let r1 = Rect::from_min_max(Pos2::new(c.x-s*0.35+off, c.y-s*0.55), Pos2::new(c.x+s*0.35+off, c.y+s*0.35));
            let r2 = Rect::from_min_max(Pos2::new(c.x-s*0.35-off, c.y-s*0.35), Pos2::new(c.x+s*0.35-off, c.y+s*0.55));
            painter.rect_stroke(r1, s*0.05, st);
            painter.rect_stroke(r2, s*0.05, st);
        }
        "doc-blank" => { icon_doc_outline(painter, c, s, st); }
        "doc-text" => {
            icon_doc_outline(painter, c, s, st);
            for i in 0..3 {
                let y = c.y - s*0.15 + i as f32 * s*0.22;
                ls(Pos2::new(c.x - s*0.25, y), Pos2::new(c.x + s*0.15, y));
            }
        }
        "doc-pdf" => {
            icon_doc_outline(painter, c, s, st);
            painter.text(c + Vec2::new(0.0, s*0.1), egui::Align2::CENTER_CENTER,
                "P", egui::FontId::proportional(s*0.6), color);
        }
        "doc-spreadsheet" => {
            icon_doc_outline(painter, c, s, st);
            for i in 0..2 { for j in 0..2 {
                let x = c.x - s*0.18 + j as f32 * s*0.24;
                let y = c.y - s*0.05 + i as f32 * s*0.22;
                painter.rect_stroke(Rect::from_min_size(Pos2::new(x, y), Vec2::splat(s*0.18)), 0.0, st);
            }}
        }
        "doc-stack" => {
            for i in 0..3 {
                let off = i as f32 * s*0.12;
                let r = Rect::from_min_max(
                    Pos2::new(c.x - s*0.35 + off, c.y - s*0.5 + off),
                    Pos2::new(c.x + s*0.35 + off, c.y + s*0.4 + off));
                painter.rect_stroke(r, s*0.05, st);
            }
        }

        // ── Edit ────────────────────────────────────────────────────────────
        "scissors" => {
            painter.circle_stroke(Pos2::new(c.x - s*0.3, c.y + s*0.35), s*0.2, st);
            painter.circle_stroke(Pos2::new(c.x + s*0.3, c.y + s*0.35), s*0.2, st);
            ls(Pos2::new(c.x - s*0.15, c.y + s*0.2), Pos2::new(c.x + s*0.3, c.y - s*0.5));
            ls(Pos2::new(c.x + s*0.15, c.y + s*0.2), Pos2::new(c.x - s*0.3, c.y - s*0.5));
        }
        "clipboard-copy" => {
            painter.rect_stroke(Rect::from_center_size(c, Vec2::new(s*0.8, s)), s*0.08, st);
            ls(Pos2::new(c.x - s*0.15, c.y - s*0.5), Pos2::new(c.x - s*0.15, c.y - s*0.65));
            ls(Pos2::new(c.x + s*0.15, c.y - s*0.5), Pos2::new(c.x + s*0.15, c.y - s*0.65));
            ls(Pos2::new(c.x - s*0.15, c.y - s*0.65), Pos2::new(c.x + s*0.15, c.y - s*0.65));
        }
        "clipboard-paste" => {
            draw_menu_icon(painter, rect, "clipboard-copy", color);
            ls(Pos2::new(c.x, c.y - s*0.1), Pos2::new(c.x, c.y + s*0.3));
            ls(Pos2::new(c.x - s*0.12, c.y + s*0.15), Pos2::new(c.x, c.y + s*0.3));
            ls(Pos2::new(c.x + s*0.12, c.y + s*0.15), Pos2::new(c.x, c.y + s*0.3));
        }
        "pencil" => {
            let tip = Pos2::new(c.x - s*0.4, c.y + s*0.4);
            let top = Pos2::new(c.x + s*0.35, c.y - s*0.45);
            lsh(tip, top);
            ls(tip, Pos2::new(c.x - s*0.5, c.y + s*0.55));
        }
        "eraser" => {
            let pts = [
                Pos2::new(c.x - s*0.4, c.y + s*0.2),
                Pos2::new(c.x - s*0.1, c.y - s*0.3),
                Pos2::new(c.x + s*0.4, c.y - s*0.1),
                Pos2::new(c.x + s*0.1, c.y + s*0.4),
            ];
            for i in 0..4 { ls(pts[i], pts[(i+1)%4]); }
            ls(Pos2::new(c.x - s*0.25, c.y - s*0.05), Pos2::new(c.x + s*0.25, c.y + s*0.15));
        }
        "pen" => {
            let tip = Pos2::new(c.x - s*0.45, c.y + s*0.45);
            let top = Pos2::new(c.x + s*0.3, c.y - s*0.4);
            lsh(tip, top);
            ls(Pos2::new(c.x + s*0.2, c.y - s*0.5), Pos2::new(c.x + s*0.4, c.y - s*0.3));
        }
        "brush" => {
            ls(Pos2::new(c.x - s*0.1, c.y + s*0.5), Pos2::new(c.x, c.y));
            painter.circle_filled(Pos2::new(c.x + s*0.15, c.y - s*0.25), s*0.25, color);
        }
        "type-text" => {
            painter.text(c, egui::Align2::CENTER_CENTER, "T", egui::FontId::proportional(s*1.2), color);
        }
        "bold" => {
            painter.text(c, egui::Align2::CENTER_CENTER, "B", egui::FontId::proportional(s*1.2), color);
        }
        "italic" => {
            painter.text(c, egui::Align2::CENTER_CENTER, "I", egui::FontId::proportional(s*1.2), color);
        }
        "underline" => {
            painter.text(Pos2::new(c.x, c.y - s*0.1), egui::Align2::CENTER_CENTER,
                "U", egui::FontId::proportional(s*1.0), color);
            ls(Pos2::new(c.x - s*0.3, c.y + s*0.45), Pos2::new(c.x + s*0.3, c.y + s*0.45));
        }
        "strikethrough" => {
            painter.text(c, egui::Align2::CENTER_CENTER, "S", egui::FontId::proportional(s*1.0), color);
            ls(Pos2::new(c.x - s*0.4, c.y), Pos2::new(c.x + s*0.4, c.y));
        }

        // ── Navigation ──────────────────────────────────────────────────────
        "arrow-left" => {
            ls(Pos2::new(c.x + s*0.4, c.y), Pos2::new(c.x - s*0.4, c.y));
            ls(Pos2::new(c.x - s*0.4, c.y), Pos2::new(c.x - s*0.1, c.y - s*0.3));
            ls(Pos2::new(c.x - s*0.4, c.y), Pos2::new(c.x - s*0.1, c.y + s*0.3));
        }
        "arrow-right" => {
            ls(Pos2::new(c.x - s*0.4, c.y), Pos2::new(c.x + s*0.4, c.y));
            ls(Pos2::new(c.x + s*0.4, c.y), Pos2::new(c.x + s*0.1, c.y - s*0.3));
            ls(Pos2::new(c.x + s*0.4, c.y), Pos2::new(c.x + s*0.1, c.y + s*0.3));
        }
        "arrow-up" => {
            ls(Pos2::new(c.x, c.y + s*0.4), Pos2::new(c.x, c.y - s*0.4));
            ls(Pos2::new(c.x, c.y - s*0.4), Pos2::new(c.x - s*0.3, c.y - s*0.1));
            ls(Pos2::new(c.x, c.y - s*0.4), Pos2::new(c.x + s*0.3, c.y - s*0.1));
        }
        "arrow-down" => {
            ls(Pos2::new(c.x, c.y - s*0.4), Pos2::new(c.x, c.y + s*0.4));
            ls(Pos2::new(c.x, c.y + s*0.4), Pos2::new(c.x - s*0.3, c.y + s*0.1));
            ls(Pos2::new(c.x, c.y + s*0.4), Pos2::new(c.x + s*0.3, c.y + s*0.1));
        }
        "chevron-left" => {
            ls(Pos2::new(c.x + s*0.2, c.y - s*0.4), Pos2::new(c.x - s*0.2, c.y));
            ls(Pos2::new(c.x - s*0.2, c.y), Pos2::new(c.x + s*0.2, c.y + s*0.4));
        }
        "chevron-right" => {
            ls(Pos2::new(c.x - s*0.2, c.y - s*0.4), Pos2::new(c.x + s*0.2, c.y));
            ls(Pos2::new(c.x + s*0.2, c.y), Pos2::new(c.x - s*0.2, c.y + s*0.4));
        }
        "chevron-up" => {
            ls(Pos2::new(c.x - s*0.4, c.y + s*0.2), Pos2::new(c.x, c.y - s*0.2));
            ls(Pos2::new(c.x, c.y - s*0.2), Pos2::new(c.x + s*0.4, c.y + s*0.2));
        }
        "chevron-down" => {
            ls(Pos2::new(c.x - s*0.4, c.y - s*0.2), Pos2::new(c.x, c.y + s*0.2));
            ls(Pos2::new(c.x, c.y + s*0.2), Pos2::new(c.x + s*0.4, c.y - s*0.2));
        }
        "home" => {
            ls(Pos2::new(c.x - s*0.45, c.y), Pos2::new(c.x, c.y - s*0.45));
            ls(Pos2::new(c.x, c.y - s*0.45), Pos2::new(c.x + s*0.45, c.y));
            painter.rect_stroke(Rect::from_min_max(
                Pos2::new(c.x - s*0.3, c.y), Pos2::new(c.x + s*0.3, c.y + s*0.4)), 0.0, st);
            ls(Pos2::new(c.x, c.y + s*0.4), Pos2::new(c.x, c.y + s*0.1));
        }
        "external-link" => {
            painter.rect_stroke(Rect::from_min_max(
                Pos2::new(c.x - s*0.4, c.y - s*0.2), Pos2::new(c.x + s*0.2, c.y + s*0.4)), s*0.05, st);
            ls(Pos2::new(c.x + s*0.1, c.y - s*0.4), Pos2::new(c.x + s*0.45, c.y - s*0.4));
            ls(Pos2::new(c.x + s*0.45, c.y - s*0.4), Pos2::new(c.x + s*0.45, c.y - s*0.05));
            ls(Pos2::new(c.x + s*0.45, c.y - s*0.4), Pos2::new(c.x, c.y + s*0.05));
        }

        // ── Action ──────────────────────────────────────────────────────────
        "plus" => {
            ls(Pos2::new(c.x, c.y - s*0.4), Pos2::new(c.x, c.y + s*0.4));
            ls(Pos2::new(c.x - s*0.4, c.y), Pos2::new(c.x + s*0.4, c.y));
        }
        "minus" => {
            ls(Pos2::new(c.x - s*0.4, c.y), Pos2::new(c.x + s*0.4, c.y));
        }
        "check" => {
            ls(Pos2::new(c.x - s*0.35, c.y), Pos2::new(c.x - s*0.1, c.y + s*0.3));
            ls(Pos2::new(c.x - s*0.1, c.y + s*0.3), Pos2::new(c.x + s*0.35, c.y - s*0.3));
        }
        "x-mark" => {
            ls(Pos2::new(c.x - s*0.3, c.y - s*0.3), Pos2::new(c.x + s*0.3, c.y + s*0.3));
            ls(Pos2::new(c.x + s*0.3, c.y - s*0.3), Pos2::new(c.x - s*0.3, c.y + s*0.3));
        }
        "refresh" => {
            painter.circle_stroke(c, s*0.35, st);
            ls(Pos2::new(c.x + s*0.35, c.y - s*0.15), Pos2::new(c.x + s*0.35, c.y + s*0.15));
            ls(Pos2::new(c.x + s*0.35, c.y - s*0.15), Pos2::new(c.x + s*0.55, c.y - s*0.15));
        }
        "sync" => {
            ls(Pos2::new(c.x - s*0.4, c.y - s*0.15), Pos2::new(c.x + s*0.4, c.y - s*0.15));
            ls(Pos2::new(c.x + s*0.2, c.y - s*0.35), Pos2::new(c.x + s*0.4, c.y - s*0.15));
            ls(Pos2::new(c.x + s*0.2, c.y + s*0.05), Pos2::new(c.x + s*0.4, c.y - s*0.15));
            ls(Pos2::new(c.x + s*0.4, c.y + s*0.15), Pos2::new(c.x - s*0.4, c.y + s*0.15));
            ls(Pos2::new(c.x - s*0.2, c.y - s*0.05), Pos2::new(c.x - s*0.4, c.y + s*0.15));
            ls(Pos2::new(c.x - s*0.2, c.y + s*0.35), Pos2::new(c.x - s*0.4, c.y + s*0.15));
        }
        "download" => {
            ls(Pos2::new(c.x, c.y - s*0.4), Pos2::new(c.x, c.y + s*0.2));
            ls(Pos2::new(c.x, c.y + s*0.2), Pos2::new(c.x - s*0.25, c.y - s*0.05));
            ls(Pos2::new(c.x, c.y + s*0.2), Pos2::new(c.x + s*0.25, c.y - s*0.05));
            ls(Pos2::new(c.x - s*0.4, c.y + s*0.4), Pos2::new(c.x + s*0.4, c.y + s*0.4));
        }
        "upload" => {
            ls(Pos2::new(c.x, c.y + s*0.2), Pos2::new(c.x, c.y - s*0.4));
            ls(Pos2::new(c.x, c.y - s*0.4), Pos2::new(c.x - s*0.25, c.y - s*0.15));
            ls(Pos2::new(c.x, c.y - s*0.4), Pos2::new(c.x + s*0.25, c.y - s*0.15));
            ls(Pos2::new(c.x - s*0.4, c.y + s*0.4), Pos2::new(c.x + s*0.4, c.y + s*0.4));
        }
        "share" => {
            painter.circle_stroke(Pos2::new(c.x + s*0.3, c.y - s*0.3), s*0.15, st);
            painter.circle_stroke(Pos2::new(c.x - s*0.3, c.y), s*0.15, st);
            painter.circle_stroke(Pos2::new(c.x + s*0.3, c.y + s*0.3), s*0.15, st);
            ls(Pos2::new(c.x - s*0.15, c.y - s*0.08), Pos2::new(c.x + s*0.15, c.y - s*0.22));
            ls(Pos2::new(c.x - s*0.15, c.y + s*0.08), Pos2::new(c.x + s*0.15, c.y + s*0.22));
        }
        "export" => {
            painter.rect_stroke(Rect::from_center_size(c + Vec2::new(0.0, s*0.1), Vec2::new(s*0.7, s*0.6)), s*0.05, st);
            ls(Pos2::new(c.x, c.y - s*0.5), Pos2::new(c.x, c.y + s*0.05));
            ls(Pos2::new(c.x, c.y - s*0.5), Pos2::new(c.x - s*0.2, c.y - s*0.3));
            ls(Pos2::new(c.x, c.y - s*0.5), Pos2::new(c.x + s*0.2, c.y - s*0.3));
        }
        "import" => {
            painter.rect_stroke(Rect::from_center_size(c + Vec2::new(0.0, s*0.1), Vec2::new(s*0.7, s*0.6)), s*0.05, st);
            ls(Pos2::new(c.x, c.y + s*0.05), Pos2::new(c.x, c.y - s*0.5));
            ls(Pos2::new(c.x, c.y + s*0.05), Pos2::new(c.x - s*0.2, c.y - s*0.15));
            ls(Pos2::new(c.x, c.y + s*0.05), Pos2::new(c.x + s*0.2, c.y - s*0.15));
        }
        "link" => {
            painter.circle_stroke(Pos2::new(c.x - s*0.2, c.y - s*0.15), s*0.2, st);
            painter.circle_stroke(Pos2::new(c.x + s*0.2, c.y + s*0.15), s*0.2, st);
            ls(Pos2::new(c.x - s*0.05, c.y - s*0.05), Pos2::new(c.x + s*0.05, c.y + s*0.05));
        }

        // ── UI/View ─────────────────────────────────────────────────────────
        "eye" => {
            let pts: Vec<Pos2> = (0..=16).map(|i| {
                let t = i as f32 / 16.0;
                let x = c.x + s * 0.5 * (t * 2.0 - 1.0);
                let y = c.y - s * 0.25 * (1.0 - (t * 2.0 - 1.0).powi(2));
                Pos2::new(x, y)
            }).collect();
            for w in pts.windows(2) { ls(w[0], w[1]); }
            let pts2: Vec<Pos2> = (0..=16).map(|i| {
                let t = i as f32 / 16.0;
                let x = c.x + s * 0.5 * (t * 2.0 - 1.0);
                let y = c.y + s * 0.25 * (1.0 - (t * 2.0 - 1.0).powi(2));
                Pos2::new(x, y)
            }).collect();
            for w in pts2.windows(2) { ls(w[0], w[1]); }
            painter.circle_stroke(c, s*0.12, st);
        }
        "eye-off" => {
            draw_menu_icon(painter, rect, "eye", color);
            lsh(Pos2::new(c.x - s*0.4, c.y + s*0.4), Pos2::new(c.x + s*0.4, c.y - s*0.4));
        }
        "magnifier" => {
            painter.circle_stroke(Pos2::new(c.x - s*0.08, c.y - s*0.08), s*0.3, st);
            lsh(Pos2::new(c.x + s*0.13, c.y + s*0.13), Pos2::new(c.x + s*0.45, c.y + s*0.45));
        }
        "zoom-in" => {
            draw_menu_icon(painter, rect, "magnifier", color);
            ls(Pos2::new(c.x - s*0.08, c.y - s*0.22), Pos2::new(c.x - s*0.08, c.y + s*0.06));
            ls(Pos2::new(c.x - s*0.22, c.y - s*0.08), Pos2::new(c.x + s*0.06, c.y - s*0.08));
        }
        "zoom-out" => {
            draw_menu_icon(painter, rect, "magnifier", color);
            ls(Pos2::new(c.x - s*0.22, c.y - s*0.08), Pos2::new(c.x + s*0.06, c.y - s*0.08));
        }
        "fullscreen" => {
            // top-left corner
            ls(Pos2::new(c.x - s*0.45, c.y - s*0.25), Pos2::new(c.x - s*0.45, c.y - s*0.45));
            ls(Pos2::new(c.x - s*0.45, c.y - s*0.45), Pos2::new(c.x - s*0.25, c.y - s*0.45));
            // top-right
            ls(Pos2::new(c.x + s*0.25, c.y - s*0.45), Pos2::new(c.x + s*0.45, c.y - s*0.45));
            ls(Pos2::new(c.x + s*0.45, c.y - s*0.45), Pos2::new(c.x + s*0.45, c.y - s*0.25));
            // bottom-left
            ls(Pos2::new(c.x - s*0.45, c.y + s*0.25), Pos2::new(c.x - s*0.45, c.y + s*0.45));
            ls(Pos2::new(c.x - s*0.45, c.y + s*0.45), Pos2::new(c.x - s*0.25, c.y + s*0.45));
            // bottom-right
            ls(Pos2::new(c.x + s*0.25, c.y + s*0.45), Pos2::new(c.x + s*0.45, c.y + s*0.45));
            ls(Pos2::new(c.x + s*0.45, c.y + s*0.45), Pos2::new(c.x + s*0.45, c.y + s*0.25));
        }
        "collapse" => {
            ls(Pos2::new(c.x - s*0.3, c.y - s*0.15), Pos2::new(c.x, c.y + s*0.15));
            ls(Pos2::new(c.x, c.y + s*0.15), Pos2::new(c.x + s*0.3, c.y - s*0.15));
        }
        "expand" => {
            ls(Pos2::new(c.x - s*0.3, c.y + s*0.15), Pos2::new(c.x, c.y - s*0.15));
            ls(Pos2::new(c.x, c.y - s*0.15), Pos2::new(c.x + s*0.3, c.y + s*0.15));
        }
        "grid-view" => {
            for i in 0..2 { for j in 0..2 {
                let x = c.x - s*0.35 + j as f32 * s*0.4;
                let y = c.y - s*0.35 + i as f32 * s*0.4;
                painter.rect_stroke(Rect::from_min_size(Pos2::new(x, y), Vec2::splat(s*0.3)), s*0.03, st);
            }}
        }
        "list-view" => {
            for i in 0..3 {
                let y = c.y - s*0.3 + i as f32 * s*0.3;
                painter.circle_filled(Pos2::new(c.x - s*0.35, y), s*0.06, color);
                ls(Pos2::new(c.x - s*0.2, y), Pos2::new(c.x + s*0.4, y));
            }
        }

        // ── Communication ───────────────────────────────────────────────────
        "mail" => {
            let r = Rect::from_center_size(c, Vec2::new(s*0.9, s*0.6));
            painter.rect_stroke(r, s*0.05, st);
            ls(r.left_top(), c + Vec2::new(0.0, -s*0.05));
            ls(c + Vec2::new(0.0, -s*0.05), r.right_top());
        }
        "mail-open" => {
            let r = Rect::from_min_max(Pos2::new(c.x - s*0.45, c.y - s*0.1), Pos2::new(c.x + s*0.45, c.y + s*0.4));
            painter.rect_stroke(r, s*0.05, st);
            ls(Pos2::new(c.x - s*0.45, c.y - s*0.1), Pos2::new(c.x, c.y - s*0.45));
            ls(Pos2::new(c.x, c.y - s*0.45), Pos2::new(c.x + s*0.45, c.y - s*0.1));
        }
        "send" => {
            ls(Pos2::new(c.x - s*0.4, c.y - s*0.35), Pos2::new(c.x + s*0.4, c.y));
            ls(Pos2::new(c.x + s*0.4, c.y), Pos2::new(c.x - s*0.4, c.y + s*0.35));
            ls(Pos2::new(c.x - s*0.4, c.y + s*0.35), Pos2::new(c.x - s*0.1, c.y));
            ls(Pos2::new(c.x - s*0.1, c.y), Pos2::new(c.x - s*0.4, c.y - s*0.35));
        }
        "inbox" => {
            let r = Rect::from_center_size(c + Vec2::new(0.0, s*0.1), Vec2::new(s*0.9, s*0.6));
            painter.rect_stroke(r, s*0.05, st);
            ls(Pos2::new(c.x - s*0.45, c.y), Pos2::new(c.x - s*0.15, c.y));
            ls(Pos2::new(c.x - s*0.15, c.y), Pos2::new(c.x - s*0.15, c.y + s*0.15));
            ls(Pos2::new(c.x - s*0.15, c.y + s*0.15), Pos2::new(c.x + s*0.15, c.y + s*0.15));
            ls(Pos2::new(c.x + s*0.15, c.y + s*0.15), Pos2::new(c.x + s*0.15, c.y));
            ls(Pos2::new(c.x + s*0.15, c.y), Pos2::new(c.x + s*0.45, c.y));
        }
        "chat" => {
            painter.rect_stroke(Rect::from_center_size(c + Vec2::new(0.0, -s*0.1), Vec2::new(s*0.8, s*0.55)), s*0.1, st);
            ls(Pos2::new(c.x - s*0.1, c.y + s*0.18), Pos2::new(c.x - s*0.25, c.y + s*0.45));
            ls(Pos2::new(c.x - s*0.25, c.y + s*0.45), Pos2::new(c.x + s*0.1, c.y + s*0.18));
        }
        "phone" => {
            painter.rect_stroke(Rect::from_center_size(c, Vec2::new(s*0.45, s*0.85)), s*0.1, st);
            painter.circle_filled(Pos2::new(c.x, c.y + s*0.3), s*0.06, color);
        }
        "video" => {
            painter.rect_stroke(Rect::from_min_max(
                Pos2::new(c.x - s*0.45, c.y - s*0.25), Pos2::new(c.x + s*0.1, c.y + s*0.25)), s*0.05, st);
            ls(Pos2::new(c.x + s*0.1, c.y - s*0.15), Pos2::new(c.x + s*0.45, c.y - s*0.3));
            ls(Pos2::new(c.x + s*0.45, c.y - s*0.3), Pos2::new(c.x + s*0.45, c.y + s*0.3));
            ls(Pos2::new(c.x + s*0.45, c.y + s*0.3), Pos2::new(c.x + s*0.1, c.y + s*0.15));
        }
        "bell" => {
            painter.circle_stroke(Pos2::new(c.x, c.y - s*0.45), s*0.06, st);
            ls(Pos2::new(c.x - s*0.35, c.y + s*0.15), Pos2::new(c.x - s*0.3, c.y - s*0.15));
            ls(Pos2::new(c.x - s*0.3, c.y - s*0.15), Pos2::new(c.x + s*0.3, c.y - s*0.15));
            ls(Pos2::new(c.x + s*0.3, c.y - s*0.15), Pos2::new(c.x + s*0.35, c.y + s*0.15));
            ls(Pos2::new(c.x - s*0.35, c.y + s*0.15), Pos2::new(c.x + s*0.35, c.y + s*0.15));
            ls(Pos2::new(c.x - s*0.1, c.y + s*0.3), Pos2::new(c.x + s*0.1, c.y + s*0.3));
        }
        "bell-off" => {
            draw_menu_icon(painter, rect, "bell", color);
            lsh(Pos2::new(c.x - s*0.4, c.y + s*0.4), Pos2::new(c.x + s*0.4, c.y - s*0.4));
        }
        "at-sign" => {
            painter.circle_stroke(c, s*0.35, st);
            painter.circle_stroke(c, s*0.15, st);
            ls(Pos2::new(c.x + s*0.15, c.y), Pos2::new(c.x + s*0.35, c.y + s*0.15));
        }

        // ── Social ──────────────────────────────────────────────────────────
        "heart" => {
            let pts = [
                Pos2::new(c.x, c.y + s*0.4),
                Pos2::new(c.x - s*0.45, c.y - s*0.05),
                Pos2::new(c.x - s*0.35, c.y - s*0.35),
                Pos2::new(c.x, c.y - s*0.15),
                Pos2::new(c.x + s*0.35, c.y - s*0.35),
                Pos2::new(c.x + s*0.45, c.y - s*0.05),
            ];
            for i in 0..pts.len() { ls(pts[i], pts[(i+1) % pts.len()]); }
        }
        "star" => {
            let n = 5;
            let pts: Vec<Pos2> = (0..n*2).map(|i| {
                let angle = std::f32::consts::FRAC_PI_2 * -1.0 + i as f32 * std::f32::consts::PI / n as f32;
                let r = if i % 2 == 0 { s * 0.45 } else { s * 0.2 };
                Pos2::new(c.x + r * angle.cos(), c.y + r * angle.sin())
            }).collect();
            for i in 0..pts.len() { ls(pts[i], pts[(i+1) % pts.len()]); }
        }
        "thumbs-up" => {
            painter.rect_stroke(Rect::from_min_max(
                Pos2::new(c.x - s*0.45, c.y - s*0.1), Pos2::new(c.x - s*0.2, c.y + s*0.4)), s*0.03, st);
            ls(Pos2::new(c.x - s*0.2, c.y + s*0.1), Pos2::new(c.x + s*0.35, c.y + s*0.1));
            ls(Pos2::new(c.x + s*0.35, c.y + s*0.1), Pos2::new(c.x + s*0.35, c.y - s*0.15));
            ls(Pos2::new(c.x + s*0.35, c.y - s*0.15), Pos2::new(c.x + s*0.05, c.y - s*0.15));
            ls(Pos2::new(c.x + s*0.05, c.y - s*0.15), Pos2::new(c.x + s*0.15, c.y - s*0.4));
        }
        "thumbs-down" => {
            painter.rect_stroke(Rect::from_min_max(
                Pos2::new(c.x - s*0.45, c.y - s*0.4), Pos2::new(c.x - s*0.2, c.y + s*0.1)), s*0.03, st);
            ls(Pos2::new(c.x - s*0.2, c.y - s*0.1), Pos2::new(c.x + s*0.35, c.y - s*0.1));
            ls(Pos2::new(c.x + s*0.35, c.y - s*0.1), Pos2::new(c.x + s*0.35, c.y + s*0.15));
            ls(Pos2::new(c.x + s*0.35, c.y + s*0.15), Pos2::new(c.x + s*0.05, c.y + s*0.15));
            ls(Pos2::new(c.x + s*0.05, c.y + s*0.15), Pos2::new(c.x + s*0.15, c.y + s*0.4));
        }
        "bookmark" => {
            let pts = [
                Pos2::new(c.x - s*0.25, c.y - s*0.45),
                Pos2::new(c.x + s*0.25, c.y - s*0.45),
                Pos2::new(c.x + s*0.25, c.y + s*0.45),
                Pos2::new(c.x, c.y + s*0.15),
                Pos2::new(c.x - s*0.25, c.y + s*0.45),
            ];
            for i in 0..pts.len() { ls(pts[i], pts[(i+1) % pts.len()]); }
        }
        "flag" => {
            ls(Pos2::new(c.x - s*0.3, c.y - s*0.45), Pos2::new(c.x - s*0.3, c.y + s*0.45));
            ls(Pos2::new(c.x - s*0.3, c.y - s*0.45), Pos2::new(c.x + s*0.35, c.y - s*0.25));
            ls(Pos2::new(c.x + s*0.35, c.y - s*0.25), Pos2::new(c.x - s*0.3, c.y - s*0.05));
        }

        // ── People/User ─────────────────────────────────────────────────────
        "user" => {
            painter.circle_stroke(Pos2::new(c.x, c.y - s*0.2), s*0.22, st);
            let arc_c = Pos2::new(c.x, c.y + s*0.65);
            painter.circle_stroke(arc_c, s*0.45, st);
        }
        "users" => {
            draw_menu_icon(painter, Rect::from_center_size(c + Vec2::new(-s*0.15, 0.0),
                Vec2::splat(s*1.4)), "user", color);
            painter.circle_stroke(Pos2::new(c.x + s*0.3, c.y - s*0.25), s*0.15, st);
        }
        "user-plus" => {
            draw_menu_icon(painter, Rect::from_center_size(c + Vec2::new(-s*0.15, 0.0),
                Vec2::splat(s*1.4)), "user", color);
            ls(Pos2::new(c.x + s*0.35, c.y - s*0.15), Pos2::new(c.x + s*0.35, c.y + s*0.15));
            ls(Pos2::new(c.x + s*0.2, c.y), Pos2::new(c.x + s*0.5, c.y));
        }
        "user-minus" => {
            draw_menu_icon(painter, Rect::from_center_size(c + Vec2::new(-s*0.15, 0.0),
                Vec2::splat(s*1.4)), "user", color);
            ls(Pos2::new(c.x + s*0.2, c.y), Pos2::new(c.x + s*0.5, c.y));
        }
        "user-check" => {
            draw_menu_icon(painter, Rect::from_center_size(c + Vec2::new(-s*0.15, 0.0),
                Vec2::splat(s*1.4)), "user", color);
            ls(Pos2::new(c.x + s*0.2, c.y), Pos2::new(c.x + s*0.3, c.y + s*0.12));
            ls(Pos2::new(c.x + s*0.3, c.y + s*0.12), Pos2::new(c.x + s*0.5, c.y - s*0.12));
        }
        "user-circle" => {
            painter.circle_stroke(c, s*0.45, st);
            painter.circle_stroke(Pos2::new(c.x, c.y - s*0.12), s*0.14, st);
            let arc_c = Pos2::new(c.x, c.y + s*0.5);
            painter.circle_stroke(arc_c, s*0.3, st);
        }

        // ── Media ───────────────────────────────────────────────────────────
        "play" => {
            let pts = [
                Pos2::new(c.x - s*0.3, c.y - s*0.4),
                Pos2::new(c.x + s*0.4, c.y),
                Pos2::new(c.x - s*0.3, c.y + s*0.4),
            ];
            ls(pts[0], pts[1]); ls(pts[1], pts[2]); ls(pts[2], pts[0]);
        }
        "pause" => {
            ls(Pos2::new(c.x - s*0.2, c.y - s*0.35), Pos2::new(c.x - s*0.2, c.y + s*0.35));
            ls(Pos2::new(c.x + s*0.2, c.y - s*0.35), Pos2::new(c.x + s*0.2, c.y + s*0.35));
        }
        "stop" => {
            painter.rect_stroke(Rect::from_center_size(c, Vec2::splat(s*0.7)), s*0.05, st);
        }
        "skip-forward" => {
            ls(Pos2::new(c.x - s*0.35, c.y - s*0.3), Pos2::new(c.x + s*0.05, c.y));
            ls(Pos2::new(c.x + s*0.05, c.y), Pos2::new(c.x - s*0.35, c.y + s*0.3));
            ls(Pos2::new(c.x + s*0.15, c.y - s*0.3), Pos2::new(c.x + s*0.15, c.y + s*0.3));
        }
        "skip-back" => {
            ls(Pos2::new(c.x + s*0.35, c.y - s*0.3), Pos2::new(c.x - s*0.05, c.y));
            ls(Pos2::new(c.x - s*0.05, c.y), Pos2::new(c.x + s*0.35, c.y + s*0.3));
            ls(Pos2::new(c.x - s*0.15, c.y - s*0.3), Pos2::new(c.x - s*0.15, c.y + s*0.3));
        }
        "volume" => {
            ls(Pos2::new(c.x - s*0.35, c.y - s*0.15), Pos2::new(c.x - s*0.1, c.y - s*0.15));
            ls(Pos2::new(c.x - s*0.1, c.y - s*0.15), Pos2::new(c.x + s*0.1, c.y - s*0.35));
            ls(Pos2::new(c.x + s*0.1, c.y - s*0.35), Pos2::new(c.x + s*0.1, c.y + s*0.35));
            ls(Pos2::new(c.x + s*0.1, c.y + s*0.35), Pos2::new(c.x - s*0.1, c.y + s*0.15));
            ls(Pos2::new(c.x - s*0.1, c.y + s*0.15), Pos2::new(c.x - s*0.35, c.y + s*0.15));
            ls(Pos2::new(c.x - s*0.35, c.y + s*0.15), Pos2::new(c.x - s*0.35, c.y - s*0.15));
            painter.circle_stroke(Pos2::new(c.x + s*0.1, c.y), s*0.25, st);
        }
        "volume-off" => {
            draw_menu_icon(painter, rect, "volume", color);
            lsh(Pos2::new(c.x - s*0.3, c.y + s*0.35), Pos2::new(c.x + s*0.4, c.y - s*0.35));
        }
        "music" => {
            ls(Pos2::new(c.x - s*0.2, c.y - s*0.4), Pos2::new(c.x - s*0.2, c.y + s*0.2));
            ls(Pos2::new(c.x + s*0.2, c.y - s*0.25), Pos2::new(c.x + s*0.2, c.y + s*0.35));
            ls(Pos2::new(c.x - s*0.2, c.y - s*0.4), Pos2::new(c.x + s*0.2, c.y - s*0.25));
            painter.circle_filled(Pos2::new(c.x - s*0.3, c.y + s*0.2), s*0.12, color);
            painter.circle_filled(Pos2::new(c.x + s*0.1, c.y + s*0.35), s*0.12, color);
        }

        // ── Data ────────────────────────────────────────────────────────────
        "database" => {
            for i in 0..3 {
                let y = c.y - s*0.3 + i as f32 * s*0.3;
                let ry = s * 0.12;
                painter.circle_stroke(Pos2::new(c.x, y), s*0.35, Stroke::new(st.width, Color32::TRANSPARENT));
                // Top/bottom ellipse approximation
                ls(Pos2::new(c.x - s*0.35, y), Pos2::new(c.x + s*0.35, y));
            }
            ls(Pos2::new(c.x - s*0.35, c.y - s*0.3), Pos2::new(c.x - s*0.35, c.y + s*0.3));
            ls(Pos2::new(c.x + s*0.35, c.y - s*0.3), Pos2::new(c.x + s*0.35, c.y + s*0.3));
            painter.circle_stroke(Pos2::new(c.x, c.y - s*0.3), s*0.35, st);
        }
        "chart-bar" => {
            ls(Pos2::new(c.x - s*0.4, c.y + s*0.4), Pos2::new(c.x + s*0.4, c.y + s*0.4));
            ls(Pos2::new(c.x - s*0.4, c.y + s*0.4), Pos2::new(c.x - s*0.4, c.y - s*0.4));
            painter.rect_filled(Rect::from_min_max(
                Pos2::new(c.x - s*0.3, c.y - s*0.1), Pos2::new(c.x - s*0.1, c.y + s*0.4)), 0.0, color);
            painter.rect_filled(Rect::from_min_max(
                Pos2::new(c.x - s*0.05, c.y - s*0.3), Pos2::new(c.x + s*0.15, c.y + s*0.4)), 0.0, color);
            painter.rect_filled(Rect::from_min_max(
                Pos2::new(c.x + s*0.2, c.y), Pos2::new(c.x + s*0.4, c.y + s*0.4)), 0.0, color);
        }
        "chart-line" => {
            ls(Pos2::new(c.x - s*0.4, c.y + s*0.4), Pos2::new(c.x + s*0.4, c.y + s*0.4));
            ls(Pos2::new(c.x - s*0.4, c.y + s*0.4), Pos2::new(c.x - s*0.4, c.y - s*0.4));
            ls(Pos2::new(c.x - s*0.3, c.y + s*0.2), Pos2::new(c.x - s*0.05, c.y - s*0.1));
            ls(Pos2::new(c.x - s*0.05, c.y - s*0.1), Pos2::new(c.x + s*0.15, c.y + s*0.1));
            ls(Pos2::new(c.x + s*0.15, c.y + s*0.1), Pos2::new(c.x + s*0.35, c.y - s*0.25));
        }
        "chart-pie" => {
            painter.circle_stroke(c, s*0.4, st);
            ls(c, Pos2::new(c.x, c.y - s*0.4));
            ls(c, Pos2::new(c.x + s*0.35, c.y + s*0.2));
        }
        "table" => {
            painter.rect_stroke(Rect::from_center_size(c, Vec2::splat(s*0.85)), s*0.05, st);
            ls(Pos2::new(c.x - s*0.42, c.y - s*0.15), Pos2::new(c.x + s*0.42, c.y - s*0.15));
            ls(Pos2::new(c.x - s*0.42, c.y + s*0.15), Pos2::new(c.x + s*0.42, c.y + s*0.15));
            ls(Pos2::new(c.x - s*0.1, c.y - s*0.42), Pos2::new(c.x - s*0.1, c.y + s*0.42));
        }
        "filter" => {
            ls(Pos2::new(c.x - s*0.4, c.y - s*0.35), Pos2::new(c.x + s*0.4, c.y - s*0.35));
            ls(Pos2::new(c.x - s*0.4, c.y - s*0.35), Pos2::new(c.x - s*0.05, c.y + s*0.05));
            ls(Pos2::new(c.x + s*0.4, c.y - s*0.35), Pos2::new(c.x + s*0.05, c.y + s*0.05));
            ls(Pos2::new(c.x - s*0.05, c.y + s*0.05), Pos2::new(c.x - s*0.05, c.y + s*0.4));
            ls(Pos2::new(c.x + s*0.05, c.y + s*0.05), Pos2::new(c.x + s*0.05, c.y + s*0.4));
        }
        "sort-asc" => {
            for i in 0..3 {
                let y = c.y - s*0.3 + i as f32 * s*0.3;
                let w = s * 0.2 + i as f32 * s*0.15;
                ls(Pos2::new(c.x - s*0.35, y), Pos2::new(c.x - s*0.35 + w, y));
            }
            ls(Pos2::new(c.x + s*0.3, c.y - s*0.35), Pos2::new(c.x + s*0.3, c.y + s*0.35));
            ls(Pos2::new(c.x + s*0.3, c.y - s*0.35), Pos2::new(c.x + s*0.15, c.y - s*0.15));
            ls(Pos2::new(c.x + s*0.3, c.y - s*0.35), Pos2::new(c.x + s*0.45, c.y - s*0.15));
        }
        "sort-desc" => {
            for i in 0..3 {
                let y = c.y - s*0.3 + i as f32 * s*0.3;
                let w = s * 0.5 - i as f32 * s*0.15;
                ls(Pos2::new(c.x - s*0.35, y), Pos2::new(c.x - s*0.35 + w, y));
            }
            ls(Pos2::new(c.x + s*0.3, c.y - s*0.35), Pos2::new(c.x + s*0.3, c.y + s*0.35));
            ls(Pos2::new(c.x + s*0.3, c.y + s*0.35), Pos2::new(c.x + s*0.15, c.y + s*0.15));
            ls(Pos2::new(c.x + s*0.3, c.y + s*0.35), Pos2::new(c.x + s*0.45, c.y + s*0.15));
        }

        // ── System ──────────────────────────────────────────────────────────
        "gear" => {
            painter.circle_stroke(c, s*0.2, st);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                let inner = s * 0.28;
                let outer = s * 0.42;
                ls(Pos2::new(c.x + inner*a.cos(), c.y + inner*a.sin()),
                   Pos2::new(c.x + outer*a.cos(), c.y + outer*a.sin()));
            }
        }
        "wrench" => {
            lsh(Pos2::new(c.x - s*0.35, c.y + s*0.35), Pos2::new(c.x + s*0.1, c.y - s*0.1));
            painter.circle_stroke(Pos2::new(c.x + s*0.2, c.y - s*0.2), s*0.22, st);
        }
        "shield" => {
            let pts = [
                Pos2::new(c.x, c.y - s*0.5),
                Pos2::new(c.x + s*0.4, c.y - s*0.25),
                Pos2::new(c.x + s*0.35, c.y + s*0.15),
                Pos2::new(c.x, c.y + s*0.5),
                Pos2::new(c.x - s*0.35, c.y + s*0.15),
                Pos2::new(c.x - s*0.4, c.y - s*0.25),
            ];
            for i in 0..pts.len() { ls(pts[i], pts[(i+1) % pts.len()]); }
        }
        "lock" => {
            painter.rect_stroke(Rect::from_min_max(
                Pos2::new(c.x - s*0.3, c.y - s*0.05), Pos2::new(c.x + s*0.3, c.y + s*0.45)), s*0.05, st);
            ls(Pos2::new(c.x - s*0.2, c.y - s*0.05), Pos2::new(c.x - s*0.2, c.y - s*0.25));
            ls(Pos2::new(c.x + s*0.2, c.y - s*0.05), Pos2::new(c.x + s*0.2, c.y - s*0.25));
            painter.circle_stroke(Pos2::new(c.x, c.y - s*0.25), s*0.2, st);
        }
        "unlock" => {
            painter.rect_stroke(Rect::from_min_max(
                Pos2::new(c.x - s*0.3, c.y - s*0.05), Pos2::new(c.x + s*0.3, c.y + s*0.45)), s*0.05, st);
            ls(Pos2::new(c.x - s*0.2, c.y - s*0.05), Pos2::new(c.x - s*0.2, c.y - s*0.25));
            painter.circle_stroke(Pos2::new(c.x, c.y - s*0.25), s*0.2, st);
        }
        "key" => {
            painter.circle_stroke(Pos2::new(c.x - s*0.2, c.y), s*0.22, st);
            ls(Pos2::new(c.x + s*0.0, c.y), Pos2::new(c.x + s*0.45, c.y));
            ls(Pos2::new(c.x + s*0.3, c.y), Pos2::new(c.x + s*0.3, c.y + s*0.15));
            ls(Pos2::new(c.x + s*0.45, c.y), Pos2::new(c.x + s*0.45, c.y + s*0.15));
        }
        "terminal" => {
            painter.rect_stroke(Rect::from_center_size(c, Vec2::new(s*0.9, s*0.7)), s*0.08, st);
            ls(Pos2::new(c.x - s*0.3, c.y - s*0.1), Pos2::new(c.x - s*0.1, c.y + s*0.1));
            ls(Pos2::new(c.x - s*0.1, c.y + s*0.1), Pos2::new(c.x - s*0.3, c.y + s*0.3));
            ls(Pos2::new(c.x + s*0.0, c.y + s*0.3), Pos2::new(c.x + s*0.3, c.y + s*0.3));
        }
        "code" => {
            ls(Pos2::new(c.x - s*0.15, c.y - s*0.35), Pos2::new(c.x - s*0.4, c.y));
            ls(Pos2::new(c.x - s*0.4, c.y), Pos2::new(c.x - s*0.15, c.y + s*0.35));
            ls(Pos2::new(c.x + s*0.15, c.y - s*0.35), Pos2::new(c.x + s*0.4, c.y));
            ls(Pos2::new(c.x + s*0.4, c.y), Pos2::new(c.x + s*0.15, c.y + s*0.35));
        }
        "bug" => {
            painter.circle_stroke(c, s*0.25, st);
            painter.circle_filled(Pos2::new(c.x, c.y - s*0.3), s*0.12, color);
            for i in 0..3 {
                let y = c.y - s*0.15 + i as f32 * s*0.15;
                ls(Pos2::new(c.x - s*0.25, y), Pos2::new(c.x - s*0.45, y - s*0.1));
                ls(Pos2::new(c.x + s*0.25, y), Pos2::new(c.x + s*0.45, y - s*0.1));
            }
        }
        "cpu" => {
            painter.rect_stroke(Rect::from_center_size(c, Vec2::splat(s*0.55)), s*0.05, st);
            for i in 0..3 {
                let off = -s*0.15 + i as f32 * s*0.15;
                ls(Pos2::new(c.x + off, c.y - s*0.275), Pos2::new(c.x + off, c.y - s*0.42));
                ls(Pos2::new(c.x + off, c.y + s*0.275), Pos2::new(c.x + off, c.y + s*0.42));
                ls(Pos2::new(c.x - s*0.275, c.y + off), Pos2::new(c.x - s*0.42, c.y + off));
                ls(Pos2::new(c.x + s*0.275, c.y + off), Pos2::new(c.x + s*0.42, c.y + off));
            }
        }

        // ── Status ──────────────────────────────────────────────────────────
        "info-circle" => {
            painter.circle_stroke(c, s*0.4, st);
            painter.circle_filled(Pos2::new(c.x, c.y - s*0.2), s*0.05, color);
            ls(Pos2::new(c.x, c.y - s*0.05), Pos2::new(c.x, c.y + s*0.25));
        }
        "warning-triangle" => {
            let pts = [
                Pos2::new(c.x, c.y - s*0.4),
                Pos2::new(c.x + s*0.42, c.y + s*0.35),
                Pos2::new(c.x - s*0.42, c.y + s*0.35),
            ];
            for i in 0..3 { ls(pts[i], pts[(i+1)%3]); }
            painter.circle_filled(Pos2::new(c.x, c.y + s*0.2), s*0.04, color);
            ls(Pos2::new(c.x, c.y - s*0.15), Pos2::new(c.x, c.y + s*0.1));
        }
        "error-circle" => {
            painter.circle_stroke(c, s*0.4, st);
            ls(Pos2::new(c.x - s*0.2, c.y - s*0.2), Pos2::new(c.x + s*0.2, c.y + s*0.2));
            ls(Pos2::new(c.x + s*0.2, c.y - s*0.2), Pos2::new(c.x - s*0.2, c.y + s*0.2));
        }
        "help-circle" => {
            painter.circle_stroke(c, s*0.4, st);
            painter.text(c + Vec2::new(0.0, -s*0.05), egui::Align2::CENTER_CENTER,
                "?", egui::FontId::proportional(s*0.6), color);
        }
        "check-circle" => {
            painter.circle_stroke(c, s*0.4, st);
            ls(Pos2::new(c.x - s*0.2, c.y), Pos2::new(c.x - s*0.05, c.y + s*0.15));
            ls(Pos2::new(c.x - s*0.05, c.y + s*0.15), Pos2::new(c.x + s*0.2, c.y - s*0.15));
        }
        "x-circle" => {
            painter.circle_stroke(c, s*0.4, st);
            ls(Pos2::new(c.x - s*0.18, c.y - s*0.18), Pos2::new(c.x + s*0.18, c.y + s*0.18));
            ls(Pos2::new(c.x + s*0.18, c.y - s*0.18), Pos2::new(c.x - s*0.18, c.y + s*0.18));
        }
        "clock" => {
            painter.circle_stroke(c, s*0.4, st);
            ls(c, Pos2::new(c.x, c.y - s*0.25));
            ls(c, Pos2::new(c.x + s*0.2, c.y + s*0.05));
        }
        "calendar" => {
            painter.rect_stroke(Rect::from_center_size(c + Vec2::new(0.0, s*0.05), Vec2::new(s*0.8, s*0.7)), s*0.05, st);
            ls(Pos2::new(c.x - s*0.4, c.y - s*0.12), Pos2::new(c.x + s*0.4, c.y - s*0.12));
            ls(Pos2::new(c.x - s*0.2, c.y - s*0.3), Pos2::new(c.x - s*0.2, c.y - s*0.45));
            ls(Pos2::new(c.x + s*0.2, c.y - s*0.3), Pos2::new(c.x + s*0.2, c.y - s*0.45));
        }

        // ── Commerce ────────────────────────────────────────────────────────
        "cart" => {
            ls(Pos2::new(c.x - s*0.45, c.y - s*0.35), Pos2::new(c.x - s*0.3, c.y - s*0.35));
            ls(Pos2::new(c.x - s*0.3, c.y - s*0.35), Pos2::new(c.x - s*0.15, c.y + s*0.15));
            ls(Pos2::new(c.x - s*0.15, c.y + s*0.15), Pos2::new(c.x + s*0.35, c.y + s*0.15));
            ls(Pos2::new(c.x + s*0.35, c.y + s*0.15), Pos2::new(c.x + s*0.4, c.y - s*0.25));
            ls(Pos2::new(c.x + s*0.4, c.y - s*0.25), Pos2::new(c.x - s*0.25, c.y - s*0.25));
            painter.circle_filled(Pos2::new(c.x - s*0.1, c.y + s*0.32), s*0.07, color);
            painter.circle_filled(Pos2::new(c.x + s*0.25, c.y + s*0.32), s*0.07, color);
        }
        "credit-card" => {
            painter.rect_stroke(Rect::from_center_size(c, Vec2::new(s*0.9, s*0.55)), s*0.08, st);
            ls(Pos2::new(c.x - s*0.45, c.y - s*0.08), Pos2::new(c.x + s*0.45, c.y - s*0.08));
        }
        "wallet" => {
            painter.rect_stroke(Rect::from_center_size(c, Vec2::new(s*0.85, s*0.65)), s*0.08, st);
            painter.rect_stroke(Rect::from_min_max(
                Pos2::new(c.x + s*0.15, c.y - s*0.08), Pos2::new(c.x + s*0.42, c.y + s*0.12)), s*0.04, st);
        }
        "receipt" => {
            let pts = [
                Pos2::new(c.x - s*0.3, c.y - s*0.5),
                Pos2::new(c.x + s*0.3, c.y - s*0.5),
                Pos2::new(c.x + s*0.3, c.y + s*0.4),
                Pos2::new(c.x + s*0.15, c.y + s*0.5),
                Pos2::new(c.x, c.y + s*0.4),
                Pos2::new(c.x - s*0.15, c.y + s*0.5),
                Pos2::new(c.x - s*0.3, c.y + s*0.4),
            ];
            for i in 0..pts.len() { ls(pts[i], pts[(i+1) % pts.len()]); }
            ls(Pos2::new(c.x - s*0.15, c.y - s*0.2), Pos2::new(c.x + s*0.15, c.y - s*0.2));
            ls(Pos2::new(c.x - s*0.15, c.y), Pos2::new(c.x + s*0.15, c.y));
        }
        "tag" => {
            let pts = [
                Pos2::new(c.x - s*0.4, c.y - s*0.4),
                Pos2::new(c.x + s*0.05, c.y - s*0.4),
                Pos2::new(c.x + s*0.4, c.y),
                Pos2::new(c.x + s*0.05, c.y + s*0.4),
                Pos2::new(c.x - s*0.4, c.y + s*0.4),
            ];
            for i in 0..pts.len() { ls(pts[i], pts[(i+1) % pts.len()]); }
            painter.circle_filled(Pos2::new(c.x - s*0.2, c.y), s*0.06, color);
        }
        "percent" => {
            painter.circle_stroke(Pos2::new(c.x - s*0.2, c.y - s*0.2), s*0.12, st);
            painter.circle_stroke(Pos2::new(c.x + s*0.2, c.y + s*0.2), s*0.12, st);
            ls(Pos2::new(c.x + s*0.3, c.y - s*0.3), Pos2::new(c.x - s*0.3, c.y + s*0.3));
        }

        // ── File/Folder ─────────────────────────────────────────────────────
        "folder" => {
            let pts = [
                Pos2::new(c.x - s*0.45, c.y - s*0.25),
                Pos2::new(c.x - s*0.1, c.y - s*0.25),
                Pos2::new(c.x, c.y - s*0.4),
                Pos2::new(c.x + s*0.45, c.y - s*0.4),
                Pos2::new(c.x + s*0.45, c.y + s*0.3),
                Pos2::new(c.x - s*0.45, c.y + s*0.3),
            ];
            for i in 0..pts.len() { ls(pts[i], pts[(i+1) % pts.len()]); }
        }
        "folder-open" => {
            draw_menu_icon(painter, rect, "folder", color);
            ls(Pos2::new(c.x - s*0.45, c.y + s*0.3), Pos2::new(c.x - s*0.3, c.y));
            ls(Pos2::new(c.x - s*0.3, c.y), Pos2::new(c.x + s*0.55, c.y));
            ls(Pos2::new(c.x + s*0.55, c.y), Pos2::new(c.x + s*0.45, c.y + s*0.3));
        }
        "folder-plus" => {
            draw_menu_icon(painter, rect, "folder", color);
            ls(Pos2::new(c.x, c.y - s*0.1), Pos2::new(c.x, c.y + s*0.2));
            ls(Pos2::new(c.x - s*0.15, c.y + s*0.05), Pos2::new(c.x + s*0.15, c.y + s*0.05));
        }
        "archive" => {
            painter.rect_stroke(Rect::from_min_max(
                Pos2::new(c.x - s*0.45, c.y - s*0.4), Pos2::new(c.x + s*0.45, c.y - s*0.15)), s*0.05, st);
            painter.rect_stroke(Rect::from_min_max(
                Pos2::new(c.x - s*0.4, c.y - s*0.15), Pos2::new(c.x + s*0.4, c.y + s*0.4)), s*0.05, st);
            ls(Pos2::new(c.x - s*0.1, c.y + s*0.05), Pos2::new(c.x + s*0.1, c.y + s*0.05));
        }
        "trash" => {
            ls(Pos2::new(c.x - s*0.35, c.y - s*0.25), Pos2::new(c.x + s*0.35, c.y - s*0.25));
            ls(Pos2::new(c.x - s*0.1, c.y - s*0.25), Pos2::new(c.x - s*0.1, c.y - s*0.4));
            ls(Pos2::new(c.x + s*0.1, c.y - s*0.25), Pos2::new(c.x + s*0.1, c.y - s*0.4));
            ls(Pos2::new(c.x - s*0.1, c.y - s*0.4), Pos2::new(c.x + s*0.1, c.y - s*0.4));
            painter.rect_stroke(Rect::from_min_max(
                Pos2::new(c.x - s*0.28, c.y - s*0.25), Pos2::new(c.x + s*0.28, c.y + s*0.45)), s*0.03, st);
            ls(Pos2::new(c.x, c.y - s*0.1), Pos2::new(c.x, c.y + s*0.3));
        }
        "printer" => {
            painter.rect_stroke(Rect::from_min_max(
                Pos2::new(c.x - s*0.25, c.y - s*0.45), Pos2::new(c.x + s*0.25, c.y - s*0.15)), s*0.03, st);
            painter.rect_stroke(Rect::from_min_max(
                Pos2::new(c.x - s*0.4, c.y - s*0.15), Pos2::new(c.x + s*0.4, c.y + s*0.15)), s*0.05, st);
            painter.rect_stroke(Rect::from_min_max(
                Pos2::new(c.x - s*0.25, c.y + s*0.15), Pos2::new(c.x + s*0.25, c.y + s*0.42)), s*0.03, st);
        }

        // ── Fallback ────────────────────────────────────────────────────────
        _ => {
            painter.rect_stroke(Rect::from_center_size(c, Vec2::splat(s*0.6)), s*0.05, st);
            painter.text(c, egui::Align2::CENTER_CENTER, "?", egui::FontId::proportional(s*0.5), color);
        }
    }
}

#[cfg(feature = "render")]
fn icon_doc_outline(painter: &egui::Painter, c: Pos2, s: f32, st: Stroke) {
    let pts = [
        Pos2::new(c.x - s*0.35, c.y - s*0.55),
        Pos2::new(c.x + s*0.15, c.y - s*0.55),
        Pos2::new(c.x + s*0.35, c.y - s*0.35),
        Pos2::new(c.x + s*0.35, c.y + s*0.55),
        Pos2::new(c.x - s*0.35, c.y + s*0.55),
    ];
    for i in 0..pts.len() {
        painter.line_segment([pts[i], pts[(i + 1) % pts.len()]], st);
    }
    painter.line_segment([pts[1], Pos2::new(c.x + s*0.15, c.y - s*0.35)], st);
    painter.line_segment([Pos2::new(c.x + s*0.15, c.y - s*0.35), pts[2]], st);
}
