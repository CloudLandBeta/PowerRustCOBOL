// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Model Leaderboard — **visual prototype** (pre-spec).
//!
//! A throwaway, self-contained eframe app that shows the proposed Model
//! Leaderboard so its layout can be judged *before* any of it is wired into the
//! IDE. Nothing here reads or writes project state: every number is invented
//! sample data (see [`sample_entries`]), and the window says so.
//!
//! What it demonstrates:
//!   * a movable window holding the whole board;
//!   * the four boards — Overall, Cloud (free), Cloud (paid), Local — as tabs;
//!   * the report row `Rank | Model | Provider | Overall evaluation ***** |
//!     Details | Run tests | Apply to Grace | Apply to Specialists`;
//!   * the Details modal with the full per-model metric sheet;
//!   * the failure path: a model whose test cannot run shows an error window,
//!     loses its stars, and drops to the bottom of the list.
//!
//! Run it with:
//!   `cargo run -p cobolt-ide --example leaderboard_prototype`
//!
//! Strings are hardcoded English on purpose — a prototype is not shipped UI.
//! The real panel takes `Tr` keys in all six languages.

use eframe::egui;
use egui::{Color32, RichText};

// The IDE is a binary-only crate, so the example cannot `use cobolt_ide::…`.
// Including the real module keeps the prototype on the *actual* shipped
// palettes rather than a copy that drifts.
// The prototype uses a handful of the module's palettes and helpers; the rest
// (editor syntax colours, neumorphic relief) is dead code *here* only.
#[path = "../src/theme.rs"]
#[allow(dead_code, unused_imports)]
mod theme;
use theme::Theme;

// ── palette ────────────────────────────────────────────────────────────────
// Same family the benchmark report already uses in `app.rs` (deep navy panel,
// cornflower rim, pale-blue field labels) so the prototype reads as PowerRustCOBOL.
/// Every colour the prototype paints, derived from an IDE [`Theme`] so the
/// board follows whichever palette the project is on. Nothing here is a fixed
/// RGB: a hardcoded navy would be invisible on Classic and glaring on Light+.
#[derive(Clone, Copy)]
struct Pal {
    /// Card surface behind the board.
    card: Color32,
    rim: Color32,
    /// Primary reading colour.
    text: Color32,
    /// Small-caps captions and column headers — the theme accent, which every
    /// palette keeps vivid against its own surfaces.
    label: Color32,
    good: Color32,
    warn: Color32,
    bad: Color32,
    star_on: Color32,
    star_off: Color32,
}

impl Pal {
    fn of(th: &Theme) -> Self {
        Self {
            card: th.bg_panel,
            rim: th.panel_border(),
            text: th.text_bright,
            label: th.accent,
            // No theme carries a "good" colour, but every one carries a data
            // colour that is legible on its own surfaces — the green/teal of
            // the syntax palette.
            good: th.ed_data,
            warn: th.warn,
            bad: th.error,
            star_on: th.warn,
            star_off: theme::darken(th.text_dim, 0.45),
        }
    }
}

// ── type scale ─────────────────────────────────────────────────────────────
// Nothing renders below `SZ_SMALL`; headings use `SZ_TITLE`.
const SZ_TITLE: f32 = 18.0;
const SZ_BODY: f32 = 13.0;
const SZ_SMALL: f32 = 12.0;

// ── fixed geometry ─────────────────────────────────────────────────────────
// Every pane size here is a constant on purpose. Deriving a child's height from
// `available_height()` inside a resizable container is what makes egui panes
// inflate a little more every frame.
const BODY_H: f32 = 610.0;
/// Wide enough for the four row buttons without a horizontal scrollbar.
const TABLE_W: f32 = 900.0;
/// Padding inside the card.
const CARD_PAD: i8 = 8;

// ── model ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tier {
    CloudFree,
    CloudPaid,
    Local,
}

impl Tier {
    fn label(self) -> &'static str {
        match self {
            Tier::CloudFree => "Cloud · free",
            Tier::CloudPaid => "Cloud · paid",
            Tier::Local => "Local",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Board {
    Overall,
    CloudFree,
    CloudPaid,
    Local,
}

impl Board {
    const ALL: [Board; 4] = [
        Board::Overall,
        Board::CloudFree,
        Board::CloudPaid,
        Board::Local,
    ];

    fn title(self) -> &'static str {
        match self {
            Board::Overall => "Overall rank",
            Board::CloudFree => "Cloud free models",
            Board::CloudPaid => "Cloud paid models",
            Board::Local => "Local models",
        }
    }

    fn accepts(self, tier: Tier) -> bool {
        match self {
            Board::Overall => true,
            Board::CloudFree => tier == Tier::CloudFree,
            Board::CloudPaid => tier == Tier::CloudPaid,
            Board::Local => tier == Tier::Local,
        }
    }
}

/// One tested model. Scores are 0-100 unless the field says otherwise.
///
/// Provenance matters and is called out in the Details modal: some of these are
/// **model-reported** (the benchmark JSON the model returns about itself), some
/// are **harness-measured** (the IDE timed or counted them), and some come from
/// the **connection probe** run before the benchmark.
#[derive(Clone)]
struct Entry {
    model: &'static str,
    provider: &'static str,
    tier: Tier,
    tested_on: &'static str,

    // Headline + the twelve capability scores behind the KPI squares.
    overall: f32,
    compilation: f32,
    functional: f32,
    cobol85_generation: f32,
    modification: f32,
    debugging: f32,
    refactoring: f32,
    file_handling: f32,
    indexed_files: f32,
    table_driven: f32,
    explanation: f32,
    powerrustcobol: f32,
    type_inference: f32,
    inline_invoke: f32,

    // Harness-measured behaviour.
    reliability: f32,   // % of benchmark rounds that completed without error
    determinism: f32,   // % agreement across repeated runs at temperature 0
    hallucinations: u32, // invented verbs/controls/properties counted by the reviewer
    latency_ms: u32,    // average wall-clock per round
    out_tokens: u32,    // average completion tokens per round

    // Connection probe (collected when "Test connection" succeeds).
    ctx_in: u32,
    ctx_out: u32,

    // Cost / footprint. `None` where the tier makes it meaningless.
    params_b: Option<f32>,          // billions of parameters
    usd_per_mtok_out: Option<f32>,  // price per 1M output tokens
    ram_gb: Option<f32>,            // peak resident memory, local runs only
    hardware: Option<&'static str>, // local only
    quantization: Option<&'static str>,
}

impl Entry {
    fn hallucination_rate(&self) -> f32 {
        // Per 10k output tokens — a raw count favours terse models unfairly.
        self.hallucinations as f32 * 10_000.0 / self.out_tokens.max(1) as f32
    }

    /// Score bought per dollar, when a price is known.
    fn value_per_dollar(&self) -> Option<f32> {
        self.usd_per_mtok_out
            .filter(|c| *c > 0.0)
            .map(|c| self.overall / c)
    }
}

/// Invented data. Shaped to exercise the layout: long model ids, ties near the
/// top, a local model that wins on footprint but not on score, a free model
/// that punches above its price.
fn sample_entries() -> Vec<Entry> {
    vec![
        Entry {
            model: "claude-opus-5",
            provider: "Anthropic",
            tier: Tier::CloudPaid,
            tested_on: "2026-08-01",
            overall: 93.4,
            compilation: 89.0,
            functional: 94.0,
            cobol85_generation: 95.0,
            modification: 92.0,
            debugging: 93.0,
            refactoring: 91.0,
            file_handling: 90.0,
            indexed_files: 92.0,
            table_driven: 94.0,
            explanation: 95.0,
            powerrustcobol: 90.0,
            type_inference: 89.0,
            inline_invoke: 93.0,
            reliability: 100.0,
            determinism: 96.0,
            hallucinations: 1,
            latency_ms: 14_820,
            out_tokens: 6_140,
            ctx_in: 200_000,
            ctx_out: 64_000,
            params_b: None,
            usd_per_mtok_out: Some(75.0),
            ram_gb: None,
            hardware: None,
            quantization: None,
        },
        Entry {
            model: "gpt-5.2-codex",
            provider: "OpenAI",
            tier: Tier::CloudPaid,
            tested_on: "2026-08-01",
            overall: 91.8,
            compilation: 88.0,
            functional: 92.0,
            cobol85_generation: 93.0,
            modification: 90.0,
            debugging: 94.0,
            refactoring: 92.0,
            file_handling: 88.0,
            indexed_files: 87.0,
            table_driven: 91.0,
            explanation: 92.0,
            powerrustcobol: 84.0,
            type_inference: 90.0,
            inline_invoke: 85.0,
            reliability: 98.0,
            determinism: 93.0,
            hallucinations: 2,
            latency_ms: 11_360,
            out_tokens: 5_480,
            ctx_in: 400_000,
            ctx_out: 128_000,
            params_b: None,
            usd_per_mtok_out: Some(40.0),
            ram_gb: None,
            hardware: None,
            quantization: None,
        },
        Entry {
            model: "gemini-3-pro",
            provider: "Google",
            tier: Tier::CloudPaid,
            tested_on: "2026-07-30",
            overall: 88.6,
            compilation: 85.0,
            functional: 89.0,
            cobol85_generation: 90.0,
            modification: 86.0,
            debugging: 88.0,
            refactoring: 87.0,
            file_handling: 91.0,
            indexed_files: 89.0,
            table_driven: 86.0,
            explanation: 93.0,
            powerrustcobol: 78.0,
            type_inference: 86.0,
            inline_invoke: 74.0,
            reliability: 96.0,
            determinism: 88.0,
            hallucinations: 4,
            latency_ms: 8_940,
            out_tokens: 7_020,
            ctx_in: 1_000_000,
            ctx_out: 65_536,
            params_b: None,
            usd_per_mtok_out: Some(18.0),
            ram_gb: None,
            hardware: None,
            quantization: None,
        },
        Entry {
            model: "claude-sonnet-5",
            provider: "Anthropic",
            tier: Tier::CloudPaid,
            tested_on: "2026-07-29",
            overall: 87.9,
            compilation: 86.0,
            functional: 88.0,
            cobol85_generation: 89.0,
            modification: 89.0,
            debugging: 86.0,
            refactoring: 88.0,
            file_handling: 85.0,
            indexed_files: 86.0,
            table_driven: 88.0,
            explanation: 90.0,
            powerrustcobol: 87.0,
            type_inference: 84.0,
            inline_invoke: 88.0,
            reliability: 100.0,
            determinism: 95.0,
            hallucinations: 2,
            latency_ms: 6_720,
            out_tokens: 4_860,
            ctx_in: 200_000,
            ctx_out: 64_000,
            params_b: None,
            usd_per_mtok_out: Some(15.0),
            ram_gb: None,
            hardware: None,
            quantization: None,
        },
        Entry {
            model: "qwen3-coder-480b:free",
            provider: "OpenRouter",
            tier: Tier::CloudFree,
            tested_on: "2026-07-28",
            overall: 82.1,
            compilation: 80.0,
            functional: 83.0,
            cobol85_generation: 85.0,
            modification: 79.0,
            debugging: 81.0,
            refactoring: 80.0,
            file_handling: 78.0,
            indexed_files: 76.0,
            table_driven: 84.0,
            explanation: 82.0,
            powerrustcobol: 61.0,
            type_inference: 79.0,
            inline_invoke: 58.0,
            reliability: 84.0,
            determinism: 87.0,
            hallucinations: 6,
            latency_ms: 19_400,
            out_tokens: 5_960,
            ctx_in: 262_144,
            ctx_out: 8_192,
            params_b: Some(480.0),
            usd_per_mtok_out: None,
            ram_gb: None,
            hardware: None,
            quantization: None,
        },
        Entry {
            model: "deepseek-r1:free",
            provider: "OpenRouter",
            tier: Tier::CloudFree,
            tested_on: "2026-07-28",
            overall: 79.4,
            compilation: 76.0,
            functional: 80.0,
            cobol85_generation: 82.0,
            modification: 77.0,
            debugging: 84.0,
            refactoring: 78.0,
            file_handling: 74.0,
            indexed_files: 72.0,
            table_driven: 80.0,
            explanation: 86.0,
            powerrustcobol: 55.0,
            type_inference: 81.0,
            inline_invoke: 52.0,
            reliability: 72.0,
            determinism: 69.0,
            hallucinations: 9,
            latency_ms: 41_250,
            out_tokens: 12_480,
            ctx_in: 163_840,
            ctx_out: 16_384,
            params_b: Some(671.0),
            usd_per_mtok_out: None,
            ram_gb: None,
            hardware: None,
            quantization: None,
        },
        Entry {
            model: "llama-4-scout:free",
            provider: "HuggingFace",
            tier: Tier::CloudFree,
            tested_on: "2026-07-27",
            overall: 71.2,
            compilation: 70.0,
            functional: 71.0,
            cobol85_generation: 74.0,
            modification: 68.0,
            debugging: 69.0,
            refactoring: 70.0,
            file_handling: 66.0,
            indexed_files: 63.0,
            table_driven: 72.0,
            explanation: 78.0,
            powerrustcobol: 44.0,
            type_inference: 67.0,
            inline_invoke: 41.0,
            reliability: 66.0,
            determinism: 74.0,
            hallucinations: 11,
            latency_ms: 9_880,
            out_tokens: 3_240,
            ctx_in: 128_000,
            ctx_out: 4_096,
            params_b: Some(109.0),
            usd_per_mtok_out: None,
            ram_gb: None,
            hardware: None,
            quantization: None,
        },
        Entry {
            model: "mistral-small-3.2:free",
            provider: "Mistral",
            tier: Tier::CloudFree,
            tested_on: "2026-07-27",
            overall: 68.5,
            compilation: 67.0,
            functional: 69.0,
            cobol85_generation: 71.0,
            modification: 66.0,
            debugging: 65.0,
            refactoring: 67.0,
            file_handling: 62.0,
            indexed_files: 58.0,
            table_driven: 70.0,
            explanation: 74.0,
            powerrustcobol: 39.0,
            type_inference: 64.0,
            inline_invoke: 36.0,
            reliability: 91.0,
            determinism: 82.0,
            hallucinations: 8,
            latency_ms: 4_610,
            out_tokens: 2_780,
            ctx_in: 128_000,
            ctx_out: 8_192,
            params_b: Some(24.0),
            usd_per_mtok_out: None,
            ram_gb: None,
            hardware: None,
            quantization: None,
        },
        Entry {
            model: "qwen2.5-coder:32b",
            provider: "Ollama",
            tier: Tier::Local,
            tested_on: "2026-07-26",
            overall: 77.8,
            compilation: 75.0,
            functional: 78.0,
            cobol85_generation: 80.0,
            modification: 76.0,
            debugging: 74.0,
            refactoring: 77.0,
            file_handling: 73.0,
            indexed_files: 71.0,
            table_driven: 79.0,
            explanation: 76.0,
            powerrustcobol: 58.0,
            type_inference: 75.0,
            inline_invoke: 54.0,
            reliability: 100.0,
            determinism: 98.0,
            hallucinations: 7,
            latency_ms: 27_300,
            out_tokens: 4_120,
            ctx_in: 32_768,
            ctx_out: 8_192,
            params_b: Some(32.0),
            usd_per_mtok_out: None,
            ram_gb: Some(21.4),
            hardware: Some("Apple M3 Max · 48 GB unified"),
            quantization: Some("Q4_K_M"),
        },
        Entry {
            model: "deepseek-coder-v2:16b",
            provider: "Ollama",
            tier: Tier::Local,
            tested_on: "2026-07-26",
            overall: 72.6,
            compilation: 71.0,
            functional: 73.0,
            cobol85_generation: 75.0,
            modification: 70.0,
            debugging: 72.0,
            refactoring: 71.0,
            file_handling: 68.0,
            indexed_files: 66.0,
            table_driven: 74.0,
            explanation: 72.0,
            powerrustcobol: 49.0,
            type_inference: 70.0,
            inline_invoke: 46.0,
            reliability: 100.0,
            determinism: 97.0,
            hallucinations: 9,
            latency_ms: 15_900,
            out_tokens: 3_540,
            ctx_in: 163_840,
            ctx_out: 8_192,
            params_b: Some(16.0),
            usd_per_mtok_out: None,
            ram_gb: Some(11.2),
            hardware: Some("Apple M3 Max · 48 GB unified"),
            quantization: Some("Q4_0"),
        },
        Entry {
            model: "granite-code:8b",
            provider: "Ollama",
            tier: Tier::Local,
            tested_on: "2026-07-25",
            overall: 64.3,
            compilation: 63.0,
            functional: 64.0,
            cobol85_generation: 68.0,
            modification: 61.0,
            debugging: 60.0,
            refactoring: 62.0,
            file_handling: 59.0,
            indexed_files: 57.0,
            table_driven: 66.0,
            explanation: 65.0,
            powerrustcobol: 33.0,
            type_inference: 58.0,
            inline_invoke: 30.0,
            reliability: 100.0,
            determinism: 99.0,
            hallucinations: 12,
            latency_ms: 6_180,
            out_tokens: 2_460,
            ctx_in: 8_192,
            ctx_out: 4_096,
            params_b: Some(8.0),
            usd_per_mtok_out: None,
            ram_gb: Some(5.6),
            hardware: Some("Apple M3 Max · 48 GB unified"),
            quantization: Some("Q5_K_M"),
        },
    ]
}

// ── star rating ────────────────────────────────────────────────────────────

fn paint_star(painter: &egui::Painter, center: egui::Pos2, r: f32, color: Color32) {
    // Fan-triangulated from the centre: a five-point star is concave, so it
    // cannot be handed to `convex_polygon` whole without tearing.
    let mut pts = Vec::with_capacity(10);
    for i in 0..10 {
        let ang = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
        let rad = if i % 2 == 0 { r } else { r * 0.44 };
        pts.push(egui::pos2(
            center.x + rad * ang.cos(),
            center.y + rad * ang.sin(),
        ));
    }
    for i in 0..10 {
        painter.add(egui::Shape::convex_polygon(
            vec![center, pts[i], pts[(i + 1) % 10]],
            color,
            egui::Stroke::NONE,
        ));
    }
}

/// Five stars filled to `fraction` (0..=1), partial stars included.
fn star_rating(ui: &mut egui::Ui, p: Pal, fraction: f32, size: f32) {
    const N: usize = 5;
    let gap = 2.0;
    let width = N as f32 * size + (N as f32 - 1.0) * gap;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, size), egui::Sense::hover());
    let centre_of = |i: usize| {
        egui::pos2(
            rect.left() + size * 0.5 + i as f32 * (size + gap),
            rect.center().y,
        )
    };
    for i in 0..N {
        paint_star(ui.painter(), centre_of(i), size * 0.5, p.star_off);
    }
    let filled = egui::Rect::from_min_size(
        rect.min,
        egui::vec2(width * fraction.clamp(0.0, 1.0), rect.height()),
    );
    let painter = ui.painter().with_clip_rect(filled);
    for i in 0..N {
        paint_star(&painter, centre_of(i), size * 0.5, p.star_on);
    }
}

fn score_color(p: Pal, v: f32) -> Color32 {
    if v >= 85.0 {
        p.good
    } else if v >= 70.0 {
        p.warn
    } else {
        p.bad
    }
}

/// A themed card of an exact size. Both columns are built from this, so the
/// board and the KPI rail are the same height by construction rather than by
/// two numbers that have to be kept in step.
fn card(ui: &mut egui::Ui, p: Pal, size: egui::Vec2, add: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(
        size,
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            egui::Frame::NONE
                .fill(p.card)
                .stroke(egui::Stroke::new(1.0, p.rim))
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(egui::Margin::same(CARD_PAD))
                .show(ui, |ui| {
                    let inner = size - egui::Vec2::splat(2.0 * CARD_PAD as f32 + 2.0);
                    ui.set_min_size(inner);
                    ui.set_max_width(inner.x);
                    add(ui);
                });
        },
    );
}

/// Apply an IDE theme to the whole prototype — the same fields
/// `apply_glass_visuals` maps in `app.rs`, minus the glass/neumorphic relief.
fn apply_theme(ctx: &egui::Context, th: &Theme) {
    theme::set_active(th);
    let mut v = if th.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = th.bg_panel;
    v.window_fill = th.bg_panel;
    v.extreme_bg_color = th.bg_extreme;
    v.faint_bg_color = th.faint_bg;
    v.override_text_color = Some(th.text_bright);
    v.hyperlink_color = th.hyperlink;
    v.selection.bg_fill = th.selection;
    v.selection.stroke = egui::Stroke::new(1.0, th.text_bright);
    v.window_corner_radius = egui::CornerRadius::same(10);
    v.window_stroke = egui::Stroke::new(1.0, th.panel_border());
    for (w, fill, edge) in [
        (&mut v.widgets.noninteractive, th.bg_control, th.border_dim),
        (&mut v.widgets.inactive, th.bg_control, th.border_dim),
        (&mut v.widgets.hovered, th.bg_hover, th.border_hi),
        (&mut v.widgets.active, th.bg_active, th.border_hi),
        (&mut v.widgets.open, th.bg_control, th.border_dim),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = egui::Stroke::new(1.0, edge);
        w.fg_stroke = egui::Stroke::new(1.0, th.text_bright);
        w.corner_radius = egui::CornerRadius::same(6);
    }
    ctx.set_visuals(v);
}

// ── app ────────────────────────────────────────────────────────────────────

struct Prototype {
    entries: Vec<Entry>,
    board: Board,
    /// Index into `entries` of the row whose Details modal is open.
    details: Option<usize>,
    /// Rows whose last test run could not complete: no stars, ranked last.
    failed: std::collections::HashMap<usize, String>,
    /// Error window contents — `(model, message)`.
    error: Option<(String, String)>,
    /// Transient confirmation line under the tabs.
    status: Option<String>,
    /// Active IDE palette. The picker in the header switches it so the board
    /// can be judged under more than one theme.
    theme: &'static Theme,
}

/// What a simulated "Run Tests" does for a given model. The real button fires
/// the proficiency benchmark; here two of the sample models are rigged to fail
/// so the error path can be seen without a provider.
fn simulated_error(model: &str) -> Option<&'static str> {
    match model {
        "llama-4-scout:free" => Some(
            "HTTP 429 — Too Many Requests\n\n\
             https://router.huggingface.co/v1/chat/completions\n\n\
             The free tier for this model is rate-limited and rejected the \
             benchmark before the first round completed. Nothing was scored.\n\n\
             Retry later, or attach a paid key to this model profile.",
        ),
        "granite-code:8b" => Some(
            "Connection refused (os error 61)\n\n\
             http://localhost:11434/api/chat\n\n\
             No Ollama server answered on the profile's endpoint, so the model \
             could not be loaded and no round ran.\n\n\
             Start Ollama and pull the model, then run the tests again.",
        ),
        _ => None,
    }
}

impl Prototype {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Floor every text style at 12 px, headings at 18.
        use egui::{FontFamily::Monospace, FontFamily::Proportional, FontId, TextStyle};
        cc.egui_ctx.all_styles_mut(|style| {
            style.text_styles = [
                (TextStyle::Heading, FontId::new(SZ_TITLE, Proportional)),
                (TextStyle::Body, FontId::new(SZ_BODY, Proportional)),
                (TextStyle::Button, FontId::new(SZ_BODY, Proportional)),
                (TextStyle::Small, FontId::new(SZ_SMALL, Proportional)),
                (TextStyle::Monospace, FontId::new(SZ_BODY, Monospace)),
            ]
            .into();
        });

        let entries = sample_entries();
        // Seed one already-failed row so the "no stars, ranked last" state is
        // visible without having to click anything first.
        let mut failed = std::collections::HashMap::new();
        if let Some(i) = entries.iter().position(|e| e.model == "llama-4-scout:free") {
            failed.insert(i, "HTTP 429 — free tier rate-limited".to_string());
        }
        Self {
            entries,
            board: Board::Overall,
            details: None,
            failed,
            error: None,
            status: None,
            theme: &theme::DARK_GLASS,
        }
    }

    fn pal(&self) -> Pal {
        Pal::of(self.theme)
    }

    /// Simulate the per-model test run behind the Run Tests button.
    fn run_tests(&mut self, i: usize) {
        let model = self.entries[i].model;
        match simulated_error(model) {
            Some(msg) => {
                // Cannot run: surface the error, strip the stars, sink the row.
                self.failed.insert(
                    i,
                    msg.lines().next().unwrap_or("Test could not run").to_string(),
                );
                self.error = Some((model.to_string(), msg.to_string()));
                self.status = None;
            }
            None => {
                self.failed.remove(&i);
                self.status = Some(format!(
                    "{model}: proficiency test finished — scores and rank updated."
                ));
            }
        }
    }

    /// Indices into `self.entries` on the active board, best first. Rows whose
    /// test could not run carry no score, so they sort below every scored row
    /// rather than being ranked on stale numbers.
    fn ranked(&self) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..self.entries.len())
            .filter(|i| self.board.accepts(self.entries[*i].tier))
            .collect();
        idx.sort_by(|a, b| {
            let (fa, fb) = (self.failed.contains_key(a), self.failed.contains_key(b));
            fa.cmp(&fb).then_with(|| {
                self.entries[*b]
                    .overall
                    .partial_cmp(&self.entries[*a].overall)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });
        idx
    }

    /// "Apply to Grace" — hand this model to the orchestrator (spec 029).
    fn apply_to_grace(&mut self, i: usize) {
        let e = &self.entries[i];
        self.status = Some(format!(
            "Grace now runs on {} ({}).",
            e.model, e.provider
        ));
    }

    /// "Apply to Specialists" — hand this model to every specialist agent.
    fn apply_to_specialists(&mut self, i: usize) {
        let e = &self.entries[i];
        self.status = Some(format!(
            "{} assigned to all specialist agents (FormsDesigner, EventBinder, CodeGenerator, …).",
            e.model
        ));
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        let p = self.pal();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("COBOL proficiency test results")
                    .size(SZ_BODY)
                    .color(p.label),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(RichText::new("Reset sample").size(SZ_BODY))
                    .clicked()
                {
                    self.failed.clear();
                    self.status = None;
                }
                egui::ComboBox::from_id_salt("theme_pick")
                    .selected_text(RichText::new(self.theme.name).size(SZ_BODY))
                    .width(170.0)
                    .show_ui(ui, |ui| {
                        for t in theme::THEMES {
                            let sel = self.theme.id == t.id;
                            if ui
                                .selectable_label(sel, RichText::new(t.name).size(SZ_BODY))
                                .clicked()
                            {
                                self.theme = t;
                            }
                        }
                    });
                ui.label(RichText::new("Theme").size(SZ_SMALL).color(p.label));
                ui.label(
                    RichText::new("PROTOTYPE · sample data")
                        .size(SZ_SMALL)
                        .color(p.warn),
                );
            });
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            for b in Board::ALL {
                let count = self.entries.iter().filter(|e| b.accepts(e.tier)).count();
                let label = RichText::new(format!("{}  ({count})", b.title())).size(SZ_BODY);
                if ui.selectable_label(self.board == b, label).clicked() {
                    self.board = b;
                }
            }
            if let Some(s) = &self.status {
                ui.label(RichText::new(s).size(SZ_SMALL).color(p.good));
            }
        });
    }

    fn table(&mut self, ui: &mut egui::Ui) {
        let p = self.pal();
        let ranked = self.ranked();
        let mut run: Option<usize> = None;
        let mut grace: Option<usize> = None;
        let mut specialists: Option<usize> = None;
        ui.label(RichText::new(self.board.title()).size(SZ_TITLE).strong());
        ui.add_space(4.0);
        egui::ScrollArea::vertical()
            .id_salt("board_scroll")
            .max_height(BODY_H - 2.0 * CARD_PAD as f32 - 2.0 - (SZ_TITLE + 8.0))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("leaderboard")
                    .num_columns(5)
                    .striped(true)
                    .spacing([14.0, 7.0])
                    .min_col_width(48.0)
                    .show(ui, |ui| {
                        for h in ["Rank", "Model name", "Provider", "Overall evaluation", ""] {
                            ui.label(RichText::new(h).size(SZ_BODY).strong().color(p.label));
                        }
                        ui.end_row();

                        // Failed rows sort last and are numbered "—": they hold
                        // no score, so giving them a rank would be a lie.
                        let mut rank = 0usize;
                        for i in ranked.iter() {
                            let e = &self.entries[*i];
                            let failure = self.failed.get(i).cloned();
                            match &failure {
                                None => {
                                    rank += 1;
                                    ui.label(
                                        RichText::new(format!("#{rank}"))
                                            .size(SZ_BODY)
                                            .strong()
                                            .color(p.text),
                                    );
                                }
                                Some(_) => {
                                    ui.label(RichText::new("—").size(SZ_BODY).color(p.bad));
                                }
                            }
                            ui.label(
                                RichText::new(e.model).size(SZ_BODY).strong().color(p.text),
                            );
                            ui.label(RichText::new(e.provider).size(SZ_BODY).color(p.text));
                            match &failure {
                                None => {
                                    ui.horizontal(|ui| {
                                        star_rating(ui, p, e.overall / 100.0, 14.0);
                                        ui.label(
                                            RichText::new(format!("{:.1}%", e.overall))
                                                .size(SZ_BODY)
                                                .strong()
                                                .color(score_color(p, e.overall)),
                                        );
                                    });
                                }
                                Some(reason) => {
                                    // Kept short: the reason lives in the hover
                                    // and the error window, so one failed row
                                    // cannot widen the column for every row.
                                    ui.label(
                                        RichText::new("not rated — test failed")
                                            .size(SZ_BODY)
                                            .color(p.bad),
                                    )
                                    .on_hover_text(reason);
                                }
                            }
                            ui.horizontal(|ui| {
                                let rated = failure.is_none();
                                if ui
                                    .add_enabled(
                                        rated,
                                        egui::Button::new(
                                            RichText::new("Details").size(SZ_BODY),
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.details = Some(*i);
                                }
                                if ui
                                    .button(RichText::new("Run tests").size(SZ_BODY))
                                    .clicked()
                                {
                                    run = Some(*i);
                                }
                                // A model that could not even be reached is not
                                // a model to hand work to.
                                if ui
                                    .add_enabled(
                                        rated,
                                        egui::Button::new(
                                            RichText::new("Apply to Grace").size(SZ_BODY),
                                        ),
                                    )
                                    .clicked()
                                {
                                    grace = Some(*i);
                                }
                                if ui
                                    .add_enabled(
                                        rated,
                                        egui::Button::new(
                                            RichText::new("Apply to Specialists").size(SZ_BODY),
                                        ),
                                    )
                                    .clicked()
                                {
                                    specialists = Some(*i);
                                }
                            });
                            ui.end_row();
                        }
                    });
                ui.add_space(6.0);
                Self::provenance_note(ui);
            });
        if let Some(i) = run {
            self.run_tests(i);
        }
        if let Some(i) = grace {
            self.apply_to_grace(i);
        }
        if let Some(i) = specialists {
            self.apply_to_specialists(i);
        }
    }

    /// The error window shown when a model's test cannot run. Movable, so it can
    /// be dragged off the row it is talking about.
    fn error_window(&mut self, ctx: &egui::Context) {
        let Some((model, message)) = self.error.clone() else {
            return;
        };
        let p = self.pal();
        let mut open = true;
        let mut close = false;
        egui::Window::new(RichText::new("Test could not run").size(SZ_TITLE).strong())
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_size([460.0, 260.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(RichText::new(&model).size(SZ_BODY + 1.0).strong());
                ui.add_space(6.0);
                egui::Frame::NONE
                    .fill(self.theme.bg_extreme)
                    .stroke(egui::Stroke::new(1.0, p.bad))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_width(420.0);
                        ui.label(RichText::new(&message).size(SZ_BODY).color(p.text));
                    });
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "This model keeps its place at the bottom of the board, with no \
                         rating, until a test completes.",
                    )
                    .size(SZ_SMALL)
                    .color(p.label),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Close").size(SZ_BODY)).clicked() {
                        close = true;
                    }
                });
            });
        if close || !open {
            self.error = None;
        }
    }

    fn details_modal(&mut self, ctx: &egui::Context) {
        let Some(i) = self.details else { return };
        let rank = self
            .ranked()
            .iter()
            .position(|k| *k == i)
            .map(|p| p + 1)
            .unwrap_or(0);
        let e = self.entries[i].clone();
        let p = self.pal();
        let mut close = false;

        egui::Modal::new(egui::Id::new("leaderboard_details")).show(ctx, |ui| {
            ui.set_width(560.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(e.model).size(SZ_TITLE).strong());
                ui.label(RichText::new(e.provider).color(p.label));
                ui.label(RichText::new(e.tier.label()).color(p.label));
            });
            ui.label(
                RichText::new(format!(
                    "Rank #{rank} of {} on the “{}” board · tested {}",
                    self.ranked().len(),
                    self.board.title(),
                    e.tested_on
                ))
                .size(SZ_SMALL)
                .color(p.label),
            );
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                star_rating(ui, p, e.overall / 100.0, 20.0);
                ui.label(
                    RichText::new(format!("{:.1}%", e.overall))
                        .size(20.0)
                        .strong()
                        .color(score_color(p, e.overall)),
                );
                ui.label(RichText::new("overall score").color(p.label));
            });
            ui.add_space(10.0);

            ui.label(RichText::new("SCORES").size(SZ_SMALL).color(p.label));
            egui::Grid::new("details_scores")
                .num_columns(4)
                .striped(true)
                .spacing([18.0, 6.0])
                .show(ui, |ui| {
                    let mut cell = 0;
                    for (label, v) in [
                        ("Compilation rate", e.compilation),
                        ("Functional pass rate", e.functional),
                        ("COBOL-85 generation", e.cobol85_generation),
                        ("Indexed-file score", e.indexed_files),
                        ("Modification score", e.modification),
                        ("Debugging score", e.debugging),
                        ("Refactoring score", e.refactoring),
                        ("File-handling score", e.file_handling),
                        ("Table-driven design", e.table_driven),
                        ("Code explanation", e.explanation),
                        ("Type inference", e.type_inference),
                        ("Inline INVOKE", e.inline_invoke),
                        ("PowerRustCOBOL", e.powerrustcobol),
                    ] {
                        ui.label(RichText::new(label).color(p.label));
                        ui.label(RichText::new(format!("{v:.0}%")).color(score_color(p, v)));
                        cell += 1;
                        if cell % 2 == 0 {
                            ui.end_row();
                        }
                    }
                    if cell % 2 != 0 {
                        ui.end_row();
                    }
                });

            ui.add_space(10.0);
            ui.label(
                RichText::new("BEHAVIOUR (measured)")
                    .size(SZ_SMALL)
                    .color(p.label),
            );
            egui::Grid::new("details_behaviour")
                .num_columns(4)
                .striped(true)
                .spacing([18.0, 6.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Hallucination count").color(p.label));
                    ui.label(format!(
                        "{} ({:.1} per 10k tokens)",
                        e.hallucinations,
                        e.hallucination_rate()
                    ));
                    ui.label(RichText::new("Average latency").color(p.label));
                    ui.label(format!("{:.1} s", e.latency_ms as f32 / 1000.0));
                    ui.end_row();

                    ui.label(RichText::new("Average output tokens").color(p.label));
                    ui.label(format!("{}", e.out_tokens));
                    ui.label(RichText::new("Context window").color(p.label));
                    ui.label(format!("{} in / {} out", e.ctx_in, e.ctx_out));
                    ui.end_row();

                    ui.label(RichText::new("Reliability").color(p.label));
                    ui.label(format!("{:.0}% of runs completed", e.reliability));
                    ui.label(RichText::new("Determinism").color(p.label));
                    ui.label(format!("{:.0}% run-to-run agreement", e.determinism));
                    ui.end_row();

                    ui.label(RichText::new("Hardware required").color(p.label));
                    ui.label(e.hardware.unwrap_or("— (cloud)"));
                    ui.label(RichText::new("Quantization").color(p.label));
                    ui.label(e.quantization.unwrap_or("— (cloud)"));
                    ui.end_row();

                    ui.label(RichText::new("Peak memory").color(p.label));
                    ui.label(
                        e.ram_gb
                            .map(|g| format!("{g:.1} GB"))
                            .unwrap_or_else(|| "— (cloud)".into()),
                    );
                    ui.label(RichText::new("Parameters").color(p.label));
                    ui.label(
                        e.params_b
                            .map(|b| format!("{b:.0} B"))
                            .unwrap_or_else(|| "not published".into()),
                    );
                    ui.end_row();

                    ui.label(RichText::new("Price (output)").color(p.label));
                    ui.label(
                        e.usd_per_mtok_out
                            .map(|c| format!("${c:.2} / 1M tokens"))
                            .unwrap_or_else(|| "free".into()),
                    );
                    ui.label(RichText::new("Score per dollar").color(p.label));
                    ui.label(
                        e.value_per_dollar()
                            .map(|v| format!("{v:.2} pts per $/M"))
                            .unwrap_or_else(|| "— (no price)".into()),
                    );
                    ui.end_row();
                });

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("Open full benchmark report").clicked() {
                    // Prototype: the shipped panel reopens the stored report.
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            });
        });

        if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.details = None;
        }
    }

    fn provenance_note(ui: &mut egui::Ui) {
        ui.collapsing("Where each number would come from", |ui| {
            ui.label(
                "Model-reported (benchmark JSON, already collected): overall, compilation, \
                 functional, code preservation → modification, file handling, PowerRustCOBOL.",
            );
            ui.label(
                "Model-reported (new schema keys needed): debugging, refactoring, code \
                 explanation, indexed files, table-driven, type inference, inline INVOKE, \
                 hallucination count.",
            );
            ui.label(
                "Harness-measured (new): latency, output tokens, reliability across rounds, \
                 determinism across repeats, peak RAM for local runs.",
            );
            ui.label(
                "Connection probe (new): supported input/output token limits, parameter count \
                 and quantization for local models, price per token where the provider states it.",
            );
        });
    }
}

impl eframe::App for Prototype {
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // eframe 0.35 hands the frame's root `Ui`, and panels nest inside it;
        // only the modal still wants the `Context`.
        let ctx = root_ui.ctx().clone();
        apply_theme(&ctx, self.theme);
        let p = self.pal();
        egui::CentralPanel::default().show(root_ui, |ui| {
            ui.label(
                RichText::new("Prototype host — drag the leaderboard window by its title bar.")
                    .size(SZ_SMALL)
                    .color(p.label),
            );
        });

        // Movable, and sized to hug its contents: a resizable window whose
        // children measure themselves against the space they were given grows a
        // few pixels every frame until it hits the screen edge.
        // Brand rule: the "AI" in the product name is always the brand cyan.
        // `brand_layout_job` already emits "PowerRustCOBOL " + "AI"; the suffix
        // continues from there.
        let title = theme::brand_layout_job(
            "",
            " · Model Leaderboard",
            SZ_TITLE,
            self.theme.text_bright,
        );
        egui::Window::new(title)
            .movable(true)
            .resizable(false)
            .collapsible(false)
            .default_pos([20.0, 20.0])
            .show(&ctx, |ui| {
                ui.set_width(TABLE_W);
                self.header(ui);
                ui.add_space(8.0);
                card(ui, p, egui::vec2(TABLE_W, BODY_H), |ui| self.table(ui));
            });

        self.details_modal(&ctx);
        self.error_window(&ctx);
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 900.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Model Leaderboard — prototype",
        options,
        Box::new(|cc| Ok(Box::new(Prototype::new(cc)))),
    )
}
