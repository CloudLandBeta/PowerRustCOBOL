// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Spec 037 T1 — THROWAWAY multi-viewport spike. Not shipped; deleted once the
// findings are recorded in specs/037-main-form-lifecycle/plan.md §5.
//
// Proves, on this machine's egui/eframe 0.35 (Ui-hosted panels):
//   1. `show_viewport_immediate` nested 3 deep keeps all levels rendering.
//   2. A "modal" child + `ui.disable()` in the caller blocks caller input.
//   3. Children created with `with_taskbar(false)` add no taskbar entry
//      (Windows/Linux; macOS children of one process never get Dock entries).
//   4. `ViewportCommand::Decorations` toggles the title bar at runtime.
//   5. `ViewportCommand::Fullscreen` enters/leaves fullscreen; the actual
//      state is read back from `ViewportInfo` (event-on-change source).
//   6. `with_minimize_button(false)` / `with_maximize_button(false)` remove
//      the buttons; `ViewportCommand::{Minimized,Maximized}` still work.
//
// Run: cargo run -p cobolt-cli --example mv_spike

use eframe::egui;

#[derive(Default)]
struct Spike {
    open_l1: bool,
    open_l2: bool,
    open_l3: bool,
    modal_open: bool,
    decorations: bool,
    fullscreen_want: bool,
    fullscreen_actual: bool,
    fullscreen_changes: u32,
    frames: [u64; 4],
    typed: String,
}

impl eframe::App for Spike {
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root_ui.ctx().clone();
        let ctx = &ctx;
        self.frames[0] += 1;

        // (5) Actual fullscreen state from ViewportInfo — the plan's event
        // source. Count transitions to prove one event per real change.
        let actual = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        if actual != self.fullscreen_actual {
            self.fullscreen_actual = actual;
            self.fullscreen_changes += 1;
        }

        let modal_open = self.modal_open;
        egui::CentralPanel::default().show(root_ui, |ui| {
            // (2) Modal blocking: while the modal child is open the caller's
            // whole UI is disabled — clicks and typing must do nothing.
            if modal_open {
                ui.disable();
            }
            ui.heading("mv_spike — root (level 0)");
            ui.label(format!(
                "frames L0..L3: {} / {} / {} / {}  |  fullscreen changes: {}",
                self.frames[0],
                self.frames[1],
                self.frames[2],
                self.frames[3],
                self.fullscreen_changes
            ));
            ui.checkbox(&mut self.open_l1, "open child level 1 (skip-taskbar)");
            if ui.button("open MODAL child (blocks this window)").clicked() {
                self.modal_open = true;
            }
            if ui.button("toggle decorations (title bar)").clicked() {
                self.decorations = !self.decorations;
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Decorations(self.decorations));
            }
            if ui.button("toggle fullscreen").clicked() {
                self.fullscreen_want = !self.fullscreen_want;
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen_want));
            }
            if ui.button("minimize me").clicked() {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            if ui.button("maximize me").clicked() {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Maximized(true));
            }
            ui.text_edit_singleline(&mut self.typed);
        });

        // (1)+(3)+(6) Level-1 child: immediate viewport, no taskbar entry, no
        // min/max buttons.
        if self.open_l1 {
            let mut still_open = true;
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("l1"),
                egui::ViewportBuilder::default()
                    .with_title("child L1 — no taskbar, no min/max buttons")
                    .with_taskbar(false)
                    .with_minimize_button(false)
                    .with_maximize_button(false)
                    .with_inner_size([420.0, 220.0]),
                |vp_ui, _| {
                    self.frames[1] += 1;
                    let l1_ctx = vp_ui.ctx().clone();
                    if l1_ctx.input(|i| i.viewport().close_requested()) {
                        still_open = false;
                    }
                    egui::CentralPanel::default().show(vp_ui, |ui| {
                        ui.label("level 1 — check the taskbar: no entry for me");
                        ui.checkbox(&mut self.open_l2, "open child level 2");
                    });
                    // (1) Level-2 nested inside level-1's callback.
                    if self.open_l2 {
                        l1_ctx.show_viewport_immediate(
                            egui::ViewportId::from_hash_of("l2"),
                            egui::ViewportBuilder::default()
                                .with_title("child L2")
                                .with_taskbar(false)
                                .with_inner_size([360.0, 180.0]),
                            |vp_ui, _| {
                                self.frames[2] += 1;
                                let l2_ctx = vp_ui.ctx().clone();
                                egui::CentralPanel::default().show(vp_ui, |ui| {
                                    ui.label("level 2");
                                    ui.checkbox(&mut self.open_l3, "open child level 3");
                                });
                                if self.open_l3 {
                                    l2_ctx.show_viewport_immediate(
                                        egui::ViewportId::from_hash_of("l3"),
                                        egui::ViewportBuilder::default()
                                            .with_title("child L3")
                                            .with_taskbar(false)
                                            .with_inner_size([300.0, 140.0]),
                                        |vp_ui, _| {
                                            self.frames[3] += 1;
                                            egui::CentralPanel::default().show(vp_ui, |ui| {
                                                ui.label("level 3 — all levels live?");
                                            });
                                        },
                                    );
                                }
                            },
                        );
                    }
                },
            );
            self.open_l1 = still_open;
        }

        // (2) The modal child itself.
        if self.modal_open {
            let mut open = true;
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("modal"),
                egui::ViewportBuilder::default()
                    .with_title("MODAL child")
                    .with_taskbar(false)
                    .with_inner_size([320.0, 140.0]),
                |vp_ui, _| {
                    if vp_ui.ctx().input(|i| i.viewport().close_requested()) {
                        open = false;
                    }
                    egui::CentralPanel::default().show(vp_ui, |ui| {
                        ui.label("root must ignore clicks/typing while I'm open");
                        if ui.button("close me").clicked() {
                            open = false;
                        }
                    });
                },
            );
            self.modal_open = open;
        }
    }
}

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "mv_spike",
        eframe::NativeOptions::default(),
        Box::new(|_| Ok(Box::new(Spike::default()) as Box<dyn eframe::App>)),
    )
}
