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
    "doc-new",
    "doc-open",
    "doc-save",
    "doc-save-as",
    "doc-copy",
    "doc-blank",
    "doc-text",
    "doc-pdf",
    "doc-spreadsheet",
    "doc-stack",
    // Edit (12)
    "scissors",
    "clipboard-copy",
    "clipboard-paste",
    "pencil",
    "eraser",
    "pen",
    "brush",
    "type-text",
    "bold",
    "italic",
    "underline",
    "strikethrough",
    // Navigation (10)
    "arrow-left",
    "arrow-right",
    "arrow-up",
    "arrow-down",
    "chevron-left",
    "chevron-right",
    "chevron-up",
    "chevron-down",
    "home",
    "external-link",
    // Action (12)
    "plus",
    "minus",
    "check",
    "x-mark",
    "refresh",
    "sync",
    "download",
    "upload",
    "share",
    "export",
    "import",
    "link",
    // UI/View (10)
    "eye",
    "eye-off",
    "magnifier",
    "zoom-in",
    "zoom-out",
    "fullscreen",
    "collapse",
    "expand",
    "grid-view",
    "list-view",
    // Communication (10)
    "mail",
    "mail-open",
    "send",
    "inbox",
    "chat",
    "phone",
    "video",
    "bell",
    "bell-off",
    "at-sign",
    // Social (6)
    "heart",
    "star",
    "thumbs-up",
    "thumbs-down",
    "bookmark",
    "flag",
    // People/User (6)
    "user",
    "users",
    "user-plus",
    "user-minus",
    "user-check",
    "user-circle",
    // Media (8)
    "play",
    "pause",
    "stop",
    "skip-forward",
    "skip-back",
    "volume",
    "volume-off",
    "music",
    // Data (8)
    "database",
    "chart-bar",
    "chart-line",
    "chart-pie",
    "table",
    "filter",
    "sort-asc",
    "sort-desc",
    // System (10)
    "gear",
    "wrench",
    "shield",
    "lock",
    "unlock",
    "key",
    "terminal",
    "code",
    "bug",
    "cpu",
    // Status (8)
    "info-circle",
    "warning-triangle",
    "error-circle",
    "help-circle",
    "check-circle",
    "x-circle",
    "clock",
    "calendar",
    // Commerce (6)
    "cart",
    "credit-card",
    "wallet",
    "receipt",
    "tag",
    "percent",
    // File/Folder (6)
    "folder",
    "folder-open",
    "folder-plus",
    "archive",
    "trash",
    "printer",
    // Payroll (25)
    "payroll-check",
    "payroll-schedule",
    "payroll-deduction",
    "payroll-bonus",
    "payroll-overtime",
    "payroll-tax",
    "payroll-slip",
    "payroll-direct-deposit",
    "payroll-timesheet",
    "payroll-hours",
    "payroll-employee",
    "payroll-benefits",
    "payroll-pension",
    "payroll-vacation",
    "payroll-sick-leave",
    "payroll-commission",
    "payroll-garnishment",
    "payroll-reimbursement",
    "payroll-w2",
    "payroll-1099",
    "payroll-ytd",
    "payroll-net-pay",
    "payroll-gross-pay",
    "payroll-withholding",
    "payroll-frequency",
    // Receivables (25)
    "invoice",
    "invoice-paid",
    "invoice-overdue",
    "invoice-draft",
    "invoice-send",
    "credit-memo",
    "debit-memo",
    "aging-report",
    "collection",
    "dunning-letter",
    "payment-received",
    "partial-payment",
    "advance-payment",
    "refund",
    "write-off",
    "bad-debt",
    "interest-charge",
    "statement",
    "customer-balance",
    "account-receivable",
    "open-items",
    "clearing",
    "remittance",
    "factoring",
    "credit-limit",
    // Payments (25)
    "payment-check",
    "payment-wire",
    "payment-ach",
    "payment-cash",
    "payment-pending",
    "payment-approved",
    "payment-rejected",
    "payment-recurring",
    "payment-split",
    "payment-batch",
    "payment-void",
    "payment-reversal",
    "vendor-payment",
    "bill-pay",
    "purchase-order",
    "expense-report",
    "petty-cash",
    "bank-transfer",
    "payment-gateway",
    "payment-terms",
    "early-discount",
    "payment-plan",
    "installment",
    "escrow",
    "disbursement",
    // Stock Control (25)
    "inventory",
    "warehouse",
    "stock-in",
    "stock-out",
    "stock-count",
    "stock-transfer",
    "stock-adjust",
    "stock-reserve",
    "stock-alert",
    "stock-reorder",
    "barcode",
    "qr-code",
    "pallet",
    "shelf",
    "bin-location",
    "lot-number",
    "serial-number",
    "expiry-date",
    "fifo",
    "lifo",
    "cycle-count",
    "physical-count",
    "stock-valuation",
    "safety-stock",
    "dead-stock",
    // Transportation (25)
    "truck",
    "truck-loading",
    "truck-delivery",
    "van",
    "ship",
    "ship-cargo",
    "airplane",
    "airplane-landing",
    "helicopter",
    "train",
    "railway",
    "container",
    "forklift",
    "crane",
    "anchor",
    "compass",
    "route",
    "highway",
    "bridge",
    "toll",
    "fuel-pump",
    "tire",
    "engine",
    "speedometer",
    "odometer",
    // Logistics (25)
    "package",
    "package-open",
    "package-check",
    "package-x",
    "package-search",
    "conveyor",
    "loading-dock",
    "dispatch",
    "tracking",
    "tracking-number",
    "delivery-time",
    "express",
    "fragile",
    "hazmat",
    "temperature",
    "weight-scale",
    "dimensions",
    "customs",
    "manifest",
    "bill-of-lading",
    "cross-dock",
    "last-mile",
    "return-shipment",
    "consolidation",
    "deconsolidation",
    // Financial (25)
    "dollar",
    "euro",
    "yen",
    "pound",
    "bitcoin",
    "coins",
    "money-bag",
    "piggy-bank",
    "vault",
    "safe",
    "bank",
    "atm",
    "exchange-rate",
    "stock-market",
    "bull-market",
    "bear-market",
    "dividend",
    "interest-rate",
    "mortgage",
    "loan",
    "audit",
    "ledger",
    "balance-sheet",
    "profit-loss",
    "cash-flow",
    // Social Media (25)
    "like",
    "dislike",
    "comment",
    "repost",
    "mention",
    "hashtag",
    "trending",
    "viral",
    "follower",
    "following",
    "profile",
    "bio",
    "story",
    "reel",
    "live-stream",
    "notification-dot",
    "verified",
    "influencer",
    "engagement",
    "reach",
    "post",
    "feed",
    "timeline",
    "dm",
    "group-chat",
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
            ls(Pos2::new(c.x, c.y + s * 0.1), Pos2::new(c.x, c.y + s * 0.5));
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.3),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.3),
            );
        }
        "doc-open" => {
            icon_doc_outline(painter, c, s, st);
            ls(
                Pos2::new(c.x - s * 0.3, c.y + s * 0.5),
                Pos2::new(c.x + s * 0.5, c.y + s * 0.5),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.3),
                Pos2::new(c.x + s * 0.5, c.y + s * 0.5),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.7),
                Pos2::new(c.x + s * 0.5, c.y + s * 0.5),
            );
        }
        "doc-save" => {
            let tl = Pos2::new(c.x - s * 0.5, c.y - s * 0.6);
            let br = Pos2::new(c.x + s * 0.5, c.y + s * 0.6);
            painter.rect_stroke(
                Rect::from_min_max(tl, br),
                s * 0.1,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.6),
                Pos2::new(c.x - s * 0.2, c.y - s * 0.25),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.6),
                Pos2::new(c.x + s * 0.2, c.y - s * 0.25),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.25),
                Pos2::new(c.x + s * 0.2, c.y - s * 0.25),
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.3, c.y + s * 0.1),
                    Pos2::new(c.x + s * 0.3, c.y + s * 0.6),
                ),
                s * 0.05,
                st,
                egui::StrokeKind::Middle,
            );
        }
        "doc-save-as" => {
            draw_menu_icon(painter, rect.shrink(s * 0.15), "doc-save", color);
            painter.text(
                Pos2::new(rect.max.x - s * 0.15, rect.max.y - s * 0.1),
                egui::Align2::RIGHT_BOTTOM,
                "…",
                egui::FontId::proportional(s * 0.6),
                color,
            );
        }
        "doc-copy" => {
            let off = s * 0.15;
            let r1 = Rect::from_min_max(
                Pos2::new(c.x - s * 0.35 + off, c.y - s * 0.55),
                Pos2::new(c.x + s * 0.35 + off, c.y + s * 0.35),
            );
            let r2 = Rect::from_min_max(
                Pos2::new(c.x - s * 0.35 - off, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.35 - off, c.y + s * 0.55),
            );
            painter.rect_stroke(r1, s * 0.05, st, egui::StrokeKind::Middle);
            painter.rect_stroke(r2, s * 0.05, st, egui::StrokeKind::Middle);
        }
        "doc-blank" => {
            icon_doc_outline(painter, c, s, st);
        }
        "doc-text" => {
            icon_doc_outline(painter, c, s, st);
            for i in 0..3 {
                let y = c.y - s * 0.15 + i as f32 * s * 0.22;
                ls(Pos2::new(c.x - s * 0.25, y), Pos2::new(c.x + s * 0.15, y));
            }
        }
        "doc-pdf" => {
            icon_doc_outline(painter, c, s, st);
            painter.text(
                c + Vec2::new(0.0, s * 0.1),
                egui::Align2::CENTER_CENTER,
                "P",
                egui::FontId::proportional(s * 0.6),
                color,
            );
        }
        "doc-spreadsheet" => {
            icon_doc_outline(painter, c, s, st);
            for i in 0..2 {
                for j in 0..2 {
                    let x = c.x - s * 0.18 + j as f32 * s * 0.24;
                    let y = c.y - s * 0.05 + i as f32 * s * 0.22;
                    painter.rect_stroke(
                        Rect::from_min_size(Pos2::new(x, y), Vec2::splat(s * 0.18)),
                        0.0,
                        st,
                        egui::StrokeKind::Middle,
                    );
                }
            }
        }
        "doc-stack" => {
            for i in 0..3 {
                let off = i as f32 * s * 0.12;
                let r = Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35 + off, c.y - s * 0.5 + off),
                    Pos2::new(c.x + s * 0.35 + off, c.y + s * 0.4 + off),
                );
                painter.rect_stroke(r, s * 0.05, st, egui::StrokeKind::Middle);
            }
        }

        // ── Edit ────────────────────────────────────────────────────────────
        "scissors" => {
            painter.circle_stroke(Pos2::new(c.x - s * 0.3, c.y + s * 0.35), s * 0.2, st);
            painter.circle_stroke(Pos2::new(c.x + s * 0.3, c.y + s * 0.35), s * 0.2, st);
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.3, c.y - s * 0.5),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y + s * 0.2),
                Pos2::new(c.x - s * 0.3, c.y - s * 0.5),
            );
        }
        "clipboard-copy" => {
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::new(s * 0.8, s)),
                s * 0.08,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y - s * 0.5),
                Pos2::new(c.x - s * 0.15, c.y - s * 0.65),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y - s * 0.5),
                Pos2::new(c.x + s * 0.15, c.y - s * 0.65),
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y - s * 0.65),
                Pos2::new(c.x + s * 0.15, c.y - s * 0.65),
            );
        }
        "clipboard-paste" => {
            draw_menu_icon(painter, rect, "clipboard-copy", color);
            ls(Pos2::new(c.x, c.y - s * 0.1), Pos2::new(c.x, c.y + s * 0.3));
            ls(
                Pos2::new(c.x - s * 0.12, c.y + s * 0.15),
                Pos2::new(c.x, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.12, c.y + s * 0.15),
                Pos2::new(c.x, c.y + s * 0.3),
            );
        }
        "pencil" => {
            let tip = Pos2::new(c.x - s * 0.4, c.y + s * 0.4);
            let top = Pos2::new(c.x + s * 0.35, c.y - s * 0.45);
            lsh(tip, top);
            ls(tip, Pos2::new(c.x - s * 0.5, c.y + s * 0.55));
        }
        "eraser" => {
            let pts = [
                Pos2::new(c.x - s * 0.4, c.y + s * 0.2),
                Pos2::new(c.x - s * 0.1, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.4),
            ];
            for i in 0..4 {
                ls(pts[i], pts[(i + 1) % 4]);
            }
            ls(
                Pos2::new(c.x - s * 0.25, c.y - s * 0.05),
                Pos2::new(c.x + s * 0.25, c.y + s * 0.15),
            );
        }
        "pen" => {
            let tip = Pos2::new(c.x - s * 0.45, c.y + s * 0.45);
            let top = Pos2::new(c.x + s * 0.3, c.y - s * 0.4);
            lsh(tip, top);
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.5),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.3),
            );
        }
        "brush" => {
            ls(Pos2::new(c.x - s * 0.1, c.y + s * 0.5), Pos2::new(c.x, c.y));
            painter.circle_filled(Pos2::new(c.x + s * 0.15, c.y - s * 0.25), s * 0.25, color);
        }
        "type-text" => {
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                "T",
                egui::FontId::proportional(s * 1.2),
                color,
            );
        }
        "bold" => {
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                "B",
                egui::FontId::proportional(s * 1.2),
                color,
            );
        }
        "italic" => {
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                "I",
                egui::FontId::proportional(s * 1.2),
                color,
            );
        }
        "underline" => {
            painter.text(
                Pos2::new(c.x, c.y - s * 0.1),
                egui::Align2::CENTER_CENTER,
                "U",
                egui::FontId::proportional(s * 1.0),
                color,
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y + s * 0.45),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.45),
            );
        }
        "strikethrough" => {
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                "S",
                egui::FontId::proportional(s * 1.0),
                color,
            );
            ls(Pos2::new(c.x - s * 0.4, c.y), Pos2::new(c.x + s * 0.4, c.y));
        }

        // ── Navigation ──────────────────────────────────────────────────────
        "arrow-left" => {
            ls(Pos2::new(c.x + s * 0.4, c.y), Pos2::new(c.x - s * 0.4, c.y));
            ls(
                Pos2::new(c.x - s * 0.4, c.y),
                Pos2::new(c.x - s * 0.1, c.y - s * 0.3),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y),
                Pos2::new(c.x - s * 0.1, c.y + s * 0.3),
            );
        }
        "arrow-right" => {
            ls(Pos2::new(c.x - s * 0.4, c.y), Pos2::new(c.x + s * 0.4, c.y));
            ls(
                Pos2::new(c.x + s * 0.4, c.y),
                Pos2::new(c.x + s * 0.1, c.y - s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.4, c.y),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.3),
            );
        }
        "arrow-up" => {
            ls(Pos2::new(c.x, c.y + s * 0.4), Pos2::new(c.x, c.y - s * 0.4));
            ls(
                Pos2::new(c.x, c.y - s * 0.4),
                Pos2::new(c.x - s * 0.3, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.3, c.y - s * 0.1),
            );
        }
        "arrow-down" => {
            ls(Pos2::new(c.x, c.y - s * 0.4), Pos2::new(c.x, c.y + s * 0.4));
            ls(
                Pos2::new(c.x, c.y + s * 0.4),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x, c.y + s * 0.4),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.1),
            );
        }
        "chevron-left" => {
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.4),
                Pos2::new(c.x - s * 0.2, c.y),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.4),
            );
        }
        "chevron-right" => {
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.2, c.y),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y),
                Pos2::new(c.x - s * 0.2, c.y + s * 0.4),
            );
        }
        "chevron-up" => {
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.2),
                Pos2::new(c.x, c.y - s * 0.2),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.2),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.2),
            );
        }
        "chevron-down" => {
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.2),
                Pos2::new(c.x, c.y + s * 0.2),
            );
            ls(
                Pos2::new(c.x, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.2),
            );
        }
        "home" => {
            ls(
                Pos2::new(c.x - s * 0.45, c.y),
                Pos2::new(c.x, c.y - s * 0.45),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.45),
                Pos2::new(c.x + s * 0.45, c.y),
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.3, c.y),
                    Pos2::new(c.x + s * 0.3, c.y + s * 0.4),
                ),
                0.0,
                st,
                egui::StrokeKind::Middle,
            );
            ls(Pos2::new(c.x, c.y + s * 0.4), Pos2::new(c.x, c.y + s * 0.1));
        }
        "external-link" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y - s * 0.2),
                    Pos2::new(c.x + s * 0.2, c.y + s * 0.4),
                ),
                s * 0.05,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x + s * 0.45, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.45, c.y - s * 0.4),
                Pos2::new(c.x, c.y + s * 0.05),
            );
        }

        // ── Action ──────────────────────────────────────────────────────────
        "plus" => {
            ls(Pos2::new(c.x, c.y - s * 0.4), Pos2::new(c.x, c.y + s * 0.4));
            ls(Pos2::new(c.x - s * 0.4, c.y), Pos2::new(c.x + s * 0.4, c.y));
        }
        "minus" => {
            ls(Pos2::new(c.x - s * 0.4, c.y), Pos2::new(c.x + s * 0.4, c.y));
        }
        "check" => {
            ls(
                Pos2::new(c.x - s * 0.35, c.y),
                Pos2::new(c.x - s * 0.1, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y + s * 0.3),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.3),
            );
        }
        "x-mark" => {
            ls(
                Pos2::new(c.x - s * 0.3, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.3),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.3),
            );
        }
        "refresh" => {
            painter.circle_stroke(c, s * 0.35, st);
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.55, c.y - s * 0.15),
            );
        }
        "sync" => {
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y + s * 0.05),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.4, c.y + s * 0.15),
                Pos2::new(c.x - s * 0.4, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.05),
                Pos2::new(c.x - s * 0.4, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.35),
                Pos2::new(c.x - s * 0.4, c.y + s * 0.15),
            );
        }
        "download" => {
            ls(Pos2::new(c.x, c.y - s * 0.4), Pos2::new(c.x, c.y + s * 0.2));
            ls(
                Pos2::new(c.x, c.y + s * 0.2),
                Pos2::new(c.x - s * 0.25, c.y - s * 0.05),
            );
            ls(
                Pos2::new(c.x, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.25, c.y - s * 0.05),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.4),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.4),
            );
        }
        "upload" => {
            ls(Pos2::new(c.x, c.y + s * 0.2), Pos2::new(c.x, c.y - s * 0.4));
            ls(
                Pos2::new(c.x, c.y - s * 0.4),
                Pos2::new(c.x - s * 0.25, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.25, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.4),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.4),
            );
        }
        "share" => {
            painter.circle_stroke(Pos2::new(c.x + s * 0.3, c.y - s * 0.3), s * 0.15, st);
            painter.circle_stroke(Pos2::new(c.x - s * 0.3, c.y), s * 0.15, st);
            painter.circle_stroke(Pos2::new(c.x + s * 0.3, c.y + s * 0.3), s * 0.15, st);
            ls(
                Pos2::new(c.x - s * 0.15, c.y - s * 0.08),
                Pos2::new(c.x + s * 0.15, c.y - s * 0.22),
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.08),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.22),
            );
        }
        "export" => {
            painter.rect_stroke(
                Rect::from_center_size(c + Vec2::new(0.0, s * 0.1), Vec2::new(s * 0.7, s * 0.6)),
                s * 0.05,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.5),
                Pos2::new(c.x, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.5),
                Pos2::new(c.x - s * 0.2, c.y - s * 0.3),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.5),
                Pos2::new(c.x + s * 0.2, c.y - s * 0.3),
            );
        }
        "import" => {
            painter.rect_stroke(
                Rect::from_center_size(c + Vec2::new(0.0, s * 0.1), Vec2::new(s * 0.7, s * 0.6)),
                s * 0.05,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x, c.y + s * 0.05),
                Pos2::new(c.x, c.y - s * 0.5),
            );
            ls(
                Pos2::new(c.x, c.y + s * 0.05),
                Pos2::new(c.x - s * 0.2, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x, c.y + s * 0.05),
                Pos2::new(c.x + s * 0.2, c.y - s * 0.15),
            );
        }
        "link" => {
            painter.circle_stroke(Pos2::new(c.x - s * 0.2, c.y - s * 0.15), s * 0.2, st);
            painter.circle_stroke(Pos2::new(c.x + s * 0.2, c.y + s * 0.15), s * 0.2, st);
            ls(
                Pos2::new(c.x - s * 0.05, c.y - s * 0.05),
                Pos2::new(c.x + s * 0.05, c.y + s * 0.05),
            );
        }

        // ── UI/View ─────────────────────────────────────────────────────────
        "eye" => {
            let pts: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let t = i as f32 / 16.0;
                    let x = c.x + s * 0.5 * (t * 2.0 - 1.0);
                    let y = c.y - s * 0.25 * (1.0 - (t * 2.0 - 1.0).powi(2));
                    Pos2::new(x, y)
                })
                .collect();
            for w in pts.windows(2) {
                ls(w[0], w[1]);
            }
            let pts2: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let t = i as f32 / 16.0;
                    let x = c.x + s * 0.5 * (t * 2.0 - 1.0);
                    let y = c.y + s * 0.25 * (1.0 - (t * 2.0 - 1.0).powi(2));
                    Pos2::new(x, y)
                })
                .collect();
            for w in pts2.windows(2) {
                ls(w[0], w[1]);
            }
            painter.circle_stroke(c, s * 0.12, st);
        }
        "eye-off" => {
            draw_menu_icon(painter, rect, "eye", color);
            lsh(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.4),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.4),
            );
        }
        "magnifier" => {
            painter.circle_stroke(Pos2::new(c.x - s * 0.08, c.y - s * 0.08), s * 0.3, st);
            lsh(
                Pos2::new(c.x + s * 0.13, c.y + s * 0.13),
                Pos2::new(c.x + s * 0.45, c.y + s * 0.45),
            );
        }
        "zoom-in" => {
            draw_menu_icon(painter, rect, "magnifier", color);
            ls(
                Pos2::new(c.x - s * 0.08, c.y - s * 0.22),
                Pos2::new(c.x - s * 0.08, c.y + s * 0.06),
            );
            ls(
                Pos2::new(c.x - s * 0.22, c.y - s * 0.08),
                Pos2::new(c.x + s * 0.06, c.y - s * 0.08),
            );
        }
        "zoom-out" => {
            draw_menu_icon(painter, rect, "magnifier", color);
            ls(
                Pos2::new(c.x - s * 0.22, c.y - s * 0.08),
                Pos2::new(c.x + s * 0.06, c.y - s * 0.08),
            );
        }
        "fullscreen" => {
            // top-left corner
            ls(
                Pos2::new(c.x - s * 0.45, c.y - s * 0.25),
                Pos2::new(c.x - s * 0.45, c.y - s * 0.45),
            );
            ls(
                Pos2::new(c.x - s * 0.45, c.y - s * 0.45),
                Pos2::new(c.x - s * 0.25, c.y - s * 0.45),
            );
            // top-right
            ls(
                Pos2::new(c.x + s * 0.25, c.y - s * 0.45),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.45),
            );
            ls(
                Pos2::new(c.x + s * 0.45, c.y - s * 0.45),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.25),
            );
            // bottom-left
            ls(
                Pos2::new(c.x - s * 0.45, c.y + s * 0.25),
                Pos2::new(c.x - s * 0.45, c.y + s * 0.45),
            );
            ls(
                Pos2::new(c.x - s * 0.45, c.y + s * 0.45),
                Pos2::new(c.x - s * 0.25, c.y + s * 0.45),
            );
            // bottom-right
            ls(
                Pos2::new(c.x + s * 0.25, c.y + s * 0.45),
                Pos2::new(c.x + s * 0.45, c.y + s * 0.45),
            );
            ls(
                Pos2::new(c.x + s * 0.45, c.y + s * 0.45),
                Pos2::new(c.x + s * 0.45, c.y + s * 0.25),
            );
        }
        "collapse" => {
            ls(
                Pos2::new(c.x - s * 0.3, c.y - s * 0.15),
                Pos2::new(c.x, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.3, c.y - s * 0.15),
            );
        }
        "expand" => {
            ls(
                Pos2::new(c.x - s * 0.3, c.y + s * 0.15),
                Pos2::new(c.x, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.15),
            );
        }
        "grid-view" => {
            for i in 0..2 {
                for j in 0..2 {
                    let x = c.x - s * 0.35 + j as f32 * s * 0.4;
                    let y = c.y - s * 0.35 + i as f32 * s * 0.4;
                    painter.rect_stroke(
                        Rect::from_min_size(Pos2::new(x, y), Vec2::splat(s * 0.3)),
                        s * 0.03,
                        st,
                        egui::StrokeKind::Middle,
                    );
                }
            }
        }
        "list-view" => {
            for i in 0..3 {
                let y = c.y - s * 0.3 + i as f32 * s * 0.3;
                painter.circle_filled(Pos2::new(c.x - s * 0.35, y), s * 0.06, color);
                ls(Pos2::new(c.x - s * 0.2, y), Pos2::new(c.x + s * 0.4, y));
            }
        }

        // ── Communication ───────────────────────────────────────────────────
        "mail" => {
            let r = Rect::from_center_size(c, Vec2::new(s * 0.9, s * 0.6));
            painter.rect_stroke(r, s * 0.05, st, egui::StrokeKind::Middle);
            ls(r.left_top(), c + Vec2::new(0.0, -s * 0.05));
            ls(c + Vec2::new(0.0, -s * 0.05), r.right_top());
        }
        "mail-open" => {
            let r = Rect::from_min_max(
                Pos2::new(c.x - s * 0.45, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.45, c.y + s * 0.4),
            );
            painter.rect_stroke(r, s * 0.05, st, egui::StrokeKind::Middle);
            ls(
                Pos2::new(c.x - s * 0.45, c.y - s * 0.1),
                Pos2::new(c.x, c.y - s * 0.45),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.45),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.1),
            );
        }
        "send" => {
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.4, c.y),
            );
            ls(
                Pos2::new(c.x + s * 0.4, c.y),
                Pos2::new(c.x - s * 0.4, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.35),
                Pos2::new(c.x - s * 0.1, c.y),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y),
                Pos2::new(c.x - s * 0.4, c.y - s * 0.35),
            );
        }
        "inbox" => {
            let r =
                Rect::from_center_size(c + Vec2::new(0.0, s * 0.1), Vec2::new(s * 0.9, s * 0.6));
            painter.rect_stroke(r, s * 0.05, st, egui::StrokeKind::Middle);
            ls(
                Pos2::new(c.x - s * 0.45, c.y),
                Pos2::new(c.x - s * 0.15, c.y),
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y),
                Pos2::new(c.x - s * 0.15, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.15, c.y),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y),
                Pos2::new(c.x + s * 0.45, c.y),
            );
        }
        "chat" => {
            painter.rect_stroke(
                Rect::from_center_size(c + Vec2::new(0.0, -s * 0.1), Vec2::new(s * 0.8, s * 0.55)),
                s * 0.1,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y + s * 0.18),
                Pos2::new(c.x - s * 0.25, c.y + s * 0.45),
            );
            ls(
                Pos2::new(c.x - s * 0.25, c.y + s * 0.45),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.18),
            );
        }
        "phone" => {
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::new(s * 0.45, s * 0.85)),
                s * 0.1,
                st,
                egui::StrokeKind::Middle,
            );
            painter.circle_filled(Pos2::new(c.x, c.y + s * 0.3), s * 0.06, color);
        }
        "video" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.45, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.1, c.y + s * 0.25),
                ),
                s * 0.05,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.45, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.45, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.45, c.y + s * 0.3),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.15),
            );
        }
        "bell" => {
            painter.circle_stroke(Pos2::new(c.x, c.y - s * 0.45), s * 0.06, st);
            ls(
                Pos2::new(c.x - s * 0.35, c.y + s * 0.15),
                Pos2::new(c.x - s * 0.3, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.3, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y + s * 0.3),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.3),
            );
        }
        "bell-off" => {
            draw_menu_icon(painter, rect, "bell", color);
            lsh(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.4),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.4),
            );
        }
        "at-sign" => {
            painter.circle_stroke(c, s * 0.35, st);
            painter.circle_stroke(c, s * 0.15, st);
            ls(
                Pos2::new(c.x + s * 0.15, c.y),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.15),
            );
        }

        // ── Social ──────────────────────────────────────────────────────────
        "heart" => {
            let pts = [
                Pos2::new(c.x, c.y + s * 0.4),
                Pos2::new(c.x - s * 0.45, c.y - s * 0.05),
                Pos2::new(c.x - s * 0.35, c.y - s * 0.35),
                Pos2::new(c.x, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.05),
            ];
            for i in 0..pts.len() {
                ls(pts[i], pts[(i + 1) % pts.len()]);
            }
        }
        "star" => {
            let n = 5;
            let pts: Vec<Pos2> = (0..n * 2)
                .map(|i| {
                    let angle = std::f32::consts::FRAC_PI_2 * -1.0
                        + i as f32 * std::f32::consts::PI / n as f32;
                    let r = if i % 2 == 0 { s * 0.45 } else { s * 0.2 };
                    Pos2::new(c.x + r * angle.cos(), c.y + r * angle.sin())
                })
                .collect();
            for i in 0..pts.len() {
                ls(pts[i], pts[(i + 1) % pts.len()]);
            }
        }
        "thumbs-up" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.45, c.y - s * 0.1),
                    Pos2::new(c.x - s * 0.2, c.y + s * 0.4),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.05, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.15, c.y - s * 0.4),
            );
        }
        "thumbs-down" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.45, c.y - s * 0.4),
                    Pos2::new(c.x - s * 0.2, c.y + s * 0.1),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.05, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.4),
            );
        }
        "bookmark" => {
            let pts = [
                Pos2::new(c.x - s * 0.25, c.y - s * 0.45),
                Pos2::new(c.x + s * 0.25, c.y - s * 0.45),
                Pos2::new(c.x + s * 0.25, c.y + s * 0.45),
                Pos2::new(c.x, c.y + s * 0.15),
                Pos2::new(c.x - s * 0.25, c.y + s * 0.45),
            ];
            for i in 0..pts.len() {
                ls(pts[i], pts[(i + 1) % pts.len()]);
            }
        }
        "flag" => {
            ls(
                Pos2::new(c.x - s * 0.3, c.y - s * 0.45),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.45),
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y - s * 0.45),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.25),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.25),
                Pos2::new(c.x - s * 0.3, c.y - s * 0.05),
            );
        }

        // ── People/User ─────────────────────────────────────────────────────
        "user" => {
            painter.circle_stroke(Pos2::new(c.x, c.y - s * 0.2), s * 0.22, st);
            let arc_c = Pos2::new(c.x, c.y + s * 0.65);
            painter.circle_stroke(arc_c, s * 0.45, st);
        }
        "users" => {
            draw_menu_icon(
                painter,
                Rect::from_center_size(c + Vec2::new(-s * 0.15, 0.0), Vec2::splat(s * 1.4)),
                "user",
                color,
            );
            painter.circle_stroke(Pos2::new(c.x + s * 0.3, c.y - s * 0.25), s * 0.15, st);
        }
        "user-plus" => {
            draw_menu_icon(
                painter,
                Rect::from_center_size(c + Vec2::new(-s * 0.15, 0.0), Vec2::splat(s * 1.4)),
                "user",
                color,
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.15),
            );
            ls(Pos2::new(c.x + s * 0.2, c.y), Pos2::new(c.x + s * 0.5, c.y));
        }
        "user-minus" => {
            draw_menu_icon(
                painter,
                Rect::from_center_size(c + Vec2::new(-s * 0.15, 0.0), Vec2::splat(s * 1.4)),
                "user",
                color,
            );
            ls(Pos2::new(c.x + s * 0.2, c.y), Pos2::new(c.x + s * 0.5, c.y));
        }
        "user-check" => {
            draw_menu_icon(
                painter,
                Rect::from_center_size(c + Vec2::new(-s * 0.15, 0.0), Vec2::splat(s * 1.4)),
                "user",
                color,
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.12),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.12),
                Pos2::new(c.x + s * 0.5, c.y - s * 0.12),
            );
        }
        "user-circle" => {
            painter.circle_stroke(c, s * 0.45, st);
            painter.circle_stroke(Pos2::new(c.x, c.y - s * 0.12), s * 0.14, st);
            let arc_c = Pos2::new(c.x, c.y + s * 0.5);
            painter.circle_stroke(arc_c, s * 0.3, st);
        }

        // ── Media ───────────────────────────────────────────────────────────
        "play" => {
            let pts = [
                Pos2::new(c.x - s * 0.3, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.4, c.y),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.4),
            ];
            ls(pts[0], pts[1]);
            ls(pts[1], pts[2]);
            ls(pts[2], pts[0]);
        }
        "pause" => {
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.35),
                Pos2::new(c.x - s * 0.2, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.35),
            );
        }
        "stop" => {
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::splat(s * 0.7)),
                s * 0.05,
                st,
                egui::StrokeKind::Middle,
            );
        }
        "skip-forward" => {
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.05, c.y),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y),
                Pos2::new(c.x - s * 0.35, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.3),
            );
        }
        "skip-back" => {
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.3),
                Pos2::new(c.x - s * 0.05, c.y),
            );
            ls(
                Pos2::new(c.x - s * 0.05, c.y),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y - s * 0.3),
                Pos2::new(c.x - s * 0.15, c.y + s * 0.3),
            );
        }
        "volume" => {
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.15),
                Pos2::new(c.x - s * 0.1, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.1, c.y - s * 0.35),
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y + s * 0.35),
                Pos2::new(c.x - s * 0.1, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y + s * 0.15),
                Pos2::new(c.x - s * 0.35, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y + s * 0.15),
                Pos2::new(c.x - s * 0.35, c.y - s * 0.15),
            );
            painter.circle_stroke(Pos2::new(c.x + s * 0.1, c.y), s * 0.25, st);
        }
        "volume-off" => {
            draw_menu_icon(painter, rect, "volume", color);
            lsh(
                Pos2::new(c.x - s * 0.3, c.y + s * 0.35),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.35),
            );
        }
        "music" => {
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.4),
                Pos2::new(c.x - s * 0.2, c.y + s * 0.2),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.25),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.2, c.y - s * 0.25),
            );
            painter.circle_filled(Pos2::new(c.x - s * 0.3, c.y + s * 0.2), s * 0.12, color);
            painter.circle_filled(Pos2::new(c.x + s * 0.1, c.y + s * 0.35), s * 0.12, color);
        }

        // ── Data ────────────────────────────────────────────────────────────
        "database" => {
            for i in 0..3 {
                let y = c.y - s * 0.3 + i as f32 * s * 0.3;
                let ry = s * 0.12;
                painter.circle_stroke(
                    Pos2::new(c.x, y),
                    s * 0.35,
                    Stroke::new(st.width, Color32::TRANSPARENT),
                );
                // Top/bottom ellipse approximation
                ls(Pos2::new(c.x - s * 0.35, y), Pos2::new(c.x + s * 0.35, y));
            }
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.3),
                Pos2::new(c.x - s * 0.35, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.3),
            );
            painter.circle_stroke(Pos2::new(c.x, c.y - s * 0.3), s * 0.35, st);
        }
        "chart-bar" => {
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.4),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.4),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.4),
                Pos2::new(c.x - s * 0.4, c.y - s * 0.4),
            );
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.3, c.y - s * 0.1),
                    Pos2::new(c.x - s * 0.1, c.y + s * 0.4),
                ),
                0.0,
                color,
            );
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.05, c.y - s * 0.3),
                    Pos2::new(c.x + s * 0.15, c.y + s * 0.4),
                ),
                0.0,
                color,
            );
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(c.x + s * 0.2, c.y),
                    Pos2::new(c.x + s * 0.4, c.y + s * 0.4),
                ),
                0.0,
                color,
            );
        }
        "chart-line" => {
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.4),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.4),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.4),
                Pos2::new(c.x - s * 0.4, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y + s * 0.2),
                Pos2::new(c.x - s * 0.05, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.05, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.25),
            );
        }
        "chart-pie" => {
            painter.circle_stroke(c, s * 0.4, st);
            ls(c, Pos2::new(c.x, c.y - s * 0.4));
            ls(c, Pos2::new(c.x + s * 0.35, c.y + s * 0.2));
        }
        "table" => {
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::splat(s * 0.85)),
                s * 0.05,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.42, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.42, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.42, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.42, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y - s * 0.42),
                Pos2::new(c.x - s * 0.1, c.y + s * 0.42),
            );
        }
        "filter" => {
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.35),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.35),
                Pos2::new(c.x - s * 0.05, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.4, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.05, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x - s * 0.05, c.y + s * 0.05),
                Pos2::new(c.x - s * 0.05, c.y + s * 0.4),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y + s * 0.05),
                Pos2::new(c.x + s * 0.05, c.y + s * 0.4),
            );
        }
        "sort-asc" => {
            for i in 0..3 {
                let y = c.y - s * 0.3 + i as f32 * s * 0.3;
                let w = s * 0.2 + i as f32 * s * 0.15;
                ls(
                    Pos2::new(c.x - s * 0.35, y),
                    Pos2::new(c.x - s * 0.35 + w, y),
                );
            }
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.15, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.15),
            );
        }
        "sort-desc" => {
            for i in 0..3 {
                let y = c.y - s * 0.3 + i as f32 * s * 0.3;
                let w = s * 0.5 - i as f32 * s * 0.15;
                ls(
                    Pos2::new(c.x - s * 0.35, y),
                    Pos2::new(c.x - s * 0.35 + w, y),
                );
            }
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.35),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.35),
                Pos2::new(c.x + s * 0.45, c.y + s * 0.15),
            );
        }

        // ── System ──────────────────────────────────────────────────────────
        "gear" => {
            painter.circle_stroke(c, s * 0.2, st);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                let inner = s * 0.28;
                let outer = s * 0.42;
                ls(
                    Pos2::new(c.x + inner * a.cos(), c.y + inner * a.sin()),
                    Pos2::new(c.x + outer * a.cos(), c.y + outer * a.sin()),
                );
            }
        }
        "wrench" => {
            lsh(
                Pos2::new(c.x - s * 0.35, c.y + s * 0.35),
                Pos2::new(c.x + s * 0.1, c.y - s * 0.1),
            );
            painter.circle_stroke(Pos2::new(c.x + s * 0.2, c.y - s * 0.2), s * 0.22, st);
        }
        "shield" => {
            let pts = [
                Pos2::new(c.x, c.y - s * 0.5),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.25),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.15),
                Pos2::new(c.x, c.y + s * 0.5),
                Pos2::new(c.x - s * 0.35, c.y + s * 0.15),
                Pos2::new(c.x - s * 0.4, c.y - s * 0.25),
            ];
            for i in 0..pts.len() {
                ls(pts[i], pts[(i + 1) % pts.len()]);
            }
        }
        "lock" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.3, c.y - s * 0.05),
                    Pos2::new(c.x + s * 0.3, c.y + s * 0.45),
                ),
                s * 0.05,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.05),
                Pos2::new(c.x - s * 0.2, c.y - s * 0.25),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.05),
                Pos2::new(c.x + s * 0.2, c.y - s * 0.25),
            );
            painter.circle_stroke(Pos2::new(c.x, c.y - s * 0.25), s * 0.2, st);
        }
        "unlock" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.3, c.y - s * 0.05),
                    Pos2::new(c.x + s * 0.3, c.y + s * 0.45),
                ),
                s * 0.05,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.05),
                Pos2::new(c.x - s * 0.2, c.y - s * 0.25),
            );
            painter.circle_stroke(Pos2::new(c.x, c.y - s * 0.25), s * 0.2, st);
        }
        "key" => {
            painter.circle_stroke(Pos2::new(c.x - s * 0.2, c.y), s * 0.22, st);
            ls(
                Pos2::new(c.x + s * 0.0, c.y),
                Pos2::new(c.x + s * 0.45, c.y),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.45, c.y),
                Pos2::new(c.x + s * 0.45, c.y + s * 0.15),
            );
        }
        "terminal" => {
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::new(s * 0.9, s * 0.7)),
                s * 0.08,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y - s * 0.1),
                Pos2::new(c.x - s * 0.1, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y + s * 0.1),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.0, c.y + s * 0.3),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.3),
            );
        }
        "code" => {
            ls(
                Pos2::new(c.x - s * 0.15, c.y - s * 0.35),
                Pos2::new(c.x - s * 0.4, c.y),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y),
                Pos2::new(c.x - s * 0.15, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.4, c.y),
            );
            ls(
                Pos2::new(c.x + s * 0.4, c.y),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.35),
            );
        }
        "bug" => {
            painter.circle_stroke(c, s * 0.25, st);
            painter.circle_filled(Pos2::new(c.x, c.y - s * 0.3), s * 0.12, color);
            for i in 0..3 {
                let y = c.y - s * 0.15 + i as f32 * s * 0.15;
                ls(
                    Pos2::new(c.x - s * 0.25, y),
                    Pos2::new(c.x - s * 0.45, y - s * 0.1),
                );
                ls(
                    Pos2::new(c.x + s * 0.25, y),
                    Pos2::new(c.x + s * 0.45, y - s * 0.1),
                );
            }
        }
        "cpu" => {
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::splat(s * 0.55)),
                s * 0.05,
                st,
                egui::StrokeKind::Middle,
            );
            for i in 0..3 {
                let off = -s * 0.15 + i as f32 * s * 0.15;
                ls(
                    Pos2::new(c.x + off, c.y - s * 0.275),
                    Pos2::new(c.x + off, c.y - s * 0.42),
                );
                ls(
                    Pos2::new(c.x + off, c.y + s * 0.275),
                    Pos2::new(c.x + off, c.y + s * 0.42),
                );
                ls(
                    Pos2::new(c.x - s * 0.275, c.y + off),
                    Pos2::new(c.x - s * 0.42, c.y + off),
                );
                ls(
                    Pos2::new(c.x + s * 0.275, c.y + off),
                    Pos2::new(c.x + s * 0.42, c.y + off),
                );
            }
        }

        // ── Status ──────────────────────────────────────────────────────────
        "info-circle" => {
            painter.circle_stroke(c, s * 0.4, st);
            painter.circle_filled(Pos2::new(c.x, c.y - s * 0.2), s * 0.05, color);
            ls(
                Pos2::new(c.x, c.y - s * 0.05),
                Pos2::new(c.x, c.y + s * 0.25),
            );
        }
        "warning-triangle" => {
            let pts = [
                Pos2::new(c.x, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.42, c.y + s * 0.35),
                Pos2::new(c.x - s * 0.42, c.y + s * 0.35),
            ];
            for i in 0..3 {
                ls(pts[i], pts[(i + 1) % 3]);
            }
            painter.circle_filled(Pos2::new(c.x, c.y + s * 0.2), s * 0.04, color);
            ls(
                Pos2::new(c.x, c.y - s * 0.15),
                Pos2::new(c.x, c.y + s * 0.1),
            );
        }
        "error-circle" => {
            painter.circle_stroke(c, s * 0.4, st);
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.2),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.2),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.2),
                Pos2::new(c.x - s * 0.2, c.y + s * 0.2),
            );
        }
        "help-circle" => {
            painter.circle_stroke(c, s * 0.4, st);
            painter.text(
                c + Vec2::new(0.0, -s * 0.05),
                egui::Align2::CENTER_CENTER,
                "?",
                egui::FontId::proportional(s * 0.6),
                color,
            );
        }
        "check-circle" => {
            painter.circle_stroke(c, s * 0.4, st);
            ls(
                Pos2::new(c.x - s * 0.2, c.y),
                Pos2::new(c.x - s * 0.05, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.05, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.2, c.y - s * 0.15),
            );
        }
        "x-circle" => {
            painter.circle_stroke(c, s * 0.4, st);
            ls(
                Pos2::new(c.x - s * 0.18, c.y - s * 0.18),
                Pos2::new(c.x + s * 0.18, c.y + s * 0.18),
            );
            ls(
                Pos2::new(c.x + s * 0.18, c.y - s * 0.18),
                Pos2::new(c.x - s * 0.18, c.y + s * 0.18),
            );
        }
        "clock" => {
            painter.circle_stroke(c, s * 0.4, st);
            ls(c, Pos2::new(c.x, c.y - s * 0.25));
            ls(c, Pos2::new(c.x + s * 0.2, c.y + s * 0.05));
        }
        "calendar" => {
            painter.rect_stroke(
                Rect::from_center_size(c + Vec2::new(0.0, s * 0.05), Vec2::new(s * 0.8, s * 0.7)),
                s * 0.05,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.12),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.12),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.3),
                Pos2::new(c.x - s * 0.2, c.y - s * 0.45),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.2, c.y - s * 0.45),
            );
        }

        // ── Commerce ────────────────────────────────────────────────────────
        "cart" => {
            ls(
                Pos2::new(c.x - s * 0.45, c.y - s * 0.35),
                Pos2::new(c.x - s * 0.3, c.y - s * 0.35),
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y - s * 0.35),
                Pos2::new(c.x - s * 0.15, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.25),
            );
            ls(
                Pos2::new(c.x + s * 0.4, c.y - s * 0.25),
                Pos2::new(c.x - s * 0.25, c.y - s * 0.25),
            );
            painter.circle_filled(Pos2::new(c.x - s * 0.1, c.y + s * 0.32), s * 0.07, color);
            painter.circle_filled(Pos2::new(c.x + s * 0.25, c.y + s * 0.32), s * 0.07, color);
        }
        "credit-card" => {
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::new(s * 0.9, s * 0.55)),
                s * 0.08,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.45, c.y - s * 0.08),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.08),
            );
        }
        "wallet" => {
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::new(s * 0.85, s * 0.65)),
                s * 0.08,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x + s * 0.15, c.y - s * 0.08),
                    Pos2::new(c.x + s * 0.42, c.y + s * 0.12),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
        }
        "receipt" => {
            let pts = [
                Pos2::new(c.x - s * 0.3, c.y - s * 0.5),
                Pos2::new(c.x + s * 0.3, c.y - s * 0.5),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.4),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.5),
                Pos2::new(c.x, c.y + s * 0.4),
                Pos2::new(c.x - s * 0.15, c.y + s * 0.5),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.4),
            ];
            for i in 0..pts.len() {
                ls(pts[i], pts[(i + 1) % pts.len()]);
            }
            ls(
                Pos2::new(c.x - s * 0.15, c.y - s * 0.2),
                Pos2::new(c.x + s * 0.15, c.y - s * 0.2),
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y),
                Pos2::new(c.x + s * 0.15, c.y),
            );
        }
        "tag" => {
            let pts = [
                Pos2::new(c.x - s * 0.4, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.05, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.4, c.y),
                Pos2::new(c.x + s * 0.05, c.y + s * 0.4),
                Pos2::new(c.x - s * 0.4, c.y + s * 0.4),
            ];
            for i in 0..pts.len() {
                ls(pts[i], pts[(i + 1) % pts.len()]);
            }
            painter.circle_filled(Pos2::new(c.x - s * 0.2, c.y), s * 0.06, color);
        }
        "percent" => {
            painter.circle_stroke(Pos2::new(c.x - s * 0.2, c.y - s * 0.2), s * 0.12, st);
            painter.circle_stroke(Pos2::new(c.x + s * 0.2, c.y + s * 0.2), s * 0.12, st);
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.3),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.3),
            );
        }

        // ── File/Folder ─────────────────────────────────────────────────────
        "folder" => {
            let pts = [
                Pos2::new(c.x - s * 0.45, c.y - s * 0.25),
                Pos2::new(c.x - s * 0.1, c.y - s * 0.25),
                Pos2::new(c.x, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.45, c.y + s * 0.3),
                Pos2::new(c.x - s * 0.45, c.y + s * 0.3),
            ];
            for i in 0..pts.len() {
                ls(pts[i], pts[(i + 1) % pts.len()]);
            }
        }
        "folder-open" => {
            draw_menu_icon(painter, rect, "folder", color);
            ls(
                Pos2::new(c.x - s * 0.45, c.y + s * 0.3),
                Pos2::new(c.x - s * 0.3, c.y),
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y),
                Pos2::new(c.x + s * 0.55, c.y),
            );
            ls(
                Pos2::new(c.x + s * 0.55, c.y),
                Pos2::new(c.x + s * 0.45, c.y + s * 0.3),
            );
        }
        "folder-plus" => {
            draw_menu_icon(painter, rect, "folder", color);
            ls(Pos2::new(c.x, c.y - s * 0.1), Pos2::new(c.x, c.y + s * 0.2));
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.05),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.05),
            );
        }
        "archive" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.45, c.y - s * 0.4),
                    Pos2::new(c.x + s * 0.45, c.y - s * 0.15),
                ),
                s * 0.05,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y - s * 0.15),
                    Pos2::new(c.x + s * 0.4, c.y + s * 0.4),
                ),
                s * 0.05,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y + s * 0.05),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.05),
            );
        }
        "trash" => {
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.25),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.25),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y - s * 0.25),
                Pos2::new(c.x - s * 0.1, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y - s * 0.25),
                Pos2::new(c.x + s * 0.1, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.1, c.y - s * 0.4),
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.28, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.28, c.y + s * 0.45),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            ls(Pos2::new(c.x, c.y - s * 0.1), Pos2::new(c.x, c.y + s * 0.3));
        }
        "printer" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.25, c.y - s * 0.45),
                    Pos2::new(c.x + s * 0.25, c.y - s * 0.15),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y - s * 0.15),
                    Pos2::new(c.x + s * 0.4, c.y + s * 0.15),
                ),
                s * 0.05,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.25, c.y + s * 0.15),
                    Pos2::new(c.x + s * 0.25, c.y + s * 0.42),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
        }

        // ── Payroll ────────────────────────────────────────────────────────
        "payroll-check" => {
            // Check/cheque with dollar sign
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.5, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.5, c.y + s * 0.25),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y + s * 0.1),
                Pos2::new(c.x - s * 0.05, c.y + s * 0.1),
            );
            painter.text(
                Pos2::new(c.x + s * 0.25, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.35),
                color,
            );
        }
        "payroll-schedule" => {
            // Calendar with clock
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y - s * 0.35),
                    Pos2::new(c.x + s * 0.4, c.y + s * 0.45),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.15),
            );
            painter.circle_stroke(Pos2::new(c.x, c.y + s * 0.15), s * 0.2, st);
            ls(
                Pos2::new(c.x, c.y + s * 0.15),
                Pos2::new(c.x, c.y + s * 0.02),
            );
            ls(
                Pos2::new(c.x, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.12, c.y + s * 0.15),
            );
        }
        "payroll-deduction" => {
            // Dollar with minus
            painter.text(
                Pos2::new(c.x - s * 0.15, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.55),
                color,
            );
            lsh(
                Pos2::new(c.x + s * 0.1, c.y),
                Pos2::new(c.x + s * 0.45, c.y),
            );
        }
        "payroll-bonus" => {
            // Dollar with plus/star
            painter.text(
                Pos2::new(c.x - s * 0.15, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.55),
                color,
            );
            painter.circle_stroke(Pos2::new(c.x + s * 0.3, c.y - s * 0.25), s * 0.18, st);
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.3, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.25),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.25),
            );
        }
        "payroll-overtime" => {
            // Clock with extra arc
            painter.circle_stroke(c, s * 0.35, st);
            ls(Pos2::new(c.x, c.y), Pos2::new(c.x, c.y - s * 0.25));
            ls(Pos2::new(c.x, c.y), Pos2::new(c.x + s * 0.2, c.y + s * 0.1));
            lsh(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.2),
            );
        }
        "payroll-tax" => {
            // Dollar with percent
            painter.text(
                Pos2::new(c.x - s * 0.2, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.45),
                color,
            );
            painter.text(
                Pos2::new(c.x + s * 0.25, c.y),
                egui::Align2::CENTER_CENTER,
                "%",
                egui::FontId::proportional(s * 0.35),
                color,
            );
        }
        "payroll-slip" => {
            // Document with lines and dollar
            icon_doc_outline(painter, c, s, st);
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.2),
                Pos2::new(c.x + s * 0.15, c.y - s * 0.2),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y),
                Pos2::new(c.x + s * 0.15, c.y),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.2),
            );
            painter.text(
                Pos2::new(c.x, c.y + s * 0.4),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.2),
                color,
            );
        }
        "payroll-direct-deposit" => {
            // Bank building with arrow down
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.15),
                Pos2::new(c.x, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y - s * 0.15),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.25),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.25),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.25),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.25),
            );
            ls(
                Pos2::new(c.x, c.y + s * 0.05),
                Pos2::new(c.x, c.y + s * 0.45),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y + s * 0.35),
                Pos2::new(c.x, c.y + s * 0.45),
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y + s * 0.35),
                Pos2::new(c.x, c.y + s * 0.45),
            );
        }
        "payroll-timesheet" => {
            // Grid/table with clock
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y - s * 0.4),
                    Pos2::new(c.x + s * 0.4, c.y + s * 0.4),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y - s * 0.4),
                Pos2::new(c.x - s * 0.1, c.y + s * 0.4),
            );
            painter.circle_stroke(Pos2::new(c.x + s * 0.3, c.y - s * 0.3), s * 0.12, st);
        }
        "payroll-hours" => {
            // Clock face
            painter.circle_stroke(c, s * 0.4, st);
            lsh(Pos2::new(c.x, c.y), Pos2::new(c.x, c.y - s * 0.28));
            lsh(
                Pos2::new(c.x, c.y),
                Pos2::new(c.x + s * 0.22, c.y + s * 0.05),
            );
            painter.circle_filled(c, s * 0.05, color);
        }
        "payroll-employee" => {
            // Person with dollar
            painter.circle_stroke(Pos2::new(c.x - s * 0.15, c.y - s * 0.25), s * 0.18, st);
            ls(
                Pos2::new(c.x - s * 0.15, c.y - s * 0.07),
                Pos2::new(c.x - s * 0.15, c.y + s * 0.25),
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y + s * 0.05),
                Pos2::new(c.x + s * 0.05, c.y + s * 0.05),
            );
            painter.text(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.1),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.35),
                color,
            );
        }
        "payroll-benefits" => {
            // Heart with plus
            ls(
                Pos2::new(c.x, c.y + s * 0.3),
                Pos2::new(c.x - s * 0.35, c.y - s * 0.05),
            );
            ls(
                Pos2::new(c.x, c.y + s * 0.3),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.05),
            );
            painter.circle_stroke(Pos2::new(c.x - s * 0.18, c.y - s * 0.15), s * 0.18, st);
            painter.circle_stroke(Pos2::new(c.x + s * 0.18, c.y - s * 0.15), s * 0.18, st);
        }
        "payroll-pension" => {
            // Umbrella/shield over dollar
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.1),
                Pos2::new(c.x, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.1),
            );
            painter.text(
                Pos2::new(c.x, c.y + s * 0.2),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.4),
                color,
            );
        }
        "payroll-vacation" => {
            // Sun/palm
            painter.circle_stroke(c, s * 0.2, st);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let (sin, cos) = a.sin_cos();
                ls(
                    Pos2::new(c.x + cos * s * 0.25, c.y + sin * s * 0.25),
                    Pos2::new(c.x + cos * s * 0.4, c.y + sin * s * 0.4),
                );
            }
        }
        "payroll-sick-leave" => {
            // Cross/plus medical
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.12, c.y - s * 0.4),
                    Pos2::new(c.x + s * 0.12, c.y + s * 0.4),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y - s * 0.12),
                    Pos2::new(c.x + s * 0.4, c.y + s * 0.12),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
        }
        "payroll-commission" => {
            // Percent with arrow up
            painter.text(
                Pos2::new(c.x - s * 0.15, c.y),
                egui::Align2::CENTER_CENTER,
                "%",
                egui::FontId::proportional(s * 0.45),
                color,
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.3),
                Pos2::new(c.x + s * 0.3, c.y - s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.2),
                Pos2::new(c.x + s * 0.3, c.y - s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.4, c.y - s * 0.2),
                Pos2::new(c.x + s * 0.3, c.y - s * 0.3),
            );
        }
        "payroll-garnishment" => {
            // Dollar with lock
            painter.text(
                Pos2::new(c.x - s * 0.2, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.45),
                color,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x + s * 0.1, c.y),
                    Pos2::new(c.x + s * 0.45, c.y + s * 0.3),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            painter.circle_stroke(Pos2::new(c.x + s * 0.275, c.y - s * 0.05), s * 0.1, st);
        }
        "payroll-reimbursement" => {
            // Dollar with return arrow
            painter.text(
                Pos2::new(c.x, c.y - s * 0.1),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.4),
                color,
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.2),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.2),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.1),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.2),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.3),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.2),
            );
        }
        "payroll-w2" => {
            icon_doc_outline(painter, c, s, st);
            painter.text(
                Pos2::new(c.x, c.y),
                egui::Align2::CENTER_CENTER,
                "W2",
                egui::FontId::proportional(s * 0.3),
                color,
            );
        }
        "payroll-1099" => {
            icon_doc_outline(painter, c, s, st);
            painter.text(
                Pos2::new(c.x, c.y),
                egui::Align2::CENTER_CENTER,
                "1099",
                egui::FontId::proportional(s * 0.22),
                color,
            );
        }
        "payroll-ytd" => {
            // Chart trending up
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y - s * 0.35),
                    Pos2::new(c.x + s * 0.4, c.y + s * 0.35),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y + s * 0.2),
                Pos2::new(c.x - s * 0.1, c.y),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y),
                Pos2::new(c.x + s * 0.05, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.3, c.y - s * 0.2),
            );
        }
        "payroll-net-pay" => {
            // Dollar with down arrow (net = after deductions)
            painter.text(
                Pos2::new(c.x, c.y - s * 0.15),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.4),
                color,
            );
            ls(
                Pos2::new(c.x, c.y + s * 0.05),
                Pos2::new(c.x, c.y + s * 0.4),
            );
            ls(
                Pos2::new(c.x - s * 0.12, c.y + s * 0.28),
                Pos2::new(c.x, c.y + s * 0.4),
            );
            ls(
                Pos2::new(c.x + s * 0.12, c.y + s * 0.28),
                Pos2::new(c.x, c.y + s * 0.4),
            );
        }
        "payroll-gross-pay" => {
            // Dollar with up arrow (gross = total)
            painter.text(
                Pos2::new(c.x, c.y + s * 0.15),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.4),
                color,
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.05),
                Pos2::new(c.x, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x - s * 0.12, c.y - s * 0.28),
                Pos2::new(c.x, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x + s * 0.12, c.y - s * 0.28),
                Pos2::new(c.x, c.y - s * 0.4),
            );
        }
        "payroll-withholding" => {
            // Dollar with hand/stop
            painter.text(
                Pos2::new(c.x - s * 0.15, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.45),
                color,
            );
            painter.circle_stroke(Pos2::new(c.x + s * 0.25, c.y), s * 0.2, st);
            ls(
                Pos2::new(c.x + s * 0.11, c.y - s * 0.14),
                Pos2::new(c.x + s * 0.39, c.y + s * 0.14),
            );
        }
        "payroll-frequency" => {
            // Calendar with repeating arrows
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.35),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.35),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.05),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y - s * 0.02),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y + s * 0.12),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.05),
            );
        }

        // ── Receivables ────────────────────────────────────────────────────
        "invoice" => {
            icon_doc_outline(painter, c, s, st);
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.15, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.05),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.25),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.25),
            );
        }
        "invoice-paid" => {
            icon_doc_outline(painter, c, s, st);
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.1),
                Pos2::new(c.x - s * 0.05, c.y + s * 0.2),
            );
            lsh(
                Pos2::new(c.x - s * 0.05, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.2, c.y - s * 0.05),
            );
        }
        "invoice-overdue" => {
            icon_doc_outline(painter, c, s, st);
            painter.text(
                Pos2::new(c.x, c.y + s * 0.1),
                egui::Align2::CENTER_CENTER,
                "!",
                egui::FontId::proportional(s * 0.4),
                color,
            );
        }
        "invoice-draft" => {
            icon_doc_outline(painter, c, s, st);
            ls(
                Pos2::new(c.x - s * 0.15, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.1, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.05, c.y + s * 0.1),
            );
        }
        "invoice-send" => {
            icon_doc_outline(painter, c, s, st);
            ls(
                Pos2::new(c.x, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.25, c.y),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.25, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.1),
            );
        }
        "credit-memo" => {
            icon_doc_outline(painter, c, s, st);
            painter.text(
                Pos2::new(c.x, c.y + s * 0.05),
                egui::Align2::CENTER_CENTER,
                "CR",
                egui::FontId::proportional(s * 0.25),
                color,
            );
        }
        "debit-memo" => {
            icon_doc_outline(painter, c, s, st);
            painter.text(
                Pos2::new(c.x, c.y + s * 0.05),
                egui::Align2::CENTER_CENTER,
                "DR",
                egui::FontId::proportional(s * 0.25),
                color,
            );
        }
        "aging-report" => {
            // Bar chart descending
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.35),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.35),
                Pos2::new(c.x - s * 0.4, c.y - s * 0.35),
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.3, c.y - s * 0.25),
                    Pos2::new(c.x - s * 0.1, c.y + s * 0.35),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.05, c.y - s * 0.05),
                    Pos2::new(c.x + s * 0.15, c.y + s * 0.35),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x + s * 0.2, c.y + s * 0.1),
                    Pos2::new(c.x + s * 0.4, c.y + s * 0.35),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
        }
        "collection" => {
            // Hand reaching for dollar
            painter.text(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.15),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.35),
                color,
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.05),
            );
        }
        "dunning-letter" => {
            // Envelope with exclamation
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y - s * 0.2),
                    Pos2::new(c.x + s * 0.4, c.y + s * 0.25),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.2),
                Pos2::new(c.x, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.4, c.y - s * 0.2),
                Pos2::new(c.x, c.y + s * 0.05),
            );
            painter.text(
                Pos2::new(c.x, c.y - s * 0.35),
                egui::Align2::CENTER_CENTER,
                "!",
                egui::FontId::proportional(s * 0.25),
                color,
            );
        }
        "payment-received" => {
            // Dollar with check
            painter.text(
                Pos2::new(c.x - s * 0.15, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.5),
                color,
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.2),
            );
            lsh(
                Pos2::new(c.x + s * 0.2, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.05),
            );
        }
        "partial-payment" => {
            // Half-filled coin
            painter.circle_stroke(c, s * 0.35, st);
            ls(
                Pos2::new(c.x, c.y - s * 0.35),
                Pos2::new(c.x, c.y + s * 0.35),
            );
            painter.text(
                Pos2::new(c.x - s * 0.1, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.3),
                color,
            );
        }
        "advance-payment" => {
            // Dollar with fast-forward arrows
            painter.text(
                Pos2::new(c.x - s * 0.2, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.45),
                color,
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.25, c.y),
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.25, c.y),
            );
            ls(
                Pos2::new(c.x + s * 0.25, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.4, c.y),
            );
            ls(
                Pos2::new(c.x + s * 0.25, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.4, c.y),
            );
        }
        "refund" => {
            // Dollar with curved return arrow
            painter.text(
                Pos2::new(c.x, c.y - s * 0.1),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.4),
                color,
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.15),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y + s * 0.15),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.05),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.15),
            );
        }
        "write-off" => {
            // Dollar with X
            painter.text(
                Pos2::new(c.x - s * 0.2, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.4),
                color,
            );
            lsh(
                Pos2::new(c.x + s * 0.1, c.y - s * 0.2),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.2),
            );
            lsh(
                Pos2::new(c.x + s * 0.4, c.y - s * 0.2),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.2),
            );
        }
        "bad-debt" => {
            // Dollar with warning triangle
            painter.text(
                Pos2::new(c.x - s * 0.2, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.4),
                color,
            );
            ls(
                Pos2::new(c.x + s * 0.25, c.y - s * 0.25),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.2),
            );
            ls(
                Pos2::new(c.x + s * 0.25, c.y - s * 0.25),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.2),
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.2),
            );
        }
        "interest-charge" => {
            // Percent with dollar
            painter.text(
                Pos2::new(c.x - s * 0.15, c.y),
                egui::Align2::CENTER_CENTER,
                "%",
                egui::FontId::proportional(s * 0.45),
                color,
            );
            painter.text(
                Pos2::new(c.x + s * 0.3, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.3),
                color,
            );
        }
        "statement" => {
            // Document with multiple lines
            icon_doc_outline(painter, c, s, st);
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.25),
                Pos2::new(c.x + s * 0.15, c.y - s * 0.25),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.15, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.05),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.2),
            );
            lsh(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.35),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.35),
            );
        }
        "customer-balance" => {
            // Person with balance scale
            painter.circle_stroke(Pos2::new(c.x, c.y - s * 0.3), s * 0.15, st);
            ls(
                Pos2::new(c.x, c.y - s * 0.15),
                Pos2::new(c.x, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y + s * 0.1),
                Pos2::new(c.x - s * 0.25, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.25, c.y + s * 0.3),
            );
        }
        "account-receivable" => {
            // Ledger book with arrow in
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.4),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.4),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.4),
                Pos2::new(c.x - s * 0.35, c.y + s * 0.4),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.2, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.05),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.25, c.y + s * 0.2),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y + s * 0.14),
                Pos2::new(c.x + s * 0.25, c.y + s * 0.2),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y + s * 0.26),
                Pos2::new(c.x + s * 0.25, c.y + s * 0.2),
            );
        }
        "open-items" => {
            // List with open circles
            for i in 0..3 {
                let y = c.y - s * 0.2 + i as f32 * s * 0.2;
                painter.circle_stroke(Pos2::new(c.x - s * 0.25, y), s * 0.07, st);
                ls(Pos2::new(c.x - s * 0.1, y), Pos2::new(c.x + s * 0.35, y));
            }
        }
        "clearing" => {
            // Two arrows meeting
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.05, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.05, c.y - s * 0.2),
                Pos2::new(c.x + s * 0.05, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.05, c.y),
                Pos2::new(c.x + s * 0.05, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.4, c.y + s * 0.1),
                Pos2::new(c.x - s * 0.05, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y),
                Pos2::new(c.x - s * 0.05, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y + s * 0.2),
                Pos2::new(c.x - s * 0.05, c.y + s * 0.1),
            );
        }
        "remittance" => {
            // Envelope with dollar
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y - s * 0.2),
                    Pos2::new(c.x + s * 0.4, c.y + s * 0.25),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.2),
                Pos2::new(c.x, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.4, c.y - s * 0.2),
                Pos2::new(c.x, c.y + s * 0.05),
            );
            painter.text(
                Pos2::new(c.x, c.y - s * 0.35),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.25),
                color,
            );
        }
        "factoring" => {
            // Document with scissors
            icon_doc_outline(painter, c, s, st);
            ls(
                Pos2::new(c.x - s * 0.35, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.15),
            );
            painter.circle_stroke(Pos2::new(c.x + s * 0.25, c.y + s * 0.25), s * 0.08, st);
            painter.circle_stroke(Pos2::new(c.x + s * 0.25, c.y + s * 0.08), s * 0.08, st);
        }
        "credit-limit" => {
            // Dollar with ceiling line
            painter.text(
                Pos2::new(c.x, c.y + s * 0.05),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.5),
                color,
            );
            lsh(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.35),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.35),
                Pos2::new(c.x, c.y - s * 0.25),
            );
        }

        // ── Payments ───────────────────────────────────────────────────────
        "payment-check" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.5, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.5, c.y + s * 0.25),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.1),
            );
        }
        "payment-wire" => {
            // Lightning bolt
            ls(
                Pos2::new(c.x + s * 0.1, c.y - s * 0.4),
                Pos2::new(c.x - s * 0.1, c.y),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y),
                Pos2::new(c.x + s * 0.15, c.y),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y),
                Pos2::new(c.x - s * 0.05, c.y + s * 0.4),
            );
        }
        "payment-ach" => {
            // Bank with electronic signal
            ls(
                Pos2::new(c.x - s * 0.35, c.y),
                Pos2::new(c.x, c.y - s * 0.3),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.35, c.y),
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y),
                Pos2::new(c.x + s * 0.35, c.y),
            );
            ls(
                Pos2::new(c.x - s * 0.25, c.y),
                Pos2::new(c.x - s * 0.25, c.y + s * 0.2),
            );
            ls(
                Pos2::new(c.x + s * 0.25, c.y),
                Pos2::new(c.x + s * 0.25, c.y + s * 0.2),
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.2),
            );
            painter.circle_stroke(Pos2::new(c.x, c.y + s * 0.35), s * 0.08, st);
        }
        "payment-cash" => {
            // Banknote
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.45, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.45, c.y + s * 0.25),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            painter.circle_stroke(c, s * 0.15, st);
            painter.text(
                Pos2::new(c.x, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.25),
                color,
            );
        }
        "payment-pending" => {
            // Dollar with clock
            painter.text(
                Pos2::new(c.x - s * 0.2, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.45),
                color,
            );
            painter.circle_stroke(Pos2::new(c.x + s * 0.25, c.y), s * 0.2, st);
            ls(
                Pos2::new(c.x + s * 0.25, c.y),
                Pos2::new(c.x + s * 0.25, c.y - s * 0.13),
            );
            ls(
                Pos2::new(c.x + s * 0.25, c.y),
                Pos2::new(c.x + s * 0.35, c.y),
            );
        }
        "payment-approved" => {
            // Dollar with checkmark
            painter.text(
                Pos2::new(c.x - s * 0.15, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.5),
                color,
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.2),
            );
            lsh(
                Pos2::new(c.x + s * 0.2, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.1),
            );
        }
        "payment-rejected" => {
            // Dollar with X
            painter.text(
                Pos2::new(c.x - s * 0.15, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.5),
                color,
            );
            lsh(
                Pos2::new(c.x + s * 0.15, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.15),
            );
            lsh(
                Pos2::new(c.x + s * 0.4, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.15),
            );
        }
        "payment-recurring" => {
            // Dollar with circular arrows
            painter.text(
                Pos2::new(c.x, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.3),
                color,
            );
            painter.circle_stroke(c, s * 0.35, st);
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.05),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.05),
                Pos2::new(c.x + s * 0.3, c.y - s * 0.18),
            );
        }
        "payment-split" => {
            // Dollar splitting into two arrows
            painter.text(
                Pos2::new(c.x - s * 0.15, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.35),
                color,
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y),
                Pos2::new(c.x + s * 0.2, c.y - s * 0.2),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.2),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.2),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.2),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.2),
            );
        }
        "payment-batch" => {
            // Stacked rectangles
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.15),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.25),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.3, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.4, c.y - s * 0.15),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.25, c.y - s * 0.35),
                    Pos2::new(c.x + s * 0.45, c.y - s * 0.25),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            painter.text(
                Pos2::new(c.x, c.y + s * 0.05),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.25),
                color,
            );
        }
        "payment-void" => {
            // Dollar with circle-slash (void)
            painter.circle_stroke(c, s * 0.35, st);
            ls(
                Pos2::new(c.x - s * 0.25, c.y + s * 0.25),
                Pos2::new(c.x + s * 0.25, c.y - s * 0.25),
            );
            painter.text(
                Pos2::new(c.x, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.3),
                color,
            );
        }
        "payment-reversal" => {
            // Dollar with U-turn arrow
            painter.text(
                Pos2::new(c.x, c.y - s * 0.1),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.35),
                color,
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.3),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.2),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.4),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.3),
            );
        }
        "vendor-payment" => {
            // Person with arrow out
            painter.circle_stroke(Pos2::new(c.x - s * 0.2, c.y - s * 0.2), s * 0.15, st);
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.05),
                Pos2::new(c.x - s * 0.2, c.y + s * 0.2),
            );
            ls(
                Pos2::new(c.x, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.25, c.y),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.25, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.1),
            );
        }
        "bill-pay" => {
            // Document with dollar outgoing
            icon_doc_outline(painter, c, s, st);
            painter.text(
                Pos2::new(c.x, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.3),
                color,
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y + s * 0.35),
                Pos2::new(c.x + s * 0.45, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y + s * 0.25),
                Pos2::new(c.x + s * 0.45, c.y + s * 0.35),
            );
        }
        "purchase-order" => {
            // Clipboard with cart
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.35),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.45),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.15, c.y - s * 0.45),
                    Pos2::new(c.x + s * 0.15, c.y - s * 0.35),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y),
                Pos2::new(c.x + s * 0.2, c.y),
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.3),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.3),
            );
        }
        "expense-report" => {
            // Document with chart
            icon_doc_outline(painter, c, s, st);
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.35),
                Pos2::new(c.x - s * 0.2, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.35),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y + s * 0.35),
                Pos2::new(c.x - s * 0.1, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y + s * 0.35),
                Pos2::new(c.x + s * 0.05, c.y + s * 0.05),
            );
        }
        "petty-cash" => {
            // Small box with coins
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.2),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.2),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.05),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.05),
            );
            painter.circle_stroke(Pos2::new(c.x, c.y + s * 0.08), s * 0.08, st);
        }
        "bank-transfer" => {
            // Two banks with arrow
            ls(
                Pos2::new(c.x - s * 0.45, c.y - s * 0.1),
                Pos2::new(c.x - s * 0.25, c.y - s * 0.3),
            );
            ls(
                Pos2::new(c.x - s * 0.25, c.y - s * 0.3),
                Pos2::new(c.x - s * 0.05, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.45, c.y - s * 0.1),
                Pos2::new(c.x - s * 0.05, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.02, c.y + s * 0.02),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.02, c.y + s * 0.18),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.25, c.y - s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.25, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.1),
            );
        }
        "payment-gateway" => {
            // Shield with dollar
            ls(
                Pos2::new(c.x - s * 0.3, c.y - s * 0.35),
                Pos2::new(c.x, c.y - s * 0.45),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.45),
                Pos2::new(c.x + s * 0.3, c.y - s * 0.35),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.1),
                Pos2::new(c.x, c.y + s * 0.4),
            );
            ls(
                Pos2::new(c.x, c.y + s * 0.4),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y + s * 0.1),
                Pos2::new(c.x - s * 0.3, c.y - s * 0.35),
            );
            painter.text(
                Pos2::new(c.x, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.35),
                color,
            );
        }
        "payment-terms" => {
            // Calendar with dollar
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.35),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.35),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.15),
            );
            painter.text(
                Pos2::new(c.x, c.y + s * 0.1),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.35),
                color,
            );
        }
        "early-discount" => {
            // Clock with percent
            painter.circle_stroke(Pos2::new(c.x - s * 0.15, c.y), s * 0.25, st);
            ls(
                Pos2::new(c.x - s * 0.15, c.y),
                Pos2::new(c.x - s * 0.15, c.y - s * 0.18),
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y),
                Pos2::new(c.x - s * 0.02, c.y),
            );
            painter.text(
                Pos2::new(c.x + s * 0.3, c.y),
                egui::Align2::CENTER_CENTER,
                "%",
                egui::FontId::proportional(s * 0.3),
                color,
            );
        }
        "payment-plan" => {
            // Dollar with steps
            painter.text(
                Pos2::new(c.x - s * 0.25, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.35),
                color,
            );
            ls(
                Pos2::new(c.x, c.y + s * 0.3),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y + s * 0.3),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.3, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.1),
            );
        }
        "installment" => {
            // Three dollar signs decreasing
            painter.text(
                Pos2::new(c.x - s * 0.25, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.3),
                color,
            );
            painter.text(
                Pos2::new(c.x, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.3),
                color,
            );
            painter.text(
                Pos2::new(c.x + s * 0.25, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.3),
                color,
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.25),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.25),
            );
        }
        "escrow" => {
            // Lock with dollar inside
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.25, c.y),
                    Pos2::new(c.x + s * 0.25, c.y + s * 0.35),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            painter.circle_stroke(Pos2::new(c.x, c.y - s * 0.1), s * 0.18, st);
            painter.text(
                Pos2::new(c.x, c.y + s * 0.18),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.22),
                color,
            );
        }
        "disbursement" => {
            // Dollar with outward arrows
            painter.text(
                Pos2::new(c.x, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.35),
                color,
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.2),
                Pos2::new(c.x - s * 0.4, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.2),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.2),
                Pos2::new(c.x - s * 0.4, c.y + s * 0.4),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.4),
            );
        }

        // ── Stock Control ──────────────────────────────────────────────────
        "inventory" => {
            // Clipboard with boxes
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.35),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.45),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.15, c.y - s * 0.45),
                    Pos2::new(c.x + s * 0.15, c.y - s * 0.35),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.2, c.y - s * 0.1),
                    Pos2::new(c.x + s * 0.2, c.y + s * 0.1),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.2, c.y + s * 0.15),
                    Pos2::new(c.x + s * 0.2, c.y + s * 0.35),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
        }
        "warehouse" => {
            // Building with door
            ls(
                Pos2::new(c.x - s * 0.45, c.y - s * 0.1),
                Pos2::new(c.x, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.1),
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.45, c.y - s * 0.1),
                    Pos2::new(c.x + s * 0.45, c.y + s * 0.4),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.12, c.y + s * 0.05),
                    Pos2::new(c.x + s * 0.12, c.y + s * 0.4),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
        }
        "stock-in" => {
            // Box with arrow in
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.2),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.35),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.45),
                Pos2::new(c.x, c.y - s * 0.05),
            );
            ls(
                Pos2::new(c.x - s * 0.12, c.y - s * 0.17),
                Pos2::new(c.x, c.y - s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.12, c.y - s * 0.17),
                Pos2::new(c.x, c.y - s * 0.05),
            );
        }
        "stock-out" => {
            // Box with arrow out
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.2),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.35),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.05),
                Pos2::new(c.x, c.y - s * 0.45),
            );
            ls(
                Pos2::new(c.x - s * 0.12, c.y - s * 0.33),
                Pos2::new(c.x, c.y - s * 0.45),
            );
            ls(
                Pos2::new(c.x + s * 0.12, c.y - s * 0.33),
                Pos2::new(c.x, c.y - s * 0.45),
            );
        }
        "stock-count" => {
            // Boxes with numbers
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.15),
                    Pos2::new(c.x + s * 0.1, c.y + s * 0.3),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            painter.text(
                Pos2::new(c.x - s * 0.12, c.y + s * 0.07),
                egui::Align2::CENTER_CENTER,
                "#",
                egui::FontId::proportional(s * 0.3),
                color,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x + s * 0.05, c.y - s * 0.35),
                    Pos2::new(c.x + s * 0.4, c.y),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
        }
        "stock-transfer" => {
            // Two boxes with arrow between
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.45, c.y - s * 0.15),
                    Pos2::new(c.x - s * 0.1, c.y + s * 0.2),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x + s * 0.1, c.y - s * 0.15),
                    Pos2::new(c.x + s * 0.45, c.y + s * 0.2),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.08, c.y + s * 0.025),
                Pos2::new(c.x + s * 0.08, c.y + s * 0.025),
            );
            ls(
                Pos2::new(c.x + s * 0.02, c.y - s * 0.04),
                Pos2::new(c.x + s * 0.08, c.y + s * 0.025),
            );
            ls(
                Pos2::new(c.x + s * 0.02, c.y + s * 0.09),
                Pos2::new(c.x + s * 0.08, c.y + s * 0.025),
            );
        }
        "stock-adjust" => {
            // Box with +/-
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.25),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.05),
                Pos2::new(c.x + s * 0.0, c.y - s * 0.05),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y - s * 0.15),
                Pos2::new(c.x - s * 0.1, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y),
                Pos2::new(c.x + s * 0.25, c.y),
            );
        }
        "stock-reserve" => {
            // Box with lock
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.1),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.35),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.12, c.y + s * 0.05),
                    Pos2::new(c.x + s * 0.12, c.y + s * 0.25),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            painter.circle_stroke(Pos2::new(c.x, c.y - s * 0.05), s * 0.1, st);
        }
        "stock-alert" => {
            // Box with exclamation
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.15),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.3),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            painter.text(
                Pos2::new(c.x, c.y - s * 0.35),
                egui::Align2::CENTER_CENTER,
                "!",
                egui::FontId::proportional(s * 0.35),
                color,
            );
        }
        "stock-reorder" => {
            // Box with circular arrow
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.15),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.35),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            painter.circle_stroke(Pos2::new(c.x, c.y + s * 0.1), s * 0.15, st);
            ls(
                Pos2::new(c.x + s * 0.15, c.y + s * 0.08),
                Pos2::new(c.x + s * 0.22, c.y),
            );
        }
        "barcode" => {
            // Vertical bars of varying width
            let bars = [-0.35f32, -0.25, -0.2, -0.1, 0.0, 0.05, 0.15, 0.2, 0.3, 0.35];
            for &bx in &bars {
                ls(
                    Pos2::new(c.x + s * bx, c.y - s * 0.3),
                    Pos2::new(c.x + s * bx, c.y + s * 0.3),
                );
            }
        }
        "qr-code" => {
            // QR code pattern: three corner squares + dots
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y - s * 0.4),
                    Pos2::new(c.x - s * 0.1, c.y - s * 0.1),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x + s * 0.1, c.y - s * 0.4),
                    Pos2::new(c.x + s * 0.4, c.y - s * 0.1),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y + s * 0.1),
                    Pos2::new(c.x - s * 0.1, c.y + s * 0.4),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            painter.circle_filled(Pos2::new(c.x + s * 0.2, c.y + s * 0.2), s * 0.05, color);
            painter.circle_filled(Pos2::new(c.x + s * 0.35, c.y + s * 0.35), s * 0.05, color);
        }
        "pallet" => {
            // Pallet base with box on top
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.2),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.35),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y + s * 0.2),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x, c.y + s * 0.2),
                Pos2::new(c.x, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.35),
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.3, c.y - s * 0.2),
                    Pos2::new(c.x + s * 0.3, c.y + s * 0.2),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
        }
        "shelf" => {
            // Shelving unit
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.4),
                Pos2::new(c.x - s * 0.35, c.y + s * 0.4),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.4),
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.2),
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.2, c.y - s * 0.35),
                    Pos2::new(c.x + s * 0.05, c.y - s * 0.1),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
        }
        "bin-location" => {
            // Grid cells with marker
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y - s * 0.3),
                    Pos2::new(c.x + s * 0.4, c.y + s * 0.3),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            ls(Pos2::new(c.x, c.y - s * 0.3), Pos2::new(c.x, c.y + s * 0.3));
            ls(Pos2::new(c.x - s * 0.4, c.y), Pos2::new(c.x + s * 0.4, c.y));
            painter.circle_filled(Pos2::new(c.x + s * 0.2, c.y - s * 0.15), s * 0.08, color);
        }
        "lot-number" => {
            // Tag with hash
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.25),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.25),
                Pos2::new(c.x - s * 0.45, c.y),
            );
            painter.text(
                Pos2::new(c.x, c.y),
                egui::Align2::CENTER_CENTER,
                "#",
                egui::FontId::proportional(s * 0.35),
                color,
            );
        }
        "serial-number" => {
            // Tag with S/N
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.2),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.2),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            painter.text(
                Pos2::new(c.x, c.y),
                egui::Align2::CENTER_CENTER,
                "SN",
                egui::FontId::proportional(s * 0.25),
                color,
            );
        }
        "expiry-date" => {
            // Calendar with X
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.3),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.35),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.0),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.25),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y + s * 0.0),
                Pos2::new(c.x - s * 0.15, c.y + s * 0.25),
            );
        }
        "fifo" => {
            // Arrow left-to-right with "1"
            ls(Pos2::new(c.x - s * 0.4, c.y), Pos2::new(c.x + s * 0.4, c.y));
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.4, c.y),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.4, c.y),
            );
            painter.text(
                Pos2::new(c.x - s * 0.25, c.y - s * 0.25),
                egui::Align2::CENTER_CENTER,
                "1",
                egui::FontId::proportional(s * 0.2),
                color,
            );
            painter.text(
                Pos2::new(c.x + s * 0.25, c.y - s * 0.25),
                egui::Align2::CENTER_CENTER,
                "1",
                egui::FontId::proportional(s * 0.2),
                color,
            );
        }
        "lifo" => {
            // Arrow right-to-left with "1"
            ls(Pos2::new(c.x - s * 0.4, c.y), Pos2::new(c.x + s * 0.4, c.y));
            ls(
                Pos2::new(c.x - s * 0.3, c.y - s * 0.1),
                Pos2::new(c.x - s * 0.4, c.y),
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y + s * 0.1),
                Pos2::new(c.x - s * 0.4, c.y),
            );
            painter.text(
                Pos2::new(c.x + s * 0.25, c.y - s * 0.25),
                egui::Align2::CENTER_CENTER,
                "1",
                egui::FontId::proportional(s * 0.2),
                color,
            );
            painter.text(
                Pos2::new(c.x - s * 0.25, c.y - s * 0.25),
                egui::Align2::CENTER_CENTER,
                "N",
                egui::FontId::proportional(s * 0.2),
                color,
            );
        }
        "cycle-count" => {
            // Circular arrows with hash
            painter.circle_stroke(c, s * 0.3, st);
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.05),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.05),
                Pos2::new(c.x + s * 0.22, c.y - s * 0.15),
            );
            painter.text(
                Pos2::new(c.x, c.y),
                egui::Align2::CENTER_CENTER,
                "#",
                egui::FontId::proportional(s * 0.25),
                color,
            );
        }
        "physical-count" => {
            // Hand with clipboard
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.1, c.y - s * 0.4),
                    Pos2::new(c.x + s * 0.4, c.y + s * 0.4),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x + s * 0.02, c.y - s * 0.48),
                    Pos2::new(c.x + s * 0.28, c.y - s * 0.4),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x + s * 0.0, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.3, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.0, c.y + s * 0.0),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.0),
            );
            ls(
                Pos2::new(c.x + s * 0.0, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.1),
                Pos2::new(c.x - s * 0.1, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.1),
                Pos2::new(c.x - s * 0.4, c.y - s * 0.05),
            );
        }
        "stock-valuation" => {
            // Box with dollar and chart
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.25),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            painter.text(
                Pos2::new(c.x - s * 0.15, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.3),
                color,
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.15, c.y - s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y - s * 0.05),
                Pos2::new(c.x + s * 0.25, c.y - s * 0.15),
            );
        }
        "safety-stock" => {
            // Shield with box
            ls(
                Pos2::new(c.x - s * 0.3, c.y - s * 0.35),
                Pos2::new(c.x, c.y - s * 0.45),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.45),
                Pos2::new(c.x + s * 0.3, c.y - s * 0.35),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.1),
                Pos2::new(c.x, c.y + s * 0.4),
            );
            ls(
                Pos2::new(c.x, c.y + s * 0.4),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y + s * 0.1),
                Pos2::new(c.x - s * 0.3, c.y - s * 0.35),
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.12, c.y - s * 0.15),
                    Pos2::new(c.x + s * 0.12, c.y + s * 0.05),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
        }
        "dead-stock" => {
            // Box with X
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.25),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            lsh(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.15),
            );
            lsh(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.15),
                Pos2::new(c.x - s * 0.2, c.y + s * 0.15),
            );
        }

        // ── Transportation ──────────────────────────────────────────────────
        "truck" => {
            // Cab + body + wheels
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.5, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.15, c.y + s * 0.2),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x + s * 0.15, c.y - s * 0.05),
                    Pos2::new(c.x + s * 0.5, c.y + s * 0.2),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            painter.circle_stroke(Pos2::new(c.x - s * 0.3, c.y + s * 0.3), s * 0.1, st);
            painter.circle_stroke(Pos2::new(c.x + s * 0.35, c.y + s * 0.3), s * 0.1, st);
        }
        "truck-loading" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.5, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.15, c.y + s * 0.2),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x + s * 0.15, c.y - s * 0.05),
                    Pos2::new(c.x + s * 0.5, c.y + s * 0.2),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            painter.circle_stroke(Pos2::new(c.x - s * 0.3, c.y + s * 0.3), s * 0.1, st);
            painter.circle_stroke(Pos2::new(c.x + s * 0.35, c.y + s * 0.3), s * 0.1, st);
            // Arrow into body
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.5),
                Pos2::new(c.x - s * 0.2, c.y - s * 0.25),
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.38),
                Pos2::new(c.x - s * 0.2, c.y - s * 0.25),
            );
            ls(
                Pos2::new(c.x - s * 0.05, c.y - s * 0.38),
                Pos2::new(c.x - s * 0.2, c.y - s * 0.25),
            );
        }
        "truck-delivery" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.5, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.15, c.y + s * 0.2),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x + s * 0.15, c.y - s * 0.05),
                    Pos2::new(c.x + s * 0.5, c.y + s * 0.2),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            painter.circle_stroke(Pos2::new(c.x - s * 0.3, c.y + s * 0.3), s * 0.1, st);
            painter.circle_stroke(Pos2::new(c.x + s * 0.35, c.y + s * 0.3), s * 0.1, st);
            // Check mark on body
            ls(
                Pos2::new(c.x - s * 0.3, c.y),
                Pos2::new(c.x - s * 0.15, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.05, c.y - s * 0.15),
            );
        }
        "van" => {
            // Rounded van body
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.5, c.y - s * 0.3),
                    Pos2::new(c.x + s * 0.5, c.y + s * 0.2),
                ),
                s * 0.15,
                st,
                egui::StrokeKind::Middle,
            );
            painter.circle_stroke(Pos2::new(c.x - s * 0.3, c.y + s * 0.3), s * 0.1, st);
            painter.circle_stroke(Pos2::new(c.x + s * 0.3, c.y + s * 0.3), s * 0.1, st);
            ls(
                Pos2::new(c.x + s * 0.15, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.2),
            );
        }
        "ship" => {
            // Hull
            ls(
                Pos2::new(c.x - s * 0.5, c.y + s * 0.1),
                Pos2::new(c.x - s * 0.35, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y + s * 0.35),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y + s * 0.35),
                Pos2::new(c.x + s * 0.5, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.5, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.5, c.y + s * 0.1),
            );
            // Cabin
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.15, c.y - s * 0.2),
                    Pos2::new(c.x + s * 0.15, c.y + s * 0.1),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            // Smokestack
            ls(
                Pos2::new(c.x, c.y - s * 0.2),
                Pos2::new(c.x, c.y - s * 0.45),
            );
        }
        "ship-cargo" => {
            ls(
                Pos2::new(c.x - s * 0.5, c.y + s * 0.1),
                Pos2::new(c.x - s * 0.35, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y + s * 0.35),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y + s * 0.35),
                Pos2::new(c.x + s * 0.5, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.5, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.5, c.y + s * 0.1),
            );
            // Containers on deck
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.3, c.y - s * 0.15),
                    Pos2::new(c.x - s * 0.05, c.y + s * 0.1),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x + s * 0.05, c.y - s * 0.15),
                    Pos2::new(c.x + s * 0.3, c.y + s * 0.1),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
        }
        "airplane" => {
            // Fuselage
            ls(Pos2::new(c.x - s * 0.5, c.y), Pos2::new(c.x + s * 0.5, c.y));
            // Nose
            ls(
                Pos2::new(c.x + s * 0.5, c.y),
                Pos2::new(c.x + s * 0.55, c.y - s * 0.05),
            );
            // Wings
            ls(
                Pos2::new(c.x - s * 0.05, c.y),
                Pos2::new(c.x - s * 0.2, c.y - s * 0.35),
            );
            ls(
                Pos2::new(c.x - s * 0.05, c.y),
                Pos2::new(c.x - s * 0.2, c.y + s * 0.35),
            );
            // Tail
            ls(
                Pos2::new(c.x - s * 0.45, c.y),
                Pos2::new(c.x - s * 0.5, c.y - s * 0.2),
            );
        }
        "airplane-landing" => {
            // Tilted fuselage
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.2),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.15),
            );
            // Wings
            ls(
                Pos2::new(c.x, c.y),
                Pos2::new(c.x - s * 0.15, c.y - s * 0.35),
            );
            ls(
                Pos2::new(c.x, c.y),
                Pos2::new(c.x - s * 0.15, c.y + s * 0.25),
            );
            // Ground line
            lsh(
                Pos2::new(c.x - s * 0.5, c.y + s * 0.4),
                Pos2::new(c.x + s * 0.5, c.y + s * 0.4),
            );
            // Arrow down
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.35),
            );
        }
        "helicopter" => {
            // Body
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.3, c.y - s * 0.1),
                    Pos2::new(c.x + s * 0.2, c.y + s * 0.15),
                ),
                s * 0.08,
                st,
                egui::StrokeKind::Middle,
            );
            // Rotor top
            ls(
                Pos2::new(c.x - s * 0.45, c.y - s * 0.25),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.25),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.1),
                Pos2::new(c.x, c.y - s * 0.25),
            );
            // Tail
            ls(
                Pos2::new(c.x + s * 0.2, c.y),
                Pos2::new(c.x + s * 0.5, c.y - s * 0.1),
            );
            // Skids
            ls(
                Pos2::new(c.x - s * 0.3, c.y + s * 0.15),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.3),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.3),
            );
        }
        "train" => {
            // Body
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.4, c.y + s * 0.15),
                ),
                s * 0.06,
                st,
                egui::StrokeKind::Middle,
            );
            // Window
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.15, c.y - s * 0.2),
                    Pos2::new(c.x + s * 0.15, c.y - s * 0.05),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            // Wheels
            painter.circle_stroke(Pos2::new(c.x - s * 0.2, c.y + s * 0.25), s * 0.1, st);
            painter.circle_stroke(Pos2::new(c.x + s * 0.2, c.y + s * 0.25), s * 0.1, st);
            // Smokestack
            ls(
                Pos2::new(c.x - s * 0.25, c.y - s * 0.25),
                Pos2::new(c.x - s * 0.25, c.y - s * 0.4),
            );
        }
        "railway" => {
            // Two rails
            ls(
                Pos2::new(c.x - s * 0.5, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.5, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.5, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.5, c.y + s * 0.15),
            );
            // Ties
            for i in -2..=2 {
                let x = c.x + s * 0.2 * i as f32;
                ls(Pos2::new(x, c.y - s * 0.25), Pos2::new(x, c.y + s * 0.25));
            }
        }
        "container" => {
            // Shipping container box
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.45, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.45, c.y + s * 0.25),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            // Corrugation lines
            for i in -1..=1 {
                let x = c.x + s * 0.2 * i as f32;
                ls(Pos2::new(x, c.y - s * 0.25), Pos2::new(x, c.y + s * 0.25));
            }
        }
        "forklift" => {
            // Body
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.15, c.y - s * 0.2),
                    Pos2::new(c.x + s * 0.25, c.y + s * 0.2),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            // Forks
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.15),
                Pos2::new(c.x - s * 0.45, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.05),
                Pos2::new(c.x - s * 0.45, c.y + s * 0.05),
            );
            // Mast
            ls(
                Pos2::new(c.x - s * 0.15, c.y - s * 0.4),
                Pos2::new(c.x - s * 0.15, c.y + s * 0.2),
            );
            // Wheel
            painter.circle_stroke(Pos2::new(c.x + s * 0.15, c.y + s * 0.3), s * 0.1, st);
        }
        "crane" => {
            // Tower
            ls(Pos2::new(c.x, c.y + s * 0.5), Pos2::new(c.x, c.y - s * 0.4));
            // Boom arm
            ls(
                Pos2::new(c.x, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.4),
            );
            // Cable
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.35, c.y),
            );
            // Hook
            painter.circle_stroke(Pos2::new(c.x + s * 0.35, c.y + s * 0.05), s * 0.06, st);
            // Base
            ls(
                Pos2::new(c.x - s * 0.25, c.y + s * 0.5),
                Pos2::new(c.x + s * 0.25, c.y + s * 0.5),
            );
        }
        "anchor" => {
            // Ring at top
            painter.circle_stroke(Pos2::new(c.x, c.y - s * 0.35), s * 0.1, st);
            // Shaft
            ls(
                Pos2::new(c.x, c.y - s * 0.25),
                Pos2::new(c.x, c.y + s * 0.35),
            );
            // Cross bar
            ls(
                Pos2::new(c.x - s * 0.25, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.25, c.y - s * 0.1),
            );
            // Flukes
            ls(
                Pos2::new(c.x, c.y + s * 0.35),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x, c.y + s * 0.35),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.15),
            );
        }
        "compass" => {
            painter.circle_stroke(c, s * 0.45, st);
            // N-S needle
            ls(
                Pos2::new(c.x, c.y - s * 0.35),
                Pos2::new(c.x, c.y + s * 0.35),
            );
            // E-W
            ls(
                Pos2::new(c.x - s * 0.35, c.y),
                Pos2::new(c.x + s * 0.35, c.y),
            );
            // N pointer
            ls(
                Pos2::new(c.x, c.y - s * 0.35),
                Pos2::new(c.x - s * 0.08, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.08, c.y - s * 0.1),
            );
        }
        "route" => {
            // Winding path
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.4),
                Pos2::new(c.x - s * 0.15, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.15, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.4),
            );
            // Pin at end
            painter.circle_filled(Pos2::new(c.x + s * 0.4, c.y - s * 0.4), s * 0.08, color);
        }
        "highway" => {
            // Two parallel lines
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.5),
                Pos2::new(c.x - s * 0.2, c.y + s * 0.5),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.5),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.5),
            );
            // Dashed center
            for i in -2..=2 {
                let y = c.y + s * 0.2 * i as f32;
                ls(Pos2::new(c.x, y - s * 0.05), Pos2::new(c.x, y + s * 0.05));
            }
        }
        "bridge" => {
            // Deck
            lsh(Pos2::new(c.x - s * 0.5, c.y), Pos2::new(c.x + s * 0.5, c.y));
            // Arches below
            ls(
                Pos2::new(c.x - s * 0.4, c.y),
                Pos2::new(c.x - s * 0.25, c.y + s * 0.25),
            );
            ls(
                Pos2::new(c.x - s * 0.25, c.y + s * 0.25),
                Pos2::new(c.x - s * 0.1, c.y),
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y),
                Pos2::new(c.x + s * 0.25, c.y + s * 0.25),
            );
            ls(
                Pos2::new(c.x + s * 0.25, c.y + s * 0.25),
                Pos2::new(c.x + s * 0.4, c.y),
            );
            // Pillars
            ls(
                Pos2::new(c.x - s * 0.4, c.y),
                Pos2::new(c.x - s * 0.4, c.y + s * 0.4),
            );
            ls(
                Pos2::new(c.x + s * 0.4, c.y),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.4),
            );
        }
        "toll" => {
            // Barrier bar
            lsh(
                Pos2::new(c.x - s * 0.15, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.5, c.y - s * 0.15),
            );
            // Post
            ls(
                Pos2::new(c.x - s * 0.15, c.y - s * 0.15),
                Pos2::new(c.x - s * 0.15, c.y + s * 0.4),
            );
            // Base
            ls(
                Pos2::new(c.x - s * 0.35, c.y + s * 0.4),
                Pos2::new(c.x + s * 0.05, c.y + s * 0.4),
            );
            // Light
            painter.circle_filled(Pos2::new(c.x - s * 0.15, c.y - s * 0.3), s * 0.08, color);
        }
        "fuel-pump" => {
            // Pump body
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.3, c.y - s * 0.3),
                    Pos2::new(c.x + s * 0.1, c.y + s * 0.4),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            // Nozzle
            ls(
                Pos2::new(c.x + s * 0.1, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.1),
            );
            // Display
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.2, c.y - s * 0.2),
                    Pos2::new(c.x, c.y - s * 0.05),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
        }
        "tire" => {
            painter.circle_stroke(c, s * 0.4, st);
            painter.circle_stroke(c, s * 0.2, st);
            // Spokes
            for angle in [0.0f32, 1.57, 3.14, 4.71] {
                ls(
                    Pos2::new(c.x + s * 0.2 * angle.cos(), c.y + s * 0.2 * angle.sin()),
                    Pos2::new(c.x + s * 0.4 * angle.cos(), c.y + s * 0.4 * angle.sin()),
                );
            }
        }
        "engine" => {
            // Block
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.3, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.3, c.y + s * 0.25),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            // Pistons on top
            ls(
                Pos2::new(c.x - s * 0.15, c.y - s * 0.25),
                Pos2::new(c.x - s * 0.15, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y - s * 0.25),
                Pos2::new(c.x + s * 0.15, c.y - s * 0.4),
            );
            // Bolt
            painter.circle_filled(Pos2::new(c.x, c.y), s * 0.06, color);
        }
        "speedometer" => {
            // Half circle gauge
            painter.circle_stroke(c, s * 0.4, st);
            // Needle pointing upper-right
            lsh(c, Pos2::new(c.x + s * 0.25, c.y - s * 0.25));
            // Tick marks
            painter.circle_filled(c, s * 0.05, color);
        }
        "odometer" => {
            // Display box with digits
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.45, c.y - s * 0.15),
                    Pos2::new(c.x + s * 0.45, c.y + s * 0.15),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            // Digit separators
            for i in -1..=1 {
                let x = c.x + s * 0.18 * i as f32;
                ls(Pos2::new(x, c.y - s * 0.15), Pos2::new(x, c.y + s * 0.15));
            }
        }

        // ── Logistics ───────────────────────────────────────────────────────
        "package" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.35),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            // Top flaps
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.25),
                Pos2::new(c.x, c.y - s * 0.45),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.25),
                Pos2::new(c.x, c.y - s * 0.45),
            );
            // Tape
            ls(
                Pos2::new(c.x, c.y - s * 0.45),
                Pos2::new(c.x, c.y + s * 0.35),
            );
        }
        "package-open" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.15),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.35),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            // Open flaps
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.15),
                Pos2::new(c.x - s * 0.5, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.5, c.y - s * 0.4),
            );
        }
        "package-check" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.35),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.25),
                Pos2::new(c.x, c.y - s * 0.45),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.25),
                Pos2::new(c.x, c.y - s * 0.45),
            );
            // Check
            lsh(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.05),
                Pos2::new(c.x - s * 0.02, c.y + s * 0.2),
            );
            lsh(
                Pos2::new(c.x - s * 0.02, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.2, c.y - s * 0.05),
            );
        }
        "package-x" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.35),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.25),
                Pos2::new(c.x, c.y - s * 0.45),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.25),
                Pos2::new(c.x, c.y - s * 0.45),
            );
            // X
            lsh(
                Pos2::new(c.x - s * 0.15, c.y - s * 0.05),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.25),
            );
            lsh(
                Pos2::new(c.x + s * 0.15, c.y - s * 0.05),
                Pos2::new(c.x - s * 0.15, c.y + s * 0.25),
            );
        }
        "package-search" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.2, c.y + s * 0.35),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.25),
                Pos2::new(c.x - s * 0.1, c.y - s * 0.45),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.25),
                Pos2::new(c.x - s * 0.1, c.y - s * 0.45),
            );
            // Magnifier
            painter.circle_stroke(Pos2::new(c.x + s * 0.25, c.y + s * 0.1), s * 0.15, st);
            ls(
                Pos2::new(c.x + s * 0.35, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.5, c.y + s * 0.35),
            );
        }
        "conveyor" => {
            // Belt (two horizontal lines)
            ls(
                Pos2::new(c.x - s * 0.45, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.45, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.45, c.y + s * 0.25),
                Pos2::new(c.x + s * 0.45, c.y + s * 0.25),
            );
            // Rollers
            painter.circle_stroke(Pos2::new(c.x - s * 0.4, c.y + s * 0.175), s * 0.08, st);
            painter.circle_stroke(Pos2::new(c.x + s * 0.4, c.y + s * 0.175), s * 0.08, st);
            // Box on belt
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.15, c.y - s * 0.2),
                    Pos2::new(c.x + s * 0.15, c.y + s * 0.1),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
        }
        "loading-dock" => {
            // Dock platform
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.5, c.y),
                    Pos2::new(c.x + s * 0.1, c.y + s * 0.15),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            // Truck back
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x + s * 0.1, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.5, c.y + s * 0.15),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            // Ramp
            ls(
                Pos2::new(c.x - s * 0.5, c.y + s * 0.15),
                Pos2::new(c.x - s * 0.5, c.y + s * 0.4),
            );
        }
        "dispatch" => {
            // Clipboard
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.3, c.y - s * 0.35),
                    Pos2::new(c.x + s * 0.3, c.y + s * 0.45),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            // Clip at top
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.12, c.y - s * 0.45),
                    Pos2::new(c.x + s * 0.12, c.y - s * 0.3),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            // Arrow right
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.05),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y - s * 0.05),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.05),
            );
        }
        "tracking" => {
            // Crosshair/target
            painter.circle_stroke(c, s * 0.3, st);
            painter.circle_stroke(c, s * 0.15, st);
            painter.circle_filled(c, s * 0.04, color);
            // Cross lines extending outside
            ls(
                Pos2::new(c.x, c.y - s * 0.45),
                Pos2::new(c.x, c.y - s * 0.3),
            );
            ls(
                Pos2::new(c.x, c.y + s * 0.3),
                Pos2::new(c.x, c.y + s * 0.45),
            );
            ls(
                Pos2::new(c.x - s * 0.45, c.y),
                Pos2::new(c.x - s * 0.3, c.y),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y),
                Pos2::new(c.x + s * 0.45, c.y),
            );
        }
        "tracking-number" => {
            // Barcode-like lines
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y - s * 0.3),
                    Pos2::new(c.x + s * 0.4, c.y + s * 0.3),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            for i in -3..=3 {
                let x = c.x + s * 0.1 * i as f32;
                let h = if i % 2 == 0 { 0.2 } else { 0.15 };
                ls(Pos2::new(x, c.y - h * s), Pos2::new(x, c.y + h * s));
            }
        }
        "delivery-time" => {
            // Clock face
            painter.circle_stroke(c, s * 0.35, st);
            // Hands
            ls(c, Pos2::new(c.x + s * 0.2, c.y - s * 0.1));
            ls(c, Pos2::new(c.x, c.y - s * 0.25));
            // Small truck below
            ls(
                Pos2::new(c.x + s * 0.2, c.y + s * 0.3),
                Pos2::new(c.x + s * 0.5, c.y + s * 0.3),
            );
        }
        "express" => {
            // Lightning bolt
            lsh(
                Pos2::new(c.x + s * 0.1, c.y - s * 0.45),
                Pos2::new(c.x - s * 0.1, c.y),
            );
            lsh(
                Pos2::new(c.x - s * 0.1, c.y),
                Pos2::new(c.x + s * 0.15, c.y),
            );
            lsh(
                Pos2::new(c.x + s * 0.15, c.y),
                Pos2::new(c.x - s * 0.05, c.y + s * 0.45),
            );
        }
        "fragile" => {
            // Wine glass outline
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.2, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.4),
                Pos2::new(c.x - s * 0.15, c.y - s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.15, c.y - s * 0.05),
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y - s * 0.05),
                Pos2::new(c.x + s * 0.15, c.y - s * 0.05),
            );
            // Stem
            ls(
                Pos2::new(c.x, c.y - s * 0.05),
                Pos2::new(c.x, c.y + s * 0.25),
            );
            // Base
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.25),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.25),
            );
            // Crack
            ls(
                Pos2::new(c.x, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.08, c.y - s * 0.15),
            );
        }
        "hazmat" => {
            // Triangle warning
            ls(
                Pos2::new(c.x, c.y - s * 0.4),
                Pos2::new(c.x - s * 0.4, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.3),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.4, c.y + s * 0.3),
                Pos2::new(c.x, c.y - s * 0.4),
            );
            // Exclamation
            lsh(
                Pos2::new(c.x, c.y - s * 0.15),
                Pos2::new(c.x, c.y + s * 0.1),
            );
            painter.circle_filled(Pos2::new(c.x, c.y + s * 0.2), s * 0.04, color);
        }
        "temperature" => {
            // Thermometer body
            ls(
                Pos2::new(c.x, c.y - s * 0.45),
                Pos2::new(c.x, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.08, c.y - s * 0.45),
                Pos2::new(c.x - s * 0.08, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.08, c.y - s * 0.45),
                Pos2::new(c.x + s * 0.08, c.y + s * 0.15),
            );
            // Bulb
            painter.circle_stroke(Pos2::new(c.x, c.y + s * 0.25), s * 0.15, st);
            painter.circle_filled(Pos2::new(c.x, c.y + s * 0.25), s * 0.08, color);
        }
        "weight-scale" => {
            // Base
            ls(
                Pos2::new(c.x - s * 0.35, c.y + s * 0.35),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.35),
            );
            // Pillar
            ls(
                Pos2::new(c.x, c.y + s * 0.35),
                Pos2::new(c.x, c.y - s * 0.15),
            );
            // Beam
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.15),
            );
            // Pans (arcs)
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.15),
                Pos2::new(c.x - s * 0.45, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x - s * 0.25, c.y - s * 0.15),
                Pos2::new(c.x - s * 0.2, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x - s * 0.45, c.y + s * 0.05),
                Pos2::new(c.x - s * 0.2, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.4, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.45, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.25, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.45, c.y + s * 0.05),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.05),
            );
        }
        "dimensions" => {
            // Horizontal dimension line
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.25),
                Pos2::new(c.x - s * 0.4, c.y - s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.4, c.y - s * 0.25),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.05),
            );
            // Vertical dimension line
            ls(
                Pos2::new(c.x - s * 0.15, c.y - s * 0.4),
                Pos2::new(c.x - s * 0.15, c.y + s * 0.4),
            );
            ls(
                Pos2::new(c.x - s * 0.25, c.y - s * 0.4),
                Pos2::new(c.x - s * 0.05, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x - s * 0.25, c.y + s * 0.4),
                Pos2::new(c.x - s * 0.05, c.y + s * 0.4),
            );
        }
        "customs" => {
            // Shield
            ls(
                Pos2::new(c.x - s * 0.3, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.3, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y - s * 0.4),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y + s * 0.1),
                Pos2::new(c.x, c.y + s * 0.4),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.1),
                Pos2::new(c.x, c.y + s * 0.4),
            );
            // Check inside
            ls(
                Pos2::new(c.x - s * 0.12, c.y - s * 0.05),
                Pos2::new(c.x - s * 0.02, c.y + s * 0.08),
            );
            ls(
                Pos2::new(c.x - s * 0.02, c.y + s * 0.08),
                Pos2::new(c.x + s * 0.15, c.y - s * 0.15),
            );
        }
        "manifest" => {
            // Document with lines
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.3, c.y - s * 0.45),
                    Pos2::new(c.x + s * 0.3, c.y + s * 0.45),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            for i in -2..=2 {
                let y = c.y + s * 0.15 * i as f32;
                ls(Pos2::new(c.x - s * 0.2, y), Pos2::new(c.x + s * 0.2, y));
            }
        }
        "bill-of-lading" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.3, c.y - s * 0.45),
                    Pos2::new(c.x + s * 0.3, c.y + s * 0.45),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            // Header area
            lsh(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.2, c.y - s * 0.3),
            );
            // Content lines
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.2, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.05),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.2),
            );
            // Ship icon in header
            ls(
                Pos2::new(c.x - s * 0.1, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.1, c.y - s * 0.4),
            );
        }
        "cross-dock" => {
            // Cross shape (dock bays)
            ls(Pos2::new(c.x, c.y - s * 0.4), Pos2::new(c.x, c.y + s * 0.4));
            ls(Pos2::new(c.x - s * 0.4, c.y), Pos2::new(c.x + s * 0.4, c.y));
            // Arrow tips at ends
            ls(
                Pos2::new(c.x - s * 0.1, c.y - s * 0.3),
                Pos2::new(c.x, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y - s * 0.3),
                Pos2::new(c.x, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.4, c.y),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.4, c.y),
            );
        }
        "last-mile" => {
            // House destination
            ls(
                Pos2::new(c.x + s * 0.1, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y - s * 0.3),
                Pos2::new(c.x - s * 0.15, c.y - s * 0.1),
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.1, c.y - s * 0.1),
                    Pos2::new(c.x + s * 0.3, c.y + s * 0.2),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            // Dotted path to house
            for i in -3..=0 {
                let x = c.x + s * 0.15 * i as f32 - s * 0.15;
                painter.circle_filled(Pos2::new(x, c.y + s * 0.35), s * 0.03, color);
            }
        }
        "return-shipment" => {
            // Box
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.25, c.y - s * 0.15),
                    Pos2::new(c.x + s * 0.25, c.y + s * 0.25),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            // Return arrow (curved left)
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.35),
                Pos2::new(c.x - s * 0.2, c.y - s * 0.35),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.35),
                Pos2::new(c.x - s * 0.08, c.y - s * 0.45),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.35),
                Pos2::new(c.x - s * 0.08, c.y - s * 0.25),
            );
        }
        "consolidation" => {
            // Multiple small boxes converging into one
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.15, c.y + s * 0.05),
                    Pos2::new(c.x + s * 0.15, c.y + s * 0.35),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            // Small boxes
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.45, c.y - s * 0.4),
                    Pos2::new(c.x - s * 0.25, c.y - s * 0.2),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.1, c.y - s * 0.4),
                    Pos2::new(c.x + s * 0.1, c.y - s * 0.2),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x + s * 0.25, c.y - s * 0.4),
                    Pos2::new(c.x + s * 0.45, c.y - s * 0.2),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            // Arrows down
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.2),
                Pos2::new(c.x, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.2),
                Pos2::new(c.x, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.2),
                Pos2::new(c.x, c.y + s * 0.05),
            );
        }
        "deconsolidation" => {
            // One big box at top, arrows to small boxes
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.15, c.y - s * 0.4),
                    Pos2::new(c.x + s * 0.15, c.y - s * 0.1),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.45, c.y + s * 0.15),
                    Pos2::new(c.x - s * 0.25, c.y + s * 0.35),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.1, c.y + s * 0.15),
                    Pos2::new(c.x + s * 0.1, c.y + s * 0.35),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x + s * 0.25, c.y + s * 0.15),
                    Pos2::new(c.x + s * 0.45, c.y + s * 0.35),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.1),
                Pos2::new(c.x - s * 0.35, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.1),
                Pos2::new(c.x, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.15),
            );
        }

        // ── Financial ───────────────────────────────────────────────────────
        "dollar" => {
            painter.circle_stroke(c, s * 0.4, st);
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.7),
                color,
            );
        }
        "euro" => {
            painter.circle_stroke(c, s * 0.4, st);
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                "\u{20AC}",
                egui::FontId::proportional(s * 0.7),
                color,
            );
        }
        "yen" => {
            painter.circle_stroke(c, s * 0.4, st);
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                "\u{00A5}",
                egui::FontId::proportional(s * 0.7),
                color,
            );
        }
        "pound" => {
            painter.circle_stroke(c, s * 0.4, st);
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                "\u{00A3}",
                egui::FontId::proportional(s * 0.7),
                color,
            );
        }
        "bitcoin" => {
            painter.circle_stroke(c, s * 0.4, st);
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                "\u{20BF}",
                egui::FontId::proportional(s * 0.7),
                color,
            );
        }
        "coins" => {
            painter.circle_stroke(Pos2::new(c.x - s * 0.12, c.y + s * 0.05), s * 0.25, st);
            painter.circle_stroke(Pos2::new(c.x + s * 0.12, c.y - s * 0.05), s * 0.25, st);
        }
        "money-bag" => {
            // Bag body
            painter.circle_stroke(Pos2::new(c.x, c.y + s * 0.1), s * 0.35, st);
            // Tie at top
            ls(
                Pos2::new(c.x - s * 0.1, c.y - s * 0.25),
                Pos2::new(c.x, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y - s * 0.25),
                Pos2::new(c.x, c.y - s * 0.4),
            );
            painter.text(
                Pos2::new(c.x, c.y + s * 0.15),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.4),
                color,
            );
        }
        "piggy-bank" => {
            // Body
            painter.circle_stroke(c, s * 0.3, st);
            // Snout
            painter.circle_stroke(Pos2::new(c.x + s * 0.35, c.y), s * 0.1, st);
            // Ear
            ls(
                Pos2::new(c.x - s * 0.1, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.05, c.y - s * 0.42),
            );
            // Legs
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.3),
                Pos2::new(c.x - s * 0.15, c.y + s * 0.45),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y + s * 0.3),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.45),
            );
            // Slot
            ls(
                Pos2::new(c.x - s * 0.1, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.1, c.y - s * 0.3),
            );
        }
        "vault" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y - s * 0.4),
                    Pos2::new(c.x + s * 0.4, c.y + s * 0.4),
                ),
                s * 0.06,
                st,
                egui::StrokeKind::Middle,
            );
            // Dial
            painter.circle_stroke(c, s * 0.2, st);
            painter.circle_filled(c, s * 0.05, color);
            // Handle
            ls(
                Pos2::new(c.x + s * 0.2, c.y),
                Pos2::new(c.x + s * 0.32, c.y),
            );
        }
        "safe" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y - s * 0.35),
                    Pos2::new(c.x + s * 0.4, c.y + s * 0.35),
                ),
                s * 0.06,
                st,
                egui::StrokeKind::Middle,
            );
            // Keyhole
            painter.circle_stroke(Pos2::new(c.x, c.y - s * 0.05), s * 0.08, st);
            ls(
                Pos2::new(c.x, c.y + s * 0.03),
                Pos2::new(c.x, c.y + s * 0.15),
            );
            // Handle bar
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.15),
            );
        }
        "bank" => {
            // Pediment triangle
            ls(
                Pos2::new(c.x - s * 0.45, c.y - s * 0.1),
                Pos2::new(c.x, c.y - s * 0.4),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.45, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.1),
            );
            // Columns
            ls(
                Pos2::new(c.x - s * 0.3, c.y - s * 0.1),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.3),
            );
            ls(Pos2::new(c.x, c.y - s * 0.1), Pos2::new(c.x, c.y + s * 0.3));
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.3),
            );
            // Base
            lsh(
                Pos2::new(c.x - s * 0.45, c.y + s * 0.3),
                Pos2::new(c.x + s * 0.45, c.y + s * 0.3),
            );
        }
        "atm" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.45),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.45),
                ),
                s * 0.06,
                st,
                egui::StrokeKind::Middle,
            );
            // Screen
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.22, c.y - s * 0.35),
                    Pos2::new(c.x + s * 0.22, c.y - s * 0.1),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            // Keypad dots
            for row in 0..2 {
                for col in -1..=1 {
                    painter.circle_filled(
                        Pos2::new(
                            c.x + s * 0.12 * col as f32,
                            c.y + s * 0.08 + s * 0.15 * row as f32,
                        ),
                        s * 0.04,
                        color,
                    );
                }
            }
            // Card slot
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.38),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.38),
            );
        }
        "exchange-rate" => {
            // Two currency circles with arrows between
            painter.circle_stroke(Pos2::new(c.x - s * 0.25, c.y), s * 0.18, st);
            painter.circle_stroke(Pos2::new(c.x + s * 0.25, c.y), s * 0.18, st);
            // Arrows
            ls(
                Pos2::new(c.x - s * 0.05, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.05, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y + s * 0.15),
                Pos2::new(c.x - s * 0.05, c.y + s * 0.15),
            );
        }
        "stock-market" => {
            // Upward trending line
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.3),
                Pos2::new(c.x - s * 0.15, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.05),
                Pos2::new(c.x + s * 0.05, c.y + s * 0.2),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.3),
            );
            // Arrow tip
            ls(
                Pos2::new(c.x + s * 0.4, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.25, c.y - s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.4, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.15),
            );
        }
        "bull-market" => {
            // Up arrow
            lsh(
                Pos2::new(c.x, c.y + s * 0.35),
                Pos2::new(c.x, c.y - s * 0.35),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.15),
                Pos2::new(c.x, c.y - s * 0.35),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.15),
                Pos2::new(c.x, c.y - s * 0.35),
            );
            // Horns
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.1),
                Pos2::new(c.x - s * 0.35, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.1),
            );
        }
        "bear-market" => {
            // Down arrow
            lsh(
                Pos2::new(c.x, c.y - s * 0.35),
                Pos2::new(c.x, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.15),
                Pos2::new(c.x, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y + s * 0.15),
                Pos2::new(c.x, c.y + s * 0.35),
            );
            // Paw marks
            painter.circle_filled(Pos2::new(c.x - s * 0.25, c.y - s * 0.15), s * 0.05, color);
            painter.circle_filled(Pos2::new(c.x + s * 0.25, c.y - s * 0.15), s * 0.05, color);
        }
        "dividend" => {
            // Coin with percent sign
            painter.circle_stroke(c, s * 0.35, st);
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                "%",
                egui::FontId::proportional(s * 0.5),
                color,
            );
        }
        "interest-rate" => {
            // Percentage with up arrow
            painter.text(
                Pos2::new(c.x - s * 0.15, c.y),
                egui::Align2::CENTER_CENTER,
                "%",
                egui::FontId::proportional(s * 0.5),
                color,
            );
            ls(
                Pos2::new(c.x + s * 0.25, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.25, c.y - s * 0.25),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y - s * 0.12),
                Pos2::new(c.x + s * 0.25, c.y - s * 0.25),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.12),
                Pos2::new(c.x + s * 0.25, c.y - s * 0.25),
            );
        }
        "mortgage" => {
            // House outline
            ls(
                Pos2::new(c.x, c.y - s * 0.4),
                Pos2::new(c.x - s * 0.35, c.y - s * 0.05),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.05),
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.3, c.y - s * 0.05),
                    Pos2::new(c.x + s * 0.3, c.y + s * 0.35),
                ),
                s * 0.02,
                st,
                egui::StrokeKind::Middle,
            );
            // Dollar in house
            painter.text(
                Pos2::new(c.x, c.y + s * 0.15),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.35),
                color,
            );
        }
        "loan" => {
            // Hand receiving coin
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.15),
                Pos2::new(c.x - s * 0.1, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.05, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y + s * 0.05),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.15),
            );
            // Coin above
            painter.circle_stroke(Pos2::new(c.x, c.y - s * 0.2), s * 0.18, st);
            painter.text(
                Pos2::new(c.x, c.y - s * 0.2),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.25),
                color,
            );
        }
        "audit" => {
            // Magnifier over document
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.4),
                    Pos2::new(c.x + s * 0.15, c.y + s * 0.4),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            ls(
                Pos2::new(c.x - s * 0.25, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.05, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.25, c.y + s * 0.0),
                Pos2::new(c.x + s * 0.05, c.y + s * 0.0),
            );
            ls(
                Pos2::new(c.x - s * 0.25, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.05, c.y + s * 0.15),
            );
            // Magnifier
            painter.circle_stroke(Pos2::new(c.x + s * 0.25, c.y + s * 0.1), s * 0.15, st);
            ls(
                Pos2::new(c.x + s * 0.35, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.48, c.y + s * 0.35),
            );
        }
        "ledger" => {
            // Book with lines
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.4),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.4),
                ),
                s * 0.06,
                st,
                egui::StrokeKind::Middle,
            );
            // Spine
            ls(
                Pos2::new(c.x - s * 0.2, c.y - s * 0.4),
                Pos2::new(c.x - s * 0.2, c.y + s * 0.4),
            );
            // Lines
            ls(
                Pos2::new(c.x - s * 0.1, c.y - s * 0.2),
                Pos2::new(c.x + s * 0.25, c.y - s * 0.2),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y),
                Pos2::new(c.x + s * 0.25, c.y),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y + s * 0.2),
                Pos2::new(c.x + s * 0.25, c.y + s * 0.2),
            );
        }
        "balance-sheet" => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.4),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.4),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            // Vertical divider
            ls(Pos2::new(c.x, c.y - s * 0.4), Pos2::new(c.x, c.y + s * 0.4));
            // Horizontal header
            lsh(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.2),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.2),
            );
            // Left column items
            ls(
                Pos2::new(c.x - s * 0.28, c.y - s * 0.05),
                Pos2::new(c.x - s * 0.05, c.y - s * 0.05),
            );
            ls(
                Pos2::new(c.x - s * 0.28, c.y + s * 0.1),
                Pos2::new(c.x - s * 0.05, c.y + s * 0.1),
            );
            // Right column items
            ls(
                Pos2::new(c.x + s * 0.05, c.y - s * 0.05),
                Pos2::new(c.x + s * 0.28, c.y - s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.28, c.y + s * 0.1),
            );
        }
        "profit-loss" => {
            // Up and down arrows side by side
            // Profit (up)
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.3),
                Pos2::new(c.x - s * 0.2, c.y - s * 0.3),
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.1),
                Pos2::new(c.x - s * 0.2, c.y - s * 0.3),
            );
            ls(
                Pos2::new(c.x - s * 0.05, c.y - s * 0.1),
                Pos2::new(c.x - s * 0.2, c.y - s * 0.3),
            );
            // Loss (down)
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.05, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.3),
            );
        }
        "cash-flow" => {
            // Dollar sign with flowing arrows
            painter.text(
                Pos2::new(c.x - s * 0.15, c.y),
                egui::Align2::CENTER_CENTER,
                "$",
                egui::FontId::proportional(s * 0.6),
                color,
            );
            // Arrow right
            ls(
                Pos2::new(c.x + s * 0.1, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.2),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.1),
            );
            // Arrow left
            ls(
                Pos2::new(c.x + s * 0.4, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y + s * 0.05),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y + s * 0.25),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.15),
            );
        }

        // ── Social Media ────────────────────────────────────────────────────
        "like" => {
            // Heart shape (filled)
            painter.circle_filled(Pos2::new(c.x - s * 0.15, c.y - s * 0.1), s * 0.18, color);
            painter.circle_filled(Pos2::new(c.x + s * 0.15, c.y - s * 0.1), s * 0.18, color);
            // Point at bottom
            ls(
                Pos2::new(c.x - s * 0.3, c.y),
                Pos2::new(c.x, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y),
                Pos2::new(c.x, c.y + s * 0.35),
            );
        }
        "dislike" => {
            // Broken heart
            painter.circle_stroke(Pos2::new(c.x - s * 0.15, c.y - s * 0.1), s * 0.18, st);
            painter.circle_stroke(Pos2::new(c.x + s * 0.15, c.y - s * 0.1), s * 0.18, st);
            ls(
                Pos2::new(c.x - s * 0.3, c.y),
                Pos2::new(c.x, c.y + s * 0.35),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y),
                Pos2::new(c.x, c.y + s * 0.35),
            );
            // Crack
            lsh(
                Pos2::new(c.x, c.y - s * 0.2),
                Pos2::new(c.x + s * 0.05, c.y + s * 0.1),
            );
            lsh(
                Pos2::new(c.x + s * 0.05, c.y + s * 0.1),
                Pos2::new(c.x - s * 0.05, c.y + s * 0.25),
            );
        }
        "comment" => {
            // Speech bubble
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y - s * 0.3),
                    Pos2::new(c.x + s * 0.4, c.y + s * 0.15),
                ),
                s * 0.1,
                st,
                egui::StrokeKind::Middle,
            );
            // Tail
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.15),
                Pos2::new(c.x - s * 0.25, c.y + s * 0.4),
            );
            ls(
                Pos2::new(c.x - s * 0.25, c.y + s * 0.4),
                Pos2::new(c.x + s * 0.05, c.y + s * 0.15),
            );
        }
        "repost" => {
            // Two circular arrows
            ls(
                Pos2::new(c.x - s * 0.35, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.25, c.y - s * 0.2),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.25, c.y),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y + s * 0.1),
                Pos2::new(c.x - s * 0.35, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.25, c.y),
                Pos2::new(c.x - s * 0.35, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.25, c.y + s * 0.2),
                Pos2::new(c.x - s * 0.35, c.y + s * 0.1),
            );
        }
        "mention" => {
            // @ symbol
            painter.circle_stroke(c, s * 0.4, st);
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                "@",
                egui::FontId::proportional(s * 0.55),
                color,
            );
        }
        "hashtag" => {
            // # grid
            ls(
                Pos2::new(c.x - s * 0.15, c.y - s * 0.4),
                Pos2::new(c.x - s * 0.15, c.y + s * 0.4),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.4),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.4, c.y + s * 0.15),
            );
        }
        "trending" => {
            // Upward graph line with arrow
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.3),
                Pos2::new(c.x - s * 0.1, c.y),
            );
            ls(
                Pos2::new(c.x - s * 0.1, c.y),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.3),
            );
            // Arrow head
            ls(
                Pos2::new(c.x + s * 0.25, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.4, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.3),
            );
        }
        "viral" => {
            // Branching network
            painter.circle_filled(c, s * 0.08, color);
            // Branches
            ls(c, Pos2::new(c.x - s * 0.3, c.y - s * 0.3));
            ls(c, Pos2::new(c.x + s * 0.3, c.y - s * 0.3));
            ls(c, Pos2::new(c.x - s * 0.35, c.y + s * 0.2));
            ls(c, Pos2::new(c.x + s * 0.35, c.y + s * 0.2));
            painter.circle_filled(Pos2::new(c.x - s * 0.3, c.y - s * 0.3), s * 0.05, color);
            painter.circle_filled(Pos2::new(c.x + s * 0.3, c.y - s * 0.3), s * 0.05, color);
            painter.circle_filled(Pos2::new(c.x - s * 0.35, c.y + s * 0.2), s * 0.05, color);
            painter.circle_filled(Pos2::new(c.x + s * 0.35, c.y + s * 0.2), s * 0.05, color);
        }
        "follower" => {
            // Person silhouette with + badge
            painter.circle_stroke(Pos2::new(c.x - s * 0.1, c.y - s * 0.2), s * 0.15, st);
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.35),
                Pos2::new(c.x - s * 0.1, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y + s * 0.35),
                Pos2::new(c.x - s * 0.1, c.y + s * 0.05),
            );
            // + badge
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.15),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.05),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.05),
            );
        }
        "following" => {
            // Person with check
            painter.circle_stroke(Pos2::new(c.x - s * 0.1, c.y - s * 0.2), s * 0.15, st);
            ls(
                Pos2::new(c.x - s * 0.4, c.y + s * 0.35),
                Pos2::new(c.x - s * 0.1, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y + s * 0.35),
                Pos2::new(c.x - s * 0.1, c.y + s * 0.05),
            );
            // Check
            ls(
                Pos2::new(c.x + s * 0.2, c.y - s * 0.05),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.05),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.05),
                Pos2::new(c.x + s * 0.45, c.y - s * 0.15),
            );
        }
        "profile" => {
            // Rounded square with person
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y - s * 0.4),
                    Pos2::new(c.x + s * 0.4, c.y + s * 0.4),
                ),
                s * 0.1,
                st,
                egui::StrokeKind::Middle,
            );
            painter.circle_stroke(Pos2::new(c.x, c.y - s * 0.12), s * 0.15, st);
            ls(
                Pos2::new(c.x - s * 0.25, c.y + s * 0.35),
                Pos2::new(c.x, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.25, c.y + s * 0.35),
                Pos2::new(c.x, c.y + s * 0.1),
            );
        }
        "bio" => {
            // Document with person icon
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.4),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.4),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            painter.circle_stroke(Pos2::new(c.x, c.y - s * 0.15), s * 0.12, st);
            // Text lines below
            ls(
                Pos2::new(c.x - s * 0.2, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.2, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.15, c.y + s * 0.22),
                Pos2::new(c.x + s * 0.15, c.y + s * 0.22),
            );
        }
        "story" => {
            // Circle with gradient ring (camera icon)
            painter.circle_stroke(c, s * 0.4, sth);
            painter.circle_stroke(c, s * 0.25, st);
            painter.circle_filled(c, s * 0.08, color);
        }
        "reel" => {
            // Film reel
            painter.circle_stroke(c, s * 0.4, st);
            painter.circle_stroke(c, s * 0.12, st);
            // Sprocket holes
            for angle in [0.0f32, 1.05, 2.09, 3.14, 4.19, 5.24] {
                painter.circle_filled(
                    Pos2::new(c.x + s * 0.27 * angle.cos(), c.y + s * 0.27 * angle.sin()),
                    s * 0.05,
                    color,
                );
            }
        }
        "live-stream" => {
            // Camera with broadcast waves
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.3, c.y - s * 0.15),
                    Pos2::new(c.x + s * 0.15, c.y + s * 0.2),
                ),
                s * 0.04,
                st,
                egui::StrokeKind::Middle,
            );
            // Lens triangle
            ls(
                Pos2::new(c.x + s * 0.15, c.y - s * 0.1),
                Pos2::new(c.x + s * 0.35, c.y - s * 0.2),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.25),
            );
            ls(
                Pos2::new(c.x + s * 0.35, c.y - s * 0.2),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.25),
            );
            // REC dot
            painter.circle_filled(Pos2::new(c.x - s * 0.1, c.y + s * 0.38), s * 0.06, color);
        }
        "notification-dot" => {
            // Bell outline with dot
            ls(
                Pos2::new(c.x, c.y - s * 0.4),
                Pos2::new(c.x - s * 0.3, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x, c.y - s * 0.4),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.3, c.y + s * 0.1),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x - s * 0.35, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.15),
            );
            // Dot
            painter.circle_filled(Pos2::new(c.x + s * 0.25, c.y - s * 0.3), s * 0.1, color);
        }
        "verified" => {
            // Badge circle with check
            painter.circle_stroke(c, s * 0.38, sth);
            ls(
                Pos2::new(c.x - s * 0.18, c.y),
                Pos2::new(c.x - s * 0.05, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x - s * 0.05, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.2, c.y - s * 0.15),
            );
        }
        "influencer" => {
            // Person with star
            painter.circle_stroke(Pos2::new(c.x - s * 0.1, c.y - s * 0.15), s * 0.15, st);
            ls(
                Pos2::new(c.x - s * 0.35, c.y + s * 0.35),
                Pos2::new(c.x - s * 0.1, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.15, c.y + s * 0.35),
                Pos2::new(c.x - s * 0.1, c.y + s * 0.1),
            );
            // Star
            painter.text(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.25),
                egui::Align2::CENTER_CENTER,
                "\u{2605}",
                egui::FontId::proportional(s * 0.3),
                color,
            );
        }
        "engagement" => {
            // Heart + chart bar
            painter.circle_filled(Pos2::new(c.x - s * 0.08, c.y - s * 0.25), s * 0.1, color);
            painter.circle_filled(Pos2::new(c.x + s * 0.08, c.y - s * 0.25), s * 0.1, color);
            ls(
                Pos2::new(c.x - s * 0.17, c.y - s * 0.2),
                Pos2::new(c.x, c.y),
            );
            ls(
                Pos2::new(c.x + s * 0.17, c.y - s * 0.2),
                Pos2::new(c.x, c.y),
            );
            // Bar chart below
            ls(
                Pos2::new(c.x - s * 0.25, c.y + s * 0.35),
                Pos2::new(c.x - s * 0.25, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x, c.y + s * 0.35),
                Pos2::new(c.x, c.y + s * 0.1),
            );
            ls(
                Pos2::new(c.x + s * 0.25, c.y + s * 0.35),
                Pos2::new(c.x + s * 0.25, c.y + s * 0.2),
            );
        }
        "reach" => {
            // Broadcast waves emanating from point
            painter.circle_filled(Pos2::new(c.x - s * 0.3, c.y), s * 0.06, color);
            // Concentric arcs (quarter circles via strokes)
            ls(
                Pos2::new(c.x - s * 0.1, c.y - s * 0.15),
                Pos2::new(c.x, c.y),
            );
            ls(
                Pos2::new(c.x, c.y),
                Pos2::new(c.x - s * 0.1, c.y + s * 0.15),
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y - s * 0.25),
                Pos2::new(c.x + s * 0.2, c.y),
            );
            ls(
                Pos2::new(c.x + s * 0.2, c.y),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.25),
            );
            ls(
                Pos2::new(c.x + s * 0.3, c.y - s * 0.35),
                Pos2::new(c.x + s * 0.4, c.y),
            );
            ls(
                Pos2::new(c.x + s * 0.4, c.y),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.35),
            );
        }
        "post" => {
            // Text card
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.35),
                    Pos2::new(c.x + s * 0.35, c.y + s * 0.35),
                ),
                s * 0.06,
                st,
                egui::StrokeKind::Middle,
            );
            // Lines of text
            ls(
                Pos2::new(c.x - s * 0.22, c.y - s * 0.18),
                Pos2::new(c.x + s * 0.22, c.y - s * 0.18),
            );
            ls(
                Pos2::new(c.x - s * 0.22, c.y - s * 0.03),
                Pos2::new(c.x + s * 0.22, c.y - s * 0.03),
            );
            ls(
                Pos2::new(c.x - s * 0.22, c.y + s * 0.12),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.12),
            );
        }
        "feed" => {
            // Stacked cards
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.3, c.y - s * 0.25),
                    Pos2::new(c.x + s * 0.3, c.y + s * 0.35),
                ),
                s * 0.05,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.35, c.y - s * 0.35),
                    Pos2::new(c.x + s * 0.35, c.y - s * 0.25),
                ),
                s * 0.03,
                st,
                egui::StrokeKind::Middle,
            );
            // Content line
            ls(
                Pos2::new(c.x - s * 0.18, c.y),
                Pos2::new(c.x + s * 0.18, c.y),
            );
            ls(
                Pos2::new(c.x - s * 0.18, c.y + s * 0.15),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.15),
            );
        }
        "timeline" => {
            // Vertical line with dots
            ls(
                Pos2::new(c.x, c.y - s * 0.45),
                Pos2::new(c.x, c.y + s * 0.45),
            );
            painter.circle_filled(Pos2::new(c.x, c.y - s * 0.25), s * 0.06, color);
            painter.circle_filled(Pos2::new(c.x, c.y), s * 0.06, color);
            painter.circle_filled(Pos2::new(c.x, c.y + s * 0.25), s * 0.06, color);
            // Side lines
            ls(
                Pos2::new(c.x, c.y - s * 0.25),
                Pos2::new(c.x + s * 0.3, c.y - s * 0.25),
            );
            ls(Pos2::new(c.x, c.y), Pos2::new(c.x - s * 0.3, c.y));
            ls(
                Pos2::new(c.x, c.y + s * 0.25),
                Pos2::new(c.x + s * 0.3, c.y + s * 0.25),
            );
        }
        "dm" => {
            // Paper airplane (direct message)
            ls(
                Pos2::new(c.x - s * 0.4, c.y),
                Pos2::new(c.x + s * 0.4, c.y - s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.4, c.y - s * 0.3),
                Pos2::new(c.x + s * 0.1, c.y + s * 0.3),
            );
            ls(
                Pos2::new(c.x + s * 0.1, c.y + s * 0.3),
                Pos2::new(c.x - s * 0.4, c.y),
            );
            // Fold line
            ls(
                Pos2::new(c.x + s * 0.4, c.y - s * 0.3),
                Pos2::new(c.x - s * 0.05, c.y + s * 0.05),
            );
        }
        "group-chat" => {
            // Multiple speech bubbles
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.4, c.y - s * 0.35),
                    Pos2::new(c.x + s * 0.1, c.y + s * 0.0),
                ),
                s * 0.08,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.1, c.y - s * 0.05),
                    Pos2::new(c.x + s * 0.4, c.y + s * 0.25),
                ),
                s * 0.08,
                st,
                egui::StrokeKind::Middle,
            );
            // Tail for first
            ls(
                Pos2::new(c.x - s * 0.3, c.y),
                Pos2::new(c.x - s * 0.35, c.y + s * 0.15),
            );
            // Tail for second
            ls(
                Pos2::new(c.x + s * 0.3, c.y + s * 0.25),
                Pos2::new(c.x + s * 0.35, c.y + s * 0.4),
            );
        }

        // ── Fallback ────────────────────────────────────────────────────────
        _ => {
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::splat(s * 0.6)),
                s * 0.05,
                st,
                egui::StrokeKind::Middle,
            );
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                "?",
                egui::FontId::proportional(s * 0.5),
                color,
            );
        }
    }
}

#[cfg(feature = "render")]
fn icon_doc_outline(painter: &egui::Painter, c: Pos2, s: f32, st: Stroke) {
    let pts = [
        Pos2::new(c.x - s * 0.35, c.y - s * 0.55),
        Pos2::new(c.x + s * 0.15, c.y - s * 0.55),
        Pos2::new(c.x + s * 0.35, c.y - s * 0.35),
        Pos2::new(c.x + s * 0.35, c.y + s * 0.55),
        Pos2::new(c.x - s * 0.35, c.y + s * 0.55),
    ];
    for i in 0..pts.len() {
        painter.line_segment([pts[i], pts[(i + 1) % pts.len()]], st);
    }
    painter.line_segment([pts[1], Pos2::new(c.x + s * 0.15, c.y - s * 0.35)], st);
    painter.line_segment([Pos2::new(c.x + s * 0.15, c.y - s * 0.35), pts[2]], st);
}
