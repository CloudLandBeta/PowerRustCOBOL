#![cfg(feature = "render")]
// What one FRAME costs, and therefore what an IDLE form costs.
//
// The generated COBOL event loop blocks on `recv()` when nothing is happening,
// so it burns nothing while idle. The frame loop does not: the host asks for a
// repaint every 200 ms even when idle (`host.rs`, the `busy` ternary), so a form
// sitting untouched still renders 5 times a second, forever — and 60 times a
// second whenever `busy` holds, which includes an undrained event backlog.
//
// This measures full `render_form` passes over synthetic forms of a few sizes,
// so idle cost can be stated in numbers rather than guessed at. Synthetic on
// purpose: a benchmark that reads forms out of somebody's home directory
// measures nothing on any other machine.
//
// Run with:
// `cargo test --release -p cobolt-forms --features render --test bench_render_frame -- --nocapture`

use std::time::Instant;

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::render::{render_form, Backdrop, DesignedState, RenderInput, RenderMode};
use cobolt_forms::{Control, ControlType, Rect as MRect};

const FRAMES: usize = 60;

/// A form of `n` controls in a grid: labels, buttons and text boxes, which is
/// the mix an ordinary business form is mostly made of.
fn synthetic_form(n: usize) -> Vec<Control> {
    (0..n)
        .map(|i| {
            let ct = match i % 3 {
                0 => ControlType::Label,
                1 => ControlType::Button,
                _ => ControlType::TextBox,
            };
            let (col, row) = ((i % 6) as i32, (i / 6) as i32);
            let mut c = Control::new(format!("C-{i}"), ct, 0, 0);
            c.rect = MRect::new(20 + col * 220, 20 + row * 46, 200, 34);
            c
        })
        .collect()
}

/// Average nanoseconds for one full render pass over `controls`.
fn ns_per_frame(controls: &[Control]) -> f64 {
    let ctx = egui::Context::default();
    let active = ActiveTabs::default();
    let mut one = |controls: &[Control]| {
        let mut full = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1400.0, 900.0),
                )),
                ..Default::default()
            },
            |root_ui| {
                egui::CentralPanel::default().show(root_ui, |ui| {
                    let input = RenderInput {
                        controls,
                        state: &DesignedState,
                        form_size: egui::vec2(1400.0, 900.0),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active,
                        backdrop: Backdrop::default(),
                    };
                    render_form(ui, &input);
                });
            },
        );
        full.textures_delta.clear();
    };

    // Warm-up: first-frame work (font atlas, texture upload) is not what an
    // idle form repeats.
    one(controls);
    one(controls);

    let start = Instant::now();
    for _ in 0..FRAMES {
        one(controls);
    }
    start.elapsed().as_nanos() as f64 / FRAMES as f64
}

#[test]
fn idle_frame_cost_by_form_size() {
    println!("\n  ── One render_form pass (the work an idle form repeats) ──");
    println!("  controls   ns/frame     ms/frame   idle 5 fps   busy 60 fps");
    for n in [10usize, 30, 60, 120] {
        let controls = synthetic_form(n);
        let ns = ns_per_frame(&controls);
        println!(
            "  {n:>8}   {ns:>8.0}   {:>8.2}   {:>7.2} ms/s   {:>7.1} ms/s",
            ns / 1e6,
            ns * 5.0 / 1e6,
            ns * 60.0 / 1e6
        );
    }
    println!(
        "\n  An idle form repaints at 5 fps; anything that makes `busy` true —\n  \
         an animation, or an undrained event backlog — takes it to 60.\n"
    );
}
