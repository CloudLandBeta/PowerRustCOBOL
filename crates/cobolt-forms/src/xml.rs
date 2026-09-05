// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! XML serialization / deserialization for `.cfrm` form files.
//!
//! # Format (v1.0 — nested-program edition)
//! ```xml
//! <?xml version="1.0" encoding="UTF-8"?>
//! <Form name="MAIN-FORM" title="My App" width="800" height="600" ...>
//!
//!   <!-- Raw COBOL data declarations emitted verbatim into outer WS -->
//!   <working-storage><![CDATA[
//!        01 WS-COUNTER  PIC 9(8) VALUE 0 GLOBAL.
//!        01 WS-SHARED   PIC X    VALUE SPACES EXTERNAL.
//!   ]]></working-storage>
//!
//!   <!-- Form-level lifecycle events -->
//!   <form-events>
//!     <Event name="onLoad" paragraph="MAIN-FORM--ONLOAD"><![CDATA[
//!         MOVE 0 TO WS-COUNTER
//!     ]]></Event>
//!     <Event name="onClose" paragraph="MAIN-FORM--ONCLOSE"><![CDATA[
//!         CONTINUE.
//!     ]]></Event>
//!   </form-events>
//!
//!   <!-- Controls with per-event code -->
//!   <Control id="BTN-OK" type="Button" x="10" y="10" w="80" h="30" ...>
//!     <Property name="Caption">OK</Property>
//!     <Event name="onClick" paragraph="BTN-OK--CLICK"><![CDATA[
//!         MOVE WS-COUNTER TO WS-TXT-1-VALUE
//!     ]]></Event>
//!   </Control>
//!
//!   <!-- Recycle bin — never emitted into .cbl -->
//!   <deleted-controls>
//!     <DeletedControl id="BTN-OLD" deleted-at="2026-05-29T10:00:00">
//!       <Event name="onClick" paragraph="BTN-OLD--CLICK"><![CDATA[
//!           CONTINUE.
//!       ]]></Event>
//!     </DeletedControl>
//!   </deleted-controls>
//!
//! </Form>
//! ```
//!
//! Backward-compatible: old files with `<Event name="X" paragraph="Y"/>` (self-closing)
//! load fine — `code` will be empty.

use std::fs;
use std::io::BufReader;
use std::path::Path;

use quick_xml::{
    events::{BytesCData, BytesDecl, BytesEnd, BytesStart, BytesText, Event},
    Reader, Writer,
};
use thiserror::Error;

use crate::model::{
    derive_paragraph_name, AnimKind, AnimRepeat, AnimTrigger, AnimationDef, BgImageMode, Control,
    ControlType, DataBindingDef, DeletedControlCode, EasingKind, EventBinding, Form, PropValue,
    UserProcedure,
};

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum FormError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("XML error: {0}")]
    Xml(String),

    #[error("Missing required element: <{0}>")]
    MissingElement(String),

    #[error("Missing required attribute '{attr}' on <{element}>")]
    MissingAttr { element: String, attr: String },

    #[error("Invalid attribute value '{value}' for '{attr}'")]
    InvalidAttr { attr: String, value: String },
}

impl From<quick_xml::Error> for FormError {
    fn from(e: quick_xml::Error) -> Self {
        FormError::Xml(e.to_string())
    }
}

fn xml_err(e: impl std::fmt::Display) -> FormError {
    FormError::Xml(e.to_string())
}

// ── Attribute helpers ─────────────────────────────────────────────────────────

fn get_attr(e: &BytesStart, key: &[u8]) -> Result<Option<String>, FormError> {
    for attr in e.attributes() {
        let attr = attr.map_err(xml_err)?;
        if attr.key.as_ref() == key {
            let val = attr.unescape_value().map_err(xml_err)?.into_owned();
            return Ok(Some(val));
        }
    }
    Ok(None)
}

fn get_attr_str(e: &BytesStart, key: &[u8]) -> Result<String, FormError> {
    Ok(get_attr(e, key)?.unwrap_or_default())
}

/// Parse a `<MenuPaneBackground …/>` element's attributes (049 R39). Every
/// attribute is optional; an absent one keeps the struct default, so a
/// hand-trimmed element still loads.
fn parse_menu_pane_background(
    e: &BytesStart,
) -> Result<crate::model::MenuPaneBackground, FormError> {
    let mut mp = crate::model::MenuPaneBackground::default();
    if let Some(v) = get_attr(e, b"color")? {
        mp.color = v;
    }
    if let Some(v) = get_attr(e, b"gradient-enabled")? {
        mp.gradient_enabled = v == "true" || v == "1";
    }
    if let Some(v) = get_attr(e, b"gradient-start")? {
        mp.gradient_start_color = v;
    }
    if let Some(v) = get_attr(e, b"gradient-end")? {
        mp.gradient_end_color = v;
    }
    if let Some(v) = get_attr(e, b"gradient-direction")? {
        mp.gradient_direction = v;
    }
    if let Some(v) = get_attr(e, b"transparency")? {
        mp.transparency = v.parse::<u8>().unwrap_or(0).min(100);
    }
    if let Some(v) = get_attr(e, b"image")? {
        mp.image = v;
    }
    if let Some(v) = get_attr(e, b"image-mode")? {
        mp.image_mode = BgImageMode::from_str(&v);
    }
    Ok(mp)
}

#[allow(dead_code)]
fn get_attr_i32(e: &BytesStart, key: &[u8], default: i32) -> Result<i32, FormError> {
    Ok(get_attr(e, key)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(default))
}
fn get_attr_u32(e: &BytesStart, key: &[u8], default: u32) -> Result<u32, FormError> {
    Ok(get_attr(e, key)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(default))
}
#[allow(dead_code)]
fn get_attr_bool(e: &BytesStart, key: &[u8], default: bool) -> Result<bool, FormError> {
    Ok(get_attr(e, key)?
        .map(|v| v != "false" && v != "0")
        .unwrap_or(default))
}

// ── Owned event abstraction ───────────────────────────────────────────────────
//
// quick-xml events borrow from an internal buffer.  To allow recursive calls
// that re-use the same buffer we convert each event to a fully-owned value
// before acting on it.

type AttrPairs = Vec<(Vec<u8>, String)>; // (key-bytes, value-string)

enum OwnedEvent {
    FormStart {
        name: String,
        title: String,
        width: u32,
        height: u32,
        background: String,
        background_gradient_enabled: bool,
        background_gradient_start_color: String,
        background_gradient_end_color: String,
        background_gradient_direction: String,
        transparency: u8,
        background_image: String,
        bg_image_mode: BgImageMode,
        grid_size: u8,
        snap_to_grid: bool,
        target: String,
        theme: Option<String>,
        use_theme_background: bool,
        glass_style: crate::model::GlassStyle,
        // 037 Main form & window lifecycle
        main_form: bool,
        taskbar_icon: String,
        can_minimize: bool,
        can_maximize: bool,
        window_state: crate::model::WindowState,
        full_screen: bool,
        title_visible: bool,
        // 049 Application shell
        form_format: crate::model::FormFormat,
        // 038 Window effects opt-out
        window_effects: bool,
        // Window start position (operator, 2026-07-31)
        x: i32,
        y: i32,
        start_position: crate::model::FormStartPosition,
    },
    ControlStart(AttrPairs),
    // 049 R39 — the shell MenuPane's background, a self-closing attribute
    // element on the main form.
    MenuPaneBackground(crate::model::MenuPaneBackground),
    PropertyStart(String), // property name
    ChildrenStart,
    WorkingStorageStart,                 // <working-storage>
    SpecialNamesStart,                   // <special-names>   (spec 005)
    RepositoryStart,                     // <repository>      (spec 005)
    FileControlStart,                    // <file-control>    (spec 005)
    FileSectionStart,                    // <file-section>    (spec 005)
    UserProceduresStart,                 // <user-procedures> (spec 005)
    FormEventsStart,                     // <form-events>
    DataBindingsStart,                   // <DataBindings>
    DeletedControlsStart,                // <deleted-controls>
    DeletedControlStart(String, String), // (control_id, deleted_at) from <DeletedControl>
    /// <Event> as a *start* tag — body (CDATA or text) follows as Text/CData events.
    EventStart(String, String), // (event_name, paragraph)
    AnimationEmpty(AttrPairs),
    /// Generic start tag not matched by any specific variant above.
    StartTag(Vec<u8>), // tag local name bytes
    Text(String),
    CData(String),
    EndTag(Vec<u8>), // tag local name bytes
    Eof,
    Other,
}

/// Read the next quick-xml event and convert it to a fully owned `OwnedEvent`.
fn next_owned<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    buf: &mut Vec<u8>,
) -> Result<OwnedEvent, FormError> {
    buf.clear();
    let event = reader.read_event_into(buf)?;

    match &event {
        // ── Start tags ────────────────────────────────────────────────────────
        Event::Start(e) => {
            match e.local_name().as_ref() {
                b"Form" => {
                    let name = get_attr_str(e, b"name")?;
                    let title = get_attr_str(e, b"title")?;
                    let width = get_attr_u32(e, b"width", 800)?;
                    let height = get_attr_u32(e, b"height", 600)?;
                    let background =
                        get_attr(e, b"background")?.unwrap_or_else(|| "#FFFFFF".into());
                    let background_gradient_enabled = get_attr(e, b"background-gradient-enabled")?
                        .map(|value| value == "true" || value == "1")
                        .unwrap_or(false);
                    let background_gradient_start_color =
                        get_attr(e, b"background-gradient-start")?
                            .unwrap_or_else(|| "#F0F0F0FF".into());
                    let background_gradient_end_color = get_attr(e, b"background-gradient-end")?
                        .unwrap_or_else(|| "#C8D0DCFF".into());
                    let background_gradient_direction =
                        get_attr(e, b"background-gradient-direction")?
                            .unwrap_or_else(|| "South".into());
                    let transparency = get_attr(e, b"transparency")?
                        .and_then(|v| v.parse::<u8>().ok())
                        .unwrap_or(0);
                    let background_image = get_attr_str(e, b"background-image")?;
                    let bg_image_mode = BgImageMode::from_str(&get_attr_str(e, b"bg-image-mode")?);
                    let grid_size = get_attr(e, b"grid-size")?
                        .and_then(|v| v.parse::<u8>().ok())
                        .unwrap_or(8);
                    let snap_to_grid = get_attr(e, b"snap-to-grid")?
                        .map(|v| v != "false" && v != "0")
                        .unwrap_or(true);
                    let target = get_attr_str(e, b"target").unwrap_or_else(|_| "Custom".to_owned());
                    let theme = get_attr(e, b"theme")?
                        .map(|s| s.trim().to_owned())
                        .filter(|s| !s.is_empty());
                    let use_theme_background = get_attr(e, b"use-theme-background")?
                        .map(|v| v == "true" || v == "1")
                        .unwrap_or(false);
                    let glass_style = get_attr(e, b"glass-style")?
                        .map(|v| crate::model::GlassStyle::from_str(&v))
                        .unwrap_or_default();
                    // 037 window-lifecycle attributes — every one optional so
                    // pre-037 files load with the exact historical behaviour.
                    let main_form = get_attr(e, b"main-form")?
                        .map(|v| v == "true" || v == "1")
                        .unwrap_or(false);
                    let taskbar_icon = get_attr(e, b"taskbar-icon")?.unwrap_or_default();
                    let can_minimize = get_attr(e, b"can-minimize")?
                        .map(|v| v != "false" && v != "0")
                        .unwrap_or(true);
                    let can_maximize = get_attr(e, b"can-maximize")?
                        .map(|v| v != "false" && v != "0")
                        .unwrap_or(true);
                    let window_state = get_attr(e, b"window-state")?
                        .map(|v| crate::model::WindowState::from_str(&v))
                        .unwrap_or_default();
                    let full_screen = get_attr(e, b"full-screen")?
                        .map(|v| v == "true" || v == "1")
                        .unwrap_or(false);
                    let title_visible = get_attr(e, b"title-visible")?
                        .map(|v| v != "false" && v != "0")
                        .unwrap_or(true);
                    // 049 R1 — absent means Standalone, so every pre-049 form
                    // keeps opening in its own window (R3).
                    let form_format = get_attr(e, b"form-format")?
                        .map(|v| crate::model::FormFormat::from_str(&v))
                        .unwrap_or_default();
                    let window_effects = get_attr(e, b"window-effects")?
                        .map(|v| v != "false" && v != "0")
                        .unwrap_or(true);
                    // Window start position — optional so pre-existing files
                    // load with the exact historical behaviour (System: OS
                    // places the window, x/y unused).
                    let x = get_attr(e, b"x")?
                        .and_then(|v| v.parse::<i32>().ok())
                        .unwrap_or(0);
                    let y = get_attr(e, b"y")?
                        .and_then(|v| v.parse::<i32>().ok())
                        .unwrap_or(0);
                    let start_position = get_attr(e, b"start-position")?
                        .map(|v| crate::model::FormStartPosition::from_str(&v))
                        .unwrap_or_default();

                    Ok(OwnedEvent::FormStart {
                        name,
                        title,
                        width,
                        height,
                        background,
                        background_gradient_enabled,
                        background_gradient_start_color,
                        background_gradient_end_color,
                        background_gradient_direction,
                        transparency,
                        background_image,
                        bg_image_mode,
                        grid_size,
                        snap_to_grid,
                        target,
                        theme,
                        use_theme_background,
                        glass_style,
                        main_form,
                        taskbar_icon,
                        can_minimize,
                        can_maximize,
                        window_state,
                        full_screen,
                        title_visible,
                        form_format,
                        window_effects,
                        x,
                        y,
                        start_position,
                    })
                }
                b"Control" => {
                    let mut pairs = AttrPairs::new();
                    for attr in e.attributes() {
                        let attr = attr.map_err(xml_err)?;
                        let key = attr.key.as_ref().to_vec();
                        let val = attr.unescape_value().map_err(xml_err)?.into_owned();
                        pairs.push((key, val));
                    }
                    Ok(OwnedEvent::ControlStart(pairs))
                }
                b"Property" => {
                    let name = get_attr_str(e, b"name")?;
                    Ok(OwnedEvent::PropertyStart(name))
                }
                b"MenuPaneBackground" => {
                    Ok(OwnedEvent::MenuPaneBackground(parse_menu_pane_background(e)?))
                }
                b"Children" => Ok(OwnedEvent::ChildrenStart),
                b"working-storage" => Ok(OwnedEvent::WorkingStorageStart),
                b"special-names" => Ok(OwnedEvent::SpecialNamesStart),
                b"repository" => Ok(OwnedEvent::RepositoryStart),
                b"file-control" => Ok(OwnedEvent::FileControlStart),
                b"file-section" => Ok(OwnedEvent::FileSectionStart),
                b"user-procedures" => Ok(OwnedEvent::UserProceduresStart),
                b"form-events" => Ok(OwnedEvent::FormEventsStart),
                b"DataBindings" => Ok(OwnedEvent::DataBindingsStart),
                b"deleted-controls" => Ok(OwnedEvent::DeletedControlsStart),
                b"DeletedControl" => {
                    let id = get_attr_str(e, b"id")?;
                    let deleted_at = get_attr_str(e, b"deleted-at")?;
                    Ok(OwnedEvent::DeletedControlStart(id, deleted_at))
                }
                // <Event> as a start tag (v1.0 — has CDATA body)
                b"Event" => {
                    let ev_name = get_attr_str(e, b"name")?;
                    let paragraph = get_attr_str(e, b"paragraph")?;
                    Ok(OwnedEvent::EventStart(ev_name, paragraph))
                }
                // Generic start tag — returned as StartTag so specialised parsers
                // (e.g. collect_event_body) can match on it.
                other => Ok(OwnedEvent::StartTag(other.to_vec())),
            }
        }

        // ── Empty / self-closing tags ─────────────────────────────────────────
        Event::Empty(e) => match e.local_name().as_ref() {
            b"MenuPaneBackground" => {
                Ok(OwnedEvent::MenuPaneBackground(parse_menu_pane_background(&e)?))
            }
            b"Animation" => {
                let mut pairs = AttrPairs::new();
                for attr in e.attributes() {
                    let attr = attr.map_err(xml_err)?;
                    let key = attr.key.as_ref().to_vec();
                    let val = attr.unescape_value().map_err(xml_err)?.into_owned();
                    pairs.push((key, val));
                }
                Ok(OwnedEvent::AnimationEmpty(pairs))
            }
            _ => Ok(OwnedEvent::Other),
        },

        // ── Content ───────────────────────────────────────────────────────────
        Event::Text(t) => {
            let text = t.unescape().map_err(xml_err)?.into_owned();
            Ok(OwnedEvent::Text(text))
        }
        Event::CData(c) => {
            let text = std::str::from_utf8(c.as_ref())
                .map_err(|e| xml_err(e))?
                .to_owned();
            Ok(OwnedEvent::CData(text))
        }
        Event::End(e) => {
            let local = e.local_name().as_ref().to_vec();
            Ok(OwnedEvent::EndTag(local))
        }
        Event::Eof => Ok(OwnedEvent::Eof),
        _ => Ok(OwnedEvent::Other),
    }
}

// ── Load ──────────────────────────────────────────────────────────────────────

pub fn load_form(path: &Path) -> Result<Form, FormError> {
    let file = std::fs::File::open(path)?;
    let mut reader = Reader::from_reader(BufReader::new(file));
    reader.config_mut().trim_text(false); // keep CDATA whitespace intact
    read_form(&mut reader)
}

/// Parse a form directly from an in-memory XML string.
///
/// Used by the embed+bundle compiler, where `.cfrm` files are stored as bytes
/// inside the binary rather than read from the filesystem.
pub fn load_form_from_str(xml: &str) -> Result<Form, FormError> {
    let mut reader = Reader::from_reader(BufReader::new(xml.as_bytes()));
    reader.config_mut().trim_text(false);
    read_form(&mut reader)
}

/// Shared form-reading loop used by both `load_form` and `load_form_from_str`.
fn read_form<R: std::io::BufRead>(reader: &mut Reader<R>) -> Result<Form, FormError> {
    let mut buf = Vec::new();
    let mut form: Option<Form> = None;

    loop {
        match next_owned(reader, &mut buf)? {
            OwnedEvent::FormStart {
                name,
                title,
                width,
                height,
                background,
                background_gradient_enabled,
                background_gradient_start_color,
                background_gradient_end_color,
                background_gradient_direction,
                transparency,
                background_image,
                bg_image_mode,
                grid_size,
                snap_to_grid,
                target,
                theme,
                use_theme_background,
                glass_style,
                main_form,
                taskbar_icon,
                can_minimize,
                can_maximize,
                window_state,
                full_screen,
                title_visible,
                form_format,
                window_effects,
                x,
                y,
                start_position,
            } => {
                // Build a base Form using Form::new (populates default form_events)
                let mut f = Form::new(&name, &title, width, height);
                f.background_color = background;
                f.background_gradient_enabled = background_gradient_enabled;
                f.background_gradient_start_color = background_gradient_start_color;
                f.background_gradient_end_color = background_gradient_end_color;
                f.background_gradient_direction = background_gradient_direction;
                f.transparency = transparency;
                f.background_image = background_image;
                f.bg_image_mode = bg_image_mode;
                f.grid_size = grid_size;
                f.snap_to_grid = snap_to_grid;
                f.target = target;
                f.theme = theme;
                f.use_theme_background = use_theme_background;
                f.glass_style = glass_style;
                f.main_form = main_form;
                f.taskbar_icon = taskbar_icon;
                f.can_minimize = can_minimize;
                f.can_maximize = can_maximize;
                f.window_state = window_state;
                f.full_screen = full_screen;
                f.title_visible = title_visible;
                f.form_format = form_format;
                f.window_effects = window_effects;
                f.x = x;
                f.y = y;
                f.start_position = start_position;
                // form_events was pre-populated with empty OnLoad/OnClose stubs;
                // parse_form_body will overwrite them if <form-events> is present.
                parse_form_body(reader, &mut buf, &mut f)?;
                form = Some(f);
            }
            OwnedEvent::Eof => break,
            _ => {}
        }
    }

    let mut form = form.ok_or_else(|| FormError::MissingElement("Form".into()))?;
    // Empty REPOSITORY ⇒ seed the curated Rust-FFI type bridge; any
    // developer-authored content is preserved as-is (spec 005).
    relativize_asset_paths(&mut form);
    form.seed_repository_if_empty();
    // Containers (spec 012): flatten any legacy <Children> nesting into the flat
    // editing list with `parent` links, and migrate the old Panel `Scrollable`
    // flag to the unified `HScroll`/`VScroll` properties.
    normalize_containers(&mut form);
    seed_missing_props(&mut form);
    // 049 — a FullHeight SideMenu spans the form's whole height. Doing it here
    // means every consumer (designer, preview, run, shell, codegen) reads one
    // truthful rect, instead of each render path re-deriving it.
    form.sync_side_menu_full_height();
    // …and every Splitter's two pane Panels, for the same reason: they are
    // derived geometry, so deriving them once on load means the designer, the
    // preview, the running form, the shell and codegen all read the same two
    // rects. A form saved before the Splitter owned panes gains them here.
    form.sync_splitter_panes();
    Ok(form)
}

/// Seed properties that were added after a control was first created, so that
/// existing .cfrm files gain the new UI without a manual re-create.
/// Rewrite absolute asset paths that point INSIDE the project into the
/// project-relative form, on load.
///
/// Forms saved before this existed carry the full path the designer's picker
/// handed them — `/Users/<someone>/Documents/<project>/assets/logo.png` — which
/// breaks on every other machine, and on the author's own as soon as the
/// project moves (operator, 2026-09-04: the shipped demo project's images all
/// pointed at a directory that no longer existed).
///
/// Healing on LOAD rather than in a migration tool means a form is corrected
/// the first time it is opened and the correction is written out by the next
/// ordinary save. A path outside the project is left absolute: the developer
/// pointed there deliberately.
fn relativize_asset_paths(form: &mut crate::model::Form) {
    let Some(root) = crate::assets::current_base() else {
        return;
    };
    let rewrite = |value: &str| -> Option<String> {
        let p = std::path::Path::new(value.trim());
        if !p.is_absolute() {
            return None;
        }
        let rel = crate::assets::store(&root, p);
        // `store` returns the input unchanged when it is not under the root.
        (rel != value.trim()).then_some(rel)
    };

    if let Some(rel) = rewrite(&form.background_image) {
        form.background_image = rel;
    }
    for ctrl in &mut form.controls {
        // Any property whose value is a path: matched by NAME, because the
        // catalogue spells them ImagePath, IconPath, HeaderImage,
        // GridBackgroundImage, LeafIcon, ParentIcon and so on, and a new one
        // must not have to be added here to be healed.
        let keys: Vec<String> = ctrl
            .properties
            .keys()
            .filter(|k| {
                let k = k.to_ascii_lowercase();
                k.ends_with("path") || k.ends_with("image") || k.ends_with("icon")
            })
            .cloned()
            .collect();
        for key in keys {
            let Some(crate::model::PropValue::String(v)) = ctrl.properties.get(&key) else {
                continue;
            };
            if let Some(rel) = rewrite(v) {
                ctrl.properties
                    .insert(key, crate::model::PropValue::String(rel));
            }
        }
    }
}

fn seed_missing_props(form: &mut Form) {
    use crate::model::{ControlType, PropValue};
    for c in &mut form.controls {
        let universal_defaults = [
            ("BackgroundGradientEnabled", PropValue::Bool(false)),
            (
                "BackgroundGradientStartColor",
                PropValue::String(crate::model::DEFAULT_BACKGROUND_COLOR.into()),
            ),
            (
                "BackgroundGradientEndColor",
                PropValue::String("#C8D0DC".into()),
            ),
            (
                "BackgroundGradientDirection",
                PropValue::String("South".into()),
            ),
            ("ShadowLightColor", PropValue::String("#FFFFFFFF".into())),
        ];
        for (key, value) in universal_defaults {
            if c.get_prop(key).is_none() {
                c.set_prop(key, value);
            }
        }
        match c.control_type {
            // Border keys arrived after these controls shipped. Without the
            // backfill an existing .cfrm keeps no border property at all, and
            // `border_rows` — which shows a row only when the key is present —
            // would leave the pane rows hidden forever while `draw_control`
            // went on painting its "Single"/1px fallback box.
            // A Switch takes the same backfill for the same reason, with its
            // own colour default: before 1.63.41 a saved switch reached the
            // pane with only the `BorderStyle` the generic seed gave it, so
            // the style row could appear with no colour or width beside it —
            // and on a build between 1.63.36 and 1.63.39 that seed was
            // `Single`, which drew a rim the developer could neither see in
            // the pane nor turn off (operator screenshots, 2026-09-03).
            ControlType::Switch => {
                let defaults: &[(&str, PropValue)] = &[
                    ("BorderStyle", PropValue::String("None".into())),
                    ("BorderColor", PropValue::String("#888888".into())),
                    ("BorderWidth", PropValue::Int(1)),
                ];
                for (key, value) in defaults {
                    if c.get_prop(*key).is_none() {
                        c.set_prop(*key, value.clone());
                    }
                }
            }
            ControlType::CheckBox | ControlType::RadioButton => {
                let defaults: &[(&str, PropValue)] = &[
                    ("BorderStyle", PropValue::String("None".into())),
                    ("BorderColor", PropValue::String("#8C8CA0".into())),
                    ("BorderWidth", PropValue::Int(1)),
                ];
                for (key, value) in defaults {
                    if c.get_prop(*key).is_none() {
                        c.set_prop(*key, value.clone());
                    }
                }
            }
            ControlType::GroupBox => {
                if c.get_prop("CaptionEnabled").is_none() {
                    c.set_prop("CaptionEnabled", PropValue::Bool(true));
                }
                if c.get_prop("BorderWidth").is_none() {
                    c.set_prop("BorderWidth", PropValue::Int(1));
                }
            }
            ControlType::Panel => {
                if c.get_prop("BorderWidth").is_none() {
                    c.set_prop("BorderWidth", PropValue::Int(1));
                }
                if c.get_prop("HideBackground").is_none() {
                    c.set_prop("HideBackground", PropValue::Bool(false));
                }
                if c.get_prop("BackgroundGradientEnabled").is_none() {
                    c.set_prop("BackgroundGradientEnabled", PropValue::Bool(false));
                }
            }
            ControlType::DataGrid => {
                let defaults: &[(&str, PropValue)] = &[
                    ("AlternatingRowOpacity", PropValue::Int(20)),
                    ("AllowColumnReorder", PropValue::Bool(true)),
                    ("AllowRowResize", PropValue::Bool(true)),
                    (
                        crate::model::DATAGRID_ADVANCED_PROP,
                        PropValue::String(String::new()),
                    ),
                    ("ShowColumnFilters", PropValue::Bool(false)),
                    ("ShowCSVExportButton", PropValue::Bool(true)),
                    ("CSVExportMode", PropValue::String("Filtered".into())),
                    ("FrozenColumns", PropValue::Int(0)),
                    ("FrozenRows", PropValue::Int(0)),
                    ("GridLineStyle", PropValue::String("Solid".into())),
                    ("RowHeightOverrides", PropValue::String(String::new())),
                    ("ColumnFilters", PropValue::String(String::new())),
                    ("SelectableText", PropValue::Bool(true)),
                ];
                for (key, value) in defaults {
                    if c.get_prop(key).is_none() {
                        c.set_prop(*key, value.clone());
                    }
                }
            }
            ControlType::Splitter => {
                // A Splitter saved before it owned panes is a BAR: its
                // `SplitPosition` is a pixel offset, not a percentage, and its
                // `Orientation` named the bar's own direction rather than how
                // the panes sit. `GripStyle` is the tell — it exists only on
                // the panel-with-two-panes control — so its absence is what
                // marks a form as needing the one-time repair.
                let legacy = c.get_prop("GripStyle").is_none();
                let defaults: &[(&str, PropValue)] = &[
                    ("BorderStyle", PropValue::String("Single".into())),
                    ("BorderColor", PropValue::String("#CCCCCC".into())),
                    ("BorderWidth", PropValue::Int(1)),
                    ("LineColor", PropValue::String(String::new())),
                    (
                        "LineSize",
                        PropValue::Int(crate::splitter::DEFAULT_LINE_SIZE as i64),
                    ),
                    ("GripStyle", PropValue::String("FilledPill".into())),
                    (
                        "GripSize",
                        PropValue::Int(crate::splitter::DEFAULT_GRIP_SIZE as i64),
                    ),
                    ("GripColor", PropValue::String(String::new())),
                ];
                for (key, value) in defaults {
                    if c.get_prop(key).is_none() {
                        c.set_prop(*key, value.clone());
                    }
                }
                if legacy {
                    // A pixel offset read as a percentage would open the form
                    // with one pane shut; centre it instead and let the
                    // developer place the division again.
                    c.set_prop(
                        "SplitPosition",
                        PropValue::Int(crate::splitter::DEFAULT_SPLIT_PERCENT as i64),
                    );
                    // The old default was a 200×8 rule. Nothing fits in an 8pt
                    // pane, so an axis too thin to hold a control at all is
                    // opened out to the size a Splitter is dropped at now. A
                    // splitter the developer had already sized is left alone.
                    const USABLE: i32 = 40;
                    if c.rect.w < USABLE {
                        c.rect.w = 320;
                    }
                    if c.rect.h < USABLE {
                        c.rect.h = 220;
                    }
                }
            }
            ControlType::MenuBar => {
                let defaults: &[(&str, &str)] = &[
                    ("BackgroundColor", "#00000000"),
                    ("ForegroundColor", "#E1E6FA"),
                    ("HighlightBgColor", "#4488FF"),
                    ("HighlightFgColor", "#FFFFFF"),
                    ("SelectedBgColor", "#3366CC"),
                    ("SelectedFgColor", "#FFFFFF"),
                ];
                for &(key, val) in defaults {
                    if c.get_prop(key).is_none() {
                        c.set_prop(key, PropValue::String(val.into()));
                    }
                }
            }
            _ => {}
        }

        // `CornerRadius`/`BorderStyle`: the SAME function `Control::new`
        // calls for a brand new control, so a control saved before either
        // property existed backfills identically to one just dropped fresh —
        // a Label that predates the seed is not a special case, it is the
        // ordinary one (operator, 2026-09-03: an existing Label's
        // CornerRadius row was simply never there, because `Control::new`'s
        // own fix never runs for a control loaded from disk). LAST, so a
        // type-specific choice above it — a CheckBox's own `BorderStyle`
        // "None" — is already in place and this only fills what is still
        // absent, exactly the order `Control::new` itself uses.
        crate::model::seed_theme_owned_appearance(&c.control_type, &mut c.properties);
    }
}

/// Spec 012 post-load normalization. New `.cfrm` write controls flat with a
/// `parent` attribute; older files nested children under `<Children>`. Either way
/// we end with a single flat `form.controls` carrying `parent` links.
fn normalize_containers(form: &mut Form) {
    let roots = std::mem::take(&mut form.controls);
    let mut flat: Vec<Control> = Vec::new();
    for c in roots {
        flatten_ctrl(c, None, &mut flat);
    }
    form.controls = flat;
    for c in &mut form.controls {
        if c.is_container() && c.get_prop("HScroll").is_none() && c.get_prop("VScroll").is_none() {
            if let Some(scroll) = c.get_prop("Scrollable").map(|v| v.as_bool()) {
                c.set_prop("HScroll", PropValue::Bool(scroll));
                c.set_prop("VScroll", PropValue::Bool(scroll));
            }
        }
    }
}

fn flatten_ctrl(mut c: Control, parent: Option<String>, out: &mut Vec<Control>) {
    let kids = std::mem::take(&mut c.children);
    let id = c.id.clone();
    // A control loaded from a new flat file already carries its `parent`
    // attribute; only derive it from tree position for legacy nested files.
    if c.parent.is_none() {
        c.parent = parent;
    }
    out.push(c);
    for k in kids {
        flatten_ctrl(k, Some(id.clone()), out);
    }
}

/// Parse everything inside `<Form> … </Form>`.
fn parse_form_body<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    buf: &mut Vec<u8>,
    form: &mut Form,
) -> Result<(), FormError> {
    loop {
        match next_owned(reader, buf)? {
            // ── Controls ──────────────────────────────────────────────────────
            OwnedEvent::ControlStart(attrs) => {
                form.controls.push(parse_control(reader, buf, attrs)?);
            }

            // ── <MenuPaneBackground/> (049 R39) ───────────────────────────────
            OwnedEvent::MenuPaneBackground(mp) => {
                form.menu_pane_background = Some(mp);
            }

            // ── <working-storage> ─────────────────────────────────────────────
            OwnedEvent::WorkingStorageStart => {
                form.user_ws_source = collect_cdata_body(reader, buf, b"working-storage")?;
            }

            // ── COBOL structure blocks (spec 005) ─────────────────────────────
            OwnedEvent::SpecialNamesStart => {
                form.cobol_structure.special_names =
                    collect_cdata_body(reader, buf, b"special-names")?;
            }
            OwnedEvent::RepositoryStart => {
                form.cobol_structure.repository = collect_cdata_body(reader, buf, b"repository")?;
            }
            OwnedEvent::FileControlStart => {
                form.cobol_structure.file_control =
                    collect_cdata_body(reader, buf, b"file-control")?;
            }
            OwnedEvent::FileSectionStart => {
                form.cobol_structure.file_section =
                    collect_cdata_body(reader, buf, b"file-section")?;
            }

            // ── <user-procedures> (spec 005) — reuse the Event-list machinery ──
            OwnedEvent::UserProceduresStart => {
                form.user_procedures = parse_event_list(reader, buf, b"user-procedures")?
                    .into_iter()
                    .map(|e| UserProcedure {
                        name: e.event,
                        code: e.code,
                    })
                    .collect();
            }

            // ── <form-events> ─────────────────────────────────────────────────
            OwnedEvent::FormEventsStart => {
                form.form_events = parse_event_list(reader, buf, b"form-events")?;
                // Ensure onLoad / onClose stubs exist even if file omits them.
                for ev_name in &["onLoad", "onClose"] {
                    if !form.form_events.iter().any(|e| e.event == *ev_name) {
                        form.form_events.push(EventBinding {
                            event: ev_name.to_string(),
                            paragraph: derive_paragraph_name(&form.name, ev_name),
                            code: String::new(),
                        });
                    }
                }
            }

            // ── <DataBindings> ───────────────────────────────────────────────
            OwnedEvent::DataBindingsStart => {
                form.data_bindings = parse_data_bindings(reader, buf)?;
            }

            // ── <deleted-controls> ────────────────────────────────────────────
            OwnedEvent::DeletedControlsStart => {
                form.deleted_code = parse_deleted_controls(reader, buf)?;
            }

            // ── </Form> ───────────────────────────────────────────────────────
            OwnedEvent::EndTag(tag) if tag.as_slice() == b"Form" => break,
            OwnedEvent::Eof => break,
            _ => {}
        }
    }
    Ok(())
}

/// Collect all text/CDATA content between the current position and `</end_tag>`.
fn collect_cdata_body<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    buf: &mut Vec<u8>,
    end_tag: &[u8],
) -> Result<String, FormError> {
    let mut body = String::new();
    loop {
        match next_owned(reader, buf)? {
            OwnedEvent::Text(t) => body.push_str(&t),
            OwnedEvent::CData(c) => body.push_str(&c),
            OwnedEvent::EndTag(tag) if tag.as_slice() == end_tag => break,
            OwnedEvent::Eof => break,
            _ => {}
        }
    }
    Ok(body)
}

/// Parse a list of `<Event>` children up to `</end_tag>`.
///
/// Each `<Event>` may contain:
/// - An optional `<LocalWS><![CDATA[...]]></LocalWS>` child
/// - A top-level CDATA body (the procedure body)
///
/// Both old (bare CDATA) and new (LocalWS + CDATA) formats are accepted.
fn parse_event_list<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    buf: &mut Vec<u8>,
    end_tag: &[u8],
) -> Result<Vec<EventBinding>, FormError> {
    let mut events = Vec::new();
    loop {
        match next_owned(reader, buf)? {
            OwnedEvent::EventStart(ev_name, paragraph) => {
                let (code, local_ws) = collect_event_body(reader, buf)?;
                if !ev_name.is_empty() {
                    let code = migrate_handler_source(code, local_ws);
                    events.push(EventBinding {
                        event: ev_name,
                        paragraph,
                        code,
                    });
                }
            }
            OwnedEvent::EndTag(tag) if tag.as_slice() == end_tag => break,
            OwnedEvent::Eof => break,
            _ => {}
        }
    }
    Ok(events)
}

fn parse_data_bindings<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    buf: &mut Vec<u8>,
) -> Result<Vec<DataBindingDef>, FormError> {
    let body = collect_cdata_body(reader, buf, b"DataBindings")?;
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(trimmed).map_err(xml_err)
}

/// Read everything inside an `<Event>...</Event>` block and return
/// `(procedure_body_code, local_ws)`.
///
/// Handles both the legacy bare-CDATA format and the new format that includes
/// a `<LocalWS>` child element.
fn collect_event_body<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    buf: &mut Vec<u8>,
) -> Result<(String, String), FormError> {
    let mut code = String::new();
    let mut local_ws = String::new();
    loop {
        match next_owned(reader, buf)? {
            // <LocalWS> child — read its CDATA content
            OwnedEvent::StartTag(tag) if tag.as_slice() == b"LocalWS" => {
                local_ws = collect_cdata_body(reader, buf, b"LocalWS")?;
            }
            // Top-level text / CDATA = procedure body
            OwnedEvent::Text(t) => code.push_str(&t),
            OwnedEvent::CData(c) => code.push_str(&c),
            OwnedEvent::EndTag(tag) if tag.as_slice() == b"Event" => break,
            OwnedEvent::Eof => break,
            _ => {}
        }
    }
    Ok((code, local_ws))
}

/// Bring a loaded handler up to the current single-source format.
///
/// * New files store the **full** handler body (it already contains its own
///   `ENVIRONMENT`/`DATA`/`PROCEDURE DIVISION`) and no `<LocalWS>` — passed
///   through unchanged.
/// * Legacy files store bare PROCEDURE statements in `code` plus optional
///   `local_ws`; these are wrapped into a complete handler body so old forms
///   keep working.
/// * An empty handler (no code, no local WS) stays empty.
fn migrate_handler_source(code: String, local_ws: String) -> String {
    if code.trim().is_empty() && local_ws.trim().is_empty() {
        return String::new();
    }
    let already_full = code.to_ascii_uppercase().contains("PROCEDURE DIVISION");
    if already_full && local_ws.trim().is_empty() {
        return code;
    }
    // Legacy → wrap statements (and any local WS) into a full handler body.
    let mut t = String::new();
    t.push_str("       ENVIRONMENT DIVISION.\n");
    t.push_str("       DATA DIVISION.\n");
    t.push_str("       WORKING-STORAGE SECTION.\n");
    for line in local_ws.lines() {
        if !line.trim().is_empty() {
            t.push_str(line);
            t.push('\n');
        }
    }
    t.push_str("       LINKAGE SECTION.\n\n");
    t.push_str("       PROCEDURE DIVISION.\n");
    let body = code.trim_end();
    if body.trim().is_empty() {
        t.push_str("           CONTINUE.\n");
    } else {
        for line in body.lines() {
            t.push_str(line);
            t.push('\n');
        }
    }
    t
}

/// Parse `<deleted-controls>` children.
fn parse_deleted_controls<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    buf: &mut Vec<u8>,
) -> Result<Vec<DeletedControlCode>, FormError> {
    let mut deleted = Vec::new();
    loop {
        match next_owned(reader, buf)? {
            OwnedEvent::DeletedControlStart(control_id, deleted_at) => {
                let events = parse_event_list(reader, buf, b"DeletedControl")?;
                deleted.push(DeletedControlCode {
                    control_id,
                    deleted_at,
                    events,
                });
            }
            OwnedEvent::EndTag(tag) if tag.as_slice() == b"deleted-controls" => break,
            OwnedEvent::Eof => break,
            _ => {}
        }
    }
    Ok(deleted)
}

fn parse_control_list<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    buf: &mut Vec<u8>,
    end_tag: &[u8],
) -> Result<Vec<Control>, FormError> {
    let mut controls = Vec::new();
    loop {
        match next_owned(reader, buf)? {
            OwnedEvent::ControlStart(attrs) => {
                controls.push(parse_control(reader, buf, attrs)?);
            }
            OwnedEvent::EndTag(tag) if tag.as_slice() == end_tag => break,
            OwnedEvent::Eof => break,
            _ => {}
        }
    }
    Ok(controls)
}

/// Build a Control from an attribute pair list (already converted to owned Strings).
fn parse_control<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    buf: &mut Vec<u8>,
    attrs: AttrPairs,
) -> Result<Control, FormError> {
    // ── Decode attributes ────────────────────────────────────────────────────
    let mut id = String::new();
    let mut type_str = String::new();
    let mut x = 0i32;
    let mut y = 0i32;
    let mut w: Option<i32> = None;
    let mut h: Option<i32> = None;
    let mut tab_order = 0u32;
    let mut z_order = 0i32;
    let mut visible = true;
    let mut enabled = true;
    let mut parent: Option<String> = None;
    let mut tab: Option<u32> = None;

    for (key, val) in attrs {
        match key.as_slice() {
            b"id" => id = val,
            b"type" => type_str = val,
            b"x" => x = val.parse().unwrap_or(0),
            b"y" => y = val.parse().unwrap_or(0),
            b"w" => w = val.parse().ok(),
            b"h" => h = val.parse().ok(),
            b"tab-order" => tab_order = val.parse().unwrap_or(0),
            b"z-order" => z_order = val.parse().unwrap_or(0),
            b"visible" => visible = val != "false" && val != "0",
            b"enabled" => enabled = val != "false" && val != "0",
            // Container membership (spec 012). Empty `parent` = direct form child.
            b"parent" => parent = if val.is_empty() { None } else { Some(val) },
            b"tab" => tab = val.parse().ok(),
            _ => {}
        }
    }

    let control_type = ControlType::from_str(&type_str);
    let mut ctrl = Control::new(id, control_type, x, y);
    if let Some(wv) = w {
        ctrl.rect.w = wv;
    }
    if let Some(hv) = h {
        ctrl.rect.h = hv;
    }
    ctrl.tab_order = tab_order;
    ctrl.z_order = z_order;
    ctrl.visible = visible;
    ctrl.enabled = enabled;
    ctrl.parent = parent;
    ctrl.tab = tab;
    // Clear default properties/events set by Control::new — the file is authoritative.
    ctrl.properties.clear();
    ctrl.events.clear();

    // ── Parse child elements ─────────────────────────────────────────────────
    let mut current_prop: Option<String> = None;

    loop {
        match next_owned(reader, buf)? {
            OwnedEvent::PropertyStart(name) => {
                current_prop = Some(name);
            }
            OwnedEvent::Text(text) => {
                if let Some(ref pname) = current_prop {
                    let (pname, value) = migrate_opacity_property(pname, parse_prop_value(&text));
                    ctrl.properties.insert(pname, value);
                }
            }
            OwnedEvent::CData(text) => {
                // CDATA inside a <Property> (unlikely but handle gracefully)
                if let Some(ref pname) = current_prop {
                    let (pname, value) = migrate_opacity_property(pname, parse_prop_value(&text));
                    ctrl.properties.insert(pname, value);
                }
            }
            OwnedEvent::ChildrenStart => {
                ctrl.children = parse_control_list(reader, buf, b"Children")?;
            }
            // v1.0 — <Event ...> with optional <LocalWS> child + CDATA body
            OwnedEvent::EventStart(ev_name, paragraph) => {
                let (code, local_ws) = collect_event_body(reader, buf)?;
                if !ev_name.is_empty() {
                    let code = migrate_handler_source(code, local_ws);
                    ctrl.events.push(EventBinding {
                        event: ev_name,
                        paragraph,
                        code,
                    });
                }
            }
            OwnedEvent::AnimationEmpty(attrs) => {
                let mut name = String::new();
                let mut trigger = "OnLoad".to_owned();
                let mut kind = "FadeIn".to_owned();
                let mut duration = 400u64;
                let mut delay = 0u64;
                let mut easing = "EaseInOut".to_owned();
                let mut repeat = "Once".to_owned();
                let mut slide_dx = 0i32;
                let mut slide_dy = 0i32;
                for (key, val) in attrs {
                    match key.as_slice() {
                        b"name" => name = val,
                        b"trigger" => trigger = val,
                        b"kind" => kind = val,
                        b"duration" => duration = val.parse().unwrap_or(400),
                        b"delay" => delay = val.parse().unwrap_or(0),
                        b"easing" => easing = val,
                        b"repeat" => repeat = val,
                        b"slide-dx" => slide_dx = val.parse().unwrap_or(0),
                        b"slide-dy" => slide_dy = val.parse().unwrap_or(0),
                        _ => {}
                    }
                }
                if !name.is_empty() {
                    let mut anim = AnimationDef::new(&name);
                    anim.trigger = AnimTrigger::from_str(&trigger);
                    anim.kind = AnimKind::from_str(&kind);
                    anim.duration_ms = duration;
                    anim.delay_ms = delay;
                    anim.easing = EasingKind::from_str(&easing);
                    anim.repeat = match repeat.as_str() {
                        "Loop" => AnimRepeat::Loop,
                        "PingPong" => AnimRepeat::PingPong,
                        "Count" => AnimRepeat::Count(3),
                        _ => AnimRepeat::Once,
                    };
                    anim.slide_dx = slide_dx;
                    anim.slide_dy = slide_dy;
                    ctrl.animations.push(anim);
                }
            }
            OwnedEvent::EndTag(tag) => match tag.as_slice() {
                b"Property" => {
                    current_prop = None;
                }
                b"Control" => break,
                _ => {}
            },
            OwnedEvent::Eof => break,
            _ => {}
        }
    }

    migrate_radio_checked_to_selected(&mut ctrl);
    Ok(ctrl)
}

/// A RadioButton written before 2026-08-31 stores its state under `Checked`.
/// Rename it to `Selected` on the way in, so exactly one spelling reaches
/// everything downstream and the file upgrades itself the next time it is
/// saved — the same silent-upgrade discipline every other `.cfrm` field
/// change has followed.
///
/// A file that already carries `Selected` wins outright: if both are present
/// (hand-edited, or half-migrated), the canonical one is kept and the legacy
/// key dropped rather than merged, because two names for one state is the
/// condition this rename exists to end.
fn migrate_radio_checked_to_selected(ctrl: &mut Control) {
    if ctrl.control_type != ControlType::RadioButton {
        return;
    }
    let legacy = ctrl
        .properties
        .keys()
        .find(|k| k.eq_ignore_ascii_case(crate::model::CHECKED_PROP))
        .cloned();
    let Some(legacy) = legacy else { return };
    let has_canonical = ctrl
        .properties
        .keys()
        .any(|k| k.eq_ignore_ascii_case(crate::model::SELECTED_PROP));
    let value = ctrl.properties.shift_remove(&legacy);
    if has_canonical {
        return;
    }
    if let Some(value) = value {
        ctrl.set_prop(crate::model::SELECTED_PROP, value);
    }
}

/// Translate the legacy `Opacity` property into `Transparency` as it is read.
///
/// The two run opposite ways — `Opacity = 100` and `Transparency = 0` both mean
/// "opaque" — so the value is complemented, not just renamed. This has to happen
/// HERE, while the file's own value is in hand: `Control::new` has already
/// seeded a `Transparency` default, so a migration that ran afterwards could not
/// tell a seeded default from a value the developer chose, and a control saved
/// at `Opacity = 40` would come back fully opaque.
///
/// Every other property passes through untouched.
fn migrate_opacity_property(name: &str, value: PropValue) -> (String, PropValue) {
    if !name.eq_ignore_ascii_case("Opacity") {
        return (name.to_owned(), value);
    }
    let opacity = value.as_i64().clamp(0, 100);
    ("Transparency".to_owned(), PropValue::Int(100 - opacity))
}

fn parse_prop_value(s: &str) -> PropValue {
    let trimmed = s.trim();
    if trimmed == "true" {
        return PropValue::Bool(true);
    }
    if trimmed == "false" {
        return PropValue::Bool(false);
    }
    if let Ok(n) = trimmed.parse::<i64>() {
        return PropValue::Int(n);
    }
    PropValue::String(trimmed.to_owned())
}

// ── Save ──────────────────────────────────────────────────────────────────────

/// Spec 046 R2/R11 — the file-free half of the round trip
/// `load_form_from_str` already had: the exact `.cfrm` XML text `save_form`
/// would write, without touching disk. This is what Copy Form puts on the
/// OS clipboard.
pub fn form_to_string(form: &Form) -> Result<String, FormError> {
    let mut output = Vec::new();
    {
        let mut w = Writer::new_with_indent(&mut output, b' ', 2);

        w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

        // ── <Form ...> ────────────────────────────────────────────────────────
        let mut elem = BytesStart::new("Form");
        elem.push_attribute(("name", form.name.as_str()));
        elem.push_attribute(("title", form.title.as_str()));
        elem.push_attribute(("width", form.width.to_string().as_str()));
        elem.push_attribute(("height", form.height.to_string().as_str()));
        elem.push_attribute(("background", form.background_color.as_str()));
        if form.background_gradient_enabled {
            elem.push_attribute(("background-gradient-enabled", "true"));
            elem.push_attribute((
                "background-gradient-start",
                form.background_gradient_start_color.as_str(),
            ));
            elem.push_attribute((
                "background-gradient-end",
                form.background_gradient_end_color.as_str(),
            ));
            elem.push_attribute((
                "background-gradient-direction",
                form.background_gradient_direction.as_str(),
            ));
        }
        elem.push_attribute(("transparency", form.transparency.to_string().as_str()));
        elem.push_attribute(("grid-size", form.grid_size.to_string().as_str()));
        elem.push_attribute((
            "snap-to-grid",
            if form.snap_to_grid { "true" } else { "false" },
        ));
        elem.push_attribute(("target", form.target.as_str()));
        // 007 Form themes — additive, only written when set so old forms are
        // unchanged on round-trip.
        if let Some(theme) = form.theme.as_deref().filter(|s| !s.is_empty()) {
            elem.push_attribute(("theme", theme));
        }
        if form.use_theme_background {
            elem.push_attribute(("use-theme-background", "true"));
        }
        if form.glass_style != crate::model::GlassStyle::Classic {
            elem.push_attribute(("glass-style", form.glass_style.as_str()));
        }
        if !form.background_image.is_empty() {
            elem.push_attribute(("background-image", form.background_image.as_str()));
            elem.push_attribute(("bg-image-mode", form.bg_image_mode.as_str()));
        }
        // 037 window lifecycle — additive, only written when non-default so
        // pre-037 forms round-trip byte-identical.
        if form.main_form {
            elem.push_attribute(("main-form", "true"));
        }
        if !form.taskbar_icon.is_empty() {
            elem.push_attribute(("taskbar-icon", form.taskbar_icon.as_str()));
        }
        if !form.can_minimize {
            elem.push_attribute(("can-minimize", "false"));
        }
        if !form.can_maximize {
            elem.push_attribute(("can-maximize", "false"));
        }
        if form.window_state != crate::model::WindowState::Normal {
            elem.push_attribute(("window-state", form.window_state.as_str()));
        }
        if form.full_screen {
            elem.push_attribute(("full-screen", "true"));
        }
        if !form.title_visible {
            elem.push_attribute(("title-visible", "false"));
        }
        // 049 R1 — additive: only written when it is not the Standalone default,
        // so every pre-049 form round-trips byte-identical.
        if form.form_format != crate::model::FormFormat::Standalone {
            elem.push_attribute(("form-format", form.form_format.as_str()));
        }
        // 038 — the effects opt-out, additive like the 037 attributes.
        if !form.window_effects {
            elem.push_attribute(("window-effects", "false"));
        }
        // Window start position — additive. `x`/`y` are written whenever set,
        // even if `start-position` is not `Custom`, so a coordinate staged
        // ahead of switching the dropdown to Custom is not silently dropped.
        let x_str;
        if form.x != 0 {
            x_str = form.x.to_string();
            elem.push_attribute(("x", x_str.as_str()));
        }
        let y_str;
        if form.y != 0 {
            y_str = form.y.to_string();
            elem.push_attribute(("y", y_str.as_str()));
        }
        if form.start_position != crate::model::FormStartPosition::System {
            elem.push_attribute(("start-position", form.start_position.as_str()));
        }
        w.write_event(Event::Start(elem))?;

        // ── <working-storage> ─────────────────────────────────────────────────
        // ── <MenuPaneBackground/> (049 R39) — additive: written only when set,
        // so every pre-049 form keeps its exact on-disk shape.
        if let Some(mp) = &form.menu_pane_background {
            let mut e = BytesStart::new("MenuPaneBackground");
            e.push_attribute(("color", mp.color.as_str()));
            if mp.gradient_enabled {
                e.push_attribute(("gradient-enabled", "true"));
                e.push_attribute(("gradient-start", mp.gradient_start_color.as_str()));
                e.push_attribute(("gradient-end", mp.gradient_end_color.as_str()));
                e.push_attribute(("gradient-direction", mp.gradient_direction.as_str()));
            }
            if mp.transparency != 0 {
                let t = mp.transparency.to_string();
                e.push_attribute(("transparency", t.as_str()));
            }
            if !mp.image.is_empty() {
                e.push_attribute(("image", mp.image.as_str()));
                e.push_attribute(("image-mode", mp.image_mode.as_str()));
            }
            w.write_event(Event::Empty(e))?;
        }

        if !form.user_ws_source.trim().is_empty() {
            w.write_event(Event::Start(BytesStart::new("working-storage")))?;
            w.write_event(Event::CData(BytesCData::new(form.user_ws_source.as_str())))?;
            w.write_event(Event::End(BytesEnd::new("working-storage")))?;
        }

        // ── COBOL structure blocks (spec 005) ─────────────────────────────────
        for (tag, body) in [
            ("special-names", form.cobol_structure.special_names.as_str()),
            ("repository", form.cobol_structure.repository.as_str()),
            ("file-control", form.cobol_structure.file_control.as_str()),
            ("file-section", form.cobol_structure.file_section.as_str()),
        ] {
            if !body.trim().is_empty() {
                w.write_event(Event::Start(BytesStart::new(tag)))?;
                w.write_event(Event::CData(BytesCData::new(body)))?;
                w.write_event(Event::End(BytesEnd::new(tag)))?;
            }
        }

        // ── <user-procedures> (spec 005) ──────────────────────────────────────
        if !form.user_procedures.is_empty() {
            w.write_event(Event::Start(BytesStart::new("user-procedures")))?;
            for up in &form.user_procedures {
                let eb = EventBinding {
                    event: up.name.clone(),
                    paragraph: up.name.clone(),
                    code: up.code.clone(),
                };
                write_event_with_code(&mut w, &eb)?;
            }
            w.write_event(Event::End(BytesEnd::new("user-procedures")))?;
        }

        // ── <form-events> ─────────────────────────────────────────────────────
        if !form.form_events.is_empty() {
            w.write_event(Event::Start(BytesStart::new("form-events")))?;
            for ev in &form.form_events {
                write_event_with_code(&mut w, ev)?;
            }
            w.write_event(Event::End(BytesEnd::new("form-events")))?;
        }

        // ── <DataBindings> ───────────────────────────────────────────────────
        if !form.data_bindings.is_empty() {
            let mut elem = BytesStart::new("DataBindings");
            let schema_version = crate::model::DATA_BINDING_SCHEMA_VERSION.to_string();
            elem.push_attribute(("schema-version", schema_version.as_str()));
            w.write_event(Event::Start(elem))?;
            let json = serde_json::to_string_pretty(&form.data_bindings).map_err(xml_err)?;
            w.write_event(Event::CData(BytesCData::new(json.as_str())))?;
            w.write_event(Event::End(BytesEnd::new("DataBindings")))?;
        }

        // ── Controls ─────────────────────────────────────────────────────────
        for ctrl in &form.controls {
            write_control(&mut w, ctrl)?;
        }

        // ── <deleted-controls> ────────────────────────────────────────────────
        if !form.deleted_code.is_empty() {
            w.write_event(Event::Start(BytesStart::new("deleted-controls")))?;
            for dc in &form.deleted_code {
                let mut de = BytesStart::new("DeletedControl");
                de.push_attribute(("id", dc.control_id.as_str()));
                de.push_attribute(("deleted-at", dc.deleted_at.as_str()));
                w.write_event(Event::Start(de))?;
                for ev in &dc.events {
                    write_event_with_code(&mut w, ev)?;
                }
                w.write_event(Event::End(BytesEnd::new("DeletedControl")))?;
            }
            w.write_event(Event::End(BytesEnd::new("deleted-controls")))?;
        }

        w.write_event(Event::End(BytesEnd::new("Form")))?;
    }
    String::from_utf8(output).map_err(|e| FormError::Xml(e.to_string()))
}

pub fn save_form(form: &Form, path: &Path) -> Result<(), FormError> {
    fs::write(path, form_to_string(form)?.as_bytes())?;
    Ok(())
}

/// Write one event handler as XML.
///
/// Format (v1.1):
/// ```xml
/// <Event name="onClick" paragraph="BTN-OK--CLICK">
///   <LocalWS><![CDATA[ 01 WS-TEMP PIC X(80). ]]></LocalWS>
///   <![CDATA[ DISPLAY "hello". ]]>
/// </Event>
/// ```
///
/// If `local_ws` is empty the `<LocalWS>` child is omitted.
/// For backward compatibility, a plain CDATA body (no LocalWS child) is
/// still accepted on load.
fn write_event_with_code<W: std::io::Write>(
    w: &mut Writer<W>,
    ev: &EventBinding,
) -> Result<(), FormError> {
    let mut ee = BytesStart::new("Event");
    ee.push_attribute(("name", ev.event.as_str()));
    ee.push_attribute(("paragraph", ev.paragraph.as_str()));
    w.write_event(Event::Start(ee))?;
    // Write the full handler source as CDATA (single-source format).
    if !ev.code.is_empty() {
        w.write_event(Event::CData(BytesCData::new(ev.code.as_str())))?;
    }
    w.write_event(Event::End(BytesEnd::new("Event")))?;
    Ok(())
}

fn write_control<W: std::io::Write>(w: &mut Writer<W>, ctrl: &Control) -> Result<(), FormError> {
    let mut elem = BytesStart::new("Control");
    elem.push_attribute(("id", ctrl.id.as_str()));
    elem.push_attribute(("type", ctrl.control_type.as_str()));
    elem.push_attribute(("x", ctrl.rect.x.to_string().as_str()));
    elem.push_attribute(("y", ctrl.rect.y.to_string().as_str()));
    elem.push_attribute(("w", ctrl.rect.w.to_string().as_str()));
    elem.push_attribute(("h", ctrl.rect.h.to_string().as_str()));
    elem.push_attribute(("tab-order", ctrl.tab_order.to_string().as_str()));
    elem.push_attribute(("z-order", ctrl.z_order.to_string().as_str()));
    elem.push_attribute(("visible", if ctrl.visible { "true" } else { "false" }));
    elem.push_attribute(("enabled", if ctrl.enabled { "true" } else { "false" }));
    // Container membership (spec 012): written only when set, so plain forms and
    // older readers are unaffected.
    if let Some(p) = &ctrl.parent {
        elem.push_attribute(("parent", p.as_str()));
    }
    if let Some(t) = ctrl.tab {
        elem.push_attribute(("tab", t.to_string().as_str()));
    }
    w.write_event(Event::Start(elem))?;

    // Properties
    for (name, value) in &ctrl.properties {
        let text = prop_to_string(value);
        let mut prop = BytesStart::new("Property");
        prop.push_attribute(("name", name.as_str()));
        w.write_event(Event::Start(prop))?;
        w.write_event(Event::Text(BytesText::new(&text)))?;
        w.write_event(Event::End(BytesEnd::new("Property")))?;
    }

    // Events — always written as start/end with CDATA (v1.0 format)
    for ev in &ctrl.events {
        write_event_with_code(w, ev)?;
    }

    // Animations
    for anim in &ctrl.animations {
        let mut ae = BytesStart::new("Animation");
        ae.push_attribute(("name", anim.name.as_str()));
        ae.push_attribute(("trigger", anim.trigger.as_str()));
        ae.push_attribute(("kind", anim.kind.as_str()));
        ae.push_attribute(("duration", anim.duration_ms.to_string().as_str()));
        ae.push_attribute(("delay", anim.delay_ms.to_string().as_str()));
        ae.push_attribute(("easing", anim.easing.as_str()));
        ae.push_attribute(("repeat", anim.repeat.as_str()));
        ae.push_attribute(("slide-dx", anim.slide_dx.to_string().as_str()));
        ae.push_attribute(("slide-dy", anim.slide_dy.to_string().as_str()));
        w.write_event(Event::Empty(ae))?;
    }

    if !ctrl.children.is_empty() {
        w.write_event(Event::Start(BytesStart::new("Children")))?;
        for child in &ctrl.children {
            write_control(w, child)?;
        }
        w.write_event(Event::End(BytesEnd::new("Children")))?;
    }

    w.write_event(Event::End(BytesEnd::new("Control")))?;
    Ok(())
}

fn prop_to_string(v: &PropValue) -> String {
    match v {
        PropValue::String(s) => s.clone(),
        PropValue::Int(n) => n.to_string(),
        PropValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_binding() -> DataBindingDef {
        let fields = vec![
            crate::model::BindingField::new("CustomerId", crate::model::BindingDataType::Integer)
                .key(),
            crate::model::BindingField::new("CustomerName", crate::model::BindingDataType::Text)
                .required(),
        ];
        DataBindingDef::new(
            "Bind-Customers",
            "Customers",
            crate::model::BindingSourceDescriptor::IndexedFile {
                definition_path: "data/Customers.cidx".into(),
                record_name: "CustomerRecord".into(),
                fields,
                key_field: Some("CustomerId".into()),
                writable: true,
            },
            crate::model::BindingTargetDescriptor::DataGrid {
                control_id: "GridCustomers".into(),
            },
        )
        .with_mappings(vec![
            crate::model::FieldMapping::new(
                "CustomerName",
                crate::model::BindingTargetPath::GridColumn {
                    control_id: "GridCustomers".into(),
                    column_id: "CustomerName".into(),
                },
            ),
            crate::model::FieldMapping::new(
                "CustomerId",
                crate::model::BindingTargetPath::GridColumn {
                    control_id: "GridCustomers".into(),
                    column_id: "CustomerId".into(),
                },
            ),
        ])
    }

    fn sample_form() -> Form {
        let mut form = Form::new("MAIN-FORM", "Test App", 800, 600);
        form.background_color = "#F0F0F0".into();
        form.background_image = "/tmp/wallpaper.png".into();
        form.bg_image_mode = BgImageMode::Fit;
        // Set OnLoad code
        if let Some(ev) = form.form_events.iter_mut().find(|e| e.event == "onLoad") {
            ev.code = "    MOVE 0 TO WS-COUNTER".into();
        }
        form.user_ws_source = "       01 WS-COUNTER  PIC 9(8) VALUE 0 GLOBAL.\n".into();

        let mut btn = Control::new("BTN-OK", ControlType::Button, 10, 10);
        btn.rect.w = 80;
        btn.rect.h = 30;
        btn.properties
            .insert("Caption".into(), PropValue::String("OK".into()));
        btn.properties.insert("FontSize".into(), PropValue::Int(12));
        btn.events.push(EventBinding {
            event: "onClick".into(),
            paragraph: "BTN-OK--CLICK".into(),
            code: "    MOVE 1 TO WS-COUNTER".into(),
        });
        form.controls.push(btn);

        form
    }

    #[test]
    fn data_binding_cfrm_missing_metadata_loads_empty_and_writes_no_section() {
        let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="MAIN-FORM" title="Main" width="800" height="600" background="#FFFFFF">
  <Control id="CustomerName" type="TextBox" x="10" y="10" w="120" h="24" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="DataItem">LEGACY-CUSTOMER-NAME</Property>
    <Property name="DataFormat">X(30)</Property>
  </Control>
</Form>"##;
        let loaded = load_form_from_str(xml).expect("load old form");
        assert!(loaded.data_bindings.is_empty());
        let scalar = loaded.find_control("CustomerName").expect("scalar control");
        assert_eq!(
            scalar.get_prop("DataItem").map(PropValue::as_str),
            Some("LEGACY-CUSTOMER-NAME")
        );

        let path = std::env::temp_dir().join("cobolt_test_no_bindings_022.cfrm");
        save_form(&loaded, &path).expect("save old form");
        let saved = std::fs::read_to_string(&path).expect("read saved form");
        assert!(!saved.contains("<DataBindings"));
        assert!(saved.contains("LEGACY-CUSTOMER-NAME"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn data_binding_cfrm_roundtrip_preserves_metadata_and_unrelated_controls() {
        let mut form = sample_form();
        let mut grid = Control::new("GridCustomers", ControlType::DataGrid, 20, 60);
        grid.events.push(EventBinding {
            event: "onRowSelect".into(),
            paragraph: "GRIDCUSTOMERS--ONROWSELECT".into(),
            code: "       PROCEDURE DIVISION.\n           CONTINUE.\n".into(),
        });
        let mut scalar = Control::new("StandaloneName", ControlType::TextBox, 20, 280);
        scalar.set_prop("DataItem", PropValue::String("LEGACY-NAME".into()));
        scalar.set_prop("DataFormat", PropValue::String("X(40)".into()));
        form.controls.push(grid);
        form.controls.push(scalar);
        form.data_bindings.push(sample_binding());

        let path = std::env::temp_dir().join("cobolt_test_bindings_022.cfrm");
        save_form(&form, &path).expect("save bound form");
        let saved = std::fs::read_to_string(&path).expect("read bound form");
        assert!(saved.contains("<DataBindings schema-version=\"1\">"));
        assert!(saved.contains("CustomerName"));

        let loaded = load_form(&path).expect("load bound form");
        assert_eq!(loaded.data_bindings, form.data_bindings);
        let grid = loaded.find_control("GridCustomers").expect("grid");
        assert_eq!(grid.events.len(), 1);
        let scalar = loaded.find_control("StandaloneName").expect("scalar");
        assert_eq!(
            scalar.get_prop("DataItem").map(PropValue::as_str),
            Some("LEGACY-NAME")
        );
        assert_eq!(
            loaded.data_bindings[0].source.fields()[1].name,
            "CustomerName"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn datagrid_advanced_cfrm_legacy_grid_loads_with_seeded_defaults_023() {
        let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="MAIN-FORM" title="Main" width="800" height="600" background="#FFFFFF">
  <Control id="Actors" type="DataGrid" x="10" y="10" w="420" h="220" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Columns">Actor Id:number
Actor Caption:string</Property>
    <Property name="Rows">1	Leonardo DiCaprio</Property>
  </Control>
</Form>"##;

        let loaded = load_form_from_str(xml).expect("load legacy grid form");
        let grid = loaded.find_control("Actors").expect("grid");
        assert_eq!(
            grid.get_prop("Columns").map(PropValue::as_str),
            Some("Actor Id:number\nActor Caption:string")
        );
        assert_eq!(
            grid.get_prop(crate::model::DATAGRID_ADVANCED_PROP)
                .map(PropValue::as_str),
            Some("")
        );
        assert_eq!(
            grid.get_prop("GridLineStyle").map(PropValue::as_str),
            Some("Solid")
        );
        assert!(grid
            .get_prop("SelectableText")
            .expect("SelectableText")
            .as_bool());

        let advanced = crate::model::DataGridAdvanced::from_control(grid);
        assert_eq!(advanced.columns.len(), 2);
        assert_eq!(advanced.columns[0].title, "Actor Id");
        assert_eq!(advanced.columns[1].source_name, "Actor Caption");
    }

    /// A checkbox saved before the border keys existed carries none of them,
    /// and `border_rows` shows a row only for a key that is present — so
    /// without this backfill the pane would stay empty on every existing form
    /// while `draw_control` kept painting its "Single" fallback box. An
    /// explicit value the developer already chose is never overwritten.
    #[test]
    fn legacy_checkbox_gains_border_properties_on_load() {
        let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="MAIN-FORM" title="Main" width="800" height="600" background="#FFFFFF">
  <Control id="chkBurger1" type="CheckBox" x="20" y="20" w="150" h="24" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Caption">Big Mac - 7,49</Property>
  </Control>
  <Control id="rdoPaid" type="RadioButton" x="20" y="60" w="150" h="24" tab-order="1" z-order="0" visible="true" enabled="true">
    <Property name="BorderStyle">Sunken</Property>
  </Control>
</Form>"##;

        let loaded = load_form_from_str(xml).expect("load legacy checkbox form");

        let chk = loaded.find_control("chkBurger1").expect("checkbox");
        assert_eq!(chk.get_prop("BorderStyle").map(PropValue::as_str), Some("None"));
        assert_eq!(
            chk.get_prop("BorderColor").map(PropValue::as_str),
            Some("#8C8CA0")
        );
        assert_eq!(chk.get_prop("BorderWidth").expect("BorderWidth").as_i64(), 1);
        // The caption it already had is untouched.
        assert_eq!(
            chk.get_prop("Caption").map(PropValue::as_str),
            Some("Big Mac - 7,49")
        );

        let rdo = loaded.find_control("rdoPaid").expect("radio button");
        assert_eq!(
            rdo.get_prop("BorderStyle").map(PropValue::as_str),
            Some("Sunken"),
            "an explicit choice must survive the backfill"
        );
    }

    /// A Label saved before `CornerRadius`/`BorderStyle` were seeded on every
    /// visual control (6c64c80, 1.63.19) carries neither — `Control::new`'s
    /// own fix never runs for a control loaded from disk, so a form saved
    /// before that change kept the exact bug it fixed, forever, for every
    /// control already in it. `seed_theme_owned_appearance` closes that by
    /// running on load too, through the identical function `Control::new`
    /// itself calls — not a second, hand-kept copy of the same boundary
    /// (operator, 2026-09-03: "your merge once again removed the radius
    /// corner from labels" — traced to exactly this: a real, pre-existing
    /// PowerDemo3 Label with no `CornerRadius` property in its saved XML at
    /// all, from before the seed existed).
    #[test]
    fn legacy_label_gains_corner_radius_and_border_style_on_load() {
        let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="MAIN-FORM" title="Main" width="800" height="600" background="#FFFFFF">
  <Control id="Label-2" type="Label" x="568" y="96" w="688" h="144" tab-order="0" z-order="62" visible="true" enabled="true">
    <Property name="Caption">hello</Property>
    <Property name="BackgroundGradientEnabled">true</Property>
  </Control>
  <Control id="Label-3" type="Label" x="20" y="260" w="150" h="24" tab-order="1" z-order="0" visible="true" enabled="true">
    <Property name="Caption">already set</Property>
    <Property name="CornerRadius">10</Property>
    <Property name="BorderStyle">Fixed3D</Property>
  </Control>
</Form>"##;

        let loaded = load_form_from_str(xml).expect("load legacy label form");

        let untouched = loaded.find_control("Label-2").expect("Label-2");
        assert_eq!(
            untouched.get_prop("CornerRadius").map(PropValue::as_i64),
            Some(0),
            "a Label with no saved CornerRadius must gain the seeded default \
             on load, the same as a freshly created one — not stay absent"
        );
        assert_eq!(
            untouched.get_prop("BorderStyle").map(PropValue::as_str),
            // "the same as a freshly created one" is what this test says, and
            // a fresh Label's own seed is "None". It asserted "Single" until
            // 1.63.39 — the flat value the backfill invented, which no new
            // Label has ever carried. The claim was right; the number was the
            // bug, and it is what put a rim on every loaded Switch.
            Some("None"),
            "and the same for BorderStyle, seeded alongside it"
        );
        // The caption and the gradient flag it already had are untouched.
        assert_eq!(
            untouched.get_prop("Caption").map(PropValue::as_str),
            Some("hello")
        );
        assert!(untouched
            .get_prop("BackgroundGradientEnabled")
            .map(PropValue::as_bool)
            .unwrap_or(false));

        let chosen = loaded.find_control("Label-3").expect("Label-3");
        assert_eq!(
            chosen.get_prop("CornerRadius").map(PropValue::as_i64),
            Some(10),
            "an explicit CornerRadius must survive the backfill"
        );
        assert_eq!(
            chosen.get_prop("BorderStyle").map(PropValue::as_str),
            Some("Fixed3D"),
            "an explicit BorderStyle must survive the backfill"
        );
    }

    /// **The regression guard for the white rim around a saved Switch.**
    ///
    /// A Switch saved before `BorderStyle` reached the control at all (1.63.35)
    /// carries no such property — PowerDemo3's own `switch-form.cfrm`, written
    /// 2026-09-02, is exactly this file. When the load-time backfill (1.63.36)
    /// handed it the flat `"Single"`, the pill-stroking code 1.63.35 had just
    /// added drew a border around every one of them: a white outline hugging
    /// the track, in both the off and the on state (operator screenshots,
    /// 2026-09-03). Nothing on the canvas had asked for it and nothing short of
    /// setting `BorderStyle` by hand could take it away.
    #[test]
    fn a_legacy_switch_loads_without_a_border() {
        let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="MAIN-FORM" title="Main" width="800" height="600" background="#FFFFFF">
  <Control id="Switch-1" type="Switch" x="40" y="40" w="52" h="28" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Checked">false</Property>
  </Control>
  <Control id="Switch-2" type="Switch" x="40" y="90" w="52" h="28" tab-order="1" z-order="1" visible="true" enabled="true">
    <Property name="Checked">true</Property>
    <Property name="BorderStyle">Single</Property>
  </Control>
</Form>"##;

        let loaded = load_form_from_str(xml).expect("load legacy switch form");

        let plain = loaded.find_control("Switch-1").expect("Switch-1");
        assert_eq!(
            plain.get_prop("BorderStyle").map(PropValue::as_str),
            Some("None"),
            "a saved Switch must load frameless, like a freshly dropped one — \
             a seeded border draws a rim around the pill"
        );
        // The row still EXISTS, which is what the backfill is for: the
        // inspector shows a row only for a property that is present, so the
        // developer can still turn a border on.
        assert!(
            plain.get_prop("BorderStyle").is_some(),
            "the row must exist so the properties pane can offer it"
        );

        let chosen = loaded.find_control("Switch-2").expect("Switch-2");
        assert_eq!(
            chosen.get_prop("BorderStyle").map(PropValue::as_str),
            Some("Single"),
            "a border the developer did ask for must survive the backfill"
        );
    }

    #[test]
    fn datagrid_advanced_cfrm_roundtrip_preserves_metadata_023() {
        let mut form = sample_form();
        let mut grid = Control::new("GridActors", ControlType::DataGrid, 20, 60);
        grid.set_prop(
            "Columns",
            PropValue::String("Actor Id:number\nStatus:string".into()),
        );
        grid.set_prop("Rows", PropValue::String("1\tActive\n2\tTrial".into()));

        let mut advanced = crate::model::DataGridAdvanced::default();
        advanced.frozen_columns = 1;
        advanced.frozen_rows = 1;
        advanced.row_height = 30;
        advanced.grid_line_style = crate::model::DataGridGridLineStyle::Dots;
        advanced
            .row_overrides
            .push(crate::model::DataGridRowHeightOverride {
                row_index: 2,
                height: 44,
            });
        advanced.filters.push(crate::model::DataGridFilter {
            column_id: "STATUS".into(),
            value: "Active".into(),
            active: true,
        });
        advanced.columns.push(crate::model::DataGridColumn {
            id: "STATUS".into(),
            title: "Status".into(),
            source_name: "Status".into(),
            width: 150.0,
            frame: Some(crate::model::DataGridCellFrame {
                enabled: true,
                corner_radius: 12,
                ..crate::model::DataGridCellFrame::default()
            }),
            gauge: Some(crate::model::DataGridGauge {
                enabled: true,
                max: 1000.0,
                ..crate::model::DataGridGauge::default()
            }),
            value_style_rules: vec![crate::model::DataGridValueStyleRule {
                value: "Active".into(),
                frame_background_color: "#10B981".into(),
                ..crate::model::DataGridValueStyleRule::default()
            }],
            ..crate::model::DataGridColumn::default()
        });
        grid.set_prop(
            crate::model::DATAGRID_ADVANCED_PROP,
            PropValue::String(advanced.to_json().unwrap()),
        );
        form.controls.push(grid);

        let path = std::env::temp_dir().join("cobolt_test_datagrid_advanced_023.cfrm");
        save_form(&form, &path).expect("save advanced grid");
        let saved = std::fs::read_to_string(&path).expect("read advanced grid");
        assert!(saved.contains(r#"<Property name="AdvancedGrid">"#));
        assert!(saved.contains("Status"));
        assert!(saved.contains("Active"));

        let loaded = load_form(&path).expect("load advanced grid");
        let grid = loaded.find_control("GridActors").expect("grid");
        assert_eq!(
            grid.get_prop("Rows").map(PropValue::as_str),
            Some("1\tActive\n2\tTrial")
        );
        let parsed = crate::model::DataGridAdvanced::from_control(grid);
        assert_eq!(parsed.columns.len(), 1);
        assert_eq!(parsed.frozen_columns, 1);
        assert_eq!(parsed.frozen_rows, 1);
        assert_eq!(parsed.row_overrides[0].height, 44);
        assert_eq!(parsed.filters[0].value, "Active");
        assert_eq!(parsed.columns[0].frame.as_ref().unwrap().corner_radius, 12);
        assert!(parsed.columns[0].gauge.as_ref().unwrap().enabled);
        assert_eq!(
            parsed.grid_line_style,
            crate::model::DataGridGridLineStyle::Dots
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn roundtrip_in_memory() {
        let form = sample_form();
        let dir = std::env::temp_dir();
        let path: PathBuf = dir.join("cobolt_test_roundtrip.cfrm");
        save_form(&form, &path).expect("save_form failed");
        let loaded = load_form(&path).expect("load_form failed");

        assert_eq!(loaded.name, form.name);
        assert_eq!(loaded.title, form.title);
        assert_eq!(loaded.width, form.width);
        assert_eq!(loaded.height, form.height);
        assert_eq!(loaded.background_color, form.background_color);
        // Background image path + mode must survive the save/reload the inline
        // inspector performs immediately after a pick.
        assert_eq!(loaded.background_image, "/tmp/wallpaper.png");
        assert_eq!(loaded.bg_image_mode, BgImageMode::Fit);

        // User WS preserved
        assert!(loaded.user_ws_source.contains("WS-COUNTER"));

        // Form events with code
        let on_load = loaded.form_events.iter().find(|e| e.event == "onLoad");
        assert!(on_load.is_some());
        assert!(on_load.unwrap().code.contains("WS-COUNTER"));

        // Controls
        assert_eq!(loaded.controls.len(), 1);
        let btn = &loaded.controls[0];
        assert_eq!(btn.id, "BTN-OK");
        assert_eq!(btn.control_type, ControlType::Button);
        assert_eq!(btn.rect.x, 10);
        assert_eq!(btn.rect.w, 80);
        assert_eq!(btn.events.len(), 1);
        assert_eq!(btn.events[0].event, "onClick");
        assert_eq!(btn.events[0].paragraph, "BTN-OK--CLICK");
        assert!(btn.events[0].code.contains("WS-COUNTER"));

        let _ = std::fs::remove_file(&path);
    }

    /// Spec 046 R2/R11 — `form_to_string` is the file-free half of the same
    /// round trip `roundtrip_in_memory` proves through disk: everything a
    /// pasted form needs (control properties, the bound event's full COBOL
    /// body, form-level WS) survives a string round trip with no filesystem
    /// involved.
    #[test]
    fn form_to_string_and_load_form_from_str_round_trip() {
        let form = sample_form();
        let xml = form_to_string(&form).expect("form_to_string failed");
        let loaded = load_form_from_str(&xml).expect("load_form_from_str failed");

        assert_eq!(loaded.name, form.name);
        assert_eq!(loaded.title, form.title);
        assert_eq!(loaded.width, form.width);
        assert_eq!(loaded.height, form.height);
        assert_eq!(loaded.background_color, form.background_color);
        assert!(loaded.user_ws_source.contains("WS-COUNTER"));

        let on_load = loaded.form_events.iter().find(|e| e.event == "onLoad");
        assert!(on_load.is_some());
        assert!(on_load.unwrap().code.contains("WS-COUNTER"));

        assert_eq!(loaded.controls.len(), 1);
        let btn = &loaded.controls[0];
        assert_eq!(btn.id, "BTN-OK");
        assert_eq!(btn.control_type, ControlType::Button);
        assert_eq!(btn.events.len(), 1);
        assert_eq!(btn.events[0].event, "onClick");
        assert_eq!(btn.events[0].paragraph, "BTN-OK--CLICK");
        assert!(btn.events[0].code.contains("WS-COUNTER"));
    }

    /// Spec 046 T1 — the `save_form`/`form_to_string` refactor must not
    /// change a single byte of what actually lands on disk.
    #[test]
    fn save_form_and_form_to_string_agree() {
        let form = sample_form();
        let path = std::env::temp_dir().join("cobolt_test_form_to_string_agree.cfrm");
        save_form(&form, &path).expect("save_form failed");
        let from_disk = std::fs::read_to_string(&path).expect("read back");
        let from_string = form_to_string(&form).expect("form_to_string failed");
        assert_eq!(from_disk, from_string);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn roundtrip_theme_007() {
        // A form with a per-form theme + themed background round-trips.
        let mut form = sample_form();
        form.theme = Some("stainless-steel".into());
        form.use_theme_background = true;
        let path = std::env::temp_dir().join("cobolt_test_theme.cfrm");
        save_form(&form, &path).expect("save");
        let loaded = load_form(&path).expect("load");
        assert_eq!(loaded.theme.as_deref(), Some("stainless-steel"));
        assert!(loaded.use_theme_background);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn roundtrip_window_lifecycle_037() {
        // Every 037 field set to its NON-default value survives save → load.
        let mut form = sample_form();
        form.main_form = true;
        form.taskbar_icon = "assets/app-icon.png".into();
        form.can_minimize = false;
        form.can_maximize = false;
        form.window_state = crate::model::WindowState::Maximized;
        form.full_screen = true;
        form.title_visible = false;
        let path = std::env::temp_dir().join("cobolt_test_window_lifecycle_037.cfrm");
        save_form(&form, &path).expect("save");
        let loaded = load_form(&path).expect("load");
        assert!(loaded.main_form, "main_form");
        assert_eq!(loaded.taskbar_icon, "assets/app-icon.png", "taskbar_icon");
        assert!(!loaded.can_minimize, "can_minimize");
        assert!(!loaded.can_maximize, "can_maximize");
        assert_eq!(
            loaded.window_state,
            crate::model::WindowState::Maximized,
            "window_state"
        );
        assert!(loaded.full_screen, "full_screen");
        assert!(!loaded.title_visible, "title_visible");
        println!(
            "037 round-trip: main_form={} taskbar_icon={:?} can_minimize={} can_maximize={} \
             window_state={} full_screen={} title_visible={}",
            loaded.main_form,
            loaded.taskbar_icon,
            loaded.can_minimize,
            loaded.can_maximize,
            loaded.window_state.as_str(),
            loaded.full_screen,
            loaded.title_visible
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn menu_pane_background_round_trips_049() {
        // 049 R39 (model half) — the MenuPane background group round-trips with
        // every field set; a form without it saves no element and loads None.
        use crate::model::MenuPaneBackground;

        let mut form = sample_form();
        form.menu_pane_background = Some(MenuPaneBackground {
            color: "#1A2B3CFF".into(),
            gradient_enabled: true,
            gradient_start_color: "#101010".into(),
            gradient_end_color: "#404040".into(),
            gradient_direction: "East".into(),
            transparency: 35,
            image: "assets/rail.png".into(),
            image_mode: BgImageMode::Tile,
        });
        let path = std::env::temp_dir().join("cobolt_test_menu_pane_bg_049.cfrm");
        save_form(&form, &path).expect("save");
        let xml = std::fs::read_to_string(&path).expect("read");
        let loaded = load_form(&path).expect("load");
        let mp = loaded
            .menu_pane_background
            .as_ref()
            .expect("the MenuPane background survived");
        assert_eq!(mp.color, "#1A2B3CFF");
        assert!(mp.gradient_enabled);
        assert_eq!(mp.gradient_start_color, "#101010");
        assert_eq!(mp.gradient_end_color, "#404040");
        assert_eq!(mp.gradient_direction, "East");
        assert_eq!(mp.transparency, 35);
        assert_eq!(mp.image, "assets/rail.png");
        assert_eq!(mp.image_mode, BgImageMode::Tile);
        assert!(xml.contains("<MenuPaneBackground"), "element written");
        let _ = std::fs::remove_file(&path);

        // Absent ⇒ None, and no element written (a pre-049 file's shape).
        let plain = sample_form();
        let p2 = std::env::temp_dir().join("cobolt_test_menu_pane_bg_none_049.cfrm");
        save_form(&plain, &p2).expect("save plain");
        let plain_xml = std::fs::read_to_string(&p2).expect("read plain");
        assert!(!plain_xml.contains("MenuPaneBackground"));
        let plain_loaded = load_form(&p2).expect("load plain");
        assert!(plain_loaded.menu_pane_background.is_none());
        let _ = std::fs::remove_file(&p2);

        println!(
            "049 R39 MenuPane background — 8/8 fields round-trip \
             (color={} gradient={}→{} dir={} transparency={} image={} mode={}); \
             absent element loads None",
            mp.color,
            mp.gradient_start_color,
            mp.gradient_end_color,
            mp.gradient_direction,
            mp.transparency,
            mp.image,
            mp.image_mode.as_str()
        );
    }

    #[test]
    fn side_menu_control_round_trips_049() {
        // 049 R45 — a SideMenu survives save → load, and a form carrying a
        // MenuBar round-trips byte-identical, so no existing project can be
        // turned into a shell app by accident (R3).
        let mut form = sample_form();
        let mut side = Control::new("SIDE-1", ControlType::SideMenu, 0, 0);
        side.rect.w = 200;
        side.rect.h = 400;
        side.properties.insert(
            "ForegroundColor".into(),
            PropValue::String("#E1E6FA".into()),
        );
        form.controls.push(side);

        let path = std::env::temp_dir().join("cobolt_test_side_menu_049.cfrm");
        save_form(&form, &path).expect("save");
        let loaded = load_form(&path).expect("load");
        let found = loaded
            .controls
            .iter()
            .find(|c| c.id == "SIDE-1")
            .expect("the SideMenu survived the round-trip");
        assert_eq!(found.control_type, ControlType::SideMenu);
        assert_eq!(found.rect.w, 200, "the WIDTH is the developer's to set");
        // 049 — FullHeight is on by default, so the designed 400 is replaced
        // by the form's own height on load: the sidebar IS the window's
        // vertical extent, and the stored geometry now says so.
        assert!(found.side_menu_full_height(), "FullHeight defaults to on");
        assert_eq!(found.rect.y, 0, "a full-height sidebar starts at the top");
        assert_eq!(
            found.rect.h, form.height as i32,
            "a full-height sidebar is exactly as tall as the form"
        );
        let _ = std::fs::remove_file(&path);

        // FullHeight off ⇒ the developer's placement is kept verbatim.
        let mut form_off = sample_form();
        let mut side_off = Control::new("SIDE-2", ControlType::SideMenu, 0, 0);
        side_off.rect.w = 200;
        side_off.rect.y = 40;
        side_off.rect.h = 400;
        side_off
            .properties
            .insert("FullHeight".into(), PropValue::Bool(false));
        form_off.controls.push(side_off);
        let path_off = std::env::temp_dir().join("cobolt_test_side_menu_049_off.cfrm");
        save_form(&form_off, &path_off).expect("save");
        let loaded_off = load_form(&path_off).expect("load");
        let found_off = loaded_off
            .controls
            .iter()
            .find(|c| c.id == "SIDE-2")
            .expect("the SideMenu survived the round-trip");
        assert!(!found_off.side_menu_full_height(), "the property round-trips");
        assert_eq!(found_off.rect.y, 40, "FullHeight off keeps the placed Y");
        assert_eq!(found_off.rect.h, 400, "FullHeight off keeps the placed height");
        let _ = std::fs::remove_file(&path_off);

        // A legacy SideMenu written before the property existed reads as ON,
        // so an old shell project keeps a full-height sidebar.
        let mut legacy = Control::new("SIDE-3", ControlType::SideMenu, 0, 0);
        legacy.properties.shift_remove("FullHeight");
        assert!(
            legacy.side_menu_full_height(),
            "absent FullHeight means on (legacy .cfrm)"
        );
        // The property is a SideMenu's alone — a MenuBar is a horizontal strip.
        let bar = Control::new("BAR-1", ControlType::MenuBar, 0, 0);
        assert!(bar.get_prop("FullHeight").is_none());
        assert!(!bar.side_menu_full_height());
        assert!(bar.get_prop("Collapsed").is_none());
        assert!(!bar.side_menu_collapsed());

        // `Collapsed` — the state the application OPENS in — round-trips, and
        // defaults to open both when unset and on a legacy control.
        let mut form_col = sample_form();
        let mut side_col = Control::new("SIDE-4", ControlType::SideMenu, 0, 0);
        assert!(!side_col.side_menu_collapsed(), "a new SideMenu opens open");
        side_col.set_prop("Collapsed", true);
        form_col.controls.push(side_col);
        let path_col = std::env::temp_dir().join("cobolt_test_side_menu_049_collapsed.cfrm");
        save_form(&form_col, &path_col).expect("save");
        let loaded_col = load_form(&path_col).expect("load");
        assert!(
            loaded_col
                .controls
                .iter()
                .find(|c| c.id == "SIDE-4")
                .expect("the SideMenu survived the round-trip")
                .side_menu_collapsed(),
            "Collapsed survives save → load"
        );
        let _ = std::fs::remove_file(&path_col);

        let mut legacy_col = Control::new("SIDE-5", ControlType::SideMenu, 0, 0);
        legacy_col.properties.shift_remove("Collapsed");
        assert!(
            !legacy_col.side_menu_collapsed(),
            "absent Collapsed means open (legacy .cfrm)"
        );

        // A MenuBar form is untouched by 049: its control keeps its type, gains
        // no SideMenu markup, and gains no form-format attribute — so it still
        // starts in classic multi-window mode (R3).
        //
        // Note: this deliberately does NOT compare two consecutive saves. Loading
        // normalises event bodies (adding the ENVIRONMENT/DATA/PROCEDURE DIVISION
        // scaffold) and drops empty properties, so save → load → save is not
        // byte-identical for ANY form in this format. That predates 049.
        let mut bar_form = sample_form();
        bar_form
            .controls
            .push(Control::new("BAR-1", ControlType::MenuBar, 0, 0));
        let bar_path = std::env::temp_dir().join("cobolt_test_menubar_049.cfrm");
        save_form(&bar_form, &bar_path).expect("save menubar");
        let bar_xml = std::fs::read_to_string(&bar_path).expect("read menubar");
        let bar_loaded = load_form(&bar_path).expect("load menubar");
        let bar_ctrl = bar_loaded
            .controls
            .iter()
            .find(|c| c.id == "BAR-1")
            .expect("the MenuBar survived");
        assert_eq!(bar_ctrl.control_type, ControlType::MenuBar);
        assert!(
            bar_xml.contains("type=\"MenuBar\""),
            "the MenuBar control must still serialise as MenuBar"
        );
        assert!(
            !bar_xml.contains("SideMenu"),
            "a MenuBar form must not gain any SideMenu markup"
        );
        assert!(
            !bar_xml.contains("form-format="),
            "a MenuBar form must not gain a form-format attribute"
        );
        assert_eq!(
            bar_loaded.form_format,
            crate::model::FormFormat::Standalone,
            "a MenuBar form stays Standalone, so the shell is not triggered (R3)"
        );
        let _ = std::fs::remove_file(&bar_path);

        println!(
            "049 R45 SideMenu — control round-trip: type={} {}x{}; \
             MenuBar form unchanged: type={} form_format={} \
             (no SideMenu markup, no form-format attribute)",
            found.control_type.as_str(),
            found.rect.w,
            found.rect.h,
            bar_ctrl.control_type.as_str(),
            bar_loaded.form_format.as_str()
        );
    }

    #[test]
    fn shell_activation_is_side_menu_only_049() {
        // AC1/AC2/AC25 (decision half) — only a SideMenu control puts a form
        // in shell mode; a MenuBar (or nothing) keeps classic mode (R2/R3).
        let plain = sample_form();
        assert!(!plain.has_side_menu(), "no menu ⇒ classic");

        let mut with_bar = sample_form();
        with_bar
            .controls
            .push(Control::new("BAR-1", ControlType::MenuBar, 0, 0));
        assert!(
            !with_bar.has_side_menu(),
            "AC25: a MenuBar must NOT trigger the shell"
        );

        let mut with_side = sample_form();
        with_side
            .controls
            .push(Control::new("SIDE-1", ControlType::SideMenu, 0, 0));
        assert!(with_side.has_side_menu(), "AC2: a SideMenu triggers it");
        assert_eq!(
            with_side.side_menu_control_id().as_deref(),
            Some("SIDE-1"),
            "the mounting id is the control's"
        );

        // Nested inside a container still counts.
        let mut nested = sample_form();
        let mut panel = Control::new("PANEL-1", ControlType::Panel, 0, 0);
        panel
            .children
            .push(Control::new("SIDE-2", ControlType::SideMenu, 0, 0));
        nested.controls.push(panel);
        assert!(nested.has_side_menu(), "nested SideMenu counts");

        println!(
            "049 AC1/AC2/AC25 (decision) — none ⇒ classic, MenuBar ⇒ classic, \
             SideMenu ⇒ shell (id SIDE-1), nested SideMenu ⇒ shell (4/4)"
        );
    }

    #[test]
    fn form_format_round_trips_049() {
        // 049 R1 — every value survives save → load, and the Standalone default
        // is never written, so a pre-049 form keeps its exact on-disk shape (R3).
        use crate::model::FormFormat;
        let mut covered = Vec::new();
        let mut no_attr_xml = String::new();
        for fmt in [
            FormFormat::Standalone,
            FormFormat::Embedded,
            FormFormat::Both,
        ] {
            let mut form = sample_form();
            form.form_format = fmt;
            let path =
                std::env::temp_dir().join(format!("cobolt_test_form_format_{}.cfrm", fmt.as_str()));
            save_form(&form, &path).expect("save");
            let xml = std::fs::read_to_string(&path).expect("read");
            let loaded = load_form(&path).expect("load");
            assert_eq!(
                loaded.form_format,
                fmt,
                "{} did not round-trip",
                fmt.as_str()
            );
            let has_attr = xml.contains("form-format=");
            assert_eq!(
                has_attr,
                fmt != FormFormat::Standalone,
                "{}: the attribute must be written only when it is not the default",
                fmt.as_str()
            );
            if fmt == FormFormat::Standalone {
                no_attr_xml = xml;
            }
            covered.push(format!("{} (attr written: {})", fmt.as_str(), has_attr));
            let _ = std::fs::remove_file(&path);
        }

        // The Standalone save carries no `form-format` at all — the same shape a
        // .cfrm written before 049 has. Loading it must give Standalone (R3).
        let legacy = load_form_from_str(&no_attr_xml).expect("legacy load");
        assert_eq!(legacy.form_format, FormFormat::Standalone);

        println!(
            "049 R1 FormFormat round-trip — {} values covered: {}; \
             a file with no form-format attribute loads as {}",
            covered.len(),
            covered.join(", "),
            legacy.form_format.as_str()
        );
    }

    #[test]
    fn window_effects_optout_round_trips_038() {
        // false survives save→load; absent ⇒ true; the default-valued attr
        // is never written (additive contract, same as 037).
        let mut form = sample_form();
        form.window_effects = false;
        let path = std::env::temp_dir().join("cobolt_test_window_effects_038.cfrm");
        save_form(&form, &path).expect("save");
        let loaded = load_form(&path).expect("load");
        assert!(!loaded.window_effects, "false round-trips");
        let _ = std::fs::remove_file(&path);

        let plain = load_form_from_str(
            r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="F" title="F" width="640" height="480" background="#FFFFFF"></Form>"##,
        )
        .expect("load");
        assert!(plain.window_effects, "absent ⇒ true");
        let path2 = std::env::temp_dir().join("cobolt_test_window_effects_default_038.cfrm");
        save_form(&plain, &path2).expect("save");
        let saved = std::fs::read_to_string(&path2).expect("read");
        assert!(
            !saved.contains("window-effects"),
            "default true must not be written"
        );
        let _ = std::fs::remove_file(&path2);
        println!(
            "038 opt-out: false round-trips = {}, absent ⇒ {}",
            !loaded.window_effects, plain.window_effects
        );
    }

    /// A form that predates window start position (no `x`/`y`/`start-position`
    /// attributes at all) must load to exactly today's behaviour: `System`,
    /// 0, 0 — the one variant nothing applies at launch. This is the
    /// byte-for-byte backward-compatibility guarantee the feature depends on.
    #[test]
    fn window_start_position_is_additive_and_backward_compatible() {
        let plain = load_form_from_str(
            r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="F" title="F" width="640" height="480" background="#FFFFFF"></Form>"##,
        )
        .expect("load");
        assert_eq!(plain.start_position, crate::model::FormStartPosition::System);
        assert_eq!((plain.x, plain.y), (0, 0));
        let path = std::env::temp_dir().join("cobolt_test_start_position_absent.cfrm");
        save_form(&plain, &path).expect("save");
        let saved = std::fs::read_to_string(&path).expect("read");
        assert!(
            !saved.contains("start-position") && !saved.contains(" x=") && !saved.contains(" y="),
            "System/0/0 must not be written — the additive contract: {saved}"
        );
        let _ = std::fs::remove_file(&path);

        // Custom + a real coordinate round-trips exactly.
        let mut form = sample_form();
        form.start_position = crate::model::FormStartPosition::Custom;
        form.x = 240;
        form.y = -30; // a second monitor to the left is a legitimate coordinate
        let path2 = std::env::temp_dir().join("cobolt_test_start_position_manual.cfrm");
        save_form(&form, &path2).expect("save");
        let loaded = load_form(&path2).expect("load");
        assert_eq!(loaded.start_position, crate::model::FormStartPosition::Custom);
        assert_eq!((loaded.x, loaded.y), (240, -30));
        let _ = std::fs::remove_file(&path2);

        // A screen-relative choice round-trips too, and x/y stay whatever
        // they were staged to — switching away from Custom does not erase
        // them, so switching back finds the developer's coordinate intact.
        let mut form2 = sample_form();
        form2.start_position = crate::model::FormStartPosition::BottomRight;
        form2.x = 15;
        form2.y = 15;
        let path3 = std::env::temp_dir().join("cobolt_test_start_position_screen_relative.cfrm");
        save_form(&form2, &path3).expect("save");
        let loaded2 = load_form(&path3).expect("load");
        assert_eq!(loaded2.start_position, crate::model::FormStartPosition::BottomRight);
        assert_eq!((loaded2.x, loaded2.y), (15, 15));
        let _ = std::fs::remove_file(&path3);
    }

    #[test]
    fn pre_037_cfrm_loads_with_window_defaults_and_saves_unchanged() {
        // A pre-037 file has none of the new attributes: it must load with the
        // historical behaviour (not main, min/max on, Normal, windowed,
        // title shown) and, still at defaults, write NONE of the new
        // attributes back — old projects stay byte-stable over this feature.
        let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="OLD-FORM" title="Old" width="640" height="480" background="#FFFFFF">
</Form>"##;
        let loaded = load_form_from_str(xml).expect("load pre-037 form");
        assert!(!loaded.main_form);
        assert!(loaded.taskbar_icon.is_empty());
        assert!(loaded.can_minimize);
        assert!(loaded.can_maximize);
        assert_eq!(loaded.window_state, crate::model::WindowState::Normal);
        assert!(!loaded.full_screen);
        assert!(loaded.title_visible);
        println!(
            "pre-037 defaults: main_form={} taskbar_icon={:?} can_minimize={} \
             can_maximize={} window_state={} full_screen={} title_visible={}",
            loaded.main_form,
            loaded.taskbar_icon,
            loaded.can_minimize,
            loaded.can_maximize,
            loaded.window_state.as_str(),
            loaded.full_screen,
            loaded.title_visible
        );
        let path = std::env::temp_dir().join("cobolt_test_pre037_defaults.cfrm");
        save_form(&loaded, &path).expect("save");
        let saved = std::fs::read_to_string(&path).expect("read back");
        for attr in [
            "main-form",
            "taskbar-icon",
            "can-minimize",
            "can-maximize",
            "window-state",
            "full-screen",
            "title-visible",
        ] {
            assert!(
                !saved.contains(attr),
                "default-valued 037 attribute {attr:?} must not be written"
            );
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn roundtrip_nested_containers_012() {
        // A deeply nested form round-trips parent/tab links and the new container
        // props (spec 012).
        let mut form = Form::new("F", "F", 640, 480);
        let mut pnl = Control::new("Pnl", ControlType::Panel, 10, 10);
        pnl.set_prop("BorderRadius", PropValue::Int(8));
        pnl.set_prop("HScroll", PropValue::Bool(true));
        pnl.set_prop("VScroll", PropValue::Bool(true));
        let mut grp = Control::new("Grp", ControlType::GroupBox, 20, 20);
        grp.parent = Some("Pnl".into());
        let mut tabs = Control::new("Tabs", ControlType::TabControl, 25, 25);
        tabs.parent = Some("Grp".into());
        let mut txt = Control::new("Txt", ControlType::TextBox, 30, 30);
        txt.parent = Some("Tabs".into());
        txt.tab = Some(1);
        form.controls = vec![pnl, grp, tabs, txt];

        let path = std::env::temp_dir().join("cobolt_test_nested_012.cfrm");
        save_form(&form, &path).expect("save");
        let loaded = load_form(&path).expect("load");
        let find = |id: &str| loaded.controls.iter().find(|c| c.id == id).unwrap();
        assert_eq!(find("Grp").parent.as_deref(), Some("Pnl"));
        assert_eq!(find("Tabs").parent.as_deref(), Some("Grp"));
        assert_eq!(find("Txt").parent.as_deref(), Some("Tabs"));
        assert_eq!(find("Txt").tab, Some(1));
        assert_eq!(find("Pnl").get_prop("BorderRadius").unwrap().as_i64(), 8);
        assert!(find("Pnl").get_prop("HScroll").unwrap().as_bool());
        assert!(find("Pnl").get_prop("VScroll").unwrap().as_bool());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn roundtrip_chart_monochrome_013() {
        // Spec 013: Monochrome + MonochromeColor round-trip on a chart.
        let mut form = Form::new("F", "F", 640, 480);
        let mut ch = Control::new("Chart-1", ControlType::BarChart, 10, 10);
        ch.set_prop("Monochrome", PropValue::Bool(true));
        ch.set_prop("MonochromeColor", PropValue::String("#2E8B8B".into()));
        form.controls = vec![ch];
        let path = std::env::temp_dir().join("cobolt_test_mono_013.cfrm");
        save_form(&form, &path).expect("save");
        let loaded = load_form(&path).expect("load");
        let c = loaded.controls.iter().find(|c| c.id == "Chart-1").unwrap();
        assert!(c.get_prop("Monochrome").unwrap().as_bool());
        assert_eq!(c.get_prop("MonochromeColor").unwrap().as_str(), "#2E8B8B");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn roundtrip_corner_radius_and_legacy_alias_016() {
        // Canonical CornerRadius round-trips.
        let mut form = Form::new("F", "F", 640, 480);
        let mut tb = Control::new("TB", ControlType::TextBox, 10, 10);
        tb.set_prop("CornerRadius", PropValue::Int(16));
        form.controls = vec![tb];
        let path = std::env::temp_dir().join("cobolt_test_corner_016.cfrm");
        save_form(&form, &path).expect("save");
        let loaded = load_form(&path).expect("load");
        assert_eq!(
            loaded.controls[0]
                .get_prop("CornerRadius")
                .unwrap()
                .as_i64(),
            16
        );
        let _ = std::fs::remove_file(&path);

        // An old-format file with only the container `BorderRadius` (no
        // CornerRadius) still carries the alias key the renderer reads (spec 016).
        let xml = r#"<?xml version="1.0"?>
<Form name="F" title="F" width="640" height="480">
  <Control id="Pnl" type="Panel" x="10" y="10" w="200" h="150">
    <Property name="BorderRadius">12</Property>
  </Control>
</Form>"#;
        let l = load_form_from_str(xml).expect("load legacy");
        let pnl = l.controls.iter().find(|c| c.id == "Pnl").unwrap();
        assert_eq!(pnl.get_prop("BorderRadius").unwrap().as_i64(), 12);
        assert!(
            pnl.get_prop("CornerRadius").is_none(),
            "legacy file has no CornerRadius"
        );
    }

    #[test]
    fn roundtrip_repeating_groupbox_015() {
        // Spec 015: GroupBox visual + repeating-group metadata round-trips.
        let mut form = Form::new("F", "F", 640, 480);
        let mut g = Control::new("CustomerCard", ControlType::GroupBox, 10, 10);
        g.set_prop("HideCaption", PropValue::Bool(true));
        g.set_prop("HideBackground", PropValue::Bool(true));
        g.set_prop("BackgroundGradientEnabled", PropValue::Bool(true));
        g.set_prop(
            "BackgroundGradientDirection",
            PropValue::String("DiagonalDown".into()),
        );
        g.set_prop("IsRepeatingGroup", PropValue::Bool(true));
        g.set_prop("ArrayName", PropValue::String("CustomerCard".into()));
        g.set_prop("LayoutDirection", PropValue::String("Grid".into()));
        g.set_prop("ItemSpacing", PropValue::Int(12));
        g.set_prop("ItemsPerRow", PropValue::Int(3));
        g.set_prop("PreviewItemCount", PropValue::Int(4));
        form.controls = vec![g];
        let path = std::env::temp_dir().join("cobolt_test_repeat_015.cfrm");
        save_form(&form, &path).expect("save");
        let loaded = load_form(&path).expect("load");
        let c = loaded
            .controls
            .iter()
            .find(|c| c.id == "CustomerCard")
            .unwrap();
        assert!(c.get_prop("HideCaption").unwrap().as_bool());
        assert!(c.get_prop("HideBackground").unwrap().as_bool());
        assert!(c.get_prop("BackgroundGradientEnabled").unwrap().as_bool());
        assert_eq!(
            c.get_prop("BackgroundGradientDirection").unwrap().as_str(),
            "DiagonalDown"
        );
        assert!(c.get_prop("IsRepeatingGroup").unwrap().as_bool());
        assert_eq!(c.get_prop("LayoutDirection").unwrap().as_str(), "Grid");
        assert_eq!(c.get_prop("ItemSpacing").unwrap().as_i64(), 12);
        assert_eq!(c.get_prop("ItemsPerRow").unwrap().as_i64(), 3);
        assert_eq!(c.get_prop("PreviewItemCount").unwrap().as_i64(), 4);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn legacy_children_and_scrollable_migrate_012() {
        // An old-format file (nested <Children>, Panel Scrollable) flattens to the
        // parent-linked list and migrates Scrollable → AutoScroll.
        let xml = r#"<?xml version="1.0"?>
<Form name="F" title="F" width="640" height="480">
  <Control id="Pnl" type="Panel" x="10" y="10" w="200" h="150">
    <Property name="Scrollable">true</Property>
    <Children>
      <Control id="Inner" type="Button" x="20" y="20" w="80" h="24"></Control>
    </Children>
  </Control>
</Form>"#;
        let loaded = load_form_from_str(xml).expect("load");
        let inner = loaded
            .controls
            .iter()
            .find(|c| c.id == "Inner")
            .expect("Inner flattened");
        assert_eq!(
            inner.parent.as_deref(),
            Some("Pnl"),
            "legacy child reparented to Pnl"
        );
        let pnl = loaded.controls.iter().find(|c| c.id == "Pnl").unwrap();
        assert!(
            pnl.get_prop("HScroll")
                .map(|v| v.as_bool())
                .unwrap_or(false)
                && pnl
                    .get_prop("VScroll")
                    .map(|v| v.as_bool())
                    .unwrap_or(false),
            "Scrollable must migrate to HScroll/VScroll"
        );
    }

    #[test]
    fn theme_absent_defaults_to_none_007() {
        // A form with no theme set (the default / old .cfrm) loads as None/false,
        // so existing forms render as Liquid Glass (R9).
        let form = sample_form();
        assert_eq!(form.theme, None);
        assert!(!form.use_theme_background);
        let path = std::env::temp_dir().join("cobolt_test_theme_absent.cfrm");
        save_form(&form, &path).expect("save");
        // No theme attribute is written when unset.
        let xml = std::fs::read_to_string(&path).expect("read");
        assert!(
            !xml.contains("theme="),
            "theme attr must be omitted when unset"
        );
        let loaded = load_form(&path).expect("load");
        assert_eq!(loaded.theme, None);
        assert!(!loaded.use_theme_background);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn roundtrip_cobol_structure_005() {
        let mut form = sample_form();
        form.cobol_structure.special_names = "       DECIMAL-POINT IS COMMA.".into();
        form.cobol_structure.repository = "       FUNCTION ALL INTRINSIC.".into();
        form.cobol_structure.file_control = "           SELECT F ASSIGN TO \"f.dat\".".into();
        form.cobol_structure.file_section = "       FD  F.\n       01 F-REC PIC X(80).".into();
        form.user_procedures = vec![UserProcedure {
            name: "RECALC-TOTAL".into(),
            code: "       PROCEDURE DIVISION.\n           ADD 1 TO WS-COUNTER.".into(),
        }];

        let path = std::env::temp_dir().join("cobolt_test_struct005.cfrm");
        save_form(&form, &path).expect("save_form failed");
        let loaded = load_form(&path).expect("load_form failed");

        assert_eq!(loaded.cobol_structure, form.cobol_structure);
        assert_eq!(loaded.user_procedures.len(), 1);
        assert_eq!(loaded.user_procedures[0].name, "RECALC-TOTAL");
        assert!(loaded.user_procedures[0]
            .code
            .contains("ADD 1 TO WS-COUNTER"));
        // Existing fields still survive alongside the new ones.
        assert!(loaded.user_ws_source.contains("WS-COUNTER"));
        assert_eq!(loaded.controls.len(), 1);

        let _ = std::fs::remove_file(&path);
    }

    /// A procedure the developer has only just created has **no body yet**.
    /// It must survive a save and reload, or it disappears from the form the
    /// moment they press Save — before they ever get to write it.
    #[test]
    fn a_procedure_with_no_body_yet_survives_a_save_and_reload() {
        let mut form = sample_form();
        form.user_procedures = vec![UserProcedure {
            name: "USER-PROC-1".into(),
            code: String::new(),
        }];

        let path = std::env::temp_dir().join("cobolt_test_empty_proc.cfrm");
        save_form(&form, &path).expect("save_form failed");
        let loaded = load_form(&path).expect("load_form failed");

        assert_eq!(
            loaded.user_procedures.len(),
            1,
            "a body-less procedure was dropped by the save/load round trip"
        );
        assert_eq!(loaded.user_procedures[0].name, "USER-PROC-1");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn xml_output_contains_expected_tags() {
        let form = sample_form();
        let dir = std::env::temp_dir();
        let path: PathBuf = dir.join("cobolt_test_tags.cfrm");
        save_form(&form, &path).expect("save_form failed");

        let xml = std::fs::read_to_string(&path).expect("read file");
        assert!(xml.contains(r#"name="MAIN-FORM""#));
        assert!(xml.contains(r#"<Control id="BTN-OK""#));
        assert!(xml.contains(r#"<Property name="Caption">OK</Property>"#));
        assert!(xml.contains(r#"<Event name="onClick" paragraph="BTN-OK--CLICK">"#));
        assert!(xml.contains("WS-COUNTER"));
        assert!(xml.contains("<working-storage>"));
        assert!(xml.contains("<form-events>"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn legacy_localws_is_migrated_into_full_source() {
        // A legacy .cfrm: <LocalWS> child + bare PROCEDURE statements in CDATA.
        let legacy = r#"<?xml version="1.0"?>
<Form name="F" title="T" width="100" height="100">
  <controls>
    <Control id="BTN" type="Button" x="0" y="0" w="10" h="10">
      <events>
        <Event name="onClick" paragraph="BTN--ONCLICK">
          <LocalWS><![CDATA[       01 WS-FLAG PIC 9 VALUE 0.]]></LocalWS>
          <![CDATA[           MOVE 1 TO WS-FLAG.]]>
        </Event>
      </events>
    </Control>
  </controls>
</Form>"#;
        let dir = std::env::temp_dir();
        let path: PathBuf = dir.join("cobolt_test_legacy.cfrm");
        std::fs::write(&path, legacy).unwrap();

        let loaded = load_form(&path).expect("load legacy form");
        let ev = &loaded.controls[0].events[0];
        // Migrated into a single full-source handler body.
        assert!(ev.code.contains("WORKING-STORAGE SECTION."));
        assert!(ev.code.contains("WS-FLAG"));
        assert!(ev.code.contains("PROCEDURE DIVISION."));
        assert!(ev.code.contains("MOVE 1 TO WS-FLAG"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn handler_template_has_skeleton() {
        let t = crate::model::event_handler_template("onClick");
        assert!(t.contains("ENVIRONMENT DIVISION."));
        assert!(t.contains("WORKING-STORAGE SECTION."));
        assert!(t.contains("LINKAGE SECTION."));
        assert!(t.contains("PROCEDURE DIVISION."));
        // No event carries data yet → no USING clause.
        assert!(!t.contains("USING"));
    }
}
