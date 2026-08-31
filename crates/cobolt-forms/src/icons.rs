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
            "sidebar-expand",
            "sidebar-collapse",
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
    (
        "Selection",
        &[
            "select-item",
            "select-all",
            "select-invert",
            "select-none",
            "lasso",
            "move",
            "arrows",
        ],
    ),
    (
        "Design",
        &[
            "paint-bucket",
            "fill",
            "color-palette",
            "object",
            "equation",
            "rotate-left",
            "rotate-right",
            "flip-horizontal",
            "flip-vertical",
            "fit-to-window",
            "thumbnails",
        ],
    ),
    (
        "Application",
        &[
            "window",
            "form",
            "application",
            "bundle",
            "component",
            "find-replace",
            "spelling",
            "speech",
            "volume-up",
            "volume-down",
            "sleep",
            "quit",
            "globe",
            "local",
        ],
    ),
    // Every control the RAD toolbox offers, one icon each, named
    // `control-<kebab of the ControlType>`. The IDE drew these for its own
    // toolbox in ad-hoc painter calls; recreated here on the 24-unit grid so a
    // developer can put a control's own icon on a menu item, a toolbar button
    // or a tree node in the app they are building (operator, 2026-08-31: "I
    // was creating examples for each control and I realized we do not have
    // icons to represent PowerRustCOBOL own controls"). The prefix keeps them
    // clear of the generic names — `form`, `window`, `table`, `toggle-on` —
    // that already mean something else in this catalogue.
    (
        "PowerRustCOBOL Controls",
        &[
            "control-button",
            "control-text-box",
            "control-label",
            "control-check-box",
            "control-radio-button",
            "control-list-box",
            "control-combo-box",
            "control-group-box",
            "control-panel",
            "control-tab-control",
            "control-data-grid",
            "control-picture-box",
            "control-progress-bar",
            "control-menu-bar",
            "control-tool-bar",
            "control-status-bar",
            "control-line",
            "control-date-time-picker",
            "control-numeric-up-down",
            "control-tree-view",
            "control-splitter",
            "control-timer",
            "control-shape",
            "control-animator",
            "control-agent-object",
            "control-rest-client",
            "control-sql-database",
            "control-indexed-file",
            "control-slider",
            "control-bar-chart",
            "control-line-chart",
            "control-pie-chart",
            "control-area-chart",
            "control-scatter-chart",
            "control-donut-chart",
            "control-knob",
            "control-gauge",
            "control-switch",
            "control-file-drop-zone",
            "control-maps",
            "control-web-search",
            "control-side-menu",
            "control-custom",
        ],
    ),
    // The vocabulary of the craft: data structures, compilation, concurrency,
    // networking, security, data modelling, version control and the handful of
    // theory ideas a developer actually draws on a whiteboard. Requested
    // alongside the control set (operator, 2026-08-31); names avoid every
    // generic already here — `code`, `database`, `network`, `api`, `terminal`,
    // `bug`, `server` and `container` (which is a SHIPPING container, in
    // Logistics) all keep their existing meanings.
    (
        "Computer Science",
        &[
            "array", "matrix", "stack-structure", "queue-structure",
            "linked-list", "hash-table", "binary-tree", "graph-nodes", "venn",
            "ring-buffer",
            "algorithm", "compiler", "interpreter", "parser", "syntax-tree",
            "bytecode", "variable", "constant", "recursion", "iteration",
            "conditional", "boolean", "regex", "flowchart",
            "thread", "process", "mutex", "deadlock", "garbage-collection",
            "stack-trace", "breakpoint", "kernel", "virtual-machine", "sandbox",
            "async", "callback", "event-loop", "scheduler",
            "socket", "protocol", "packet", "load-balancer", "proxy",
            "firewall", "microservice", "websocket", "webhook",
            "encryption", "hash-function", "checksum", "key-pair", "two-factor",
            "schema", "primary-key", "foreign-key", "join-tables", "migration",
            "replication", "sharding", "query",
            "git-branch", "git-merge", "git-commit", "pull-request",
            "repository", "diff", "patch", "ci-cd", "container-image",
            "orchestration",
            "complexity", "sorting", "binary-search", "state-machine",
            "truth-table", "bitwise", "binary-number", "hexadecimal",
            "neural-network",
        ],
    ),
    // Interface patterns, interaction and layout — the words a designer and a
    // developer use to each other. Requested alongside the control set
    // (operator, 2026-08-31). Deliberately NOT included, because the
    // catalogue already answers them: `loading` (a spinner), `user-circle`
    // (an avatar), `magnifier`, `upload`, `zoom-in` / `zoom-out`, `undo` /
    // `redo` and the whole `align-*` family.
    (
        "User Interface",
        &[
            "modal", "dialog", "tooltip", "popover", "dropdown", "accordion",
            "breadcrumb", "pagination", "stepper", "wizard", "carousel",
            "drawer", "toast", "chip", "skeleton", "scrollbar", "search-field",
            "file-upload", "empty-state",
            "wireframe", "mockup", "responsive", "dark-mode", "light-mode",
            "accessibility", "screen-reader", "keyboard-shortcut",
            "cursor-pointer", "cursor-text", "drag-drop", "click",
            "double-click", "long-press", "swipe", "focus-ring",
            "z-index", "flex-layout", "grid-layout", "padding", "margin",
            "border-radius", "drop-shadow", "opacity", "gradient", "guides",
            "ruler", "artboard", "viewport", "snap-grid",
        ],
    ),
    // National flags, `flag-<ISO 3166-1 alpha-2>`: every UN member state, plus
    // the Holy See, Palestine and Kosovo. These are LINE drawings on a
    // monochrome grid, so flags that differ only in colour share a drawing —
    // see the note above `flag_field` for the operator decision behind that.
    (
        "Flags",
        &[
            "flag-ad", "flag-ae", "flag-af", "flag-ag", "flag-al", "flag-am",
            "flag-ao", "flag-ar", "flag-at", "flag-au", "flag-az", "flag-ba",
            "flag-bb", "flag-bd", "flag-be", "flag-bf", "flag-bg", "flag-bh",
            "flag-bi", "flag-bj", "flag-bn", "flag-bo", "flag-br", "flag-bs",
            "flag-bt", "flag-bw", "flag-by", "flag-bz", "flag-ca", "flag-cd",
            "flag-cf", "flag-cg", "flag-ch", "flag-ci", "flag-cl", "flag-cm",
            "flag-cn", "flag-co", "flag-cr", "flag-cu", "flag-cv", "flag-cy",
            "flag-cz", "flag-de", "flag-dj", "flag-dk", "flag-dm", "flag-do",
            "flag-dz", "flag-ec", "flag-ee", "flag-eg", "flag-er", "flag-es",
            "flag-et", "flag-fi", "flag-fj", "flag-fm", "flag-fr", "flag-ga",
            "flag-gb", "flag-gd", "flag-ge", "flag-gh", "flag-gm", "flag-gn",
            "flag-gq", "flag-gr", "flag-gt", "flag-gw", "flag-gy", "flag-hn",
            "flag-hr", "flag-ht", "flag-hu", "flag-id", "flag-ie", "flag-il",
            "flag-in", "flag-iq", "flag-ir", "flag-is", "flag-it", "flag-jm",
            "flag-jo", "flag-jp", "flag-ke", "flag-kg", "flag-kh", "flag-ki",
            "flag-km", "flag-kn", "flag-kp", "flag-kr", "flag-kw", "flag-kz",
            "flag-la", "flag-lb", "flag-lc", "flag-li", "flag-lk", "flag-lr",
            "flag-ls", "flag-lt", "flag-lu", "flag-lv", "flag-ly", "flag-ma",
            "flag-mc", "flag-md", "flag-me", "flag-mg", "flag-mh", "flag-mk",
            "flag-ml", "flag-mm", "flag-mn", "flag-mr", "flag-mt", "flag-mu",
            "flag-mv", "flag-mw", "flag-mx", "flag-my", "flag-mz", "flag-na",
            "flag-ne", "flag-ng", "flag-ni", "flag-nl", "flag-no", "flag-np",
            "flag-nr", "flag-nz", "flag-om", "flag-pa", "flag-pe", "flag-pg",
            "flag-ph", "flag-pk", "flag-pl", "flag-ps", "flag-pt", "flag-pw",
            "flag-py", "flag-qa", "flag-ro", "flag-rs", "flag-ru", "flag-rw",
            "flag-sa", "flag-sb", "flag-sc", "flag-sd", "flag-se", "flag-sg",
            "flag-si", "flag-sk", "flag-sl", "flag-sm", "flag-sn", "flag-so",
            "flag-sr", "flag-ss", "flag-st", "flag-sv", "flag-sy", "flag-sz",
            "flag-td", "flag-tg", "flag-th", "flag-tj", "flag-tl", "flag-tm",
            "flag-tn", "flag-to", "flag-tr", "flag-tt", "flag-tv", "flag-tz",
            "flag-ua", "flag-ug", "flag-us", "flag-uy", "flag-uz", "flag-va",
            "flag-vc", "flag-ve", "flag-vn", "flag-vu", "flag-ws", "flag-xk",
            "flag-ye", "flag-za", "flag-zm", "flag-zw",
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
        .or_else(|| forms_tools_shapes(name))
        .or_else(|| control_shapes(name))
        .or_else(|| computer_science_shapes(name))
        .or_else(|| user_interface_shapes(name))
        .or_else(|| flag_shapes(name))
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
        // The Open/Collapsed control for the application shell's sidebar: a
        // framed window split by a rail, with an arrow in the wide half.
        // Drawn on the grid like every other icon — never a font glyph, so it
        // scales to the breadcrumb's height and takes the rail's own colours.
        //
        // The PAIR is the point: the arrow shows the NEXT action, never the
        // current state, so the control is never a mystery. Only the arrow
        // turns — the frame and its rail stay put, so the two read as one
        // control in two positions rather than as two different icons.
        // The rail is drawn on the LEFT (divider at x 9), because that is where
        // the sidebar it stands for actually sits. Mirrored, the icon claimed
        // a right-hand sidebar the shell has never had.
        // The arrow is centred on the WIDE pane (x 9 → 21, so centre 15), not
        // merely placed inside it: sitting flush against the divider it read as
        // crowding the rail rather than pointing away from it. It points the
        // way the rail will MOVE — right to open a left sidebar, left to close
        // it — which is the same rule as before, read off the correct side.
        "sidebar-expand" => vec![
            rr(3.0, 3.0, 18.0, 18.0, 3.0),
            p(&[(9.0, 3.2), (9.0, 20.8)]),
            pc(&[(12.75, 7.5), (17.25, 12.0), (12.75, 16.5)]),
        ],
        "sidebar-collapse" => vec![
            rr(3.0, 3.0, 18.0, 18.0, 3.0),
            p(&[(9.0, 3.2), (9.0, 20.8)]),
            pc(&[(17.25, 7.5), (12.75, 12.0), (17.25, 16.5)]),
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

/// Selection, Design, Application — the verbs and objects a form editor needs
/// (operator, 2026-08-17). Chunk 12.
///
/// Nothing here is new machinery: same 24-unit grid, same single stroke, fills
/// only where a small shape reads better solid than outlined.
#[rustfmt::skip]
fn forms_tools_shapes(name: &str) -> Option<Vec<IconShape>> {
    Some(match name {
        // ── Selection ──────────────────────────────────────────────────────
        // One marquee motif runs through the four `select-*` icons, so they
        // read as a family and differ only in what is selected.
        "select-item" => vec![
            rr(4.5, 4.5, 15.0, 15.0, 1.2),
            d(4.5, 4.5, 1.1), d(19.5, 4.5, 1.1),
            d(4.5, 19.5, 1.1), d(19.5, 19.5, 1.1),
        ],
        "select-all" => vec![
            rr(3.0, 3.0, 18.0, 18.0, 1.2),
            rr(6.5, 6.5, 4.5, 4.5, 0.6),
            rr(13.0, 6.5, 4.5, 4.5, 0.6),
            rr(6.5, 13.0, 4.5, 4.5, 0.6),
            rr(13.0, 13.0, 4.5, 4.5, 0.6),
        ],
        // Half in, half out: the diagonal wedge IS the inversion.
        "select-invert" => vec![
            rr(3.5, 3.5, 17.0, 17.0, 1.2),
            pf(&[(3.5, 20.5), (20.5, 3.5), (20.5, 20.5)]),
            p(&[(3.5, 20.5), (20.5, 3.5)]),
        ],
        // Nothing in the marquee — a cross, not a slash, so it cannot be read
        // as the diagonal that means "invert".
        "select-none" => vec![
            rr(4.5, 4.5, 15.0, 15.0, 1.2),
            p(&[(9.0, 9.0), (15.0, 15.0)]),
            p(&[(15.0, 9.0), (9.0, 15.0)]),
        ],
        // A drawn loop that has not quite closed, with its drawstring.
        "lasso" => vec![
            pathc(vec![
                L(12.0, 4.0),
                B(18.5, 4.0, 21.0, 8.0, 21.0, 10.5),
                B(21.0, 14.0, 17.0, 16.5, 12.0, 16.5),
                B(7.0, 16.5, 3.0, 14.0, 3.0, 10.5),
                B(3.0, 8.0, 5.5, 4.0, 12.0, 4.0),
            ]),
            path(vec![L(8.0, 15.8), Q(7.0, 19.0, 9.0, 20.5)]),
            d(9.6, 21.0, 1.2),
        ],
        // The classic four-way cursor.
        "move" => vec![
            p(&[(12.0, 3.0), (12.0, 21.0)]),
            p(&[(3.0, 12.0), (21.0, 12.0)]),
            pf(&[(12.0, 2.0), (9.6, 5.4), (14.4, 5.4)]),
            pf(&[(12.0, 22.0), (9.6, 18.6), (14.4, 18.6)]),
            pf(&[(2.0, 12.0), (5.4, 9.6), (5.4, 14.4)]),
            pf(&[(22.0, 12.0), (18.6, 9.6), (18.6, 14.4)]),
        ],
        // A PAIR of arrows — the shape collection, not the move cursor.
        "arrows" => vec![
            p(&[(4.5, 14.5), (14.5, 4.5)]),
            pf(&[(16.5, 2.5), (10.8, 4.0), (15.0, 8.2)]),
            p(&[(9.5, 19.5), (19.5, 9.5)]),
            pf(&[(7.5, 21.5), (13.2, 20.0), (9.0, 15.8)]),
        ],

        // ── Design ─────────────────────────────────────────────────────────
        // An upright pail with a swing handle, and one drop leaving it. The
        // first attempt was a tilted pouring bucket and read as a tent.
        "paint-bucket" => vec![
            pathc(vec![
                L(5.5, 8.5), L(17.5, 8.5), L(16.0, 20.0), L(7.0, 20.0),
            ]),
            path(vec![L(5.5, 8.5), Q(11.5, 2.5, 17.5, 8.5)]),
            p(&[(6.2, 13.0), (16.8, 13.0)]),
            pathc(vec![
                L(20.5, 12.5),
                Q(22.5, 15.5, 22.5, 17.0), Q(22.5, 19.0, 20.5, 19.0),
                Q(18.5, 19.0, 18.5, 17.0), Q(18.5, 15.5, 20.5, 12.5),
            ]),
        ],
        // A level rising in a shape: the wave is what says "filling".
        "fill" => vec![
            pc(&[(4.0, 3.5), (20.0, 3.5), (20.0, 20.5), (4.0, 20.5)]),
            IconShape::FillPath(vec![
                L(4.0, 13.0),
                Q(8.0, 10.5, 12.0, 13.0), Q(16.0, 15.5, 20.0, 13.0),
                L(20.0, 20.5), L(4.0, 20.5),
            ]),
            path(vec![
                L(4.0, 13.0),
                Q(8.0, 10.5, 12.0, 13.0), Q(16.0, 15.5, 20.0, 13.0),
            ]),
        ],
        "color-palette" => vec![
            pathc(vec![
                L(12.0, 3.0),
                B(18.5, 3.0, 21.5, 7.5, 21.5, 12.0),
                B(21.5, 16.0, 18.0, 16.5, 16.0, 16.5),
                B(14.5, 16.5, 14.0, 17.5, 14.5, 18.5),
                B(15.2, 20.0, 14.0, 21.0, 12.0, 21.0),
                B(6.5, 21.0, 2.5, 17.0, 2.5, 12.0),
                B(2.5, 7.0, 6.0, 3.0, 12.0, 3.0),
            ]),
            d(8.0, 8.5, 1.3),
            d(13.5, 6.8, 1.3),
            d(17.5, 10.5, 1.3),
            d(7.0, 14.5, 1.3),
        ],
        // An isometric cube: a shape you can place, rather than a picture.
        "object" => vec![
            pc(&[(12.0, 2.8), (20.5, 7.5), (20.5, 16.5), (12.0, 21.2), (3.5, 16.5), (3.5, 7.5)]),
            p(&[(12.0, 2.8), (12.0, 12.0)]),
            p(&[(12.0, 12.0), (20.5, 7.5)]),
            p(&[(12.0, 12.0), (3.5, 7.5)]),
        ],
        // `x =` — an equation reads as its own notation.
        "equation" => vec![
            p(&[(4.0, 7.5), (10.5, 15.5)]),
            p(&[(10.5, 7.5), (4.0, 15.5)]),
            p(&[(13.5, 9.8), (20.5, 9.8)]),
            p(&[(13.5, 13.8), (20.5, 13.8)]),
        ],
        "rotate-left" => vec![
            a(12.0, 12.5, 8.0, 200.0, 500.0),
            pf(&[(4.0, 12.5), (8.6, 10.0), (8.6, 15.0)]),
        ],
        "rotate-right" => vec![
            a(12.0, 12.5, 8.0, 340.0, 40.0),
            pf(&[(20.0, 12.5), (15.4, 10.0), (15.4, 15.0)]),
        ],
        // Mirrored wedges either side of the axis they flip about.
        "flip-horizontal" => vec![
            p(&[(12.0, 2.5), (12.0, 21.5)]),
            pc(&[(9.5, 6.0), (2.5, 12.0), (9.5, 18.0)]),
            pf(&[(14.5, 6.0), (21.5, 12.0), (14.5, 18.0)]),
        ],
        "flip-vertical" => vec![
            p(&[(2.5, 12.0), (21.5, 12.0)]),
            pc(&[(6.0, 9.5), (12.0, 2.5), (18.0, 9.5)]),
            pf(&[(6.0, 14.5), (12.0, 21.5), (18.0, 14.5)]),
        ],
        "fit-to-window" => vec![
            rr(2.5, 4.5, 19.0, 15.0, 1.5),
            p(&[(6.5, 8.5), (9.5, 11.5)]),
            pf(&[(6.0, 8.0), (10.2, 8.0), (6.0, 12.2)]),
            p(&[(17.5, 15.5), (14.5, 12.5)]),
            pf(&[(18.0, 16.0), (13.8, 16.0), (18.0, 11.8)]),
        ],
        // Pictures in a grid: the mountain says these are images, not tiles.
        "thumbnails" => vec![
            rr(2.5, 3.5, 8.5, 8.0, 1.0),
            p(&[(4.0, 9.8), (6.2, 7.0), (7.8, 9.0), (9.0, 7.8), (9.5, 9.8)]),
            d(8.6, 6.0, 0.8),
            rr(13.0, 3.5, 8.5, 8.0, 1.0),
            rr(2.5, 13.0, 8.5, 8.0, 1.0),
            rr(13.0, 13.0, 8.5, 8.0, 1.0),
        ],

        // ── Application ────────────────────────────────────────────────────
        // One window frame underpins window / form / application, so the three
        // read as the same object at three levels of detail.
        "window" => vec![
            rr(2.5, 4.0, 19.0, 16.0, 1.5),
            p(&[(2.5, 8.5), (21.5, 8.5)]),
            d(5.5, 6.2, 0.8), d(8.2, 6.2, 0.8), d(10.9, 6.2, 0.8),
        ],
        "form" => vec![
            rr(2.5, 4.0, 19.0, 16.0, 1.5),
            p(&[(2.5, 8.5), (21.5, 8.5)]),
            rr(5.5, 11.0, 13.0, 2.4, 0.5),
            p(&[(5.5, 16.0), (13.0, 16.0)]),
            rrf(15.5, 15.0, 3.0, 2.2, 0.5),
        ],
        // A window with a pointer in it: a program being USED, which is what
        // separates it from the bare `window` frame above. Four dots in a tile
        // read as a domino, not an application.
        "application" => vec![
            rr(2.5, 4.0, 19.0, 16.0, 1.5),
            p(&[(2.5, 8.5), (21.5, 8.5)]),
            pf(&[(10.0, 10.5), (10.0, 19.5), (12.3, 17.2), (13.9, 20.5), (15.6, 19.7), (14.0, 16.6), (17.2, 16.3)]),
        ],
        // A tied parcel: the band and the knot are what make it a BUNDLE
        // rather than the plain box `package-*` already draws.
        "bundle" => vec![
            rr(3.0, 7.5, 18.0, 13.0, 1.2),
            p(&[(12.0, 7.5), (12.0, 20.5)]),
            p(&[(3.0, 13.5), (21.0, 13.5)]),
            path(vec![L(12.0, 7.5), Q(9.0, 4.5, 10.5, 3.2), Q(12.0, 2.2, 12.0, 7.5)]),
            path(vec![L(12.0, 7.5), Q(15.0, 4.5, 13.5, 3.2), Q(12.0, 2.2, 12.0, 7.5)]),
        ],
        // A puzzle piece — the part that plugs into something bigger. A tab on
        // top and a tab on the right, both real curves; the first attempt read
        // as a notched box.
        "component" => vec![
            pathc(vec![
                L(4.5, 5.5), L(9.0, 5.5),
                Q(9.0, 2.0, 12.0, 2.0), Q(15.0, 2.0, 15.0, 5.5),
                L(19.5, 5.5), L(19.5, 10.0),
                Q(23.0, 10.0, 23.0, 13.0), Q(23.0, 16.0, 19.5, 16.0),
                L(19.5, 20.5), L(4.5, 20.5),
            ]),
        ],
        // THIS becomes THAT: two fields and the arrow between them. Arrows
        // crammed inside a lens turned to mud at a menu row's size.
        "find-replace" => vec![
            rr(2.5, 3.5, 10.0, 6.0, 1.0),
            rr(11.5, 14.5, 10.0, 6.0, 1.0),
            path(vec![L(7.5, 10.5), Q(7.5, 14.0, 12.5, 14.0)]),
            pf(&[(14.5, 14.0), (11.2, 12.3), (11.2, 15.7)]),
        ],
        // Lines of text, the proof-reader's squiggle under them, and a tick to
        // say it passed. The letter "A" the first attempt drew was too fine to
        // survive a 16 px row.
        "spelling" => vec![
            p(&[(3.0, 6.0), (18.0, 6.0)]),
            p(&[(3.0, 10.0), (13.0, 10.0)]),
            path(vec![
                L(3.0, 15.5), Q(4.7, 13.3, 6.4, 15.5), Q(8.1, 17.7, 9.8, 15.5),
                Q(11.5, 13.3, 13.2, 15.5),
            ]),
            p(&[(4.0, 19.0), (8.0, 22.0), (14.0, 13.5)]),
        ],
        // Words SAID rather than written: the same bubble `comment` uses, with a
        // wave inside instead of lines of text. Two attempts at a talking head
        // (head + shoulders + sound arcs) collapsed into a blob at a menu row's
        // size — too many curves in too little space.
        "speech" => vec![
            chat_bubble(),
            path(vec![
                L(6.5, 11.0), Q(8.2, 8.4, 9.9, 11.0), Q(11.6, 13.6, 13.3, 11.0),
                Q(15.0, 8.4, 16.7, 11.0),
            ]),
        ],
        "volume-up" => vec![
            pc(&[(3.0, 9.5), (7.0, 9.5), (11.5, 5.5), (11.5, 18.5), (7.0, 14.5), (3.0, 14.5)]),
            a(13.5, 12.0, 3.2, 300.0, 60.0),
            p(&[(18.5, 8.5), (18.5, 13.5)]),
            p(&[(16.0, 11.0), (21.0, 11.0)]),
        ],
        "volume-down" => vec![
            pc(&[(3.0, 9.5), (7.0, 9.5), (11.5, 5.5), (11.5, 18.5), (7.0, 14.5), (3.0, 14.5)]),
            a(13.5, 12.0, 3.2, 300.0, 60.0),
            p(&[(15.5, 11.0), (21.0, 11.0)]),
        ],
        // A crescent: the one shape that means "asleep" without words.
        "sleep" => vec![
            pathc(vec![
                L(19.0, 15.5),
                B(13.0, 18.5, 6.5, 15.0, 6.5, 9.5),
                B(6.5, 7.0, 7.6, 5.0, 9.0, 3.5),
                B(3.0, 5.5, 1.5, 13.0, 6.0, 18.0),
                B(9.5, 21.5, 16.0, 20.5, 19.0, 15.5),
            ]),
        ],
        // Out through the door — distinct from `power`, which switches off.
        "quit" => vec![
            p(&[(13.0, 3.5), (4.0, 3.5), (4.0, 20.5), (13.0, 20.5)]),
            p(&[(10.5, 12.0), (20.5, 12.0)]),
            pf(&[(22.0, 12.0), (17.5, 9.4), (17.5, 14.6)]),
        ],
        "globe" => vec![
            c(12.0, 12.0, 9.0),
            p(&[(3.0, 12.0), (21.0, 12.0)]),
            IconShape::Arc(12.0, 12.0, 4.6, 270.0, 450.0),
            IconShape::Arc(12.0, 12.0, 4.6, 90.0, 270.0),
            path(vec![L(4.6, 7.0), Q(12.0, 10.0, 19.4, 7.0)]),
            path(vec![L(4.6, 17.0), Q(12.0, 14.0, 19.4, 17.0)]),
        ],
        // One place, not the whole world: the pin, and the ground it stands on.
        "local" => vec![
            pathc(vec![
                L(12.0, 3.0),
                B(15.6, 3.0, 18.0, 5.6, 18.0, 8.8),
                B(18.0, 12.5, 13.5, 15.5, 12.0, 17.5),
                B(10.5, 15.5, 6.0, 12.5, 6.0, 8.8),
                B(6.0, 5.6, 8.4, 3.0, 12.0, 3.0),
            ]),
            d(12.0, 8.8, 1.6),
            IconShape::Arc(12.0, 19.5, 8.0, 200.0, 340.0),
        ],

        _ => return None,
    })
}

/// The RAD toolbox's own control icons, recreated on the 24-unit grid.
///
/// The IDE draws these in `panels/toolbox.rs::paint_control_icon` with ad-hoc
/// painter calls sized off the button; that code stays where it is (it is the
/// toolbox's own chrome). These are the same motifs re-authored as catalogue
/// shape data, so they scale, style and export like every other icon here —
/// and so a developer can put a control's icon in the app they are BUILDING,
/// which is what the toolbox drawings could never do.
///
/// One rule per control: whatever the toolbox uses to tell it apart at 26 px
/// is what is kept. Where a control's motif would collide with a generic name
/// already in the catalogue (`toggle-on` for Switch, `table` for DataGrid,
/// `map-pin` for Maps), the drawing is pulled towards the CONTROL — a frame, a
/// caption, a designed widget — rather than the bare concept.
#[rustfmt::skip]
fn control_shapes(name: &str) -> Option<Vec<IconShape>> {
    Some(match name {
        // ── Common input ───────────────────────────────────────────────────
        "control-button" => vec![
            rr(2.5, 6.5, 19.0, 11.0, 2.2),
            p(&[(8.5, 12.0), (15.5, 12.0)]),
        ],
        // The caret between its serifs — a text FIELD, not a line of text.
        "control-text-box" => vec![
            rr(2.5, 7.0, 19.0, 10.0, 1.2),
            p(&[(6.4, 9.4), (6.4, 14.6)]),
            p(&[(5.5, 9.4), (7.3, 9.4)]),
            p(&[(5.5, 14.6), (7.3, 14.6)]),
        ],
        // A DRAWN "A" over its baseline — never a font glyph (fonts vary,
        // drawings do not), the same rule `doc-pdf`'s "P" follows.
        "control-label" => vec![
            p(&[(7.0, 18.0), (12.0, 5.5), (17.0, 18.0)]),
            p(&[(9.0, 14.0), (15.0, 14.0)]),
            p(&[(4.5, 21.0), (19.5, 21.0)]),
        ],
        "control-check-box" => vec![
            rr(2.5, 7.8, 8.4, 8.4, 1.2),
            p(&[(4.4, 12.0), (6.3, 14.4), (9.4, 9.6)]),
            p(&[(13.0, 12.0), (20.5, 12.0)]),
        ],
        "control-radio-button" => vec![
            c(6.9, 12.0, 4.3),
            d(6.9, 12.0, 1.9),
            p(&[(13.0, 12.0), (20.5, 12.0)]),
        ],
        // A list with its first row SELECTED — the fill is what separates it
        // from `list-view`, which is a view mode, not a control.
        "control-list-box" => vec![
            rr(3.0, 3.6, 18.0, 16.8, 1.2),
            rrf(4.2, 4.8, 15.6, 2.6, 0.6),
            p(&[(3.0, 12.0), (21.0, 12.0)]),
            p(&[(3.0, 16.2), (21.0, 16.2)]),
        ],
        "control-combo-box" => vec![
            rr(2.5, 7.0, 19.0, 10.0, 1.2),
            p(&[(16.3, 7.0), (16.3, 17.0)]),
            p(&[(17.6, 10.8), (18.9, 13.2), (20.2, 10.8)]),
            p(&[(5.0, 12.0), (13.5, 12.0)]),
        ],
        // The caption BREAKS the frame — that gap is the whole glyph.
        "control-group-box" => vec![
            p(&[(6.5, 4.4), (11.5, 4.4)]),
            p(&[(3.0, 6.0), (5.2, 6.0)]),
            p(&[(12.8, 6.0), (21.0, 6.0)]),
            p(&[(3.0, 6.0), (3.0, 20.0)]),
            p(&[(21.0, 6.0), (21.0, 20.0)]),
            p(&[(3.0, 20.0), (21.0, 20.0)]),
        ],
        // Two stacked frames: a surface with depth, not a bordered group.
        "control-panel" => vec![
            rr(6.0, 6.5, 15.5, 14.0, 1.2),
            rr(2.5, 3.5, 15.5, 14.0, 1.2),
        ],
        "control-tab-control" => vec![
            rr(2.5, 8.0, 19.0, 12.0, 1.5),
            p(&[(3.0, 8.0), (3.0, 3.6), (10.0, 3.6), (10.0, 8.0)]),
            p(&[(11.3, 8.0), (11.3, 5.4), (17.6, 5.4), (17.6, 8.0)]),
        ],
        // A grid WITH a filled header band — `table` has neither the band nor
        // the row rules.
        "control-data-grid" => vec![
            rr(2.5, 4.0, 19.0, 16.0, 1.0),
            rrf(3.6, 5.1, 16.8, 2.4, 0.5),
            p(&[(2.5, 8.6), (21.5, 8.6)]),
            p(&[(8.8, 8.6), (8.8, 20.0)]),
            p(&[(15.2, 8.6), (15.2, 20.0)]),
            p(&[(2.5, 12.4), (21.5, 12.4)]),
            p(&[(2.5, 16.2), (21.5, 16.2)]),
        ],
        "control-picture-box" => vec![
            rr(2.5, 4.5, 19.0, 15.0, 1.0),
            c(8.0, 9.2, 1.8),
            p(&[(3.2, 19.0), (9.2, 12.6), (13.6, 19.0)]),
            p(&[(11.6, 19.0), (16.4, 13.8), (20.8, 19.0)]),
        ],
        "control-progress-bar" => vec![
            rr(2.5, 9.0, 19.0, 6.0, 3.0),
            rrf(3.7, 10.2, 10.4, 3.6, 1.8),
        ],
        // ── Bars and menus: the STRIP inside a window frame ─────────────────
        "control-menu-bar" => vec![
            rr(2.5, 4.0, 19.0, 16.0, 1.2),
            p(&[(2.5, 9.2), (21.5, 9.2)]),
            p(&[(5.0, 6.6), (8.2, 6.6)]),
            p(&[(10.4, 6.6), (13.6, 6.6)]),
            p(&[(15.8, 6.6), (19.0, 6.6)]),
        ],
        "control-tool-bar" => vec![
            rr(2.5, 4.0, 19.0, 16.0, 1.2),
            p(&[(2.5, 10.6), (21.5, 10.6)]),
            rrf(4.6, 6.0, 3.0, 3.0, 0.6),
            rrf(8.8, 6.0, 3.0, 3.0, 0.6),
            rrf(13.0, 6.0, 3.0, 3.0, 0.6),
            rrf(17.2, 6.0, 3.0, 3.0, 0.6),
        ],
        "control-status-bar" => vec![
            rr(2.5, 4.0, 19.0, 16.0, 1.2),
            p(&[(2.5, 15.4), (21.5, 15.4)]),
            p(&[(9.0, 15.4), (9.0, 20.0)]),
            p(&[(15.0, 15.4), (15.0, 20.0)]),
        ],
        // A tall rail of rows — deliberately the mirror of the menu bar.
        "control-side-menu" => vec![
            rr(2.5, 4.0, 19.0, 16.0, 1.2),
            p(&[(9.0, 4.0), (9.0, 20.0)]),
            p(&[(4.2, 8.0), (7.4, 8.0)]),
            p(&[(4.2, 12.0), (7.4, 12.0)]),
            p(&[(4.2, 16.0), (7.4, 16.0)]),
        ],
        // ── Layout ─────────────────────────────────────────────────────────
        // Endpoint handles say "a designed Line", not a rule or a divider.
        "control-line" => vec![
            p(&[(5.0, 19.0), (19.0, 5.0)]),
            d(5.0, 19.0, 1.7),
            d(19.0, 5.0, 1.7),
        ],
        "control-shape" => vec![
            pc(&[(12.0, 3.5), (20.0, 12.0), (12.0, 20.5), (4.0, 12.0)]),
        ],
        // Two panes and the grip between them.
        "control-splitter" => vec![
            rr(2.5, 4.5, 8.4, 15.0, 1.0),
            rr(13.1, 4.5, 8.4, 15.0, 1.0),
            d(12.0, 9.0, 1.1),
            d(12.0, 12.0, 1.1),
            d(12.0, 15.0, 1.1),
        ],
        // ── Pickers and steppers ───────────────────────────────────────────
        "control-date-time-picker" => vec![
            rr(3.0, 5.5, 18.0, 15.0, 1.2),
            p(&[(3.0, 10.0), (21.0, 10.0)]),
            p(&[(8.0, 3.5), (8.0, 7.2)]),
            p(&[(16.0, 3.5), (16.0, 7.2)]),
            d(7.6, 13.6, 1.1), d(12.0, 13.6, 1.1), d(16.4, 13.6, 1.1),
            d(7.6, 17.4, 1.1), d(12.0, 17.4, 1.1),
        ],
        "control-numeric-up-down" => vec![
            rr(2.5, 7.0, 19.0, 10.0, 1.2),
            p(&[(16.3, 7.0), (16.3, 17.0)]),
            p(&[(16.3, 12.0), (21.5, 12.0)]),
            p(&[(17.7, 10.4), (18.9, 9.2), (20.1, 10.4)]),
            p(&[(17.7, 13.6), (18.9, 14.8), (20.1, 13.6)]),
            p(&[(5.0, 12.0), (10.0, 12.0)]),
        ],
        "control-tree-view" => vec![
            d(5.2, 5.2, 1.8),
            p(&[(5.2, 7.0), (5.2, 18.4)]),
            p(&[(5.2, 10.6), (10.8, 10.6)]),
            d(12.4, 10.6, 1.6),
            p(&[(5.2, 18.4), (10.8, 18.4)]),
            d(12.4, 18.4, 1.6),
            p(&[(12.4, 12.2), (12.4, 14.5), (17.4, 14.5)]),
            d(18.8, 14.5, 1.4),
        ],
        "control-slider" => vec![
            p(&[(3.0, 11.0), (21.0, 11.0)]),
            c(14.0, 11.0, 3.0),
            d(14.0, 11.0, 1.3),
            p(&[(3.0, 15.5), (3.0, 17.5)]),
            p(&[(7.5, 15.5), (7.5, 17.5)]),
            p(&[(12.0, 15.5), (12.0, 17.5)]),
            p(&[(16.5, 15.5), (16.5, 17.5)]),
            p(&[(21.0, 15.5), (21.0, 17.5)]),
        ],
        // A full dial with its pointer, open at the bottom where a knob's
        // travel stops — `chart-gauge` is a half circle, this is not.
        "control-knob" => vec![
            c(12.0, 12.0, 7.0),
            p(&[(13.8, 10.2), (17.0, 7.0)]),
            d(12.0, 12.0, 1.4),
            p(&[(4.9, 19.1), (3.2, 20.8)]),
            p(&[(19.1, 19.1), (20.8, 20.8)]),
        ],
        "control-gauge" => vec![
            a(12.0, 15.0, 8.0, 180.0, 360.0),
            p(&[(12.0, 15.0), (17.0, 10.4)]),
            d(12.0, 15.0, 1.5),
            p(&[(4.0, 15.0), (4.0, 17.5)]),
            p(&[(20.0, 15.0), (20.0, 17.5)]),
        ],
        // A pill with the thumb to the RIGHT and a frame around it: the
        // designed control, where bare `toggle-on` is the state.
        "control-switch" => vec![
            rr(2.5, 8.0, 19.0, 8.0, 4.0),
            d(17.0, 12.0, 2.4),
            p(&[(6.0, 12.0), (9.5, 12.0)]),
        ],
        // ── Media and graphics ─────────────────────────────────────────────
        "control-animator" => vec![
            rr(2.5, 6.0, 19.0, 12.0, 1.5),
            p(&[(7.0, 6.0), (7.0, 8.2)]),
            p(&[(12.0, 6.0), (12.0, 8.2)]),
            p(&[(17.0, 6.0), (17.0, 8.2)]),
            p(&[(7.0, 18.0), (7.0, 15.8)]),
            p(&[(12.0, 18.0), (12.0, 15.8)]),
            p(&[(17.0, 18.0), (17.0, 15.8)]),
            pf(&[(10.4, 9.4), (10.4, 14.6), (15.0, 12.0)]),
        ],
        // The map VIEWPORT with a pin in it — `map-pin` is the pin alone.
        "control-maps" => vec![
            rr(2.5, 4.5, 19.0, 15.0, 1.5),
            pathc(vec![
                L(12.0, 7.0),
                B(14.4, 7.0, 16.0, 8.8, 16.0, 10.9),
                B(16.0, 13.3, 13.2, 15.3, 12.0, 16.8),
                B(10.8, 15.3, 8.0, 13.3, 8.0, 10.9),
                B(8.0, 8.8, 9.6, 7.0, 12.0, 7.0),
            ]),
            d(12.0, 10.8, 1.4),
        ],
        // ── Non-visual ─────────────────────────────────────────────────────
        // A stopwatch: crown and stem say "a Timer you start", not a clock.
        "control-timer" => vec![
            c(12.0, 13.8, 7.0),
            p(&[(12.0, 13.8), (12.0, 9.4)]),
            p(&[(12.0, 13.8), (15.6, 13.8)]),
            d(12.0, 13.8, 1.2),
            p(&[(9.8, 3.6), (14.2, 3.6)]),
            p(&[(12.0, 3.6), (12.0, 6.8)]),
        ],
        // A robot head with its antenna — the agent, not a person.
        "control-agent-object" => vec![
            rr(4.5, 8.6, 15.0, 11.0, 3.0),
            c(9.0, 12.6, 1.4),
            c(15.0, 12.6, 1.4),
            p(&[(9.2, 16.4), (14.8, 16.4)]),
            p(&[(12.0, 8.6), (12.0, 5.6)]),
            d(12.0, 4.2, 1.4),
        ],
        // A globe with its endpoints marked — the calls it makes, where the
        // catalogue's `globe` is the world itself.
        "control-rest-client" => vec![
            c(12.0, 12.0, 7.0),
            p(&[(5.0, 12.0), (19.0, 12.0)]),
            a(12.0, 12.0, 3.6, 270.0, 450.0),
            a(12.0, 12.0, 3.6, 90.0, 270.0),
            d(12.0, 5.0, 1.3), d(19.0, 12.0, 1.3),
            d(12.0, 19.0, 1.3), d(5.0, 12.0, 1.3),
        ],
        // The cylinder, plus the query caret that makes it SQL.
        "control-sql-database" => vec![
            pathc(vec![
                L(3.5, 6.5),
                Q(3.5, 4.2, 11.0, 4.2), Q(18.5, 4.2, 18.5, 6.5),
                Q(18.5, 8.8, 11.0, 8.8), Q(3.5, 8.8, 3.5, 6.5),
            ]),
            p(&[(3.5, 6.5), (3.5, 15.5)]),
            p(&[(18.5, 6.5), (18.5, 11.0)]),
            path(vec![L(3.5, 15.5), Q(3.5, 17.8, 11.0, 17.8)]),
            path(vec![L(3.5, 11.0), Q(3.5, 13.3, 11.0, 13.3), Q(18.5, 13.3, 18.5, 11.0)]),
            p(&[(14.0, 17.0), (16.6, 19.4), (14.0, 21.8)]),
            p(&[(18.0, 21.8), (21.5, 21.8)]),
        ],
        // A record page with its key: the INDEXED file, keyed access.
        "control-indexed-file" => vec![
            pathc(vec![
                L(4.5, 2.8), L(13.0, 2.8), L(17.5, 7.3), L(17.5, 17.0), L(4.5, 17.0),
            ]),
            p(&[(13.0, 2.8), (13.0, 7.3), (17.5, 7.3)]),
            p(&[(7.0, 10.5), (14.5, 10.5)]),
            p(&[(7.0, 13.5), (12.0, 13.5)]),
            c(8.4, 19.6, 2.6),
            p(&[(10.9, 19.6), (20.5, 19.6)]),
            p(&[(18.2, 19.6), (18.2, 22.0)]),
            p(&[(20.5, 19.6), (20.5, 21.6)]),
        ],
        // Dashed frame, arrow, tray — the universal drop target.
        "control-file-drop-zone" => {
            let mut v = Vec::new();
            // A dashed border, authored as eight short strokes.
            for (x0, x1) in [(2.5, 7.0), (9.5, 14.5), (17.0, 21.5)] {
                v.push(p(&[(x0, 3.5), (x1, 3.5)]));
                v.push(p(&[(x0, 20.5), (x1, 20.5)]));
            }
            for (y0, y1) in [(3.5, 7.5), (10.0, 14.0), (16.5, 20.5)] {
                v.push(p(&[(2.5, y0), (2.5, y1)]));
                v.push(p(&[(21.5, y0), (21.5, y1)]));
            }
            v.push(p(&[(12.0, 7.0), (12.0, 13.5)]));
            v.push(p(&[(9.2, 10.7), (12.0, 13.5), (14.8, 10.7)]));
            v.push(p(&[(7.0, 16.8), (17.0, 16.8)]));
            v
        }
        // A lens over a page of results — a SEARCH of the web, not a lens.
        "control-web-search" => vec![
            rr(2.5, 4.5, 19.0, 15.0, 1.5),
            p(&[(2.5, 8.4), (21.5, 8.4)]),
            c(10.4, 13.4, 3.6),
            p(&[(13.0, 16.0), (16.6, 19.6)]),
            p(&[(5.6, 6.4), (9.0, 6.4)]),
        ],
        // A plugin-provided control: the catalogue frame with a socket cut out
        // of it, the shape a third party fills in.
        "control-custom" => vec![
            path(vec![
                L(9.5, 3.5), L(3.5, 3.5), L(3.5, 9.0),
            ]),
            pathc(vec![
                L(3.5, 9.0),
                Q(6.6, 9.0, 6.6, 12.0), Q(6.6, 15.0, 3.5, 15.0),
                L(3.5, 20.5), L(20.5, 20.5), L(20.5, 3.5), L(9.5, 3.5),
            ]),
            d(14.5, 12.0, 1.5),
        ],
        // ── Charts ─────────────────────────────────────────────────────────
        "control-bar-chart" => vec![
            p(&[(3.0, 4.0), (3.0, 19.5), (21.0, 19.5)]),
            rrf(5.4, 14.0, 3.0, 5.5, 0.4),
            rrf(9.6, 10.0, 3.0, 9.5, 0.4),
            rrf(13.8, 12.2, 3.0, 7.3, 0.4),
            rrf(18.0, 6.8, 3.0, 12.7, 0.4),
        ],
        "control-line-chart" => vec![
            p(&[(3.0, 4.0), (3.0, 19.5), (21.0, 19.5)]),
            p(&[(5.0, 15.0), (9.5, 10.5), (13.5, 13.5), (19.5, 6.5)]),
            d(5.0, 15.0, 1.3), d(9.5, 10.5, 1.3),
            d(13.5, 13.5, 1.3), d(19.5, 6.5, 1.3),
        ],
        "control-pie-chart" => vec![
            c(12.0, 12.0, 8.0),
            p(&[(12.0, 12.0), (20.0, 12.0)]),
            p(&[(12.0, 12.0), (9.3, 4.5)]),
            p(&[(12.0, 12.0), (5.2, 15.9)]),
        ],
        "control-donut-chart" => vec![
            c(12.0, 12.0, 8.0),
            c(12.0, 12.0, 3.6),
            p(&[(15.6, 12.0), (20.0, 12.0)]),
            p(&[(10.8, 8.6), (9.3, 4.5)]),
            p(&[(9.1, 13.8), (5.2, 15.9)]),
        ],
        "control-area-chart" => vec![
            p(&[(3.0, 4.0), (3.0, 19.5), (21.0, 19.5)]),
            pf(&[(5.0, 19.5), (5.0, 15.0), (9.5, 10.5), (13.5, 13.5), (19.5, 7.5), (19.5, 19.5)]),
            p(&[(5.0, 15.0), (9.5, 10.5), (13.5, 13.5), (19.5, 7.5)]),
        ],
        "control-scatter-chart" => vec![
            p(&[(3.0, 4.0), (3.0, 19.5), (21.0, 19.5)]),
            c(7.0, 14.5, 1.5),
            c(11.0, 8.5, 1.5),
            c(13.5, 15.5, 1.5),
            c(17.5, 10.0, 1.5),
            c(19.5, 15.0, 1.5),
        ],

        _ => return None,
    })
}

/// Computer-science concepts.
///
/// Each one is drawn as the thing a developer would sketch to explain it, not
/// as a letterform: a stack is plates with a push arrow, a mutex is the lock
/// ON the shared line, recursion is the box that contains itself. Anything
/// that would need a label to be understood is drawn differently until it does
/// not.
#[rustfmt::skip]
fn computer_science_shapes(name: &str) -> Option<Vec<IconShape>> {
    Some(match name {
        // ── Data structures ────────────────────────────────────────────────
        "array" => vec![
            rr(2.0, 8.5, 20.0, 7.0, 1.0),
            p(&[(7.0, 8.5), (7.0, 15.5)]),
            p(&[(12.0, 8.5), (12.0, 15.5)]),
            p(&[(17.0, 8.5), (17.0, 15.5)]),
        ],
        // Brackets are what make a grid of dots a MATRIX.
        "matrix" => vec![
            p(&[(6.0, 4.0), (3.5, 4.0), (3.5, 20.0), (6.0, 20.0)]),
            p(&[(18.0, 4.0), (20.5, 4.0), (20.5, 20.0), (18.0, 20.0)]),
            d(8.5, 8.0, 1.2), d(12.0, 8.0, 1.2), d(15.5, 8.0, 1.2),
            d(8.5, 12.0, 1.2), d(12.0, 12.0, 1.2), d(15.5, 12.0, 1.2),
            d(8.5, 16.0, 1.2), d(12.0, 16.0, 1.2), d(15.5, 16.0, 1.2),
        ],
        // Plates, and the arrow that only ever reaches the top one.
        "stack-structure" => vec![
            rr(5.0, 11.5, 14.0, 3.4, 0.6),
            rr(5.0, 15.4, 14.0, 3.4, 0.6),
            p(&[(12.0, 2.5), (12.0, 9.0)]),
            p(&[(9.4, 6.4), (12.0, 9.0), (14.6, 6.4)]),
        ],
        // In one end, out the other — that is the whole difference.
        "queue-structure" => vec![
            rr(6.0, 8.5, 12.0, 7.0, 0.8),
            p(&[(10.0, 8.5), (10.0, 15.5)]),
            p(&[(14.0, 8.5), (14.0, 15.5)]),
            p(&[(1.5, 12.0), (5.0, 12.0)]),
            p(&[(3.4, 10.1), (5.3, 12.0), (3.4, 13.9)]),
            p(&[(19.0, 12.0), (22.5, 12.0)]),
            p(&[(20.6, 10.1), (22.5, 12.0), (20.6, 13.9)]),
        ],
        "linked-list" => vec![
            rr(2.0, 9.0, 7.0, 6.0, 0.8),
            rr(15.0, 9.0, 7.0, 6.0, 0.8),
            p(&[(9.0, 12.0), (14.0, 12.0)]),
            p(&[(12.2, 10.2), (14.2, 12.0), (12.2, 13.8)]),
            d(6.6, 12.0, 1.2),
            d(19.6, 12.0, 1.2),
        ],
        // A key dropping into one of the buckets.
        "hash-table" => vec![
            p(&[(4.0, 3.0), (20.0, 3.0)]),
            p(&[(12.0, 3.0), (12.0, 8.5)]),
            p(&[(9.8, 6.3), (12.0, 8.5), (14.2, 6.3)]),
            rr(2.5, 10.5, 19.0, 10.0, 0.8),
            p(&[(9.0, 10.5), (9.0, 20.5)]),
            p(&[(15.0, 10.5), (15.0, 20.5)]),
            d(12.0, 15.5, 1.6),
        ],
        "binary-tree" => vec![
            d(12.0, 4.5, 1.8),
            d(6.0, 12.0, 1.6),
            d(18.0, 12.0, 1.6),
            d(3.0, 19.5, 1.4),
            d(9.0, 19.5, 1.4),
            p(&[(10.8, 6.0), (7.2, 10.6)]),
            p(&[(13.2, 6.0), (16.8, 10.6)]),
            p(&[(5.0, 13.4), (3.7, 18.2)]),
            p(&[(7.0, 13.4), (8.3, 18.2)]),
        ],
        // A cycle, not a hierarchy: every node reaches two others.
        "graph-nodes" => vec![
            c(12.0, 4.8, 2.2),
            c(4.5, 17.5, 2.2),
            c(19.5, 17.5, 2.2),
            c(12.0, 12.0, 2.2),
            p(&[(10.6, 6.5), (5.7, 15.6)]),
            p(&[(13.4, 6.5), (18.3, 15.6)]),
            p(&[(6.7, 17.5), (17.3, 17.5)]),
            p(&[(12.0, 7.0), (12.0, 9.8)]),
        ],
        "venn" => vec![
            c(8.8, 12.0, 6.4),
            c(15.2, 12.0, 6.4),
        ],
        // Fixed cells around a ring, with the write head moving on.
        "ring-buffer" => vec![
            c(12.0, 12.0, 7.5),
            c(12.0, 12.0, 3.6),
            p(&[(12.0, 4.5), (12.0, 8.4)]),
            p(&[(18.5, 15.8), (15.1, 13.8)]),
            p(&[(5.5, 15.8), (8.9, 13.8)]),
            pf(&[(19.4, 6.4), (15.6, 6.9), (17.6, 10.1)]),
        ],

        // ── Language and compilation ───────────────────────────────────────
        // Ordered steps, and the machine that runs them.
        "algorithm" => vec![
            p(&[(3.0, 5.5), (13.0, 5.5)]),
            p(&[(3.0, 10.0), (13.0, 10.0)]),
            p(&[(3.0, 14.5), (10.0, 14.5)]),
            d(1.2, 5.5, 1.0), d(1.2, 10.0, 1.0), d(1.2, 14.5, 1.0),
            c(16.5, 17.0, 3.4),
            p(&[(16.5, 12.4), (16.5, 13.6)]),
            p(&[(16.5, 20.4), (16.5, 21.6)]),
            p(&[(11.9, 17.0), (13.1, 17.0)]),
            p(&[(19.9, 17.0), (21.1, 17.0)]),
        ],
        // Source in, machine code out.
        "compiler" => vec![
            rr(1.5, 5.0, 7.5, 14.0, 1.0),
            p(&[(3.2, 8.5), (7.3, 8.5)]),
            p(&[(3.2, 12.0), (7.3, 12.0)]),
            p(&[(3.2, 15.5), (5.8, 15.5)]),
            p(&[(10.0, 12.0), (14.5, 12.0)]),
            p(&[(12.6, 10.1), (14.5, 12.0), (12.6, 13.9)]),
            rr(15.5, 5.0, 7.0, 14.0, 1.0),
            p(&[(17.2, 9.0), (20.8, 9.0)]),
            p(&[(17.2, 12.0), (20.8, 12.0)]),
            p(&[(17.2, 15.0), (19.0, 15.0)]),
        ],
        // The same source, executed a line at a time — the caret IS the point.
        "interpreter" => vec![
            rr(2.5, 4.0, 19.0, 16.0, 1.2),
            p(&[(6.0, 8.0), (17.5, 8.0)]),
            p(&[(9.5, 12.0), (17.5, 12.0)]),
            p(&[(9.5, 16.0), (14.5, 16.0)]),
            pf(&[(5.4, 10.2), (5.4, 13.8), (8.2, 12.0)]),
        ],
        // A flat line of tokens fanning out into structure.
        "parser" => vec![
            p(&[(3.0, 5.0), (21.0, 5.0)]),
            p(&[(8.0, 3.4), (8.0, 6.6)]),
            p(&[(13.0, 3.4), (13.0, 6.6)]),
            p(&[(12.0, 8.0), (12.0, 10.5)]),
            p(&[(5.5, 15.0), (5.5, 12.5), (18.5, 12.5), (18.5, 15.0)]),
            p(&[(12.0, 12.5), (12.0, 15.0)]),
            d(5.5, 17.0, 1.6), d(12.0, 17.0, 1.6), d(18.5, 17.0, 1.6),
        ],
        // The tree the parser produced: an operator over its operands.
        "syntax-tree" => vec![
            c(12.0, 5.0, 2.6),
            p(&[(10.6, 5.0), (13.4, 5.0)]),
            p(&[(12.0, 3.6), (12.0, 6.4)]),
            c(5.5, 15.5, 2.6),
            c(18.5, 15.5, 2.6),
            p(&[(10.3, 6.9), (7.0, 13.4)]),
            p(&[(13.7, 6.9), (17.0, 13.4)]),
            p(&[(5.5, 18.1), (5.5, 21.0)]),
            p(&[(18.5, 18.1), (18.5, 21.0)]),
        ],
        // Compiled instructions in a chip — code that is no longer text.
        "bytecode" => vec![
            rr(5.5, 5.5, 13.0, 13.0, 1.2),
            p(&[(8.5, 9.5), (10.5, 12.0), (8.5, 14.5)]),
            p(&[(12.0, 14.5), (15.5, 14.5)]),
            p(&[(9.0, 2.5), (9.0, 5.5)]),
            p(&[(15.0, 2.5), (15.0, 5.5)]),
            p(&[(9.0, 18.5), (9.0, 21.5)]),
            p(&[(15.0, 18.5), (15.0, 21.5)]),
            p(&[(2.5, 9.0), (5.5, 9.0)]),
            p(&[(2.5, 15.0), (5.5, 15.0)]),
            p(&[(18.5, 9.0), (21.5, 9.0)]),
            p(&[(18.5, 15.0), (21.5, 15.0)]),
        ],
        // A named box whose contents can change: the tag, and the swap.
        "variable" => vec![
            rr(3.0, 8.0, 18.0, 9.0, 1.2),
            p(&[(7.0, 8.0), (7.0, 17.0)]),
            d(5.0, 12.5, 1.2),
            p(&[(10.5, 12.5), (17.5, 12.5)]),
            p(&[(15.6, 10.6), (17.5, 12.5), (15.6, 14.4)]),
            p(&[(10.5, 12.5), (10.5, 12.5)]),
        ],
        // The same box, locked.
        "constant" => vec![
            rr(3.0, 10.5, 18.0, 8.5, 1.2),
            p(&[(7.5, 13.5), (16.5, 13.5)]),
            p(&[(7.5, 16.0), (16.5, 16.0)]),
            a(12.0, 10.5, 3.6, 180.0, 360.0),
        ],
        // A box that contains itself.
        "recursion" => vec![
            rr(2.5, 4.0, 19.0, 16.0, 1.2),
            rr(7.0, 8.0, 10.5, 8.0, 1.0),
            rr(10.2, 10.8, 4.1, 2.6, 0.5),
        ],
        // A step, and the arrow that brings you back to it.
        "iteration" => vec![
            rr(7.5, 8.5, 9.0, 7.0, 1.0),
            path(vec![L(7.5, 6.0), Q(2.0, 6.0, 2.0, 12.0), Q(2.0, 18.0, 7.5, 18.0)]),
            p(&[(16.5, 6.0), (18.5, 6.0)]),
            path(vec![L(18.5, 6.0), Q(22.0, 6.0, 22.0, 12.0), Q(22.0, 18.0, 18.5, 18.0)]),
            p(&[(18.5, 18.0), (16.5, 18.0)]),
            p(&[(9.6, 16.1), (7.5, 18.0), (9.6, 19.9)]),
        ],
        // One question, two ways out.
        "conditional" => vec![
            pc(&[(12.0, 3.0), (19.0, 9.5), (12.0, 16.0), (5.0, 9.5)]),
            p(&[(19.0, 9.5), (21.5, 9.5), (21.5, 20.0)]),
            p(&[(5.0, 9.5), (2.5, 9.5), (2.5, 20.0)]),
            p(&[(20.1, 18.0), (21.5, 20.0), (22.9, 18.0)]),
            p(&[(1.1, 18.0), (2.5, 20.0), (3.9, 18.0)]),
        ],
        // Two states and nothing in between.
        "boolean" => vec![
            rr(2.0, 7.5, 20.0, 9.0, 4.5),
            p(&[(7.0, 9.5), (7.0, 14.5)]),
            c(16.0, 12.0, 2.6),
        ],
        // The characters that make a pattern a pattern.
        "regex" => vec![
            p(&[(6.0, 4.5), (3.0, 4.5), (3.0, 19.5), (6.0, 19.5)]),
            p(&[(18.0, 4.5), (21.0, 4.5), (21.0, 19.5), (18.0, 19.5)]),
            d(8.5, 14.5, 1.3),
            p(&[(14.5, 8.0), (14.5, 15.0)]),
            p(&[(11.6, 9.8), (17.4, 13.2)]),
            p(&[(17.4, 9.8), (11.6, 13.2)]),
        ],
        "flowchart" => vec![
            rr(6.5, 2.5, 11.0, 4.5, 2.25),
            p(&[(12.0, 7.0), (12.0, 9.5)]),
            pc(&[(12.0, 9.5), (17.5, 13.5), (12.0, 17.5), (6.5, 13.5)]),
            p(&[(12.0, 17.5), (12.0, 19.5)]),
            rr(6.5, 19.5, 11.0, 2.5, 0.5),
            p(&[(17.5, 13.5), (21.0, 13.5)]),
        ],

        // ── Runtime and concurrency ────────────────────────────────────────
        // Threads of execution: several, running side by side, from one start.
        "thread" => vec![
            d(3.5, 12.0, 1.6),
            path(vec![L(5.2, 12.0), Q(9.0, 12.0, 11.0, 6.5), Q(13.0, 1.5, 21.0, 5.0)]),
            path(vec![L(5.2, 12.0), Q(11.0, 12.0, 15.0, 12.0), Q(19.0, 12.0, 21.0, 12.0)]),
            path(vec![L(5.2, 12.0), Q(9.0, 12.0, 11.0, 17.5), Q(13.0, 22.5, 21.0, 19.0)]),
        ],
        // A running program: its own window, its own work.
        "process" => vec![
            rr(2.5, 4.0, 19.0, 16.0, 1.2),
            p(&[(2.5, 8.0), (21.5, 8.0)]),
            d(5.2, 6.0, 0.8), d(7.6, 6.0, 0.8),
            c(12.0, 14.2, 3.0),
            p(&[(12.0, 9.8), (12.0, 11.2)]),
            p(&[(12.0, 17.2), (12.0, 18.6)]),
            p(&[(7.6, 14.2), (9.0, 14.2)]),
            p(&[(15.0, 14.2), (16.4, 14.2)]),
        ],
        // The lock on the shared line — one holder at a time.
        "mutex" => vec![
            p(&[(2.0, 6.0), (22.0, 6.0)]),
            p(&[(2.0, 18.0), (22.0, 18.0)]),
            rr(7.5, 11.0, 9.0, 6.0, 1.0),
            a(12.0, 11.0, 2.8, 180.0, 360.0),
            d(12.0, 14.0, 1.1),
        ],
        // Each holds what the other is waiting for.
        "deadlock" => vec![
            rr(2.5, 3.0, 8.0, 8.0, 1.0),
            rr(13.5, 13.0, 8.0, 8.0, 1.0),
            path(vec![L(10.5, 5.5), Q(17.5, 5.5, 17.5, 12.0)]),
            p(&[(15.9, 10.4), (17.5, 12.4), (19.1, 10.4)]),
            path(vec![L(13.5, 18.5), Q(6.5, 18.5, 6.5, 12.0)]),
            p(&[(4.9, 13.6), (6.5, 11.6), (8.1, 13.6)]),
        ],
        "garbage-collection" => vec![
            p(&[(3.5, 7.0), (20.5, 7.0)]),
            p(&[(9.5, 7.0), (9.5, 4.5), (14.5, 4.5), (14.5, 7.0)]),
            path(vec![L(5.5, 7.0), L(6.8, 20.5), L(17.2, 20.5), L(18.5, 7.0)]),
            p(&[(9.6, 11.0), (12.0, 11.0)]),
            path(vec![L(12.0, 11.0), Q(15.5, 11.0, 15.5, 15.0)]),
            p(&[(10.9, 9.6), (9.3, 11.0), (10.9, 12.4)]),
            p(&[(14.1, 13.8), (15.5, 15.4), (16.9, 13.8)]),
        ],
        // Stacked frames, innermost on top, and what went wrong.
        "stack-trace" => vec![
            rr(2.5, 4.0, 13.0, 4.0, 0.6),
            rr(2.5, 9.5, 13.0, 4.0, 0.6),
            rr(2.5, 15.0, 13.0, 4.0, 0.6),
            p(&[(19.0, 6.5), (19.0, 12.5)]),
            d(19.0, 15.5, 1.3),
        ],
        // The gutter dot that stops the program on this line.
        "breakpoint" => vec![
            d(5.5, 12.0, 3.2),
            p(&[(11.0, 7.0), (21.0, 7.0)]),
            p(&[(11.0, 12.0), (21.0, 12.0)]),
            p(&[(11.0, 17.0), (17.5, 17.0)]),
        ],
        // The core, at the centre of everything the machine does.
        "kernel" => vec![
            rr(5.5, 5.5, 13.0, 13.0, 1.2),
            c(12.0, 12.0, 3.4),
            d(12.0, 12.0, 1.2),
            p(&[(9.0, 2.5), (9.0, 5.5)]),
            p(&[(15.0, 2.5), (15.0, 5.5)]),
            p(&[(9.0, 18.5), (9.0, 21.5)]),
            p(&[(15.0, 18.5), (15.0, 21.5)]),
            p(&[(2.5, 9.0), (5.5, 9.0)]),
            p(&[(2.5, 15.0), (5.5, 15.0)]),
            p(&[(18.5, 9.0), (21.5, 9.0)]),
            p(&[(18.5, 15.0), (21.5, 15.0)]),
        ],
        // A machine inside a machine.
        "virtual-machine" => vec![
            rr(2.0, 4.0, 20.0, 14.0, 1.2),
            p(&[(9.0, 18.0), (9.0, 20.5)]),
            p(&[(15.0, 18.0), (15.0, 20.5)]),
            p(&[(6.5, 20.5), (17.5, 20.5)]),
            rr(6.5, 7.0, 11.0, 8.0, 0.8),
            p(&[(9.5, 9.5), (11.5, 11.0), (9.5, 12.5)]),
        ],
        // Walled off: the fence is what makes it a sandbox.
        "sandbox" => {
            let mut v = Vec::new();
            for (x0, x1) in [(2.5, 7.5), (10.0, 14.0), (16.5, 21.5)] {
                v.push(p(&[(x0, 4.0), (x1, 4.0)]));
                v.push(p(&[(x0, 20.0), (x1, 20.0)]));
            }
            for (y0, y1) in [(4.0, 8.0), (10.5, 13.5), (16.0, 20.0)] {
                v.push(p(&[(2.5, y0), (2.5, y1)]));
                v.push(p(&[(21.5, y0), (21.5, y1)]));
            }
            v.push(c(12.0, 12.0, 4.0));
            v.push(pf(&[(10.6, 9.6), (10.6, 14.4), (14.4, 12.0)]));
            v
        }
        // Two tasks in flight at once, started together, landing apart.
        "async" => vec![
            d(3.0, 8.0, 1.5),
            p(&[(4.5, 8.0), (17.0, 8.0)]),
            p(&[(15.1, 6.1), (17.0, 8.0), (15.1, 9.9)]),
            d(3.0, 16.0, 1.5),
            p(&[(4.5, 16.0), (21.5, 16.0)]),
            p(&[(19.6, 14.1), (21.5, 16.0), (19.6, 17.9)]),
        ],
        // Handed out, and called back when the work is done.
        "callback" => vec![
            path(vec![L(3.0, 8.0), Q(12.0, 3.0, 21.0, 8.0)]),
            p(&[(19.0, 5.6), (21.4, 8.2), (18.6, 9.6)]),
            path(vec![L(21.0, 16.0), Q(12.0, 21.0, 3.0, 16.0)]),
            p(&[(5.0, 18.4), (2.6, 15.8), (5.4, 14.4)]),
        ],
        // Round and round, picking up whatever arrived.
        "event-loop" => vec![
            a(12.0, 12.0, 7.5, 130.0, 410.0),
            pf(&[(5.0, 12.6), (8.6, 14.6), (8.2, 10.6)]),
            d(12.0, 4.5, 1.5),
            d(19.5, 12.0, 1.5),
            d(14.6, 18.9, 1.5),
        ],
        // The clock decides which task runs next.
        "scheduler" => vec![
            c(7.0, 7.0, 4.5),
            p(&[(7.0, 4.2), (7.0, 7.0), (9.4, 7.0)]),
            rr(13.0, 4.0, 9.0, 4.0, 0.6),
            rr(2.0, 13.5, 20.0, 3.5, 0.6),
            rr(2.0, 18.5, 20.0, 3.5, 0.6),
        ],

        // ── Networking ─────────────────────────────────────────────────────
        "socket" => vec![
            rr(12.5, 6.5, 9.0, 11.0, 1.2),
            c(17.0, 12.0, 2.6),
            p(&[(2.5, 9.5), (9.5, 9.5)]),
            p(&[(2.5, 14.5), (9.5, 14.5)]),
            p(&[(9.5, 8.0), (9.5, 16.0)]),
            p(&[(9.5, 12.0), (12.5, 12.0)]),
        ],
        // Two ends agreeing on the same layers.
        "protocol" => vec![
            c(4.0, 12.0, 2.4),
            c(20.0, 12.0, 2.4),
            p(&[(8.0, 7.5), (16.0, 7.5)]),
            p(&[(8.0, 12.0), (16.0, 12.0)]),
            p(&[(8.0, 16.5), (16.0, 16.5)]),
            p(&[(6.4, 12.0), (8.0, 12.0)]),
            p(&[(16.0, 12.0), (17.6, 12.0)]),
        ],
        // Header and payload, in flight.
        "packet" => vec![
            rr(5.0, 8.0, 14.0, 8.0, 1.0),
            p(&[(9.5, 8.0), (9.5, 16.0)]),
            p(&[(11.5, 11.0), (16.5, 11.0)]),
            p(&[(11.5, 13.5), (14.5, 13.5)]),
            p(&[(1.5, 10.5), (3.5, 10.5)]),
            p(&[(1.5, 13.5), (3.5, 13.5)]),
            p(&[(20.5, 10.5), (22.5, 10.5)]),
            p(&[(20.5, 13.5), (22.5, 13.5)]),
        ],
        // One in, three out, evenly.
        "load-balancer" => vec![
            rr(8.5, 2.5, 7.0, 4.5, 0.8),
            p(&[(12.0, 7.0), (12.0, 10.0)]),
            p(&[(3.5, 10.0), (20.5, 10.0)]),
            p(&[(3.5, 10.0), (3.5, 13.5)]),
            p(&[(12.0, 10.0), (12.0, 13.5)]),
            p(&[(20.5, 10.0), (20.5, 13.5)]),
            rr(1.0, 13.5, 5.0, 5.0, 0.6),
            rr(9.5, 13.5, 5.0, 5.0, 0.6),
            rr(18.0, 13.5, 5.0, 5.0, 0.6),
        ],
        // Everything goes through the middle.
        "proxy" => vec![
            c(3.5, 12.0, 2.4),
            rr(8.5, 7.5, 7.0, 9.0, 1.0),
            c(20.5, 12.0, 2.4),
            p(&[(5.9, 12.0), (8.5, 12.0)]),
            p(&[(15.5, 12.0), (18.1, 12.0)]),
            p(&[(11.0, 10.5), (13.0, 12.0), (11.0, 13.5)]),
        ],
        // A wall, and what it stops.
        "firewall" => vec![
            p(&[(2.0, 8.0), (22.0, 8.0)]),
            p(&[(2.0, 13.0), (22.0, 13.0)]),
            p(&[(2.0, 18.0), (22.0, 18.0)]),
            p(&[(8.0, 3.5), (8.0, 8.0)]),
            p(&[(16.0, 3.5), (16.0, 8.0)]),
            p(&[(12.0, 8.0), (12.0, 13.0)]),
            p(&[(5.0, 13.0), (5.0, 18.0)]),
            p(&[(19.0, 13.0), (19.0, 18.0)]),
            p(&[(9.0, 18.0), (9.0, 22.0)]),
            p(&[(15.0, 18.0), (15.0, 22.0)]),
        ],
        // Small, separate, and talking to each other.
        "microservice" => vec![
            rr(2.0, 2.5, 7.0, 7.0, 1.0),
            rr(15.0, 2.5, 7.0, 7.0, 1.0),
            rr(8.5, 14.5, 7.0, 7.0, 1.0),
            p(&[(9.0, 6.0), (15.0, 6.0)]),
            p(&[(6.5, 9.5), (10.0, 14.5)]),
            p(&[(17.5, 9.5), (14.0, 14.5)]),
        ],
        // The connection stays open, both ways.
        "websocket" => vec![
            rr(2.0, 5.5, 8.0, 13.0, 1.0),
            rr(14.0, 5.5, 8.0, 13.0, 1.0),
            p(&[(10.0, 9.5), (14.0, 9.5)]),
            p(&[(12.2, 7.6), (14.2, 9.5), (12.2, 11.4)]),
            p(&[(14.0, 14.5), (10.0, 14.5)]),
            p(&[(11.8, 12.6), (9.8, 14.5), (11.8, 16.4)]),
        ],
        // The hook something else calls.
        "webhook" => vec![
            path(vec![
                L(7.0, 4.5), Q(15.5, 4.5, 15.5, 11.0), L(15.5, 15.0),
            ]),
            p(&[(4.6, 6.9), (7.0, 4.5), (9.4, 6.9)]),
            rr(11.5, 15.0, 8.0, 6.5, 1.0),
            p(&[(2.5, 18.0), (11.5, 18.0)]),
            d(2.5, 18.0, 1.4),
        ],

        // ── Security ───────────────────────────────────────────────────────
        // The page, locked: what is inside can no longer be read.
        "encryption" => vec![
            rr(2.5, 3.0, 12.0, 15.0, 1.0),
            p(&[(5.0, 7.0), (11.5, 7.0)]),
            p(&[(5.0, 10.5), (11.5, 10.5)]),
            p(&[(5.0, 14.0), (9.0, 14.0)]),
            rr(13.0, 14.5, 9.0, 7.0, 1.0),
            a(17.5, 14.5, 3.0, 180.0, 360.0),
            d(17.5, 18.0, 1.2),
        ],
        // Any input, a fixed-length digest.
        "hash-function" => vec![
            p(&[(1.5, 6.0), (7.0, 6.0)]),
            p(&[(1.5, 10.0), (5.0, 10.0)]),
            p(&[(1.5, 14.0), (8.0, 14.0)]),
            rr(9.0, 4.0, 7.0, 16.0, 1.0),
            p(&[(11.0, 7.5), (11.0, 16.5)]),
            p(&[(14.0, 7.5), (14.0, 16.5)]),
            p(&[(9.6, 10.0), (15.4, 10.0)]),
            p(&[(9.6, 14.0), (15.4, 14.0)]),
            p(&[(17.0, 12.0), (22.5, 12.0)]),
        ],
        // The number that proves nothing changed.
        "checksum" => vec![
            rr(3.0, 2.5, 14.0, 15.0, 1.0),
            p(&[(5.8, 6.5), (14.2, 6.5)]),
            p(&[(5.8, 10.0), (14.2, 10.0)]),
            p(&[(5.8, 13.5), (11.0, 13.5)]),
            c(16.5, 17.5, 4.5),
            p(&[(14.3, 17.6), (15.9, 19.4), (18.8, 15.6)]),
        ],
        // One key opens what the other locked.
        "key-pair" => vec![
            c(5.5, 7.5, 3.0),
            p(&[(7.8, 9.5), (13.0, 15.0)]),
            p(&[(11.2, 13.1), (12.8, 11.5)]),
            c(18.5, 16.5, 3.0),
            d(18.5, 16.5, 1.2),
            p(&[(16.2, 14.5), (11.0, 9.0)]),
            p(&[(12.8, 10.9), (11.2, 12.5)]),
        ],
        // Something you have, and the code it shows.
        "two-factor" => vec![
            rr(2.5, 3.5, 9.0, 17.0, 1.5),
            p(&[(5.5, 6.5), (8.5, 6.5)]),
            d(7.0, 17.5, 1.0),
            rr(13.0, 8.0, 9.0, 8.0, 1.0),
            a(17.5, 8.0, 2.8, 180.0, 360.0),
            d(15.5, 12.0, 0.9),
            d(17.5, 12.0, 0.9),
            d(19.5, 12.0, 0.9),
        ],

        // ── Data modelling ─────────────────────────────────────────────────
        // Tables, and the relations between them.
        "schema" => vec![
            rr(2.0, 2.5, 8.0, 6.0, 0.8),
            p(&[(2.0, 5.0), (10.0, 5.0)]),
            rr(14.0, 2.5, 8.0, 6.0, 0.8),
            p(&[(14.0, 5.0), (22.0, 5.0)]),
            rr(8.0, 15.0, 8.0, 6.0, 0.8),
            p(&[(8.0, 17.5), (16.0, 17.5)]),
            p(&[(10.0, 8.5), (10.0, 15.0)]),
            p(&[(18.0, 8.5), (18.0, 12.0), (14.0, 12.0), (14.0, 15.0)]),
        ],
        "primary-key" => vec![
            rr(2.5, 5.0, 19.0, 14.0, 1.0),
            p(&[(2.5, 9.5), (21.5, 9.5)]),
            p(&[(2.5, 14.5), (21.5, 14.5)]),
            c(6.0, 7.2, 1.5),
            p(&[(7.5, 7.2), (10.5, 7.2)]),
            p(&[(9.5, 6.2), (9.5, 8.2)]),
            p(&[(13.5, 7.2), (19.0, 7.2)]),
            p(&[(6.0, 12.0), (19.0, 12.0)]),
            p(&[(6.0, 16.8), (19.0, 16.8)]),
        ],
        // The key that lives in another table.
        "foreign-key" => vec![
            rr(1.5, 3.0, 9.0, 7.0, 0.8),
            p(&[(1.5, 5.8), (10.5, 5.8)]),
            c(4.2, 8.0, 1.2),
            rr(13.5, 14.0, 9.0, 7.0, 0.8),
            p(&[(13.5, 16.8), (22.5, 16.8)]),
            c(16.2, 19.0, 1.2),
            p(&[(5.6, 8.0), (9.0, 8.0), (9.0, 19.0), (14.8, 19.0)]),
        ],
        // What the two have in common.
        "join-tables" => vec![
            rr(2.0, 7.0, 12.0, 10.0, 1.0),
            rr(10.0, 7.0, 12.0, 10.0, 1.0),
            p(&[(10.0, 9.5), (14.0, 9.5)]),
            p(&[(10.0, 12.0), (14.0, 12.0)]),
            p(&[(10.0, 14.5), (14.0, 14.5)]),
        ],
        // One shape of data becomes another.
        "migration" => vec![
            pathc(vec![
                L(1.5, 6.5), Q(1.5, 4.5, 5.5, 4.5), Q(9.5, 4.5, 9.5, 6.5),
                Q(9.5, 8.5, 5.5, 8.5), Q(1.5, 8.5, 1.5, 6.5),
            ]),
            p(&[(1.5, 6.5), (1.5, 15.0)]),
            p(&[(9.5, 6.5), (9.5, 15.0)]),
            path(vec![L(1.5, 15.0), Q(1.5, 17.0, 5.5, 17.0), Q(9.5, 17.0, 9.5, 15.0)]),
            pathc(vec![
                L(14.5, 6.5), Q(14.5, 4.5, 18.5, 4.5), Q(22.5, 4.5, 22.5, 6.5),
                Q(22.5, 8.5, 18.5, 8.5), Q(14.5, 8.5, 14.5, 6.5),
            ]),
            p(&[(14.5, 6.5), (14.5, 15.0)]),
            p(&[(22.5, 6.5), (22.5, 15.0)]),
            path(vec![L(14.5, 15.0), Q(14.5, 17.0, 18.5, 17.0), Q(22.5, 17.0, 22.5, 15.0)]),
            p(&[(10.5, 20.5), (13.5, 20.5)]),
            p(&[(11.9, 19.1), (13.7, 20.5), (11.9, 21.9)]),
        ],
        // The same data, kept in more than one place.
        "replication" => vec![
            rr(2.0, 3.0, 9.0, 7.0, 1.0),
            p(&[(2.0, 5.5), (11.0, 5.5)]),
            rr(13.0, 14.0, 9.0, 7.0, 1.0),
            p(&[(13.0, 16.5), (22.0, 16.5)]),
            rr(2.0, 14.0, 9.0, 7.0, 1.0),
            p(&[(2.0, 16.5), (11.0, 16.5)]),
            p(&[(6.5, 10.0), (6.5, 14.0)]),
            p(&[(9.5, 10.0), (17.5, 12.0), (17.5, 14.0)]),
        ],
        // One table, split across several stores.
        "sharding" => vec![
            rr(7.0, 2.0, 10.0, 5.0, 0.8),
            p(&[(12.0, 7.0), (12.0, 9.5)]),
            p(&[(3.5, 9.5), (20.5, 9.5)]),
            p(&[(3.5, 9.5), (3.5, 12.5)]),
            p(&[(12.0, 9.5), (12.0, 12.5)]),
            p(&[(20.5, 9.5), (20.5, 12.5)]),
            rr(1.0, 12.5, 5.0, 8.0, 0.8),
            rr(9.5, 12.5, 5.0, 8.0, 0.8),
            rr(18.0, 12.5, 5.0, 8.0, 0.8),
        ],
        // A question asked of a table.
        "query" => vec![
            rr(2.5, 3.5, 19.0, 12.0, 1.0),
            p(&[(2.5, 7.5), (21.5, 7.5)]),
            p(&[(9.0, 7.5), (9.0, 15.5)]),
            c(14.5, 17.0, 4.0),
            p(&[(17.4, 19.9), (21.0, 23.0)]),
        ],

        // ── Version control and delivery ───────────────────────────────────
        "git-branch" => vec![
            c(6.5, 5.0, 2.6),
            c(6.5, 19.0, 2.6),
            c(17.5, 5.0, 2.6),
            p(&[(6.5, 7.6), (6.5, 16.4)]),
            path(vec![L(17.5, 7.6), Q(17.5, 12.5, 12.0, 12.5), L(6.5, 12.5)]),
        ],
        "git-merge" => vec![
            c(6.5, 5.0, 2.6),
            c(6.5, 19.0, 2.6),
            c(17.5, 12.0, 2.6),
            p(&[(6.5, 7.6), (6.5, 16.4)]),
            path(vec![L(6.5, 7.6), Q(6.5, 12.0, 11.5, 12.0), L(14.9, 12.0)]),
        ],
        "git-commit" => vec![
            c(12.0, 12.0, 3.4),
            p(&[(2.0, 12.0), (8.6, 12.0)]),
            p(&[(15.4, 12.0), (22.0, 12.0)]),
        ],
        // A branch, offered up for review.
        "pull-request" => vec![
            c(5.5, 5.0, 2.6),
            c(5.5, 19.0, 2.6),
            p(&[(5.5, 7.6), (5.5, 16.4)]),
            c(18.5, 19.0, 2.6),
            path(vec![L(18.5, 16.4), L(18.5, 8.0), Q(18.5, 5.0, 15.0, 5.0), L(11.0, 5.0)]),
            p(&[(13.2, 2.8), (10.8, 5.0), (13.2, 7.2)]),
        ],
        // The store, with its history.
        "repository" => vec![
            path(vec![L(5.0, 3.0), L(19.0, 3.0), L(19.0, 21.0), L(5.0, 21.0)]),
            path(vec![L(5.0, 3.0), Q(2.5, 3.0, 2.5, 5.5), L(2.5, 18.5), Q(2.5, 21.0, 5.0, 21.0)]),
            p(&[(5.0, 17.0), (19.0, 17.0)]),
            c(12.0, 9.0, 2.6),
            p(&[(12.0, 3.0), (12.0, 6.4)]),
            p(&[(12.0, 11.6), (12.0, 14.5)]),
        ],
        "diff" => vec![
            rr(2.0, 4.0, 8.5, 16.0, 0.8),
            rr(13.5, 4.0, 8.5, 16.0, 0.8),
            p(&[(4.0, 9.0), (8.5, 9.0)]),
            p(&[(6.25, 6.75), (6.25, 11.25)]),
            p(&[(15.5, 15.0), (20.0, 15.0)]),
            p(&[(4.0, 15.0), (8.5, 15.0)]),
            p(&[(15.5, 9.0), (20.0, 9.0)]),
        ],
        // The small fix that goes over the break.
        "patch" => vec![
            p(&[(2.0, 12.0), (7.5, 12.0)]),
            p(&[(16.5, 12.0), (22.0, 12.0)]),
            rr(6.5, 6.5, 11.0, 11.0, 2.0),
            p(&[(9.5, 12.0), (14.5, 12.0)]),
            p(&[(12.0, 9.5), (12.0, 14.5)]),
        ],
        // Stages that run every time, and start again.
        "ci-cd" => vec![
            rr(1.5, 8.5, 5.5, 7.0, 0.8),
            rr(9.25, 8.5, 5.5, 7.0, 0.8),
            rr(17.0, 8.5, 5.5, 7.0, 0.8),
            p(&[(7.0, 12.0), (9.25, 12.0)]),
            p(&[(14.75, 12.0), (17.0, 12.0)]),
            path(vec![L(19.75, 15.5), Q(19.75, 20.5, 12.0, 20.5), Q(4.25, 20.5, 4.25, 15.5)]),
            p(&[(2.7, 17.0), (4.25, 15.0), (5.8, 17.0)]),
        ],
        // Layers, sealed and shipped as one.
        "container-image" => vec![
            rr(3.5, 4.0, 17.0, 4.5, 0.8),
            rr(3.5, 9.75, 17.0, 4.5, 0.8),
            rr(3.5, 15.5, 17.0, 4.5, 0.8),
            d(7.0, 6.25, 1.0),
            d(7.0, 12.0, 1.0),
            d(7.0, 17.75, 1.0),
        ],
        // One hand on all of them.
        "orchestration" => vec![
            c(12.0, 5.0, 3.0),
            p(&[(12.0, 8.0), (12.0, 11.0)]),
            p(&[(4.0, 11.0), (20.0, 11.0)]),
            p(&[(4.0, 11.0), (4.0, 14.0)]),
            p(&[(12.0, 11.0), (12.0, 14.0)]),
            p(&[(20.0, 11.0), (20.0, 14.0)]),
            rr(1.5, 14.0, 5.0, 6.5, 0.8),
            rr(9.5, 14.0, 5.0, 6.5, 0.8),
            rr(17.5, 14.0, 5.0, 6.5, 0.8),
        ],

        // ── Theory ─────────────────────────────────────────────────────────
        // Cost against input: the curve everyone draws.
        "complexity" => vec![
            p(&[(3.0, 3.0), (3.0, 20.5), (21.5, 20.5)]),
            path(vec![L(4.5, 19.5), Q(13.0, 19.0, 16.0, 12.0), Q(18.0, 7.0, 18.5, 4.0)]),
            p(&[(4.5, 19.5), (20.5, 17.0)]),
        ],
        // Out of order in, in order out.
        "sorting" => vec![
            rr(2.5, 13.0, 3.0, 7.5, 0.4),
            rr(6.9, 8.0, 3.0, 12.5, 0.4),
            rr(11.3, 15.5, 3.0, 5.0, 0.4),
            rr(15.7, 4.0, 3.0, 16.5, 0.4),
            p(&[(20.5, 4.0), (20.5, 20.5)]),
            p(&[(18.9, 18.9), (20.5, 20.9), (22.1, 18.9)]),
        ],
        // Halve the range, then halve it again.
        "binary-search" => vec![
            rr(2.0, 9.0, 20.0, 6.0, 0.8),
            p(&[(12.0, 9.0), (12.0, 15.0)]),
            p(&[(7.0, 9.0), (7.0, 15.0)]),
            p(&[(17.0, 9.0), (17.0, 15.0)]),
            p(&[(4.5, 5.5), (9.5, 5.5)]),
            p(&[(7.0, 5.5), (7.0, 8.0)]),
            p(&[(14.5, 18.5), (19.5, 18.5)]),
            p(&[(17.0, 16.0), (17.0, 18.5)]),
        ],
        // States, and what moves you between them.
        "state-machine" => vec![
            c(5.5, 12.0, 3.5),
            c(18.5, 12.0, 3.5),
            path(vec![L(8.6, 10.4), Q(12.0, 7.0, 15.4, 10.4)]),
            p(&[(13.3, 9.4), (15.8, 10.6), (14.7, 12.6)]),
            path(vec![L(15.4, 13.6), Q(12.0, 17.0, 8.6, 13.6)]),
            p(&[(10.7, 14.6), (8.2, 13.4), (9.3, 11.4)]),
        ],
        // Inputs on the left, the answer on the right.
        "truth-table" => vec![
            rr(3.0, 4.0, 18.0, 16.0, 1.0),
            p(&[(3.0, 8.0), (21.0, 8.0)]),
            p(&[(15.0, 4.0), (15.0, 20.0)]),
            p(&[(9.0, 4.0), (9.0, 20.0)]),
            d(6.0, 11.0, 1.1), d(12.0, 11.0, 1.1), d(18.0, 11.0, 1.1),
            d(6.0, 15.0, 1.1),
            d(18.0, 17.0, 1.1),
        ],
        // Two rows of bits, one operator.
        "bitwise" => vec![
            p(&[(2.5, 6.0), (10.5, 6.0)]),
            p(&[(2.5, 11.0), (10.5, 11.0)]),
            p(&[(2.5, 18.0), (10.5, 18.0)]),
            p(&[(2.5, 14.0), (10.5, 14.0)]),
            d(14.5, 6.0, 1.2), c(18.5, 6.0, 1.2),
            c(14.5, 11.0, 1.2), d(18.5, 11.0, 1.2),
            d(14.5, 18.0, 1.2), d(18.5, 18.0, 1.2),
        ],
        // Ones and zeros, drawn — never typed.
        "binary-number" => vec![
            p(&[(4.0, 5.5), (4.0, 18.5)]),
            p(&[(2.5, 7.5), (4.0, 5.5)]),
            c(11.0, 12.0, 3.4),
            p(&[(19.0, 5.5), (19.0, 18.5)]),
            p(&[(17.5, 7.5), (19.0, 5.5)]),
        ],
        // Base sixteen: the digits that run past nine.
        "hexadecimal" => vec![
            pc(&[(12.0, 2.5), (20.5, 7.25), (20.5, 16.75), (12.0, 21.5), (3.5, 16.75), (3.5, 7.25)]),
            p(&[(8.5, 9.0), (13.5, 15.0)]),
            p(&[(13.5, 9.0), (8.5, 15.0)]),
            p(&[(15.5, 15.0), (17.5, 15.0)]),
        ],
        // Layers of nodes, everything wired to everything.
        "neural-network" => vec![
            d(4.0, 6.0, 1.6), d(4.0, 12.0, 1.6), d(4.0, 18.0, 1.6),
            d(12.0, 8.0, 1.6), d(12.0, 16.0, 1.6),
            d(20.0, 12.0, 1.6),
            p(&[(4.0, 6.0), (12.0, 8.0)]),
            p(&[(4.0, 12.0), (12.0, 8.0)]),
            p(&[(4.0, 12.0), (12.0, 16.0)]),
            p(&[(4.0, 18.0), (12.0, 16.0)]),
            p(&[(12.0, 8.0), (20.0, 12.0)]),
            p(&[(12.0, 16.0), (20.0, 12.0)]),
        ],

        _ => return None,
    })
}

/// User-interface patterns, interaction and layout.
///
/// A pattern is drawn as its silhouette — the shape you would recognise across
/// a room with the labels removed: a modal is a sheet over a dimmed page, an
/// accordion is rows where exactly one has opened, padding is the gap the
/// content does NOT fill. Interaction icons all build on one pointer glyph so
/// click, double-click and long-press read as a family.
#[rustfmt::skip]
fn user_interface_shapes(name: &str) -> Option<Vec<IconShape>> {
    /// The shared arrow cursor, tip at (x, y).
    fn pointer(x: f32, y: f32) -> IconShape {
        pathc(vec![
            L(x, y),
            L(x, y + 9.5),
            L(x + 2.4, y + 7.2),
            L(x + 4.2, y + 10.6),
            L(x + 6.0, y + 9.6),
            L(x + 4.2, y + 6.4),
            L(x + 7.4, y + 6.0),
        ])
    }
    Some(match name {
        // ── Overlays ───────────────────────────────────────────────────────
        // A sheet over the page it blocks.
        "modal" => vec![
            rr(2.0, 3.0, 20.0, 18.0, 1.2),
            rrf(5.0, 7.0, 14.0, 10.0, 1.0),
            p(&[(7.5, 10.5), (16.5, 10.5)]),
            p(&[(7.5, 13.5), (13.0, 13.5)]),
        ],
        // A question and the two ways out of it.
        "dialog" => vec![
            rr(2.0, 4.5, 20.0, 15.0, 1.2),
            p(&[(5.0, 8.5), (19.0, 8.5)]),
            p(&[(5.0, 11.5), (14.0, 11.5)]),
            rr(10.0, 14.5, 5.0, 3.0, 0.6),
            rrf(16.0, 14.5, 5.0, 3.0, 0.6),
        ],
        // A small label with a tail, hanging off what it explains.
        "tooltip" => vec![
            rr(2.5, 5.0, 19.0, 8.0, 1.2),
            p(&[(5.5, 8.0), (18.5, 8.0)]),
            p(&[(5.5, 10.5), (13.5, 10.5)]),
            pf(&[(9.5, 13.0), (14.5, 13.0), (11.0, 16.5)]),
            d(11.0, 19.5, 1.6),
        ],
        // Bigger than a tooltip, anchored to a control.
        "popover" => vec![
            rr(2.5, 2.5, 19.0, 12.0, 1.2),
            p(&[(2.5, 6.0), (21.5, 6.0)]),
            p(&[(5.5, 9.5), (18.5, 9.5)]),
            p(&[(5.5, 12.0), (13.5, 12.0)]),
            pf(&[(9.5, 14.5), (14.5, 14.5), (12.0, 17.5)]),
            rr(8.0, 18.5, 8.0, 3.5, 0.8),
        ],
        // The field, and the list it opened.
        "dropdown" => vec![
            rr(2.5, 3.0, 19.0, 6.0, 1.0),
            p(&[(16.0, 4.8), (17.8, 7.0), (19.6, 4.8)]),
            rr(2.5, 11.0, 19.0, 10.5, 1.0),
            p(&[(2.5, 14.5), (21.5, 14.5)]),
            p(&[(2.5, 18.0), (21.5, 18.0)]),
        ],
        // Rows, exactly one of them open.
        "accordion" => vec![
            rr(2.5, 2.5, 19.0, 4.0, 0.8),
            p(&[(17.0, 3.8), (18.4, 5.2), (19.8, 3.8)]),
            rr(2.5, 8.0, 19.0, 9.0, 0.8),
            p(&[(17.0, 11.2), (18.4, 9.8), (19.8, 11.2)]),
            p(&[(5.0, 14.0), (14.0, 14.0)]),
            rr(2.5, 18.5, 19.0, 4.0, 0.8),
            p(&[(17.0, 19.8), (18.4, 21.2), (19.8, 19.8)]),
        ],
        // Where you are, and how you got here.
        "breadcrumb" => vec![
            rrf(1.5, 9.5, 4.5, 5.0, 0.8),
            p(&[(7.5, 9.5), (9.9, 12.0), (7.5, 14.5)]),
            rr(11.0, 9.5, 4.5, 5.0, 0.8),
            p(&[(17.0, 9.5), (19.4, 12.0), (17.0, 14.5)]),
            rr(20.5, 9.5, 3.0, 5.0, 0.8),
        ],
        // Page after page, with the one you are on filled in.
        "pagination" => vec![
            p(&[(3.5, 9.5), (1.5, 12.0), (3.5, 14.5)]),
            rr(5.5, 9.0, 5.0, 6.0, 0.8),
            rrf(11.5, 9.0, 5.0, 6.0, 0.8),
            rr(17.5, 9.0, 5.0, 6.0, 0.8),
            p(&[(20.5, 16.5), (22.5, 19.0), (20.5, 21.5)]),
        ],
        // One of several steps, done in order.
        "stepper" => vec![
            p(&[(4.0, 12.0), (20.0, 12.0)]),
            d(4.0, 12.0, 2.6),
            d(12.0, 12.0, 2.6),
            c(20.0, 12.0, 2.6),
        ],
        // The steps, inside the window that walks you through them.
        "wizard" => vec![
            rr(2.0, 3.5, 20.0, 17.0, 1.2),
            d(6.0, 8.0, 1.6),
            d(12.0, 8.0, 1.6),
            c(18.0, 8.0, 1.6),
            p(&[(7.6, 8.0), (10.4, 8.0)]),
            p(&[(13.6, 8.0), (16.4, 8.0)]),
            p(&[(5.0, 13.0), (14.0, 13.0)]),
            rrf(14.0, 15.5, 6.0, 3.5, 0.7),
        ],
        // One panel at a time, with the rest waiting either side.
        "carousel" => vec![
            rr(6.5, 4.5, 11.0, 12.0, 1.0),
            p(&[(4.0, 6.5), (4.0, 14.5)]),
            p(&[(20.0, 6.5), (20.0, 14.5)]),
            d(9.0, 19.5, 1.1),
            d(12.0, 19.5, 1.1),
            d(15.0, 19.5, 1.1),
        ],
        // A panel that comes in from the edge.
        "drawer" => vec![
            rr(2.0, 4.0, 20.0, 16.0, 1.2),
            p(&[(9.5, 4.0), (9.5, 20.0)]),
            rrf(2.0, 4.0, 7.5, 16.0, 1.2),
            p(&[(13.0, 12.0), (19.5, 12.0)]),
            p(&[(17.3, 9.8), (19.7, 12.0), (17.3, 14.2)]),
        ],
        // A message that comes and goes on its own.
        "toast" => vec![
            rr(2.0, 14.0, 20.0, 7.0, 1.2),
            d(5.8, 17.5, 1.4),
            p(&[(9.0, 16.5), (19.0, 16.5)]),
            p(&[(9.0, 19.0), (15.0, 19.0)]),
            p(&[(2.0, 4.0), (22.0, 4.0)]),
            p(&[(2.0, 4.0), (2.0, 8.0)]),
            p(&[(22.0, 4.0), (22.0, 8.0)]),
        ],
        // A small removable tag.
        "chip" => vec![
            rr(2.0, 8.0, 20.0, 8.0, 4.0),
            p(&[(5.5, 12.0), (13.0, 12.0)]),
            p(&[(16.0, 10.0), (19.0, 14.0)]),
            p(&[(19.0, 10.0), (16.0, 14.0)]),
        ],
        // Where the content will be, before it arrives.
        "skeleton" => vec![
            rr(2.0, 4.0, 20.0, 16.0, 1.2),
            rrf(4.5, 7.0, 5.0, 5.0, 2.5),
            rrf(11.5, 7.5, 8.0, 1.8, 0.9),
            rrf(11.5, 10.5, 5.5, 1.8, 0.9),
            rrf(4.5, 15.0, 15.0, 1.8, 0.9),
        ],
        // The track and its thumb, beside the content.
        "scrollbar" => vec![
            rr(2.5, 3.5, 13.0, 17.0, 1.0),
            rr(17.5, 3.5, 4.0, 17.0, 2.0),
            rrf(18.4, 5.5, 2.2, 7.0, 1.1),
        ],
        // A field you type a search into — the lens is IN the control.
        "search-field" => vec![
            rr(1.5, 8.0, 21.0, 8.0, 4.0),
            c(6.5, 12.0, 2.6),
            p(&[(8.4, 13.9), (10.2, 15.7)]),
            p(&[(12.5, 12.0), (19.0, 12.0)]),
        ],
        // A file, going up.
        "file-upload" => vec![
            pathc(vec![
                L(4.5, 2.5), L(13.0, 2.5), L(17.5, 7.0), L(17.5, 14.0), L(4.5, 14.0),
            ]),
            p(&[(13.0, 2.5), (13.0, 7.0), (17.5, 7.0)]),
            p(&[(11.0, 22.0), (11.0, 15.5)]),
            p(&[(8.2, 18.3), (11.0, 15.5), (13.8, 18.3)]),
        ],
        // Nothing here yet, and the frame that says so.
        "empty-state" => {
            let mut v = Vec::new();
            for (x0, x1) in [(3.0, 8.0), (10.0, 14.0), (16.0, 21.0)] {
                v.push(p(&[(x0, 4.5), (x1, 4.5)]));
                v.push(p(&[(x0, 19.5), (x1, 19.5)]));
            }
            for (y0, y1) in [(4.5, 8.0), (10.5, 13.5), (16.0, 19.5)] {
                v.push(p(&[(3.0, y0), (3.0, y1)]));
                v.push(p(&[(21.0, y0), (21.0, y1)]));
            }
            v.push(p(&[(9.0, 12.0), (15.0, 12.0)]));
            v
        }

        // ── Craft ──────────────────────────────────────────────────────────
        // Boxes standing in for content: the layout before the design.
        "wireframe" => vec![
            rr(2.0, 3.5, 20.0, 17.0, 1.2),
            p(&[(2.0, 7.5), (22.0, 7.5)]),
            rr(4.5, 10.0, 6.0, 8.0, 0.6),
            p(&[(12.5, 11.0), (19.5, 11.0)]),
            p(&[(12.5, 14.0), (19.5, 14.0)]),
            p(&[(12.5, 17.0), (16.5, 17.0)]),
        ],
        // The design, shown in the thing it will run on.
        "mockup" => vec![
            rr(2.0, 3.0, 14.0, 13.0, 1.0),
            p(&[(6.0, 16.0), (6.0, 18.5)]),
            p(&[(12.0, 16.0), (12.0, 18.5)]),
            p(&[(3.5, 18.5), (14.5, 18.5)]),
            rr(16.5, 9.0, 6.0, 12.0, 1.0),
            p(&[(18.5, 10.8), (20.5, 10.8)]),
        ],
        // The same page, at three widths.
        "responsive" => vec![
            rr(1.5, 5.0, 12.0, 10.0, 1.0),
            p(&[(5.5, 15.0), (5.5, 17.5)]),
            p(&[(3.0, 17.5), (8.0, 17.5)]),
            rr(15.0, 7.0, 7.5, 14.0, 1.0),
            p(&[(17.5, 9.0), (20.0, 9.0)]),
            d(18.75, 19.0, 0.9),
        ],
        "dark-mode" => vec![
            rr(2.5, 2.5, 19.0, 19.0, 3.0),
            path(vec![
                L(15.5, 6.5),
                B(11.0, 7.0, 8.5, 9.5, 8.5, 12.5),
                B(8.5, 15.5, 11.0, 17.8, 15.5, 17.8),
                B(12.8, 16.0, 11.5, 14.5, 11.5, 12.2),
                B(11.5, 9.8, 12.8, 8.0, 15.5, 6.5),
            ]),
        ],
        "light-mode" => vec![
            rr(2.5, 2.5, 19.0, 19.0, 3.0),
            c(12.0, 12.0, 3.6),
            p(&[(12.0, 5.0), (12.0, 6.6)]),
            p(&[(12.0, 17.4), (12.0, 19.0)]),
            p(&[(5.0, 12.0), (6.6, 12.0)]),
            p(&[(17.4, 12.0), (19.0, 12.0)]),
            p(&[(7.1, 7.1), (8.2, 8.2)]),
            p(&[(15.8, 15.8), (16.9, 16.9)]),
            p(&[(16.9, 7.1), (15.8, 8.2)]),
            p(&[(8.2, 15.8), (7.1, 16.9)]),
        ],
        // The international access symbol, drawn on our own grid.
        "accessibility" => vec![
            c(12.0, 12.0, 9.0),
            d(12.0, 6.6, 1.5),
            p(&[(6.8, 10.0), (17.2, 10.0)]),
            p(&[(12.0, 9.5), (12.0, 14.5)]),
            p(&[(12.0, 14.5), (8.6, 18.5)]),
            p(&[(12.0, 14.5), (15.4, 18.5)]),
        ],
        // The screen, read out loud.
        "screen-reader" => vec![
            rr(1.5, 5.0, 12.0, 11.0, 1.0),
            p(&[(5.5, 16.0), (5.5, 18.5)]),
            p(&[(3.0, 18.5), (8.0, 18.5)]),
            a(15.0, 12.0, 3.0, 300.0, 420.0),
            a(15.0, 12.0, 5.6, 305.0, 415.0),
            a(15.0, 12.0, 8.2, 310.0, 410.0),
        ],
        // A key, with its modifier.
        "keyboard-shortcut" => vec![
            rr(1.5, 6.5, 9.0, 9.0, 1.2),
            p(&[(4.0, 11.0), (8.0, 11.0)]),
            p(&[(13.5, 11.0), (15.5, 11.0)]),
            rr(13.0, 6.5, 9.5, 9.0, 1.2),
            p(&[(16.0, 12.5), (17.75, 9.5), (19.5, 12.5)]),
        ],
        "cursor-pointer" => vec![pointer(7.5, 3.0)],
        // The I-beam that says "text goes here".
        "cursor-text" => vec![
            p(&[(12.0, 4.0), (12.0, 20.0)]),
            p(&[(9.0, 4.0), (15.0, 4.0)]),
            p(&[(9.0, 20.0), (15.0, 20.0)]),
        ],
        // Picked up, and the target it is going to.
        "drag-drop" => vec![
            rrf(2.5, 2.5, 8.0, 8.0, 1.0),
            p(&[(11.5, 6.5), (16.0, 11.0)]),
            p(&[(13.4, 10.6), (16.4, 11.4), (15.6, 8.4)]),
            rr(13.0, 13.0, 9.0, 9.0, 1.0),
            p(&[(15.5, 17.5), (19.5, 17.5)]),
            p(&[(17.5, 15.5), (17.5, 19.5)]),
        ],
        "click" => vec![
            pointer(9.0, 8.0),
            p(&[(6.5, 6.0), (5.0, 4.0)]),
            p(&[(11.0, 5.0), (11.0, 2.5)]),
            p(&[(15.5, 6.0), (17.0, 4.0)]),
        ],
        "double-click" => vec![
            pointer(9.0, 8.0),
            p(&[(6.5, 6.0), (5.0, 4.0)]),
            p(&[(11.0, 5.0), (11.0, 2.5)]),
            p(&[(15.5, 6.0), (17.0, 4.0)]),
            p(&[(4.0, 8.5), (2.0, 7.5)]),
            p(&[(18.0, 8.5), (20.0, 7.5)]),
        ],
        // Held down: the pointer, and the time it is waiting out.
        "long-press" => vec![
            pointer(6.0, 8.0),
            a(13.5, 6.0, 4.5, 270.0, 540.0),
            p(&[(13.5, 6.0), (13.5, 3.0)]),
        ],
        // A finger, going across.
        "swipe" => vec![
            path(vec![L(3.0, 8.0), Q(12.0, 3.0, 21.0, 8.0)]),
            p(&[(18.6, 5.8), (21.4, 8.2), (18.3, 9.6)]),
            p(&[(12.0, 12.0), (12.0, 20.5)]),
            p(&[(9.0, 14.5), (9.0, 20.5)]),
            p(&[(15.0, 14.5), (15.0, 20.5)]),
        ],
        // The ring that says the keyboard is here.
        "focus-ring" => vec![
            rr(6.0, 8.5, 12.0, 7.0, 1.2),
            rr(2.5, 5.0, 19.0, 14.0, 2.4),
            p(&[(2.5, 12.0), (2.5, 12.0)]),
        ],

        // ── Layout ─────────────────────────────────────────────────────────
        // Which sheet is on top.
        "z-index" => vec![
            rr(2.0, 11.0, 12.0, 9.0, 1.0),
            rr(6.0, 7.5, 12.0, 9.0, 1.0),
            rr(10.0, 4.0, 12.0, 9.0, 1.0),
        ],
        // A row that shares out the space it is given.
        "flex-layout" => vec![
            rr(1.5, 5.0, 21.0, 14.0, 1.2),
            rrf(3.5, 7.5, 4.0, 9.0, 0.6),
            rrf(10.0, 7.5, 4.0, 9.0, 0.6),
            rrf(16.5, 7.5, 4.0, 9.0, 0.6),
            p(&[(7.5, 20.5), (10.0, 20.5)]),
            p(&[(14.0, 20.5), (16.5, 20.5)]),
        ],
        // Columns and gutters — the grid you design ON, not a view mode.
        "grid-layout" => vec![
            rr(1.5, 3.5, 21.0, 17.0, 1.2),
            rrf(3.5, 6.0, 3.5, 12.0, 0.5),
            rrf(9.0, 6.0, 3.5, 12.0, 0.5),
            rrf(14.5, 6.0, 3.5, 12.0, 0.5),
            p(&[(20.5, 6.0), (20.5, 18.0)]),
        ],
        // The gap kept INSIDE the box.
        "padding" => vec![
            rr(2.0, 2.0, 20.0, 20.0, 1.2),
            rrf(7.0, 7.0, 10.0, 10.0, 0.8),
            p(&[(12.0, 2.5), (12.0, 6.5)]),
            p(&[(12.0, 17.5), (12.0, 21.5)]),
            p(&[(2.5, 12.0), (6.5, 12.0)]),
            p(&[(17.5, 12.0), (21.5, 12.0)]),
        ],
        // The gap kept OUTSIDE it.
        "margin" => vec![
            rr(6.5, 6.5, 11.0, 11.0, 1.0),
            p(&[(2.0, 1.5), (6.0, 1.5)]),
            p(&[(1.5, 2.0), (1.5, 6.0)]),
            p(&[(18.0, 1.5), (22.0, 1.5)]),
            p(&[(22.5, 2.0), (22.5, 6.0)]),
            p(&[(2.0, 22.5), (6.0, 22.5)]),
            p(&[(1.5, 18.0), (1.5, 22.0)]),
            p(&[(18.0, 22.5), (22.0, 22.5)]),
            p(&[(22.5, 18.0), (22.5, 22.0)]),
        ],
        // The corner itself, with the radius that made it.
        "border-radius" => vec![
            path(vec![L(3.5, 20.5), L(3.5, 10.0), Q(3.5, 3.5, 10.0, 3.5), L(20.5, 3.5)]),
            p(&[(10.0, 10.0), (10.0, 3.5)]),
            p(&[(10.0, 10.0), (3.5, 10.0)]),
            d(10.0, 10.0, 1.2),
            d(3.5, 20.5, 1.2),
            d(20.5, 3.5, 1.2),
        ],
        "drop-shadow" => vec![
            rrf(8.0, 8.0, 13.0, 13.0, 1.5),
            rr(3.0, 3.0, 13.0, 13.0, 1.5),
        ],
        // Solid over see-through.
        "opacity" => vec![
            c(12.0, 12.0, 9.0),
            p(&[(12.0, 3.0), (12.0, 21.0)]),
            rrf(4.0, 7.5, 4.0, 4.0, 0.3),
            rrf(8.0, 11.5, 4.0, 4.0, 0.3),
            rrf(4.0, 15.5, 4.0, 4.0, 0.3),
        ],
        // One colour running into another.
        "gradient" => vec![
            rr(2.5, 2.5, 19.0, 19.0, 1.5),
            rrf(2.5, 2.5, 19.0, 5.0, 1.5),
            p(&[(2.5, 9.5), (21.5, 9.5)]),
            p(&[(4.5, 13.0), (19.5, 13.0)]),
            p(&[(7.5, 16.5), (16.5, 16.5)]),
            p(&[(10.5, 20.0), (13.5, 20.0)]),
        ],
        // The lines you align to, not the ones you ship.
        "guides" => vec![
            rr(2.5, 2.5, 19.0, 19.0, 1.2),
            p(&[(9.0, 2.5), (9.0, 21.5)]),
            p(&[(2.5, 15.5), (21.5, 15.5)]),
            rr(11.5, 5.5, 7.0, 7.0, 0.6),
        ],
        "ruler" => vec![
            rr(1.5, 8.0, 21.0, 8.0, 1.0),
            p(&[(5.0, 8.0), (5.0, 12.5)]),
            p(&[(8.0, 8.0), (8.0, 10.5)]),
            p(&[(11.0, 8.0), (11.0, 12.5)]),
            p(&[(14.0, 8.0), (14.0, 10.5)]),
            p(&[(17.0, 8.0), (17.0, 12.5)]),
            p(&[(20.0, 8.0), (20.0, 10.5)]),
        ],
        // The named canvas a design lives on.
        "artboard" => vec![
            p(&[(6.0, 1.5), (6.0, 22.5)]),
            p(&[(17.0, 1.5), (17.0, 22.5)]),
            p(&[(1.5, 6.0), (22.5, 6.0)]),
            p(&[(1.5, 17.0), (22.5, 17.0)]),
        ],
        // What the user can actually see of it.
        "viewport" => vec![
            p(&[(2.5, 8.0), (2.5, 2.5), (8.0, 2.5)]),
            p(&[(16.0, 2.5), (21.5, 2.5), (21.5, 8.0)]),
            p(&[(21.5, 16.0), (21.5, 21.5), (16.0, 21.5)]),
            p(&[(8.0, 21.5), (2.5, 21.5), (2.5, 16.0)]),
            rr(8.5, 8.5, 7.0, 7.0, 0.8),
        ],
        // The dots a control lands on whether you aimed or not.
        "snap-grid" => {
            let mut v = Vec::new();
            for j in 0..5 {
                for i in 0..5 {
                    v.push(d(3.0 + i as f32 * 4.5, 3.0 + j as f32 * 4.5, 0.75));
                }
            }
            v.push(rr(7.5, 7.5, 9.0, 9.0, 0.8));
            v
        }

        _ => return None,
    })
}

// ── National flags ──────────────────────────────────────────────────────────
//
// ⚠️ Read this before adding one.
//
// These are LINE drawings on the same monochrome grid as every other icon: one
// tint colour, at most one accent, no per-shape colour. A national flag is
// mostly defined by its COLOURS, so a great many of them reduce to the same
// drawing here — Italy, Ireland, France, Belgium, Romania, Chad, Mali and Guinea
// are all "three vertical bands" and are genuinely indistinguishable. That was
// put to the operator (2026-08-17) and they asked for the full set anyway, so
// the collisions are deliberate and known, not an oversight to be reported.
//
// What CAN be told apart is geometry: band direction and count, a canton, a
// cross, a saltire, a hoist triangle, a disc, a crescent, a star. Every flag
// below is composed from those, so the ones that differ, differ honestly.

/// The flag field: a 19×13 rectangle (close to the common 3:2), centred.
const FLAG_X: f32 = 2.5;
const FLAG_Y: f32 = 5.5;
const FLAG_W: f32 = 19.0;
const FLAG_H: f32 = 13.0;

fn flag_field() -> IconShape {
    rr(FLAG_X, FLAG_Y, FLAG_W, FLAG_H, 0.8)
}

/// The field, plus whatever is on it.
fn flag(charges: Vec<IconShape>) -> Vec<IconShape> {
    let mut v = vec![flag_field()];
    v.extend(charges);
    v
}

/// `n` vertical bands — the `n - 1` dividing lines between them.
fn vbands(n: usize) -> Vec<IconShape> {
    (1..n)
        .map(|i| {
            let x = FLAG_X + FLAG_W * i as f32 / n as f32;
            p(&[(x, FLAG_Y), (x, FLAG_Y + FLAG_H)])
        })
        .collect()
}

/// `n` horizontal bands.
fn hbands(n: usize) -> Vec<IconShape> {
    (1..n)
        .map(|i| {
            let y = FLAG_Y + FLAG_H * i as f32 / n as f32;
            p(&[(FLAG_X, y), (FLAG_X + FLAG_W, y)])
        })
        .collect()
}

/// A canton in the upper hoist, sized as a fraction of the field.
fn canton(wf: f32, hf: f32) -> IconShape {
    rr(FLAG_X, FLAG_Y, FLAG_W * wf, FLAG_H * hf, 0.4)
}

/// A charge centred on the field, at (dx, dy) from its centre.
fn flag_centre(dx: f32, dy: f32) -> (f32, f32) {
    (FLAG_X + FLAG_W / 2.0 + dx, FLAG_Y + FLAG_H / 2.0 + dy)
}

/// A filled disc on the field.
fn fdisc(dx: f32, dy: f32, r: f32) -> IconShape {
    let (cx, cy) = flag_centre(dx, dy);
    d(cx, cy, r)
}

/// An outlined disc on the field.
fn fring(dx: f32, dy: f32, r: f32) -> IconShape {
    let (cx, cy) = flag_centre(dx, dy);
    c(cx, cy, r)
}

/// A five-pointed star, filled, centred at (dx, dy) from the field centre.
fn fstar(dx: f32, dy: f32, r: f32) -> IconShape {
    let (cx, cy) = flag_centre(dx, dy);
    star_at(cx, cy, r)
}

/// A five-pointed star anywhere on the grid.
fn star_at(cx: f32, cy: f32, r: f32) -> IconShape {
    let mut pts = Vec::with_capacity(10);
    for i in 0..10 {
        // Point up: start at -90°, alternating outer/inner radius.
        let ang = (-90.0 + i as f32 * 36.0).to_radians();
        let rad = if i % 2 == 0 { r } else { r * 0.42 };
        pts.push((cx + rad * ang.cos(), cy + rad * ang.sin()));
    }
    IconShape::FillPath(pts.into_iter().map(|(x, y)| L(x, y)).collect())
}

/// A crescent opening toward the fly, with its companion star.
fn crescent_star(dx: f32) -> Vec<IconShape> {
    let (cx, cy) = flag_centre(dx, 0.0);
    vec![
        IconShape::Arc(cx, cy, 3.6, 55.0, 305.0),
        IconShape::Arc(cx + 1.3, cy, 2.9, 300.0, 60.0),
        star_at(cx + 5.0, cy, 1.5),
    ]
}

/// A centred cross reaching the edges.
fn centred_cross() -> Vec<IconShape> {
    let (cx, cy) = flag_centre(0.0, 0.0);
    vec![
        p(&[(cx, FLAG_Y), (cx, FLAG_Y + FLAG_H)]),
        p(&[(FLAG_X, cy), (FLAG_X + FLAG_W, cy)]),
    ]
}

/// A Nordic cross — the upright shifted toward the hoist.
fn nordic_cross() -> Vec<IconShape> {
    let x = FLAG_X + FLAG_W * 0.36;
    let cy = FLAG_Y + FLAG_H / 2.0;
    vec![
        p(&[(x, FLAG_Y), (x, FLAG_Y + FLAG_H)]),
        p(&[(FLAG_X, cy), (FLAG_X + FLAG_W, cy)]),
    ]
}

/// A saltire — corner to corner, both ways.
fn saltire() -> Vec<IconShape> {
    vec![
        p(&[(FLAG_X, FLAG_Y), (FLAG_X + FLAG_W, FLAG_Y + FLAG_H)]),
        p(&[(FLAG_X + FLAG_W, FLAG_Y), (FLAG_X, FLAG_Y + FLAG_H)]),
    ]
}

/// A triangle based on the hoist, reaching `depth` of the way across.
fn hoist_triangle(depth: f32) -> IconShape {
    pf(&[
        (FLAG_X, FLAG_Y),
        (FLAG_X + FLAG_W * depth, FLAG_Y + FLAG_H / 2.0),
        (FLAG_X, FLAG_Y + FLAG_H),
    ])
}

/// A band along the hoist, `wf` of the field wide.
fn hoist_band(wf: f32) -> IconShape {
    let x = FLAG_X + FLAG_W * wf;
    p(&[(x, FLAG_Y), (x, FLAG_Y + FLAG_H)])
}

/// National flags, `flag-<ISO 3166-1 alpha-2>`. Chunk 13.
///
/// See the note above [`flag_field`] for why many of these share a drawing.
#[rustfmt::skip]
fn flag_shapes(name: &str) -> Option<Vec<IconShape>> {
    // A short cross that stops before the edges — Switzerland, Denmark's
    // neighbours in style, and every "couped" cross.
    let couped_cross = || -> Vec<IconShape> {
        let (cx, cy) = flag_centre(0.0, 0.0);
        vec![
            p(&[(cx, cy - 4.0), (cx, cy + 4.0)]),
            p(&[(cx - 4.0, cy), (cx + 4.0, cy)]),
        ]
    };
    // The six-pointed star of Israel, as two crossed triangles.
    let hexagram = || -> Vec<IconShape> {
        let (cx, cy) = flag_centre(0.0, 0.0);
        let r = 3.2;
        vec![
            pc(&[(cx, cy - r), (cx + r * 0.866, cy + r * 0.5), (cx - r * 0.866, cy + r * 0.5)]),
            pc(&[(cx, cy + r), (cx + r * 0.866, cy - r * 0.5), (cx - r * 0.866, cy - r * 0.5)]),
        ]
    };
    // A striped field (USA, Greece, Malaysia, Uruguay, Thailand). Capped at 5
    // in practice: the field is 13 units tall and the stroke is 1.5, so seven
    // stripes very nearly touch and nine fill in solid at a menu row's size.
    // Nobody can count stripes on a 16 px icon anyway — what has to survive is
    // "striped", and the canton is what separates these from one another.
    let stripes = |n: usize| -> Vec<IconShape> {
        (1..n)
            .map(|i| {
                let y = FLAG_Y + FLAG_H * i as f32 / n as f32;
                p(&[(FLAG_X, y), (FLAG_X + FLAG_W, y)])
            })
            .collect()
    };

    Some(match name {
        // ── Europe ─────────────────────────────────────────────────────────
        "flag-fr" | "flag-it" | "flag-ie" | "flag-be" | "flag-ro" | "flag-md"
        | "flag-ad" => flag(vbands(3)),
        "flag-de" | "flag-nl" | "flag-ru" | "flag-hu" | "flag-bg" | "flag-ee"
        | "flag-lt" | "flag-lu" | "flag-rs" | "flag-si" | "flag-sk" | "flag-hr" => flag(hbands(3)),
        "flag-at" | "flag-lv" => flag(hbands(3)),
        "flag-pl" | "flag-ua" | "flag-mc" | "flag-sm" | "flag-by" => flag(hbands(2)),
        "flag-pt" => { let mut v = flag(vec![hoist_band(0.4)]); v.push(fring(-1.5, 0.0, 2.4)); v }
        "flag-es" => { let mut v = flag(hbands(3)); v.push(fring(-4.0, 0.0, 1.8)); v }
        "flag-dk" | "flag-no" | "flag-se" | "flag-fi" | "flag-is" => flag(nordic_cross()),
        "flag-ch" => flag(couped_cross()),
        "flag-gb" => { let mut v = flag(saltire()); v.extend(centred_cross()); v }
        "flag-gr" => { let mut v = flag(stripes(5)); v.push(canton(0.42, 0.55)); v }
        "flag-cy" | "flag-xk" => flag(vec![fring(0.0, -1.0, 2.6), fstar(0.0, 3.0, 1.2)]),
        "flag-al" | "flag-me" | "flag-mk" => flag(vec![fdisc(0.0, 0.0, 3.0)]),
        "flag-ba" => flag(vec![pf(&[(9.0, FLAG_Y), (18.0, FLAG_Y), (9.0, FLAG_Y + FLAG_H)])]),
        "flag-cz" => flag(vec![
            p(&[(FLAG_X, FLAG_Y + FLAG_H / 2.0), (FLAG_X + FLAG_W, FLAG_Y + FLAG_H / 2.0)]),
            hoist_triangle(0.42),
        ]),
        "flag-mt" => { let mut v = flag(vec![hoist_band(0.5)]); v.push(fring(-6.0, -3.5, 1.2)); v }
        "flag-li" => { let mut v = flag(hbands(2)); v.push(fring(-6.0, -3.5, 1.2)); v }
        "flag-va" => { let mut v = flag(vec![hoist_band(0.5)]); v.extend(couped_cross()); v }

        // ── Asia ───────────────────────────────────────────────────────────
        "flag-jp" | "flag-bd" | "flag-pw" | "flag-la" => flag(vec![fdisc(0.0, 0.0, 3.2)]),
        "flag-in" | "flag-ne" => { let mut v = flag(hbands(3)); v.push(fring(0.0, 0.0, 2.2)); v }
        "flag-id" | "flag-sg" => flag(hbands(2)),
        "flag-cn" => flag(vec![
            fstar(-6.0, -3.0, 2.0),
            fstar(-2.6, -4.6, 0.9), fstar(-1.4, -2.9, 0.9),
            fstar(-1.6, -0.9, 0.9), fstar(-3.2, 0.4, 0.9),
        ]),
        "flag-vn" | "flag-ma" | "flag-so" | "flag-tl" => flag(vec![fstar(0.0, 0.0, 3.2)]),
        "flag-kr" => flag(vec![
            fring(0.0, 0.0, 2.8),
            path(vec![L(9.5, 12.0), Q(11.0, 9.6, 12.0, 12.0), Q(13.0, 14.4, 14.5, 12.0)]),
            p(&[(4.8, 8.2), (7.0, 9.6)]), p(&[(17.0, 8.2), (19.2, 9.6)]),
            p(&[(4.8, 15.8), (7.0, 14.4)]), p(&[(17.0, 15.8), (19.2, 14.4)]),
        ]),
        "flag-kp" => { let mut v = flag(stripes(5)); v.push(fring(-4.0, 0.0, 1.8)); v }
        "flag-il" => { let mut v = flag(vec![
            p(&[(FLAG_X, FLAG_Y + 2.2), (FLAG_X + FLAG_W, FLAG_Y + 2.2)]),
            p(&[(FLAG_X, FLAG_Y + FLAG_H - 2.2), (FLAG_X + FLAG_W, FLAG_Y + FLAG_H - 2.2)]),
        ]); v.extend(hexagram()); v }
        "flag-tr" | "flag-tn" | "flag-dz" | "flag-mr" | "flag-az" | "flag-tm"
        | "flag-uz" | "flag-mv" | "flag-pk" | "flag-ly" | "flag-km" => flag(crescent_star(0.0)),
        "flag-sa" | "flag-af" | "flag-iq" | "flag-ir" | "flag-eg" | "flag-sy"
        | "flag-ye" | "flag-jo" | "flag-ps" | "flag-kw" | "flag-ae" | "flag-sd" => flag(hbands(3)),
        "flag-qa" | "flag-bh" => flag(vec![p(&[
            (FLAG_X + 6.0, FLAG_Y), (FLAG_X + 4.0, FLAG_Y + 2.2),
            (FLAG_X + 6.0, FLAG_Y + 4.3), (FLAG_X + 4.0, FLAG_Y + 6.5),
            (FLAG_X + 6.0, FLAG_Y + 8.7), (FLAG_X + 4.0, FLAG_Y + 10.8),
            (FLAG_X + 6.0, FLAG_Y + FLAG_H),
        ])]),
        "flag-om" | "flag-ge" => { let mut v = flag(hbands(3)); v.push(hoist_band(0.28)); v }
        "flag-lb" => { let mut v = flag(vec![
            p(&[(FLAG_X, FLAG_Y + 3.2), (FLAG_X + FLAG_W, FLAG_Y + 3.2)]),
            p(&[(FLAG_X, FLAG_Y + FLAG_H - 3.2), (FLAG_X + FLAG_W, FLAG_Y + FLAG_H - 3.2)]),
        ]); v.push(pf(&[(12.0, 8.8), (14.4, 13.2), (9.6, 13.2)])); v }
        "flag-lk" | "flag-bt" | "flag-bn" => flag(vec![
            rr(FLAG_X + 6.0, FLAG_Y + 2.0, 12.0, 9.0, 0.5),
        ]),
        "flag-np" => vec![
            pathc(vec![
                L(4.5, 3.0), L(18.0, 10.0), L(9.0, 10.0),
                L(18.0, 17.5), L(4.5, 17.5),
            ]),
            fdisc(-4.0, -3.4, 1.1),
            star_at(9.0, 15.0, 1.4),
        ],
        "flag-my" | "flag-ph" => { let mut v = flag(stripes(5)); v.push(canton(0.45, 0.5)); v }
        "flag-th" | "flag-cr" | "flag-kh" | "flag-mm" => flag(stripes(5)),
        "flag-mn" | "flag-am" | "flag-kg" | "flag-tj" => flag(vbands(3)),
        "flag-kz" | "flag-vc" => flag(vec![fdisc(0.0, -1.0, 2.0), fstar(0.0, 3.2, 1.2)]),

        // ── Africa ─────────────────────────────────────────────────────────
        "flag-td" | "flag-ml" | "flag-gn" | "flag-ci" | "flag-ng" | "flag-sn"
        | "flag-cm" | "flag-ga" | "flag-bj" | "flag-gw" | "flag-bf" => flag(vbands(3)),
        "flag-gh" | "flag-et" | "flag-sl" | "flag-mw" | "flag-gm"
        | "flag-bw" | "flag-ls" | "flag-ug" | "flag-rw" | "flag-bi" | "flag-cg"
        | "flag-cf" | "flag-tg" => flag(hbands(3)),
        "flag-za" | "flag-mz" | "flag-tz" | "flag-cd" | "flag-na"
        | "flag-st" | "flag-dj" | "flag-ss" | "flag-er" | "flag-sz" | "flag-zw"
        | "flag-zm" | "flag-ke" | "flag-ao" | "flag-mg" | "flag-gq" | "flag-lr"
        | "flag-mu" | "flag-sc" | "flag-cv" => {
            let mut v = flag(hbands(3)); v.push(hoist_triangle(0.38)); v
        }

        // ── Americas ───────────────────────────────────────────────────────
        "flag-us" => { let mut v = flag(stripes(5)); v.push(canton(0.4, 0.54)); v }
        "flag-ca" | "flag-pe" => { let mut v = flag(vec![hoist_band(0.25), hoist_band(0.75)]); v.push(fdisc(0.0, 0.0, 2.4)); v }
        "flag-br" => flag(vec![
            pc(&[(12.0, 7.0), (19.5, 12.0), (12.0, 17.0), (4.5, 12.0)]),
            fring(0.0, 0.0, 2.6),
        ]),
        "flag-ar" | "flag-hn" | "flag-ni" | "flag-sv" | "flag-gt" => {
            let mut v = flag(hbands(3)); v.push(fdisc(0.0, 0.0, 2.0)); v
        }
        "flag-mx" | "flag-co" | "flag-ve" | "flag-ec" | "flag-bo" => {
            let mut v = flag(hbands(3)); v.push(fring(0.0, 0.0, 2.0)); v
        }
        "flag-cl" | "flag-cu" | "flag-tt" => {
            let mut v = flag(hbands(2)); v.push(canton(0.36, 0.5)); v.push(fstar(-6.0, -3.2, 1.3)); v
        }
        "flag-uy" => { let mut v = flag(stripes(5)); v.push(canton(0.42, 0.55)); v }
        "flag-jm" => flag(saltire()),
        "flag-py" | "flag-do" => { let mut v = flag(hbands(3)); v.extend(centred_cross()); v }
        "flag-bs" | "flag-gy" | "flag-bz" | "flag-pa" | "flag-ht"
        | "flag-sr" | "flag-ag" | "flag-bb" | "flag-dm" | "flag-gd" | "flag-kn"
        | "flag-lc" => { let mut v = flag(hbands(3)); v.push(hoist_triangle(0.38)); v }

        // ── Oceania ────────────────────────────────────────────────────────
        "flag-au" | "flag-nz" | "flag-fj" | "flag-tv" => {
            let mut v = flag(vec![canton(0.45, 0.5)]);
            v.push(fstar(4.0, 1.5, 1.4)); v.push(fstar(6.5, -2.5, 1.0));
            v
        }
        "flag-pg" => flag(vec![
            p(&[(FLAG_X, FLAG_Y), (FLAG_X + FLAG_W, FLAG_Y + FLAG_H)]),
            fstar(4.0, 2.0, 1.3),
        ]),
        "flag-ws" | "flag-to" => { let mut v = flag(vec![canton(0.45, 0.5)]); v.push(fstar(-4.5, -3.0, 1.4)); v }
        "flag-ki" | "flag-nr" | "flag-mh" | "flag-sb" | "flag-vu" | "flag-fm" => {
            let mut v = flag(vec![p(&[(FLAG_X, FLAG_Y + FLAG_H), (FLAG_X + FLAG_W, FLAG_Y)])]);
            v.push(fstar(-4.5, 2.5, 1.3));
            v
        }

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
mod sidebar_toggle_tests {
    use super::*;

    /// The icon grid is 24 wide, so this is its centre line.
    const CENTRE: f32 = 12.0;

    /// The x coordinates of a polyline shape's points.
    fn xs(shape: &IconShape) -> Vec<f32> {
        let ops = match shape {
            IconShape::Stroke(ops) | IconShape::StrokeClosed(ops) | IconShape::FillPath(ops) => ops,
            _ => return Vec::new(),
        };
        ops.iter()
            .filter_map(|op| match op {
                L(x, _) => Some(*x),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_rail_is_drawn_on_the_left_where_the_sidebar_actually_is() {
        // Mirrored, this icon claimed a right-hand sidebar the shell has never
        // had — and it was wrong in BOTH pane states, so nothing gave it away.
        for name in ["sidebar-expand", "sidebar-collapse"] {
            let shapes = base_shapes(name).unwrap_or_else(|| panic!("{name} has no drawing"));
            let divider = xs(&shapes[1]);
            assert_eq!(divider.len(), 2, "{name}: the divider is one straight line");
            assert!(
                divider.iter().all(|x| *x < CENTRE),
                "{name}: the rail must sit left of centre, got {divider:?}"
            );
        }
    }

    #[test]
    fn each_arrow_points_the_way_its_rail_will_move() {
        // The arrow shows the NEXT action: a collapsed left rail opens to the
        // right, an open one closes to the left.
        let expand = xs(&base_shapes("sidebar-expand").expect("expand")[2]);
        let tip = expand.iter().copied().fold(f32::MIN, f32::max);
        assert_eq!(
            expand.iter().filter(|x| **x == tip).count(),
            1,
            "expand must point right (one rightmost tip), got {expand:?}"
        );

        let collapse = xs(&base_shapes("sidebar-collapse").expect("collapse")[2]);
        let tip = collapse.iter().copied().fold(f32::MAX, f32::min);
        assert_eq!(
            collapse.iter().filter(|x| **x == tip).count(),
            1,
            "collapse must point left (one leftmost tip), got {collapse:?}"
        );
    }

    #[test]
    fn the_arrow_sits_in_the_wide_pane_never_on_the_rail() {
        for name in ["sidebar-expand", "sidebar-collapse"] {
            let shapes = base_shapes(name).unwrap_or_else(|| panic!("{name} has no drawing"));
            let divider = xs(&shapes[1])[0];
            let arrow = xs(&shapes[2]);
            assert!(
                arrow.iter().all(|x| *x > divider),
                "{name}: the arrow belongs to the content pane, got {arrow:?} against rail {divider}"
            );
        }
    }
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

    /// EVERY control the toolbox offers has an icon, derived by one rule:
    /// `control-` + the kebab-case of the `ControlType` name.
    ///
    /// This is the invariant the operator actually asked for — "we do not have
    /// icons to represent PowerRustCOBOL own controls" (2026-08-31) — and it
    /// is the one that rots silently: control number 43 gets added to
    /// `ControlType::ALL`, the toolbox grows a drawing for it, and the
    /// catalogue quietly does not. Deriving the name here rather than listing
    /// it means a new control fails this test until it has an icon.
    #[test]
    fn every_control_type_has_an_icon() {
        fn kebab(camel: &str) -> String {
            let mut out = String::new();
            for (i, ch) in camel.chars().enumerate() {
                if ch.is_ascii_uppercase() && i > 0 {
                    out.push('-');
                }
                out.push(ch.to_ascii_lowercase());
            }
            out
        }
        let catalogue: std::collections::HashSet<&str> = menu_icon_names().collect();
        let mut missing = Vec::new();
        for ct in crate::model::ControlType::ALL {
            let name = format!("control-{}", kebab(ct.as_str()));
            if !catalogue.contains(name.as_str()) || icon_shapes(&name).is_none() {
                missing.push(name);
            }
        }
        assert!(
            missing.is_empty(),
            "{} control(s) have no catalogue icon: {missing:?}",
            missing.len()
        );
        // `Custom` is not in `ControlType::ALL` (plugin controls are found at
        // run time), but a plugin control still needs something to draw.
        assert!(
            catalogue.contains("control-custom"),
            "a plugin-provided control needs an icon too"
        );
        eprintln!(
            "control icons — {}/{} ControlType::ALL entries covered, plus control-custom",
            crate::model::ControlType::ALL.len(),
            crate::model::ControlType::ALL.len()
        );
    }

    /// The three sets added on 2026-08-31, counted — and every name in them
    /// still drawable. The counts are the report the operator reads; a drop
    /// means something was deleted, which this catalogue never does.
    #[test]
    fn the_new_sets_are_complete() {
        let count = |cat: &str| -> usize {
            MENU_ICON_CATEGORIES
                .iter()
                .find(|(c, _)| *c == cat)
                .map(|(_, n)| n.len())
                .unwrap_or(0)
        };
        let controls = count("PowerRustCOBOL Controls");
        let cs = count("Computer Science");
        let ui = count("User Interface");
        assert_eq!(controls, 43, "42 ControlType::ALL entries + Custom");
        assert!(cs >= 79, "computer-science set, got {cs}");
        assert!(ui >= 49, "user-interface set, got {ui}");

        // Nothing in the three new sets may collide with a name that was
        // already there — the picker searches by name, so a collision would
        // silently retarget an existing icon.
        let mut seen = std::collections::HashSet::new();
        for name in menu_icon_names() {
            assert!(seen.insert(name), "duplicate name reached the catalogue: {name}");
        }
        let total: usize = MENU_ICON_CATEGORIES.iter().map(|(_, n)| n.len()).sum();
        eprintln!(
            "new icon sets — controls {controls} · computer science {cs} · \
             user interface {ui} = {} added, catalogue now {total}",
            controls + cs + ui
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
