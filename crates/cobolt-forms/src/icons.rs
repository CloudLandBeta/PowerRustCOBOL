// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Built-in vector icon catalogue for menu items (spec 018 R29/R30).
//!
//! Every icon is **pure vector shape data**, rendered by one engine — not
//! ad-hoc painter calls, and never a raster. That is what keeps 600+ icons
//! looking like one set:
//!
//! - **Resolution-independent.** Coordinates are `f32` on a 24-unit
//!   *reference* grid (y down, live area roughly 2..22 — the same convention
//!   professional stroke sets use). The renderer maps the grid onto the
//!   target rect's largest centred square, so the SAME icon paints a 16 px
//!   menu row, a 128 px tile, or a 512 px splash with proportional stroke
//!   weight and no quality loss. There is no raster asset anywhere.
//! - **Styleable.** [`draw_menu_icon_styled`] takes an [`IconStyle`]: any
//!   tint colour, an optional second **accent colour** (applied to the
//!   filled accent shapes), and an [`IconEffect`] — a soft **drop shadow**
//!   or a **neumorphic emboss** (light above, dark below, matching the IDE's
//!   Neumorphic surface style). [`draw_menu_icon`] is the plain-tint
//!   shorthand.
//! - **One stroke.** Everything is drawn with a single 1.5-unit stroke
//!   ([`STROKE_UNITS`]) — no heavy/light mixing. Professional icon sets keep
//!   one weight; so do we.
//! - **Round caps and joins.** egui strokes have no cap style, so the
//!   renderer plants a stroke-width dot on every authored vertex, giving the
//!   rounded, pen-drawn look. The SVG emitter uses real `round` caps.
//! - **Curves.** Paths support quadratic/cubic beziers and there is a
//!   first-class arc shape — curves are flattened by the engine, never
//!   hand-approximated per icon.
//! - **Fills are accents.** Only small dots and tiny arrowhead/wedge fills;
//!   an icon is otherwise pure line work.
//! - **Angles.** 0° = +x (right), 90° = +y (down, screen space); an arc
//!   sweeps linearly from its start angle to its end angle in whichever
//!   direction the pair implies.
//!
//! Icons are tinted with the caller's colour (typically the menu item's
//! foreground). Names are stored in menu sidecar YAML (`MenuItem.icon`), so
//! **names are a stable API** — designs may improve, names never change.

#[cfg(feature = "render")]
use egui::{Color32, Pos2, Rect, Stroke, Vec2};

/// The single stroke weight, in grid units (of 24).
pub const STROKE_UNITS: f32 = 1.5;

/// Full catalogue of available icon names, grouped by category. This is the
/// ONE source of truth: the menu editor's icon picker renders these groups
/// directly, and the tests walk it to prove every name has a drawing.
pub const MENU_ICON_CATEGORIES: &[(&str, &[&str])] = &[
    (
        "Document",
        &[
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
            "doc-image",
            "doc-code",
            "doc-zip",
            "doc-lock",
            "doc-search",
            "doc-chart",
            "doc-certificate",
            "doc-signature",
            "doc-attachment",
            "doc-history",
        ],
    ),
    (
        "Edit",
        &[
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
            "undo",
            "redo",
            "indent",
            "outdent",
            "align-left",
            "align-center",
            "align-right",
            "align-justify",
            "highlighter",
            "format-clear",
        ],
    ),
    (
        "Navigation",
        &[
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
            "arrows-up-down",
            "arrows-left-right",
            "corner-up-left",
            "corner-up-right",
            "double-chevron-left",
            "double-chevron-right",
            "map-pin",
            "map",
            "crosshair",
            "sitemap",
        ],
    ),
    (
        "Action",
        &[
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
            "unlink",
            "power",
            "pin",
            "unpin",
            "drag-handle",
            "broom",
            "magic-wand",
            "lightning",
            "repeat",
            "shuffle",
            "timer-start",
            "stamp",
        ],
    ),
    (
        "UI/View",
        &[
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
            "columns",
            "rows",
            "sidebar-left",
            "sidebar-right",
            "split-view",
            "layers",
            "layout-dashboard",
            "maximize",
            "minimize",
            "picture-in-picture",
        ],
    ),
    (
        "Communication",
        &[
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
            "phone-incoming",
            "phone-outgoing",
            "phone-missed",
            "voicemail",
            "chat-dots",
            "chat-check",
            "broadcast",
            "antenna",
            "satellite",
            "megaphone",
        ],
    ),
    (
        "Social",
        &[
            "heart",
            "star",
            "thumbs-up",
            "thumbs-down",
            "bookmark",
            "flag",
            "gift",
            "trophy",
            "crown",
            "diamond",
            "balloon",
            "party",
            "handshake",
            "ribbon",
        ],
    ),
    (
        "People",
        &[
            "user",
            "users",
            "user-plus",
            "user-minus",
            "user-check",
            "user-circle",
            "user-x",
            "user-gear",
            "user-shield",
            "user-star",
            "team",
            "id-badge",
            "contact-book",
            "presenter",
        ],
    ),
    (
        "Media",
        &[
            "play",
            "pause",
            "stop",
            "skip-forward",
            "skip-back",
            "volume",
            "volume-off",
            "music",
            "record",
            "eject",
            "fast-forward",
            "rewind",
            "playlist",
            "microphone",
            "headphones",
            "camera",
            "film",
            "image",
        ],
    ),
    (
        "Data",
        &[
            "database",
            "chart-bar",
            "chart-line",
            "chart-pie",
            "table",
            "filter",
            "sort-asc",
            "sort-desc",
            "chart-area",
            "chart-scatter",
            "chart-donut",
            "chart-gauge",
            "pivot-table",
            "data-flow",
            "api",
            "cloud-database",
            "histogram",
            "treemap",
            "funnel",
            "kpi",
        ],
    ),
    (
        "System",
        &[
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
            "gears",
            "hammer",
            "screwdriver",
            "sliders",
            "toggle-on",
            "toggle-off",
            "server",
            "network",
            "wifi",
            "hard-drive",
            "plugin",
            "memory-chip",
        ],
    ),
    (
        "Status",
        &[
            "info-circle",
            "warning-triangle",
            "error-circle",
            "help-circle",
            "check-circle",
            "x-circle",
            "clock",
            "calendar",
            "hourglass",
            "stopwatch",
            "alarm",
            "calendar-check",
            "calendar-x",
            "progress",
            "loading",
            "battery-full",
            "battery-low",
            "traffic-light",
        ],
    ),
    (
        "Commerce",
        &[
            "cart",
            "credit-card",
            "wallet",
            "receipt",
            "tag",
            "percent",
            "store",
            "basket",
            "cash-register",
            "gift-card",
            "voucher",
            "coupon",
            "discount-badge",
            "open-sign",
            "shopping-bag",
            "cart-plus",
            "cart-minus",
            "point-of-sale",
        ],
    ),
    (
        "File/Folder",
        &[
            "folder",
            "folder-open",
            "folder-plus",
            "archive",
            "trash",
            "printer",
            "folder-minus",
            "folder-lock",
            "folder-search",
            "folder-sync",
            "scanner",
            "shredder",
            "file-cabinet",
            "paperclip",
        ],
    ),
    (
        "Payroll",
        &[
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
            "payroll-raise",
            "payroll-severance",
            "payroll-stipend",
            "payroll-allowance",
            "payroll-holiday-pay",
            "payroll-shift-differential",
            "payroll-payday",
            "payroll-bank-details",
            "payroll-union-dues",
            "payroll-audit",
        ],
    ),
    (
        "Receivables",
        &[
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
            "invoice-recurring",
            "invoice-dispute",
            "promissory-note",
            "payment-reminder",
            "grace-period",
            "late-fee",
            "credit-score",
            "cash-application",
            "collections-call",
            "guarantor",
        ],
    ),
    (
        "Payments",
        &[
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
            "payment-mobile",
            "payment-qr",
            "payment-contactless",
            "payment-crypto",
            "payment-standing-order",
            "payment-authorization",
            "payment-capture",
            "payment-limit",
            "payment-receipt",
            "payment-schedule",
        ],
    ),
    (
        "Stock Control",
        &[
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
            "sku",
            "stock-aging",
            "stock-return",
            "stock-damage",
            "stock-quarantine",
            "stock-picking",
            "stock-putaway",
            "kitting",
            "batch-tracking",
            "min-max-level",
        ],
    ),
    (
        "Transportation",
        &[
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
            "airplane-takeoff",
            "box-truck",
            "tanker-ship",
            "ferry",
            "cargo-train",
            "metro",
            "tram",
            "harbor",
            "runway",
            "traffic-cone",
        ],
    ),
    (
        "Logistics",
        &[
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
            "inbound",
            "outbound",
            "route-planning",
            "proof-of-delivery",
            "freight",
            "pallet-jack",
            "cold-chain",
            "drop-shipping",
            "reverse-logistics",
            "supply-chain",
        ],
    ),
    (
        "Financial",
        &[
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
            "rupee",
            "won",
            "lira",
            "treasury-bond",
            "portfolio",
            "capital",
            "depreciation",
            "amortization",
            "equity",
            "liability",
            "asset",
            "budget",
        ],
    ),
    (
        "Social Media",
        &[
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
            "subscriber",
            "unfollow",
            "block-user",
            "mute",
            "poll",
            "emoji-react",
            "gif",
            "tag-friend",
            "save-post",
            "analytics-social",
        ],
    ),
    (
        "Departments",
        &[
            "dept-hr",
            "dept-finance",
            "dept-accounting",
            "dept-it",
            "dept-legal",
            "dept-marketing",
            "dept-sales",
            "dept-operations",
            "dept-manufacturing",
            "dept-engineering",
            "dept-rnd",
            "dept-procurement",
            "dept-customer-service",
            "dept-quality",
            "dept-executive",
            "dept-security",
            "dept-maintenance",
            "dept-training",
            "dept-logistics",
            "dept-warehouse",
            "dept-compliance",
            "dept-facilities",
            "dept-health-safety",
            "dept-pr",
            "dept-treasury",
            "dept-audit",
        ],
    ),
    (
        "Transactions",
        &[
            "buy",
            "sell",
            "withdraw",
            "deposit",
            "return",
            "exchange",
            "order",
            "delivery",
            "borrow",
            "lend",
            "showback",
            "chargeback",
            "quote",
            "subscription",
            "renewal",
            "cancellation",
            "auction",
            "bid",
            "settlement",
            "layaway",
            "preorder",
            "backorder",
            "cash-on-delivery",
            "trade-in",
            "donation",
            "recurring-charge",
        ],
    ),
    (
        "Vehicles",
        &[
            "car",
            "bus",
            "motorcycle",
            "bicycle",
            "scooter",
            "taxi",
            "pickup",
            "suv",
            "minivan",
            "ambulance",
            "fire-truck",
            "police-car",
            "tractor",
            "excavator",
            "bulldozer",
            "dump-truck",
            "cement-mixer",
            "tow-truck",
            "trailer",
            "rv",
            "golf-cart",
            "snowmobile",
            "jet-ski",
            "sailboat",
            "hot-air-balloon",
            "cable-car",
        ],
    ),
    (
        "Devices",
        &[
            "desktop-computer",
            "laptop",
            "monitor",
            "all-in-one",
            "computer-tower",
            "mainframe",
            "terminal-crt",
            "retro-computer",
            "punch-card",
            "tablet",
            "smartphone",
            "smartwatch",
            "fitness-band",
            "vr-headset",
            "earbuds",
        ],
    ),
    (
        "SaaS",
        &[
            "saas-crm",
            "saas-erp",
            "saas-hrtech",
            "saas-bi",
            "saas-lms",
            "saas-cms",
            "saas-wcm",
            "saas-itsm",
            "saas-plm",
            "saas-scm",
            "saas-pos",
            "saas-chatbot",
        ],
    ),
    (
        "PaaS",
        &[
            "apaas",
            "dbpaas",
            "ipaas",
            "mpaas",
            "cpaas",
            "baas",
            "mbaas",
            "faas",
            "secpaas",
            "aiaas",
        ],
    ),
    (
        "ERP Modules",
        &[
            "erp-fi",
            "erp-co",
            "erp-sd",
            "erp-mm",
            "erp-pp",
            "erp-qm",
            "erp-pm",
            "erp-scm",
        ],
    ),
    (
        "Military",
        &[
            "tank",
            "apc",
            "humvee",
            "submarine",
            "warship",
            "aircraft-carrier",
            "patrol-boat",
            "fighter-jet",
            "bomber",
            "military-helicopter",
            "military-drone",
            "rocket",
            "missile",
            "torpedo",
            "artillery",
            "mortar",
            "bullet",
            "ammo-box",
            "magazine-clip",
            "rifle",
            "pistol",
            "machine-gun",
            "sniper-rifle",
            "grenade",
            "landmine",
            "military-shield",
            "armor-vest",
            "armor-plate",
            "combat-helmet",
            "uniform-jacket",
            "uniform-pants",
            "combat-boots",
            "dog-tags",
            "medal",
            "rank-chevrons",
            "radar",
            "periscope",
            "night-vision",
            "parachute",
            "bunker",
        ],
    ),
];

/// Every icon name, in catalogue order.
pub fn menu_icon_names() -> impl Iterator<Item = &'static str> {
    MENU_ICON_CATEGORIES
        .iter()
        .flat_map(|(_, names)| names.iter().copied())
}

// ── Shape DSL ───────────────────────────────────────────────────────────────

/// One step of a path, in grid coordinates. The first op of a path is its
/// start point (its target point, regardless of variant).
#[derive(Clone, Copy, Debug)]
pub(crate) enum PathOp {
    /// Straight line to (x, y).
    L(f32, f32),
    /// Quadratic bezier: control (cx, cy), end (x, y).
    Q(f32, f32, f32, f32),
    /// Cubic bezier: controls (c1x, c1y), (c2x, c2y), end (x, y).
    B(f32, f32, f32, f32, f32, f32),
}

/// One drawable shape of an icon, in grid coordinates.
#[derive(Clone, Debug)]
pub(crate) enum IconShape {
    /// Open stroked path.
    Stroke(Vec<PathOp>),
    /// Closed stroked path.
    StrokeClosed(Vec<PathOp>),
    /// Closed filled path (small accents only — keep it convex).
    FillPath(Vec<PathOp>),
    /// Stroked circle: cx, cy, r.
    Circle(f32, f32, f32),
    /// Filled dot: cx, cy, r.
    Dot(f32, f32, f32),
    /// Stroked arc: cx, cy, r, start°, end° (0° = +x, 90° = +y/down; the
    /// sweep interpolates linearly from start to end).
    Arc(f32, f32, f32, f32, f32),
    /// Stroked rounded rect: x, y, w, h, corner radius.
    RRect(f32, f32, f32, f32, f32),
    /// Filled rounded rect: x, y, w, h, corner radius.
    RRectFill(f32, f32, f32, f32, f32),
}

use PathOp::{B, L, Q};

/// Open polyline through the given points.
fn p(pts: &[(f32, f32)]) -> IconShape {
    IconShape::Stroke(pts.iter().map(|&(x, y)| L(x, y)).collect())
}
/// Closed stroked polygon through the given points.
fn pc(pts: &[(f32, f32)]) -> IconShape {
    IconShape::StrokeClosed(pts.iter().map(|&(x, y)| L(x, y)).collect())
}
/// Closed filled polygon (accent) through the given points.
fn pf(pts: &[(f32, f32)]) -> IconShape {
    IconShape::FillPath(pts.iter().map(|&(x, y)| L(x, y)).collect())
}
/// Open stroked path with curves.
fn path(ops: Vec<PathOp>) -> IconShape {
    IconShape::Stroke(ops)
}
/// Closed stroked path with curves.
fn pathc(ops: Vec<PathOp>) -> IconShape {
    IconShape::StrokeClosed(ops)
}
/// Stroked circle.
fn c(cx: f32, cy: f32, r: f32) -> IconShape {
    IconShape::Circle(cx, cy, r)
}
/// Filled dot.
fn d(cx: f32, cy: f32, r: f32) -> IconShape {
    IconShape::Dot(cx, cy, r)
}
/// Stroked arc (see [`IconShape::Arc`] for the angle convention).
fn a(cx: f32, cy: f32, r: f32, a0: f32, a1: f32) -> IconShape {
    IconShape::Arc(cx, cy, r, a0, a1)
}
/// Stroked rounded rect.
fn rr(x: f32, y: f32, w: f32, h: f32, rad: f32) -> IconShape {
    IconShape::RRect(x, y, w, h, rad)
}
/// Filled rounded rect (accent).
fn rrf(x: f32, y: f32, w: f32, h: f32, rad: f32) -> IconShape {
    IconShape::RRectFill(x, y, w, h, rad)
}

// ── Shared motifs ───────────────────────────────────────────────────────────
// Larger drawings reused across categories, so a "document" or a "person"
// looks identical wherever it appears.

/// A document page with a folded top-right corner. Returns outline + fold.
fn page() -> [IconShape; 2] {
    [
        pathc(vec![
            L(6.5, 2.5),
            L(13.0, 2.5),
            L(17.5, 7.0),
            L(17.5, 21.5),
            L(6.5, 21.5),
        ]),
        p(&[(13.0, 2.5), (13.0, 7.0), (17.5, 7.0)]),
    ]
}

/// A person: head + shoulders, centred on `cx`, scaled by `k` (1.0 = the
/// standard single-user size), with the shoulder baseline at `base`.
fn person(cx: f32, base: f32, k: f32) -> [IconShape; 2] {
    let r = 3.4 * k;
    let head_cy = base - 9.2 * k;
    let w = 6.2 * k;
    let top = base - 4.6 * k;
    [
        c(cx, head_cy, r),
        path(vec![
            L(cx - w, base),
            Q(cx - w, top, cx, top),
            Q(cx + w, top, cx + w, base),
        ]),
    ]
}

/// A banknote: rounded rect + centre coin ring.
fn banknote() -> [IconShape; 2] {
    [rr(2.5, 7.0, 19.0, 10.0, 1.5), c(12.0, 12.0, 2.6)]
}

/// A circular badge (the container for symbol-in-circle icons).
fn badge() -> IconShape {
    c(12.0, 12.0, 9.0)
}

/// A hand-drawn dollar glyph centred at (cx, cy), scaled by `k`
/// (1.0 ≈ fills a 9-radius badge).
fn dollar_glyph(cx: f32, cy: f32, k: f32) -> [IconShape; 2] {
    [
        path(vec![
            L(cx + 3.0 * k, cy - 3.4 * k),
            Q(cx + 2.6 * k, cy - 5.0 * k, cx, cy - 5.0 * k),
            Q(cx - 3.0 * k, cy - 5.0 * k, cx - 3.0 * k, cy - 2.5 * k),
            Q(cx - 3.0 * k, cy, cx, cy),
            Q(cx + 3.0 * k, cy, cx + 3.0 * k, cy + 2.5 * k),
            Q(cx + 3.0 * k, cy + 5.0 * k, cx, cy + 5.0 * k),
            Q(cx - 2.6 * k, cy + 5.0 * k, cx - 3.0 * k, cy + 3.4 * k),
        ]),
        p(&[(cx, cy - 6.6 * k), (cx, cy + 6.6 * k)]),
    ]
}

/// A truck seen from the side: cargo box, cab, two wheels.
fn truck_body() -> [IconShape; 4] {
    [
        rr(2.0, 7.0, 12.5, 9.0, 1.0),
        path(vec![
            L(14.5, 9.5),
            L(18.5, 9.5),
            L(21.5, 13.0),
            L(21.5, 16.0),
            L(14.5, 16.0),
        ]),
        c(7.0, 18.0, 2.2),
        c(17.5, 18.0, 2.2),
    ]
}

/// Extend a shape list with a slice of shapes.
fn add(v: &mut Vec<IconShape>, more: impl IntoIterator<Item = IconShape>) {
    v.extend(more);
}

// ── Catalogue dispatch ──────────────────────────────────────────────────────

/// The drawing for an icon name, or `None` for an unknown name.
pub(crate) fn icon_shapes(name: &str) -> Option<Vec<IconShape>> {
    base_shapes(name)
        .or_else(|| view_comm_shapes(name))
        .or_else(|| people_media_data_shapes(name))
        .or_else(|| system_commerce_shapes(name))
        .or_else(|| payroll_receivables_shapes(name))
        .or_else(|| payments_stock_shapes(name))
        .or_else(|| transport_logistics_shapes(name))
        .or_else(|| financial_social_shapes(name))
        .or_else(|| departments_transactions_shapes(name))
        .or_else(|| vehicles_military_shapes(name))
        .or_else(|| devices_cloud_shapes(name))
}

/// Document, Edit, Navigation, Action.
#[rustfmt::skip]
fn base_shapes(name: &str) -> Option<Vec<IconShape>> {
    Some(match name {
        // ── Document ───────────────────────────────────────────────────────
        "doc-new" => {
            let mut v = Vec::new();
            add(&mut v, page());
            v.push(p(&[(12.0, 11.0), (12.0, 17.0)]));
            v.push(p(&[(9.0, 14.0), (15.0, 14.0)]));
            v
        }
        "doc-open" => {
            let mut v = Vec::new();
            add(&mut v, page());
            v.push(p(&[(9.0, 14.0), (15.0, 14.0)]));
            v.push(p(&[(12.5, 11.5), (15.0, 14.0), (12.5, 16.5)]));
            v
        }
        "doc-save" => vec![
            pathc(vec![L(4.5, 4.5), L(15.5, 4.5), L(19.5, 8.5), L(19.5, 19.5), L(4.5, 19.5)]),
            p(&[(8.0, 4.5), (8.0, 9.0), (15.0, 9.0), (15.0, 4.5)]),
            rr(8.0, 12.5, 8.0, 6.0, 0.8),
        ],
        "doc-save-as" => vec![
            pathc(vec![L(3.5, 4.0), L(14.5, 4.0), L(18.5, 8.0), L(18.5, 18.5), L(3.5, 18.5)]),
            p(&[(6.5, 4.0), (6.5, 8.5), (13.0, 8.5), (13.0, 4.0)]),
            p(&[(19.0, 16.0), (19.0, 22.0)]),
            p(&[(16.0, 19.0), (22.0, 19.0)]),
        ],
        "doc-copy" => vec![rr(4.0, 3.0, 11.0, 14.0, 1.5), rr(9.0, 7.0, 11.0, 14.0, 1.5)],
        "doc-blank" => page().to_vec(),
        "doc-text" => {
            let mut v = Vec::new();
            add(&mut v, page());
            v.push(p(&[(9.5, 12.0), (14.5, 12.0)]));
            v.push(p(&[(9.5, 15.0), (14.5, 15.0)]));
            v.push(p(&[(9.5, 18.0), (12.5, 18.0)]));
            v
        }
        "doc-pdf" => {
            // A drawn "P" — never a font glyph; fonts vary, drawings don't.
            let mut v = Vec::new();
            add(&mut v, page());
            v.push(p(&[(10.0, 17.5), (10.0, 10.5)]));
            v.push(path(vec![
                L(10.0, 10.5),
                L(12.8, 10.5),
                Q(14.8, 10.5, 14.8, 12.5),
                Q(14.8, 14.5, 12.8, 14.5),
                L(10.0, 14.5),
            ]));
            v
        }
        "doc-spreadsheet" => {
            let mut v = Vec::new();
            add(&mut v, page());
            v.push(rr(8.5, 10.5, 7.0, 8.5, 0.5));
            v.push(p(&[(8.5, 14.75), (15.5, 14.75)]));
            v.push(p(&[(12.0, 10.5), (12.0, 19.0)]));
            v
        }
        "doc-stack" => vec![
            rr(4.5, 7.5, 12.5, 13.0, 1.5),
            p(&[(7.0, 4.5), (16.0, 4.5), (19.5, 8.0), (19.5, 16.5)]),
        ],
        "doc-image" => {
            let mut v = Vec::new();
            add(&mut v, page());
            v.push(c(10.7, 11.5, 1.3));
            v.push(p(&[(8.5, 18.5), (11.5, 14.5), (13.3, 16.6), (15.0, 14.8), (16.8, 18.5)]));
            v
        }
        "doc-code" => {
            let mut v = Vec::new();
            add(&mut v, page());
            v.push(p(&[(10.5, 12.0), (8.5, 14.5), (10.5, 17.0)]));
            v.push(p(&[(13.5, 12.0), (15.5, 14.5), (13.5, 17.0)]));
            v
        }
        "doc-zip" => {
            let mut v = Vec::new();
            add(&mut v, page());
            v.push(p(&[(11.0, 3.8), (11.0, 5.2)]));
            v.push(p(&[(11.0, 7.0), (11.0, 8.4)]));
            v.push(p(&[(11.0, 10.2), (11.0, 11.6)]));
            v.push(rr(9.5, 13.2, 3.0, 3.4, 0.8));
            v
        }
        "doc-lock" => {
            let mut v = Vec::new();
            add(&mut v, page());
            v.push(rr(9.3, 13.0, 5.4, 5.0, 1.0));
            v.push(a(12.0, 13.0, 1.9, 180.0, 360.0));
            v
        }
        "doc-search" => {
            let mut v = Vec::new();
            add(&mut v, page());
            v.push(c(11.5, 13.0, 3.0));
            v.push(p(&[(13.7, 15.2), (16.3, 17.8)]));
            v
        }
        "doc-chart" => {
            let mut v = Vec::new();
            add(&mut v, page());
            v.push(p(&[(9.5, 18.0), (9.5, 15.0)]));
            v.push(p(&[(12.0, 18.0), (12.0, 11.5)]));
            v.push(p(&[(14.5, 18.0), (14.5, 13.5)]));
            v
        }
        "doc-certificate" => {
            let mut v = Vec::new();
            add(&mut v, page());
            v.push(c(12.0, 13.5, 2.5));
            v.push(p(&[(10.9, 15.7), (9.9, 19.3)]));
            v.push(p(&[(13.1, 15.7), (14.1, 19.3)]));
            v
        }
        "doc-signature" => {
            let mut v = Vec::new();
            add(&mut v, page());
            v.push(path(vec![
                L(8.5, 16.5),
                Q(10.5, 12.5, 11.5, 15.5),
                Q(12.3, 17.5, 13.3, 15.5),
                L(15.5, 15.5),
            ]));
            v
        }
        "doc-attachment" => {
            let mut v = Vec::new();
            add(&mut v, page());
            v.push(path(vec![
                L(9.7, 17.3), L(9.7, 12.0),
                Q(9.7, 9.7, 12.0, 9.7), Q(14.3, 9.7, 14.3, 12.0),
                L(14.3, 15.4), Q(14.3, 16.9, 12.9, 16.9),
                Q(11.5, 16.9, 11.5, 15.4), L(11.5, 12.4),
            ]));
            v
        }
        "doc-history" => {
            let mut v = Vec::new();
            add(&mut v, page());
            v.push(c(12.0, 14.0, 3.4));
            v.push(p(&[(12.0, 12.2), (12.0, 14.0), (13.6, 15.0)]));
            v
        }

        // ── Edit ───────────────────────────────────────────────────────────
        "scissors" => vec![
            c(7.5, 17.5, 2.5),
            c(16.5, 17.5, 2.5),
            p(&[(9.3, 15.7), (17.0, 4.5)]),
            p(&[(14.7, 15.7), (7.0, 4.5)]),
            d(12.0, 12.6, 0.9),
        ],
        "clipboard-copy" => vec![
            rr(5.5, 4.5, 13.0, 17.0, 1.5),
            rr(9.5, 2.8, 5.0, 3.4, 1.0),
            rr(9.0, 10.5, 6.0, 7.0, 0.8),
        ],
        "clipboard-paste" => vec![
            rr(5.5, 4.5, 13.0, 17.0, 1.5),
            rr(9.5, 2.8, 5.0, 3.4, 1.0),
            p(&[(12.0, 9.5), (12.0, 17.0)]),
            p(&[(9.5, 14.5), (12.0, 17.0), (14.5, 14.5)]),
        ],
        "pencil" => vec![
            pathc(vec![L(4.5, 19.5), L(4.5, 16.5), L(15.5, 5.5), L(18.5, 8.5), L(7.5, 19.5)]),
            p(&[(13.5, 7.5), (16.5, 10.5)]),
        ],
        "eraser" => vec![
            path(vec![
                L(5.0, 15.8), L(13.0, 7.8),
                Q(14.0, 6.8, 15.0, 7.8), L(18.7, 11.5),
                Q(19.7, 12.5, 18.7, 13.5), L(12.5, 19.7),
                L(8.8, 19.7), L(5.0, 16.0),
            ]),
            p(&[(9.5, 11.3), (15.2, 17.0)]),
            p(&[(15.0, 19.7), (20.5, 19.7)]),
        ],
        "pen" => vec![
            pc(&[(4.5, 19.5), (6.0, 13.0), (14.0, 5.0), (19.0, 10.0), (11.0, 18.0)]),
            p(&[(4.5, 19.5), (10.7, 13.3)]),
            c(11.6, 12.4, 1.2),
        ],
        "brush" => vec![
            rr(10.5, 2.5, 3.0, 7.5, 1.0),
            rr(9.5, 10.0, 5.0, 3.5, 0.8),
            pc(&[(9.5, 13.5), (14.5, 13.5), (16.5, 19.5), (7.5, 19.5)]),
        ],
        "type-text" => vec![
            p(&[(6.5, 9.0), (6.5, 6.0), (17.5, 6.0), (17.5, 9.0)]),
            p(&[(12.0, 6.0), (12.0, 18.5)]),
            p(&[(9.5, 18.5), (14.5, 18.5)]),
        ],
        "bold" => vec![
            p(&[(8.0, 5.0), (8.0, 19.0)]),
            path(vec![
                L(8.0, 5.0), L(13.0, 5.0),
                Q(16.5, 5.0, 16.5, 8.5), Q(16.5, 12.0, 13.0, 12.0), L(8.0, 12.0),
            ]),
            path(vec![
                L(8.0, 12.0), L(13.7, 12.0),
                Q(17.5, 12.0, 17.5, 15.5), Q(17.5, 19.0, 13.7, 19.0), L(8.0, 19.0),
            ]),
        ],
        "italic" => vec![
            p(&[(11.0, 5.0), (19.0, 5.0)]),
            p(&[(5.0, 19.0), (13.0, 19.0)]),
            p(&[(15.0, 5.0), (9.0, 19.0)]),
        ],
        "underline" => vec![
            path(vec![
                L(7.0, 4.5), L(7.0, 11.0),
                Q(7.0, 16.0, 12.0, 16.0), Q(17.0, 16.0, 17.0, 11.0), L(17.0, 4.5),
            ]),
            p(&[(6.0, 19.5), (18.0, 19.5)]),
        ],
        "strikethrough" => vec![
            path(vec![
                L(16.5, 6.5),
                Q(16.0, 4.5, 12.0, 4.5),
                Q(8.0, 4.5, 8.0, 7.2),
                Q(8.0, 9.6, 11.5, 10.6),
            ]),
            path(vec![
                L(12.5, 13.4),
                Q(16.0, 14.4, 16.0, 17.0),
                Q(16.0, 19.7, 12.0, 19.7),
                Q(8.3, 19.7, 7.5, 17.5),
            ]),
            p(&[(5.0, 12.0), (19.0, 12.0)]),
        ],
        "undo" => vec![
            p(&[(8.5, 13.5), (4.0, 9.0), (8.5, 4.5)]),
            p(&[(4.0, 9.0), (13.5, 9.0)]),
            a(13.5, 14.5, 5.5, 270.0, 450.0),
            p(&[(13.5, 20.0), (10.5, 20.0)]),
        ],
        "redo" => vec![
            p(&[(15.5, 13.5), (20.0, 9.0), (15.5, 4.5)]),
            p(&[(20.0, 9.0), (10.5, 9.0)]),
            a(10.5, 14.5, 5.5, 270.0, 90.0),
            p(&[(10.5, 20.0), (13.5, 20.0)]),
        ],
        "indent" => vec![
            p(&[(4.0, 5.0), (20.0, 5.0)]),
            p(&[(11.0, 9.5), (20.0, 9.5)]),
            p(&[(11.0, 14.0), (20.0, 14.0)]),
            p(&[(4.0, 18.5), (20.0, 18.5)]),
            pc(&[(4.0, 9.0), (8.0, 11.75), (4.0, 14.5)]),
        ],
        "outdent" => vec![
            p(&[(4.0, 5.0), (20.0, 5.0)]),
            p(&[(11.0, 9.5), (20.0, 9.5)]),
            p(&[(11.0, 14.0), (20.0, 14.0)]),
            p(&[(4.0, 18.5), (20.0, 18.5)]),
            pc(&[(8.0, 9.0), (4.0, 11.75), (8.0, 14.5)]),
        ],
        "align-left" => vec![
            p(&[(4.0, 6.0), (20.0, 6.0)]),
            p(&[(4.0, 10.0), (14.0, 10.0)]),
            p(&[(4.0, 14.0), (20.0, 14.0)]),
            p(&[(4.0, 18.0), (14.0, 18.0)]),
        ],
        "align-center" => vec![
            p(&[(4.0, 6.0), (20.0, 6.0)]),
            p(&[(7.0, 10.0), (17.0, 10.0)]),
            p(&[(4.0, 14.0), (20.0, 14.0)]),
            p(&[(7.0, 18.0), (17.0, 18.0)]),
        ],
        "align-right" => vec![
            p(&[(4.0, 6.0), (20.0, 6.0)]),
            p(&[(10.0, 10.0), (20.0, 10.0)]),
            p(&[(4.0, 14.0), (20.0, 14.0)]),
            p(&[(10.0, 18.0), (20.0, 18.0)]),
        ],
        "align-justify" => vec![
            p(&[(4.0, 6.0), (20.0, 6.0)]),
            p(&[(4.0, 10.0), (20.0, 10.0)]),
            p(&[(4.0, 14.0), (20.0, 14.0)]),
            p(&[(4.0, 18.0), (20.0, 18.0)]),
        ],
        "highlighter" => vec![
            pc(&[(11.0, 4.0), (19.0, 9.0), (13.5, 15.5), (7.5, 11.5)]),
            pc(&[(7.5, 11.5), (13.5, 15.5), (11.5, 17.5), (6.0, 14.0)]),
            p(&[(3.5, 20.5), (16.0, 20.5)]),
        ],
        "format-clear" => vec![
            p(&[(5.5, 8.0), (5.5, 5.5), (14.5, 5.5), (14.5, 8.0)]),
            p(&[(10.0, 5.5), (10.0, 18.5)]),
            p(&[(8.0, 18.5), (12.0, 18.5)]),
            p(&[(15.5, 14.5), (20.5, 19.5)]),
            p(&[(20.5, 14.5), (15.5, 19.5)]),
        ],

        // ── Navigation ─────────────────────────────────────────────────────
        "arrow-left" => vec![
            p(&[(19.5, 12.0), (4.5, 12.0)]),
            p(&[(10.5, 6.0), (4.5, 12.0), (10.5, 18.0)]),
        ],
        "arrow-right" => vec![
            p(&[(4.5, 12.0), (19.5, 12.0)]),
            p(&[(13.5, 6.0), (19.5, 12.0), (13.5, 18.0)]),
        ],
        "arrow-up" => vec![
            p(&[(12.0, 19.5), (12.0, 4.5)]),
            p(&[(6.0, 10.5), (12.0, 4.5), (18.0, 10.5)]),
        ],
        "arrow-down" => vec![
            p(&[(12.0, 4.5), (12.0, 19.5)]),
            p(&[(6.0, 13.5), (12.0, 19.5), (18.0, 13.5)]),
        ],
        "chevron-left" => vec![p(&[(15.0, 5.0), (8.0, 12.0), (15.0, 19.0)])],
        "chevron-right" => vec![p(&[(9.0, 5.0), (16.0, 12.0), (9.0, 19.0)])],
        "chevron-up" => vec![p(&[(5.0, 15.0), (12.0, 8.0), (19.0, 15.0)])],
        "chevron-down" => vec![p(&[(5.0, 9.0), (12.0, 16.0), (19.0, 9.0)])],
        "home" => vec![
            p(&[(4.5, 11.5), (12.0, 4.5), (19.5, 11.5)]),
            p(&[(6.5, 10.0), (6.5, 20.0), (17.5, 20.0), (17.5, 10.0)]),
            p(&[(10.5, 20.0), (10.5, 15.0), (13.5, 15.0), (13.5, 20.0)]),
        ],
        "external-link" => vec![
            path(vec![
                L(10.0, 5.5), L(6.5, 5.5),
                Q(4.5, 5.5, 4.5, 7.5), L(4.5, 17.5),
                Q(4.5, 19.5, 6.5, 19.5), L(16.5, 19.5),
                Q(18.5, 19.5, 18.5, 17.5), L(18.5, 14.0),
            ]),
            p(&[(13.0, 11.0), (20.5, 3.5)]),
            p(&[(15.5, 3.5), (20.5, 3.5), (20.5, 8.5)]),
        ],
        "arrows-up-down" => vec![
            p(&[(8.0, 19.5), (8.0, 4.5)]),
            p(&[(4.5, 8.0), (8.0, 4.5), (11.5, 8.0)]),
            p(&[(16.0, 4.5), (16.0, 19.5)]),
            p(&[(12.5, 16.0), (16.0, 19.5), (19.5, 16.0)]),
        ],
        "arrows-left-right" => vec![
            p(&[(19.5, 8.0), (4.5, 8.0)]),
            p(&[(8.0, 4.5), (4.5, 8.0), (8.0, 11.5)]),
            p(&[(4.5, 16.0), (19.5, 16.0)]),
            p(&[(16.0, 12.5), (19.5, 16.0), (16.0, 19.5)]),
        ],
        "corner-up-left" => vec![
            path(vec![L(19.0, 19.0), L(19.0, 12.0), Q(19.0, 8.0, 15.0, 8.0), L(4.5, 8.0)]),
            p(&[(9.0, 3.5), (4.5, 8.0), (9.0, 12.5)]),
        ],
        "corner-up-right" => vec![
            path(vec![L(5.0, 19.0), L(5.0, 12.0), Q(5.0, 8.0, 9.0, 8.0), L(19.5, 8.0)]),
            p(&[(15.0, 3.5), (19.5, 8.0), (15.0, 12.5)]),
        ],
        "double-chevron-left" => vec![
            p(&[(11.5, 5.0), (5.0, 12.0), (11.5, 19.0)]),
            p(&[(18.5, 5.0), (12.0, 12.0), (18.5, 19.0)]),
        ],
        "double-chevron-right" => vec![
            p(&[(12.5, 5.0), (19.0, 12.0), (12.5, 19.0)]),
            p(&[(5.5, 5.0), (12.0, 12.0), (5.5, 19.0)]),
        ],
        "map-pin" => vec![
            pathc(vec![
                L(12.0, 21.5),
                Q(5.5, 14.0, 5.5, 9.5),
                Q(5.5, 3.0, 12.0, 3.0),
                Q(18.5, 3.0, 18.5, 9.5),
                Q(18.5, 14.0, 12.0, 21.5),
            ]),
            c(12.0, 9.5, 2.5),
        ],
        "map" => vec![
            pc(&[
                (4.0, 5.5), (9.3, 3.5), (14.7, 5.5), (20.0, 3.5),
                (20.0, 18.5), (14.7, 20.5), (9.3, 18.5), (4.0, 20.5),
            ]),
            p(&[(9.3, 3.5), (9.3, 18.5)]),
            p(&[(14.7, 5.5), (14.7, 20.5)]),
        ],
        "crosshair" => vec![
            c(12.0, 12.0, 7.0),
            p(&[(12.0, 2.5), (12.0, 6.5)]),
            p(&[(12.0, 17.5), (12.0, 21.5)]),
            p(&[(2.5, 12.0), (6.5, 12.0)]),
            p(&[(17.5, 12.0), (21.5, 12.0)]),
        ],
        "sitemap" => vec![
            rr(9.0, 3.0, 6.0, 5.0, 1.0),
            rr(3.0, 16.0, 5.5, 5.0, 1.0),
            rr(9.25, 16.0, 5.5, 5.0, 1.0),
            rr(15.5, 16.0, 5.5, 5.0, 1.0),
            p(&[(12.0, 8.0), (12.0, 16.0)]),
            p(&[(5.75, 16.0), (5.75, 12.0), (18.25, 12.0), (18.25, 16.0)]),
        ],

        // ── Action ─────────────────────────────────────────────────────────
        "plus" => vec![p(&[(12.0, 4.5), (12.0, 19.5)]), p(&[(4.5, 12.0), (19.5, 12.0)])],
        "minus" => vec![p(&[(4.5, 12.0), (19.5, 12.0)])],
        "check" => vec![p(&[(4.5, 12.5), (9.5, 17.5), (19.5, 6.5)])],
        "x-mark" => vec![p(&[(6.0, 6.0), (18.0, 18.0)]), p(&[(18.0, 6.0), (6.0, 18.0)])],
        "refresh" => vec![
            a(12.0, 12.0, 7.5, 270.0, 540.0),
            p(&[(2.3, 14.4), (4.5, 12.0), (6.7, 14.4)]),
        ],
        "sync" => vec![
            a(12.0, 12.0, 7.5, 240.0, 360.0),
            p(&[(17.3, 9.8), (19.5, 12.0), (21.7, 9.8)]),
            a(12.0, 12.0, 7.5, 60.0, 180.0),
            p(&[(2.3, 14.2), (4.5, 12.0), (6.7, 14.2)]),
        ],
        "download" => vec![
            p(&[(4.0, 15.5), (4.0, 19.5), (20.0, 19.5), (20.0, 15.5)]),
            p(&[(12.0, 3.5), (12.0, 14.5)]),
            p(&[(7.0, 10.0), (12.0, 15.0), (17.0, 10.0)]),
        ],
        "upload" => vec![
            p(&[(4.0, 15.5), (4.0, 19.5), (20.0, 19.5), (20.0, 15.5)]),
            p(&[(12.0, 14.5), (12.0, 3.5)]),
            p(&[(7.0, 9.0), (12.0, 4.0), (17.0, 9.0)]),
        ],
        "share" => vec![
            c(6.0, 12.0, 2.8),
            c(18.0, 5.5, 2.8),
            c(18.0, 18.5, 2.8),
            p(&[(8.5, 10.7), (15.6, 6.9)]),
            p(&[(8.5, 13.3), (15.6, 17.1)]),
        ],
        "export" => vec![
            p(&[(16.0, 9.5), (19.5, 9.5), (19.5, 20.5), (4.5, 20.5), (4.5, 9.5), (8.0, 9.5)]),
            p(&[(12.0, 14.5), (12.0, 3.0)]),
            p(&[(8.5, 6.3), (12.0, 2.9), (15.5, 6.3)]),
        ],
        "import" => vec![
            p(&[(16.0, 9.5), (19.5, 9.5), (19.5, 20.5), (4.5, 20.5), (4.5, 9.5), (8.0, 9.5)]),
            p(&[(12.0, 3.0), (12.0, 13.5)]),
            p(&[(8.5, 10.2), (12.0, 13.7), (15.5, 10.2)]),
        ],
        "link" => vec![rr(3.5, 9.0, 10.0, 6.0, 3.0), rr(10.5, 9.0, 10.0, 6.0, 3.0)],
        "unlink" => vec![
            rr(2.5, 9.0, 8.5, 6.0, 3.0),
            rr(13.0, 9.0, 8.5, 6.0, 3.0),
            p(&[(10.3, 3.8), (11.3, 6.2)]),
            p(&[(13.7, 3.8), (12.7, 6.2)]),
            p(&[(10.3, 20.2), (11.3, 17.8)]),
            p(&[(13.7, 20.2), (12.7, 17.8)]),
        ],
        "power" => vec![a(12.0, 13.0, 7.0, 300.0, 600.0), p(&[(12.0, 3.0), (12.0, 11.5)])],
        "pin" => vec![
            pc(&[(9.0, 4.0), (15.0, 4.0), (15.0, 10.0), (17.5, 13.0), (6.5, 13.0), (9.0, 10.0)]),
            p(&[(12.0, 13.0), (12.0, 20.5)]),
        ],
        "unpin" => vec![
            pc(&[(9.0, 4.0), (15.0, 4.0), (15.0, 10.0), (17.5, 13.0), (6.5, 13.0), (9.0, 10.0)]),
            p(&[(12.0, 13.0), (12.0, 20.5)]),
            p(&[(4.5, 4.5), (19.5, 19.5)]),
        ],
        "drag-handle" => vec![
            d(9.0, 6.0, 1.3), d(15.0, 6.0, 1.3),
            d(9.0, 12.0, 1.3), d(15.0, 12.0, 1.3),
            d(9.0, 18.0, 1.3), d(15.0, 18.0, 1.3),
        ],
        "broom" => vec![
            p(&[(12.0, 3.0), (12.0, 11.0)]),
            pc(&[(8.5, 11.0), (15.5, 11.0), (17.5, 18.5), (6.5, 18.5)]),
            p(&[(10.3, 14.5), (9.8, 18.5)]),
            p(&[(13.7, 14.5), (14.2, 18.5)]),
        ],
        "magic-wand" => vec![
            p(&[(4.5, 19.5), (14.0, 10.0)]),
            p(&[(17.0, 3.0), (17.0, 9.0)]),
            p(&[(14.0, 6.0), (20.0, 6.0)]),
            p(&[(20.0, 12.0), (20.0, 15.0)]),
            p(&[(18.5, 13.5), (21.5, 13.5)]),
        ],
        "lightning" => vec![pc(&[
            (13.0, 3.0), (6.0, 13.5), (11.0, 13.5), (9.5, 21.0), (17.5, 10.5), (12.5, 10.5),
        ])],
        "repeat" => vec![
            path(vec![L(3.5, 11.0), L(3.5, 10.0), Q(3.5, 6.0, 7.5, 6.0), L(20.5, 6.0)]),
            p(&[(17.3, 2.3), (21.0, 6.0), (17.3, 9.7)]),
            path(vec![L(20.5, 13.0), L(20.5, 14.0), Q(20.5, 18.0, 16.5, 18.0), L(3.5, 18.0)]),
            p(&[(6.7, 14.3), (3.0, 18.0), (6.7, 21.7)]),
        ],
        "shuffle" => vec![
            p(&[(3.5, 6.0), (8.0, 6.0), (16.0, 18.0), (20.5, 18.0)]),
            p(&[(17.5, 15.0), (20.9, 18.0), (17.5, 21.0)]),
            p(&[(3.5, 18.0), (8.0, 18.0), (16.0, 6.0), (20.5, 6.0)]),
            p(&[(17.5, 3.0), (20.9, 6.0), (17.5, 9.0)]),
        ],
        "timer-start" => vec![
            c(12.0, 13.5, 7.5),
            p(&[(10.0, 3.0), (14.0, 3.0)]),
            p(&[(12.0, 3.0), (12.0, 6.0)]),
            pc(&[(10.5, 10.5), (15.5, 13.5), (10.5, 16.5)]),
        ],
        "stamp" => vec![
            a(12.0, 6.5, 2.0, 180.0, 360.0),
            p(&[(10.0, 6.5), (10.0, 10.5)]),
            p(&[(14.0, 6.5), (14.0, 10.5)]),
            p(&[(10.0, 10.5), (8.5, 12.0), (8.5, 15.5), (15.5, 15.5), (15.5, 12.0), (14.0, 10.5)]),
            rr(5.5, 17.5, 13.0, 3.0, 1.0),
        ],

        _ => return None,
    })
}

/// The classic handset outline, shared by the phone family (the arrow or
/// mark variants add their glyph in the free top-right corner).
fn phone_handset() -> IconShape {
    pathc(vec![
        L(7.8, 3.6),
        Q(9.0, 3.6, 9.4, 4.7),
        L(10.8, 8.1),
        Q(11.2, 9.1, 10.4, 9.8),
        L(9.0, 11.1),
        Q(10.8, 14.8, 14.4, 16.5),
        L(15.7, 15.1),
        Q(16.4, 14.3, 17.4, 14.7),
        L(20.8, 16.1),
        Q(21.9, 16.6, 21.9, 17.7),
        L(21.9, 19.3),
        Q(21.9, 20.6, 20.4, 20.6),
        Q(13.6, 21.0, 8.0, 15.4),
        Q(2.9, 10.2, 3.0, 4.9),
        Q(3.0, 3.6, 4.4, 3.6),
    ])
}

/// A speech bubble with its tail at the bottom-left.
fn chat_bubble() -> IconShape {
    pathc(vec![
        L(3.5, 6.5),
        Q(3.5, 4.5, 5.5, 4.5),
        L(18.5, 4.5),
        Q(20.5, 4.5, 20.5, 6.5),
        L(20.5, 13.5),
        Q(20.5, 15.5, 18.5, 15.5),
        L(8.5, 15.5),
        L(3.5, 19.5),
    ])
}

/// UI/View, Communication — chunk 2.
#[rustfmt::skip]
fn view_comm_shapes(name: &str) -> Option<Vec<IconShape>> {
    Some(match name {
        // ── UI/View ────────────────────────────────────────────────────────
        "eye" => vec![
            pathc(vec![
                L(2.5, 12.0),
                Q(7.0, 6.5, 12.0, 6.5), Q(17.0, 6.5, 21.5, 12.0),
                Q(17.0, 17.5, 12.0, 17.5), Q(7.0, 17.5, 2.5, 12.0),
            ]),
            c(12.0, 12.0, 3.0),
        ],
        "eye-off" => vec![
            pathc(vec![
                L(2.5, 12.0),
                Q(7.0, 6.5, 12.0, 6.5), Q(17.0, 6.5, 21.5, 12.0),
                Q(17.0, 17.5, 12.0, 17.5), Q(7.0, 17.5, 2.5, 12.0),
            ]),
            c(12.0, 12.0, 3.0),
            p(&[(4.5, 4.0), (19.5, 20.0)]),
        ],
        "magnifier" => vec![c(10.5, 10.5, 6.5), p(&[(15.3, 15.3), (20.5, 20.5)])],
        "zoom-in" => vec![
            c(10.5, 10.5, 6.5),
            p(&[(15.3, 15.3), (20.5, 20.5)]),
            p(&[(10.5, 7.8), (10.5, 13.2)]),
            p(&[(7.8, 10.5), (13.2, 10.5)]),
        ],
        "zoom-out" => vec![
            c(10.5, 10.5, 6.5),
            p(&[(15.3, 15.3), (20.5, 20.5)]),
            p(&[(7.8, 10.5), (13.2, 10.5)]),
        ],
        "fullscreen" => vec![
            p(&[(4.0, 9.0), (4.0, 4.0), (9.0, 4.0)]),
            p(&[(15.0, 4.0), (20.0, 4.0), (20.0, 9.0)]),
            p(&[(20.0, 15.0), (20.0, 20.0), (15.0, 20.0)]),
            p(&[(9.0, 20.0), (4.0, 20.0), (4.0, 15.0)]),
        ],
        "collapse" => vec![
            p(&[(4.5, 4.5), (10.0, 10.0)]),
            p(&[(6.0, 10.0), (10.0, 10.0), (10.0, 6.0)]),
            p(&[(19.5, 19.5), (14.0, 14.0)]),
            p(&[(18.0, 14.0), (14.0, 14.0), (14.0, 18.0)]),
        ],
        "expand" => vec![
            p(&[(10.0, 10.0), (4.5, 4.5)]),
            p(&[(4.5, 9.0), (4.5, 4.5), (9.0, 4.5)]),
            p(&[(14.0, 14.0), (19.5, 19.5)]),
            p(&[(19.5, 15.0), (19.5, 19.5), (15.0, 19.5)]),
        ],
        "grid-view" => vec![
            rr(4.0, 4.0, 7.0, 7.0, 1.2),
            rr(13.0, 4.0, 7.0, 7.0, 1.2),
            rr(4.0, 13.0, 7.0, 7.0, 1.2),
            rr(13.0, 13.0, 7.0, 7.0, 1.2),
        ],
        "list-view" => vec![
            d(5.5, 6.0, 1.2), p(&[(9.5, 6.0), (20.0, 6.0)]),
            d(5.5, 12.0, 1.2), p(&[(9.5, 12.0), (20.0, 12.0)]),
            d(5.5, 18.0, 1.2), p(&[(9.5, 18.0), (20.0, 18.0)]),
        ],
        "columns" => vec![
            rr(3.5, 4.5, 17.0, 15.0, 1.5),
            p(&[(9.2, 4.5), (9.2, 19.5)]),
            p(&[(14.8, 4.5), (14.8, 19.5)]),
        ],
        "rows" => vec![
            rr(3.5, 4.5, 17.0, 15.0, 1.5),
            p(&[(3.5, 9.5), (20.5, 9.5)]),
            p(&[(3.5, 14.5), (20.5, 14.5)]),
        ],
        "sidebar-left" => vec![rr(3.5, 4.5, 17.0, 15.0, 1.5), p(&[(9.5, 4.5), (9.5, 19.5)])],
        "sidebar-right" => vec![rr(3.5, 4.5, 17.0, 15.0, 1.5), p(&[(14.5, 4.5), (14.5, 19.5)])],
        "split-view" => vec![rr(3.5, 4.5, 17.0, 15.0, 1.5), p(&[(12.0, 4.5), (12.0, 19.5)])],
        "layers" => vec![
            pc(&[(12.0, 3.5), (21.0, 8.5), (12.0, 13.5), (3.0, 8.5)]),
            p(&[(3.0, 12.5), (12.0, 17.5), (21.0, 12.5)]),
            p(&[(3.0, 16.5), (12.0, 21.5), (21.0, 16.5)]),
        ],
        "layout-dashboard" => vec![
            rr(4.0, 4.0, 7.0, 10.0, 1.2),
            rr(13.0, 4.0, 7.0, 6.0, 1.2),
            rr(13.0, 12.0, 7.0, 8.0, 1.2),
            rr(4.0, 16.0, 7.0, 4.0, 1.2),
        ],
        "maximize" => vec![
            rr(4.5, 6.5, 13.0, 13.0, 1.5),
            p(&[(11.0, 13.0), (19.5, 4.5)]),
            p(&[(15.0, 4.5), (19.5, 4.5), (19.5, 9.0)]),
        ],
        "minimize" => vec![rr(4.5, 4.5, 15.0, 15.0, 1.5), p(&[(7.5, 16.5), (16.5, 16.5)])],
        "picture-in-picture" => vec![
            rr(3.5, 5.0, 17.0, 14.0, 1.5),
            rr(12.5, 12.0, 6.5, 5.0, 0.8),
        ],

        // ── Communication ──────────────────────────────────────────────────
        "mail" => vec![
            rr(3.0, 5.5, 18.0, 13.0, 1.5),
            p(&[(3.5, 6.5), (12.0, 13.0), (20.5, 6.5)]),
        ],
        "mail-open" => vec![
            pathc(vec![L(3.5, 10.0), L(12.0, 3.5), L(20.5, 10.0), L(20.5, 20.0), L(3.5, 20.0)]),
            p(&[(3.5, 10.0), (12.0, 15.5), (20.5, 10.0)]),
        ],
        "send" => vec![
            pc(&[(20.5, 3.5), (3.5, 10.3), (9.9, 13.4), (12.9, 20.3)]),
            p(&[(20.5, 3.5), (9.9, 13.4)]),
        ],
        "inbox" => vec![
            rr(3.0, 4.5, 18.0, 15.0, 1.5),
            p(&[(3.0, 13.0), (8.0, 13.0), (10.0, 15.5), (14.0, 15.5), (16.0, 13.0), (21.0, 13.0)]),
        ],
        "chat" => vec![chat_bubble()],
        "phone" => vec![phone_handset()],
        "video" => vec![
            rr(3.0, 7.0, 12.0, 10.0, 1.5),
            pc(&[(15.0, 11.0), (20.5, 7.5), (20.5, 16.5), (15.0, 13.0)]),
        ],
        "bell" => vec![
            path(vec![
                L(6.0, 17.5), L(6.0, 10.5),
                Q(6.0, 4.5, 12.0, 4.5), Q(18.0, 4.5, 18.0, 10.5), L(18.0, 17.5),
            ]),
            p(&[(4.5, 17.5), (19.5, 17.5)]),
            a(12.0, 17.8, 2.0, 15.0, 165.0),
            p(&[(12.0, 3.0), (12.0, 4.5)]),
        ],
        "bell-off" => vec![
            path(vec![
                L(6.0, 17.5), L(6.0, 10.5),
                Q(6.0, 4.5, 12.0, 4.5), Q(18.0, 4.5, 18.0, 10.5), L(18.0, 17.5),
            ]),
            p(&[(4.5, 17.5), (19.5, 17.5)]),
            a(12.0, 17.8, 2.0, 15.0, 165.0),
            p(&[(4.5, 4.0), (19.5, 20.0)]),
        ],
        "at-sign" => vec![
            c(12.0, 12.0, 4.0),
            a(12.0, 12.0, 8.5, 20.0, 340.0),
            path(vec![L(16.0, 12.0), L(16.0, 14.5), Q(16.0, 16.5, 18.5, 16.5)]),
        ],
        "phone-incoming" => vec![
            phone_handset(),
            p(&[(21.0, 3.0), (15.5, 8.5)]),
            p(&[(15.5, 5.0), (15.5, 8.5), (19.0, 8.5)]),
        ],
        "phone-outgoing" => vec![
            phone_handset(),
            p(&[(15.5, 8.5), (21.0, 3.0)]),
            p(&[(21.0, 6.5), (21.0, 3.0), (17.5, 3.0)]),
        ],
        "phone-missed" => vec![
            phone_handset(),
            p(&[(15.5, 3.5), (20.5, 8.5)]),
            p(&[(20.5, 3.5), (15.5, 8.5)]),
        ],
        "voicemail" => vec![
            c(6.5, 12.0, 3.5),
            c(17.5, 12.0, 3.5),
            p(&[(6.5, 15.5), (17.5, 15.5)]),
        ],
        "chat-dots" => vec![
            chat_bubble(),
            d(8.5, 10.0, 1.2),
            d(12.0, 10.0, 1.2),
            d(15.5, 10.0, 1.2),
        ],
        "chat-check" => vec![chat_bubble(), p(&[(9.0, 10.0), (11.3, 12.3), (15.5, 8.0)])],
        "broadcast" => vec![
            d(12.0, 12.0, 1.8),
            a(12.0, 12.0, 5.0, -45.0, 45.0),
            a(12.0, 12.0, 5.0, 135.0, 225.0),
            a(12.0, 12.0, 8.5, -45.0, 45.0),
            a(12.0, 12.0, 8.5, 135.0, 225.0),
        ],
        "antenna" => vec![
            p(&[(8.5, 21.0), (12.0, 7.0), (15.5, 21.0)]),
            p(&[(10.0, 16.0), (14.0, 16.0)]),
            d(12.0, 7.0, 1.3),
            a(12.0, 6.5, 3.5, 210.0, 330.0),
            a(12.0, 6.5, 6.0, 220.0, 320.0),
        ],
        "satellite" => vec![
            rr(2.5, 10.0, 5.0, 4.0, 0.5),
            rr(16.5, 10.0, 5.0, 4.0, 0.5),
            rr(9.0, 9.0, 6.0, 6.0, 1.0),
            p(&[(7.5, 12.0), (9.0, 12.0)]),
            p(&[(15.0, 12.0), (16.5, 12.0)]),
            a(12.0, 6.0, 3.0, 210.0, 330.0),
        ],
        "megaphone" => vec![
            pc(&[(4.0, 9.5), (14.5, 4.5), (14.5, 17.5), (4.0, 12.5)]),
            p(&[(8.0, 12.7), (8.0, 17.0), (11.0, 17.0), (11.0, 14.1)]),
            a(17.5, 11.0, 3.0, -60.0, 60.0),
        ],

        _ => return None,
    })
}

/// People, Media, Data, Social — chunk 3.
#[rustfmt::skip]
fn people_media_data_shapes(name: &str) -> Option<Vec<IconShape>> {
    Some(match name {
        // ── People ─────────────────────────────────────────────────────────
        "user" => person(12.0, 20.0, 1.0).to_vec(),
        "users" => {
            let mut v = person(9.5, 20.0, 0.88).to_vec();
            v.push(c(16.8, 9.3, 2.8));
            v.push(path(vec![L(16.2, 15.8), Q(20.9, 15.8, 20.9, 20.0)]));
            v
        }
        "user-plus" => {
            let mut v = person(10.0, 20.0, 0.95).to_vec();
            v.push(p(&[(18.5, 8.0), (18.5, 14.0)]));
            v.push(p(&[(15.5, 11.0), (21.5, 11.0)]));
            v
        }
        "user-minus" => {
            let mut v = person(10.0, 20.0, 0.95).to_vec();
            v.push(p(&[(15.5, 11.0), (21.5, 11.0)]));
            v
        }
        "user-check" => {
            let mut v = person(10.0, 20.0, 0.95).to_vec();
            v.push(p(&[(15.5, 11.0), (17.7, 13.2), (21.7, 9.2)]));
            v
        }
        "user-circle" => vec![
            c(12.0, 12.0, 9.0),
            c(12.0, 9.5, 3.0),
            path(vec![L(6.2, 18.6), Q(6.5, 14.6, 12.0, 14.6), Q(17.5, 14.6, 17.8, 18.6)]),
        ],
        "user-x" => {
            let mut v = person(10.0, 20.0, 0.95).to_vec();
            v.push(p(&[(16.3, 8.8), (20.7, 13.2)]));
            v.push(p(&[(20.7, 8.8), (16.3, 13.2)]));
            v
        }
        "user-gear" => {
            let mut v = person(10.0, 20.0, 0.95).to_vec();
            v.push(c(18.0, 12.0, 1.7));
            v.push(p(&[(18.0, 9.0), (18.0, 9.9)]));
            v.push(p(&[(18.0, 14.1), (18.0, 15.0)]));
            v.push(p(&[(15.0, 12.0), (15.9, 12.0)]));
            v.push(p(&[(20.1, 12.0), (21.0, 12.0)]));
            v
        }
        "user-shield" => {
            let mut v = person(10.0, 20.0, 0.95).to_vec();
            v.push(pathc(vec![
                L(18.5, 9.0),
                L(21.2, 10.0), L(21.2, 12.8),
                Q(21.2, 15.2, 18.5, 16.4),
                Q(15.8, 15.2, 15.8, 12.8), L(15.8, 10.0),
            ]));
            v
        }
        "user-star" => {
            let mut v = person(9.5, 20.0, 0.9).to_vec();
            v.push(pc(&[
                (18.5, 8.5), (19.6, 10.7), (21.9, 10.9), (20.2, 12.5), (20.7, 14.8),
                (18.5, 13.5), (16.3, 14.8), (16.8, 12.5), (15.1, 10.9), (17.4, 10.7),
            ]));
            v
        }
        "team" => {
            let mut v = person(12.0, 19.0, 0.85).to_vec();
            v.push(c(5.5, 9.8, 2.2));
            v.push(path(vec![L(2.5, 17.5), Q(2.5, 13.8, 5.8, 13.8)]));
            v.push(c(18.5, 9.8, 2.2));
            v.push(path(vec![L(21.5, 17.5), Q(21.5, 13.8, 18.2, 13.8)]));
            v
        }
        "id-badge" => vec![
            rr(4.5, 3.5, 15.0, 17.0, 1.5),
            rr(10.0, 2.2, 4.0, 2.6, 1.2),
            c(12.0, 10.5, 2.4),
            path(vec![L(8.0, 17.0), Q(8.0, 14.0, 12.0, 14.0), Q(16.0, 14.0, 16.0, 17.0)]),
        ],
        "contact-book" => vec![
            rr(5.0, 3.5, 14.0, 17.0, 1.5),
            p(&[(3.5, 7.0), (5.0, 7.0)]),
            p(&[(3.5, 12.0), (5.0, 12.0)]),
            p(&[(3.5, 17.0), (5.0, 17.0)]),
            c(12.0, 9.5, 2.2),
            path(vec![L(8.5, 16.0), Q(8.5, 13.2, 12.0, 13.2), Q(15.5, 13.2, 15.5, 16.0)]),
        ],
        "presenter" => {
            let mut v = person(8.5, 19.0, 0.85).to_vec();
            v.push(rr(13.5, 4.5, 8.0, 10.0, 1.0));
            v.push(p(&[(15.0, 11.5), (17.2, 8.8), (19.8, 10.6)]));
            v
        }

        // ── Media ──────────────────────────────────────────────────────────
        "play" => vec![pc(&[(8.0, 5.0), (19.0, 12.0), (8.0, 19.0)])],
        "pause" => vec![rr(6.5, 5.0, 4.0, 14.0, 1.0), rr(13.5, 5.0, 4.0, 14.0, 1.0)],
        "stop" => vec![rr(5.5, 5.5, 13.0, 13.0, 1.5)],
        "skip-forward" => vec![
            pc(&[(5.0, 5.0), (13.0, 12.0), (5.0, 19.0)]),
            p(&[(17.5, 5.0), (17.5, 19.0)]),
        ],
        "skip-back" => vec![
            pc(&[(19.0, 5.0), (11.0, 12.0), (19.0, 19.0)]),
            p(&[(6.5, 5.0), (6.5, 19.0)]),
        ],
        "volume" => vec![
            pc(&[(4.0, 9.5), (8.0, 9.5), (13.0, 5.0), (13.0, 19.0), (8.0, 14.5), (4.0, 14.5)]),
            a(13.0, 12.0, 4.0, -50.0, 50.0),
            a(13.0, 12.0, 7.0, -50.0, 50.0),
        ],
        "volume-off" => vec![
            pc(&[(4.0, 9.5), (8.0, 9.5), (13.0, 5.0), (13.0, 19.0), (8.0, 14.5), (4.0, 14.5)]),
            p(&[(16.5, 9.5), (21.5, 14.5)]),
            p(&[(21.5, 9.5), (16.5, 14.5)]),
        ],
        "music" => vec![
            p(&[(9.0, 17.5), (9.0, 5.0)]),
            p(&[(19.0, 16.0), (19.0, 3.5)]),
            p(&[(9.0, 5.0), (19.0, 3.5)]),
            c(6.8, 17.5, 2.2),
            c(16.8, 16.0, 2.2),
        ],
        "record" => vec![c(12.0, 12.0, 8.0), d(12.0, 12.0, 3.5)],
        "eject" => vec![
            pc(&[(5.0, 13.0), (12.0, 5.5), (19.0, 13.0)]),
            p(&[(5.0, 17.5), (19.0, 17.5)]),
        ],
        "fast-forward" => vec![
            pc(&[(4.0, 6.0), (11.0, 12.0), (4.0, 18.0)]),
            pc(&[(12.5, 6.0), (19.5, 12.0), (12.5, 18.0)]),
        ],
        "rewind" => vec![
            pc(&[(20.0, 6.0), (13.0, 12.0), (20.0, 18.0)]),
            pc(&[(11.5, 6.0), (4.5, 12.0), (11.5, 18.0)]),
        ],
        "playlist" => vec![
            p(&[(4.0, 6.0), (15.0, 6.0)]),
            p(&[(4.0, 11.0), (15.0, 11.0)]),
            p(&[(4.0, 16.0), (10.0, 16.0)]),
            p(&[(18.5, 16.0), (18.5, 7.0)]),
            p(&[(18.5, 7.0), (21.5, 8.5)]),
            c(16.7, 16.0, 1.8),
        ],
        "microphone" => vec![
            rr(9.5, 3.5, 5.0, 10.0, 2.5),
            a(12.0, 12.5, 5.5, 0.0, 180.0),
            p(&[(12.0, 18.0), (12.0, 21.0)]),
            p(&[(9.0, 21.0), (15.0, 21.0)]),
        ],
        "headphones" => vec![
            a(12.0, 13.0, 8.5, 180.0, 360.0),
            rr(2.8, 13.0, 4.0, 6.5, 1.5),
            rr(17.2, 13.0, 4.0, 6.5, 1.5),
        ],
        "camera" => vec![
            rr(3.0, 7.0, 18.0, 13.0, 2.0),
            p(&[(8.5, 7.0), (9.8, 4.5), (14.2, 4.5), (15.5, 7.0)]),
            c(12.0, 13.5, 3.8),
        ],
        "film" => vec![
            rr(3.5, 4.5, 17.0, 15.0, 1.5),
            p(&[(7.5, 4.5), (7.5, 19.5)]),
            p(&[(16.5, 4.5), (16.5, 19.5)]),
            p(&[(7.5, 12.0), (16.5, 12.0)]),
            p(&[(3.5, 8.2), (7.5, 8.2)]),
            p(&[(3.5, 15.8), (7.5, 15.8)]),
            p(&[(16.5, 8.2), (20.5, 8.2)]),
            p(&[(16.5, 15.8), (20.5, 15.8)]),
        ],
        "image" => vec![
            rr(3.5, 4.5, 17.0, 15.0, 1.5),
            c(9.0, 9.5, 1.8),
            p(&[(6.0, 17.0), (10.5, 11.5), (13.3, 14.8), (15.8, 12.2), (20.0, 17.0)]),
        ],

        // ── Data ───────────────────────────────────────────────────────────
        "database" => vec![
            pathc(vec![
                L(4.5, 7.0),
                Q(4.5, 4.5, 12.0, 4.5), Q(19.5, 4.5, 19.5, 7.0),
                Q(19.5, 9.5, 12.0, 9.5), Q(4.5, 9.5, 4.5, 7.0),
            ]),
            p(&[(4.5, 7.0), (4.5, 17.0)]),
            p(&[(19.5, 7.0), (19.5, 17.0)]),
            path(vec![L(4.5, 17.0), Q(4.5, 19.5, 12.0, 19.5), Q(19.5, 19.5, 19.5, 17.0)]),
            path(vec![L(4.5, 12.0), Q(4.5, 14.5, 12.0, 14.5), Q(19.5, 14.5, 19.5, 12.0)]),
        ],
        "chart-bar" => vec![
            p(&[(3.5, 19.5), (20.5, 19.5)]),
            rr(6.0, 11.0, 3.0, 8.5, 0.5),
            rr(10.5, 6.5, 3.0, 13.0, 0.5),
            rr(15.0, 13.5, 3.0, 6.0, 0.5),
        ],
        "chart-line" => vec![
            p(&[(4.0, 4.0), (4.0, 20.0), (20.5, 20.0)]),
            p(&[(6.5, 15.5), (10.5, 10.5), (13.5, 13.0), (19.0, 6.5)]),
        ],
        "chart-pie" => vec![
            c(12.0, 12.0, 8.5),
            p(&[(12.0, 12.0), (12.0, 3.5)]),
            p(&[(12.0, 12.0), (20.5, 12.0)]),
        ],
        "table" => vec![
            rr(3.5, 4.5, 17.0, 15.0, 1.5),
            p(&[(3.5, 9.0), (20.5, 9.0)]),
            p(&[(9.2, 9.0), (9.2, 19.5)]),
            p(&[(14.8, 9.0), (14.8, 19.5)]),
        ],
        "filter" => vec![pc(&[
            (3.5, 5.0), (20.5, 5.0), (14.0, 12.5), (14.0, 19.0), (10.0, 16.5), (10.0, 12.5),
        ])],
        "sort-asc" => vec![
            p(&[(4.0, 6.5), (9.0, 6.5)]),
            p(&[(4.0, 12.0), (12.0, 12.0)]),
            p(&[(4.0, 17.5), (15.0, 17.5)]),
            p(&[(18.5, 17.5), (18.5, 6.0)]),
            p(&[(15.5, 9.0), (18.5, 6.0), (21.5, 9.0)]),
        ],
        "sort-desc" => vec![
            p(&[(4.0, 6.5), (15.0, 6.5)]),
            p(&[(4.0, 12.0), (12.0, 12.0)]),
            p(&[(4.0, 17.5), (9.0, 17.5)]),
            p(&[(18.5, 6.5), (18.5, 18.0)]),
            p(&[(15.5, 15.0), (18.5, 18.0), (21.5, 15.0)]),
        ],
        "chart-area" => vec![
            p(&[(4.0, 4.0), (4.0, 20.0), (20.5, 20.0)]),
            pc(&[(6.0, 17.5), (6.0, 13.0), (10.0, 9.0), (13.5, 12.0), (18.5, 6.5), (18.5, 17.5)]),
        ],
        "chart-scatter" => vec![
            p(&[(4.0, 4.0), (4.0, 20.0), (20.5, 20.0)]),
            d(8.0, 15.0, 1.4),
            d(11.0, 9.5, 1.4),
            d(15.0, 13.0, 1.4),
            d(18.0, 7.0, 1.4),
            d(13.0, 16.5, 1.4),
        ],
        "chart-donut" => vec![
            c(12.0, 12.0, 8.5),
            c(12.0, 12.0, 3.5),
            p(&[(12.0, 3.5), (12.0, 8.5)]),
        ],
        "chart-gauge" => vec![
            a(12.0, 14.0, 8.0, 180.0, 360.0),
            p(&[(12.0, 14.0), (16.5, 9.0)]),
            d(12.0, 14.0, 1.5),
            p(&[(4.0, 14.0), (6.0, 14.0)]),
            p(&[(18.0, 14.0), (20.0, 14.0)]),
        ],
        "pivot-table" => vec![
            rr(3.5, 4.5, 17.0, 15.0, 1.5),
            p(&[(3.5, 9.0), (20.5, 9.0)]),
            p(&[(9.2, 4.5), (9.2, 19.5)]),
            p(&[(11.5, 11.5), (18.5, 17.5)]),
            p(&[(18.5, 14.0), (18.5, 17.5), (15.0, 17.5)]),
        ],
        "data-flow" => vec![
            rr(3.0, 3.0, 6.0, 5.0, 1.0),
            rr(15.0, 3.0, 6.0, 5.0, 1.0),
            rr(9.0, 16.0, 6.0, 5.0, 1.0),
            p(&[(6.0, 8.0), (6.0, 12.0), (12.0, 12.0), (12.0, 16.0)]),
            p(&[(18.0, 8.0), (18.0, 12.0), (12.0, 12.0)]),
        ],
        "api" => vec![
            path(vec![
                L(8.5, 4.5), Q(6.0, 4.5, 6.0, 7.0), L(6.0, 9.5),
                Q(6.0, 12.0, 4.0, 12.0), Q(6.0, 12.0, 6.0, 14.5),
                L(6.0, 17.0), Q(6.0, 19.5, 8.5, 19.5),
            ]),
            path(vec![
                L(15.5, 4.5), Q(18.0, 4.5, 18.0, 7.0), L(18.0, 9.5),
                Q(18.0, 12.0, 20.0, 12.0), Q(18.0, 12.0, 18.0, 14.5),
                L(18.0, 17.0), Q(18.0, 19.5, 15.5, 19.5),
            ]),
            d(10.0, 12.0, 1.2),
            d(14.0, 12.0, 1.2),
        ],
        "cloud-database" => vec![
            pathc(vec![
                L(7.0, 17.5),
                Q(3.0, 17.5, 3.0, 13.75), Q(3.0, 10.5, 6.5, 10.0),
                Q(7.0, 5.5, 11.5, 5.5), Q(15.5, 5.5, 16.5, 9.0),
                Q(21.0, 9.5, 21.0, 13.5), Q(21.0, 17.5, 17.0, 17.5),
            ]),
            p(&[(8.5, 13.0), (15.5, 13.0)]),
            p(&[(8.5, 15.3), (15.5, 15.3)]),
        ],
        "histogram" => vec![
            p(&[(3.5, 20.0), (20.5, 20.0)]),
            rr(4.5, 12.0, 4.0, 8.0, 0.3),
            rr(8.5, 7.0, 4.0, 13.0, 0.3),
            rr(12.5, 10.0, 4.0, 10.0, 0.3),
            rr(16.5, 14.5, 4.0, 5.5, 0.3),
        ],
        "treemap" => vec![
            rr(3.5, 3.5, 17.0, 17.0, 1.0),
            p(&[(12.5, 3.5), (12.5, 20.5)]),
            p(&[(12.5, 12.0), (20.5, 12.0)]),
            p(&[(3.5, 14.5), (12.5, 14.5)]),
            p(&[(16.5, 12.0), (16.5, 20.5)]),
        ],
        "funnel" => vec![
            pc(&[(4.0, 4.5), (20.0, 4.5), (14.5, 11.5), (14.5, 19.5), (9.5, 19.5), (9.5, 11.5)]),
            p(&[(7.2, 8.5), (16.8, 8.5)]),
            p(&[(9.5, 12.5), (14.5, 12.5)]),
        ],
        "kpi" => vec![
            rr(3.5, 4.5, 17.0, 15.0, 1.5),
            p(&[(6.5, 16.0), (11.0, 11.5), (13.5, 14.0), (17.5, 8.5)]),
            p(&[(14.5, 8.5), (17.5, 8.5), (17.5, 11.5)]),
        ],

        // ── Social ─────────────────────────────────────────────────────────
        "heart" => vec![pathc(vec![
            L(12.0, 20.0),
            Q(4.0, 14.5, 3.5, 9.5), Q(3.5, 4.5, 8.0, 4.5),
            Q(10.8, 4.5, 12.0, 7.5), Q(13.2, 4.5, 16.0, 4.5),
            Q(20.5, 4.5, 20.5, 9.5), Q(20.0, 14.5, 12.0, 20.0),
        ])],
        "star" => vec![pc(&[
            (12.0, 3.5), (14.2, 9.4), (20.6, 9.7), (15.6, 13.7), (17.3, 19.8),
            (12.0, 16.3), (6.7, 19.8), (8.4, 13.7), (3.4, 9.7), (9.8, 9.4),
        ])],
        "thumbs-up" => vec![
            rr(3.0, 10.5, 4.0, 9.0, 0.8),
            pathc(vec![
                L(7.0, 11.3), L(11.2, 4.2),
                Q(13.8, 4.6, 13.4, 7.6), L(12.8, 10.4), L(18.8, 10.4),
                Q(21.2, 10.6, 20.7, 13.0), L(19.5, 18.1),
                Q(19.1, 19.7, 17.3, 19.7), L(7.0, 19.7),
            ]),
        ],
        "thumbs-down" => vec![
            rr(3.0, 4.5, 4.0, 9.0, 0.8),
            pathc(vec![
                L(7.0, 12.7), L(11.2, 19.8),
                Q(13.8, 19.4, 13.4, 16.4), L(12.8, 13.6), L(18.8, 13.6),
                Q(21.2, 13.4, 20.7, 11.0), L(19.5, 5.9),
                Q(19.1, 4.3, 17.3, 4.3), L(7.0, 4.3),
            ]),
        ],
        "bookmark" => vec![pc(&[
            (6.5, 3.5), (17.5, 3.5), (17.5, 20.5), (12.0, 16.0), (6.5, 20.5),
        ])],
        "flag" => vec![
            p(&[(5.5, 21.0), (5.5, 3.5)]),
            pathc(vec![
                L(5.5, 4.5),
                Q(9.0, 3.0, 12.0, 4.5), Q(15.0, 6.0, 18.5, 4.5),
                L(18.5, 12.5),
                Q(15.0, 14.0, 12.0, 12.5), Q(9.0, 11.0, 5.5, 12.5),
            ]),
        ],
        "gift" => vec![
            rr(4.0, 10.5, 16.0, 10.0, 1.0),
            rr(3.0, 7.0, 18.0, 3.5, 0.8),
            p(&[(12.0, 7.0), (12.0, 20.5)]),
            c(9.6, 5.1, 2.0),
            c(14.4, 5.1, 2.0),
        ],
        "trophy" => vec![
            path(vec![
                L(7.5, 4.0), L(16.5, 4.0), L(16.5, 10.0),
                Q(16.5, 14.5, 12.0, 14.5), Q(7.5, 14.5, 7.5, 10.0), L(7.5, 4.0),
            ]),
            a(5.6, 7.2, 2.1, 90.0, 270.0),
            a(18.4, 7.2, 2.1, -90.0, 90.0),
            p(&[(12.0, 14.5), (12.0, 17.5)]),
            p(&[(9.5, 17.5), (14.5, 17.5)]),
            p(&[(7.5, 20.5), (16.5, 20.5)]),
        ],
        "crown" => vec![pc(&[
            (4.0, 18.5), (3.5, 7.5), (8.2, 11.5), (12.0, 5.5), (15.8, 11.5), (20.5, 7.5),
            (20.0, 18.5),
        ])],
        "diamond" => vec![
            pc(&[(7.0, 5.0), (17.0, 5.0), (21.0, 10.0), (12.0, 20.5), (3.0, 10.0)]),
            p(&[(3.0, 10.0), (21.0, 10.0)]),
            p(&[(7.0, 5.0), (9.5, 10.0), (12.0, 20.5)]),
            p(&[(17.0, 5.0), (14.5, 10.0), (12.0, 20.5)]),
        ],
        "balloon" => vec![
            pathc(vec![
                L(12.0, 16.0),
                Q(6.0, 13.0, 6.0, 8.5), Q(6.0, 3.5, 12.0, 3.5),
                Q(18.0, 3.5, 18.0, 8.5), Q(18.0, 13.0, 12.0, 16.0),
            ]),
            pc(&[(12.0, 16.0), (10.9, 17.6), (13.1, 17.6)]),
            path(vec![L(12.0, 17.6), Q(10.0, 19.5, 12.0, 21.3)]),
        ],
        "party" => vec![
            pc(&[(3.5, 20.5), (8.5, 9.5), (14.5, 15.5)]),
            p(&[(6.0, 15.0), (11.5, 12.5)]),
            p(&[(15.0, 9.0), (17.5, 6.5)]),
            p(&[(17.0, 12.0), (20.5, 11.0)]),
            p(&[(12.5, 7.0), (13.0, 3.5)]),
            d(19.5, 7.5, 1.0),
            d(15.5, 4.5, 0.9),
            d(20.5, 15.0, 0.9),
        ],
        "handshake" => vec![
            p(&[(2.5, 10.0), (6.5, 10.0), (10.0, 13.5)]),
            p(&[(21.5, 10.0), (17.5, 10.0), (14.0, 13.5)]),
            p(&[(10.0, 13.5), (14.0, 10.8)]),
            p(&[(14.0, 13.5), (10.0, 10.8)]),
            p(&[(10.7, 15.4), (12.4, 16.7)]),
            p(&[(13.3, 14.6), (15.0, 15.9)]),
        ],
        "ribbon" => vec![
            c(12.0, 9.0, 5.5),
            c(12.0, 9.0, 3.0),
            p(&[(9.3, 13.6), (7.5, 20.5), (12.0, 17.8), (16.5, 20.5), (14.7, 13.6)]),
        ],

        _ => return None,
    })
}

/// A folder outline (tab at top-left).
fn folder_body() -> IconShape {
    pathc(vec![
        L(3.5, 6.0),
        L(9.0, 6.0),
        L(11.3, 8.5),
        L(20.5, 8.5),
        L(20.5, 19.5),
        L(3.5, 19.5),
    ])
}

/// A calendar body: frame, header line, two hanger pins.
fn calendar_body() -> [IconShape; 4] {
    [
        rr(3.5, 5.0, 17.0, 15.0, 1.5),
        p(&[(3.5, 9.5), (20.5, 9.5)]),
        p(&[(8.0, 3.0), (8.0, 6.5)]),
        p(&[(16.0, 3.0), (16.0, 6.5)]),
    ]
}

/// A ticket with side notches (voucher/coupon family).
fn ticket() -> IconShape {
    pathc(vec![
        L(3.0, 7.0),
        L(21.0, 7.0),
        L(21.0, 10.5),
        Q(19.0, 10.5, 19.0, 12.0),
        Q(19.0, 13.5, 21.0, 13.5),
        L(21.0, 17.0),
        L(3.0, 17.0),
        L(3.0, 13.5),
        Q(5.0, 13.5, 5.0, 12.0),
        Q(5.0, 10.5, 3.0, 10.5),
    ])
}

/// A shopping cart (basket + wheels), leaving the top-right corner free for
/// a plus/minus badge.
fn cart_body() -> [IconShape; 3] {
    [
        p(&[(2.5, 6.0), (5.0, 6.0), (7.3, 16.0), (16.5, 16.0), (18.3, 9.0), (5.8, 9.0)]),
        IconShape::Dot(8.8, 19.2, 1.6),
        IconShape::Dot(15.2, 19.2, 1.6),
    ]
}

/// System, Status, Commerce, File/Folder — chunk 4.
#[rustfmt::skip]
fn system_commerce_shapes(name: &str) -> Option<Vec<IconShape>> {
    Some(match name {
        // ── System ─────────────────────────────────────────────────────────
        "gear" => vec![
            c(12.0, 12.0, 6.2),
            c(12.0, 12.0, 2.6),
            p(&[(18.2, 12.0), (20.6, 12.0)]),
            p(&[(16.4, 16.4), (18.1, 18.1)]),
            p(&[(12.0, 18.2), (12.0, 20.6)]),
            p(&[(7.6, 16.4), (5.9, 18.1)]),
            p(&[(5.8, 12.0), (3.4, 12.0)]),
            p(&[(7.6, 7.6), (5.9, 5.9)]),
            p(&[(12.0, 5.8), (12.0, 3.4)]),
            p(&[(16.4, 7.6), (18.1, 5.9)]),
        ],
        "wrench" => vec![pathc(vec![
            L(4.5, 17.5),
            L(12.5, 9.5),
            Q(11.5, 6.0, 14.0, 3.9),
            Q(15.5, 2.8, 17.5, 3.2),
            L(15.0, 6.0),
            L(16.8, 7.8),
            L(19.8, 5.3),
            Q(20.8, 7.5, 19.6, 9.5),
            Q(17.5, 11.8, 14.5, 11.2),
            L(6.8, 19.0),
            Q(5.5, 20.3, 4.5, 19.3),
        ])],
        "shield" => vec![pathc(vec![
            L(12.0, 3.0),
            L(20.0, 6.0),
            L(20.0, 11.5),
            Q(20.0, 17.5, 12.0, 21.0),
            Q(4.0, 17.5, 4.0, 11.5),
            L(4.0, 6.0),
        ])],
        "lock" => vec![
            rr(5.5, 10.5, 13.0, 9.5, 1.5),
            path(vec![
                L(8.0, 10.5), L(8.0, 7.5),
                Q(8.0, 3.5, 12.0, 3.5), Q(16.0, 3.5, 16.0, 7.5), L(16.0, 10.5),
            ]),
            d(12.0, 15.2, 1.4),
        ],
        "unlock" => vec![
            rr(5.5, 10.5, 13.0, 9.5, 1.5),
            path(vec![
                L(8.0, 10.5), L(8.0, 7.5),
                Q(8.0, 3.5, 12.0, 3.5), Q(16.0, 3.5, 16.0, 7.5),
            ]),
            d(12.0, 15.2, 1.4),
        ],
        "key" => vec![
            c(7.5, 16.5, 3.8),
            p(&[(10.2, 13.8), (19.5, 4.5)]),
            p(&[(17.2, 6.8), (19.2, 8.8)]),
            p(&[(19.5, 4.5), (21.5, 6.5)]),
        ],
        "terminal" => vec![
            rr(3.0, 4.5, 18.0, 15.0, 1.5),
            p(&[(6.5, 9.0), (10.0, 12.0), (6.5, 15.0)]),
            p(&[(12.5, 15.0), (17.0, 15.0)]),
        ],
        "code" => vec![
            p(&[(8.5, 6.5), (3.5, 12.0), (8.5, 17.5)]),
            p(&[(15.5, 6.5), (20.5, 12.0), (15.5, 17.5)]),
            p(&[(13.7, 4.5), (10.3, 19.5)]),
        ],
        "bug" => vec![
            pathc(vec![
                L(12.0, 7.5),
                Q(16.5, 7.5, 16.5, 13.0), Q(16.5, 19.5, 12.0, 19.5),
                Q(7.5, 19.5, 7.5, 13.0), Q(7.5, 7.5, 12.0, 7.5),
            ]),
            p(&[(12.0, 7.5), (12.0, 19.5)]),
            p(&[(9.8, 5.2), (8.2, 3.2)]),
            p(&[(14.2, 5.2), (15.8, 3.2)]),
            p(&[(7.5, 10.5), (4.0, 9.0)]),
            p(&[(7.5, 13.5), (3.5, 13.5)]),
            p(&[(7.5, 16.5), (4.0, 18.0)]),
            p(&[(16.5, 10.5), (20.0, 9.0)]),
            p(&[(16.5, 13.5), (20.5, 13.5)]),
            p(&[(16.5, 16.5), (20.0, 18.0)]),
        ],
        "cpu" => vec![
            rr(6.0, 6.0, 12.0, 12.0, 1.5),
            rr(9.5, 9.5, 5.0, 5.0, 0.5),
            p(&[(9.5, 6.0), (9.5, 3.5)]),
            p(&[(14.5, 6.0), (14.5, 3.5)]),
            p(&[(9.5, 18.0), (9.5, 20.5)]),
            p(&[(14.5, 18.0), (14.5, 20.5)]),
            p(&[(6.0, 9.5), (3.5, 9.5)]),
            p(&[(6.0, 14.5), (3.5, 14.5)]),
            p(&[(18.0, 9.5), (20.5, 9.5)]),
            p(&[(18.0, 14.5), (20.5, 14.5)]),
        ],
        "gears" => vec![
            c(8.5, 9.0, 4.4),
            c(8.5, 9.0, 1.8),
            p(&[(12.9, 9.0), (14.7, 9.0)]),
            p(&[(10.7, 12.8), (11.6, 14.4)]),
            p(&[(6.3, 12.8), (5.4, 14.4)]),
            p(&[(4.1, 9.0), (2.3, 9.0)]),
            p(&[(6.3, 5.2), (5.4, 3.6)]),
            p(&[(10.7, 5.2), (11.6, 3.6)]),
            c(16.5, 16.0, 3.4),
            c(16.5, 16.0, 1.4),
            p(&[(19.4, 17.7), (20.8, 18.5)]),
            p(&[(16.5, 19.4), (16.5, 21.0)]),
            p(&[(13.6, 17.7), (12.2, 18.5)]),
            p(&[(13.6, 14.3), (12.2, 13.5)]),
            p(&[(16.5, 12.6), (16.5, 11.0)]),
            p(&[(19.4, 14.3), (20.8, 13.5)]),
        ],
        "hammer" => vec![
            pc(&[(3.5, 8.0), (8.0, 3.5), (13.5, 7.0), (10.5, 11.5)]),
            p(&[(10.5, 11.0), (19.0, 19.5)]),
        ],
        "screwdriver" => vec![
            p(&[(3.5, 20.5), (5.5, 18.5)]),
            p(&[(5.5, 18.5), (13.0, 11.0)]),
            pc(&[(13.0, 11.0), (16.8, 7.2), (19.8, 10.2), (16.0, 14.0)]),
        ],
        "sliders" => vec![
            p(&[(3.5, 6.5), (20.5, 6.5)]),
            c(9.0, 6.5, 2.0),
            p(&[(3.5, 12.0), (20.5, 12.0)]),
            c(15.0, 12.0, 2.0),
            p(&[(3.5, 17.5), (20.5, 17.5)]),
            c(7.0, 17.5, 2.0),
        ],
        "toggle-on" => vec![rr(3.0, 7.5, 18.0, 9.0, 4.5), d(16.5, 12.0, 3.0)],
        "toggle-off" => vec![rr(3.0, 7.5, 18.0, 9.0, 4.5), c(7.5, 12.0, 3.0)],
        "server" => vec![
            rr(3.5, 4.0, 17.0, 7.0, 1.5),
            rr(3.5, 13.0, 17.0, 7.0, 1.5),
            d(7.0, 7.5, 1.1),
            d(7.0, 16.5, 1.1),
            p(&[(11.0, 7.5), (17.0, 7.5)]),
            p(&[(11.0, 16.5), (17.0, 16.5)]),
        ],
        "network" => vec![
            rr(9.0, 3.0, 6.0, 5.0, 1.0),
            rr(3.0, 16.0, 6.0, 5.0, 1.0),
            rr(15.0, 16.0, 6.0, 5.0, 1.0),
            p(&[(12.0, 8.0), (12.0, 12.0)]),
            p(&[(6.0, 16.0), (6.0, 12.0), (18.0, 12.0), (18.0, 16.0)]),
        ],
        "wifi" => vec![
            d(12.0, 18.5, 1.5),
            a(12.0, 18.5, 4.5, 210.0, 330.0),
            a(12.0, 18.5, 8.0, 215.0, 325.0),
            a(12.0, 18.5, 11.2, 220.0, 320.0),
        ],
        "hard-drive" => vec![
            rr(3.5, 8.0, 17.0, 8.5, 1.5),
            p(&[(3.5, 13.0), (20.5, 13.0)]),
            d(6.5, 14.8, 0.9),
            d(9.5, 14.8, 0.9),
        ],
        "plugin" => vec![
            p(&[(9.5, 3.5), (9.5, 8.5)]),
            p(&[(14.5, 3.5), (14.5, 8.5)]),
            pathc(vec![
                L(6.5, 8.5), L(17.5, 8.5), L(17.5, 11.5),
                Q(17.5, 16.0, 12.0, 16.0), Q(6.5, 16.0, 6.5, 11.5),
            ]),
            p(&[(12.0, 16.0), (12.0, 20.5)]),
        ],
        "memory-chip" => vec![
            rr(5.0, 7.0, 14.0, 10.0, 1.5),
            a(12.0, 7.0, 1.5, 0.0, 180.0),
            rr(8.0, 10.0, 8.0, 4.0, 0.5),
            p(&[(8.0, 17.0), (8.0, 20.0)]),
            p(&[(12.0, 17.0), (12.0, 20.0)]),
            p(&[(16.0, 17.0), (16.0, 20.0)]),
        ],

        // ── Status ─────────────────────────────────────────────────────────
        "info-circle" => vec![badge(), d(12.0, 7.8, 1.2), p(&[(12.0, 11.0), (12.0, 16.5)])],
        "warning-triangle" => vec![
            pc(&[(12.0, 4.0), (21.5, 20.0), (2.5, 20.0)]),
            p(&[(12.0, 9.5), (12.0, 14.5)]),
            d(12.0, 17.2, 1.2),
        ],
        "error-circle" => vec![badge(), p(&[(12.0, 7.5), (12.0, 13.5)]), d(12.0, 16.8, 1.2)],
        "help-circle" => vec![
            badge(),
            path(vec![
                L(9.5, 9.5),
                Q(9.5, 7.0, 12.0, 7.0), Q(14.7, 7.0, 14.7, 9.3),
                Q(14.7, 11.2, 12.3, 11.8), L(12.1, 13.4),
            ]),
            d(12.0, 16.6, 1.2),
        ],
        "check-circle" => vec![badge(), p(&[(7.5, 12.5), (10.8, 15.8), (16.8, 8.8)])],
        "x-circle" => vec![
            badge(),
            p(&[(8.5, 8.5), (15.5, 15.5)]),
            p(&[(15.5, 8.5), (8.5, 15.5)]),
        ],
        "clock" => vec![badge(), p(&[(12.0, 6.5), (12.0, 12.0), (16.0, 14.5)])],
        "calendar" => calendar_body().to_vec(),
        "hourglass" => vec![
            p(&[(6.5, 3.5), (17.5, 3.5)]),
            p(&[(6.5, 20.5), (17.5, 20.5)]),
            path(vec![
                L(7.5, 3.5), L(7.5, 7.0),
                Q(7.5, 10.0, 12.0, 12.0), Q(16.5, 10.0, 16.5, 7.0), L(16.5, 3.5),
            ]),
            path(vec![
                L(7.5, 20.5), L(7.5, 17.0),
                Q(7.5, 14.0, 12.0, 12.0), Q(16.5, 14.0, 16.5, 17.0), L(16.5, 20.5),
            ]),
        ],
        "stopwatch" => vec![
            c(12.0, 13.5, 7.5),
            p(&[(10.0, 3.0), (14.0, 3.0)]),
            p(&[(12.0, 3.0), (12.0, 6.0)]),
            p(&[(12.0, 13.5), (15.5, 10.0)]),
        ],
        "alarm" => vec![
            c(12.0, 13.0, 7.5),
            p(&[(3.8, 7.0), (6.8, 4.2)]),
            p(&[(20.2, 7.0), (17.2, 4.2)]),
            p(&[(12.0, 9.0), (12.0, 13.0), (15.0, 14.5)]),
            p(&[(6.5, 19.5), (5.0, 21.0)]),
            p(&[(17.5, 19.5), (19.0, 21.0)]),
        ],
        "calendar-check" => {
            let mut v = calendar_body().to_vec();
            v.push(p(&[(8.5, 14.5), (11.0, 17.0), (15.5, 11.5)]));
            v
        }
        "calendar-x" => {
            let mut v = calendar_body().to_vec();
            v.push(p(&[(9.0, 12.5), (15.0, 18.5)]));
            v.push(p(&[(15.0, 12.5), (9.0, 18.5)]));
            v
        }
        "progress" => vec![rr(3.0, 9.5, 18.0, 5.0, 2.5), rrf(5.0, 11.0, 7.0, 2.0, 1.0)],
        "loading" => vec![a(12.0, 12.0, 8.0, -90.0, 180.0)],
        "battery-full" => vec![
            rr(2.5, 8.0, 16.0, 8.0, 1.5),
            rr(19.5, 10.5, 2.0, 3.0, 0.8),
            rrf(4.5, 10.0, 3.0, 4.0, 0.5),
            rrf(8.5, 10.0, 3.0, 4.0, 0.5),
            rrf(12.5, 10.0, 3.0, 4.0, 0.5),
        ],
        "battery-low" => vec![
            rr(2.5, 8.0, 16.0, 8.0, 1.5),
            rr(19.5, 10.5, 2.0, 3.0, 0.8),
            rrf(4.5, 10.0, 3.0, 4.0, 0.5),
        ],
        "traffic-light" => vec![
            rr(8.0, 3.0, 8.0, 18.0, 2.5),
            c(12.0, 7.0, 1.7),
            c(12.0, 12.0, 1.7),
            d(12.0, 17.0, 1.7),
        ],

        // ── Commerce ───────────────────────────────────────────────────────
        "cart" => vec![
            p(&[(3.0, 4.5), (6.0, 4.5), (8.5, 15.0), (18.5, 15.0), (20.5, 7.5), (7.0, 7.5)]),
            d(9.5, 18.7, 1.7),
            d(17.0, 18.7, 1.7),
        ],
        "credit-card" => vec![
            rr(2.5, 5.5, 19.0, 13.0, 2.0),
            p(&[(2.5, 9.5), (21.5, 9.5)]),
            p(&[(5.5, 14.5), (10.5, 14.5)]),
        ],
        "wallet" => vec![
            rr(3.0, 6.0, 18.0, 13.0, 2.0),
            path(vec![
                L(21.0, 10.0), L(16.5, 10.0),
                Q(14.0, 10.0, 14.0, 12.5), Q(14.0, 15.0, 16.5, 15.0), L(21.0, 15.0),
            ]),
            d(17.0, 12.5, 1.2),
        ],
        "receipt" => vec![
            pathc(vec![
                L(5.5, 3.5), L(18.5, 3.5), L(18.5, 20.5), L(16.3, 19.0), L(14.2, 20.5),
                L(12.0, 19.0), L(9.8, 20.5), L(7.7, 19.0), L(5.5, 20.5),
            ]),
            p(&[(8.5, 8.0), (15.5, 8.0)]),
            p(&[(8.5, 11.5), (15.5, 11.5)]),
            p(&[(8.5, 15.0), (13.0, 15.0)]),
        ],
        "tag" => vec![
            pathc(vec![
                L(3.5, 3.5), L(11.5, 3.5), L(20.5, 12.5),
                Q(21.5, 13.5, 20.5, 14.5), L(14.5, 20.5),
                Q(13.5, 21.5, 12.5, 20.5), L(3.5, 11.5),
            ]),
            c(7.8, 7.8, 1.6),
        ],
        "percent" => vec![p(&[(5.0, 19.0), (19.0, 5.0)]), c(7.5, 7.5, 2.8), c(16.5, 16.5, 2.8)],
        "store" => vec![
            p(&[(3.5, 9.5), (5.0, 4.5), (19.0, 4.5), (20.5, 9.5)]),
            a(6.3, 9.5, 2.85, 0.0, 180.0),
            a(12.0, 9.5, 2.85, 0.0, 180.0),
            a(17.7, 9.5, 2.85, 0.0, 180.0),
            p(&[(4.5, 12.0), (4.5, 20.0), (19.5, 20.0), (19.5, 12.0)]),
            p(&[(10.0, 20.0), (10.0, 15.0), (14.0, 15.0), (14.0, 20.0)]),
        ],
        "basket" => vec![
            pc(&[(3.5, 9.5), (20.5, 9.5), (18.5, 19.5), (5.5, 19.5)]),
            p(&[(8.5, 9.5), (12.0, 4.2), (15.5, 9.5)]),
            p(&[(9.0, 12.0), (9.7, 17.0)]),
            p(&[(12.0, 12.0), (12.0, 17.0)]),
            p(&[(15.0, 12.0), (14.3, 17.0)]),
        ],
        "cash-register" => vec![
            rr(3.5, 13.0, 17.0, 6.5, 1.0),
            rr(6.0, 7.5, 12.0, 5.5, 1.0),
            rr(8.5, 4.0, 7.0, 3.5, 0.5),
            d(9.0, 10.2, 0.8),
            d(12.0, 10.2, 0.8),
            d(15.0, 10.2, 0.8),
            p(&[(3.5, 16.0), (20.5, 16.0)]),
        ],
        "gift-card" => vec![
            rr(2.5, 5.5, 19.0, 13.0, 2.0),
            c(8.3, 9.0, 1.7),
            c(11.7, 9.0, 1.7),
            p(&[(10.0, 10.5), (10.0, 18.5)]),
        ],
        "voucher" => vec![
            ticket(),
            p(&[(12.0, 8.5), (12.0, 9.8)]),
            p(&[(12.0, 11.4), (12.0, 12.6)]),
            p(&[(12.0, 14.2), (12.0, 15.5)]),
        ],
        "coupon" => vec![
            ticket(),
            p(&[(9.0, 15.0), (15.0, 9.0)]),
            d(9.5, 9.5, 1.1),
            d(14.5, 14.5, 1.1),
        ],
        "discount-badge" => vec![
            pc(&[
                (20.8, 12.0), (18.65, 14.75), (18.22, 18.22), (14.76, 18.65),
                (12.0, 20.8), (9.24, 18.65), (5.78, 18.22), (5.35, 14.75),
                (3.2, 12.0), (5.35, 9.25), (5.78, 5.78), (9.24, 5.35),
                (12.0, 3.2), (14.76, 5.35), (18.22, 5.78), (18.65, 9.25),
            ]),
            p(&[(9.4, 14.6), (14.6, 9.4)]),
            d(9.8, 9.8, 1.0),
            d(14.2, 14.2, 1.0),
        ],
        "open-sign" => vec![
            p(&[(6.0, 3.5), (18.0, 3.5)]),
            p(&[(7.5, 6.5), (9.5, 3.5)]),
            p(&[(16.5, 6.5), (14.5, 3.5)]),
            rr(3.5, 6.5, 17.0, 11.0, 1.5),
            c(9.0, 12.0, 1.6),
            p(&[(12.5, 10.4), (12.5, 13.6)]),
            p(&[(15.5, 10.4), (15.5, 13.6)]),
        ],
        "shopping-bag" => vec![
            pc(&[(5.0, 8.0), (19.0, 8.0), (20.0, 20.5), (4.0, 20.5)]),
            a(12.0, 8.0, 3.5, 180.0, 360.0),
        ],
        "cart-plus" => {
            let mut v = cart_body().to_vec();
            v.push(p(&[(19.5, 3.0), (19.5, 7.6)]));
            v.push(p(&[(17.2, 5.3), (21.8, 5.3)]));
            v
        }
        "cart-minus" => {
            let mut v = cart_body().to_vec();
            v.push(p(&[(17.2, 5.3), (21.8, 5.3)]));
            v
        }
        "point-of-sale" => vec![
            pc(&[(7.0, 4.0), (17.0, 4.0), (18.5, 10.0), (5.5, 10.0)]),
            p(&[(8.0, 6.8), (16.0, 6.8)]),
            rr(5.5, 10.0, 13.0, 9.5, 1.5),
            d(8.5, 13.0, 0.9),
            d(12.0, 13.0, 0.9),
            d(15.5, 13.0, 0.9),
            d(8.5, 16.0, 0.9),
            d(12.0, 16.0, 0.9),
            d(15.5, 16.0, 0.9),
        ],

        // ── File/Folder ────────────────────────────────────────────────────
        "folder" => vec![folder_body()],
        "folder-open" => vec![
            path(vec![
                L(19.0, 11.0), L(19.0, 8.5), L(11.3, 8.5), L(9.0, 6.0), L(3.5, 6.0),
                L(3.5, 19.5),
            ]),
            pathc(vec![L(6.5, 11.0), L(21.5, 11.0), L(19.0, 19.5), L(3.5, 19.5)]),
        ],
        "folder-plus" => vec![
            folder_body(),
            p(&[(12.0, 11.0), (12.0, 17.0)]),
            p(&[(9.0, 14.0), (15.0, 14.0)]),
        ],
        "folder-minus" => vec![folder_body(), p(&[(9.0, 14.0), (15.0, 14.0)])],
        "folder-lock" => vec![
            folder_body(),
            rr(9.8, 12.5, 4.4, 4.5, 0.8),
            a(12.0, 12.5, 1.6, 180.0, 360.0),
        ],
        "folder-search" => vec![
            folder_body(),
            c(11.5, 13.0, 2.8),
            p(&[(13.6, 15.1), (16.0, 17.5)]),
        ],
        "folder-sync" => vec![
            folder_body(),
            a(12.0, 14.0, 3.0, -60.0, 180.0),
            p(&[(13.2, 10.6), (15.0, 11.2), (14.2, 12.9)]),
        ],
        "archive" => vec![
            rr(3.0, 4.5, 18.0, 4.5, 1.0),
            rr(4.5, 9.0, 15.0, 10.5, 1.0),
            p(&[(9.5, 12.5), (14.5, 12.5)]),
        ],
        "trash" => vec![
            p(&[(4.0, 7.0), (20.0, 7.0)]),
            p(&[(9.5, 7.0), (9.5, 4.5), (14.5, 4.5), (14.5, 7.0)]),
            pathc(vec![L(5.5, 7.0), L(6.5, 20.5), L(17.5, 20.5), L(18.5, 7.0)]),
            p(&[(10.0, 10.5), (10.0, 17.0)]),
            p(&[(14.0, 10.5), (14.0, 17.0)]),
        ],
        "printer" => vec![
            rr(7.0, 3.5, 10.0, 4.5, 0.5),
            rr(3.5, 8.0, 17.0, 8.0, 1.5),
            rr(7.0, 13.5, 10.0, 6.5, 0.5),
            d(17.5, 10.5, 0.9),
        ],
        "scanner" => vec![
            rr(3.0, 11.5, 18.0, 7.0, 1.5),
            p(&[(6.0, 14.5), (18.0, 14.5)]),
            p(&[(8.0, 11.5), (9.0, 5.5), (17.5, 5.5), (16.5, 11.5)]),
        ],
        "shredder" => vec![
            rr(3.5, 4.0, 17.0, 6.5, 1.0),
            p(&[(6.5, 7.2), (17.5, 7.2)]),
            p(&[(7.0, 12.5), (7.0, 19.0)]),
            p(&[(10.3, 12.5), (10.3, 16.5)]),
            p(&[(13.7, 12.5), (13.7, 19.0)]),
            p(&[(17.0, 12.5), (17.0, 16.5)]),
        ],
        "file-cabinet" => vec![
            rr(4.5, 3.5, 15.0, 17.0, 1.5),
            p(&[(4.5, 12.0), (19.5, 12.0)]),
            p(&[(10.5, 7.5), (13.5, 7.5)]),
            p(&[(10.5, 16.0), (13.5, 16.0)]),
        ],
        "paperclip" => vec![path(vec![
            L(16.5, 8.0),
            L(16.5, 15.5),
            Q(16.5, 20.0, 12.0, 20.0),
            Q(7.5, 20.0, 7.5, 15.5),
            L(7.5, 8.0),
            Q(7.5, 4.0, 11.0, 4.0),
            Q(14.5, 4.0, 14.5, 8.0),
            L(14.5, 15.0),
            Q(14.5, 17.5, 11.9, 17.5),
            Q(9.5, 17.5, 9.5, 15.0),
            L(9.5, 9.0),
        ])],

        _ => return None,
    })
}

/// An axis-aligned ellipse outline (approximated by four quadratics).
fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> IconShape {
    pathc(vec![
        L(cx - rx, cy),
        Q(cx - rx, cy - ry, cx, cy - ry),
        Q(cx + rx, cy - ry, cx + rx, cy),
        Q(cx + rx, cy + ry, cx, cy + ry),
        Q(cx - rx, cy + ry, cx - rx, cy),
    ])
}

/// An invoice: page + item lines. `with_total` adds the ruled total and a
/// small drawn dollar.
fn invoice_page(with_total: bool) -> Vec<IconShape> {
    let mut v = page().to_vec();
    v.push(p(&[(9.5, 10.5), (14.5, 10.5)]));
    v.push(p(&[(9.5, 13.5), (14.5, 13.5)]));
    if with_total {
        add(&mut v, dollar_glyph(12.0, 17.3, 0.26));
    }
    v
}

/// Payroll, Receivables — chunk 5.
#[rustfmt::skip]
fn payroll_receivables_shapes(name: &str) -> Option<Vec<IconShape>> {
    Some(match name {
        // ── Payroll ────────────────────────────────────────────────────────
        "payroll-check" => {
            let mut v = vec![
                rr(2.5, 7.0, 19.0, 10.0, 1.2),
                p(&[(5.0, 10.5), (10.0, 10.5)]),
                p(&[(5.0, 13.5), (9.0, 13.5)]),
                path(vec![
                    L(13.0, 14.0),
                    Q(14.5, 12.5, 15.5, 14.0), Q(16.5, 15.5, 18.0, 14.0),
                ]),
            ];
            add(&mut v, dollar_glyph(17.0, 10.0, 0.3));
            v
        }
        "payroll-schedule" => {
            let mut v = calendar_body().to_vec();
            v.push(c(12.0, 14.5, 3.2));
            v.push(p(&[(12.0, 12.9), (12.0, 14.5), (13.8, 15.6)]));
            v
        }
        "payroll-deduction" => {
            let mut v = banknote().to_vec();
            v.push(c(18.5, 17.5, 3.5));
            v.push(p(&[(16.7, 17.5), (20.3, 17.5)]));
            v
        }
        "payroll-bonus" => {
            let mut v = banknote().to_vec();
            v.push(c(18.5, 17.5, 3.5));
            v.push(p(&[(16.7, 17.5), (20.3, 17.5)]));
            v.push(p(&[(18.5, 15.7), (18.5, 19.3)]));
            v
        }
        "payroll-overtime" => vec![
            c(10.5, 12.0, 7.0),
            p(&[(10.5, 8.0), (10.5, 12.0), (14.0, 14.0)]),
            p(&[(19.5, 5.5), (19.5, 10.5)]),
            p(&[(17.0, 8.0), (22.0, 8.0)]),
        ],
        "payroll-tax" => {
            let mut v = page().to_vec();
            v.push(p(&[(9.5, 16.5), (14.5, 10.5)]));
            v.push(d(10.0, 11.0, 1.0));
            v.push(d(14.0, 16.0, 1.0));
            v
        }
        "payroll-slip" => {
            let mut v = page().to_vec();
            v.push(p(&[(9.5, 9.5), (14.5, 9.5)]));
            add(&mut v, dollar_glyph(12.0, 14.8, 0.4));
            v
        }
        "payroll-direct-deposit" => vec![
            p(&[(12.0, 2.5), (12.0, 6.5)]),
            p(&[(9.6, 4.4), (12.0, 6.8), (14.4, 4.4)]),
            rr(2.5, 9.0, 19.0, 9.0, 1.5),
            c(12.0, 13.5, 2.4),
        ],
        "payroll-timesheet" => vec![
            rr(3.5, 4.5, 17.0, 15.0, 1.5),
            p(&[(3.5, 9.0), (20.5, 9.0)]),
            p(&[(9.2, 9.0), (9.2, 19.5)]),
            c(15.0, 14.5, 3.0),
            p(&[(15.0, 12.9), (15.0, 14.5), (16.6, 15.5)]),
        ],
        "payroll-hours" => vec![
            c(12.0, 12.0, 8.5),
            p(&[(12.0, 6.5), (12.0, 12.0), (17.0, 15.0)]),
            p(&[(12.0, 3.5), (12.0, 5.0)]),
            p(&[(12.0, 19.0), (12.0, 20.5)]),
            p(&[(3.5, 12.0), (5.0, 12.0)]),
            p(&[(19.0, 12.0), (20.5, 12.0)]),
        ],
        "payroll-employee" => {
            let mut v = person(10.0, 20.0, 0.95).to_vec();
            v.push(c(18.5, 11.0, 3.6));
            add(&mut v, dollar_glyph(18.5, 11.0, 0.32));
            v
        }
        "payroll-benefits" => {
            let mut v = vec![pathc(vec![
                L(12.0, 3.0),
                L(20.0, 6.0), L(20.0, 11.5),
                Q(20.0, 17.5, 12.0, 21.0),
                Q(4.0, 17.5, 4.0, 11.5), L(4.0, 6.0),
            ])];
            add(&mut v, dollar_glyph(12.0, 11.5, 0.5));
            v
        }
        "payroll-pension" => vec![
            a(12.0, 11.0, 8.5, 180.0, 360.0),
            p(&[(3.5, 11.0), (20.5, 11.0)]),
            path(vec![L(12.0, 11.0), L(12.0, 17.5), Q(12.0, 20.0, 9.8, 20.0)]),
            p(&[(12.0, 2.5), (12.0, 4.0)]),
        ],
        "payroll-vacation" => vec![
            rr(4.0, 9.0, 16.0, 10.5, 1.5),
            p(&[(9.5, 9.0), (9.5, 6.5), (14.5, 6.5), (14.5, 9.0)]),
            p(&[(8.5, 9.0), (8.5, 19.5)]),
            p(&[(15.5, 9.0), (15.5, 19.5)]),
        ],
        "payroll-sick-leave" => vec![pc(&[
            (10.0, 4.0), (14.0, 4.0), (14.0, 10.0), (20.0, 10.0), (20.0, 14.0), (14.0, 14.0),
            (14.0, 20.0), (10.0, 20.0), (10.0, 14.0), (4.0, 14.0), (4.0, 10.0), (10.0, 10.0),
        ])],
        "payroll-commission" => {
            let mut v = vec![
                p(&[(3.5, 17.0), (8.0, 12.5), (11.5, 15.0), (17.0, 9.0)]),
                p(&[(14.0, 9.0), (17.0, 9.0), (17.0, 12.0)]),
            ];
            add(&mut v, dollar_glyph(6.0, 6.0, 0.32));
            v
        }
        "payroll-garnishment" => {
            let mut v = banknote().to_vec();
            v.push(p(&[(17.0, 17.0), (21.5, 21.5)]));
            v.push(p(&[(21.5, 18.5), (21.5, 21.5), (18.5, 21.5)]));
            v
        }
        "payroll-reimbursement" => {
            let mut v = vec![
                a(12.0, 12.0, 7.5, 270.0, 540.0),
                p(&[(2.3, 14.4), (4.5, 12.0), (6.7, 14.4)]),
            ];
            add(&mut v, dollar_glyph(12.0, 12.0, 0.42));
            v
        }
        "payroll-w2" => {
            let mut v = page().to_vec();
            v.push(p(&[(8.5, 10.5), (9.8, 16.0), (11.2, 12.5), (12.6, 16.0), (13.9, 10.5)]));
            v.push(p(&[(9.0, 18.5), (15.0, 18.5)]));
            v
        }
        "payroll-1099" => {
            let mut v = page().to_vec();
            v.push(p(&[(7.2, 12.2), (8.4, 11.2)]));
            v.push(p(&[(8.4, 11.2), (8.4, 15.6)]));
            v.push(c(10.6, 13.4, 1.7));
            v.push(c(13.3, 12.9, 1.25));
            v.push(p(&[(14.55, 12.9), (14.55, 15.4)]));
            v.push(c(15.9, 12.9, 1.25));
            v.push(p(&[(17.15, 12.9), (17.15, 15.4)]));
            v
        }
        "payroll-ytd" => {
            let mut v = calendar_body().to_vec();
            v.push(p(&[(6.5, 17.5), (10.0, 14.0), (13.0, 16.0), (17.5, 12.0)]));
            v
        }
        "payroll-net-pay" => {
            let mut v = banknote().to_vec();
            v.push(c(18.5, 17.5, 3.5));
            v.push(p(&[(16.8, 17.5), (18.2, 18.9), (20.4, 16.3)]));
            v
        }
        "payroll-gross-pay" => {
            let mut v = banknote().to_vec();
            v.push(c(18.5, 17.5, 3.5));
            v.push(p(&[(18.5, 19.3), (18.5, 15.9)]));
            v.push(p(&[(16.9, 17.3), (18.5, 15.7), (20.1, 17.3)]));
            v
        }
        "payroll-withholding" => {
            let mut v = banknote().to_vec();
            v.push(rr(16.9, 16.7, 3.2, 2.8, 0.5));
            v.push(a(18.5, 16.7, 1.1, 180.0, 360.0));
            v
        }
        "payroll-frequency" => {
            let mut v = calendar_body().to_vec();
            v.push(a(12.0, 15.0, 3.2, -60.0, 180.0));
            v.push(p(&[(13.3, 11.5), (15.1, 12.1), (14.3, 13.9)]));
            v
        }
        "payroll-raise" => {
            let mut v = person(10.0, 20.0, 0.95).to_vec();
            v.push(p(&[(18.5, 14.0), (18.5, 7.0)]));
            v.push(p(&[(15.8, 9.7), (18.5, 7.0), (21.2, 9.7)]));
            v
        }
        "payroll-severance" => {
            let mut v = person(9.0, 20.0, 0.9).to_vec();
            v.push(p(&[(14.5, 11.0), (21.5, 11.0)]));
            v.push(p(&[(18.8, 8.3), (21.5, 11.0), (18.8, 13.7)]));
            v
        }
        "payroll-stipend" => vec![
            ellipse(12.0, 7.0, 6.5, 2.2),
            p(&[(5.5, 7.0), (5.5, 16.5)]),
            p(&[(18.5, 7.0), (18.5, 16.5)]),
            path(vec![L(5.5, 16.5), Q(5.5, 18.7, 12.0, 18.7), Q(18.5, 18.7, 18.5, 16.5)]),
            path(vec![L(5.5, 10.2), Q(5.5, 12.4, 12.0, 12.4), Q(18.5, 12.4, 18.5, 10.2)]),
            path(vec![L(5.5, 13.4), Q(5.5, 15.6, 12.0, 15.6), Q(18.5, 15.6, 18.5, 13.4)]),
        ],
        "payroll-allowance" => vec![
            path(vec![L(4.0, 17.5), L(16.0, 17.5), Q(19.0, 17.5, 20.0, 15.5), L(20.5, 14.5)]),
            p(&[(4.0, 17.5), (4.0, 14.5), (6.5, 12.5)]),
            c(12.0, 8.5, 3.5),
            p(&[(12.0, 6.9), (12.0, 10.1)]),
        ],
        "payroll-holiday-pay" => {
            let mut v = vec![
                c(12.0, 12.0, 4.5),
                p(&[(12.0, 3.5), (12.0, 5.8)]),
                p(&[(12.0, 18.2), (12.0, 20.5)]),
                p(&[(3.5, 12.0), (5.8, 12.0)]),
                p(&[(18.2, 12.0), (20.5, 12.0)]),
                p(&[(6.0, 6.0), (7.6, 7.6)]),
                p(&[(16.4, 16.4), (18.0, 18.0)]),
                p(&[(18.0, 6.0), (16.4, 7.6)]),
                p(&[(7.6, 16.4), (6.0, 18.0)]),
            ];
            add(&mut v, dollar_glyph(12.0, 12.0, 0.3));
            v
        }
        "payroll-shift-differential" => vec![
            c(12.0, 12.0, 8.0),
            p(&[(12.0, 4.0), (12.0, 20.0)]),
            p(&[(3.5, 12.0), (5.5, 12.0)]),
            p(&[(6.0, 6.0), (7.5, 7.5)]),
            p(&[(6.0, 18.0), (7.5, 16.5)]),
            d(16.5, 9.0, 0.9),
            d(18.5, 13.0, 0.9),
            d(16.0, 16.0, 0.9),
        ],
        "payroll-payday" => {
            let mut v = calendar_body().to_vec();
            add(&mut v, dollar_glyph(12.0, 14.8, 0.4));
            v
        }
        "payroll-bank-details" => vec![
            rr(3.0, 6.0, 18.0, 12.0, 1.5),
            p(&[(5.5, 10.0), (9.0, 7.5), (12.5, 10.0)]),
            p(&[(6.5, 10.0), (6.5, 13.5)]),
            p(&[(9.0, 10.0), (9.0, 13.5)]),
            p(&[(11.5, 10.0), (11.5, 13.5)]),
            p(&[(14.5, 12.0), (19.0, 12.0)]),
            p(&[(14.5, 14.5), (17.5, 14.5)]),
        ],
        "payroll-union-dues" => {
            let mut v = person(9.5, 20.0, 0.88).to_vec();
            v.push(c(16.8, 9.3, 2.8));
            v.push(path(vec![L(16.2, 15.8), Q(20.9, 15.8, 20.9, 20.0)]));
            add(&mut v, dollar_glyph(4.8, 6.5, 0.3));
            v
        }
        "payroll-audit" => {
            let mut v = banknote().to_vec();
            v.push(c(15.0, 15.0, 4.5));
            v.push(p(&[(18.2, 18.2), (21.5, 21.5)]));
            v
        }

        // ── Receivables ────────────────────────────────────────────────────
        "invoice" => invoice_page(true),
        "invoice-paid" => {
            let mut v = invoice_page(false);
            v.push(c(17.5, 17.5, 4.0));
            v.push(p(&[(15.6, 17.5), (17.1, 19.0), (19.6, 16.2)]));
            v
        }
        "invoice-overdue" => {
            let mut v = invoice_page(false);
            v.push(c(17.5, 17.5, 4.0));
            v.push(p(&[(17.5, 15.6), (17.5, 17.5), (19.2, 18.5)]));
            v
        }
        "invoice-draft" => {
            let mut v = invoice_page(false);
            v.push(p(&[(13.5, 18.0), (19.0, 12.5)]));
            v.push(pf(&[(13.5, 18.0), (12.6, 19.4), (14.4, 19.2)]));
            v
        }
        "invoice-send" => {
            let mut v = page().to_vec();
            v.push(p(&[(9.5, 10.5), (14.5, 10.5)]));
            v.push(pc(&[(21.5, 12.5), (13.5, 15.7), (16.6, 17.1), (18.0, 20.3)]));
            v.push(p(&[(21.5, 12.5), (16.6, 17.1)]));
            v
        }
        "credit-memo" => {
            let mut v = invoice_page(false);
            v.push(c(17.5, 17.5, 4.0));
            v.push(p(&[(15.6, 17.5), (19.4, 17.5)]));
            v
        }
        "debit-memo" => {
            let mut v = invoice_page(false);
            v.push(c(17.5, 17.5, 4.0));
            v.push(p(&[(15.6, 17.5), (19.4, 17.5)]));
            v.push(p(&[(17.5, 15.6), (17.5, 19.4)]));
            v
        }
        "aging-report" => vec![
            p(&[(3.5, 19.5), (20.5, 19.5)]),
            rr(5.0, 8.0, 3.0, 11.5, 0.4),
            rr(9.5, 11.0, 3.0, 8.5, 0.4),
            rr(14.0, 13.5, 3.0, 6.0, 0.4),
            c(18.5, 7.0, 3.0),
            p(&[(18.5, 5.4), (18.5, 7.0), (20.0, 8.0)]),
        ],
        "collection" => vec![
            d(8.0, 5.0, 1.2),
            d(12.0, 4.0, 1.2),
            d(16.0, 5.0, 1.2),
            p(&[(12.0, 7.0), (12.0, 11.0)]),
            p(&[(9.8, 9.0), (12.0, 11.3), (14.2, 9.0)]),
            path(vec![L(4.0, 17.5), L(16.0, 17.5), Q(19.0, 17.5, 20.0, 15.5)]),
            p(&[(4.0, 17.5), (4.0, 14.5), (6.5, 12.8)]),
        ],
        "dunning-letter" => vec![
            rr(3.0, 7.0, 18.0, 12.0, 1.5),
            p(&[(3.5, 8.0), (12.0, 14.0), (20.5, 8.0)]),
            c(18.5, 5.5, 3.2),
            p(&[(18.5, 3.9), (18.5, 6.0)]),
            d(18.5, 7.3, 0.8),
        ],
        "payment-received" => vec![
            p(&[(12.0, 2.0), (12.0, 6.0)]),
            p(&[(9.8, 4.2), (12.0, 6.4), (14.2, 4.2)]),
            rr(2.5, 8.5, 19.0, 10.0, 1.5),
            c(12.0, 13.5, 2.5),
        ],
        "partial-payment" => vec![
            rr(2.5, 7.0, 19.0, 10.0, 1.5),
            c(7.5, 12.0, 2.4),
            p(&[(15.0, 10.0), (19.0, 10.0)]),
            p(&[(15.0, 14.0), (19.0, 14.0)]),
        ],
        "advance-payment" => vec![
            rr(2.5, 7.0, 19.0, 10.0, 1.5),
            p(&[(8.0, 9.5), (11.0, 12.0), (8.0, 14.5)]),
            p(&[(12.5, 9.5), (15.5, 12.0), (12.5, 14.5)]),
        ],
        "refund" => vec![
            p(&[(19.0, 5.5), (8.0, 5.5)]),
            p(&[(10.8, 3.0), (8.0, 5.5), (10.8, 8.0)]),
            rr(2.5, 9.5, 19.0, 9.5, 1.5),
            c(12.0, 14.25, 2.5),
        ],
        "write-off" => {
            let mut v = page().to_vec();
            v.push(p(&[(8.0, 10.0), (16.0, 18.0)]));
            v.push(p(&[(16.0, 10.0), (8.0, 18.0)]));
            v
        }
        "bad-debt" => {
            let mut v = vec![rr(2.5, 7.0, 19.0, 10.0, 1.5)];
            v.push(p(&[(12.0, 7.0), (10.5, 10.5), (13.0, 12.5), (11.0, 17.0)]));
            v
        }
        "interest-charge" => vec![
            p(&[(5.5, 18.5), (15.5, 8.5)]),
            c(7.5, 10.5, 2.3),
            c(13.5, 16.5, 2.3),
            p(&[(18.5, 14.0), (18.5, 5.5)]),
            p(&[(16.0, 8.0), (18.5, 5.5), (21.0, 8.0)]),
        ],
        "statement" => {
            let mut v = page().to_vec();
            v.push(p(&[(9.5, 10.0), (14.5, 10.0)]));
            v.push(p(&[(9.5, 12.5), (14.5, 12.5)]));
            v.push(p(&[(9.5, 15.2), (14.5, 15.2)]));
            add(&mut v, dollar_glyph(12.0, 18.2, 0.24));
            v
        }
        "customer-balance" => {
            let mut v = person(9.0, 20.0, 0.85).to_vec();
            v.push(p(&[(14.5, 8.0), (21.5, 8.0)]));
            v.push(p(&[(18.0, 8.0), (18.0, 12.5)]));
            v.push(pc(&[(14.6, 8.0), (16.4, 8.0), (15.5, 10.5)]));
            v.push(pc(&[(19.6, 8.0), (21.4, 8.0), (20.5, 10.5)]));
            v
        }
        "account-receivable" => vec![
            rr(2.5, 8.5, 19.0, 10.0, 1.5),
            c(11.0, 13.5, 2.4),
            p(&[(21.5, 2.5), (16.5, 7.5)]),
            p(&[(16.5, 4.7), (16.5, 7.5), (19.3, 7.5)]),
        ],
        "open-items" => vec![
            rr(4.0, 4.5, 4.0, 4.0, 0.8),
            p(&[(4.9, 6.6), (5.9, 7.6), (7.3, 5.7)]),
            p(&[(10.5, 6.5), (20.0, 6.5)]),
            rr(4.0, 10.0, 4.0, 4.0, 0.8),
            p(&[(10.5, 12.0), (20.0, 12.0)]),
            rr(4.0, 15.5, 4.0, 4.0, 0.8),
            p(&[(10.5, 17.5), (20.0, 17.5)]),
        ],
        "clearing" => vec![
            p(&[(3.5, 8.0), (10.0, 8.0)]),
            p(&[(7.8, 5.8), (10.2, 8.0), (7.8, 10.2)]),
            p(&[(20.5, 16.0), (14.0, 16.0)]),
            p(&[(16.7, 13.8), (14.3, 16.0), (16.7, 18.2)]),
            p(&[(9.5, 13.0), (11.5, 15.0), (15.0, 10.5)]),
        ],
        "remittance" => {
            let mut v = vec![
                rr(3.0, 6.5, 18.0, 12.0, 1.5),
                p(&[(3.5, 7.5), (12.0, 13.5), (20.5, 7.5)]),
                c(18.5, 17.5, 3.4),
            ];
            add(&mut v, dollar_glyph(18.5, 17.5, 0.24));
            v
        }
        "factoring" => vec![
            rr(3.0, 4.5, 8.0, 11.0, 1.0),
            p(&[(4.8, 7.5), (9.2, 7.5)]),
            p(&[(4.8, 10.5), (9.2, 10.5)]),
            p(&[(12.0, 10.0), (15.5, 10.0)]),
            p(&[(13.9, 8.4), (15.8, 10.0), (13.9, 11.6)]),
            c(18.5, 10.0, 2.8),
            p(&[(18.5, 8.7), (18.5, 11.3)]),
        ],
        "credit-limit" => vec![
            rr(3.0, 13.5, 10.0, 7.0, 1.0),
            p(&[(3.0, 15.8), (13.0, 15.8)]),
            a(17.0, 9.0, 4.5, 180.0, 360.0),
            p(&[(17.0, 9.0), (19.8, 6.2)]),
            d(17.0, 9.0, 1.0),
        ],
        "invoice-recurring" => {
            let mut v = page().to_vec();
            v.push(p(&[(9.5, 10.5), (14.5, 10.5)]));
            v.push(a(17.5, 17.5, 3.0, -60.0, 180.0));
            v.push(p(&[(18.7, 13.9), (20.4, 14.6), (19.6, 16.3)]));
            v
        }
        "invoice-dispute" => {
            let mut v = invoice_page(false);
            v.push(p(&[(12.0, 10.0), (12.0, 14.5)]));
            v.push(d(12.0, 17.0, 1.1));
            v
        }
        "promissory-note" => {
            let mut v = page().to_vec();
            v.push(path(vec![
                L(8.5, 15.5),
                Q(10.0, 13.5, 11.0, 15.0), Q(12.0, 16.5, 13.0, 15.0),
            ]));
            v.push(c(15.5, 17.5, 1.8));
            v
        }
        "payment-reminder" => vec![
            path(vec![
                L(6.0, 17.5), L(6.0, 10.5),
                Q(6.0, 4.5, 12.0, 4.5), Q(18.0, 4.5, 18.0, 10.5), L(18.0, 17.5),
            ]),
            p(&[(4.5, 17.5), (19.5, 17.5)]),
            p(&[(12.0, 8.5), (12.0, 12.5)]),
            d(12.0, 14.8, 0.9),
        ],
        "grace-period" => {
            let mut v = calendar_body().to_vec();
            v.push(p(&[(10.0, 12.0), (14.0, 12.0)]));
            v.push(p(&[(10.0, 18.5), (14.0, 18.5)]));
            v.push(path(vec![
                L(10.7, 12.0), L(10.7, 13.5),
                Q(10.7, 14.8, 12.0, 15.3), Q(13.3, 14.8, 13.3, 13.5), L(13.3, 12.0),
            ]));
            v.push(path(vec![
                L(10.7, 18.5), L(10.7, 17.0),
                Q(10.7, 15.8, 12.0, 15.3), Q(13.3, 15.8, 13.3, 17.0), L(13.3, 18.5),
            ]));
            v
        }
        "late-fee" => {
            let mut v = vec![
                c(10.5, 12.0, 7.5),
                p(&[(10.5, 7.5), (10.5, 12.0), (14.5, 14.0)]),
                c(19.5, 17.5, 3.4),
            ];
            add(&mut v, dollar_glyph(19.5, 17.5, 0.24));
            v
        }
        "credit-score" => vec![
            a(12.0, 15.0, 8.5, 180.0, 360.0),
            p(&[(12.0, 15.0), (17.0, 10.0)]),
            d(12.0, 15.0, 1.4),
            d(5.5, 10.5, 0.8),
            d(12.0, 5.7, 0.8),
            d(18.5, 10.5, 0.8),
        ],
        "cash-application" => vec![
            rr(2.5, 9.0, 19.0, 9.5, 1.5),
            c(17.0, 6.0, 3.5),
            p(&[(13.0, 6.0), (14.7, 6.0)]),
            p(&[(19.3, 6.0), (21.0, 6.0)]),
            p(&[(17.0, 2.2), (17.0, 3.9)]),
            d(17.0, 6.0, 1.0),
        ],
        "collections-call" => {
            let mut v = vec![phone_handset()];
            add(&mut v, dollar_glyph(18.0, 6.0, 0.32));
            v
        }
        "guarantor" => {
            let mut v = person(10.0, 20.0, 0.95).to_vec();
            v.push(pathc(vec![
                L(19.0, 4.0),
                L(21.8, 5.0), L(21.8, 7.6),
                Q(21.8, 9.8, 19.0, 11.0),
                Q(16.2, 9.8, 16.2, 7.6), L(16.2, 5.0),
            ]));
            v
        }

        _ => return None,
    })
}

/// A credit card: rounded rect + magnetic band.
fn card_body() -> [IconShape; 2] {
    [rr(2.5, 6.5, 19.0, 12.0, 2.0), p(&[(2.5, 10.0), (21.5, 10.0)])]
}

/// A bank: pediment, columns, base.
fn bank_body() -> [IconShape; 6] {
    [
        p(&[(4.0, 8.5), (12.0, 3.5), (20.0, 8.5)]),
        p(&[(5.0, 11.0), (19.0, 11.0)]),
        p(&[(6.5, 11.0), (6.5, 16.0)]),
        p(&[(12.0, 11.0), (12.0, 16.0)]),
        p(&[(17.5, 11.0), (17.5, 16.0)]),
        p(&[(4.5, 18.5), (19.5, 18.5)]),
    ]
}

/// An isometric carton: diamond top + two visible faces.
fn box3d() -> [IconShape; 3] {
    [
        pc(&[(3.5, 8.0), (12.0, 4.0), (20.5, 8.0), (12.0, 12.0)]),
        p(&[(3.5, 8.0), (3.5, 16.0), (12.0, 20.5), (12.0, 12.0)]),
        p(&[(20.5, 8.0), (20.5, 16.0), (12.0, 20.5)]),
    ]
}

/// Payments, Stock Control — chunk 6.
#[rustfmt::skip]
fn payments_stock_shapes(name: &str) -> Option<Vec<IconShape>> {
    Some(match name {
        // ── Payments ───────────────────────────────────────────────────────
        "payment-check" => vec![
            rr(2.5, 7.0, 19.0, 10.0, 1.2),
            p(&[(5.0, 10.5), (10.0, 10.5)]),
            p(&[(5.0, 13.5), (9.0, 13.5)]),
            p(&[(13.0, 12.0), (15.5, 14.5), (19.5, 9.5)]),
        ],
        "payment-wire" => vec![
            c(12.0, 12.0, 8.0),
            ellipse(12.0, 12.0, 3.6, 8.0),
            p(&[(4.0, 12.0), (20.0, 12.0)]),
        ],
        "payment-ach" => {
            let mut v = bank_body().to_vec();
            v.push(a(12.0, 14.0, 3.0, -60.0, 180.0));
            v.push(p(&[(13.3, 10.5), (15.1, 11.1), (14.3, 12.9)]));
            v
        }
        "payment-cash" => {
            let mut v = banknote().to_vec();
            v.push(c(17.5, 17.5, 2.8));
            v.push(p(&[(17.5, 16.3), (17.5, 18.7)]));
            v
        }
        "payment-pending" => {
            let mut v = banknote().to_vec();
            v.push(p(&[(16.8, 15.5), (20.2, 15.5)]));
            v.push(p(&[(16.8, 20.5), (20.2, 20.5)]));
            v.push(path(vec![
                L(17.4, 15.5), L(17.4, 16.6),
                Q(17.4, 17.5, 18.5, 18.0), Q(19.6, 17.5, 19.6, 16.6), L(19.6, 15.5),
            ]));
            v.push(path(vec![
                L(17.4, 20.5), L(17.4, 19.4),
                Q(17.4, 18.5, 18.5, 18.0), Q(19.6, 18.5, 19.6, 19.4), L(19.6, 20.5),
            ]));
            v
        }
        "payment-approved" => {
            let mut v = card_body().to_vec();
            v.push(p(&[(7.5, 14.5), (10.0, 17.0), (16.0, 11.5)]));
            v
        }
        "payment-rejected" => {
            let mut v = card_body().to_vec();
            v.push(p(&[(9.0, 12.5), (15.0, 17.5)]));
            v.push(p(&[(15.0, 12.5), (9.0, 17.5)]));
            v
        }
        "payment-recurring" => {
            let mut v = card_body().to_vec();
            v.push(a(12.0, 14.5, 2.8, -60.0, 180.0));
            v.push(p(&[(13.1, 11.3), (14.8, 11.9), (14.0, 13.5)]));
            v
        }
        "payment-split" => vec![
            rr(2.5, 10.0, 19.0, 9.0, 1.5),
            c(12.0, 14.5, 2.4),
            p(&[(7.0, 7.0), (4.0, 4.0)]),
            p(&[(4.0, 6.5), (4.0, 4.0), (6.5, 4.0)]),
            p(&[(17.0, 7.0), (20.0, 4.0)]),
            p(&[(17.5, 4.0), (20.0, 4.0), (20.0, 6.5)]),
        ],
        "payment-batch" => vec![
            rr(4.5, 10.5, 15.0, 8.0, 1.0),
            p(&[(6.0, 8.0), (18.0, 8.0)]),
            p(&[(7.5, 5.5), (16.5, 5.5)]),
            c(12.0, 14.5, 2.2),
        ],
        "payment-void" => vec![
            rr(2.5, 7.0, 19.0, 10.0, 1.5),
            c(12.0, 12.0, 2.5),
            p(&[(4.0, 20.0), (20.0, 4.0)]),
        ],
        "payment-reversal" => vec![
            path(vec![
                L(6.0, 8.0), L(6.0, 5.5),
                Q(6.0, 3.0, 9.0, 3.0), L(15.0, 3.0),
                Q(18.0, 3.0, 18.0, 5.5), L(18.0, 8.0),
            ]),
            p(&[(15.7, 6.0), (18.0, 8.4), (20.3, 6.0)]),
            rr(2.5, 11.0, 19.0, 8.5, 1.5),
            c(12.0, 15.25, 2.3),
        ],
        "vendor-payment" => {
            let mut v = person(9.0, 20.0, 0.9).to_vec();
            v.push(rr(13.5, 8.0, 8.0, 5.5, 0.8));
            v.push(c(17.5, 10.75, 1.4));
            v
        }
        "bill-pay" => {
            let mut v = page().to_vec();
            v.push(rr(11.0, 13.0, 9.5, 6.0, 1.0));
            v.push(p(&[(11.0, 15.0), (20.5, 15.0)]));
            v
        }
        "purchase-order" => {
            let mut v = page().to_vec();
            v.push(p(&[
                (8.5, 11.0), (9.7, 11.0), (10.8, 15.8), (15.3, 15.8), (16.3, 12.2), (10.3, 12.2),
            ]));
            v.push(d(11.7, 17.9, 1.0));
            v.push(d(14.6, 17.9, 1.0));
            v
        }
        "expense-report" => {
            let mut v = page().to_vec();
            v.push(p(&[(9.0, 11.0), (15.5, 17.0)]));
            v.push(p(&[(15.5, 14.0), (15.5, 17.0), (12.5, 17.0)]));
            v
        }
        "petty-cash" => vec![
            c(9.5, 14.0, 4.5),
            p(&[(9.5, 12.0), (9.5, 16.0)]),
            c(16.0, 10.5, 3.2),
            p(&[(16.0, 9.1), (16.0, 11.9)]),
        ],
        "bank-transfer" => {
            let mut v = bank_body().to_vec();
            v.push(p(&[(7.0, 21.0), (17.0, 21.0)]));
            v.push(p(&[(8.8, 19.4), (6.8, 21.0), (8.8, 22.6)]));
            v.push(p(&[(15.2, 19.4), (17.2, 21.0), (15.2, 22.6)]));
            v
        }
        "payment-gateway" => vec![
            path(vec![
                L(5.0, 20.0), L(5.0, 10.0),
                Q(5.0, 4.0, 12.0, 4.0), Q(19.0, 4.0, 19.0, 10.0), L(19.0, 20.0),
            ]),
            p(&[(8.0, 14.0), (15.0, 14.0)]),
            p(&[(12.7, 11.7), (15.2, 14.0), (12.7, 16.3)]),
        ],
        "payment-terms" => {
            let mut v = page().to_vec();
            v.push(p(&[(9.5, 10.5), (11.0, 10.5)]));
            v.push(p(&[(13.0, 10.5), (14.5, 10.5)]));
            v.push(p(&[(9.5, 13.0), (11.0, 13.0)]));
            v.push(p(&[(13.0, 13.0), (14.5, 13.0)]));
            v.push(p(&[(9.5, 15.5), (11.0, 15.5)]));
            v.push(p(&[(13.0, 15.5), (14.5, 15.5)]));
            v
        }
        "early-discount" => vec![
            c(10.0, 12.0, 7.5),
            p(&[(10.0, 8.0), (10.0, 12.0), (13.0, 13.5)]),
            p(&[(16.5, 9.0), (21.5, 4.0)]),
            d(17.2, 4.8, 0.9),
            d(20.8, 8.2, 0.9),
        ],
        "payment-plan" => {
            let mut v = calendar_body().to_vec();
            v.push(d(8.0, 13.0, 1.1));
            v.push(d(12.0, 13.0, 1.1));
            v.push(d(16.0, 13.0, 1.1));
            v.push(d(8.0, 17.0, 1.1));
            v.push(d(12.0, 17.0, 1.1));
            v
        }
        "installment" => vec![
            rr(3.5, 10.0, 5.0, 4.5, 0.6),
            rr(9.5, 10.0, 5.0, 4.5, 0.6),
            rr(15.5, 10.0, 5.0, 4.5, 0.6),
            p(&[(4.7, 17.5), (5.8, 18.6), (7.6, 16.4)]),
            p(&[(11.0, 17.7), (13.0, 17.7)]),
            p(&[(17.0, 17.7), (19.0, 17.7)]),
        ],
        "escrow" => vec![
            rr(5.5, 11.0, 13.0, 9.0, 1.5),
            path(vec![
                L(8.0, 11.0), L(8.0, 8.0),
                Q(8.0, 4.5, 12.0, 4.5), Q(16.0, 4.5, 16.0, 8.0), L(16.0, 11.0),
            ]),
            c(12.0, 15.5, 2.2),
            p(&[(12.0, 14.5), (12.0, 16.5)]),
        ],
        "disbursement" => vec![
            rr(7.0, 4.0, 10.0, 7.0, 1.0),
            c(12.0, 7.5, 1.7),
            p(&[(6.0, 13.5), (6.0, 17.5)]),
            p(&[(4.3, 15.9), (6.0, 17.7), (7.7, 15.9)]),
            p(&[(12.0, 13.5), (12.0, 19.5)]),
            p(&[(10.3, 17.9), (12.0, 19.7), (13.7, 17.9)]),
            p(&[(18.0, 13.5), (18.0, 17.5)]),
            p(&[(16.3, 15.9), (18.0, 17.7), (19.7, 15.9)]),
        ],
        "payment-mobile" => {
            let mut v = vec![
                rr(7.0, 3.0, 10.0, 18.0, 2.0),
                p(&[(10.5, 5.2), (13.5, 5.2)]),
                d(12.0, 18.8, 0.9),
            ];
            add(&mut v, dollar_glyph(12.0, 12.0, 0.34));
            v
        }
        "payment-qr" => vec![
            rr(7.0, 3.0, 10.0, 18.0, 2.0),
            rr(9.0, 7.0, 6.0, 6.0, 0.5),
            rrf(10.7, 8.7, 2.6, 2.6, 0.3),
            d(9.8, 15.5, 0.8),
            d(12.0, 14.0, 0.8),
            d(14.2, 15.8, 0.8),
        ],
        "payment-contactless" => vec![
            rr(2.5, 7.0, 15.0, 10.5, 1.5),
            p(&[(2.5, 10.2), (17.5, 10.2)]),
            a(18.5, 12.2, 2.2, -55.0, 55.0),
            a(18.5, 12.2, 4.2, -50.0, 50.0),
        ],
        "payment-crypto" => vec![
            pc(&[
                (12.0, 3.5), (19.4, 7.75), (19.4, 16.25), (12.0, 20.5),
                (4.6, 16.25), (4.6, 7.75),
            ]),
            p(&[(9.5, 8.0), (9.5, 16.0)]),
            path(vec![
                L(9.5, 8.0), L(13.0, 8.0),
                Q(15.0, 8.0, 15.0, 10.0), Q(15.0, 12.0, 13.0, 12.0), L(9.5, 12.0),
            ]),
            path(vec![
                L(9.5, 12.0), L(13.5, 12.0),
                Q(15.5, 12.0, 15.5, 14.0), Q(15.5, 16.0, 13.5, 16.0), L(9.5, 16.0),
            ]),
            p(&[(11.0, 6.8), (11.0, 8.0)]),
            p(&[(11.0, 16.0), (11.0, 17.2)]),
        ],
        "payment-standing-order" => {
            let mut v = calendar_body().to_vec();
            v.push(p(&[(8.0, 13.5), (14.5, 13.5)]));
            v.push(p(&[(12.7, 11.9), (14.7, 13.5), (12.7, 15.1)]));
            v.push(p(&[(16.0, 16.5), (9.5, 16.5)]));
            v.push(p(&[(11.3, 14.9), (9.3, 16.5), (11.3, 18.1)]));
            v
        }
        "payment-authorization" => {
            let mut v = card_body().to_vec();
            v.push(c(8.0, 14.5, 1.8));
            v.push(p(&[(9.4, 14.5), (14.5, 14.5)]));
            v.push(p(&[(12.5, 14.5), (12.5, 16.0)]));
            v.push(p(&[(14.5, 14.5), (14.5, 16.3)]));
            v
        }
        "payment-capture" => {
            let mut v = card_body().to_vec();
            v.push(p(&[(12.0, 11.5), (12.0, 16.5)]));
            v.push(p(&[(9.8, 14.5), (12.0, 16.8), (14.2, 14.5)]));
            v
        }
        "payment-limit" => {
            let mut v = card_body().to_vec();
            v.push(p(&[(5.5, 14.5), (15.0, 14.5)]));
            v.push(p(&[(17.5, 12.5), (17.5, 16.5)]));
            v
        }
        "payment-receipt" => {
            let mut v = vec![pathc(vec![
                L(6.5, 3.5), L(17.5, 3.5), L(17.5, 20.0), L(15.7, 18.7), L(13.9, 20.0),
                L(12.0, 18.7), L(10.1, 20.0), L(8.3, 18.7), L(6.5, 20.0),
            ])];
            add(&mut v, dollar_glyph(12.0, 9.5, 0.3));
            v.push(p(&[(9.0, 14.5), (15.0, 14.5)]));
            v
        }
        "payment-schedule" => vec![
            p(&[(4.0, 6.0), (13.0, 6.0)]),
            p(&[(4.0, 12.0), (13.0, 12.0)]),
            p(&[(4.0, 18.0), (13.0, 18.0)]),
            c(18.0, 12.0, 3.5),
            p(&[(18.0, 10.2), (18.0, 12.0), (19.8, 13.0)]),
        ],

        // ── Stock Control ──────────────────────────────────────────────────
        "inventory" => box3d().to_vec(),
        "warehouse" => vec![
            p(&[(3.0, 10.0), (12.0, 4.0), (21.0, 10.0)]),
            p(&[(4.5, 9.0), (4.5, 20.0)]),
            p(&[(19.5, 9.0), (19.5, 20.0)]),
            p(&[(4.5, 20.0), (19.5, 20.0)]),
            rr(7.0, 14.5, 4.5, 5.5, 0.4),
            rr(12.5, 14.5, 4.5, 5.5, 0.4),
        ],
        "stock-in" => vec![
            pc(&[(5.0, 10.0), (12.0, 6.5), (19.0, 10.0), (12.0, 13.5)]),
            p(&[(5.0, 10.0), (5.0, 16.5), (12.0, 20.0), (12.0, 13.5)]),
            p(&[(19.0, 10.0), (19.0, 16.5), (12.0, 20.0)]),
            p(&[(12.0, 1.5), (12.0, 5.0)]),
            p(&[(10.0, 3.2), (12.0, 5.3), (14.0, 3.2)]),
        ],
        "stock-out" => vec![
            pc(&[(5.0, 10.0), (12.0, 6.5), (19.0, 10.0), (12.0, 13.5)]),
            p(&[(5.0, 10.0), (5.0, 16.5), (12.0, 20.0), (12.0, 13.5)]),
            p(&[(19.0, 10.0), (19.0, 16.5), (12.0, 20.0)]),
            p(&[(12.0, 5.0), (12.0, 1.5)]),
            p(&[(10.0, 3.3), (12.0, 1.3), (14.0, 3.3)]),
        ],
        "stock-count" => {
            let mut v = box3d().to_vec();
            v.push(p(&[(5.5, 13.0), (7.3, 14.8), (10.5, 11.3)]));
            v
        }
        "stock-transfer" => vec![
            rr(3.0, 5.0, 7.0, 7.0, 0.8),
            rr(14.0, 12.0, 7.0, 7.0, 0.8),
            p(&[(11.5, 8.5), (17.5, 8.5), (17.5, 10.5)]),
            p(&[(15.7, 8.9), (17.5, 10.8), (19.3, 8.9)]),
        ],
        "stock-adjust" => {
            let mut v = box3d().to_vec();
            v.push(p(&[(2.8, 4.0), (6.2, 4.0)]));
            v.push(p(&[(17.8, 4.0), (21.2, 4.0)]));
            v.push(p(&[(19.5, 2.3), (19.5, 5.7)]));
            v
        }
        "stock-reserve" => {
            let mut v = box3d().to_vec();
            v.push(rr(9.8, 14.2, 4.4, 4.0, 0.6));
            v.push(a(12.0, 14.2, 1.5, 180.0, 360.0));
            v
        }
        "stock-alert" => {
            let mut v = box3d().to_vec();
            v.push(c(19.5, 4.5, 3.0));
            v.push(p(&[(19.5, 3.0), (19.5, 4.9)]));
            v.push(d(19.5, 6.2, 0.7));
            v
        }
        "stock-reorder" => {
            let mut v = box3d().to_vec();
            v.push(a(12.0, 16.0, 2.8, -60.0, 180.0));
            v.push(p(&[(13.1, 12.9), (14.8, 13.5), (14.0, 15.0)]));
            v
        }
        "barcode" => vec![
            p(&[(3.5, 6.0), (3.5, 18.0)]),
            p(&[(6.5, 6.0), (6.5, 15.0)]),
            p(&[(9.0, 6.0), (9.0, 18.0)]),
            p(&[(11.5, 6.0), (11.5, 15.0)]),
            p(&[(14.0, 6.0), (14.0, 18.0)]),
            p(&[(16.5, 6.0), (16.5, 15.0)]),
            p(&[(20.5, 6.0), (20.5, 18.0)]),
        ],
        "qr-code" => vec![
            rr(4.0, 4.0, 6.0, 6.0, 0.5),
            rrf(5.8, 5.8, 2.4, 2.4, 0.3),
            rr(14.0, 4.0, 6.0, 6.0, 0.5),
            rrf(15.8, 5.8, 2.4, 2.4, 0.3),
            rr(4.0, 14.0, 6.0, 6.0, 0.5),
            rrf(5.8, 15.8, 2.4, 2.4, 0.3),
            d(14.8, 14.8, 1.0),
            d(18.8, 14.8, 1.0),
            d(14.8, 18.8, 1.0),
            d(18.8, 18.8, 1.0),
            d(16.8, 16.8, 1.0),
        ],
        "pallet" => vec![
            p(&[(3.0, 15.0), (21.0, 15.0)]),
            p(&[(3.0, 18.5), (21.0, 18.5)]),
            p(&[(5.0, 15.0), (5.0, 18.5)]),
            p(&[(12.0, 15.0), (12.0, 18.5)]),
            p(&[(19.0, 15.0), (19.0, 18.5)]),
            rr(5.0, 8.0, 6.0, 7.0, 0.5),
            rr(12.5, 8.0, 6.0, 7.0, 0.5),
        ],
        "shelf" => vec![
            p(&[(4.0, 3.5), (4.0, 20.5)]),
            p(&[(20.0, 3.5), (20.0, 20.5)]),
            p(&[(4.0, 11.0), (20.0, 11.0)]),
            p(&[(4.0, 19.5), (20.0, 19.5)]),
            rr(6.5, 6.0, 4.5, 5.0, 0.4),
            rr(13.0, 6.0, 4.5, 5.0, 0.4),
            rr(9.5, 14.5, 4.5, 5.0, 0.4),
        ],
        "bin-location" => vec![
            pathc(vec![
                L(12.0, 11.0),
                Q(8.8, 7.0, 8.8, 5.2), Q(8.8, 2.2, 12.0, 2.2),
                Q(15.2, 2.2, 15.2, 5.2), Q(15.2, 7.0, 12.0, 11.0),
            ]),
            c(12.0, 5.0, 1.3),
            rr(5.0, 14.0, 14.0, 6.5, 0.8),
            p(&[(12.0, 14.0), (12.0, 20.5)]),
        ],
        "lot-number" => {
            let mut v = box3d().to_vec();
            v.push(rr(15.5, 2.0, 6.0, 4.0, 0.8));
            v.push(d(17.0, 4.0, 0.7));
            v
        }
        "serial-number" => vec![
            p(&[(4.0, 15.0), (4.0, 20.0)]),
            p(&[(6.5, 15.0), (6.5, 20.0)]),
            p(&[(9.0, 15.0), (9.0, 20.0)]),
            p(&[(11.5, 15.0), (11.5, 20.0)]),
            p(&[(14.0, 5.0), (12.5, 11.0)]),
            p(&[(18.0, 5.0), (16.5, 11.0)]),
            p(&[(11.8, 6.8), (19.3, 6.8)]),
            p(&[(11.2, 9.2), (18.7, 9.2)]),
        ],
        "expiry-date" => {
            let mut v = calendar_body().to_vec();
            v.push(p(&[(12.0, 11.5), (12.0, 15.0)]));
            v.push(d(12.0, 17.3, 1.0));
            v
        }
        "fifo" => vec![
            rr(4.5, 9.5, 4.0, 5.0, 0.5),
            rr(10.0, 9.5, 4.0, 5.0, 0.5),
            rr(15.5, 9.5, 4.0, 5.0, 0.5),
            p(&[(6.5, 3.5), (6.5, 7.5)]),
            p(&[(4.9, 6.0), (6.5, 7.8), (8.1, 6.0)]),
            p(&[(17.5, 16.5), (17.5, 20.5)]),
            p(&[(15.9, 19.0), (17.5, 20.8), (19.1, 19.0)]),
        ],
        "lifo" => vec![
            rr(8.0, 14.5, 8.0, 5.0, 0.5),
            rr(8.0, 8.5, 8.0, 5.0, 0.5),
            p(&[(5.5, 3.0), (5.5, 7.0)]),
            p(&[(3.9, 5.5), (5.5, 7.3), (7.1, 5.5)]),
            p(&[(18.5, 7.0), (18.5, 3.0)]),
            p(&[(16.9, 4.5), (18.5, 2.7), (20.1, 4.5)]),
        ],
        "cycle-count" => vec![
            rr(9.0, 9.7, 6.0, 4.6, 0.6),
            a(12.0, 12.0, 8.3, 240.0, 360.0),
            p(&[(18.1, 9.8), (20.3, 12.0), (22.5, 9.8)]),
            a(12.0, 12.0, 8.3, 60.0, 180.0),
            p(&[(1.5, 14.2), (3.7, 12.0), (5.9, 14.2)]),
        ],
        "physical-count" => vec![
            rr(5.5, 4.5, 13.0, 17.0, 1.5),
            rr(9.5, 2.8, 5.0, 3.4, 1.0),
            rr(8.5, 9.0, 7.0, 5.0, 0.6),
            p(&[(9.5, 17.0), (11.0, 18.5), (14.5, 15.0)]),
        ],
        "stock-valuation" => {
            let mut v = box3d().to_vec();
            add(&mut v, dollar_glyph(12.0, 16.4, 0.28));
            v
        }
        "safety-stock" => {
            let mut v = box3d().to_vec();
            v.push(pathc(vec![
                L(19.0, 2.0),
                L(21.8, 3.0), L(21.8, 5.6),
                Q(21.8, 7.8, 19.0, 9.0),
                Q(16.2, 7.8, 16.2, 5.6), L(16.2, 3.0),
            ]));
            v
        }
        "dead-stock" => {
            let mut v = box3d().to_vec();
            v.push(p(&[(17.5, 2.5), (21.5, 6.5)]));
            v.push(p(&[(21.5, 2.5), (17.5, 6.5)]));
            v
        }
        "sku" => vec![
            pathc(vec![
                L(3.5, 3.5), L(11.5, 3.5), L(20.5, 12.5),
                Q(21.5, 13.5, 20.5, 14.5), L(14.5, 20.5),
                Q(13.5, 21.5, 12.5, 20.5), L(3.5, 11.5),
            ]),
            c(7.8, 7.8, 1.6),
            p(&[(10.8, 10.2), (13.6, 13.0)]),
            p(&[(13.0, 8.0), (15.8, 10.8)]),
        ],
        "stock-aging" => {
            let mut v = box3d().to_vec();
            v.push(p(&[(16.6, 2.2), (21.4, 2.2)]));
            v.push(p(&[(16.6, 8.2), (21.4, 8.2)]));
            v.push(path(vec![
                L(17.3, 2.2), L(17.3, 3.6),
                Q(17.3, 4.7, 19.0, 5.2), Q(20.7, 4.7, 20.7, 3.6), L(20.7, 2.2),
            ]));
            v.push(path(vec![
                L(17.3, 8.2), L(17.3, 6.8),
                Q(17.3, 5.7, 19.0, 5.2), Q(20.7, 5.7, 20.7, 6.8), L(20.7, 8.2),
            ]));
            v
        }
        "stock-return" => {
            let mut v = box3d().to_vec();
            v.push(p(&[(20.5, 4.0), (15.0, 4.0)]));
            v.push(p(&[(17.0, 1.8), (14.7, 4.0), (17.0, 6.2)]));
            v
        }
        "stock-damage" => {
            let mut v = box3d().to_vec();
            v.push(p(&[(6.5, 12.5), (8.0, 14.5), (6.5, 16.5), (8.0, 18.5)]));
            v
        }
        "stock-quarantine" => {
            let mut v = box3d().to_vec();
            v.push(pc(&[(19.0, 2.0), (22.0, 7.0), (16.0, 7.0)]));
            v.push(p(&[(19.0, 3.8), (19.0, 5.0)]));
            v.push(d(19.0, 6.1, 0.5));
            v
        }
        "stock-picking" => {
            let mut v = box3d().to_vec();
            v.push(p(&[(14.0, 9.0), (19.5, 3.5)]));
            v.push(p(&[(15.7, 3.5), (19.5, 3.5), (19.5, 7.3)]));
            v
        }
        "stock-putaway" => {
            let mut v = box3d().to_vec();
            v.push(p(&[(19.5, 3.5), (14.0, 9.0)]));
            v.push(p(&[(14.0, 5.2), (14.0, 9.0), (17.8, 9.0)]));
            v
        }
        "kitting" => vec![
            rr(3.0, 4.0, 5.0, 5.0, 0.5),
            rr(3.0, 10.5, 5.0, 5.0, 0.5),
            rr(3.0, 17.0, 5.0, 5.0, 0.5),
            p(&[(8.5, 6.5), (13.5, 9.5)]),
            p(&[(8.5, 13.0), (13.5, 12.0)]),
            p(&[(8.5, 19.5), (13.5, 14.5)]),
            rr(14.0, 8.0, 7.5, 8.0, 0.8),
        ],
        "batch-tracking" => vec![
            rr(3.0, 3.5, 6.0, 6.0, 0.6),
            rr(15.0, 14.5, 6.0, 6.0, 0.6),
            path(vec![L(9.5, 6.5), Q(18.0, 6.5, 18.0, 14.0)]),
            p(&[(16.4, 12.2), (18.0, 14.2), (19.6, 12.2)]),
        ],
        "min-max-level" => vec![
            rr(6.0, 4.0, 12.0, 16.5, 1.0),
            p(&[(6.0, 8.0), (18.0, 8.0)]),
            p(&[(6.0, 16.0), (18.0, 16.0)]),
            p(&[(19.0, 8.0), (21.5, 8.0)]),
            p(&[(19.0, 16.0), (21.5, 16.0)]),
            path(vec![
                L(7.5, 12.0),
                Q(9.0, 10.8, 10.5, 12.0), Q(12.0, 13.2, 13.5, 12.0), Q(15.0, 10.8, 16.5, 12.0),
            ]),
        ],

        _ => return None,
    })
}

/// The classic top-view airplane silhouette, scaled by `k` about the grid
/// centre and shifted by (dx, dy).
fn airplane_glyph(k: f32, dx: f32, dy: f32) -> IconShape {
    let t = |x: f32, y: f32| ((x - 12.0) * k + 12.0 + dx, (y - 12.0) * k + 12.0 + dy);
    let pts = [
        t(12.0, 2.5),
        t(13.3, 5.5),
        t(13.3, 8.5),
        t(21.5, 13.0),
        t(21.5, 15.0),
        t(13.3, 12.8),
        t(13.3, 17.0),
        t(16.0, 19.3),
        t(16.0, 20.8),
        t(12.0, 19.6),
        t(8.0, 20.8),
        t(8.0, 19.3),
        t(10.7, 17.0),
        t(10.7, 12.8),
        t(2.5, 15.0),
        t(2.5, 13.0),
        t(10.7, 8.5),
        t(10.7, 5.5),
    ];
    pc(&pts)
}

/// Lapping water under a hull.
fn waves() -> IconShape {
    path(vec![
        L(4.0, 21.7),
        Q(6.0, 20.5, 8.0, 21.7),
        Q(10.0, 22.9, 12.0, 21.7),
        Q(14.0, 20.5, 16.0, 21.7),
        Q(18.0, 22.9, 20.0, 21.7),
    ])
}

/// A ship hull (trapezoid, waterline at y=14).
fn hull() -> IconShape {
    pc(&[(3.0, 14.0), (21.0, 14.0), (18.5, 19.5), (5.5, 19.5)])
}

/// Transportation, Logistics — chunk 7.
#[rustfmt::skip]
fn transport_logistics_shapes(name: &str) -> Option<Vec<IconShape>> {
    Some(match name {
        // ── Transportation ─────────────────────────────────────────────────
        "truck" => truck_body().to_vec(),
        "truck-loading" => {
            let mut v = truck_body().to_vec();
            v.push(p(&[(7.0, 2.0), (7.0, 5.0)]));
            v.push(p(&[(5.2, 3.4), (7.0, 5.3), (8.8, 3.4)]));
            v
        }
        "truck-delivery" => {
            let mut v = truck_body().to_vec();
            v.push(p(&[(4.8, 11.0), (6.8, 13.0), (10.5, 9.3)]));
            v
        }
        "van" => vec![
            path(vec![
                L(2.5, 16.0), L(2.5, 9.5),
                Q(2.5, 7.5, 4.5, 7.5), L(13.0, 7.5),
                Q(16.0, 7.5, 18.0, 9.5), L(20.5, 12.0),
                Q(21.5, 13.0, 21.5, 14.5), L(21.5, 16.0),
            ]),
            p(&[(13.0, 7.5), (13.0, 12.0)]),
            c(7.0, 17.5, 2.2),
            c(16.5, 17.5, 2.2),
        ],
        "ship" => vec![
            hull(),
            rr(14.0, 9.5, 4.5, 4.5, 0.4),
            rr(5.5, 11.0, 7.5, 3.0, 0.3),
            waves(),
        ],
        "ship-cargo" => vec![
            hull(),
            rr(5.0, 11.2, 5.0, 2.8, 0.3),
            rr(10.5, 11.2, 5.0, 2.8, 0.3),
            rr(7.75, 8.2, 5.0, 2.8, 0.3),
            rr(16.5, 8.2, 3.5, 5.8, 0.4),
            waves(),
        ],
        "airplane" => vec![airplane_glyph(1.0, 0.0, 0.0)],
        "airplane-landing" => vec![
            airplane_glyph(0.72, 2.5, -2.0),
            p(&[(3.0, 20.5), (21.0, 20.5)]),
            p(&[(4.5, 12.0), (4.5, 17.0)]),
            p(&[(2.9, 15.5), (4.5, 17.3), (6.1, 15.5)]),
        ],
        "airplane-takeoff" => vec![
            airplane_glyph(0.72, 2.5, -2.0),
            p(&[(3.0, 20.5), (21.0, 20.5)]),
            p(&[(4.5, 17.0), (4.5, 12.0)]),
            p(&[(2.9, 13.5), (4.5, 11.7), (6.1, 13.5)]),
        ],
        "helicopter" => vec![
            ellipse(10.5, 14.0, 5.5, 3.8),
            p(&[(16.0, 14.0), (21.0, 12.5)]),
            p(&[(21.0, 10.5), (21.0, 14.5)]),
            p(&[(3.5, 7.0), (18.5, 7.0)]),
            p(&[(11.0, 10.2), (11.0, 7.0)]),
            p(&[(5.5, 19.5), (16.0, 19.5)]),
            p(&[(8.0, 17.8), (8.0, 19.5)]),
            p(&[(13.0, 17.8), (13.0, 19.5)]),
        ],
        "train" => vec![
            rr(3.0, 6.0, 13.0, 11.0, 1.5),
            path(vec![L(16.0, 6.0), Q(20.5, 6.0, 20.5, 10.5), L(20.5, 17.0), L(16.0, 17.0)]),
            rr(5.5, 8.5, 3.5, 3.5, 0.4),
            rr(11.0, 8.5, 3.5, 3.5, 0.4),
            c(7.0, 19.0, 1.8),
            c(13.0, 19.0, 1.8),
            c(17.5, 19.0, 1.8),
        ],
        "railway" => vec![
            p(&[(8.0, 3.5), (4.0, 20.5)]),
            p(&[(16.0, 3.5), (20.0, 20.5)]),
            p(&[(7.0, 7.0), (17.0, 7.0)]),
            p(&[(6.0, 12.0), (18.0, 12.0)]),
            p(&[(5.0, 17.0), (19.0, 17.0)]),
        ],
        "container" => vec![
            rr(2.5, 7.0, 19.0, 10.0, 1.0),
            p(&[(7.5, 7.0), (7.5, 17.0)]),
            p(&[(12.0, 7.0), (12.0, 17.0)]),
            p(&[(16.5, 7.0), (16.5, 17.0)]),
        ],
        "forklift" => vec![
            rr(3.0, 10.0, 8.0, 6.0, 0.8),
            p(&[(5.0, 10.0), (5.0, 6.5), (10.0, 6.5), (11.0, 10.0)]),
            p(&[(13.0, 4.0), (13.0, 17.5)]),
            p(&[(13.0, 15.0), (19.5, 15.0)]),
            p(&[(13.0, 17.5), (21.0, 17.5)]),
            c(5.5, 18.5, 2.0),
            c(11.0, 18.5, 2.0),
        ],
        "crane" => vec![
            p(&[(6.0, 20.5), (6.0, 4.5)]),
            p(&[(2.5, 6.5), (21.0, 6.5)]),
            p(&[(6.0, 3.5), (2.5, 6.5)]),
            p(&[(6.0, 3.5), (21.0, 6.5)]),
            p(&[(17.5, 6.5), (17.5, 11.0)]),
            a(17.5, 11.8, 1.1, -70.0, 190.0),
            p(&[(3.5, 20.5), (8.5, 20.5)]),
        ],
        "anchor" => vec![
            c(12.0, 4.7, 1.8),
            p(&[(12.0, 6.5), (12.0, 18.0)]),
            p(&[(8.0, 8.8), (16.0, 8.8)]),
            a(12.0, 12.5, 6.5, 20.0, 160.0),
            p(&[(3.7, 13.4), (5.9, 14.7)]),
            p(&[(20.3, 13.4), (18.1, 14.7)]),
        ],
        "compass" => vec![
            c(12.0, 12.0, 8.5),
            pc(&[(15.5, 8.5), (13.5, 13.5), (8.5, 15.5), (10.5, 10.5)]),
            d(12.0, 12.0, 0.9),
        ],
        "route" => vec![
            c(5.0, 18.5, 2.2),
            path(vec![L(7.2, 18.5), Q(14.0, 18.5, 14.0, 12.0), Q(14.0, 6.5, 16.5, 6.0)]),
            pathc(vec![
                L(19.0, 8.8),
                Q(16.8, 6.0, 16.8, 4.8), Q(16.8, 2.6, 19.0, 2.6),
                Q(21.2, 2.6, 21.2, 4.8), Q(21.2, 6.0, 19.0, 8.8),
            ]),
            d(19.0, 4.7, 0.9),
        ],
        "highway" => vec![
            p(&[(9.5, 3.5), (4.0, 20.5)]),
            p(&[(14.5, 3.5), (20.0, 20.5)]),
            p(&[(12.0, 5.0), (12.0, 7.5)]),
            p(&[(12.0, 10.5), (12.0, 13.0)]),
            p(&[(12.0, 16.0), (12.0, 18.5)]),
        ],
        "bridge" => vec![
            p(&[(2.5, 13.0), (21.5, 13.0)]),
            a(12.0, 13.0, 8.0, 180.0, 360.0),
            p(&[(4.0, 13.0), (4.0, 18.5)]),
            p(&[(20.0, 13.0), (20.0, 18.5)]),
            waves(),
        ],
        "toll" => vec![
            rr(3.5, 6.0, 6.0, 12.0, 0.8),
            p(&[(2.8, 6.0), (6.5, 3.5), (10.2, 6.0)]),
            p(&[(9.5, 10.5), (21.5, 10.5)]),
            d(10.0, 10.5, 1.0),
            p(&[(12.5, 9.3), (14.0, 11.7)]),
            p(&[(16.0, 9.3), (17.5, 11.7)]),
        ],
        "fuel-pump" => vec![
            rr(4.0, 5.0, 9.0, 15.0, 1.2),
            rr(6.0, 7.5, 5.0, 4.0, 0.5),
            path(vec![
                L(13.0, 9.5), L(15.5, 9.5),
                Q(17.5, 9.5, 17.5, 11.5), L(17.5, 16.0),
                Q(17.5, 18.0, 19.5, 18.0), Q(21.5, 18.0, 21.5, 16.0),
                L(21.5, 8.5), L(19.5, 6.0),
            ]),
            p(&[(3.0, 20.0), (14.0, 20.0)]),
        ],
        "tire" => vec![
            c(12.0, 12.0, 8.5),
            c(12.0, 12.0, 4.0),
            p(&[(17.5, 12.0), (20.0, 12.0)]),
            p(&[(15.9, 15.9), (17.7, 17.7)]),
            p(&[(12.0, 17.5), (12.0, 20.0)]),
            p(&[(8.1, 15.9), (6.3, 17.7)]),
            p(&[(6.5, 12.0), (4.0, 12.0)]),
            p(&[(8.1, 8.1), (6.3, 6.3)]),
            p(&[(12.0, 6.5), (12.0, 4.0)]),
            p(&[(15.9, 8.1), (17.7, 6.3)]),
        ],
        "engine" => vec![
            rr(5.0, 8.0, 14.0, 9.0, 1.0),
            p(&[(7.0, 8.0), (7.0, 5.5), (11.0, 5.5), (11.0, 8.0)]),
            p(&[(19.0, 10.5), (21.5, 10.5)]),
            p(&[(5.0, 13.0), (2.5, 13.0)]),
            d(8.0, 12.5, 0.8),
            d(12.0, 12.5, 0.8),
            d(16.0, 12.5, 0.8),
        ],
        "speedometer" => vec![
            a(12.0, 14.0, 8.5, 180.0, 360.0),
            p(&[(12.0, 14.0), (16.8, 9.2)]),
            d(12.0, 14.0, 1.3),
            p(&[(5.9, 9.9), (7.0, 10.9)]),
            p(&[(12.0, 5.5), (12.0, 7.0)]),
            p(&[(18.1, 9.9), (17.0, 10.9)]),
        ],
        "odometer" => vec![
            rr(3.5, 8.5, 17.0, 7.0, 1.0),
            p(&[(7.75, 8.5), (7.75, 15.5)]),
            p(&[(12.0, 8.5), (12.0, 15.5)]),
            p(&[(16.25, 8.5), (16.25, 15.5)]),
            p(&[(5.6, 10.5), (5.6, 13.5)]),
            p(&[(9.9, 10.5), (9.9, 13.5)]),
            p(&[(14.1, 10.5), (14.1, 13.5)]),
            p(&[(18.4, 10.5), (18.4, 13.5)]),
        ],
        "box-truck" => vec![
            rr(2.0, 5.5, 12.5, 10.5, 1.0),
            path(vec![
                L(14.5, 8.5), L(18.5, 8.5), L(21.5, 12.0), L(21.5, 16.0), L(14.5, 16.0),
            ]),
            c(7.0, 18.0, 2.2),
            c(17.5, 18.0, 2.2),
        ],
        "tanker-ship" => vec![
            hull(),
            ellipse(10.0, 11.5, 6.0, 2.5),
            rr(17.0, 8.5, 3.0, 5.5, 0.4),
            waves(),
        ],
        "ferry" => vec![
            pc(&[(2.5, 14.5), (21.5, 14.5), (19.0, 19.5), (5.0, 19.5)]),
            rr(6.0, 9.5, 12.0, 5.0, 0.8),
            d(8.5, 12.0, 0.8),
            d(12.0, 12.0, 0.8),
            d(15.5, 12.0, 0.8),
            waves(),
        ],
        "cargo-train" => vec![
            rr(2.5, 9.0, 6.5, 7.0, 0.8),
            rr(10.5, 10.5, 5.0, 5.5, 0.5),
            rr(17.0, 10.5, 4.5, 5.5, 0.5),
            d(4.5, 17.8, 1.4),
            d(7.5, 17.8, 1.4),
            d(13.0, 17.8, 1.4),
            d(19.0, 17.8, 1.4),
            p(&[(2.0, 20.5), (22.0, 20.5)]),
        ],
        "metro" => vec![
            rr(5.0, 3.5, 14.0, 15.0, 2.5),
            rr(7.5, 6.0, 9.0, 5.0, 1.0),
            d(8.0, 15.5, 1.1),
            d(16.0, 15.5, 1.1),
            p(&[(4.0, 21.0), (20.0, 21.0)]),
        ],
        "tram" => vec![
            p(&[(4.0, 3.0), (20.0, 3.0)]),
            p(&[(9.0, 6.0), (12.0, 3.0), (15.0, 6.0)]),
            rr(5.5, 6.0, 13.0, 12.5, 2.0),
            rr(8.0, 8.5, 8.0, 4.0, 0.8),
            d(8.5, 16.0, 1.0),
            d(15.5, 16.0, 1.0),
        ],
        "harbor" => vec![
            pc(&[(9.5, 20.0), (10.5, 6.5), (13.5, 6.5), (14.5, 20.0)]),
            rr(10.0, 4.0, 4.0, 2.5, 0.5),
            p(&[(8.0, 4.5), (5.0, 3.0)]),
            p(&[(16.0, 4.5), (19.0, 3.0)]),
            p(&[(10.1, 11.0), (13.9, 11.0)]),
            p(&[(9.8, 15.0), (14.2, 15.0)]),
            p(&[(7.0, 20.0), (17.0, 20.0)]),
        ],
        "runway" => vec![
            p(&[(6.0, 3.5), (2.5, 20.5)]),
            p(&[(18.0, 3.5), (21.5, 20.5)]),
            p(&[(12.0, 5.0), (12.0, 8.0)]),
            p(&[(12.0, 11.0), (12.0, 14.0)]),
            p(&[(12.0, 17.0), (12.0, 20.0)]),
            p(&[(7.2, 7.0), (9.0, 7.0)]),
            p(&[(15.0, 7.0), (16.8, 7.0)]),
        ],
        "traffic-cone" => vec![
            pc(&[(8.5, 18.0), (11.0, 4.5), (13.0, 4.5), (15.5, 18.0)]),
            rr(6.0, 18.0, 12.0, 2.5, 0.8),
            p(&[(9.9, 9.5), (14.1, 9.5)]),
            p(&[(9.2, 13.5), (14.8, 13.5)]),
        ],

        // ── Logistics ──────────────────────────────────────────────────────
        "package" => {
            let mut v = box3d().to_vec();
            v.push(p(&[(7.75, 6.0), (16.25, 10.0)]));
            v
        }
        "package-open" => vec![
            pc(&[(3.5, 8.0), (12.0, 12.0), (20.5, 8.0), (12.0, 4.0)]),
            p(&[(3.5, 8.0), (3.5, 16.0), (12.0, 20.5), (12.0, 12.0)]),
            p(&[(20.5, 8.0), (20.5, 16.0), (12.0, 20.5)]),
            p(&[(3.5, 8.0), (1.0, 5.0), (7.5, 2.8), (12.0, 4.0)]),
            p(&[(20.5, 8.0), (23.0, 5.0), (16.5, 2.8), (12.0, 4.0)]),
        ],
        "package-check" => {
            let mut v = box3d().to_vec();
            v.push(p(&[(14.0, 14.0), (15.8, 15.8), (19.0, 12.3)]));
            v
        }
        "package-x" => {
            let mut v = box3d().to_vec();
            v.push(p(&[(14.5, 13.0), (18.5, 17.0)]));
            v.push(p(&[(18.5, 13.0), (14.5, 17.0)]));
            v
        }
        "package-search" => {
            let mut v = box3d().to_vec();
            v.push(c(18.5, 4.5, 2.8));
            v.push(p(&[(20.6, 6.6), (22.8, 8.8)]));
            v
        }
        "conveyor" => vec![
            rr(2.5, 10.0, 19.0, 4.5, 2.25),
            d(6.0, 12.25, 1.3),
            d(12.0, 12.25, 1.3),
            d(18.0, 12.25, 1.3),
            rr(8.5, 4.0, 7.0, 6.0, 0.6),
        ],
        "loading-dock" => vec![
            pc(&[(2.5, 13.0), (10.0, 13.0), (10.0, 19.5), (2.5, 19.5)]),
            rr(12.5, 7.5, 9.0, 9.0, 0.8),
            p(&[(15.5, 7.5), (15.5, 16.5)]),
            p(&[(18.5, 7.5), (18.5, 16.5)]),
            d(14.5, 18.3, 1.5),
            d(19.0, 18.3, 1.5),
        ],
        "dispatch" => vec![
            rr(3.0, 11.0, 8.0, 8.0, 0.8),
            p(&[(13.0, 15.0), (21.0, 7.0)]),
            p(&[(16.8, 7.0), (21.0, 7.0), (21.0, 11.2)]),
        ],
        "tracking" => vec![
            pathc(vec![
                L(12.0, 14.5),
                Q(8.5, 10.0, 8.5, 7.9), Q(8.5, 4.5, 12.0, 4.5),
                Q(15.5, 4.5, 15.5, 7.9), Q(15.5, 10.0, 12.0, 14.5),
            ]),
            d(12.0, 7.9, 1.2),
            p(&[(12.0, 16.5), (12.0, 18.0)]),
            p(&[(12.0, 19.5), (12.0, 21.0)]),
        ],
        "tracking-number" => vec![
            p(&[(4.0, 5.0), (4.0, 11.0)]),
            p(&[(6.5, 5.0), (6.5, 11.0)]),
            p(&[(9.0, 5.0), (9.0, 11.0)]),
            p(&[(11.5, 5.0), (11.5, 11.0)]),
            p(&[(14.0, 5.0), (14.0, 11.0)]),
            c(15.5, 15.5, 4.0),
            p(&[(18.4, 18.4), (21.0, 21.0)]),
        ],
        "delivery-time" => vec![
            rr(2.5, 9.5, 10.0, 7.0, 0.8),
            path(vec![
                L(12.5, 11.5), L(15.5, 11.5), L(17.5, 13.5), L(17.5, 16.5), L(12.5, 16.5),
            ]),
            c(6.0, 18.3, 1.8),
            c(14.5, 18.3, 1.8),
            c(18.5, 6.0, 3.5),
            p(&[(18.5, 4.2), (18.5, 6.0), (20.3, 7.0)]),
        ],
        "express" => vec![
            rr(6.0, 8.0, 10.5, 7.5, 0.8),
            path(vec![
                L(16.5, 10.0), L(19.0, 10.0), L(21.5, 12.5), L(21.5, 15.5), L(16.5, 15.5),
            ]),
            c(9.5, 17.5, 2.0),
            c(19.0, 17.5, 2.0),
            p(&[(1.5, 9.5), (4.5, 9.5)]),
            p(&[(0.8, 12.0), (3.8, 12.0)]),
            p(&[(1.5, 14.5), (4.5, 14.5)]),
        ],
        "fragile" => vec![
            path(vec![
                L(7.5, 3.5),
                Q(7.5, 10.5, 12.0, 10.5), Q(16.5, 10.5, 16.5, 3.5),
            ]),
            p(&[(7.5, 3.5), (16.5, 3.5)]),
            p(&[(12.0, 10.5), (12.0, 17.5)]),
            p(&[(8.5, 20.0), (15.5, 20.0)]),
        ],
        "hazmat" => vec![
            pc(&[(12.0, 2.5), (21.5, 12.0), (12.0, 21.5), (2.5, 12.0)]),
            p(&[(12.0, 8.0), (12.0, 13.5)]),
            d(12.0, 16.5, 1.1),
        ],
        "temperature" => vec![
            c(12.0, 17.5, 3.0),
            path(vec![
                L(10.3, 15.1), L(10.3, 5.5),
                Q(10.3, 3.5, 12.0, 3.5), Q(13.7, 3.5, 13.7, 5.5), L(13.7, 15.1),
            ]),
            p(&[(12.0, 15.5), (12.0, 8.5)]),
            p(&[(15.5, 6.5), (17.5, 6.5)]),
            p(&[(15.5, 9.5), (17.5, 9.5)]),
            p(&[(15.5, 12.5), (17.5, 12.5)]),
        ],
        "weight-scale" => vec![
            rr(4.0, 9.0, 16.0, 11.0, 1.5),
            a(12.0, 15.5, 3.5, 180.0, 360.0),
            p(&[(12.0, 15.5), (14.2, 13.3)]),
            p(&[(6.0, 9.0), (6.0, 6.5), (18.0, 6.5), (18.0, 9.0)]),
        ],
        "dimensions" => vec![
            rr(6.5, 6.5, 11.0, 11.0, 0.8),
            p(&[(6.5, 20.0), (17.5, 20.0)]),
            p(&[(8.3, 18.4), (6.5, 20.0), (8.3, 21.6)]),
            p(&[(15.7, 18.4), (17.5, 20.0), (15.7, 21.6)]),
            p(&[(20.5, 6.5), (20.5, 17.5)]),
            p(&[(18.9, 8.3), (20.5, 6.5), (22.1, 8.3)]),
            p(&[(18.9, 15.7), (20.5, 17.5), (22.1, 15.7)]),
        ],
        "customs" => vec![
            pc(&[
                (8.5, 3.5), (15.5, 3.5), (20.5, 8.5), (20.5, 15.5),
                (15.5, 20.5), (8.5, 20.5), (3.5, 15.5), (3.5, 8.5),
            ]),
            p(&[(8.0, 12.5), (10.8, 15.3), (16.0, 9.5)]),
        ],
        "manifest" => vec![
            rr(5.5, 4.5, 13.0, 17.0, 1.5),
            rr(9.5, 2.8, 5.0, 3.4, 1.0),
            p(&[(8.5, 10.0), (15.5, 10.0)]),
            p(&[(8.5, 13.0), (15.5, 13.0)]),
            p(&[(8.5, 16.0), (12.5, 16.0)]),
        ],
        "bill-of-lading" => {
            let mut v = page().to_vec();
            v.push(p(&[(9.5, 9.5), (14.5, 9.5)]));
            v.push(pc(&[(8.5, 15.0), (15.5, 15.0), (14.2, 17.8), (9.8, 17.8)]));
            v.push(p(&[(12.0, 11.5), (12.0, 15.0)]));
            v
        }
        "cross-dock" => vec![
            rr(8.5, 6.5, 7.0, 11.0, 0.8),
            p(&[(2.5, 10.5), (19.0, 10.5)]),
            p(&[(16.8, 8.5), (19.3, 10.5), (16.8, 12.5)]),
            p(&[(21.5, 13.5), (5.0, 13.5)]),
            p(&[(7.2, 11.5), (4.7, 13.5), (7.2, 15.5)]),
        ],
        "last-mile" => vec![
            p(&[(5.0, 12.0), (11.0, 7.0), (17.0, 12.0)]),
            p(&[(6.5, 11.0), (6.5, 18.5), (15.5, 18.5), (15.5, 11.0)]),
            pathc(vec![
                L(19.0, 10.5),
                Q(16.8, 7.8, 16.8, 6.4), Q(16.8, 4.0, 19.0, 4.0),
                Q(21.2, 4.0, 21.2, 6.4), Q(21.2, 7.8, 19.0, 10.5),
            ]),
            d(19.0, 6.3, 0.8),
        ],
        "return-shipment" => vec![
            rr(6.5, 10.0, 11.0, 8.5, 0.8),
            path(vec![
                L(17.5, 10.0), L(17.5, 6.5),
                Q(17.5, 4.0, 15.0, 4.0), L(8.0, 4.0),
            ]),
            p(&[(10.2, 1.8), (7.6, 4.0), (10.2, 6.2)]),
        ],
        "consolidation" => vec![
            rr(9.0, 9.5, 6.0, 5.5, 0.6),
            p(&[(2.5, 12.0), (7.5, 12.0)]),
            p(&[(5.7, 10.2), (7.7, 12.0), (5.7, 13.8)]),
            p(&[(12.0, 2.5), (12.0, 7.5)]),
            p(&[(10.2, 5.7), (12.0, 7.7), (13.8, 5.7)]),
            p(&[(21.5, 12.0), (16.5, 12.0)]),
            p(&[(18.3, 10.2), (16.3, 12.0), (18.3, 13.8)]),
        ],
        "deconsolidation" => vec![
            rr(9.0, 9.5, 6.0, 5.5, 0.6),
            p(&[(7.5, 12.0), (2.5, 12.0)]),
            p(&[(4.3, 10.2), (2.3, 12.0), (4.3, 13.8)]),
            p(&[(12.0, 7.5), (12.0, 2.5)]),
            p(&[(10.2, 4.3), (12.0, 2.3), (13.8, 4.3)]),
            p(&[(16.5, 12.0), (21.5, 12.0)]),
            p(&[(19.7, 10.2), (21.7, 12.0), (19.7, 13.8)]),
        ],
        "inbound" => vec![
            p(&[(4.0, 10.0), (12.0, 5.0), (20.0, 10.0)]),
            p(&[(5.5, 9.0), (5.5, 19.5)]),
            p(&[(18.5, 9.0), (18.5, 19.5)]),
            p(&[(5.5, 19.5), (18.5, 19.5)]),
            p(&[(12.0, 10.5), (12.0, 16.0)]),
            p(&[(9.8, 13.8), (12.0, 16.2), (14.2, 13.8)]),
        ],
        "outbound" => vec![
            p(&[(4.0, 10.0), (12.0, 5.0), (20.0, 10.0)]),
            p(&[(5.5, 9.0), (5.5, 19.5)]),
            p(&[(18.5, 9.0), (18.5, 19.5)]),
            p(&[(5.5, 19.5), (18.5, 19.5)]),
            p(&[(12.0, 16.0), (12.0, 10.5)]),
            p(&[(9.8, 12.7), (12.0, 10.3), (14.2, 12.7)]),
        ],
        "route-planning" => vec![
            rr(3.5, 4.5, 17.0, 15.0, 1.5),
            p(&[(6.0, 17.0), (9.0, 9.0), (15.0, 15.0), (18.0, 7.0)]),
            d(6.0, 17.0, 1.2),
            d(18.0, 7.0, 1.2),
        ],
        "proof-of-delivery" => {
            let mut v = page().to_vec();
            v.push(p(&[(9.5, 10.5), (11.0, 12.0), (14.0, 8.7)]));
            v.push(path(vec![
                L(8.5, 16.0),
                Q(10.0, 14.0, 11.0, 15.7), Q(12.0, 17.4, 13.0, 15.7), L(15.5, 15.7),
            ]));
            v
        }
        "freight" => vec![
            rr(2.0, 6.5, 13.0, 8.0, 0.5),
            p(&[(5.5, 6.5), (5.5, 14.5)]),
            p(&[(9.0, 6.5), (9.0, 14.5)]),
            path(vec![
                L(15.0, 9.5), L(18.5, 9.5), L(21.5, 13.0), L(21.5, 14.5), L(15.0, 14.5),
            ]),
            c(6.5, 17.0, 2.2),
            c(18.0, 17.0, 2.2),
        ],
        "pallet-jack" => vec![
            p(&[(6.0, 3.5), (8.5, 9.0)]),
            rr(6.5, 9.0, 4.5, 6.5, 0.8),
            p(&[(8.0, 15.5), (21.0, 15.5)]),
            p(&[(8.0, 18.0), (21.0, 18.0)]),
            d(10.0, 19.5, 1.4),
            d(19.0, 19.5, 1.4),
        ],
        "cold-chain" => vec![
            rr(3.0, 12.0, 8.5, 8.5, 0.8),
            p(&[(16.5, 2.5), (16.5, 11.5)]),
            p(&[(12.6, 4.75), (20.4, 9.25)]),
            p(&[(12.6, 9.25), (20.4, 4.75)]),
        ],
        "drop-shipping" => {
            let mut v = person(5.5, 17.0, 0.7).to_vec();
            add(&mut v, person(18.5, 17.0, 0.7));
            v.push(p(&[(8.5, 7.0), (15.5, 7.0)]));
            v.push(p(&[(13.7, 5.4), (15.7, 7.0), (13.7, 8.6)]));
            v.push(rr(9.5, 10.5, 5.0, 4.5, 0.5));
            v
        }
        "reverse-logistics" => vec![
            rr(6.5, 4.0, 11.0, 7.0, 0.8),
            path(vec![
                L(19.0, 9.0), L(19.0, 14.0),
                Q(19.0, 17.5, 15.5, 17.5), L(6.5, 17.5),
            ]),
            p(&[(8.7, 15.3), (6.1, 17.5), (8.7, 19.7)]),
        ],
        "supply-chain" => vec![
            c(4.8, 12.0, 2.3),
            c(12.0, 12.0, 2.3),
            c(19.2, 12.0, 2.3),
            p(&[(7.1, 12.0), (9.7, 12.0)]),
            p(&[(14.3, 12.0), (16.9, 12.0)]),
        ],

        _ => return None,
    })
}

/// Financial, Social Media — chunk 8.
#[rustfmt::skip]
fn financial_social_shapes(name: &str) -> Option<Vec<IconShape>> {
    Some(match name {
        // ── Financial ──────────────────────────────────────────────────────
        "dollar" => {
            let mut v = vec![badge()];
            add(&mut v, dollar_glyph(12.0, 12.0, 0.82));
            v
        }
        "euro" => vec![
            badge(),
            a(13.0, 12.0, 4.8, 50.0, 310.0),
            p(&[(6.8, 10.5), (13.0, 10.5)]),
            p(&[(6.8, 13.5), (12.4, 13.5)]),
        ],
        "yen" => vec![
            badge(),
            p(&[(8.5, 6.5), (12.0, 11.5)]),
            p(&[(15.5, 6.5), (12.0, 11.5)]),
            p(&[(12.0, 11.5), (12.0, 17.5)]),
            p(&[(9.0, 13.0), (15.0, 13.0)]),
            p(&[(9.0, 15.5), (15.0, 15.5)]),
        ],
        "pound" => vec![
            badge(),
            path(vec![
                L(14.8, 8.2),
                Q(14.4, 6.4, 12.3, 6.4), Q(10.0, 6.4, 10.0, 9.0), L(10.0, 17.4),
            ]),
            p(&[(8.2, 12.0), (13.8, 12.0)]),
            p(&[(8.2, 17.4), (15.8, 17.4)]),
        ],
        "bitcoin" => vec![
            badge(),
            p(&[(9.7, 7.7), (9.7, 16.3)]),
            path(vec![
                L(9.7, 7.7), L(13.0, 7.7),
                Q(15.0, 7.7, 15.0, 9.7), Q(15.0, 11.7, 13.0, 11.7), L(9.7, 11.7),
            ]),
            path(vec![
                L(9.7, 11.7), L(13.4, 11.7),
                Q(15.5, 11.7, 15.5, 13.9), Q(15.5, 16.3, 13.4, 16.3), L(9.7, 16.3),
            ]),
            p(&[(11.5, 6.3), (11.5, 7.7)]),
            p(&[(11.5, 16.3), (11.5, 17.7)]),
        ],
        "coins" => vec![
            c(9.0, 10.0, 5.5),
            p(&[(9.0, 7.9), (9.0, 12.1)]),
            a(15.0, 14.0, 5.5, -55.0, 180.0),
        ],
        "money-bag" => {
            let mut v = vec![
                p(&[(9.5, 6.5), (8.0, 3.5)]),
                p(&[(14.5, 6.5), (16.0, 3.5)]),
                p(&[(9.0, 6.5), (15.0, 6.5)]),
                path(vec![
                    L(9.5, 6.5),
                    Q(4.0, 10.5, 4.0, 15.0), Q(4.0, 20.5, 12.0, 20.5),
                    Q(20.0, 20.5, 20.0, 15.0), Q(20.0, 10.5, 14.5, 6.5),
                ]),
            ];
            add(&mut v, dollar_glyph(12.0, 14.0, 0.4));
            v
        }
        "piggy-bank" => vec![
            ellipse(11.0, 13.0, 7.5, 5.5),
            p(&[(14.0, 8.2), (15.5, 6.0), (16.8, 8.6)]),
            p(&[(7.0, 18.0), (7.0, 20.0)]),
            p(&[(15.0, 18.0), (15.0, 20.0)]),
            p(&[(9.5, 8.0), (13.0, 8.0)]),
            d(15.8, 11.5, 0.8),
            c(19.3, 13.0, 1.6),
        ],
        "vault" => vec![
            rr(3.5, 3.5, 17.0, 17.0, 1.5),
            c(12.0, 12.0, 5.5),
            p(&[(12.0, 8.5), (12.0, 15.5)]),
            p(&[(8.5, 12.0), (15.5, 12.0)]),
            d(6.0, 6.0, 0.8),
            d(18.0, 6.0, 0.8),
            d(6.0, 18.0, 0.8),
            d(18.0, 18.0, 0.8),
        ],
        "safe" => vec![
            rr(4.0, 4.5, 16.0, 15.0, 1.5),
            c(10.0, 12.0, 3.0),
            p(&[(10.0, 9.6), (10.0, 12.0)]),
            p(&[(16.5, 9.5), (16.5, 14.5)]),
            p(&[(6.0, 19.5), (6.0, 21.0)]),
            p(&[(18.0, 19.5), (18.0, 21.0)]),
        ],
        "bank" => bank_body().to_vec(),
        "atm" => vec![
            rr(5.0, 3.5, 14.0, 17.0, 1.5),
            rr(7.5, 6.0, 9.0, 5.0, 0.5),
            p(&[(7.5, 13.5), (16.5, 13.5)]),
            d(9.0, 16.5, 0.8),
            d(12.0, 16.5, 0.8),
            d(15.0, 16.5, 0.8),
            p(&[(9.0, 20.5), (15.0, 20.5)]),
        ],
        "exchange-rate" => {
            let mut v = dollar_glyph(6.5, 7.5, 0.42).to_vec();
            v.push(a(17.8, 16.5, 3.0, 50.0, 310.0));
            v.push(p(&[(13.9, 15.4), (17.8, 15.4)]));
            v.push(p(&[(13.9, 17.6), (17.4, 17.6)]));
            v.push(p(&[(13.5, 6.0), (20.5, 6.0)]));
            v.push(p(&[(18.3, 4.2), (20.7, 6.0), (18.3, 7.8)]));
            v.push(p(&[(10.5, 18.0), (3.5, 18.0)]));
            v.push(p(&[(5.7, 16.2), (3.3, 18.0), (5.7, 19.8)]));
            v
        }
        "stock-market" => vec![
            rr(4.5, 9.0, 3.0, 6.0, 0.3),
            p(&[(6.0, 5.5), (6.0, 9.0)]),
            p(&[(6.0, 15.0), (6.0, 18.5)]),
            rr(10.5, 6.0, 3.0, 7.0, 0.3),
            p(&[(12.0, 3.5), (12.0, 6.0)]),
            p(&[(12.0, 13.0), (12.0, 16.0)]),
            rr(16.5, 11.0, 3.0, 6.0, 0.3),
            p(&[(18.0, 8.0), (18.0, 11.0)]),
            p(&[(18.0, 17.0), (18.0, 20.5)]),
        ],
        "bull-market" => vec![
            p(&[(3.5, 18.5), (9.0, 13.0), (12.5, 15.5), (20.5, 7.5)]),
            p(&[(16.5, 7.5), (20.5, 7.5), (20.5, 11.5)]),
            p(&[(3.5, 20.5), (20.5, 20.5)]),
        ],
        "bear-market" => vec![
            p(&[(3.5, 7.5), (9.0, 12.5), (12.5, 10.0), (20.5, 17.0)]),
            p(&[(16.5, 17.0), (20.5, 17.0), (20.5, 13.0)]),
            p(&[(3.5, 20.5), (20.5, 20.5)]),
        ],
        "dividend" => vec![
            c(12.0, 9.0, 4.5),
            p(&[(12.0, 7.0), (12.0, 11.0)]),
            p(&[(7.0, 14.5), (7.0, 19.0)]),
            p(&[(5.3, 17.4), (7.0, 19.2), (8.7, 17.4)]),
            p(&[(17.0, 14.5), (17.0, 19.0)]),
            p(&[(15.3, 17.4), (17.0, 19.2), (18.7, 17.4)]),
        ],
        "interest-rate" => vec![
            p(&[(5.0, 10.5), (12.5, 3.0)]),
            c(6.5, 4.8, 1.8),
            c(11.0, 8.7, 1.8),
            path(vec![L(4.0, 20.5), Q(12.0, 20.5, 16.0, 16.0), Q(19.5, 12.0, 19.5, 7.5)]),
            p(&[(17.7, 9.3), (19.5, 7.3), (21.3, 9.3)]),
        ],
        "mortgage" => {
            let mut v = vec![
                p(&[(4.0, 11.5), (12.0, 4.5), (20.0, 11.5)]),
                p(&[(6.0, 10.0), (6.0, 19.5), (18.0, 19.5), (18.0, 10.0)]),
            ];
            add(&mut v, dollar_glyph(12.0, 14.8, 0.36));
            v
        }
        "loan" => vec![
            path(vec![L(4.0, 19.0), L(16.0, 19.0), Q(19.0, 19.0, 20.0, 17.0)]),
            p(&[(4.0, 19.0), (4.0, 16.0), (6.5, 14.0)]),
            rr(6.5, 4.5, 11.0, 7.0, 0.8),
            c(12.0, 8.0, 1.8),
        ],
        "audit" => {
            let mut v = page().to_vec();
            v.push(c(11.5, 13.0, 3.0));
            v.push(p(&[(10.3, 13.0), (11.2, 14.0), (13.0, 11.8)]));
            v.push(p(&[(13.7, 15.2), (16.3, 17.8)]));
            v
        }
        "ledger" => vec![
            rr(4.5, 3.5, 15.0, 17.0, 1.5),
            p(&[(8.0, 3.5), (8.0, 20.5)]),
            p(&[(10.5, 8.0), (16.5, 8.0)]),
            p(&[(10.5, 11.5), (16.5, 11.5)]),
            p(&[(10.5, 15.0), (14.5, 15.0)]),
        ],
        "balance-sheet" => vec![
            p(&[(12.0, 4.5), (12.0, 17.5)]),
            p(&[(4.0, 7.0), (20.0, 7.0)]),
            p(&[(4.0, 7.0), (2.5, 11.5)]),
            p(&[(4.0, 7.0), (5.5, 11.5)]),
            a(4.0, 11.2, 1.6, 0.0, 180.0),
            p(&[(20.0, 7.0), (18.5, 11.5)]),
            p(&[(20.0, 7.0), (21.5, 11.5)]),
            a(20.0, 11.2, 1.6, 0.0, 180.0),
            pc(&[(9.0, 20.5), (15.0, 20.5), (12.0, 17.5)]),
        ],
        "profit-loss" => vec![
            p(&[(3.5, 12.0), (20.5, 12.0)]),
            rr(5.5, 6.0, 3.0, 6.0, 0.3),
            rr(10.0, 8.0, 3.0, 4.0, 0.3),
            rr(15.0, 12.0, 3.0, 4.5, 0.3),
        ],
        "cash-flow" => {
            let mut v = dollar_glyph(7.0, 8.0, 0.5).to_vec();
            v.push(path(vec![L(13.0, 5.5), Q(19.0, 5.5, 19.0, 11.5), L(19.0, 15.0)]));
            v.push(p(&[(16.8, 13.2), (19.0, 15.4), (21.2, 13.2)]));
            v.push(d(12.0, 19.5, 1.1));
            v.push(d(15.5, 19.5, 1.1));
            v
        }
        "rupee" => vec![
            badge(),
            p(&[(8.5, 6.5), (15.5, 6.5)]),
            p(&[(8.5, 9.5), (15.5, 9.5)]),
            path(vec![
                L(10.3, 6.5),
                Q(13.8, 7.5, 13.8, 10.0), Q(13.8, 12.5, 10.3, 12.5), L(9.0, 12.5),
            ]),
            p(&[(9.0, 12.5), (14.5, 17.5)]),
        ],
        "won" => vec![
            badge(),
            p(&[(7.5, 7.0), (9.2, 17.0), (12.0, 9.5), (14.8, 17.0), (16.5, 7.0)]),
            p(&[(6.5, 10.5), (17.5, 10.5)]),
            p(&[(6.5, 13.0), (17.5, 13.0)]),
        ],
        "lira" => vec![
            badge(),
            path(vec![
                L(11.0, 6.5), L(11.0, 15.0),
                Q(11.0, 17.5, 13.5, 17.5), Q(16.0, 17.5, 16.0, 15.0),
            ]),
            p(&[(8.5, 11.5), (13.5, 8.5)]),
            p(&[(8.5, 14.0), (13.5, 11.0)]),
        ],
        "treasury-bond" => vec![
            rr(3.5, 5.0, 17.0, 14.0, 1.0),
            rr(5.0, 6.5, 14.0, 11.0, 0.5),
            p(&[(7.0, 9.0), (17.0, 9.0)]),
            c(12.0, 12.5, 2.2),
            p(&[(11.0, 14.3), (10.0, 17.0)]),
            p(&[(13.0, 14.3), (14.0, 17.0)]),
        ],
        "portfolio" => vec![
            rr(3.5, 8.0, 17.0, 11.5, 1.5),
            p(&[(9.5, 8.0), (9.5, 5.5), (14.5, 5.5), (14.5, 8.0)]),
            p(&[(6.5, 16.0), (10.0, 12.5), (13.0, 14.5), (17.5, 11.0)]),
        ],
        "capital" => vec![
            p(&[(6.0, 20.5), (18.0, 20.5)]),
            p(&[(7.5, 18.0), (16.5, 18.0)]),
            p(&[(9.0, 18.0), (9.0, 8.5)]),
            p(&[(15.0, 18.0), (15.0, 8.5)]),
            p(&[(7.5, 8.5), (16.5, 8.5)]),
            rr(6.5, 5.5, 11.0, 3.0, 0.5),
        ],
        "depreciation" => vec![
            c(5.5, 7.5, 2.2),
            p(&[(5.5, 6.5), (5.5, 8.5)]),
            path(vec![L(3.5, 6.0), Q(10.0, 7.0, 14.0, 11.0), Q(18.0, 15.0, 20.5, 19.0)]),
            p(&[(17.4, 18.6), (20.6, 19.1), (20.1, 15.9)]),
        ],
        "amortization" => vec![
            rr(4.0, 6.0, 3.0, 14.0, 0.3),
            rr(8.5, 9.0, 3.0, 11.0, 0.3),
            rr(13.0, 12.0, 3.0, 8.0, 0.3),
            rr(17.5, 15.0, 3.0, 5.0, 0.3),
        ],
        "equity" => vec![
            a(12.0, 12.0, 8.0, -20.0, 260.0),
            p(&[(12.0, 12.0), (12.0, 4.0)]),
            p(&[(12.0, 12.0), (19.5, 9.3)]),
            pc(&[(14.0, 10.0), (16.5, 3.6), (20.6, 8.0)]),
        ],
        "liability" => vec![
            pc(&[(8.0, 8.0), (16.0, 8.0), (18.5, 19.5), (5.5, 19.5)]),
            a(12.0, 8.0, 4.0, 180.0, 360.0),
        ],
        "asset" => {
            let mut v = vec![rr(5.5, 5.5, 13.0, 13.0, 1.5)];
            add(&mut v, dollar_glyph(12.0, 12.0, 0.45));
            v
        }
        "budget" => vec![
            rr(5.5, 3.5, 13.0, 17.0, 1.5),
            rr(7.5, 5.5, 9.0, 3.5, 0.4),
            d(9.0, 12.0, 1.0),
            d(12.0, 12.0, 1.0),
            d(15.0, 12.0, 1.0),
            d(9.0, 15.0, 1.0),
            d(12.0, 15.0, 1.0),
            d(15.0, 15.0, 1.0),
            d(9.0, 18.0, 1.0),
            d(12.0, 18.0, 1.0),
            d(15.0, 18.0, 1.0),
        ],

        // ── Social Media ───────────────────────────────────────────────────
        "like" => vec![
            pathc(vec![
                L(11.0, 20.0),
                Q(4.0, 14.8, 3.5, 10.2), Q(3.5, 5.6, 7.6, 5.6),
                Q(10.0, 5.6, 11.0, 8.2), Q(12.0, 5.6, 14.4, 5.6),
                Q(18.5, 5.6, 18.5, 10.2), Q(18.0, 14.8, 11.0, 20.0),
            ]),
            p(&[(20.5, 2.5), (20.5, 6.5)]),
            p(&[(18.5, 4.5), (22.5, 4.5)]),
        ],
        "dislike" => vec![
            pathc(vec![
                L(12.0, 20.0),
                Q(4.0, 14.5, 3.5, 9.5), Q(3.5, 4.5, 8.0, 4.5),
                Q(10.8, 4.5, 12.0, 7.5), Q(13.2, 4.5, 16.0, 4.5),
                Q(20.5, 4.5, 20.5, 9.5), Q(20.0, 14.5, 12.0, 20.0),
            ]),
            p(&[(12.0, 7.5), (10.8, 10.5), (13.2, 12.5), (11.6, 16.0)]),
        ],
        "comment" => vec![
            chat_bubble(),
            p(&[(7.0, 9.0), (17.0, 9.0)]),
            p(&[(7.0, 12.0), (14.0, 12.0)]),
        ],
        "repost" => vec![
            p(&[(4.5, 15.0), (4.5, 8.5), (15.0, 8.5)]),
            p(&[(12.8, 6.3), (15.2, 8.5), (12.8, 10.7)]),
            p(&[(19.5, 9.0), (19.5, 15.5), (9.0, 15.5)]),
            p(&[(11.2, 13.3), (8.8, 15.5), (11.2, 17.7)]),
        ],
        "mention" => {
            let mut v = person(9.0, 20.0, 0.9).to_vec();
            v.push(c(18.0, 9.0, 1.8));
            v.push(a(18.0, 9.0, 3.8, 20.0, 340.0));
            v.push(path(vec![L(19.8, 9.0), L(19.8, 10.2), Q(19.8, 11.2, 21.0, 11.2)]));
            v
        }
        "hashtag" => vec![
            p(&[(9.5, 4.5), (7.5, 19.5)]),
            p(&[(16.5, 4.5), (14.5, 19.5)]),
            p(&[(5.5, 9.0), (19.5, 9.0)]),
            p(&[(4.5, 15.0), (18.5, 15.0)]),
        ],
        "trending" => vec![
            p(&[(3.0, 17.5), (9.0, 11.5), (13.0, 14.5), (21.0, 6.5)]),
            p(&[(16.9, 6.5), (21.0, 6.5), (21.0, 10.6)]),
        ],
        "viral" => vec![
            d(12.0, 12.0, 1.6),
            p(&[(12.0, 8.5), (12.0, 4.5)]),
            p(&[(12.0, 15.5), (12.0, 19.5)]),
            p(&[(8.5, 12.0), (4.5, 12.0)]),
            p(&[(15.5, 12.0), (19.5, 12.0)]),
            p(&[(9.5, 9.5), (6.7, 6.7)]),
            p(&[(14.5, 14.5), (17.3, 17.3)]),
            p(&[(14.5, 9.5), (17.3, 6.7)]),
            p(&[(9.5, 14.5), (6.7, 17.3)]),
        ],
        "follower" => {
            let mut v = person(14.5, 20.0, 0.95).to_vec();
            v.push(p(&[(2.5, 11.0), (8.0, 11.0)]));
            v.push(p(&[(5.8, 8.8), (8.2, 11.0), (5.8, 13.2)]));
            v
        }
        "following" => {
            let mut v = person(10.0, 20.0, 0.95).to_vec();
            v.push(c(18.5, 11.0, 3.6));
            v.push(p(&[(16.9, 11.0), (18.2, 12.3), (20.3, 9.9)]));
            v
        }
        "profile" => vec![
            rr(3.0, 5.5, 18.0, 13.0, 1.5),
            c(8.0, 10.5, 2.0),
            path(vec![L(5.0, 15.8), Q(5.0, 13.3, 8.0, 13.3), Q(11.0, 13.3, 11.0, 15.8)]),
            p(&[(13.5, 9.5), (19.0, 9.5)]),
            p(&[(13.5, 12.5), (19.0, 12.5)]),
            p(&[(13.5, 15.5), (17.0, 15.5)]),
        ],
        "bio" => {
            let mut v = page().to_vec();
            v.push(c(12.0, 11.0, 2.2));
            v.push(path(vec![L(9.0, 16.5), Q(9.0, 13.8, 12.0, 13.8), Q(15.0, 13.8, 15.0, 16.5)]));
            v.push(p(&[(9.5, 19.0), (14.5, 19.0)]));
            v
        }
        "story" => vec![
            a(12.0, 12.0, 8.5, -80.0, 70.0),
            a(12.0, 12.0, 8.5, 100.0, 250.0),
            c(12.0, 12.0, 5.0),
        ],
        "reel" => vec![
            rr(3.5, 3.5, 17.0, 17.0, 2.0),
            p(&[(3.5, 8.0), (20.5, 8.0)]),
            p(&[(7.0, 3.5), (9.5, 8.0)]),
            p(&[(12.5, 3.5), (15.0, 8.0)]),
            pc(&[(10.0, 11.0), (15.5, 14.0), (10.0, 17.0)]),
        ],
        "live-stream" => vec![
            rr(3.0, 8.0, 11.0, 9.0, 1.5),
            pc(&[(14.0, 10.5), (18.0, 8.0), (18.0, 17.0), (14.0, 14.5)]),
            a(20.0, 12.5, 2.0, -60.0, 60.0),
            a(20.0, 12.5, 4.0, -55.0, 55.0),
        ],
        "notification-dot" => vec![
            path(vec![
                L(6.0, 17.5), L(6.0, 10.5),
                Q(6.0, 4.5, 12.0, 4.5), Q(18.0, 4.5, 18.0, 10.5), L(18.0, 17.5),
            ]),
            p(&[(4.5, 17.5), (19.5, 17.5)]),
            a(12.0, 17.8, 2.0, 15.0, 165.0),
            d(18.7, 5.2, 2.0),
        ],
        "verified" => vec![
            pc(&[
                (20.8, 12.0), (18.65, 14.75), (18.22, 18.22), (14.76, 18.65),
                (12.0, 20.8), (9.24, 18.65), (5.78, 18.22), (5.35, 14.75),
                (3.2, 12.0), (5.35, 9.25), (5.78, 5.78), (9.24, 5.35),
                (12.0, 3.2), (14.76, 5.35), (18.22, 5.78), (18.65, 9.25),
            ]),
            p(&[(8.5, 12.3), (11.0, 14.8), (15.8, 9.5)]),
        ],
        "influencer" => {
            let mut v = person(12.0, 19.0, 0.9).to_vec();
            v.push(d(4.5, 6.0, 1.2));
            v.push(d(12.0, 4.0, 1.2));
            v.push(d(19.5, 6.0, 1.2));
            v
        }
        "engagement" => vec![
            chat_bubble(),
            pathc(vec![
                L(12.0, 13.0),
                Q(8.6, 10.6, 8.4, 8.6), Q(8.4, 6.6, 10.2, 6.6),
                Q(11.4, 6.6, 12.0, 7.8), Q(12.6, 6.6, 13.8, 6.6),
                Q(15.6, 6.6, 15.6, 8.6), Q(15.4, 10.6, 12.0, 13.0),
            ]),
        ],
        "reach" => vec![
            d(4.5, 19.5, 1.6),
            a(4.5, 19.5, 5.0, -90.0, 0.0),
            a(4.5, 19.5, 9.5, -90.0, 0.0),
            a(4.5, 19.5, 14.0, -90.0, 0.0),
        ],
        "post" => vec![
            rr(3.5, 3.5, 17.0, 17.0, 1.5),
            c(8.5, 8.5, 1.5),
            p(&[(5.5, 13.5), (9.5, 9.5), (12.5, 12.3), (15.0, 10.0), (18.5, 13.5)]),
            p(&[(6.5, 16.5), (17.5, 16.5)]),
        ],
        "feed" => vec![
            rr(3.5, 3.0, 17.0, 8.0, 1.2),
            rr(5.5, 4.8, 4.0, 4.2, 0.5),
            p(&[(11.5, 7.0), (18.0, 7.0)]),
            rr(3.5, 13.0, 17.0, 8.0, 1.2),
            rr(5.5, 14.8, 4.0, 4.2, 0.5),
            p(&[(11.5, 17.0), (18.0, 17.0)]),
        ],
        "timeline" => vec![
            p(&[(7.0, 3.0), (7.0, 21.0)]),
            d(7.0, 6.0, 1.5),
            p(&[(10.0, 6.0), (19.0, 6.0)]),
            d(7.0, 12.0, 1.5),
            p(&[(10.0, 12.0), (19.0, 12.0)]),
            d(7.0, 18.0, 1.5),
            p(&[(10.0, 18.0), (19.0, 18.0)]),
        ],
        "dm" => vec![
            badge(),
            pc(&[(16.8, 7.6), (6.4, 11.8), (10.4, 13.6), (12.2, 17.4)]),
            p(&[(16.8, 7.6), (10.4, 13.6)]),
        ],
        "group-chat" => vec![
            rr(9.5, 3.0, 11.5, 8.0, 2.5),
            pathc(vec![
                L(3.5, 10.5),
                Q(3.5, 8.5, 5.5, 8.5), L(13.0, 8.5),
                Q(15.0, 8.5, 15.0, 10.5), L(15.0, 15.5),
                Q(15.0, 17.5, 13.0, 17.5), L(8.0, 17.5), L(3.5, 21.0),
            ]),
        ],
        "subscriber" => {
            let mut v = person(9.5, 20.0, 0.9).to_vec();
            v.push(path(vec![
                L(16.0, 11.0), L(16.0, 8.6),
                Q(16.0, 6.0, 18.7, 6.0), Q(21.4, 6.0, 21.4, 8.6), L(21.4, 11.0),
            ]));
            v.push(p(&[(15.2, 11.0), (22.2, 11.0)]));
            v.push(d(18.7, 12.6, 0.8));
            v
        }
        "unfollow" => {
            let mut v = person(10.0, 20.0, 0.95).to_vec();
            v.push(c(18.5, 11.0, 3.6));
            v.push(p(&[(17.1, 9.6), (19.9, 12.4)]));
            v.push(p(&[(19.9, 9.6), (17.1, 12.4)]));
            v
        }
        "block-user" => {
            let mut v = person(10.0, 20.0, 0.95).to_vec();
            v.push(c(18.0, 10.5, 4.0));
            v.push(p(&[(15.2, 13.3), (20.8, 7.7)]));
            v
        }
        "mute" => vec![
            pc(&[(4.0, 9.5), (8.0, 9.5), (13.0, 5.0), (13.0, 19.0), (8.0, 14.5), (4.0, 14.5)]),
            p(&[(4.0, 4.5), (20.0, 19.5)]),
        ],
        "poll" => vec![
            rr(4.5, 4.5, 12.0, 3.4, 0.4),
            rr(4.5, 10.3, 16.0, 3.4, 0.4),
            rr(4.5, 16.1, 8.0, 3.4, 0.4),
        ],
        "emoji-react" => vec![
            c(12.0, 12.0, 8.5),
            d(9.0, 9.5, 1.2),
            d(15.0, 9.5, 1.2),
            a(12.0, 12.5, 4.5, 25.0, 155.0),
        ],
        "gif" => vec![
            rr(3.0, 6.5, 18.0, 11.0, 1.8),
            a(8.0, 12.0, 2.3, -40.0, 220.0),
            p(&[(8.0, 12.0), (10.1, 12.0)]),
            p(&[(12.4, 9.7), (12.4, 14.3)]),
            p(&[(15.2, 14.3), (15.2, 9.7), (17.8, 9.7)]),
            p(&[(15.2, 12.0), (17.2, 12.0)]),
        ],
        "tag-friend" => {
            let mut v = person(9.5, 20.0, 0.9).to_vec();
            v.push(pc(&[(15.0, 5.5), (18.6, 5.5), (21.6, 8.5), (18.4, 11.7), (15.0, 8.3)]));
            v.push(d(16.8, 7.2, 0.8));
            v
        }
        "save-post" => vec![
            pc(&[(6.5, 3.5), (17.5, 3.5), (17.5, 20.5), (12.0, 16.0), (6.5, 20.5)]),
            p(&[(12.0, 8.0), (12.0, 13.0)]),
            p(&[(9.5, 10.5), (14.5, 10.5)]),
        ],
        "analytics-social" => vec![
            rr(4.0, 12.0, 3.0, 8.0, 0.3),
            rr(8.5, 8.0, 3.0, 12.0, 0.3),
            rr(13.0, 10.0, 3.0, 10.0, 0.3),
            pathc(vec![
                L(19.0, 8.6),
                Q(16.9, 7.1, 16.8, 5.9), Q(16.8, 4.7, 17.9, 4.7),
                Q(18.6, 4.7, 19.0, 5.4), Q(19.4, 4.7, 20.1, 4.7),
                Q(21.2, 4.7, 21.2, 5.9), Q(21.1, 7.1, 19.0, 8.6),
            ]),
        ],

        _ => return None,
    })
}

/// An open hand: palm line + thumb (things are given or received over it).
fn open_hand() -> [IconShape; 2] {
    [
        path(vec![L(4.0, 17.5), L(16.0, 17.5), Q(19.0, 17.5, 20.0, 15.5)]),
        p(&[(4.0, 17.5), (4.0, 14.5), (6.5, 12.8)]),
    ]
}

/// Departments, Transactions — chunk 9.
#[rustfmt::skip]
fn departments_transactions_shapes(name: &str) -> Option<Vec<IconShape>> {
    Some(match name {
        // ── Departments ────────────────────────────────────────────────────
        "dept-hr" => {
            let mut v = person(10.0, 20.0, 0.95).to_vec();
            v.push(pathc(vec![
                L(18.5, 13.6),
                Q(15.4, 11.4, 15.3, 9.6), Q(15.3, 7.8, 16.9, 7.8),
                Q(17.9, 7.8, 18.5, 8.8), Q(19.1, 7.8, 20.1, 7.8),
                Q(21.7, 7.8, 21.7, 9.6), Q(21.6, 11.4, 18.5, 13.6),
            ]));
            v
        }
        "dept-finance" => vec![
            rr(2.5, 8.5, 19.0, 10.0, 1.5),
            c(8.0, 13.5, 2.4),
            p(&[(13.5, 15.5), (15.5, 13.0), (17.0, 14.2), (19.0, 11.5)]),
        ],
        "dept-accounting" => vec![
            rr(4.0, 4.5, 16.0, 15.0, 1.5),
            p(&[(4.0, 8.5), (20.0, 8.5)]),
            p(&[(4.0, 12.5), (20.0, 12.5)]),
            p(&[(4.0, 16.5), (20.0, 16.5)]),
            d(8.0, 8.5, 1.3),
            d(11.0, 8.5, 1.3),
            d(14.5, 12.5, 1.3),
            d(17.5, 12.5, 1.3),
            d(9.5, 16.5, 1.3),
            d(13.0, 16.5, 1.3),
        ],
        "dept-it" => vec![
            rr(3.5, 4.5, 17.0, 11.5, 1.2),
            p(&[(8.5, 8.0), (6.0, 10.2), (8.5, 12.4)]),
            p(&[(15.5, 8.0), (18.0, 10.2), (15.5, 12.4)]),
            p(&[(12.0, 16.0), (12.0, 18.5)]),
            p(&[(8.5, 18.5), (15.5, 18.5)]),
        ],
        "dept-legal" => vec![
            pc(&[(6.0, 6.5), (10.5, 2.0), (14.0, 5.5), (9.5, 10.0)]),
            p(&[(12.2, 7.8), (19.5, 15.0)]),
            p(&[(13.0, 20.5), (21.0, 20.5)]),
        ],
        "dept-marketing" => vec![
            c(12.0, 12.0, 8.0),
            c(12.0, 12.0, 4.6),
            d(12.0, 12.0, 1.5),
            p(&[(20.5, 3.5), (12.0, 12.0)]),
            p(&[(18.6, 3.3), (20.7, 3.3), (20.7, 5.4)]),
        ],
        "dept-sales" => vec![
            p(&[(3.0, 17.5), (8.5, 12.0), (12.5, 15.0), (19.0, 8.0)]),
            p(&[(15.5, 8.0), (19.0, 8.0), (19.0, 11.5)]),
            c(6.0, 5.5, 2.4),
            p(&[(6.0, 4.4), (6.0, 6.6)]),
        ],
        "dept-operations" => vec![
            rr(5.5, 4.5, 13.0, 17.0, 1.5),
            rr(9.5, 2.8, 5.0, 3.4, 1.0),
            c(12.0, 13.0, 3.0),
            c(12.0, 13.0, 1.2),
            p(&[(12.0, 8.9), (12.0, 10.0)]),
            p(&[(12.0, 16.0), (12.0, 17.1)]),
            p(&[(7.9, 13.0), (9.0, 13.0)]),
            p(&[(15.0, 13.0), (16.1, 13.0)]),
        ],
        "dept-manufacturing" => vec![
            pathc(vec![
                L(3.5, 20.0), L(3.5, 9.5), L(8.5, 13.0), L(8.5, 9.5),
                L(13.5, 13.0), L(13.5, 9.5), L(20.5, 13.2), L(20.5, 20.0),
            ]),
            rr(10.5, 16.0, 3.0, 4.0, 0.4),
        ],
        "dept-engineering" => vec![
            p(&[(12.0, 2.5), (12.0, 4.5)]),
            d(12.0, 4.5, 1.3),
            p(&[(12.0, 4.5), (7.0, 19.5)]),
            p(&[(12.0, 4.5), (17.0, 19.5)]),
            p(&[(9.7, 11.5), (14.3, 11.5)]),
            a(12.0, 6.0, 14.5, 78.0, 102.0),
        ],
        "dept-rnd" => vec![
            p(&[(10.0, 3.5), (10.0, 9.0)]),
            p(&[(14.0, 3.5), (14.0, 9.0)]),
            p(&[(8.5, 3.5), (15.5, 3.5)]),
            pathc(vec![
                L(10.0, 9.0), L(5.0, 17.5),
                Q(3.8, 20.0, 6.5, 20.0), L(17.5, 20.0),
                Q(20.2, 20.0, 19.0, 17.5), L(14.0, 9.0),
            ]),
            d(10.5, 15.5, 1.0),
            d(13.5, 17.0, 0.9),
        ],
        "dept-procurement" => {
            let mut v = cart_body().to_vec();
            v.push(p(&[(17.5, 3.5), (19.2, 5.2), (22.0, 2.4)]));
            v
        }
        "dept-customer-service" => vec![
            a(12.0, 12.0, 7.5, 180.0, 360.0),
            rr(3.5, 12.0, 3.5, 6.0, 1.2),
            rr(17.0, 12.0, 3.5, 6.0, 1.2),
            path(vec![L(18.7, 18.0), Q(18.7, 20.5, 16.0, 20.5), L(13.0, 20.5)]),
            d(12.3, 20.5, 1.1),
        ],
        "dept-quality" => vec![
            pathc(vec![
                L(12.0, 3.0),
                L(20.0, 6.0), L(20.0, 11.5),
                Q(20.0, 17.5, 12.0, 21.0),
                Q(4.0, 17.5, 4.0, 11.5), L(4.0, 6.0),
            ]),
            p(&[(8.5, 11.5), (11.0, 14.0), (15.5, 8.5)]),
        ],
        "dept-executive" => vec![
            rr(3.5, 8.0, 17.0, 11.5, 1.5),
            p(&[(9.5, 8.0), (9.5, 5.5), (14.5, 5.5), (14.5, 8.0)]),
            p(&[(3.5, 12.5), (20.5, 12.5)]),
            d(12.0, 12.5, 1.2),
        ],
        "dept-security" => vec![
            pathc(vec![
                L(12.0, 3.0),
                L(20.0, 6.0), L(20.0, 11.5),
                Q(20.0, 17.5, 12.0, 21.0),
                Q(4.0, 17.5, 4.0, 11.5), L(4.0, 6.0),
            ]),
            d(12.0, 10.5, 1.6),
            p(&[(12.0, 12.1), (12.0, 14.5)]),
        ],
        "dept-maintenance" => vec![
            c(9.0, 9.0, 4.2),
            c(9.0, 9.0, 1.8),
            p(&[(9.0, 3.2), (9.0, 4.8)]),
            p(&[(9.0, 13.2), (9.0, 14.8)]),
            p(&[(3.2, 9.0), (4.8, 9.0)]),
            p(&[(13.2, 9.0), (14.8, 9.0)]),
            a(18.2, 7.2, 2.4, 110.0, 340.0),
            p(&[(16.6, 9.0), (6.5, 19.5)]),
        ],
        "dept-training" => vec![
            pc(&[(12.0, 4.0), (21.5, 8.5), (12.0, 13.0), (2.5, 8.5)]),
            path(vec![
                L(6.5, 10.4), L(6.5, 15.0),
                Q(6.5, 17.5, 12.0, 17.5), Q(17.5, 17.5, 17.5, 15.0), L(17.5, 10.4),
            ]),
            p(&[(21.5, 8.5), (21.5, 13.5)]),
            d(21.5, 14.5, 0.9),
        ],
        "dept-logistics" => vec![
            c(12.0, 12.0, 8.0),
            p(&[(4.0, 12.0), (20.0, 12.0)]),
            ellipse(12.0, 12.0, 3.6, 8.0),
            rr(15.5, 15.5, 6.0, 5.5, 0.6),
            p(&[(18.5, 15.5), (18.5, 21.0)]),
        ],
        "dept-warehouse" => vec![
            rr(3.5, 8.0, 17.0, 12.0, 1.0),
            p(&[(2.5, 8.0), (12.0, 3.0), (21.5, 8.0)]),
            rr(8.0, 12.0, 8.0, 8.0, 0.5),
            p(&[(8.0, 14.5), (16.0, 14.5)]),
            p(&[(8.0, 17.0), (16.0, 17.0)]),
        ],
        "dept-compliance" => vec![
            rr(5.5, 4.5, 13.0, 17.0, 1.5),
            rr(9.5, 2.8, 5.0, 3.4, 1.0),
            pathc(vec![
                L(12.0, 8.5),
                L(15.2, 9.7), L(15.2, 12.6),
                Q(15.2, 15.2, 12.0, 16.5),
                Q(8.8, 15.2, 8.8, 12.6), L(8.8, 9.7),
            ]),
            p(&[(10.5, 12.3), (11.6, 13.4), (13.6, 11.0)]),
        ],
        "dept-facilities" => vec![
            rr(4.0, 4.0, 10.0, 16.5, 1.0),
            rr(7.5, 16.0, 3.0, 4.5, 0.4),
            d(7.0, 7.5, 0.9),
            d(11.0, 7.5, 0.9),
            d(7.0, 11.5, 0.9),
            d(11.0, 11.5, 0.9),
            c(17.5, 15.0, 2.6),
            d(17.5, 15.0, 1.0),
            p(&[(17.5, 11.5), (17.5, 12.4)]),
            p(&[(17.5, 17.6), (17.5, 18.5)]),
            p(&[(14.0, 15.0), (14.9, 15.0)]),
            p(&[(20.1, 15.0), (21.0, 15.0)]),
        ],
        "dept-health-safety" => vec![
            pathc(vec![
                L(12.0, 3.0),
                L(20.0, 6.0), L(20.0, 11.5),
                Q(20.0, 17.5, 12.0, 21.0),
                Q(4.0, 17.5, 4.0, 11.5), L(4.0, 6.0),
            ]),
            p(&[(12.0, 8.0), (12.0, 15.0)]),
            p(&[(8.5, 11.5), (15.5, 11.5)]),
        ],
        "dept-pr" => vec![
            chat_bubble(),
            pc(&[
                (12.0, 6.2), (13.0, 8.4), (15.4, 8.6), (13.6, 10.2), (14.2, 12.5),
                (12.0, 11.2), (9.8, 12.5), (10.4, 10.2), (8.6, 8.6), (11.0, 8.4),
            ]),
        ],
        "dept-treasury" => vec![
            path(vec![
                L(4.0, 12.0), L(4.0, 10.5),
                Q(4.0, 6.5, 12.0, 6.5), Q(20.0, 6.5, 20.0, 10.5), L(20.0, 12.0),
            ]),
            rr(4.0, 12.0, 16.0, 8.0, 1.0),
            rr(10.75, 12.0, 2.5, 3.5, 0.5),
            p(&[(8.0, 6.9), (8.0, 12.0)]),
            p(&[(16.0, 6.9), (16.0, 12.0)]),
        ],
        "dept-audit" => vec![
            c(10.5, 10.5, 6.5),
            p(&[(7.8, 10.7), (9.8, 12.7), (13.4, 8.6)]),
            p(&[(15.3, 15.3), (20.5, 20.5)]),
        ],

        // ── Transactions ───────────────────────────────────────────────────
        "buy" => {
            let mut v = cart_body().to_vec();
            v.push(p(&[(19.5, 2.5), (19.5, 6.5)]));
            v.push(p(&[(17.8, 5.0), (19.5, 6.8), (21.2, 5.0)]));
            v
        }
        "sell" => vec![
            pathc(vec![
                L(3.5, 3.5), L(10.5, 3.5), L(18.5, 11.5),
                Q(19.5, 12.5, 18.5, 13.5), L(13.5, 18.5),
                Q(12.5, 19.5, 11.5, 18.5), L(3.5, 10.5),
            ]),
            c(7.3, 7.3, 1.5),
            p(&[(14.5, 21.0), (21.0, 21.0)]),
            p(&[(19.0, 19.2), (21.3, 21.0), (19.0, 22.8)]),
        ],
        "withdraw" => vec![
            rr(3.5, 3.5, 17.0, 3.0, 0.6),
            p(&[(12.0, 7.5), (12.0, 11.0)]),
            p(&[(10.0, 9.2), (12.0, 11.3), (14.0, 9.2)]),
            rr(5.5, 13.0, 13.0, 7.5, 0.8),
            c(12.0, 16.75, 1.9),
        ],
        "deposit" => vec![
            rr(5.5, 3.5, 13.0, 7.5, 0.8),
            c(12.0, 7.25, 1.9),
            p(&[(12.0, 12.5), (12.0, 16.0)]),
            p(&[(10.0, 14.2), (12.0, 16.3), (14.0, 14.2)]),
            rr(3.5, 17.5, 17.0, 3.0, 0.6),
        ],
        "return" => vec![
            rr(6.0, 12.0, 12.0, 7.5, 0.8),
            path(vec![
                L(18.0, 12.0), L(18.0, 8.5),
                Q(18.0, 4.5, 12.5, 4.5), Q(7.0, 4.5, 7.0, 8.5), L(7.0, 10.0),
            ]),
            p(&[(5.2, 8.2), (7.0, 10.3), (8.8, 8.2)]),
        ],
        "exchange" => vec![
            p(&[(4.0, 8.5), (20.0, 8.5)]),
            p(&[(17.5, 6.2), (20.3, 8.5), (17.5, 10.8)]),
            p(&[(20.0, 15.5), (4.0, 15.5)]),
            p(&[(6.5, 13.2), (3.7, 15.5), (6.5, 17.8)]),
        ],
        "order" => {
            let mut v = page().to_vec();
            v.push(d(9.7, 10.5, 0.8));
            v.push(p(&[(11.5, 10.5), (14.5, 10.5)]));
            v.push(d(9.7, 13.5, 0.8));
            v.push(p(&[(11.5, 13.5), (14.5, 13.5)]));
            v.push(d(9.7, 16.5, 0.8));
            v.push(p(&[(11.5, 16.5), (14.5, 16.5)]));
            v
        }
        "delivery" => {
            let mut v = vec![
                rr(6.5, 3.5, 11.0, 9.0, 0.8),
                p(&[(12.0, 3.5), (12.0, 12.5)]),
            ];
            add(&mut v, open_hand());
            v
        }
        "borrow" => {
            let mut v = open_hand().to_vec();
            v.push(p(&[(19.5, 3.5), (13.5, 9.5)]));
            v.push(p(&[(13.5, 5.7), (13.5, 9.5), (17.3, 9.5)]));
            v
        }
        "lend" => {
            let mut v = open_hand().to_vec();
            v.push(p(&[(13.5, 9.5), (19.5, 3.5)]));
            v.push(p(&[(15.7, 3.5), (19.5, 3.5), (19.5, 7.3)]));
            v
        }
        "showback" => {
            let mut v = page().to_vec();
            v.push(pathc(vec![
                L(8.5, 13.5),
                Q(10.2, 11.3, 12.0, 11.3), Q(13.8, 11.3, 15.5, 13.5),
                Q(13.8, 15.7, 12.0, 15.7), Q(10.2, 15.7, 8.5, 13.5),
            ]));
            v.push(d(12.0, 13.5, 1.0));
            v
        }
        "chargeback" => vec![
            rr(2.5, 4.5, 19.0, 11.0, 1.8),
            p(&[(2.5, 8.0), (21.5, 8.0)]),
            p(&[(19.5, 18.5), (7.0, 18.5)]),
            p(&[(9.3, 16.3), (6.7, 18.5), (9.3, 20.7)]),
        ],
        "quote" => {
            let mut v = vec![chat_bubble()];
            add(&mut v, dollar_glyph(12.0, 10.0, 0.35));
            v
        }
        "subscription" => vec![
            rr(3.0, 7.0, 18.0, 12.0, 1.5),
            p(&[(3.5, 8.0), (12.0, 14.0), (20.5, 8.0)]),
            a(12.0, 15.0, 2.6, -60.0, 180.0),
            p(&[(13.1, 12.0), (14.7, 12.6), (14.0, 14.1)]),
        ],
        "renewal" => vec![
            a(12.0, 12.0, 7.5, 270.0, 540.0),
            p(&[(2.3, 14.4), (4.5, 12.0), (6.7, 14.4)]),
            p(&[(9.0, 12.0), (11.3, 14.3), (15.5, 9.5)]),
        ],
        "cancellation" => vec![c(12.0, 12.0, 8.5), p(&[(6.0, 6.0), (18.0, 18.0)])],
        "auction" => vec![
            c(12.0, 8.5, 5.0),
            p(&[(12.0, 6.5), (12.0, 10.5)]),
            rr(11.0, 13.5, 2.0, 7.0, 1.0),
        ],
        "bid" => vec![
            c(9.5, 14.0, 4.5),
            p(&[(9.5, 12.0), (9.5, 16.0)]),
            p(&[(17.5, 16.0), (17.5, 6.0)]),
            p(&[(14.8, 8.7), (17.5, 6.0), (20.2, 8.7)]),
        ],
        "settlement" => vec![
            p(&[(2.5, 12.0), (6.5, 12.0), (10.2, 15.5)]),
            p(&[(21.5, 12.0), (17.5, 12.0), (13.8, 15.5)]),
            p(&[(10.2, 15.5), (14.2, 12.6)]),
            p(&[(13.8, 15.5), (9.8, 12.6)]),
            c(12.0, 5.5, 2.4),
            p(&[(12.0, 4.4), (12.0, 6.6)]),
        ],
        "layaway" => {
            let mut v = box3d().to_vec();
            v.push(p(&[(17.5, 2.5), (17.5, 6.5)]));
            v.push(p(&[(20.5, 2.5), (20.5, 6.5)]));
            v
        }
        "preorder" => {
            let mut v = box3d().to_vec();
            v.push(c(19.0, 4.5, 3.0));
            v.push(p(&[(19.0, 3.0), (19.0, 4.5), (20.6, 5.4)]));
            v
        }
        "backorder" => vec![
            rr(4.0, 10.0, 7.5, 7.5, 0.8),
            p(&[(7.75, 10.0), (7.75, 17.5)]),
            p(&[(13.0, 6.0), (15.0, 6.0)]),
            p(&[(17.0, 6.0), (19.0, 6.0)]),
            p(&[(20.5, 7.5), (20.5, 9.5)]),
            p(&[(20.5, 11.5), (20.5, 13.5)]),
            p(&[(19.0, 15.0), (17.0, 15.0)]),
            p(&[(15.0, 15.0), (13.0, 15.0)]),
            p(&[(11.5, 13.5), (11.5, 11.5)]),
            p(&[(11.5, 9.5), (11.5, 7.5)]),
        ],
        "cash-on-delivery" => {
            let mut v = box3d().to_vec();
            v.push(c(19.5, 4.5, 3.0));
            v.push(p(&[(19.5, 3.2), (19.5, 5.8)]));
            v
        }
        "trade-in" => vec![
            rr(7.5, 8.5, 9.0, 7.5, 0.8),
            a(12.0, 12.0, 8.5, 190.0, 360.0),
            p(&[(18.7, 9.8), (20.5, 12.0), (22.3, 9.8)]),
            a(12.0, 12.0, 8.5, 10.0, 180.0),
            p(&[(1.7, 14.2), (3.5, 12.0), (5.3, 14.2)]),
        ],
        "donation" => {
            let mut v = open_hand().to_vec();
            v.push(pathc(vec![
                L(12.0, 10.8),
                Q(8.6, 8.4, 8.4, 6.4), Q(8.4, 4.4, 10.2, 4.4),
                Q(11.4, 4.4, 12.0, 5.6), Q(12.6, 4.4, 13.8, 4.4),
                Q(15.6, 4.4, 15.6, 6.4), Q(15.4, 8.4, 12.0, 10.8),
            ]));
            v
        }
        "recurring-charge" => vec![
            c(12.0, 12.0, 4.5),
            p(&[(12.0, 10.0), (12.0, 14.0)]),
            ellipse(12.0, 12.0, 9.0, 4.5),
        ],

        _ => return None,
    })
}

/// A sedan profile: body, beltline, two wheels, window divider.
fn car_body() -> [IconShape; 5] {
    [
        path(vec![
            L(2.5, 15.5),
            L(2.5, 12.5),
            Q(2.5, 11.0, 5.0, 10.8),
            L(7.0, 7.5),
            Q(7.5, 6.8, 8.5, 6.8),
            L(15.0, 6.8),
            Q(16.0, 6.8, 16.8, 7.6),
            L(19.5, 10.8),
            Q(21.5, 11.0, 21.5, 12.8),
            L(21.5, 15.5),
        ]),
        p(&[(5.0, 10.8), (19.5, 10.8)]),
        IconShape::Circle(7.0, 16.5, 2.2),
        IconShape::Circle(17.0, 16.5, 2.2),
        p(&[(11.5, 6.8), (11.5, 10.8)]),
    ]
}

/// The military shield outline (shared by shield-family icons).
fn mil_shield() -> IconShape {
    pathc(vec![
        L(12.0, 3.0),
        L(20.0, 6.0),
        L(20.0, 11.5),
        Q(20.0, 17.5, 12.0, 21.0),
        Q(4.0, 17.5, 4.0, 11.5),
        L(4.0, 6.0),
    ])
}

/// A cloud outline with a flat bottom — the PaaS family container.
fn cloud() -> IconShape {
    pathc(vec![
        L(7.0, 16.5),
        Q(3.0, 16.5, 3.0, 12.5),
        Q(3.0, 9.0, 6.5, 8.5),
        Q(7.0, 4.0, 11.5, 4.0),
        Q(15.5, 4.0, 16.5, 7.5),
        Q(21.0, 8.0, 21.0, 12.0),
        Q(21.0, 16.5, 17.0, 16.5),
    ])
}

/// A browser window — the SaaS family container. Glyphs go in the body
/// (roughly x 5..19, y 9.5..18.5).
fn browser() -> [IconShape; 4] {
    [
        rr(3.0, 4.0, 18.0, 16.0, 1.5),
        p(&[(3.0, 8.0), (21.0, 8.0)]),
        d(5.5, 6.0, 0.7),
        d(7.5, 6.0, 0.7),
    ]
}

/// An ERP module tile: rounded square + connector pins on both sides.
/// Glyphs go in the centre (roughly x 6..18, y 6..18).
fn module_tile() -> [IconShape; 5] {
    [
        rr(4.0, 4.0, 16.0, 16.0, 2.0),
        p(&[(2.5, 9.0), (4.0, 9.0)]),
        p(&[(2.5, 15.0), (4.0, 15.0)]),
        p(&[(20.0, 9.0), (21.5, 9.0)]),
        p(&[(20.0, 15.0), (21.5, 15.0)]),
    ]
}

/// Devices, SaaS, PaaS, ERP modules — chunk 11.
#[rustfmt::skip]
fn devices_cloud_shapes(name: &str) -> Option<Vec<IconShape>> {
    Some(match name {
        // ── Devices ────────────────────────────────────────────────────────
        "desktop-computer" => vec![
            rr(3.0, 5.0, 5.0, 14.0, 0.8),
            d(5.5, 7.5, 0.7),
            p(&[(4.5, 10.0), (6.5, 10.0)]),
            rr(10.0, 6.5, 11.0, 8.5, 0.8),
            p(&[(15.5, 15.0), (15.5, 17.5)]),
            p(&[(12.5, 18.0), (18.5, 18.0)]),
        ],
        "laptop" => vec![
            rr(5.0, 5.0, 14.0, 9.5, 1.0),
            pc(&[(4.5, 14.5), (19.5, 14.5), (21.0, 17.5), (3.0, 17.5)]),
        ],
        "monitor" => vec![
            rr(3.5, 4.5, 17.0, 11.5, 1.2),
            p(&[(12.0, 16.0), (12.0, 18.5)]),
            p(&[(8.0, 19.0), (16.0, 19.0)]),
        ],
        "all-in-one" => vec![
            rr(4.0, 4.0, 16.0, 12.0, 1.2),
            p(&[(4.0, 13.0), (20.0, 13.0)]),
            p(&[(10.5, 16.0), (9.0, 19.5)]),
            p(&[(13.5, 16.0), (15.0, 19.5)]),
            p(&[(7.5, 19.5), (16.5, 19.5)]),
        ],
        "computer-tower" => vec![
            rr(7.5, 3.0, 9.0, 18.0, 1.2),
            d(12.0, 6.0, 1.0),
            p(&[(9.5, 9.0), (14.5, 9.0)]),
            p(&[(9.5, 11.5), (14.5, 11.5)]),
            d(10.5, 16.5, 0.7),
            d(13.5, 16.5, 0.7),
        ],
        "mainframe" => vec![
            rr(5.0, 3.0, 14.0, 18.0, 1.0),
            c(9.5, 7.5, 2.0),
            c(14.5, 7.5, 2.0),
            p(&[(5.0, 12.0), (19.0, 12.0)]),
            d(8.0, 15.0, 0.8),
            d(11.0, 15.0, 0.8),
            d(14.0, 15.0, 0.8),
            p(&[(8.0, 18.0), (16.0, 18.0)]),
        ],
        "terminal-crt" => vec![
            rr(4.0, 3.5, 16.0, 13.0, 2.0),
            rr(6.5, 5.5, 11.0, 8.5, 1.0),
            p(&[(8.5, 8.0), (10.3, 9.7), (8.5, 11.4)]),
            p(&[(12.0, 11.5), (14.5, 11.5)]),
            pc(&[(8.0, 16.5), (16.0, 16.5), (17.0, 20.0), (7.0, 20.0)]),
        ],
        "retro-computer" => vec![
            rr(7.5, 2.5, 9.0, 7.0, 0.8),
            pc(&[(3.5, 12.0), (20.5, 12.0), (21.5, 18.5), (2.5, 18.5)]),
            p(&[(5.5, 14.5), (18.5, 14.5)]),
            p(&[(6.0, 16.5), (18.0, 16.5)]),
        ],
        "punch-card" => vec![
            pathc(vec![L(4.0, 5.5), L(17.5, 5.5), L(20.0, 8.0), L(20.0, 18.5), L(4.0, 18.5)]),
            rrf(6.0, 9.0, 1.4, 0.9, 0.2),
            rrf(9.0, 9.0, 1.4, 0.9, 0.2),
            rrf(13.5, 9.0, 1.4, 0.9, 0.2),
            rrf(7.5, 12.0, 1.4, 0.9, 0.2),
            rrf(11.0, 12.0, 1.4, 0.9, 0.2),
            rrf(16.0, 12.0, 1.4, 0.9, 0.2),
            rrf(6.0, 15.0, 1.4, 0.9, 0.2),
            rrf(10.0, 15.0, 1.4, 0.9, 0.2),
            rrf(14.5, 15.0, 1.4, 0.9, 0.2),
        ],
        "tablet" => vec![rr(5.5, 3.5, 13.0, 17.0, 1.8), d(12.0, 18.7, 0.8)],
        "smartphone" => vec![
            rr(7.5, 3.0, 9.0, 18.0, 1.8),
            p(&[(10.5, 5.0), (13.5, 5.0)]),
            d(12.0, 18.9, 0.8),
        ],
        "smartwatch" => vec![
            rr(7.5, 7.0, 9.0, 10.0, 2.5),
            p(&[(9.5, 7.0), (9.5, 3.5), (14.5, 3.5), (14.5, 7.0)]),
            p(&[(9.5, 17.0), (9.5, 20.5), (14.5, 20.5), (14.5, 17.0)]),
            p(&[(12.0, 10.5), (12.0, 12.0), (13.5, 13.0)]),
        ],
        "fitness-band" => vec![
            rr(9.5, 3.0, 5.0, 18.0, 2.5),
            rr(10.3, 8.5, 3.4, 7.0, 1.0),
            p(&[(11.0, 12.0), (12.0, 13.0), (13.0, 11.5)]),
        ],
        "vr-headset" => vec![
            rr(3.5, 8.0, 17.0, 8.5, 3.0),
            a(12.0, 16.5, 2.2, 200.0, 340.0),
            p(&[(3.5, 12.0), (2.0, 12.0)]),
            p(&[(20.5, 12.0), (22.0, 12.0)]),
            c(8.5, 12.0, 1.8),
            c(15.5, 12.0, 1.8),
        ],
        "earbuds" => vec![
            c(8.5, 9.5, 2.2),
            p(&[(8.5, 11.7), (8.5, 16.5)]),
            c(15.5, 9.5, 2.2),
            p(&[(15.5, 11.7), (15.5, 16.5)]),
        ],

        // ── SaaS (browser window + domain glyph) ───────────────────────────
        "saas-crm" => {
            let mut v = browser().to_vec();
            v.push(c(9.5, 12.5, 1.8));
            v.push(path(vec![L(6.5, 17.5), Q(6.5, 14.9, 9.5, 14.9), Q(12.5, 14.9, 12.5, 17.5)]));
            v.push(pathc(vec![
                L(16.0, 16.2),
                Q(14.0, 14.7, 13.9, 13.5), Q(13.9, 12.3, 15.0, 12.3),
                Q(15.6, 12.3, 16.0, 13.0), Q(16.4, 12.3, 17.0, 12.3),
                Q(18.1, 12.3, 18.1, 13.5), Q(18.0, 14.7, 16.0, 16.2),
            ]));
            v
        }
        "saas-erp" => {
            let mut v = browser().to_vec();
            v.push(c(12.0, 13.5, 2.6));
            v.push(d(12.0, 13.5, 1.0));
            v.push(p(&[(12.0, 9.7), (12.0, 10.9)]));
            v.push(p(&[(12.0, 16.1), (12.0, 17.3)]));
            v.push(p(&[(8.2, 13.5), (9.4, 13.5)]));
            v.push(p(&[(14.6, 13.5), (15.8, 13.5)]));
            v.push(p(&[(13.84, 15.34), (14.69, 16.19)]));
            v.push(p(&[(10.16, 15.34), (9.31, 16.19)]));
            v.push(p(&[(10.16, 11.66), (9.31, 10.81)]));
            v.push(p(&[(13.84, 11.66), (14.69, 10.81)]));
            v
        }
        "saas-hrtech" => {
            let mut v = browser().to_vec();
            v.push(c(9.0, 12.5, 1.7));
            v.push(path(vec![L(6.3, 17.5), Q(6.3, 15.0, 9.0, 15.0), Q(11.7, 15.0, 11.7, 17.5)]));
            v.push(c(15.0, 12.5, 1.7));
            v.push(path(vec![L(12.9, 17.5), Q(12.9, 15.0, 15.0, 15.0), Q(17.7, 15.0, 17.7, 17.5)]));
            v
        }
        "saas-bi" => {
            let mut v = browser().to_vec();
            v.push(p(&[(6.0, 17.5), (18.5, 17.5)]));
            v.push(p(&[(8.5, 17.5), (8.5, 14.0)]));
            v.push(p(&[(12.0, 17.5), (12.0, 10.5)]));
            v.push(p(&[(15.5, 17.5), (15.5, 12.5)]));
            v
        }
        "saas-lms" => {
            let mut v = browser().to_vec();
            v.push(pc(&[(12.0, 10.0), (17.0, 12.3), (12.0, 14.6), (7.0, 12.3)]));
            v.push(path(vec![
                L(9.5, 13.4), L(9.5, 15.2),
                Q(9.5, 16.6, 12.0, 16.6), Q(14.5, 16.6, 14.5, 15.2), L(14.5, 13.4),
            ]));
            v.push(p(&[(17.0, 12.3), (17.0, 15.5)]));
            v
        }
        "saas-cms" => {
            let mut v = browser().to_vec();
            v.push(p(&[(6.5, 12.0), (11.0, 12.0)]));
            v.push(p(&[(6.5, 15.0), (9.5, 15.0)]));
            v.push(p(&[(12.5, 16.5), (17.5, 11.5)]));
            v.push(pf(&[(12.5, 16.5), (11.7, 17.8), (13.3, 17.6)]));
            v
        }
        "saas-wcm" => {
            let mut v = browser().to_vec();
            v.push(c(12.0, 13.5, 3.6));
            v.push(p(&[(8.4, 13.5), (15.6, 13.5)]));
            v.push(ellipse(12.0, 13.5, 1.6, 3.6));
            v
        }
        "saas-itsm" => {
            let mut v = browser().to_vec();
            v.push(a(15.8, 10.8, 2.6, 110.0, 340.0));
            v.push(p(&[(14.3, 12.9), (8.5, 18.3)]));
            v
        }
        "saas-plm" => {
            let mut v = browser().to_vec();
            v.push(rr(10.3, 11.8, 3.4, 3.4, 0.5));
            v.push(a(12.0, 13.5, 4.8, 20.0, 260.0));
            v.push(p(&[(10.0, 8.4), (11.4, 9.4), (10.2, 10.6)]));
            v
        }
        "saas-scm" => {
            let mut v = browser().to_vec();
            v.push(c(7.5, 13.5, 1.6));
            v.push(c(12.0, 13.5, 1.6));
            v.push(c(16.5, 13.5, 1.6));
            v.push(p(&[(9.1, 13.5), (10.4, 13.5)]));
            v.push(p(&[(13.6, 13.5), (14.9, 13.5)]));
            v
        }
        "saas-pos" => {
            let mut v = browser().to_vec();
            v.push(rr(8.5, 10.5, 7.0, 3.0, 0.5));
            v.push(rr(7.0, 13.5, 10.0, 4.0, 0.5));
            v.push(d(9.5, 15.5, 0.7));
            v.push(d(12.0, 15.5, 0.7));
            v.push(d(14.5, 15.5, 0.7));
            v
        }
        "saas-chatbot" => {
            let mut v = browser().to_vec();
            v.push(rr(7.5, 11.0, 9.0, 6.5, 1.5));
            v.push(d(10.3, 14.0, 0.9));
            v.push(d(13.7, 14.0, 0.9));
            v.push(p(&[(12.0, 11.0), (12.0, 9.5)]));
            v.push(d(12.0, 9.0, 0.6));
            v
        }

        // ── PaaS (cloud + domain glyph) ────────────────────────────────────
        "apaas" => vec![cloud(), rr(9.0, 9.5, 6.0, 5.0, 0.8), p(&[(9.0, 11.0), (15.0, 11.0)])],
        "dbpaas" => vec![
            cloud(),
            ellipse(12.0, 9.8, 2.8, 1.0),
            p(&[(9.2, 9.8), (9.2, 13.2)]),
            p(&[(14.8, 9.8), (14.8, 13.2)]),
            path(vec![L(9.2, 13.2), Q(9.2, 14.4, 12.0, 14.4), Q(14.8, 14.4, 14.8, 13.2)]),
        ],
        "ipaas" => vec![
            cloud(),
            c(8.8, 11.8, 1.5),
            c(15.2, 11.8, 1.5),
            p(&[(10.3, 11.8), (13.7, 11.8)]),
        ],
        "mpaas" => vec![cloud(), rr(10.0, 8.8, 4.0, 6.4, 1.0), d(12.0, 13.9, 0.5)],
        "cpaas" => vec![
            cloud(),
            pathc(vec![
                L(8.8, 9.5), L(15.2, 9.5),
                Q(16.2, 9.5, 16.2, 10.5), L(16.2, 12.7),
                Q(16.2, 13.7, 15.2, 13.7), L(11.0, 13.7), L(8.8, 15.5),
            ]),
        ],
        "baas" => vec![
            cloud(),
            rr(8.8, 9.0, 6.4, 2.6, 0.5),
            rr(8.8, 12.4, 6.4, 2.6, 0.5),
            d(10.2, 10.3, 0.5),
            d(10.2, 13.7, 0.5),
        ],
        "mbaas" => vec![
            cloud(),
            rr(9.2, 8.8, 3.4, 6.0, 0.8),
            p(&[(14.2, 10.3), (16.8, 10.3)]),
            p(&[(15.9, 9.4), (16.9, 10.3), (15.9, 11.2)]),
            p(&[(16.8, 13.0), (14.2, 13.0)]),
            p(&[(15.1, 12.1), (14.1, 13.0), (15.1, 13.9)]),
        ],
        "faas" => vec![
            cloud(),
            p(&[(10.0, 8.7), (14.5, 15.3)]),
            p(&[(12.15, 11.8), (9.5, 15.3)]),
        ],
        "secpaas" => vec![
            cloud(),
            pathc(vec![
                L(12.0, 8.7),
                L(14.8, 9.7), L(14.8, 11.9),
                Q(14.8, 13.9, 12.0, 15.0),
                Q(9.2, 13.9, 9.2, 11.9), L(9.2, 9.7),
            ]),
        ],
        "aiaas" => vec![
            cloud(),
            d(8.8, 14.2, 0.8),
            d(12.0, 8.8, 0.8),
            d(15.2, 14.2, 0.8),
            p(&[(9.5, 13.0), (11.3, 10.0)]),
            p(&[(12.7, 10.0), (14.5, 13.0)]),
        ],

        // ── ERP Modules (module tile + domain glyph) ───────────────────────
        "erp-fi" => {
            let mut v = module_tile().to_vec();
            add(&mut v, dollar_glyph(12.0, 12.0, 0.52));
            v
        }
        "erp-co" => {
            let mut v = module_tile().to_vec();
            v.push(a(12.0, 13.5, 4.0, 180.0, 360.0));
            v.push(p(&[(12.0, 13.5), (14.5, 11.0)]));
            v.push(d(12.0, 13.5, 0.9));
            v
        }
        "erp-sd" => {
            let mut v = module_tile().to_vec();
            v.push(p(&[
                (8.0, 8.5), (9.3, 8.5), (10.4, 13.0), (15.4, 13.0), (16.4, 9.8), (9.8, 9.8),
            ]));
            v.push(d(11.3, 15.2, 0.9));
            v.push(d(14.4, 15.2, 0.9));
            v
        }
        "erp-mm" => {
            let mut v = module_tile().to_vec();
            v.push(pc(&[(8.0, 10.4), (12.0, 8.4), (16.0, 10.4), (12.0, 12.4)]));
            v.push(p(&[(8.0, 10.4), (8.0, 13.8), (12.0, 15.8), (12.0, 12.4)]));
            v.push(p(&[(16.0, 10.4), (16.0, 13.8), (12.0, 15.8)]));
            v
        }
        "erp-pp" => {
            let mut v = module_tile().to_vec();
            v.push(pathc(vec![
                L(8.0, 16.0), L(8.0, 10.5), L(11.0, 12.3), L(11.0, 10.5),
                L(14.0, 12.3), L(14.0, 10.5), L(16.5, 12.0), L(16.5, 16.0),
            ]));
            v
        }
        "erp-qm" => {
            let mut v = module_tile().to_vec();
            v.push(c(12.0, 11.5, 3.2));
            v.push(p(&[(10.5, 11.5), (11.7, 12.7), (13.7, 10.3)]));
            v.push(p(&[(10.8, 14.2), (10.0, 16.5)]));
            v.push(p(&[(13.2, 14.2), (14.0, 16.5)]));
            v
        }
        "erp-pm" => {
            let mut v = module_tile().to_vec();
            v.push(a(15.0, 9.2, 2.5, 110.0, 340.0));
            v.push(p(&[(13.6, 11.2), (8.3, 16.3)]));
            v
        }
        "erp-scm" => {
            let mut v = module_tile().to_vec();
            v.push(c(8.5, 12.0, 1.5));
            v.push(c(15.5, 12.0, 1.5));
            v.push(p(&[(10.0, 12.0), (13.0, 12.0)]));
            v.push(p(&[(12.4, 10.9), (13.7, 12.0), (12.4, 13.1)]));
            v
        }

        _ => return None,
    })
}

/// Vehicles, Military — chunk 10.
#[rustfmt::skip]
fn vehicles_military_shapes(name: &str) -> Option<Vec<IconShape>> {
    Some(match name {
        // ── Vehicles ───────────────────────────────────────────────────────
        "car" => car_body().to_vec(),
        "bus" => vec![
            rr(3.0, 5.5, 18.0, 13.0, 1.5),
            p(&[(3.0, 10.0), (21.0, 10.0)]),
            p(&[(7.5, 5.5), (7.5, 10.0)]),
            p(&[(12.0, 5.5), (12.0, 10.0)]),
            p(&[(16.5, 5.5), (16.5, 10.0)]),
            c(7.0, 18.8, 1.9),
            c(17.0, 18.8, 1.9),
        ],
        "motorcycle" => vec![
            c(5.5, 16.0, 3.5),
            c(18.5, 16.0, 3.5),
            p(&[(18.5, 16.0), (15.5, 7.0)]),
            p(&[(13.8, 7.0), (17.2, 7.0)]),
            p(&[(5.5, 16.0), (9.5, 10.0), (14.0, 10.0), (15.8, 13.0)]),
            p(&[(8.0, 9.9), (11.5, 9.9)]),
        ],
        "bicycle" => vec![
            c(5.5, 15.5, 4.0),
            c(18.5, 15.5, 4.0),
            p(&[(5.5, 15.5), (9.5, 8.5), (15.0, 8.5), (18.5, 15.5)]),
            p(&[(9.5, 8.5), (12.5, 15.5), (5.5, 15.5)]),
            p(&[(15.0, 8.5), (14.3, 6.3)]),
            p(&[(13.3, 6.0), (15.3, 6.0)]),
            p(&[(9.5, 8.5), (9.0, 6.5)]),
            p(&[(8.0, 6.3), (10.0, 6.3)]),
        ],
        "scooter" => vec![
            c(4.8, 18.7, 1.8),
            c(15.0, 18.7, 1.8),
            p(&[(4.5, 16.9), (14.0, 16.9)]),
            p(&[(14.0, 16.9), (18.0, 5.5)]),
            p(&[(16.0, 5.2), (20.0, 5.2)]),
        ],
        "taxi" => {
            let mut v = car_body().to_vec();
            v.push(rr(10.0, 4.2, 4.0, 2.6, 0.4));
            v
        }
        "pickup" => vec![
            path(vec![
                L(2.5, 15.5), L(2.5, 11.5), L(4.5, 7.5), L(10.5, 7.5),
                L(12.5, 11.3), L(21.5, 11.3), L(21.5, 15.5),
            ]),
            p(&[(12.5, 11.3), (12.5, 15.5)]),
            c(6.5, 16.5, 2.2),
            c(17.5, 16.5, 2.2),
        ],
        "suv" => vec![
            path(vec![
                L(2.5, 15.5), L(2.5, 9.5),
                Q(2.5, 7.0, 5.0, 7.0), L(17.5, 7.0),
                Q(19.0, 7.0, 19.8, 8.2), L(21.5, 11.0), L(21.5, 15.5),
            ]),
            p(&[(4.5, 11.0), (21.5, 11.0)]),
            p(&[(8.5, 7.0), (8.5, 11.0)]),
            p(&[(14.0, 7.0), (14.0, 11.0)]),
            c(6.8, 16.5, 2.3),
            c(17.2, 16.5, 2.3),
        ],
        "minivan" => vec![
            path(vec![
                L(2.5, 15.5), L(2.5, 11.0),
                Q(2.5, 7.2, 7.0, 6.8), L(14.0, 6.8),
                Q(18.5, 6.8, 20.5, 10.5),
                Q(21.5, 12.0, 21.5, 13.5), L(21.5, 15.5),
            ]),
            p(&[(4.5, 10.8), (20.3, 10.8)]),
            p(&[(11.0, 6.8), (11.0, 10.8)]),
            c(7.0, 16.6, 2.2),
            c(17.0, 16.6, 2.2),
        ],
        "ambulance" => vec![
            rr(2.5, 7.5, 13.0, 9.5, 1.0),
            path(vec![
                L(15.5, 10.0), L(18.5, 10.0), L(21.5, 13.0), L(21.5, 17.0), L(15.5, 17.0),
            ]),
            p(&[(8.0, 10.0), (8.0, 14.0)]),
            p(&[(6.0, 12.0), (10.0, 12.0)]),
            c(6.5, 18.5, 2.0),
            c(18.0, 18.5, 2.0),
            rr(6.5, 5.8, 3.0, 1.7, 0.4),
        ],
        "fire-truck" => vec![
            rr(2.0, 8.0, 13.0, 8.0, 1.0),
            path(vec![
                L(15.0, 10.0), L(18.5, 10.0), L(21.5, 13.0), L(21.5, 16.0), L(15.0, 16.0),
            ]),
            p(&[(3.0, 5.0), (14.0, 5.0)]),
            p(&[(3.0, 6.8), (14.0, 6.8)]),
            p(&[(5.5, 5.0), (5.5, 6.8)]),
            p(&[(8.5, 5.0), (8.5, 6.8)]),
            p(&[(11.5, 5.0), (11.5, 6.8)]),
            c(6.5, 18.0, 2.2),
            c(17.5, 18.0, 2.2),
        ],
        "police-car" => {
            let mut v = car_body().to_vec();
            v.push(rr(9.5, 4.3, 5.0, 2.2, 0.4));
            v.push(p(&[(12.0, 4.3), (12.0, 6.5)]));
            v
        }
        "tractor" => vec![
            c(16.5, 15.0, 5.0),
            d(16.5, 15.0, 1.2),
            c(5.5, 17.0, 2.8),
            path(vec![
                L(2.8, 14.2), L(2.8, 11.0), L(8.0, 11.0), L(8.0, 6.5),
                L(12.5, 6.5), L(12.5, 10.8),
            ]),
            p(&[(5.5, 11.0), (5.5, 7.5)]),
        ],
        "excavator" => vec![
            rr(6.0, 16.0, 12.0, 4.0, 2.0),
            rr(7.0, 10.5, 7.0, 5.5, 0.8),
            p(&[(14.0, 12.0), (19.0, 6.0)]),
            p(&[(19.0, 6.0), (21.5, 11.0)]),
            pc(&[(21.5, 11.0), (20.3, 14.0), (22.8, 13.2)]),
        ],
        "bulldozer" => vec![
            rr(5.5, 15.0, 12.0, 4.5, 2.2),
            rr(7.5, 9.0, 6.0, 6.0, 0.8),
            p(&[(13.5, 12.5), (19.0, 13.5)]),
            rr(19.0, 10.5, 2.0, 9.0, 0.8),
        ],
        "dump-truck" => vec![
            path(vec![
                L(2.5, 15.7), L(2.5, 10.5), L(4.5, 7.5), L(8.5, 7.5), L(8.5, 15.7),
            ]),
            pc(&[(9.5, 7.2), (21.3, 10.0), (19.8, 15.7), (9.5, 15.7)]),
            c(5.5, 17.7, 2.0),
            c(12.0, 17.7, 2.0),
            c(17.0, 17.7, 2.0),
        ],
        "cement-mixer" => vec![
            path(vec![
                L(2.5, 15.5), L(2.5, 10.5), L(4.5, 8.0), L(8.0, 8.0), L(8.0, 15.5),
            ]),
            pc(&[(9.0, 10.0), (18.5, 7.0), (20.5, 12.0), (11.0, 15.0)]),
            p(&[(13.0, 8.7), (15.5, 13.7)]),
            p(&[(15.0, 13.8), (15.0, 15.5)]),
            c(5.5, 17.5, 2.0),
            c(16.5, 17.5, 2.0),
        ],
        "tow-truck" => vec![
            path(vec![
                L(2.5, 15.5), L(2.5, 10.0), L(4.5, 7.5), L(9.0, 7.5), L(9.0, 15.5),
            ]),
            p(&[(9.0, 12.5), (20.5, 12.5)]),
            p(&[(18.5, 12.5), (21.0, 7.5)]),
            p(&[(21.0, 7.5), (21.0, 10.5)]),
            a(20.5, 11.2, 0.9, -70.0, 190.0),
            c(6.0, 17.5, 2.0),
            c(16.0, 17.5, 2.0),
        ],
        "trailer" => vec![
            rr(2.5, 7.0, 15.0, 9.5, 0.8),
            p(&[(17.5, 12.0), (21.5, 12.0)]),
            p(&[(19.5, 12.0), (19.5, 15.5)]),
            c(8.0, 18.5, 2.0),
            c(13.0, 18.5, 2.0),
        ],
        "rv" => vec![
            path(vec![
                L(2.5, 16.0), L(2.5, 9.0),
                Q(2.5, 6.5, 5.0, 6.5), L(18.0, 6.5),
                Q(21.5, 6.5, 21.5, 10.0), L(21.5, 16.0),
            ]),
            rr(5.0, 9.0, 4.0, 3.5, 0.4),
            rr(15.5, 9.0, 3.5, 3.5, 0.4),
            rr(10.5, 10.0, 3.0, 6.0, 0.4),
            c(6.5, 17.5, 2.0),
            c(17.0, 17.5, 2.0),
        ],
        "golf-cart" => vec![
            p(&[(4.5, 4.5), (17.5, 4.5)]),
            p(&[(5.5, 4.5), (5.5, 12.0)]),
            p(&[(16.5, 4.5), (16.5, 9.0)]),
            path(vec![
                L(3.0, 14.5), L(3.0, 12.0), L(8.0, 12.0), L(10.0, 9.0),
                L(19.0, 9.0), L(21.0, 12.0), L(21.0, 14.5),
            ]),
            c(6.5, 16.0, 2.2),
            c(17.5, 16.0, 2.2),
        ],
        "snowmobile" => vec![
            pc(&[(4.0, 13.5), (9.5, 10.0), (17.0, 10.0), (19.5, 13.5)]),
            p(&[(15.5, 10.0), (17.5, 6.5)]),
            rr(8.5, 15.0, 12.5, 3.2, 1.6),
            p(&[(2.5, 18.0), (7.5, 18.0)]),
            p(&[(4.8, 18.0), (4.8, 14.0)]),
        ],
        "jet-ski" => vec![
            pc(&[(2.5, 15.5), (19.0, 15.5), (21.5, 12.0), (13.0, 11.0), (6.0, 11.5)]),
            p(&[(8.0, 11.3), (10.5, 6.5)]),
            p(&[(9.2, 6.0), (12.0, 7.0)]),
            waves(),
        ],
        "sailboat" => vec![
            pc(&[(4.0, 16.0), (20.0, 16.0), (17.5, 20.0), (6.5, 20.0)]),
            p(&[(12.0, 3.0), (12.0, 16.0)]),
            pc(&[(12.0, 3.5), (12.0, 14.5), (5.0, 14.5)]),
            pc(&[(13.5, 5.5), (19.0, 14.5), (13.5, 14.5)]),
        ],
        "hot-air-balloon" => vec![
            pathc(vec![
                L(9.8, 15.5),
                Q(4.0, 12.0, 4.0, 8.0), Q(4.0, 2.5, 12.0, 2.5),
                Q(20.0, 2.5, 20.0, 8.0), Q(20.0, 12.0, 14.2, 15.5),
            ]),
            p(&[(12.0, 2.5), (12.0, 15.5)]),
            path(vec![L(9.8, 15.5), Q(7.0, 9.0, 9.0, 3.4)]),
            path(vec![L(14.2, 15.5), Q(17.0, 9.0, 15.0, 3.4)]),
            rr(9.5, 18.0, 5.0, 3.5, 0.5),
            p(&[(9.8, 15.5), (10.3, 18.0)]),
            p(&[(14.2, 15.5), (13.7, 18.0)]),
        ],
        "cable-car" => vec![
            p(&[(2.5, 4.0), (21.5, 6.0)]),
            p(&[(12.0, 5.0), (12.0, 9.0)]),
            rr(5.5, 9.0, 13.0, 9.5, 1.2),
            p(&[(5.5, 13.0), (18.5, 13.0)]),
            p(&[(12.0, 9.0), (12.0, 13.0)]),
        ],

        // ── Military ───────────────────────────────────────────────────────
        "tank" => vec![
            rr(3.0, 14.5, 18.0, 5.0, 2.5),
            d(6.5, 17.0, 1.3),
            d(10.2, 17.0, 1.3),
            d(13.8, 17.0, 1.3),
            d(17.5, 17.0, 1.3),
            pc(&[(3.5, 11.0), (20.5, 11.0), (19.5, 14.5), (4.5, 14.5)]),
            rr(8.0, 6.5, 7.0, 4.5, 1.0),
            p(&[(15.0, 8.5), (22.5, 8.5)]),
            p(&[(22.5, 7.6), (22.5, 9.4)]),
        ],
        "apc" => vec![
            pc(&[(4.0, 9.5), (16.5, 9.5), (21.0, 13.0), (20.5, 15.5), (3.5, 15.5), (2.8, 12.5)]),
            rr(9.0, 7.0, 4.0, 2.5, 0.6),
            c(6.0, 17.5, 2.0),
            c(11.0, 17.5, 2.0),
            c(16.0, 17.5, 2.0),
        ],
        "humvee" => vec![
            path(vec![
                L(2.5, 14.5), L(2.5, 11.0), L(9.0, 11.0), L(10.5, 7.5), L(17.0, 7.5),
                L(17.5, 11.0), L(21.5, 11.0), L(21.5, 14.5),
            ]),
            p(&[(2.5, 11.0), (2.5, 8.5), (9.5, 8.5)]),
            p(&[(13.5, 7.5), (13.5, 11.0)]),
            c(6.5, 16.0, 2.6),
            c(17.5, 16.0, 2.6),
        ],
        "submarine" => vec![
            ellipse(11.5, 13.0, 9.0, 3.6),
            rr(9.0, 6.5, 5.0, 4.5, 0.8),
            p(&[(11.5, 6.5), (11.5, 4.0)]),
            p(&[(11.5, 4.0), (13.5, 4.0)]),
            waves(),
        ],
        "warship" => vec![
            hull(),
            rr(9.0, 8.5, 6.0, 5.5, 0.6),
            p(&[(4.0, 9.5), (7.5, 11.0)]),
            rr(6.5, 11.0, 3.5, 3.0, 0.5),
            p(&[(12.0, 4.5), (12.0, 8.5)]),
            p(&[(10.5, 5.5), (13.5, 5.5)]),
            waves(),
        ],
        "aircraft-carrier" => vec![
            pc(&[(2.5, 12.0), (21.5, 12.0), (19.5, 17.0), (5.0, 17.0)]),
            rr(14.0, 7.0, 4.0, 5.0, 0.5),
            airplane_glyph(0.28, -4.0, -3.5),
            waves(),
        ],
        "patrol-boat" => vec![
            pc(&[(3.5, 14.0), (20.5, 14.0), (18.0, 18.5), (6.0, 18.5)]),
            rr(8.0, 10.0, 6.0, 4.0, 0.6),
            p(&[(10.5, 10.0), (10.5, 7.0)]),
            p(&[(16.0, 12.0), (19.5, 10.5)]),
            waves(),
        ],
        "fighter-jet" => vec![
            pc(&[
                (12.0, 2.5), (13.2, 7.5), (19.5, 13.5), (19.5, 15.5), (13.2, 13.0),
                (13.2, 16.0), (15.5, 18.5), (15.5, 20.0), (12.0, 18.8), (8.5, 20.0),
                (8.5, 18.5), (10.8, 16.0), (10.8, 13.0), (4.5, 15.5), (4.5, 13.5),
                (10.8, 7.5),
            ]),
            p(&[(12.0, 5.0), (12.0, 8.0)]),
        ],
        "bomber" => vec![pc(&[
            (12.0, 5.5), (22.0, 17.0), (15.0, 14.5), (12.0, 16.5), (9.0, 14.5), (2.0, 17.0),
        ])],
        "military-helicopter" => vec![
            ellipse(11.0, 13.5, 6.0, 3.0),
            p(&[(16.5, 13.0), (21.5, 11.5)]),
            p(&[(21.5, 9.5), (21.5, 13.5)]),
            p(&[(3.0, 6.5), (19.0, 6.5)]),
            p(&[(11.0, 10.5), (11.0, 6.5)]),
            p(&[(6.0, 18.5), (15.5, 18.5)]),
            p(&[(8.0, 16.4), (8.0, 18.5)]),
            p(&[(13.0, 16.4), (13.0, 18.5)]),
            p(&[(5.0, 13.5), (8.5, 13.5)]),
        ],
        "military-drone" => vec![
            rr(9.5, 10.5, 5.0, 4.0, 0.8),
            p(&[(9.5, 11.5), (4.5, 7.5)]),
            p(&[(14.5, 11.5), (19.5, 7.5)]),
            p(&[(2.0, 7.0), (7.0, 7.0)]),
            p(&[(17.0, 7.0), (22.0, 7.0)]),
            p(&[(4.5, 7.0), (4.5, 6.0)]),
            p(&[(19.5, 7.0), (19.5, 6.0)]),
            d(12.0, 16.5, 1.2),
        ],
        "rocket" => vec![
            pathc(vec![
                L(12.0, 2.5),
                Q(15.5, 6.0, 15.5, 11.0), L(15.5, 15.5), L(8.5, 15.5), L(8.5, 11.0),
                Q(8.5, 6.0, 12.0, 2.5),
            ]),
            c(12.0, 9.5, 1.7),
            path(vec![L(8.5, 12.5), Q(5.5, 14.5, 5.5, 18.0), L(8.5, 15.5)]),
            path(vec![L(15.5, 12.5), Q(18.5, 14.5, 18.5, 18.0), L(15.5, 15.5)]),
            path(vec![L(10.6, 17.3), Q(12.0, 21.5, 13.4, 17.3)]),
        ],
        "missile" => vec![
            pc(&[(2.5, 10.5), (16.0, 10.5), (21.5, 12.0), (16.0, 13.5), (2.5, 13.5)]),
            p(&[(4.5, 10.5), (2.5, 8.0)]),
            p(&[(4.5, 13.5), (2.5, 16.0)]),
            p(&[(10.0, 10.5), (8.5, 8.5)]),
        ],
        "torpedo" => vec![
            rr(3.0, 10.0, 15.0, 4.0, 2.0),
            p(&[(18.5, 10.0), (20.5, 14.0)]),
            p(&[(20.5, 10.0), (18.5, 14.0)]),
            p(&[(16.0, 10.0), (17.5, 8.0)]),
            p(&[(16.0, 14.0), (17.5, 16.0)]),
        ],
        "artillery" => vec![
            c(9.0, 15.5, 3.5),
            d(9.0, 15.5, 1.1),
            p(&[(10.0, 13.0), (20.2, 4.4)]),
            p(&[(12.1, 14.6), (22.0, 6.3)]),
            p(&[(20.2, 4.4), (22.0, 6.3)]),
            p(&[(9.0, 15.5), (3.0, 19.5)]),
            p(&[(9.0, 15.5), (15.0, 19.5)]),
        ],
        "mortar" => vec![
            pc(&[(8.5, 17.0), (15.0, 6.5), (18.0, 8.5), (11.5, 19.0)]),
            p(&[(6.5, 20.5), (15.5, 20.5)]),
            p(&[(10.5, 12.0), (5.5, 20.3)]),
        ],
        "bullet" => vec![
            path(vec![
                L(9.5, 9.0),
                Q(9.5, 4.5, 12.0, 3.0), Q(14.5, 4.5, 14.5, 9.0),
            ]),
            rr(9.5, 9.0, 5.0, 9.0, 0.4),
            rr(8.8, 18.5, 6.4, 2.0, 0.4),
        ],
        "ammo-box" => vec![
            rr(3.5, 8.0, 17.0, 11.0, 1.0),
            rr(2.8, 5.8, 18.4, 2.6, 0.5),
            p(&[(7.0, 12.0), (7.0, 16.0)]),
            p(&[(9.5, 12.0), (9.5, 16.0)]),
            p(&[(14.5, 12.0), (14.5, 16.0)]),
            p(&[(17.0, 12.0), (17.0, 16.0)]),
        ],
        "magazine-clip" => vec![
            pathc(vec![
                L(9.5, 3.5), L(15.5, 3.5),
                Q(16.5, 11.0, 12.0, 19.5), L(8.0, 17.0),
                Q(9.3, 10.5, 9.5, 5.0),
            ]),
            p(&[(9.5, 6.5), (15.8, 6.5)]),
            p(&[(9.3, 10.0), (15.4, 10.0)]),
        ],
        "rifle" => vec![
            p(&[(13.5, 9.5), (22.0, 9.5)]),
            p(&[(19.5, 8.0), (19.5, 9.5)]),
            rr(8.0, 9.0, 5.5, 3.5, 0.4),
            pc(&[(2.5, 10.5), (8.0, 9.5), (8.0, 12.0), (3.5, 13.5)]),
            p(&[(10.5, 12.5), (9.8, 15.5)]),
            pc(&[(11.5, 12.5), (14.0, 12.5), (13.3, 16.0), (11.0, 15.5)]),
        ],
        "pistol" => vec![
            rr(4.0, 8.0, 15.0, 3.5, 0.6),
            pc(&[(12.5, 11.5), (17.5, 11.5), (15.8, 18.5), (11.8, 18.0)]),
            a(11.0, 13.0, 1.8, -30.0, 170.0),
        ],
        "machine-gun" => vec![
            p(&[(12.5, 9.0), (22.0, 9.0)]),
            rr(6.0, 7.5, 6.5, 4.0, 0.5),
            p(&[(2.5, 9.0), (6.0, 9.0)]),
            rr(8.0, 11.5, 3.5, 4.0, 0.4),
            p(&[(17.0, 9.0), (15.0, 14.0)]),
            p(&[(17.0, 9.0), (19.0, 14.0)]),
        ],
        "sniper-rifle" => vec![
            p(&[(13.0, 10.5), (22.3, 10.5)]),
            rr(7.0, 9.5, 6.0, 3.0, 0.4),
            rr(8.5, 6.5, 5.0, 2.2, 1.1),
            p(&[(10.0, 8.7), (10.0, 9.5)]),
            p(&[(12.5, 8.7), (12.5, 9.5)]),
            pc(&[(2.5, 10.0), (7.0, 9.5), (7.0, 12.5), (3.5, 13.5)]),
            p(&[(10.0, 12.5), (9.3, 15.0)]),
            p(&[(17.0, 10.5), (15.5, 14.5)]),
            p(&[(17.0, 10.5), (18.5, 14.5)]),
        ],
        "grenade" => vec![
            rr(8.5, 9.0, 7.0, 10.5, 2.5),
            p(&[(8.5, 12.5), (15.5, 12.5)]),
            p(&[(8.5, 16.0), (15.5, 16.0)]),
            p(&[(12.0, 9.0), (12.0, 19.5)]),
            rr(10.5, 6.5, 3.0, 2.5, 0.4),
            path(vec![L(13.5, 6.5), Q(17.0, 5.5, 18.5, 8.0)]),
            c(16.5, 4.5, 1.7),
        ],
        "landmine" => vec![
            a(12.0, 14.0, 8.0, 180.0, 360.0),
            p(&[(4.0, 14.0), (4.0, 16.0)]),
            p(&[(20.0, 14.0), (20.0, 16.0)]),
            p(&[(4.0, 16.0), (20.0, 16.0)]),
            rr(9.5, 9.5, 5.0, 2.0, 0.5),
        ],
        "military-shield" => vec![
            mil_shield(),
            pc(&[
                (12.0, 8.0), (12.9, 9.9), (15.0, 10.1), (13.4, 11.5), (13.9, 13.6),
                (12.0, 12.4), (10.1, 13.6), (10.6, 11.5), (9.0, 10.1), (11.1, 9.9),
            ]),
        ],
        "armor-vest" => vec![
            pathc(vec![
                L(7.0, 4.5), L(9.5, 4.5),
                Q(9.5, 7.0, 12.0, 7.0), Q(14.5, 7.0, 14.5, 4.5), L(17.0, 4.5),
                L(19.0, 7.5), L(19.0, 18.0),
                Q(19.0, 19.5, 17.5, 19.5), L(6.5, 19.5),
                Q(5.0, 19.5, 5.0, 18.0), L(5.0, 7.5),
            ]),
            p(&[(5.0, 12.0), (19.0, 12.0)]),
            p(&[(8.5, 14.5), (15.5, 14.5)]),
            p(&[(8.5, 17.0), (15.5, 17.0)]),
        ],
        "armor-plate" => vec![
            pc(&[
                (6.0, 3.5), (18.0, 3.5), (20.0, 6.0), (20.0, 17.5),
                (18.0, 20.5), (6.0, 20.5), (4.0, 17.5), (4.0, 6.0),
            ]),
            p(&[(9.5, 11.0), (14.5, 11.0)]),
            p(&[(10.5, 14.0), (13.5, 14.0)]),
        ],
        "combat-helmet" => vec![
            a(12.0, 13.0, 8.0, 180.0, 360.0),
            p(&[(3.5, 13.0), (20.5, 13.0)]),
            path(vec![L(6.5, 14.8), Q(12.0, 19.5, 17.5, 14.8)]),
        ],
        "uniform-jacket" => vec![
            pathc(vec![
                L(6.5, 5.0), L(10.0, 5.0), L(12.0, 8.0), L(14.0, 5.0), L(17.5, 5.0),
                L(19.5, 8.0), L(19.5, 20.0), L(4.5, 20.0), L(4.5, 8.0),
            ]),
            p(&[(10.0, 5.0), (9.5, 9.5), (12.0, 8.0)]),
            p(&[(14.0, 5.0), (14.5, 9.5), (12.0, 8.0)]),
            p(&[(12.0, 8.0), (12.0, 20.0)]),
            p(&[(6.5, 15.0), (9.0, 15.0)]),
            p(&[(15.0, 15.0), (17.5, 15.0)]),
        ],
        "uniform-pants" => vec![
            pathc(vec![
                L(6.5, 3.5), L(17.5, 3.5), L(18.5, 20.5), L(14.0, 20.5),
                L(12.0, 10.5), L(10.0, 20.5), L(5.5, 20.5),
            ]),
            p(&[(6.5, 6.0), (17.5, 6.0)]),
            rr(6.9, 11.5, 3.0, 3.2, 0.4),
        ],
        "combat-boots" => vec![
            pathc(vec![
                L(7.5, 3.5), L(13.5, 3.5), L(13.5, 12.5),
                Q(17.0, 13.0, 19.5, 15.5),
                Q(21.0, 17.0, 20.5, 19.5), L(7.5, 19.5),
            ]),
            p(&[(8.5, 6.0), (12.5, 6.0)]),
            p(&[(8.5, 8.5), (12.5, 8.5)]),
            p(&[(8.5, 11.0), (12.5, 11.0)]),
            p(&[(7.5, 17.5), (20.5, 17.5)]),
        ],
        "dog-tags" => vec![
            a(12.0, 7.0, 4.5, 160.0, 380.0),
            rr(8.0, 10.0, 4.5, 8.0, 2.0),
            rr(12.5, 11.5, 4.5, 8.0, 2.0),
            d(10.2, 11.8, 0.6),
            d(14.7, 13.3, 0.6),
        ],
        "medal" => vec![
            pc(&[(9.0, 3.5), (15.0, 3.5), (15.0, 9.0), (12.0, 11.5), (9.0, 9.0)]),
            c(12.0, 15.5, 4.2),
            pc(&[
                (12.0, 13.2), (12.7, 14.7), (14.3, 14.9), (13.1, 16.0), (13.4, 17.6),
                (12.0, 16.7), (10.6, 17.6), (10.9, 16.0), (9.7, 14.9), (11.3, 14.7),
            ]),
        ],
        "rank-chevrons" => vec![
            p(&[(5.0, 9.0), (12.0, 4.0), (19.0, 9.0)]),
            p(&[(5.0, 13.5), (12.0, 8.5), (19.0, 13.5)]),
            p(&[(5.0, 18.0), (12.0, 13.0), (19.0, 18.0)]),
        ],
        "radar" => vec![
            c(12.0, 12.0, 8.5),
            p(&[(12.0, 12.0), (18.0, 5.5)]),
            d(8.5, 9.5, 1.0),
            d(15.0, 15.5, 1.0),
            p(&[(12.0, 3.5), (12.0, 6.0)]),
            p(&[(12.0, 18.0), (12.0, 20.5)]),
            p(&[(3.5, 12.0), (6.0, 12.0)]),
            p(&[(18.0, 12.0), (20.5, 12.0)]),
        ],
        "periscope" => vec![
            path(vec![
                L(9.5, 20.5), L(9.5, 6.5),
                Q(9.5, 4.0, 12.0, 4.0), L(17.0, 4.0), L(17.0, 8.5), L(14.5, 8.5),
                L(14.5, 20.5),
            ]),
            p(&[(9.5, 17.0), (14.5, 17.0)]),
            p(&[(16.9, 4.8), (16.9, 7.6)]),
        ],
        "night-vision" => vec![
            c(8.0, 12.0, 3.8),
            c(16.0, 12.0, 3.8),
            d(8.0, 12.0, 1.4),
            d(16.0, 12.0, 1.4),
            p(&[(10.5, 8.5), (13.5, 8.5)]),
            p(&[(12.0, 8.5), (12.0, 6.0)]),
            p(&[(4.2, 12.0), (2.5, 12.0)]),
            p(&[(19.8, 12.0), (21.5, 12.0)]),
        ],
        "parachute" => vec![
            a(12.0, 10.0, 8.5, 180.0, 360.0),
            a(6.2, 10.0, 2.7, 0.0, 180.0),
            a(12.0, 10.0, 2.9, 0.0, 180.0),
            a(17.8, 10.0, 2.7, 0.0, 180.0),
            p(&[(3.5, 10.0), (11.0, 17.5)]),
            p(&[(20.5, 10.0), (13.0, 17.5)]),
            d(12.0, 18.5, 1.4),
        ],
        "bunker" => vec![
            a(12.0, 17.0, 9.0, 180.0, 360.0),
            p(&[(2.5, 17.0), (21.5, 17.0)]),
            rr(8.5, 12.5, 7.0, 2.2, 0.5),
            d(12.0, 9.8, 0.8),
        ],

        _ => return None,
    })
}

// ── Flattening ──────────────────────────────────────────────────────────────

/// Flatten a path into grid-space points, plus the authored vertex list
/// (op endpoints) where the renderer plants round-cap dots.
fn flatten_path(ops: &[PathOp], closed: bool) -> (Vec<(f32, f32)>, Vec<(f32, f32)>) {
    let mut pts: Vec<(f32, f32)> = Vec::new();
    let mut verts: Vec<(f32, f32)> = Vec::new();
    for op in ops {
        let cur = *pts.last().unwrap_or(&(0.0, 0.0));
        match *op {
            L(x, y) => {
                pts.push((x, y));
                verts.push((x, y));
            }
            Q(cx, cy, x, y) => {
                if pts.is_empty() {
                    pts.push((x, y));
                    verts.push((x, y));
                    continue;
                }
                const N: usize = 10;
                for i in 1..=N {
                    let t = i as f32 / N as f32;
                    let mt = 1.0 - t;
                    let px = mt * mt * cur.0 + 2.0 * mt * t * cx + t * t * x;
                    let py = mt * mt * cur.1 + 2.0 * mt * t * cy + t * t * y;
                    pts.push((px, py));
                }
                verts.push((x, y));
            }
            B(c1x, c1y, c2x, c2y, x, y) => {
                if pts.is_empty() {
                    pts.push((x, y));
                    verts.push((x, y));
                    continue;
                }
                const N: usize = 14;
                for i in 1..=N {
                    let t = i as f32 / N as f32;
                    let mt = 1.0 - t;
                    let px = mt * mt * mt * cur.0
                        + 3.0 * mt * mt * t * c1x
                        + 3.0 * mt * t * t * c2x
                        + t * t * t * x;
                    let py = mt * mt * mt * cur.1
                        + 3.0 * mt * mt * t * c1y
                        + 3.0 * mt * t * t * c2y
                        + t * t * t * y;
                    pts.push((px, py));
                }
                verts.push((x, y));
            }
        }
    }
    if closed && pts.len() > 2 {
        // Closing segment is implicit in closed_line; vertices already carry
        // caps at both ends of it.
    }
    (pts, verts)
}

/// Flatten an arc into grid-space points (start/end are its cap vertices).
fn flatten_arc(cx: f32, cy: f32, r: f32, a0: f32, a1: f32) -> Vec<(f32, f32)> {
    let sweep = (a1 - a0).abs();
    let steps = ((sweep / 10.0).ceil() as usize).max(4);
    (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32;
            let ang = (a0 + (a1 - a0) * t).to_radians();
            (cx + r * ang.cos(), cy + r * ang.sin())
        })
        .collect()
}

/// A rounded rect as a closed grid-space polyline.
fn flatten_rrect(x: f32, y: f32, w: f32, h: f32, rad: f32) -> Vec<(f32, f32)> {
    let r = rad.min(w * 0.5).min(h * 0.5).max(0.0);
    if r <= 0.05 {
        return vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
    }
    let mut pts = Vec::new();
    let corners = [
        (x + w - r, y + r, 270.0, 360.0),
        (x + w - r, y + h - r, 0.0, 90.0),
        (x + r, y + h - r, 90.0, 180.0),
        (x + r, y + r, 180.0, 270.0),
    ];
    for &(ccx, ccy, s, e) in &corners {
        for i in 0..=4 {
            let ang = (s + (e - s) * i as f32 / 4.0).to_radians();
            pts.push((ccx + r * ang.cos(), ccy + r * ang.sin()));
        }
    }
    pts
}

// ── egui renderer ───────────────────────────────────────────────────────────

/// A rendering effect applied under the icon's line work.
#[cfg(feature = "render")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IconEffect {
    /// Line work only.
    Plain,
    /// A soft drop shadow beneath the glyph (two feathered passes).
    DropShadow {
        color: Color32,
        /// Offset in grid units (of 24); ~0.8 reads well.
        offset: f32,
    },
    /// Neumorphic emboss: a light pass toward the top-left and a dark pass
    /// toward the bottom-right, matching the IDE's Neumorphic surface style.
    Neumorphic {
        light: Color32,
        dark: Color32,
        /// Offset in grid units (of 24); ~0.7 reads well.
        offset: f32,
    },
}

/// How to paint an icon: tint, optional accent, optional effect. The icon
/// stays a pure vector — effects are extra render passes, never a raster.
#[cfg(feature = "render")]
#[derive(Clone, Copy, Debug)]
pub struct IconStyle {
    /// Line-work colour.
    pub color: Color32,
    /// Colour for the FILLED accent shapes (dots, filled paths/rects);
    /// `None` paints them in `color` (the classic single-tint look).
    pub accent: Option<Color32>,
    pub effect: IconEffect,
}

#[cfg(feature = "render")]
impl IconStyle {
    /// Single-tint, no effect — the default menu look.
    pub fn tint(color: Color32) -> Self {
        Self { color, accent: None, effect: IconEffect::Plain }
    }

    /// A ready-made drop-shadow style over the given tint.
    pub fn shadowed(color: Color32) -> Self {
        Self {
            color,
            accent: None,
            effect: IconEffect::DropShadow {
                color: Color32::from_black_alpha(70),
                offset: 0.8,
            },
        }
    }

    /// A ready-made neumorphic style for light chrome (soft white above,
    /// blue-grey below — the classic extruded look).
    pub fn neumorphic(color: Color32) -> Self {
        Self {
            color,
            accent: None,
            effect: IconEffect::Neumorphic {
                light: Color32::from_rgba_unmultiplied(255, 255, 255, 170),
                dark: Color32::from_rgba_unmultiplied(120, 135, 160, 110),
                offset: 0.7,
            },
        }
    }
}

/// One paint pass of every shape, with the grid origin shifted by `shift`
/// grid units (effects pass a diagonal shift; the main pass passes zero).
#[cfg(feature = "render")]
fn paint_pass(
    painter: &egui::Painter,
    shapes: &[IconShape],
    origin: Pos2,
    k: f32,
    shift: Vec2,
    stroke_color: Color32,
    fill_color: Color32,
) {
    let m = |(x, y): (f32, f32)| {
        Pos2::new(origin.x + (x + shift.x) * k, origin.y + (y + shift.y) * k)
    };
    let w = (STROKE_UNITS * k).max(0.95);
    let stroke = Stroke::new(w, stroke_color);
    let cap = w * 0.5;

    for s in shapes {
        match s {
            IconShape::Stroke(ops) => {
                let (pts, verts) = flatten_path(ops, false);
                if pts.len() >= 2 {
                    painter.add(egui::Shape::line(pts.iter().map(|&pt| m(pt)).collect(), stroke));
                }
                for &v in &verts {
                    painter.circle_filled(m(v), cap, stroke_color);
                }
            }
            IconShape::StrokeClosed(ops) => {
                let (pts, verts) = flatten_path(ops, true);
                if pts.len() >= 3 {
                    painter.add(egui::Shape::closed_line(
                        pts.iter().map(|&pt| m(pt)).collect(),
                        stroke,
                    ));
                }
                for &v in &verts {
                    painter.circle_filled(m(v), cap, stroke_color);
                }
            }
            IconShape::FillPath(ops) => {
                let (pts, _) = flatten_path(ops, true);
                if pts.len() >= 3 {
                    painter.add(egui::Shape::convex_polygon(
                        pts.iter().map(|&pt| m(pt)).collect(),
                        fill_color,
                        Stroke::new(w * 0.6, fill_color),
                    ));
                }
            }
            IconShape::Circle(cx, cy, r) => {
                painter.circle_stroke(m((*cx, *cy)), r * k, stroke);
            }
            IconShape::Dot(cx, cy, r) => {
                painter.circle_filled(m((*cx, *cy)), r * k, fill_color);
            }
            IconShape::Arc(cx, cy, r, a0, a1) => {
                let pts = flatten_arc(*cx, *cy, *r, *a0, *a1);
                painter.add(egui::Shape::line(pts.iter().map(|&pt| m(pt)).collect(), stroke));
                if let (Some(&first), Some(&last)) = (pts.first(), pts.last()) {
                    painter.circle_filled(m(first), cap, stroke_color);
                    painter.circle_filled(m(last), cap, stroke_color);
                }
            }
            IconShape::RRect(x, y, wd, h, rad) => {
                let pts = flatten_rrect(*x, *y, *wd, *h, *rad);
                painter.add(egui::Shape::closed_line(
                    pts.iter().map(|&pt| m(pt)).collect(),
                    stroke,
                ));
            }
            IconShape::RRectFill(x, y, wd, h, rad) => {
                let pts = flatten_rrect(*x, *y, *wd, *h, *rad);
                painter.add(egui::Shape::convex_polygon(
                    pts.iter().map(|&pt| m(pt)).collect(),
                    fill_color,
                    Stroke::NONE,
                ));
            }
        }
    }
}

/// Draw the named icon into `rect` with full styling — any size (the vector
/// grid maps to the rect; 16 px row or 128 px tile alike), any colours, and
/// an optional shadow/neumorphic effect. Unknown names draw nothing (a menu
/// with a stale icon name degrades to no icon, not a panic).
#[cfg(feature = "render")]
pub fn draw_menu_icon_styled(painter: &egui::Painter, rect: Rect, name: &str, style: &IconStyle) {
    let Some(shapes) = icon_shapes(name) else {
        return;
    };
    let size = rect.width().min(rect.height());
    if size <= 1.0 {
        return;
    }
    let k = size / 24.0;
    let origin = rect.center() - Vec2::splat(size * 0.5);

    match style.effect {
        IconEffect::Plain => {}
        IconEffect::DropShadow { color, offset } => {
            // Two feathered passes fake a blur without one.
            let half = Color32::from_rgba_unmultiplied(
                color.r(),
                color.g(),
                color.b(),
                (color.a() as f32 * 0.55) as u8,
            );
            paint_pass(painter, &shapes, origin, k, Vec2::splat(offset * 1.45), half, half);
            paint_pass(painter, &shapes, origin, k, Vec2::splat(offset), color, color);
        }
        IconEffect::Neumorphic { light, dark, offset } => {
            paint_pass(painter, &shapes, origin, k, Vec2::splat(-offset), light, light);
            paint_pass(painter, &shapes, origin, k, Vec2::splat(offset), dark, dark);
        }
    }
    let fill = style.accent.unwrap_or(style.color);
    paint_pass(painter, &shapes, origin, k, Vec2::ZERO, style.color, fill);
}

/// Draw the named icon single-tinted with no effect (the plain menu look).
#[cfg(feature = "render")]
pub fn draw_menu_icon(painter: &egui::Painter, rect: Rect, name: &str, color: Color32) {
    draw_menu_icon_styled(painter, rect, name, &IconStyle::tint(color));
}

/// The style for a control's `IconEffect` property value (`None` | `Shadow` |
/// `Neumorphic`, case-insensitive; anything else is plain).
#[cfg(feature = "render")]
pub fn icon_style_for_effect(effect: &str, color: Color32) -> IconStyle {
    if effect.eq_ignore_ascii_case("shadow") {
        IconStyle::shadowed(color)
    } else if effect.eq_ignore_ascii_case("neumorphic") {
        IconStyle::neumorphic(color)
    } else {
        IconStyle::tint(color)
    }
}

// ── SVG emitter ─────────────────────────────────────────────────────────────
// Feature-free: powers the contact-sheet example (visual QA) and any future
// documentation tooling. Produces the same geometry the egui renderer paints.

fn svg_path_data(ops: &[PathOp], closed: bool) -> String {
    let mut out = String::new();
    for (i, op) in ops.iter().enumerate() {
        match *op {
            L(x, y) => {
                if i == 0 {
                    out.push_str(&format!("M{x:.2} {y:.2}"));
                } else {
                    out.push_str(&format!(" L{x:.2} {y:.2}"));
                }
            }
            Q(cx, cy, x, y) => {
                if i == 0 {
                    out.push_str(&format!("M{x:.2} {y:.2}"));
                } else {
                    out.push_str(&format!(" Q{cx:.2} {cy:.2} {x:.2} {y:.2}"));
                }
            }
            B(c1x, c1y, c2x, c2y, x, y) => {
                if i == 0 {
                    out.push_str(&format!("M{x:.2} {y:.2}"));
                } else {
                    out.push_str(&format!(" C{c1x:.2} {c1y:.2} {c2x:.2} {c2y:.2} {x:.2} {y:.2}"));
                }
            }
        }
    }
    if closed {
        out.push_str(" Z");
    }
    out
}

/// An effect for SVG export, mirroring the egui [`IconEffect`] but with CSS
/// colours (feature-free — usable by tooling without egui).
pub enum SvgIconEffect<'a> {
    Plain,
    /// `feDropShadow` filter.
    DropShadow { color: &'a str, opacity: f32, offset: f32, blur: f32 },
    /// Two opposing `feDropShadow`s — the neumorphic emboss.
    Neumorphic {
        light: &'a str,
        dark: &'a str,
        offset: f32,
        blur: f32,
    },
}

/// The named icon as a standalone styled `<svg>` element. The viewBox is the
/// 24-unit reference grid; being a vector, it renders at ANY size (`width`/
/// `height` are whatever the embedder chooses — 128×128 included). `accent`
/// colours the filled accent shapes; `None` for unknown names.
pub fn icon_svg_styled(
    name: &str,
    color: &str,
    accent: Option<&str>,
    effect: &SvgIconEffect<'_>,
) -> Option<String> {
    let inner = icon_svg_body(name, color, accent)?;
    let svg = match effect {
        SvgIconEffect::Plain => format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">{inner}</svg>"#
        ),
        SvgIconEffect::DropShadow { color: sc, opacity, offset, blur } => format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><filter id="ds" x="-30%" y="-30%" width="160%" height="160%"><feDropShadow dx="{offset}" dy="{offset}" stdDeviation="{blur}" flood-color="{sc}" flood-opacity="{opacity}"/></filter><g filter="url(#ds)">{inner}</g></svg>"#
        ),
        SvgIconEffect::Neumorphic { light, dark, offset, blur } => format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><filter id="nm" x="-30%" y="-30%" width="160%" height="160%"><feDropShadow dx="-{offset}" dy="-{offset}" stdDeviation="{blur}" flood-color="{light}" flood-opacity="0.9"/><feDropShadow dx="{offset}" dy="{offset}" stdDeviation="{blur}" flood-color="{dark}" flood-opacity="0.6"/></filter><g filter="url(#nm)">{inner}</g></svg>"#
        ),
    };
    Some(svg)
}

/// The named icon as a standalone single-tint `<svg>` element.
pub fn icon_svg(name: &str, color: &str) -> Option<String> {
    icon_svg_styled(name, color, None, &SvgIconEffect::Plain)
}

/// The icon's shape elements (no `<svg>` wrapper).
fn icon_svg_body(name: &str, color: &str, accent: Option<&str>) -> Option<String> {
    let shapes = icon_shapes(name)?;
    let fill_color = accent.unwrap_or(color);
    let mut body = String::new();
    let sw = STROKE_UNITS;
    for s in &shapes {
        match s {
            IconShape::Stroke(ops) => {
                body.push_str(&format!(
                    r#"<path d="{}" fill="none" stroke="{color}" stroke-width="{sw}" stroke-linecap="round" stroke-linejoin="round"/>"#,
                    svg_path_data(ops, false)
                ));
            }
            IconShape::StrokeClosed(ops) => {
                body.push_str(&format!(
                    r#"<path d="{}" fill="none" stroke="{color}" stroke-width="{sw}" stroke-linecap="round" stroke-linejoin="round"/>"#,
                    svg_path_data(ops, true)
                ));
            }
            IconShape::FillPath(ops) => {
                body.push_str(&format!(
                    r#"<path d="{}" fill="{fill_color}" stroke="{fill_color}" stroke-width="{:.2}" stroke-linejoin="round"/>"#,
                    svg_path_data(ops, true),
                    sw * 0.6,
                ));
            }
            IconShape::Circle(cx, cy, r) => {
                body.push_str(&format!(
                    r#"<circle cx="{cx:.2}" cy="{cy:.2}" r="{r:.2}" fill="none" stroke="{color}" stroke-width="{sw}"/>"#
                ));
            }
            IconShape::Dot(cx, cy, r) => {
                body.push_str(&format!(
                    r#"<circle cx="{cx:.2}" cy="{cy:.2}" r="{r:.2}" fill="{fill_color}"/>"#
                ));
            }
            IconShape::Arc(cx, cy, r, a0, a1) => {
                let pts = flatten_arc(*cx, *cy, *r, *a0, *a1);
                let mut dstr = String::new();
                for (i, (x, y)) in pts.iter().enumerate() {
                    dstr.push_str(&format!(
                        "{}{x:.2} {y:.2}",
                        if i == 0 { "M" } else { " L" }
                    ));
                }
                body.push_str(&format!(
                    r#"<path d="{dstr}" fill="none" stroke="{color}" stroke-width="{sw}" stroke-linecap="round" stroke-linejoin="round"/>"#
                ));
            }
            IconShape::RRect(x, y, w, h, rad) => {
                body.push_str(&format!(
                    r#"<rect x="{x:.2}" y="{y:.2}" width="{w:.2}" height="{h:.2}" rx="{rad:.2}" fill="none" stroke="{color}" stroke-width="{sw}"/>"#
                ));
            }
            IconShape::RRectFill(x, y, w, h, rad) => {
                body.push_str(&format!(
                    r#"<rect x="{x:.2}" y="{y:.2}" width="{w:.2}" height="{h:.2}" rx="{rad:.2}" fill="{fill_color}"/>"#
                ));
            }
        }
    }
    Some(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every catalogue name must have a drawing, names must be unique, and
    /// every shape must stay on the 24×24 grid.
    #[test]
    fn catalogue_names_are_unique_and_drawable() {
        let mut seen = std::collections::HashSet::new();
        let mut missing: Vec<&str> = Vec::new();
        let mut total = 0usize;
        for (cat, names) in MENU_ICON_CATEGORIES {
            for name in *names {
                total += 1;
                assert!(seen.insert(*name), "duplicate icon name: {name} (in {cat})");
                if icon_shapes(name).map(|v| v.is_empty()).unwrap_or(true) {
                    missing.push(name);
                }
            }
        }
        assert!(
            missing.is_empty(),
            "{} of {total} icons have no drawing yet: {missing:?}",
            missing.len()
        );
    }

    /// All geometry stays on the 24×24 grid (with half a stroke of slack).
    #[test]
    fn all_shapes_stay_on_the_grid() {
        let ok = |v: f32| (-0.8..=24.8).contains(&v);
        let mut checked = 0usize;
        for name in menu_icon_names() {
            let Some(shapes) = icon_shapes(name) else { continue };
            for s in &shapes {
                let pts: Vec<(f32, f32)> = match s {
                    IconShape::Stroke(ops) | IconShape::StrokeClosed(ops)
                    | IconShape::FillPath(ops) => flatten_path(ops, false).0,
                    IconShape::Circle(cx, cy, r) | IconShape::Dot(cx, cy, r) => {
                        vec![(cx - r, cy - r), (cx + r, cy + r)]
                    }
                    IconShape::Arc(cx, cy, r, a0, a1) => flatten_arc(*cx, *cy, *r, *a0, *a1),
                    IconShape::RRect(x, y, w, h, _) | IconShape::RRectFill(x, y, w, h, _) => {
                        vec![(*x, *y), (x + w, y + h)]
                    }
                };
                for (x, y) in pts {
                    assert!(
                        ok(x) && ok(y),
                        "icon '{name}' leaves the grid at ({x:.1}, {y:.1})"
                    );
                }
                checked += 1;
            }
        }
        eprintln!("icon grid check — {checked} shapes verified on the 24×24 grid");
    }

    /// Every icon paints through the real egui renderer without panicking, at
    /// a menu-row 16 px and a tile 128 px, in plain, accent, drop-shadow and
    /// neumorphic styles — the icon is a vector, the size is the caller's.
    #[cfg(feature = "render")]
    #[test]
    fn every_icon_renders_headlessly_in_every_style() {
        let ctx = egui::Context::default();
        let styles = [
            IconStyle::tint(Color32::WHITE),
            IconStyle {
                color: Color32::from_rgb(30, 100, 220),
                accent: Some(Color32::from_rgb(220, 120, 30)),
                effect: IconEffect::Plain,
            },
            IconStyle::shadowed(Color32::BLACK),
            IconStyle::neumorphic(Color32::from_rgb(90, 100, 120)),
        ];
        let mut painted = 0usize;
        for (si, style) in styles.iter().enumerate() {
            let mut full = ctx.run_ui(egui::RawInput::default(), |ui| {
                let painter = ui.painter();
                for (i, name) in menu_icon_names().enumerate() {
                    let size = if i % 2 == 0 { 16.0 } else { 128.0 };
                    let rect = Rect::from_min_size(
                        Pos2::new((i % 16) as f32 * 130.0, (i / 16) as f32 * 130.0),
                        Vec2::splat(size),
                    );
                    draw_menu_icon_styled(painter, rect, name, style);
                    painted += 1;
                }
            });
            full.textures_delta.clear();
            let _ = si;
        }
        eprintln!(
            "icon render check — {painted} paints ({} icons x {} styles, \
             alternating 16 px and 128 px), no panics",
            painted / styles.len(),
            styles.len()
        );
    }

    /// Catalogue size: the redesign doubles the historical 322-name set.
    #[test]
    fn catalogue_size_doubles_the_baseline() {
        let total: usize = MENU_ICON_CATEGORIES.iter().map(|(_, n)| n.len()).sum();
        let cats = MENU_ICON_CATEGORIES.len();
        assert!(
            total >= 644,
            "catalogue must at least double the 322-icon baseline, got {total}"
        );
        eprintln!(
            "icon catalogue — {total} icons across {cats} categories \
             ({}x the 322-icon baseline)",
            (total as f32 / 322.0 * 100.0).round() / 100.0
        );
    }
}
