// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Claude mascot**: while Claude talks to the IDE over MCP, a small
//! pixel creature sits on the top edge of the window with a balloon that says
//! what Claude is doing ("Claude is checking the code", and the file it named).
//!
//! - It shows only while an agent is connected: the tool server is listening
//!   and an admitted request arrived within [`LINGER`], or a call is running.
//!   The transport has no session (one POST per request), so a quiet agent
//!   fades out after [`LINGER`] and comes back with its next request.
//! - It never takes input: it is painted on its own layer, and it runs from the
//!   pointer instead. When the pointer comes within [`FLEE_RADIUS`] of it, or
//!   onto it, it runs to the end of the top edge farther from the pointer,
//!   dropping under the pointer if its way passes it, so it never runs through it.
//! - The pointer over the balloon (a narrow window, where the far end is where
//!   it already stands) turns the balloon nearly transparent.

use std::time::{Duration, Instant};

use egui::{pos2, vec2, Color32, Pos2, Rect};

use crate::agent_kit::ide_host::AgentActivity;
use crate::i18n::Tr;

/// How long after the last admitted MCP request the agent counts as connected.
pub const LINGER: Duration = Duration::from_secs(120);
/// How long a finished call's phrase stays in the balloon (a call can take
/// milliseconds; the developer must still be able to read it).
pub const HOLD: Duration = Duration::from_millis(2500);
/// How near the pointer may come before the mascot runs.
pub const FLEE_RADIUS: f32 = 80.0;
/// Running speed, points per second.
const SPEED: f32 = 1600.0;
/// One pixel of the sprite, in points.
const CELL: f32 = 2.5;
/// Gap from the window's sides and top.
const MARGIN: f32 = 14.0;
const TOP: f32 = 2.0;
/// How far below the pointer the sprite passes when its way crosses it.
const CLEARANCE: f32 = 12.0;
/// How long the drop eases in and out on each side of the pointer.
const DIP_RAMP: f32 = 40.0;
/// The balloon never grows wider than this.
const BALLOON_MAX_W: f32 = 340.0;
const BALLOON_GAP: f32 = 6.0;
const BLINK_EVERY: Duration = Duration::from_millis(4200);
const BLINK_FOR: Duration = Duration::from_millis(150);

/// Claude's colour, and the balloon's: fixed, so they read on every IDE theme.
pub const BODY: Color32 = Color32::from_rgb(0xD9, 0x77, 0x57);
const EYES: Color32 = Color32::from_rgb(0x1A, 0x1A, 0x1A);
const BALLOON_FILL: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
const BALLOON_INK: Color32 = Color32::from_rgb(0x1F, 0x1F, 0x1F);
const BALLOON_WEAK: Color32 = Color32::from_rgb(0x5A, 0x5A, 0x5A);

/// The sprite: `#` body, `E` eye, `.` nothing. Rows 6 and 7 are the legs.
const SPRITE: [&str; 8] = [
    "..########..",
    "..#E####E#..",
    "..#E####E#..",
    "############",
    "..########..",
    "..########..",
    "..#.#..#.#..",
    "..#.#..#.#..",
];
/// The legs' other step while it runs.
const STEP: [&str; 2] = ["..#.#..#.#..", ".#.#....#.#."];
const COLS: usize = 12;
const ROWS: usize = 8;
pub const SPRITE_W: f32 = COLS as f32 * CELL;
pub const SPRITE_H: f32 = ROWS as f32 * CELL;

/// What the balloon says Claude is doing, from the tool it called.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Doing {
    Connected,
    Working,
    Reading,
    Reference,
    Checking,
    Generating,
    Building,
    Looking,
    Running,
    Changing,
    Creating,
    Opening,
}

impl Doing {
    /// The phrase for an MCP tool; a tool this list does not know is "working".
    pub fn of(tool: &str) -> Self {
        match tool {
            "list_files" | "describe" => Doing::Reading,
            "kb_lookup" | "kb_search" => Doing::Reference,
            "check" | "validate" => Doing::Checking,
            "regenerate" => Doing::Generating,
            "build" => Doing::Building,
            "render_form" => Doing::Looking,
            "run_form" => Doing::Running,
            "set_definition" | "add_to_project" | "add_crate" | "remove_crate" | "add_powerchat" => Doing::Changing,
            "create_project" => Doing::Creating,
            "open_project" => Doing::Opening,
            _ => Doing::Working,
        }
    }

    pub fn text(self, tr: &Tr) -> &'static str {
        match self {
            Doing::Connected => tr.mascot_connected,
            Doing::Working => tr.mascot_working,
            Doing::Reading => tr.mascot_reading,
            Doing::Reference => tr.mascot_reference,
            Doing::Checking => tr.mascot_checking,
            Doing::Generating => tr.mascot_generating,
            Doing::Building => tr.mascot_building,
            Doing::Looking => tr.mascot_looking,
            Doing::Running => tr.mascot_running,
            Doing::Changing => tr.mascot_changing,
            Doing::Creating => tr.mascot_creating,
            Doing::Opening => tr.mascot_opening,
        }
    }
}

/// Whether an agent is connected now: the tools are served, and a call runs or
/// an admitted request arrived within [`LINGER`].
pub fn connected(listening: bool, activity: &AgentActivity, now: Instant) -> bool {
    listening
        && (!activity.running.is_empty()
            || activity.last_seen.is_some_and(|t| now.saturating_duration_since(t) < LINGER))
}

/// What the balloon says: the newest running call, else the last finished one
/// for [`HOLD`], else "connected". The second value is the file or name the
/// call named (empty when it named none).
pub fn balloon(activity: &AgentActivity, now: Instant) -> (Doing, String) {
    if let Some((tool, target, _)) = activity.running.last() {
        return (Doing::of(tool), target.clone());
    }
    if let Some((tool, target, ended)) = &activity.last_done {
        if now.saturating_duration_since(*ended) < HOLD {
            return (Doing::of(tool), target.clone());
        }
    }
    (Doing::Connected, String::new())
}

/// The span of sprite centres along the top edge.
pub fn span(screen: Rect) -> (f32, f32) {
    let left = screen.left() + MARGIN + SPRITE_W / 2.0;
    let right = (screen.right() - MARGIN - SPRITE_W / 2.0).max(left);
    (left, right)
}

/// Where the mascot runs to: the end of the top edge farther from the pointer.
pub fn flee_target(span: (f32, f32), pointer_x: f32) -> f32 {
    let (left, right) = span;
    if pointer_x - left < right - pointer_x {
        right
    } else {
        left
    }
}

/// Whether the pointer is near enough to make it run: within [`FLEE_RADIUS`]
/// of the sprite, or over the balloon.
pub fn threatened(sprite: Rect, balloon: Option<Rect>, pointer: Pos2) -> bool {
    sprite.expand(FLEE_RADIUS).contains(pointer) || balloon.is_some_and(|b| b.contains(pointer))
}

/// One frame of running: at most `SPEED * dt` toward `target`.
pub fn step(x: f32, target: f32, dt: f32) -> f32 {
    let d = target - x;
    let max = SPEED * dt.max(0.0);
    if d.abs() <= max {
        target
    } else {
        x + max * d.signum()
    }
}

/// How far the sprite drops, at centre `x`, to pass under a pointer in the top
/// band: fully while it is under the pointer, easing in and out over
/// [`DIP_RAMP`] on each side; nothing when the pointer is below its way.
pub fn dip(x: f32, pointer: Pos2, top: f32) -> f32 {
    let sprite_top = top + TOP;
    if pointer.y >= sprite_top + SPRITE_H + CLEARANCE || pointer.y < sprite_top - CLEARANCE {
        return 0.0;
    }
    let full = pointer.y + CLEARANCE - sprite_top;
    let d = (x - pointer.x).abs() - (SPRITE_W / 2.0 + CLEARANCE);
    if d <= 0.0 {
        return full;
    }
    let t = (1.0 - d / DIP_RAMP).clamp(0.0, 1.0);
    full * t * t * (3.0 - 2.0 * t)
}

/// The sprite's rectangle at centre `x`, dropped by `drop`.
pub fn sprite_rect(x: f32, top: f32, drop: f32) -> Rect {
    Rect::from_min_size(pos2(x - SPRITE_W / 2.0, top + TOP + drop), vec2(SPRITE_W, SPRITE_H))
}

/// The balloon's rectangle beside the sprite, on the side toward the middle of
/// the window.
pub fn balloon_rect(sprite: Rect, size: egui::Vec2, screen: Rect) -> Rect {
    let toward_left = sprite.center().x > screen.center().x;
    let x = if toward_left {
        sprite.left() - BALLOON_GAP - size.x
    } else {
        sprite.right() + BALLOON_GAP
    };
    Rect::from_min_size(pos2(x, sprite.top()), size)
}

/// Keep the end of a long path or name, which is the part that says which file.
fn shorten(target: &str, max: usize) -> String {
    let n = target.chars().count();
    if n <= max {
        return target.to_owned();
    }
    let tail: String = target.chars().skip(n - (max - 1)).collect();
    format!("…{tail}")
}

/// The mascot's own state, kept by the app between frames.
#[derive(Debug, Default)]
pub struct ClaudeMascot {
    /// The sprite's centre x; `None` while it is not shown.
    x: Option<f32>,
    /// Where it is running to.
    target: Option<f32>,
    last_frame: Option<Instant>,
    born: Option<Instant>,
    /// Where it was painted last frame (tests).
    painted: Option<Rect>,
}

impl ClaudeMascot {
    /// Where the sprite was painted last frame; `None` when it was not.
    pub fn painted(&self) -> Option<Rect> {
        self.painted
    }

    /// One frame: hide, or move and paint. Takes no input.
    pub fn show(&mut self, ctx: &egui::Context, listening: bool, activity: &AgentActivity, tr: &Tr, now: Instant) {
        if !connected(listening, activity, now) {
            *self = Self::default();
            return;
        }
        let screen = ctx.content_rect();
        let span = span(screen);
        let dt = self.last_frame.map_or(0.0, |t| now.saturating_duration_since(t).as_secs_f32()).min(0.05);
        self.last_frame = Some(now);
        let born = *self.born.get_or_insert(now);
        let mut x = self.x.unwrap_or(span.1).clamp(span.0, span.1);
        let pointer = ctx.input(|i| i.pointer.hover_pos());

        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("claude_mascot")));
        let (doing, target) = balloon(activity, now);
        let title = painter.layout_no_wrap(doing.text(tr).to_owned(), egui::FontId::proportional(12.0), Color32::PLACEHOLDER);
        let detail = (!target.is_empty())
            .then(|| painter.layout_no_wrap(shorten(&target, 48), egui::FontId::monospace(10.5), Color32::PLACEHOLDER));
        let text_w = title.size().x.max(detail.as_ref().map_or(0.0, |g| g.size().x));
        let text_h = title.size().y + detail.as_ref().map_or(0.0, |g| g.size().y + 1.0);
        let size = vec2((text_w + 16.0).min(BALLOON_MAX_W), (text_h + 8.0).max(SPRITE_H));

        let still = self.target.is_none();
        let sprite_at_rest = sprite_rect(x, screen.top(), 0.0);
        let balloon_at_rest = still.then(|| balloon_rect(sprite_at_rest, size, screen));
        if let Some(p) = pointer {
            if threatened(sprite_at_rest, balloon_at_rest, p) {
                let to = flee_target(span, p.x);
                if (to - x).abs() > 0.5 {
                    self.target = Some(to);
                }
            }
        }
        let mut drop = 0.0;
        let running = self.target.is_some();
        if let Some(to) = self.target {
            x = step(x, to, dt);
            if let Some(p) = pointer {
                drop = dip(x, p, screen.top());
            }
            if x == to {
                self.target = None;
            }
        }
        self.x = Some(x);

        let t = now.saturating_duration_since(born);
        let bob = if running {
            -((t.as_secs_f32() * 22.0).sin().abs() * 2.0)
        } else if !activity.running.is_empty() {
            (t.as_secs_f32() * 6.0).sin() * 1.0
        } else {
            0.0
        };
        let sprite = sprite_rect(x, screen.top(), drop + bob);
        let blink = !running && (t.as_millis() % BLINK_EVERY.as_millis()) < BLINK_FOR.as_millis();
        let legs = if running && (t.as_millis() / 70) % 2 == 1 { 1 } else { 0 };
        paint_sprite(&painter, sprite.min, blink, legs);
        self.painted = Some(sprite);

        if !running {
            let rect = balloon_rect(sprite, size, screen);
            let faded = pointer.is_some_and(|p| rect.contains(p));
            let alpha = if faded { 0.15 } else { 1.0 };
            let fill = BALLOON_FILL.gamma_multiply(alpha);
            let edge = BODY.gamma_multiply(alpha);
            painter.rect(rect, 8.0, fill, egui::Stroke::new(1.0, edge), egui::StrokeKind::Inside);
            let tip = if rect.left() > sprite.right() {
                [pos2(rect.left() + 0.5, rect.top() + 6.0), pos2(rect.left() + 0.5, rect.top() + 14.0), pos2(sprite.right() + 1.0, rect.top() + 9.0)]
            } else {
                [pos2(rect.right() - 0.5, rect.top() + 6.0), pos2(rect.right() - 0.5, rect.top() + 14.0), pos2(sprite.left() - 1.0, rect.top() + 9.0)]
            };
            painter.add(egui::Shape::convex_polygon(tip.to_vec(), fill, egui::Stroke::new(1.0, edge)));
            let ink = |c: Color32| c.gamma_multiply(alpha);
            let clip = rect.shrink(4.0);
            let at = pos2(rect.left() + 8.0, rect.top() + 4.0);
            painter.with_clip_rect(clip).galley(at, title.clone(), ink(BALLOON_INK));
            if let Some(d) = &detail {
                painter.with_clip_rect(clip).galley(at + vec2(0.0, title.size().y + 1.0), d.clone(), ink(BALLOON_WEAK));
            }
        }

        // Repaint every frame while it runs, at 20 a second while a call works;
        // otherwise only when something will change: the next blink, the end of
        // the held phrase, the end of LINGER.
        if running {
            ctx.request_repaint();
        } else if !activity.running.is_empty() {
            // The working bob is slow; 20 frames a second is plenty for it.
            ctx.request_repaint_after(Duration::from_millis(50));
        } else {
            let every = BLINK_EVERY.as_millis();
            let into = t.as_millis() % every;
            let next_blink = if into < BLINK_FOR.as_millis() { BLINK_FOR.as_millis() - into } else { every - into };
            let mut wait = Duration::from_millis(next_blink as u64);
            if let Some((_, _, ended)) = &activity.last_done {
                let held = now.saturating_duration_since(*ended);
                if held < HOLD {
                    wait = wait.min(HOLD - held);
                }
            }
            if let Some(seen) = activity.last_seen {
                let quiet = now.saturating_duration_since(seen);
                if quiet < LINGER {
                    wait = wait.min(LINGER - quiet);
                }
            }
            ctx.request_repaint_after(wait);
        }
    }
}

fn paint_sprite(painter: &egui::Painter, origin: Pos2, blink: bool, legs: usize) {
    for (r, row) in SPRITE.iter().enumerate() {
        let row = if r == ROWS - 1 { STEP[legs] } else { row };
        for (c, cell) in row.bytes().enumerate() {
            let colour = match cell {
                b'#' => BODY,
                b'E' if blink && r == 1 => BODY,
                b'E' => EYES,
                _ => continue,
            };
            let min = origin + vec2(c as f32 * CELL, r as f32 * CELL);
            painter.rect_filled(Rect::from_min_size(min, vec2(CELL, CELL)), 0.0, colour);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn activity(seen_ago: Option<u64>, running: &[(&str, &str)], now: Instant) -> AgentActivity {
        AgentActivity {
            last_seen: seen_ago.map(|s| now - Duration::from_secs(s)),
            running: running.iter().map(|(t, a)| ((*t).to_owned(), (*a).to_owned(), now)).collect(),
            last_done: None,
        }
    }

    #[test]
    fn it_shows_only_while_an_agent_is_connected() {
        let now = Instant::now();
        assert!(!connected(false, &activity(Some(1), &[], now), now), "no listener, no mascot");
        assert!(!connected(true, &activity(None, &[], now), now), "nobody ever called");
        assert!(connected(true, &activity(Some(5), &[], now), now), "a request 5 s ago");
        assert!(!connected(true, &activity(Some(LINGER.as_secs() + 1), &[], now), now), "quiet past LINGER");
        assert!(
            connected(true, &activity(Some(LINGER.as_secs() + 60), &[("build", "")], now), now),
            "a long build keeps it on however old the request"
        );
    }

    #[test]
    fn the_balloon_says_what_claude_is_doing() {
        let now = Instant::now();
        let a = activity(Some(0), &[("list_files", ""), ("check", "forms/Main.cfrm")], now);
        assert_eq!(balloon(&a, now), (Doing::Checking, "forms/Main.cfrm".to_owned()), "the newest running call");
        let mut done = activity(Some(0), &[], now);
        done.last_done = Some(("regenerate".into(), "forms/Main.cfrm".into(), now - Duration::from_millis(500)));
        assert_eq!(balloon(&done, now).0, Doing::Generating, "a finished call is held");
        done.last_done = Some(("regenerate".into(), "x".into(), now - HOLD - Duration::from_millis(1)));
        assert_eq!(balloon(&done, now), (Doing::Connected, String::new()), "then it says connected");
        assert_eq!(Doing::of("some_future_tool"), Doing::Working);
        let tr = crate::i18n::Language::English.tr();
        for tool in ["list_files", "kb_search", "check", "regenerate", "build", "render_form", "run_form", "add_crate", "create_project", "open_project", "x"] {
            assert!(Doing::of(tool).text(&tr).starts_with("Claude"), "{tool} names Claude");
        }
    }

    #[test]
    fn it_runs_to_the_far_end_from_the_pointer() {
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 900.0));
        let s = span(screen);
        assert_eq!(flee_target(s, 1300.0), s.0, "pointer on the right: run left");
        assert_eq!(flee_target(s, 100.0), s.1, "pointer on the left: run right");
        let sprite = sprite_rect(s.1, 0.0, 0.0);
        assert!(threatened(sprite, None, sprite.center()), "the pointer on it");
        assert!(threatened(sprite, None, sprite.left_center() - vec2(FLEE_RADIUS - 1.0, 0.0)), "near it");
        assert!(!threatened(sprite, None, sprite.left_center() - vec2(FLEE_RADIUS + 1.0, 0.0)), "far from it");
        let b = Rect::from_min_size(pos2(500.0, 2.0), vec2(200.0, 30.0));
        assert!(threatened(sprite, Some(b), pos2(600.0, 10.0)), "the pointer over the balloon");
        assert_eq!(step(100.0, 0.0, 0.01), 100.0 - SPEED * 0.01);
        assert_eq!(step(5.0, 0.0, 0.05), 0.0, "it stops on the target");
    }

    /// The way from one end to the other passes the pointer; the sprite drops
    /// under it, so at no frame does it cover the pointer.
    #[test]
    fn it_never_runs_through_the_pointer() {
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 900.0));
        let s = span(screen);
        for py in [3.0_f32, 8.0, 14.0, 22.0, 30.0] {
            let p = pos2(s.1 - 60.0, py);
            let to = flee_target(s, p.x);
            assert_eq!(to, s.0);
            let mut x = s.1;
            let mut frames = 0;
            while x != to {
                x = step(x, to, 1.0 / 120.0);
                let r = sprite_rect(x, screen.top(), dip(x, p, screen.top()));
                assert!(!r.contains(p), "pointer {p:?} covered at x={x}: {r:?}");
                frames += 1;
            }
            assert!(frames > 10);
        }
        assert_eq!(dip(s.0, pos2(s.0, 200.0), 0.0), 0.0, "a pointer below its way needs no drop");
    }

    #[test]
    fn a_long_target_keeps_its_end() {
        assert_eq!(shorten("forms/Main.cfrm", 48), "forms/Main.cfrm");
        let long = format!("{}/forms/Customers.cfrm", "a".repeat(80));
        let s = shorten(&long, 30);
        assert_eq!(s.chars().count(), 30);
        assert!(s.starts_with('…') && s.ends_with("forms/Customers.cfrm"));
    }

    fn frame(ctx: &egui::Context, m: &mut ClaudeMascot, listening: bool, a: &AgentActivity, pointer: Option<Pos2>, now: Instant) -> usize {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 900.0)));
        if let Some(p) = pointer {
            input.events.push(egui::Event::PointerMoved(p));
        }
        let tr = crate::i18n::Language::English.tr();
        let mut out = ctx.run_ui(input, |ui| m.show(ui.ctx(), listening, a, &tr, now));
        out.textures_delta.clear();
        out.shapes
            .iter()
            .filter(|c| matches!(&c.shape, egui::Shape::Rect(r) if r.fill == BODY))
            .count()
    }

    /// Painted on the top edge while connected, gone when not; the pointer on
    /// it sends it to the other end.
    #[test]
    fn it_paints_on_the_top_edge_and_flees_the_pointer() {
        let ctx = egui::Context::default();
        let mut m = ClaudeMascot::default();
        let now = Instant::now();
        let off = activity(None, &[], now);
        assert_eq!(frame(&ctx, &mut m, true, &off, None, now), 0, "not connected: nothing painted");
        assert!(m.painted().is_none());

        let on = activity(Some(1), &[("check", "forms/Main.cfrm")], now);
        let cells = frame(&ctx, &mut m, true, &on, None, now);
        assert!(cells > 40, "the sprite's body cells were painted: {cells}");
        let at = m.painted().expect("painted");
        assert!(at.top() < 10.0 && at.right() > 1300.0, "top right at first: {at:?}");

        let mut t = now;
        let mut pointer = Some(at.center());
        for _ in 0..120 {
            t += Duration::from_millis(16);
            frame(&ctx, &mut m, true, &on, pointer, t);
            pointer = None;
        }
        let fled = m.painted().expect("painted");
        assert!(fled.left() < 100.0, "it ran to the left end: {fled:?}");
        assert_eq!(frame(&ctx, &mut m, false, &on, None, t), 0, "the listener stopped: gone");
        println!("mascot: {cells} cells at {at:?}, fled to {fled:?}");
    }
}
