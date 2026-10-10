// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The IDE's per-project **background image**: which file is painted, the
//! texture it was decoded into, and the moments that texture is let go.
//!
//! This used to live inside `CoboltApp`, where the only way to ask "does the
//! background go away when the path is removed?" was to run the whole IDE. It
//! is a unit of its own now so that the question has a headless test: a
//! `Context`, a throwaway PNG and a read of the shapes the frame produced.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use egui::{Color32, Context, LayerId, Rect, TextureHandle, Vec2};

use crate::project_model::CoboltProject;

/// The file a project names as its background, resolved against the folder of
/// its manifest. `None` when it names none (an empty or blank path), and for a
/// relative path when the manifest's location is not known.
pub fn project_image_path(project: &CoboltProject, project_file: Option<&Path>) -> Option<PathBuf> {
    let raw = project.ide.background_image.trim();
    if raw.is_empty() {
        return None;
    }
    let p = Path::new(raw);
    if p.is_absolute() {
        return Some(p.to_path_buf());
    }
    Some(project_file?.parent()?.join(p))
}

/// The decoded background image, kept between frames.
///
/// A cache is only ever correct while it agrees with what the project names, so
/// [`Self::paint`] is told that every frame and lets go of the texture the
/// moment it no longer applies: the path is gone, the file is gone, or the file
/// on disk is not the one that was decoded.
#[derive(Default)]
pub struct BackgroundCache {
    attempt: Option<Attempt>,
}

/// One decode of one file. A file that would not decode is remembered too
/// (`texture: None`), so a corrupt image costs one read, not one per frame.
struct Attempt {
    path: PathBuf,
    modified: Option<SystemTime>,
    texture: Option<TextureHandle>,
}

impl BackgroundCache {
    /// The background setting changed (Save, Browse, Clear): drop what was
    /// decoded and ask for a frame, so the next paint reflects the new setting
    /// instead of waiting for whatever input happens to arrive.
    pub fn reset(&mut self, ctx: &Context) {
        self.attempt = None;
        ctx.request_repaint();
    }

    /// Paint the project's background on the window's background layer, scaled
    /// to cover and tinted by `opacity` (0 = invisible, 100 = fully opaque).
    ///
    /// `image` is [`project_image_path`]; `None` paints nothing and drops any
    /// texture still cached for a path the project no longer names.
    pub fn paint(&mut self, ctx: &Context, image: Option<&Path>, opacity: u8) {
        let Some(path) = image else {
            self.attempt = None;
            return;
        };
        let opacity = opacity.min(100);
        if opacity == 0 {
            return;
        }
        // The file is checked every frame — one `stat`, which is nothing next
        // to a frame — so a deleted image stops being painted, and a replaced
        // one is read again, rather than the first decode living on for the
        // whole session.
        let Ok(meta) = std::fs::metadata(path) else {
            self.attempt = None;
            return;
        };
        let modified = meta.modified().ok();
        let current = matches!(
            &self.attempt,
            Some(a) if a.path == path && a.modified == modified
        );
        if !current {
            self.attempt = Some(Attempt {
                path: path.to_path_buf(),
                modified,
                texture: cobolt_forms::paint::load_image_texture(ctx, &path.display().to_string()),
            });
        }
        let Some(texture) = self.attempt.as_ref().and_then(|a| a.texture.as_ref()) else {
            return;
        };
        let tex_size = texture.size_vec2();
        if tex_size.x <= 0.0 || tex_size.y <= 0.0 {
            return;
        }
        let dest = cover(ctx.content_rect(), tex_size);
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        // Drawn over the opaque floor, scaled by `background_opacity` so the
        // texture shows through more or less.
        let alpha = (opacity as f32 / 100.0 * 255.0) as u8;
        ctx.layer_painter(LayerId::background())
            .image(texture.id(), dest, uv, Color32::from_white_alpha(alpha));
    }

    #[cfg(test)]
    fn texture_id(&self) -> Option<egui::TextureId> {
        self.attempt
            .as_ref()
            .and_then(|a| a.texture.as_ref())
            .map(|t| t.id())
    }
}

/// Scale `tex` up until it fills `screen` in both directions, centred: the
/// image covers the window and loses its overflow at the edges.
fn cover(screen: Rect, tex: Vec2) -> Rect {
    let s = (screen.width() / tex.x).max(screen.height() / tex.y);
    let (dw, dh) = (tex.x * s, tex.y * s);
    let offset = egui::vec2((screen.width() - dw) * 0.5, (screen.height() - dh) * 0.5);
    Rect::from_min_size(screen.min + offset, egui::vec2(dw, dh))
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, PointerButton, Pos2, RawInput, TextureId};

    /// A solid-colour PNG of the given size, on disk.
    fn write_png(path: &Path, w: u32, h: u32, rgb: [u8; 3]) {
        let img = image::RgbImage::from_pixel(w, h, image::Rgb(rgb));
        img.save(path).expect("write the test image");
    }

    fn input() -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0))),
            ..Default::default()
        }
    }

    /// Every texture the frame sampled besides the glyph atlas (`Managed(0)`):
    /// the pictures on screen. The background is the only thing these frames
    /// draw, so a non-empty answer means "an image is showing".
    fn pictures(mut out: egui::FullOutput) -> Vec<TextureId> {
        fn walk(shape: &egui::Shape, into: &mut Vec<TextureId>) {
            match shape {
                egui::Shape::Mesh(m) if m.texture_id != TextureId::default() => {
                    into.push(m.texture_id);
                }
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, into)),
                _ => {}
            }
        }
        let mut found = Vec::new();
        for clipped in &out.shapes {
            walk(&clipped.shape, &mut found);
        }
        out.textures_delta.clear();
        found
    }

    /// One frame in which the cache is asked to paint `image`.
    fn frame(
        ctx: &Context,
        cache: &mut BackgroundCache,
        image: Option<&Path>,
        opacity: u8,
    ) -> Vec<TextureId> {
        pictures(ctx.run_ui(input(), |ui| cache.paint(ui.ctx(), image, opacity)))
    }

    fn scratch(tag: &str) -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix(&format!("prc-ide-bg-{tag}-"))
            .tempdir()
            .expect("temp dir")
    }

    #[test]
    fn a_project_without_an_image_names_no_path() {
        let mut p = CoboltProject::new("Demo", "src/main.cbl");
        let file = Path::new("/work/demo/cobolt.toml");
        assert_eq!(project_image_path(&p, Some(file)), None);
        p.ide.background_image = "   ".into();
        assert_eq!(project_image_path(&p, Some(file)), None, "blank is none");
    }

    #[test]
    fn a_relative_image_is_resolved_against_the_manifests_folder() {
        let mut p = CoboltProject::new("Demo", "src/main.cbl");
        p.ide.background_image = "Assets/bg.png".into();
        let file = Path::new("/work/demo/cobolt.toml");
        assert_eq!(
            project_image_path(&p, Some(file)),
            Some(PathBuf::from("/work/demo/Assets/bg.png"))
        );
        assert_eq!(project_image_path(&p, None), None, "no manifest, no anchor");
        p.ide.background_image = "/elsewhere/bg.png".into();
        assert_eq!(
            project_image_path(&p, None),
            Some(PathBuf::from("/elsewhere/bg.png")),
            "an absolute path needs no anchor"
        );
    }

    /// An image that is set is on screen, and it is the cached texture.
    #[test]
    fn an_image_that_is_set_is_painted() {
        let dir = scratch("set");
        let png = dir.path().join("bg.png");
        write_png(&png, 64, 32, [200, 30, 30]);
        let ctx = Context::default();
        let mut cache = BackgroundCache::default();

        let shown = frame(&ctx, &mut cache, Some(&png), 70);
        assert_eq!(shown.len(), 1, "one picture on the background layer");
        assert_eq!(Some(shown[0]), cache.texture_id(), "and it is the cached one");
    }

    /// THE REPORTED CASE. The path is removed: the next frame draws no image,
    /// and the texture is let go — not merely hidden behind an early return.
    #[test]
    fn removing_the_path_removes_the_image_and_drops_the_texture() {
        let dir = scratch("remove");
        let png = dir.path().join("bg.png");
        write_png(&png, 64, 32, [30, 30, 200]);
        let ctx = Context::default();
        let mut cache = BackgroundCache::default();
        assert_eq!(frame(&ctx, &mut cache, Some(&png), 70).len(), 1);

        let shown = frame(&ctx, &mut cache, None, 70);
        assert!(shown.is_empty(), "no path, no picture: {shown:?}");
        assert!(cache.texture_id().is_none(), "the stale texture is dropped");
        // …and it stays gone on every later frame.
        for _ in 0..3 {
            assert!(frame(&ctx, &mut cache, None, 70).is_empty());
        }
    }

    /// The same, when the path is removed and the cache is never told so by the
    /// caller (the old code kept the texture and relied on an early return).
    #[test]
    fn a_stale_cache_does_not_outlive_the_path_it_was_decoded_from() {
        let dir = scratch("stale");
        let png = dir.path().join("bg.png");
        write_png(&png, 16, 16, [10, 120, 10]);
        let ctx = Context::default();
        let mut cache = BackgroundCache::default();
        frame(&ctx, &mut cache, Some(&png), 100);
        assert!(cache.texture_id().is_some());
        // No `reset`, no other signal: only the next paint's arguments.
        assert!(frame(&ctx, &mut cache, None, 100).is_empty());
        assert!(cache.texture_id().is_none());
    }

    /// Browsing to another file replaces the picture; the old texture goes.
    #[test]
    fn a_changed_path_is_loaded_again() {
        let dir = scratch("change");
        let (a, b) = (dir.path().join("a.png"), dir.path().join("b.png"));
        write_png(&a, 32, 32, [255, 0, 0]);
        write_png(&b, 48, 24, [0, 0, 255]);
        let ctx = Context::default();
        let mut cache = BackgroundCache::default();
        let first = frame(&ctx, &mut cache, Some(&a), 70);
        let second = frame(&ctx, &mut cache, Some(&b), 70);
        assert_eq!((first.len(), second.len()), (1, 1));
        assert_ne!(first[0], second[0], "a different texture for a different file");
        assert_eq!(Some(second[0]), cache.texture_id());
        // Same file again: the cache is reused, not rebuilt.
        let third = frame(&ctx, &mut cache, Some(&b), 70);
        assert_eq!(third, second);
    }

    /// The file itself is deleted while its path is still set.
    #[test]
    fn a_deleted_file_is_no_longer_painted() {
        let dir = scratch("delete");
        let png = dir.path().join("bg.png");
        write_png(&png, 32, 32, [90, 90, 90]);
        let ctx = Context::default();
        let mut cache = BackgroundCache::default();
        assert_eq!(frame(&ctx, &mut cache, Some(&png), 70).len(), 1);

        std::fs::remove_file(&png).unwrap();
        assert!(frame(&ctx, &mut cache, Some(&png), 70).is_empty());
        assert!(cache.texture_id().is_none());

        // Put a file back and it appears again, with no restart.
        write_png(&png, 32, 32, [90, 90, 90]);
        assert_eq!(frame(&ctx, &mut cache, Some(&png), 70).len(), 1);
    }

    /// A replaced file at the same path is read again.
    #[test]
    fn a_replaced_file_is_read_again() {
        let dir = scratch("replace");
        let png = dir.path().join("bg.png");
        write_png(&png, 32, 32, [1, 2, 3]);
        let ctx = Context::default();
        let mut cache = BackgroundCache::default();
        let before = frame(&ctx, &mut cache, Some(&png), 70);

        // Filesystems stamp to a coarse tick; make the new file unmistakably newer.
        write_png(&png, 40, 40, [4, 5, 6]);
        let later = SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(&png)
            .unwrap()
            .set_modified(later)
            .unwrap();

        let after = frame(&ctx, &mut cache, Some(&png), 70);
        assert_ne!(before, after, "the new bytes got a new texture");
    }

    /// 0 % is invisible: nothing is drawn, and nothing is decoded for it.
    #[test]
    fn zero_opacity_paints_nothing() {
        let dir = scratch("zero");
        let png = dir.path().join("bg.png");
        write_png(&png, 8, 8, [1, 1, 1]);
        let ctx = Context::default();
        let mut cache = BackgroundCache::default();
        assert!(frame(&ctx, &mut cache, Some(&png), 0).is_empty());
        assert!(cache.texture_id().is_none());
        // Turned up again, it appears.
        assert_eq!(frame(&ctx, &mut cache, Some(&png), 40).len(), 1);
    }

    /// A file that is not an image paints nothing, is not retried every frame,
    /// and does not take the frame down.
    #[test]
    fn a_file_that_is_not_an_image_paints_nothing() {
        let dir = scratch("junk");
        let junk = dir.path().join("bg.png");
        std::fs::write(&junk, b"this is not a PNG").unwrap();
        let ctx = Context::default();
        let mut cache = BackgroundCache::default();
        for _ in 0..3 {
            assert!(frame(&ctx, &mut cache, Some(&junk), 70).is_empty());
        }
        assert!(cache.attempt.as_ref().is_some_and(|a| a.texture.is_none()));
    }

    /// A setting change asks for a frame, so it never waits on stray input.
    #[test]
    fn a_setting_change_requests_a_repaint() {
        let ctx = Context::default();
        let mut cache = BackgroundCache::default();
        // Let the context settle: a fresh one owes its first frame.
        for _ in 0..4 {
            if !ctx.has_requested_repaint() {
                break;
            }
            ctx.run_ui(input(), |_| {}).textures_delta.clear();
        }
        assert!(!ctx.has_requested_repaint(), "the context is idle");
        cache.reset(&ctx);
        assert!(ctx.has_requested_repaint(), "reset asks for the next frame");
    }

    /// Suspect checked and ruled out: the frame in which a button is clicked
    /// (Save, Clear) is followed by another at once, so a stale picture painted
    /// in the click frame does not stay on screen waiting for more input.
    #[test]
    fn the_frame_after_a_click_is_requested_immediately() {
        let ctx = Context::default();
        let at = Pos2::new(40.0, 20.0);
        let click = |pressed: bool| RawInput {
            events: vec![Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }],
            ..input()
        };
        let mut moved = input();
        moved.events.push(Event::PointerMoved(at));
        ctx.run_ui(moved, |ui| {
            ui.button("Save");
        })
        .textures_delta
        .clear();
        ctx.run_ui(click(true), |ui| {
            ui.button("Save");
        })
        .textures_delta
        .clear();
        let mut released = ctx.run_ui(click(false), |ui| {
            ui.button("Save");
        });
        released.textures_delta.clear();
        let delay = released
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|v| v.repaint_delay)
            .expect("the root viewport reported");
        assert_eq!(delay, std::time::Duration::ZERO, "a click frame schedules the next");
    }

    #[test]
    fn the_image_covers_the_window_and_stays_centred() {
        let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        // Wider than the window: height fits, the sides overflow equally.
        let wide = cover(screen, egui::vec2(400.0, 200.0));
        assert_eq!(wide.height(), 600.0);
        assert_eq!(wide.width(), 1200.0);
        assert_eq!(wide.center(), screen.center());
        // Taller than the window: width fits, top and bottom overflow equally.
        let tall = cover(screen, egui::vec2(100.0, 400.0));
        assert_eq!(tall.width(), 800.0);
        assert_eq!(tall.height(), 3200.0);
        assert_eq!(tall.center(), screen.center());
    }

    /// Opacity scales the tint's alpha, not the colour.
    #[test]
    fn opacity_is_the_alpha_of_the_tint() {
        let dir = scratch("alpha");
        let png = dir.path().join("bg.png");
        write_png(&png, 8, 8, [255, 255, 255]);
        let ctx = Context::default();
        let mut cache = BackgroundCache::default();
        let mut out = ctx.run_ui(input(), |ui| cache.paint(ui.ctx(), Some(&png), 50));
        out.textures_delta.clear();
        let alphas: Vec<u8> = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Mesh(m) if m.texture_id != TextureId::default() => {
                    m.vertices.first().map(|v| v.color.a())
                }
                _ => None,
            })
            .collect();
        assert_eq!(alphas, vec![127], "50 % of 255, truncated as it always was");
    }
}
