// Regression test for the prompt box's SIZE SOURCE: the bordered box the
// developer sees is a Frame of exactly the dragged height, and the editor
// inside it is frameless and scrolled. Pasting more lines than the box holds
// must not change its size — the old layout let the TextEdit's own frame (which
// is content-sized) be the visible border, so a paste grew it past the viewport
// and the box read as clipped by the pane.
#[test]
fn pasting_more_lines_than_the_box_holds_never_resizes_it() {
    const MARGIN: f32 = 4.0;
    const BOX_H: f32 = 72.0;
    let ctx = egui::Context::default();
    let box_h_probe = std::cell::Cell::new(0.0_f32);

    let mut run = |text: &mut String| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 600.0),
        ));
        ctx.run_ui(input, |ui| {
            let ui: &mut egui::Ui = ui;
            ui.horizontal(|ui| {
                let frame = egui::Frame::NONE
                    .stroke(egui::Stroke::new(1.0, egui::Color32::GRAY))
                    .inner_margin(egui::Margin::same(MARGIN as i8));
                let box_size = egui::vec2(400.0, BOX_H);
                // The frame's own border counts: its stroke is drawn OUTSIDE the
                // margin box, so the viewport must give it back.
                let view_size = box_size - egui::Vec2::splat(2.0 * (MARGIN + 1.0));
                let inner = frame.show(ui, |ui| {
                    ui.set_min_size(view_size);
                    ui.set_max_size(view_size);
                    egui::ScrollArea::vertical()
                        .id_salt("prompt_scroll")
                        .auto_shrink([false, false])
                        .min_scrolled_height(view_size.y)
                        .max_height(view_size.y)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(text)
                                    .frame(egui::Frame::NONE)
                                    .desired_rows(3)
                                    .desired_width(f32::INFINITY),
                            )
                        })
                        .inner
                });
                box_h_probe.set(inner.response.rect.height());
            });
        })
        .textures_delta
        .clear();
    };

    let mut short = String::from("one line");
    run(&mut short);
    run(&mut short);
    let with_one_line = box_h_probe.get();

    let mut pasted = (1..=20)
        .map(|i| format!("pasted line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    run(&mut pasted);
    run(&mut pasted);
    let with_twenty_lines = box_h_probe.get();

    assert!(
        (with_one_line - BOX_H).abs() < 0.5,
        "the box did not open at its own height ({with_one_line} vs {BOX_H})"
    );
    assert!(
        (with_twenty_lines - with_one_line).abs() < 0.5,
        "pasting 20 lines resized the box ({with_one_line} → {with_twenty_lines}); \
         only the grip drag may change its height"
    );
}

// Regression test for the chat-prompt resize pattern (Grace chat and the
// designer's AI Assistant pane): a drag-only `ui.interact` grip registered
// AFTER a multiline TextEdit must win the hit-test and receive pointer drags
// in egui 0.35 — the TextEdit/ScrollArea must not steal them as text select.
#[test]
fn grip_drag_pattern_receives_the_drag_over_the_text_edit() {
    let ctx = egui::Context::default();
    let mut height = 72.0_f32;
    let mut text = String::from("hello world");
    let box_rect_probe = std::cell::Cell::new(egui::Rect::NOTHING);
    let grip_dragged_probe = std::cell::Cell::new(false);

    let mut run = |events: Vec<egui::Event>, height: &mut f32, text: &mut String| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 600.0),
        ));
        input.events = events;
        ctx.run_ui(input, |ui| {
            {
                let ui: &mut egui::Ui = ui;
                ui.horizontal(|ui| {
                    let box_size = egui::vec2(400.0, *height);
                    let inner = ui.allocate_ui(box_size, |ui| {
                        ui.set_min_size(box_size);
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(text)
                                        .desired_rows(3)
                                        .desired_width(f32::INFINITY),
                                )
                            })
                            .inner
                    });
                    let box_rect = inner.response.rect;
                    box_rect_probe.set(box_rect);
                    let grip_rect = egui::Rect::from_min_size(
                        box_rect.max - egui::vec2(14.0, 14.0),
                        egui::vec2(14.0, 14.0),
                    );
                    let grip = ui.interact(grip_rect, egui::Id::new("grip"), egui::Sense::drag());
                    if grip.dragged() {
                        grip_dragged_probe.set(true);
                        *height = (*height + grip.drag_delta().y).clamp(72.0, 300.0);
                    }
                });
            }
        })
        .textures_delta
        .clear();
    };

    run(vec![], &mut height, &mut text); // frame 1: register widgets
    let grip_center = box_rect_probe.get().max - egui::vec2(7.0, 7.0);
    run(
        vec![
            egui::Event::PointerMoved(grip_center),
            egui::Event::PointerButton {
                pos: grip_center,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
        &mut height,
        &mut text,
    );
    run(
        vec![egui::Event::PointerMoved(grip_center + egui::vec2(0.0, 30.0))],
        &mut height,
        &mut text,
    );
    run(
        vec![egui::Event::PointerButton {
            pos: grip_center + egui::vec2(0.0, 30.0),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
        &mut height,
        &mut text,
    );

    assert!(
        grip_dragged_probe.get(),
        "grip never reported dragged() — the TextEdit or ScrollArea is stealing the drag"
    );
    assert!(
        height > 72.0,
        "grip drag did not grow the box (height stayed {height})"
    );
}
