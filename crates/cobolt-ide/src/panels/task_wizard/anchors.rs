// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Where the Task Wizard's targets were painted, this frame (spec 104 R20, R35).
//!
//! A widget that a step can point at reports its rect with [`mark`] from wherever it is
//! drawn. The rects land in a per-thread sink that exists **only while the wizard is on**:
//! with it off, [`mark`] is one check of an empty slot, records nothing and allocates
//! nothing (R35). Every viewport of the IDE — the main window and each Form Designer, which
//! egui runs immediately, on this thread, inside the main window's frame — reports into the
//! same sink, so one frame's [`Marks`] describe every window.
//!
//! The sink also carries the anchor the engine wants **revealed**: when that anchor's rect
//! lies outside the clip of the scroll area it is drawn in, [`mark`] asks that scroll area to
//! bring it into view, from inside the very `Ui` that owns the scroll — which is the only
//! place egui can do it.

use std::cell::RefCell;

use eframe::egui::{self, LayerId, Rect, ViewportId};

/// A thing a step can point at, or light as its live region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnchorId {
    /// The Task Wizard toggle and its ▾, on either toolbar — always a hole in the dim (R16).
    Toggle,
    /// The project tree's root node: Project Settings.
    ProjectRoot,
    /// The project tree's *Forms* category header.
    FormsCategory,
    /// The Settings pane's *Model Providers…* button.
    SettingsModelProviders,
    /// The Settings pane's *Manage agents…* button.
    SettingsManageAgents,
    /// The Model Providers Manager's window.
    ModelsWindow,
    /// The manager's provider list.
    ModelsProviderList,
    /// The manager's endpoint field.
    ModelsEndpoint,
    /// The manager's key field **and** its Test button, as one rect. The wizard points at the
    /// field; it never reads it (R28).
    ModelsKeyAndTest,
    /// The manager's Save button.
    ModelsSave,
    /// The Agents Manager's window.
    AgentsWindow,
    /// The Agents Manager's *Agent × Model* tab button.
    AgentsModelTab,
    /// The provider scope picker on that tab.
    AgentsScope,
    /// The grid of agents and their models on that tab.
    AgentsGrid,
    /// The Agents Manager's Save button, which also closes it.
    AgentsSave,
    /// A Form Designer's canvas.
    Canvas,
    /// The task's control, where the canvas paints it.
    SubjectOnCanvas,
    /// The floating toolbar of the canvas's DataGrid edit mode.
    GridEditToolbar,
    /// A Form Designer's Properties pane.
    PropertiesPane,
    /// The ◀ that reopens a hidden Properties pane.
    PropertiesReopen,
    /// The Properties pane's *Visuals* tab.
    PropsTabVisuals,
    /// The Properties pane's *Events* tab.
    PropsTabEvents,
    /// The Properties pane's *Anim* tab.
    AnimTab,
    /// The Properties pane's Data Binding source buttons (or *Edit current binding*).
    PropertiesBinding,
    /// The Data Binding editor's window.
    BindingWindow,
    /// The editor's source cards.
    BindingSource,
    /// The editor's list of source fields.
    BindingFields,
    /// The editor's Apply button.
    BindingApply,
    /// The Properties pane's *Edit DataGrid settings…* button.
    GridSettingsButton,
    /// The DataGrid settings window; it closes with its ✕.
    GridSettingsWindow,
    /// That window's Columns section.
    GridSettingsColumns,
    /// The TreeView's *Nodes* box in Properties.
    TreeItems,
    /// A map's centre rows (latitude and longitude) in Properties.
    MapCenter,
    /// A map's Zoom row in Properties.
    MapZoom,
    /// A map's colour rows in Properties.
    MapColors,
    /// The Properties pane's *Edit Menu…* button (MenuBar and SideMenu).
    MenuEditButton,
    /// The menu editor's window.
    MenuEditorWindow,
    /// The menu editor's *Add item* button.
    MenuAddItem,
    /// The menu editor's action and target-form pickers.
    MenuItemTarget,
    /// The menu editor's shortcut capture.
    MenuItemShortcut,
    /// The menu editor's Save button.
    MenuSave,
    /// The animations tab's ➕ Add button.
    AnimAdd,
    /// The animation's Trigger, Effect and Repeat rows.
    AnimFields,
    /// The animation's ▶ Preview button.
    AnimPreview,
}

impl AnchorId {
    /// Is it painted in a Form Designer's own window (rather than the IDE's)?
    pub fn in_designer(self) -> bool {
        use AnchorId::*;
        match self {
            Toggle | ProjectRoot | FormsCategory | SettingsModelProviders
            | SettingsManageAgents | ModelsWindow | ModelsProviderList | ModelsEndpoint
            | ModelsKeyAndTest | ModelsSave | AgentsWindow | AgentsModelTab | AgentsScope
            | AgentsGrid | AgentsSave => false,
            Canvas | SubjectOnCanvas | GridEditToolbar | PropertiesPane | PropertiesReopen
            | PropsTabVisuals | PropsTabEvents | AnimTab | PropertiesBinding | BindingWindow
            | BindingSource | BindingFields | BindingApply | GridSettingsButton
            | GridSettingsWindow | GridSettingsColumns | TreeItems | MapCenter | MapZoom
            | MapColors | MenuEditButton
            | MenuEditorWindow | MenuAddItem | MenuItemTarget | MenuItemShortcut | MenuSave
            | AnimAdd | AnimFields | AnimPreview => true,
        }
    }
}

/// One rect a frame painted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mark {
    pub id: AnchorId,
    /// Where it was painted.
    pub rect: Rect,
    /// The clip it was painted under. A rect outside it is scrolled out of view.
    pub clip: Rect,
    /// The OS window it was painted in.
    pub viewport: ViewportId,
    /// The layer of a window anchor ([`mark_window`]), so the overlay can raise it.
    pub layer: Option<LayerId>,
}

impl Mark {
    /// Is the whole rect on screen? Half a pixel of slack for rounding at a clip edge.
    pub fn on_screen(&self) -> bool {
        self.rect.width() > 0.0
            && self.rect.height() > 0.0
            && self.clip.expand(0.5).contains_rect(self.rect)
    }
}

/// Every rect one frame painted, in painting order.
#[derive(Debug, Clone, Default)]
pub struct Marks {
    marks: Vec<Mark>,
}

impl Marks {
    /// Record a rect. Public for tests and for fixtures; the IDE uses [`mark`].
    pub fn push(&mut self, mark: Mark) {
        self.marks.push(mark);
    }

    /// Where `id` was painted, preferring `viewport` when several windows painted one (each
    /// Form Designer has its own canvas and its own toggle), else the window that painted it
    /// last. Within that window every mark of `id` is **united**: a target made of several
    /// widgets — an animation's Trigger, Effect and Repeat rows, which the pane sorts A–Z —
    /// is marked once per widget. The clip is the last mark's.
    pub fn find(&self, id: AnchorId, prefer: Option<ViewportId>) -> Option<Mark> {
        let last = self
            .marks
            .iter()
            .rev()
            .filter(|m| m.id == id)
            .find(|m| prefer.is_none() || Some(m.viewport) == prefer)
            .or_else(|| self.marks.iter().rev().find(|m| m.id == id))?;
        let mut out = *last;
        for m in self.marks.iter().filter(|m| m.id == id && m.viewport == last.viewport) {
            out.rect = out.rect.union(m.rect);
            out.layer = out.layer.or(m.layer);
        }
        Some(out)
    }

    /// Every mark painted in `viewport` for `id`.
    pub fn all_in(&self, id: AnchorId, viewport: ViewportId) -> impl Iterator<Item = &Mark> {
        self.marks
            .iter()
            .filter(move |m| m.id == id && m.viewport == viewport)
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }
}

struct Sink {
    marks: Marks,
    reveal: Option<AnchorId>,
}

thread_local! {
    static SINK: RefCell<Option<Sink>> = const { RefCell::new(None) };
}

/// Open the sink for one frame. Called by the IDE at the start of a frame, **only** while
/// the wizard is on; `reveal` is the anchor the engine asked to bring into view.
pub fn install(reveal: Option<AnchorId>) {
    SINK.with(|s| {
        *s.borrow_mut() = Some(Sink {
            marks: Marks::default(),
            reveal,
        })
    });
}

/// Close the sink and hand back what the frame painted. With no sink open, empty.
pub fn take() -> Marks {
    SINK.with(|s| s.borrow_mut().take().map(|s| s.marks).unwrap_or_default())
}

/// The marks so far this frame, without closing the sink — for a viewport's overlay, which
/// paints before the frame ends.
pub fn snapshot() -> Marks {
    SINK.with(|s| s.borrow().as_ref().map(|s| s.marks.clone()).unwrap_or_default())
}

/// Is a sink open? `false` whenever the wizard is off — so a pane that keeps rects for the
/// wizard (the Properties rows) keeps none then.
pub fn is_open() -> bool {
    SINK.with(|s| s.borrow().is_some())
}

/// Report where `id` was painted. One check and nothing else while the wizard is off.
pub fn mark(ui: &egui::Ui, id: AnchorId, rect: Rect) {
    SINK.with(|s| {
        let mut slot = s.borrow_mut();
        let Some(sink) = slot.as_mut() else {
            return;
        };
        let clip = ui.clip_rect();
        sink.marks.push(Mark {
            id,
            rect,
            clip,
            viewport: ui.ctx().viewport_id(),
            layer: None,
        });
        if sink.reveal == Some(id) && !clip.contains_rect(rect) {
            ui.scroll_to_rect(rect, Some(egui::Align::Center));
        }
    });
}

/// Report a rect that a panel kept from its own frame — the project tree records its rows
/// for the Walkthrough — with the clip it was drawn under. Bringing it into view is the
/// panel's own business (`ProjectPanel::reveal_walkthrough`).
pub fn mark_rect(viewport: ViewportId, id: AnchorId, rect: Rect, clip: Rect) {
    SINK.with(|s| {
        let mut slot = s.borrow_mut();
        let Some(sink) = slot.as_mut() else {
            return;
        };
        sink.marks.push(Mark {
            id,
            rect,
            clip,
            viewport,
            layer: None,
        });
    });
}

/// Report a window's outer rect and layer. A window is never clipped, so its clip is itself.
pub fn mark_window(ctx: &egui::Context, id: AnchorId, response: &egui::Response) {
    SINK.with(|s| {
        let mut slot = s.borrow_mut();
        let Some(sink) = slot.as_mut() else {
            return;
        };
        sink.marks.push(Mark {
            id,
            rect: response.rect,
            clip: response.rect,
            viewport: ctx.viewport_id(),
            layer: Some(response.layer_id),
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: AnchorId, vp: u64, x: f32) -> Mark {
        Mark {
            id,
            rect: Rect::from_min_size(egui::pos2(x, 0.0), egui::vec2(10.0, 10.0)),
            clip: Rect::EVERYTHING,
            viewport: ViewportId::from_hash_of(vp),
            layer: None,
        }
    }

    #[test]
    fn with_no_sink_a_mark_records_nothing() {
        let _ = take();
        assert!(!is_open());
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            mark(ui, AnchorId::Canvas, Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(5.0, 5.0)));
        });
        out.textures_delta.clear();
        assert!(take().is_empty(), "a mark with the wizard off must record nothing (R35)");
    }

    #[test]
    fn a_sink_collects_every_mark_until_taken() {
        install(None);
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            mark(ui, AnchorId::Canvas, Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(5.0, 5.0)));
            mark(ui, AnchorId::Toggle, Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(5.0, 5.0)));
        });
        out.textures_delta.clear();
        let marks = take();
        assert!(marks.find(AnchorId::Canvas, None).is_some());
        assert!(marks.find(AnchorId::Toggle, None).is_some());
        assert!(!is_open(), "take closes the sink");
    }

    #[test]
    fn the_preferred_window_wins_and_its_marks_are_united() {
        let mut marks = Marks::default();
        marks.push(m(AnchorId::Canvas, 1, 0.0));
        marks.push(m(AnchorId::Canvas, 2, 20.0));
        marks.push(m(AnchorId::Canvas, 1, 40.0));
        let vp1 = ViewportId::from_hash_of(1u64);
        let vp2 = ViewportId::from_hash_of(2u64);
        let in2 = marks.find(AnchorId::Canvas, Some(vp2)).unwrap().rect;
        assert_eq!((in2.min.x, in2.max.x), (20.0, 30.0), "only window 2's own mark");
        let in1 = marks.find(AnchorId::Canvas, Some(vp1)).unwrap().rect;
        assert_eq!((in1.min.x, in1.max.x), (0.0, 50.0), "window 1's two marks, united");
        assert_eq!(marks.find(AnchorId::Canvas, None).unwrap().viewport, vp1, "the last painted");
        let vp3 = ViewportId::from_hash_of(3u64);
        assert_eq!(
            marks.find(AnchorId::Canvas, Some(vp3)).unwrap().viewport,
            vp1,
            "no mark in the preferred window falls back to the last one painted"
        );
    }

    #[test]
    fn a_rect_outside_its_clip_is_not_on_screen() {
        let mut mk = m(AnchorId::TreeItems, 1, 0.0);
        assert!(mk.on_screen());
        mk.clip = Rect::from_min_size(egui::pos2(0.0, 100.0), egui::vec2(50.0, 50.0));
        assert!(!mk.on_screen(), "a row scrolled out of its pane is not on screen");
        mk.rect = Rect::NOTHING;
        assert!(!mk.on_screen());
    }
}
