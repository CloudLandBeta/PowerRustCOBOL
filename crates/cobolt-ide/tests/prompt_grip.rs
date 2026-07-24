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
        });
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
