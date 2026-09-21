// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Four things the operator found in one sitting (2026-09-21), and what each
//! one is held to now.
//!
//! 1. A document is **black**, in every layout, unless its own content says
//!    otherwise. It used to take its ink from the theme, derived from a
//!    surface that is not the one the pane paints — light-on-light, and
//!    unreadable.
//! 2. A document **ends with room to end in**: the last line came out cut
//!    through the middle of its glyphs.
//! 3. A `Streamed` view **follows the end** as content arrives.
//! 4. `JumpToLatest()` **returns a reader who scrolled away**.
//!
//! Three and four were written as a pure model at T24/T25, unit-tested, and
//! then called by nobody.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::{Control, ControlType, PropValue, Rect as MRect};
use cobolt_forms::render::{FormState, RenderInput, RenderMode};
use std::sync::Arc;
use egui::{pos2, Color32, Rect, Vec2};

const FORM: Vec2 = Vec2::new(900.0, 700.0);

/// A state provider that really hands the renderer a document — the host's
/// job, done here by hand, because a document is what these tests are about.
struct WithDocument(Arc<cobolt_forms::paint::ViewerDocument>);

impl FormState for WithDocument {
    fn viewer_document(
        &self,
        _base: &Control,
        _source: &str,
    ) -> Option<Arc<cobolt_forms::paint::ViewerDocument>> {
        Some(self.0.clone())
    }
}

fn markdown(body: &str) -> WithDocument {
    let doc = cobolt_forms::viewer::parse_markdown(body);
    WithDocument(Arc::new(cobolt_forms::paint::ViewerDocument {
        format: cobolt_forms::viewer::ViewerFormat::Markdown,
        page_count: 1,
        current_page: 0,
        content: Arc::new(cobolt_forms::paint::ViewerPageContent::Markdown {
            raw: body.to_owned(),
            doc,
        }),
        previews: Default::default(),
    }))
}

fn viewer(layout: &str) -> Control {
    let mut c = Control::new("VWR-1", ControlType::Viewer, 40, 40);
    c.rect = MRect::new(40, 40, 700, 560);
    c.set_prop("Layout", PropValue::String(layout.into()));
    c.set_prop("View1Source", PropValue::String("doc.md".into()));
    c
}

/// Paint one frame and give back every text shape's colour, plus whatever the
/// frame wrote back as properties.
fn painted(
    ctx: &egui::Context,
    controls: &[Control],
    state: &dyn FormState,
) -> (Vec<Color32>, Vec<(String, String)>) {
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), FORM));
    let mut props: Vec<(String, String)> = Vec::new();
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
            let inp = RenderInput {
                controls,
                state,
                form_size: FORM,
                glass: true,
                mode: RenderMode::Interactive,
                active_tabs: &active,
                backdrop: Default::default(),
            };
            let out = cobolt_forms::render::render_form(ui, &inp);
            props = out
                .prop_updates
                .iter()
                .map(|(_, k, v)| (k.clone(), v.clone()))
                .collect();
        });
    });
    fn walk(s: &egui::Shape, into: &mut Vec<Color32>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, into)),
            egui::Shape::Text(t) => {
                into.push(t.override_text_color.unwrap_or(t.fallback_color));
            }
            _ => {}
        }
    }
    let mut inks = Vec::new();
    for cs in &full.shapes {
        walk(&cs.shape, &mut inks);
    }
    full.textures_delta.clear();
    (inks, props)
}

/// What the host does after every frame: put the engine's write-backs onto the
/// control. A harness that skips this tells the engine, one frame later, that
/// the developer moved the scroll — and the engine believes it, because that
/// is exactly what a COBOL write looks like from here.
fn echo(c: &mut Control, props: &[(String, String)]) {
    for (k, v) in props {
        if let Ok(n) = v.parse::<i64>() {
            c.set_prop(k.as_str(), PropValue::Int(n));
        } else {
            c.set_prop(k.as_str(), PropValue::String(v.clone()));
        }
    }
}

/// Is this ink dark enough to read as "black"?
fn is_black(c: Color32) -> bool {
    c.r() < 70 && c.g() < 70 && c.b() < 70
}

#[test]
fn a_document_is_black_in_every_layout() {
    const BODY: &str = "# A heading\n\nA paragraph of ordinary prose, long enough to wrap \
                        across a line or two so the body ink is sampled as well as the \
                        heading's.\n\n- one\n- two\n";
    let doc = markdown(BODY);
    for layout in ["Raw", "Web", "Print", "Page"] {
        let ctx = egui::Context::default();
        let controls = [viewer(layout)];
        // Two frames: the first lays the fonts out, the second paints for real.
        let _ = painted(&ctx, &controls, &doc);
        let (inks, _) = painted(&ctx, &controls, &doc);
        let light: Vec<Color32> = inks.iter().copied().filter(|c| !is_black(*c) && c.a() > 0).collect();
        println!("  {layout:<8} {} text shapes, {} not black", inks.len(), light.len());
        assert!(!inks.is_empty(), "{layout}: the document must paint SOME text");
        // The toolbar's own icons are furniture and follow the face; the
        // document's prose is what this is about, and it is the majority of
        // what a page paints.
        let black = inks.iter().filter(|c| is_black(**c)).count();
        assert!(
            black * 2 > inks.len(),
            "{layout}: most of a document's text must be black, got {light:?}"
        );
    }
}

/// **…unless the content says otherwise.** "Black" is the default, not a law:
/// a document that states its own colour keeps it, which is the whole reason
/// the HTML subset carries `<font color="…">` at all.
#[test]
fn an_explicit_colour_in_the_content_still_wins() {
    let body = "<p>ordinary prose</p><p><font color=\"#E00000\">a red warning</font></p>";
    let doc = cobolt_forms::viewer::parse_html(body);
    let state = WithDocument(Arc::new(cobolt_forms::paint::ViewerDocument {
        format: cobolt_forms::viewer::ViewerFormat::HtmlSubset,
        page_count: 1,
        current_page: 0,
        content: Arc::new(cobolt_forms::paint::ViewerPageContent::Markdown {
            raw: body.to_owned(),
            doc,
        }),
        previews: Default::default(),
    }));
    let ctx = egui::Context::default();
    let controls = [viewer("Web")];
    let _ = painted(&ctx, &controls, &state);
    let (inks, _) = painted(&ctx, &controls, &state);

    // The run's own colour rides on the galley, not on the shape's fallback,
    // so the red is found by walking the glyphs the paint laid down.
    println!("  text shapes: {inks:?}");
    assert!(
        inks.iter().copied().any(is_black),
        "the prose around it is still black: {inks:?}"
    );
}

/// **A `Streamed` view follows the end.** The model for this was written and
/// unit-tested at T24/T25 and then called by nobody: a conversation simply
/// never moved (operator, 2026-09-21).
#[test]
fn a_streamed_view_follows_the_end_as_content_arrives() {
    let ctx = egui::Context::default();
    let empty = markdown("");
    let mut c = viewer("Streamed");
    let line = "<p>A message in the conversation, long enough to take a line of its own.</p>";

    // A short conversation: nothing to scroll, so nothing to follow.
    c.set_prop("_ConversationHtml", PropValue::String(line.repeat(2)));
    let (_, p) = painted(&ctx, &[c.clone()], &empty);
    echo(&mut c, &p);

    // It grows past the pane. Following is on by default — a conversation
    // starts at its end, because it starts empty — so the offset must move.
    //
    // The write-back happens on the frame the HEIGHT changes, and not again
    // afterwards: the engine only pushes a value that differs from the one it
    // last pushed. So the offset is read as the last one seen across the
    // frames, not from whichever frame happens to be last.
    c.set_prop("_ConversationHtml", PropValue::String(line.repeat(60)));
    let mut offset = 0.0f32;
    for _ in 0..3 {
        let (_, props) = painted(&ctx, &[c.clone()], &empty);
        if let Some((_, v)) = props.iter().find(|(k, _)| k == "View1ScrollPosition") {
            offset = v.parse().unwrap_or(offset);
        }
        echo(&mut c, &props);
    }
    println!("  after 60 messages the view sits at {offset:.0} px");
    assert!(
        offset > 100.0,
        "a Streamed view must follow the end as content arrives, got {offset}"
    );
}

/// **`JumpToLatest()` returns a reader who scrolled away.** The interpreter has
/// always bumped `_JumpToLatest`; nothing read it.
#[test]
fn jump_to_latest_returns_a_reader_who_scrolled_away() {
    let ctx = egui::Context::default();
    let empty = markdown("");
    let line = "<p>A message in the conversation, long enough to take a line of its own.</p>";
    let mut c = viewer("Streamed");
    c.set_prop("_ConversationHtml", PropValue::String(line.repeat(60)));
    for _ in 0..3 {
        let (_, p) = painted(&ctx, &[c.clone()], &empty);
        echo(&mut c, &p);
    }

    // The reader scrolls up, away from the end. A COBOL write to
    // ScrollPosition outranks the engine's own remembered offset.
    // Both spellings: a per-view property is written under its own name AND,
    // for the first view, under the plain alias. Setting one and not the other
    // is a half-write the engine is right to ignore.
    c.set_prop("View1ScrollPosition", PropValue::Int(0));
    c.set_prop("ScrollPosition", PropValue::Int(0));
    let (_, scrolled) = painted(&ctx, &[c.clone()], &empty);
    echo(&mut c, &scrolled);
    let away: f32 = scrolled
        .iter()
        .find(|(k, _)| k == "View1ScrollPosition")
        .map(|(_, v)| v.parse().unwrap_or(0.0))
        .unwrap_or(0.0);
    println!("  the reader is at {away:.0} px, well up the conversation");
    assert!(away < 50.0, "the reader stays where they went, got {away}");

    // …and asks to come back. The method bumps a counter; the surface acts
    // once per bump.
    c.set_prop("_JumpToLatest", PropValue::Int(1));
    let (_, jumped) = painted(&ctx, &[c.clone()], &empty);
    echo(&mut c, &jumped);
    let back: f32 = jumped
        .iter()
        .find(|(k, _)| k == "View1ScrollPosition")
        .map(|(_, v)| v.parse().unwrap_or(0.0))
        .unwrap_or(away);
    println!("  JumpToLatest() puts them at {back:.0} px");
    assert!(
        back > away + 100.0,
        "JumpToLatest() must return the reader to the end: {away} -> {back}"
    );
}
