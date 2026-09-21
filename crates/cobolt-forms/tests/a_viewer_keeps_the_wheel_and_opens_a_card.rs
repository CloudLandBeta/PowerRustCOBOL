#![cfg(feature = "render")]
//! Two reports about a Viewer's pointer, one afternoon (operator, 2026-09-20):
//!
//!   * "Scroll up/down the content is passing through the form itself. It
//!     should not." — the Viewer READ the wheel and never consumed it, so the
//!     same notch scrolled the document and then the form behind it;
//!   * "double clicking a card must show the selected page" — a contact sheet
//!     where two clicks did what one click did.
//!
//! Both are asserted through the real interactive renderer, driven with real
//! events, because both are about what the engine does with an event rather
//! than about what a function returns when called directly.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{DesignedState, RenderInput, RenderMode, RenderOutput};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Rect, Vec2};

const FORM: Vec2 = Vec2::new(900.0, 700.0);

/// A side-by-side Viewer, optionally with a different document on each side.
fn split_viewer(left: &str, right: &str) -> Control {
    let mut c = Control::new("VWR-1", ControlType::Viewer, 40, 40);
    c.rect = MRect::new(40, 40, 700, 560);
    c.set_prop("Layout", PropValue::String("Web".into()));
    c.set_prop("SplitMode", PropValue::String("LeftRight".into()));
    c.set_prop("View1Source", PropValue::String(left.into()));
    c.set_prop("View2Source", PropValue::String(right.into()));
    c
}

/// Where the Split button sits in a view's own toolbar, in that view's rect.
fn split_button(view: egui::Rect) -> egui::Pos2 {
    let slots = cobolt_forms::viewer::toolbar_slots(cobolt_forms::viewer::ViewRect::new(
        view.min.x,
        view.min.y,
        view.width(),
        cobolt_forms::viewer::TOOLBAR_HEIGHT,
    ));
    let (_, slot) = slots
        .into_iter()
        .find(|(a, _)| *a == cobolt_forms::viewer::ToolbarAction::Split)
        .expect("every view's toolbar carries the Split action");
    pos2(slot.x + slot.w / 2.0, slot.y + slot.h / 2.0)
}

fn click_at(p: egui::Pos2) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(p),
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        },
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        },
    ]
}

fn viewer(cards: bool) -> Control {
    let mut c = Control::new("VWR-1", ControlType::Viewer, 40, 40);
    c.rect = MRect::new(40, 40, 700, 560);
    c.set_prop("Layout", PropValue::String("Web".into()));
    if cards {
        c.set_prop("View1ViewMode", PropValue::String("Cards".into()));
    }
    c
}

/// One frame, and what leaked past the controls.
///
/// `leaked` is read from the SAME place an ancestor `ScrollArea` reads it —
/// `smooth_scroll_delta`, after the form's content has run — so this measures
/// the actual mechanism by which a notch reaches the form, not a proxy for it.
fn frame(
    ctx: &egui::Context,
    controls: &[Control],
    events: Vec<egui::Event>,
) -> (RenderOutput, Vec2) {
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), FORM));
    // `InputState::modifiers` is updated by `Event::ModifiersChanged`, NOT by
    // the modifiers hanging off a key event — a real backend sends both, and a
    // test that sends only the decorated key leaves `i.modifiers.command`
    // false and every Cmd shortcut silently unpressed. Emit what the backend
    // would.
    let mut events = events;
    if let Some(m) = events.iter().find_map(|e| match e {
        egui::Event::Key { modifiers, .. } if *modifiers != egui::Modifiers::NONE => {
            Some(*modifiers)
        }
        _ => None,
    }) {
        events.insert(0, egui::Event::ModifiersChanged(m));
    }
    input.events = events;
    let mut produced = RenderOutput::default();
    let mut leaked = Vec2::ZERO;
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| {
                let inp = RenderInput {
                    controls,
                    state: &DesignedState,
                    form_size: FORM,
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Default::default(),
                };
                produced = cobolt_forms::render::render_form(ui, &inp);
                leaked = ui.input(|i| i.smooth_scroll_delta);
            });
    });
    full.textures_delta.clear();
    COPIED.with(|c| {
        *c.borrow_mut() = full
            .platform_output
            .commands
            .iter()
            .filter_map(|cmd| match cmd {
                egui::OutputCommand::CopyText(t) => Some(t.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");
    });
    (produced, leaked)
}

thread_local! {
    /// What the last frame put on the clipboard. egui reports a copy as an
    /// `OutputCommand` on the platform output, which the render output never
    /// sees — so the harness keeps it where a test can read it.
    static COPIED: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

fn last_copied() -> String {
    COPIED.with(|c| c.borrow().clone())
}

fn wheel_at(p: egui::Pos2) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(p),
        egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -40.0),
            modifiers: Default::default(),
            phase: egui::TouchPhase::Move,
        },
    ]
}

/// **The Viewer keeps the wheel.** Over the control it is consumed; away from
/// it, it is not — and the second half is what stops the first from being
/// vacuously true. A Viewer that ate every notch on the form would pass a test
/// that only checked the first.
#[test]
fn a_wheel_notch_over_a_viewer_never_reaches_the_form() {
    let ctx = egui::Context::default();
    let controls = [viewer(false)];

    // Warm the context: the first pass of a fresh Context lays out and
    // registers, and a pointer that has never been anywhere is nowhere.
    frame(&ctx, &controls, Vec::new());

    let over = pos2(400.0, 300.0); // inside the Viewer's 40,40 700x560 rect
    let away = pos2(820.0, 660.0); // bare form, past the control
    let (_, leaked_over) = frame(&ctx, &controls, wheel_at(over));
    let (_, leaked_away) = frame(&ctx, &controls, wheel_at(away));

    println!("  pointer            leaked to the form");
    println!("  ---------------    ------------------");
    println!("  over the Viewer    {leaked_over:?}");
    println!("  off the Viewer     {leaked_away:?}");

    assert_eq!(
        leaked_over,
        Vec2::ZERO,
        "a notch over the Viewer must not reach the form"
    );
    assert_ne!(
        leaked_away,
        Vec2::ZERO,
        "a notch away from the Viewer must still reach the form — otherwise \
         the assertion above proves nothing"
    );
}

/// **One click selects a card; two open it.** The double-click must leave
/// `Cards` for `Full` on the page it hit, or it does exactly what the single
/// click already did.
#[test]
fn double_clicking_a_card_opens_that_page() {
    let ctx = egui::Context::default();
    let controls = [viewer(true)];
    frame(&ctx, &controls, Vec::new());

    // The first card sits at the top-left of the content area, inside the
    // control and below its toolbar band.
    let card = pos2(120.0, 200.0);
    let press = |pressed: bool| egui::Event::PointerButton {
        pos: card,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };

    // Two complete presses in one frame is what egui reads as a double click.
    let (out, _) = frame(
        &ctx,
        &controls,
        vec![
            egui::Event::PointerMoved(card),
            press(true),
            press(false),
            press(true),
            press(false),
        ],
    );

    let wrote = |k: &str| {
        out.prop_updates
            .iter()
            .find(|(id, key, _)| id == "VWR-1" && key == k)
            .map(|(_, _, v)| v.clone())
    };
    let mode = wrote("View1ViewMode");
    println!("  View1ViewMode after a double-click on a card: {mode:?}");
    assert_eq!(
        mode.as_deref(),
        Some("Full"),
        "a double-clicked card must open the page, not stay on the contact sheet"
    );
}

/// **Closing a side closes the document that was on it.**
///
/// Side-by-side holding two different files is a reader comparing two things.
/// Asking for a single view on the LEFT means "I am done with the left one", so
/// the survivor must be the RIGHT document — closing the side while keeping
/// exactly the file just dismissed is what was reported (operator,
/// 2026-09-20). Two views of ONE file have no file to lose, and there the view
/// closes and nothing else.
#[test]
fn closing_the_left_side_leaves_the_right_document_behind() {
    let wrote = |c: &Control, at: egui::Pos2| -> Vec<(String, String)> {
        let ctx = egui::Context::default();
        let controls = [c.clone()];
        frame(&ctx, &controls, Vec::new());
        let (out, _) = frame(&ctx, &controls, click_at(at));
        out.prop_updates
            .iter()
            .map(|(_, k, v)| (k.clone(), v.clone()))
            .collect()
    };
    // The left view occupies the left half of the control's 700x560 rect.
    let left_view = Rect::from_min_size(pos2(40.0, 40.0), Vec2::new(350.0, 560.0));

    let two = split_viewer("left.md", "right.md");
    let updates = wrote(&two, split_button(left_view));
    let get = |k: &str| {
        updates
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.clone())
    };
    println!("  two files, closing the LEFT side:");
    for (k, v) in &updates {
        println!("    {k} = {v:?}");
    }
    assert_eq!(get("SplitMode").as_deref(), Some("None"), "the split must close");
    assert_eq!(
        get("View1Source").as_deref(),
        Some("right.md"),
        "the surviving view must show the document that was NOT dismissed"
    );
    assert_eq!(
        get("View2Source").as_deref(),
        Some(""),
        "the closed side keeps no document"
    );

    // The carve-out: one file on both sides is one document seen twice.
    let same = split_viewer("only.md", "only.md");
    let updates = wrote(&same, split_button(left_view));
    println!("  one file on both sides, closing the LEFT side:");
    for (k, v) in &updates {
        println!("    {k} = {v:?}");
    }
    let moved = updates.iter().any(|(k, _)| k == "View2Source");
    assert!(
        !moved,
        "two views of one file lose no document — only the view closes"
    );
}

/// Apply what a frame wrote back, the way a host does. Without this the
/// control never changes and a multi-step gesture cannot be told from a
/// single one.
fn apply(c: &mut Control, out: &RenderOutput) {
    for (_, key, val) in &out.prop_updates {
        c.set_prop(key, PropValue::String(val.clone()));
    }
}

fn prop(c: &Control, k: &str) -> String {
    c.get_prop(k).map(|v| v.as_str().to_owned()).unwrap_or_default()
}

/// **A view that closed the split must not keep voting once it is gone.**
///
/// Each view reconciles the control-wide `SplitMode` through its own shared
/// value, which prefers what the VIEW decided unless the property changed since
/// the view last looked. A second view that closed the split then stopped
/// rendering never looked again — so the moment the split reopened it woke,
/// found no change against its frozen `seen`, re-asserted its stale "closed"
/// and wrote `SplitMode = None` straight back. Side-by-side could not be
/// reached again for the life of the form (operator, 2026-09-20).
#[test]
fn the_split_can_be_reopened_after_a_view_closed_it() {
    let ctx = egui::Context::default();
    let mut c = split_viewer("a.md", "b.md");
    frame(&ctx, &[c.clone()], Vec::new());

    // Close from the RIGHT side, which leaves the left document in place.
    let right = Rect::from_min_size(pos2(390.0, 40.0), Vec2::new(350.0, 560.0));
    let (out, _) = frame(&ctx, &[c.clone()], click_at(split_button(right)));
    apply(&mut c, &out);
    println!("  after closing from the right: SplitMode = {:?}", prop(&c, "SplitMode"));
    assert_eq!(prop(&c, "SplitMode"), "None", "the split must close");

    let (out, _) = frame(&ctx, &[c.clone()], Vec::new());
    apply(&mut c, &out);

    // Reopen from the one view that is left.
    let whole = Rect::from_min_size(pos2(40.0, 40.0), Vec2::new(700.0, 560.0));
    let (out, _) = frame(&ctx, &[c.clone()], click_at(split_button(whole)));
    apply(&mut c, &out);
    println!("  after asking for side-by-side: SplitMode = {:?}", prop(&c, "SplitMode"));
    assert_eq!(
        prop(&c, "SplitMode"),
        "LeftRight",
        "asking for side-by-side must open it"
    );

    // And it STAYS open. This is the half the bug failed: the second view woke
    // on the very next frame and wrote the split shut again.
    for n in 1..=3 {
        let (out, _) = frame(&ctx, &[c.clone()], Vec::new());
        apply(&mut c, &out);
        println!("  {n} frame(s) later: SplitMode = {:?}", prop(&c, "SplitMode"));
    }
    assert_eq!(
        prop(&c, "SplitMode"),
        "LeftRight",
        "a reopened split must stay open — no view may write it shut on its own"
    );
}

/// **Fullscreen moves the Viewer somewhere else, and the pointer with it.**
///
/// The control opens a window of its own (`R13/AC5` — "Fullscreen is not
/// working as it is supposed to. It is maximizing the view inside the viewer
/// instead of the entire screen", operator 2026-09-20). The copy left in the
/// form is still PAINTED — a form must not show a hole where a control is —
/// but it must not be SENSED, because one surface owns the pointer at a time
/// and it is the one the operator is looking at.
///
/// Asserted by clicking the in-form toolbar: windowed, that press lands and
/// writes a property; fullscreen, the same press at the same point writes
/// nothing, because the control is not there any more.
#[test]
fn a_fullscreen_viewer_stops_sensing_the_copy_left_in_the_form() {
    let at = |fullscreen: bool| -> Vec<String> {
        let mut c = viewer(false);
        c.set_prop("Fullscreen", PropValue::Bool(fullscreen));
        let ctx = egui::Context::default();
        let controls = [c];
        frame(&ctx, &controls, Vec::new());
        // The Find button in the IN-FORM toolbar, which toggles a property.
        let view = Rect::from_min_size(pos2(40.0, 40.0), Vec2::new(700.0, 560.0));
        let slots = cobolt_forms::viewer::toolbar_slots(cobolt_forms::viewer::ViewRect::new(
            view.min.x,
            view.min.y,
            view.width(),
            cobolt_forms::viewer::TOOLBAR_HEIGHT,
        ));
        let (_, slot) = slots
            .into_iter()
            .find(|(a, _)| *a == cobolt_forms::viewer::ToolbarAction::Find)
            .expect("the toolbar carries Find");
        let p = pos2(slot.x + slot.w / 2.0, slot.y + slot.h / 2.0);
        let (out, _) = frame(&ctx, &controls, click_at(p));
        out.prop_updates.iter().map(|(_, k, _)| k.clone()).collect()
    };

    let windowed = at(false);
    let full = at(true);
    println!("  in-form toolbar click, windowed:   {windowed:?}");
    println!("  in-form toolbar click, fullscreen: {full:?}");
    assert!(
        windowed.iter().any(|k| k == "View1FindOpen"),
        "windowed, the in-form toolbar must work: {windowed:?}"
    );
    assert!(
        !full.iter().any(|k| k == "View1FindOpen"),
        "fullscreen, the copy left in the form must not answer the pointer: {full:?}"
    );
}

/// **Fullscreen is the form's own window, and it is given back.**
///
/// Two reports against the first attempt, which opened a window of its own
/// (operator, 2026-09-20): the toolbar came out clipped under the menu bar,
/// and leaving fullscreen left a blank form behind. Both came from the second
/// window — a decorationless fullscreen one covers screen the platform does
/// not mean you to draw in, and it was being opened from deep inside the
/// parent's own pass rather than at the top of a frame.
///
/// There is no second window now: the form's window goes fullscreen through
/// the platform's own command and the Viewer covers it. Asserted on the
/// commands the frame actually emitted, because "the window was asked" is the
/// whole of this crate's half — what the platform then does is its own.
#[test]
fn fullscreen_asks_the_window_and_gives_it_back() {
    let commands = |fullscreen: bool, ctx: &egui::Context| -> Vec<String> {
        let mut c = viewer(false);
        c.set_prop("Fullscreen", PropValue::Bool(fullscreen));
        let controls = [c];
        let active = ActiveTabs::new();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), FORM));
        let mut full = ctx.run_ui(input, |root| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(root, |ui| {
                    let inp = RenderInput {
                        controls: &controls,
                        state: &DesignedState,
                        form_size: FORM,
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active,
                        backdrop: Default::default(),
                    };
                    let _ = cobolt_forms::render::render_form(ui, &inp);
                });
        });
        full.textures_delta.clear();
        full.viewport_output
            .values()
            .flat_map(|v| v.commands.iter())
            .filter_map(|c| match c {
                egui::ViewportCommand::Fullscreen(on) => Some(format!("Fullscreen({on})")),
                _ => None,
            })
            .collect()
    };

    let ctx = egui::Context::default();
    let entering = commands(true, &ctx);
    println!("  entering fullscreen: {entering:?}");
    assert!(
        entering.iter().any(|c| c == "Fullscreen(true)"),
        "turning Fullscreen on must ask the window for it: {entering:?}"
    );

    // Asked ONCE: a command re-sent every frame is a window told to do
    // something it is already doing, sixty times a second.
    let staying = commands(true, &ctx);
    println!("  a second fullscreen frame: {staying:?}");
    assert!(
        staying.is_empty(),
        "already fullscreen, nothing more should be asked: {staying:?}"
    );

    let leaving = commands(false, &ctx);
    println!("  leaving fullscreen: {leaving:?}");
    assert!(
        leaving.iter().any(|c| c == "Fullscreen(false)"),
        "turning Fullscreen off must give the window back — this is what left a \
         blank form behind: {leaving:?}"
    );

    // And once back, it stays back.
    let after = commands(false, &ctx);
    println!("  a second windowed frame: {after:?}");
    assert!(after.is_empty(), "nothing further to ask: {after:?}");
}

/// **Entering fullscreen must not read as leaving it** (operator, 2026-09-20:
/// *"Fullscreen now stay in an infinite loop, entering and leaving fullscreen
/// and back"*).
///
/// The overlay asks the window for fullscreen, and on a later frame asks the
/// platform whether it is still there — leaving through the platform's own
/// control (the green button, `Esc`, a Space swipe) must turn the property off,
/// because the property follows the window and never the other way round.
///
/// But a platform does not go fullscreen in the frame it is asked. macOS
/// *animates* into it over hundreds of milliseconds, and reports
/// `fullscreen: Some(false)` for every one of those frames. The check could not
/// tell "not there YET" from "left", so the second frame turned the property
/// off while the first frame's command was still being carried out — and then
/// the window arrived in fullscreen with the property saying otherwise, and the
/// two chased each other forever.
///
/// `fullscreen_asks_the_window_and_gives_it_back` above could not see this: it
/// leaves `RawInput` alone, so the viewport reports `None` and the code's
/// `unwrap_or(true)` keeps the overlay alive. This one says what the platform
/// really says.
#[test]
fn a_platform_that_has_not_gone_fullscreen_yet_is_not_a_platform_leaving_it() {
    // One frame, told exactly what the window's fullscreen state is.
    let frame_with = |ctx: &egui::Context, platform: Option<bool>| -> Vec<(String, String)> {
        let mut c = viewer(false);
        c.set_prop("Fullscreen", PropValue::Bool(true));
        let controls = [c];
        let active = ActiveTabs::new();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), FORM));
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .fullscreen = platform;
        let mut out_props = Vec::new();
        let mut full = ctx.run_ui(input, |root| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(root, |ui| {
                    let inp = RenderInput {
                        controls: &controls,
                        state: &DesignedState,
                        form_size: FORM,
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active,
                        backdrop: Default::default(),
                    };
                    let out = cobolt_forms::render::render_form(ui, &inp);
                    out_props = out
                        .prop_updates
                        .iter()
                        .map(|(_, k, v)| (k.clone(), v.clone()))
                        .collect();
                });
        });
        full.textures_delta.clear();
        out_props
    };

    let ctx = egui::Context::default();

    // Frame 1: the property is on, the window is not fullscreen yet — this is
    // the frame that ASKS. Nothing about the property may change.
    let asking = frame_with(&ctx, Some(false));
    println!("  frame 1, window not yet fullscreen: {asking:?}");
    assert!(
        !asking.iter().any(|(k, v)| k == "Fullscreen" && v == "false"),
        "the frame that asks for fullscreen must not also turn it off: {asking:?}"
    );

    // Frame 2: the platform is STILL animating in. Still not a departure.
    let waiting = frame_with(&ctx, Some(false));
    println!("  frame 2, still animating in:        {waiting:?}");
    assert!(
        !waiting.iter().any(|(k, v)| k == "Fullscreen" && v == "false"),
        "waiting for the platform is not the operator leaving: {waiting:?}"
    );

    // Frame 3: it arrived.
    let arrived = frame_with(&ctx, Some(true));
    println!("  frame 3, window is fullscreen:      {arrived:?}");
    assert!(
        !arrived.iter().any(|(k, v)| k == "Fullscreen" && v == "false"),
        "being fullscreen is not leaving it: {arrived:?}"
    );

    // Frame 4: NOW the window is out of fullscreen, and it was not us — the
    // green button, Esc, a swipe. THIS is the departure the property follows.
    let left = frame_with(&ctx, Some(false));
    println!("  frame 4, the operator left:         {left:?}");
    assert!(
        left.iter().any(|(k, v)| k == "Fullscreen" && v == "false"),
        "leaving through the platform's own control must turn the property off: {left:?}"
    );
}

/// **Text a reader cannot select is text they cannot copy** (operator,
/// 2026-09-20: "Any content but images: text can be selected & copy").
///
/// Driven through the real engine, because a selection is expressed in the
/// runs the PAINT laid down and no other surface knows where those are.
/// Asserted on the clipboard, because what reaches it is the whole claim.
#[test]
fn a_reader_can_select_the_text_and_copy_it() {
    use cobolt_forms::viewer::{TextAnchor, TextSelection};

    let ctx = egui::Context::default();
    let mut c = viewer(false);
    // A Markdown document with words a copy can be recognised by.
    let doc = "# Quarterly report\n\nRevenue rose in every region.\n";
    let dir = std::env::temp_dir().join("prc-viewer-selection-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("report.md");
    std::fs::write(&path, doc).unwrap();
    c.set_prop("Source", PropValue::String(path.to_string_lossy().into_owned()));
    let controls = [c];

    // Two frames: the first lays the document out, the second senses against
    // what it laid down.
    frame(&ctx, &controls, Vec::new());
    frame(&ctx, &controls, Vec::new());

    // Select everything, then copy — the keyboard half, which is the half a
    // reader reaches for without being told.
    let over = pos2(400.0, 300.0);
    let key = |k: egui::Key| egui::Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    };
    let (probe, _) = frame(
        &ctx,
        &controls,
        vec![egui::Event::PointerMoved(over), key(egui::Key::A)],
    );
    let sel_id = cobolt_forms::render::control_widget_id(None, "VWR-1")
        .with(("viewer-view", 0usize))
        .with("viewer-selection");
    let stored = ctx.memory(|m| m.data.get_temp::<TextSelection>(sel_id));
    println!("  after Cmd+A the stored selection is {stored:?}");
    let _ = probe;
    let (out, _) = frame(
        &ctx,
        &controls,
        vec![egui::Event::PointerMoved(over), key(egui::Key::C)],
    );
    let _ = &out;
    let copied = last_copied();
    println!("  Cmd+A then Cmd+C copied {:?}", copied);
    assert!(
        copied.contains("Quarterly report"),
        "Select All then Copy must put the document's text on the clipboard, got {copied:?}"
    );
    assert!(
        copied.contains("Revenue rose in every region."),
        "including its body, got {copied:?}"
    );
    // Runs are joined by a newline, not run together: they are separate
    // BLOCKS, and flattening them pastes a document with its shape gone.
    assert!(
        copied.contains('\n'),
        "separate blocks stay separate lines: {copied:?}"
    );

    // And the arithmetic the paint is handed: a selection of part of one run
    // copies exactly that part.
    let partial = TextSelection {
        anchor: TextAnchor::new(0, 0),
        head: TextAnchor::new(0, 9),
    };
    println!("  a nine-character selection in run 0: {:?}", partial.span_in(0, 17));
    assert_eq!(partial.span_in(0, 17), Some((0, 9)));
}

/// **Fullscreen in, fullscreen out, and it STAYS out** (operator, 2026-09-20:
/// *"Fullscreen now stay in an infinite loop, entering and leaving fullscreen
/// and back"* — and, after the arrival race was fixed, *"loop still happening
/// after leave fullscreen"*).
///
/// A Viewer is drawn on two surfaces: the copy in the form, and the fullscreen
/// overlay. They have separate WIDGET id spaces on purpose — one control must
/// not share widget ids with itself — and the per-view live state was keyed off
/// that same id space, so each surface kept **its own** copy of control-wide
/// values like `Fullscreen`, adopting whatever it last wrote.
///
/// That is a ping-pong with a frame of latency. Entering by the in-form
/// toolbar left the in-form cache owning `true`; the overlay then owned the
/// property for as long as fullscreen lasted, and leaving by the overlay's
/// toolbar left the overlay cache owning `false`. The moment the property came
/// back to the form, the in-form cache re-asserted the `true` it still
/// believed, the overlay re-asserted its `false`, and neither ever saw the
/// other's frames. The operator's trace shows it exactly: `st=false
/// resolved=true` followed by `st=true resolved=false`, forever.
///
/// Live state belongs to the CONTROL; widget ids belong to the SURFACE. This
/// drives the operator's own gesture — in by the toolbar button, out by the
/// toolbar button — and holds the property still afterwards.
#[test]
fn leaving_fullscreen_by_the_toolbar_does_not_ping_pong() {
    let ctx = egui::Context::default();
    let mut c = viewer(false);
    c.set_prop("Fullscreen", PropValue::Bool(false));
    let mut platform: Option<bool> = Some(false);
    let mut pending: Option<bool> = None;

    // One frame: apply what the platform was asked for last time, render, let
    // the host apply the write-backs, and report what was written.
    let mut step = |ctx: &egui::Context,
                    c: &mut cobolt_forms::model::Control,
                    platform: &mut Option<bool>,
                    pending: &mut Option<bool>,
                    click: Option<egui::Pos2>|
     -> Vec<String> {
        if let Some(on) = pending.take() {
            *platform = Some(on);
        }
        let controls = [c.clone()];
        let active = ActiveTabs::new();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), FORM));
        input.viewports.entry(egui::ViewportId::ROOT).or_default().fullscreen = *platform;
        if let Some(at) = click {
            input.events = click_at(at);
        }
        let mut written: Vec<String> = Vec::new();
        let mut full = ctx.run_ui(input, |root| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
                let inp = RenderInput {
                    controls: &controls,
                    state: &DesignedState,
                    form_size: FORM,
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Default::default(),
                };
                let out = cobolt_forms::render::render_form(ui, &inp);
                written = out
                    .prop_updates
                    .iter()
                    .filter(|(_, k, _)| k == "Fullscreen")
                    .map(|(_, _, v)| v.clone())
                    .collect();
            });
        });
        for v in &full
            .viewport_output
            .values()
            .flat_map(|v| v.commands.iter())
            .filter_map(|x| match x {
                egui::ViewportCommand::Fullscreen(on) => Some(*on),
                _ => None,
            })
            .collect::<Vec<bool>>()
        {
            *pending = Some(*v);
        }
        full.textures_delta.clear();
        for v in &written {
            c.set_prop("Fullscreen", PropValue::Bool(v == "true"));
        }
        written
    };

    // Where each surface draws its Fullscreen button: the in-form copy sits at
    // the control's own rect, the overlay across the whole screen.
    let button_at = |x: f32, y: f32, w: f32| -> egui::Pos2 {
        let (_, slot) = cobolt_forms::viewer::toolbar_slots(cobolt_forms::viewer::ViewRect::new(
            x,
            y,
            w,
            cobolt_forms::viewer::TOOLBAR_HEIGHT,
        ))
        .into_iter()
        .find(|(a, _)| *a == cobolt_forms::viewer::ToolbarAction::Fullscreen)
        .expect("the toolbar carries Fullscreen");
        pos2(slot.x + slot.w / 2.0, slot.y + slot.h / 2.0)
    };

    step(&ctx, &mut c, &mut platform, &mut pending, None);

    // ── IN, by the in-form toolbar button ──
    let entered = step(&ctx, &mut c, &mut platform, &mut pending, Some(button_at(40.0, 40.0, 700.0)));
    println!("  click in  -> wrote {entered:?}");
    assert_eq!(entered, vec!["true".to_string()], "the button turns it on");
    for _ in 0..4 {
        step(&ctx, &mut c, &mut platform, &mut pending, None);
    }
    assert_eq!(platform, Some(true), "the window is fullscreen by now");

    // ── OUT, by the overlay's toolbar button ──
    let left = step(&ctx, &mut c, &mut platform, &mut pending, Some(button_at(0.0, 0.0, FORM.x)));
    println!("  click out -> wrote {left:?}");
    assert_eq!(left, vec!["false".to_string()], "the button turns it off");

    // ── and it STAYS off. This is the loop, and where it showed itself. ──
    let mut after: Vec<String> = Vec::new();
    for i in 0..12 {
        let w = step(&ctx, &mut c, &mut platform, &mut pending, None);
        if !w.is_empty() {
            after.push(format!("frame {i}: {w:?}"));
        }
    }
    println!("  writes after leaving: {after:?}");
    assert!(
        after.is_empty(),
        "leaving fullscreen must settle — these frames kept writing the property, \
         which is the ping-pong the operator saw: {after:?}"
    );
    assert_eq!(
        c.get_prop("Fullscreen").map(|v| v.as_bool()),
        Some(false),
        "…and it must be OFF at the end"
    );
}
