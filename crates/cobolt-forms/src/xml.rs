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
    Reader, Writer,
    events::{BytesCData, BytesDecl, BytesEnd, BytesStart, BytesText, Event},
};
use thiserror::Error;

use crate::model::{
    AnimationDef, AnimKind, AnimRepeat, AnimTrigger, BgImageMode, Control, ControlType,
    DeletedControlCode, EasingKind, EventBinding, Form, PropValue, UserProcedure,
    derive_paragraph_name,
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

#[allow(dead_code)]
fn get_attr_i32(e: &BytesStart, key: &[u8], default: i32) -> Result<i32, FormError> {
    Ok(get_attr(e, key)?.and_then(|v| v.parse().ok()).unwrap_or(default))
}
fn get_attr_u32(e: &BytesStart, key: &[u8], default: u32) -> Result<u32, FormError> {
    Ok(get_attr(e, key)?.and_then(|v| v.parse().ok()).unwrap_or(default))
}
#[allow(dead_code)]
fn get_attr_bool(e: &BytesStart, key: &[u8], default: bool) -> Result<bool, FormError> {
    Ok(get_attr(e, key)?.map(|v| v != "false" && v != "0").unwrap_or(default))
}

// ── Owned event abstraction ───────────────────────────────────────────────────
//
// quick-xml events borrow from an internal buffer.  To allow recursive calls
// that re-use the same buffer we convert each event to a fully-owned value
// before acting on it.

type AttrPairs = Vec<(Vec<u8>, String)>; // (key-bytes, value-string)

enum OwnedEvent {
    FormStart {
        name:             String,
        title:            String,
        width:            u32,
        height:           u32,
        background:       String,
        transparency:     u8,
        background_image: String,
        bg_image_mode:    BgImageMode,
        grid_size:        u8,
        snap_to_grid:     bool,
        target:           String,
        theme:            Option<String>,
        use_theme_background: bool,
        glass_style: crate::model::GlassStyle,
    },
    ControlStart(AttrPairs),
    PropertyStart(String),              // property name
    ChildrenStart,
    WorkingStorageStart,                // <working-storage>
    SpecialNamesStart,                  // <special-names>   (spec 005)
    RepositoryStart,                    // <repository>      (spec 005)
    FileControlStart,                   // <file-control>    (spec 005)
    FileSectionStart,                   // <file-section>    (spec 005)
    UserProceduresStart,                // <user-procedures> (spec 005)
    FormEventsStart,                    // <form-events>
    DeletedControlsStart,               // <deleted-controls>
    DeletedControlStart(String, String),// (control_id, deleted_at) from <DeletedControl>
    /// <Event> as a *start* tag — body (CDATA or text) follows as Text/CData events.
    EventStart(String, String),         // (event_name, paragraph)
    AnimationEmpty(AttrPairs),
    /// Generic start tag not matched by any specific variant above.
    StartTag(Vec<u8>),                  // tag local name bytes
    Text(String),
    CData(String),
    EndTag(Vec<u8>),                    // tag local name bytes
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
                    let name             = get_attr_str(e, b"name")?;
                    let title            = get_attr_str(e, b"title")?;
                    let width            = get_attr_u32(e,  b"width",  800)?;
                    let height           = get_attr_u32(e,  b"height", 600)?;
                    let background       = get_attr(e, b"background")?
                                              .unwrap_or_else(|| "#FFFFFF".into());
                    let transparency     = get_attr(e, b"transparency")?
                                              .and_then(|v| v.parse::<u8>().ok())
                                              .unwrap_or(0);
                    let background_image = get_attr_str(e, b"background-image")?;
                    let bg_image_mode    = BgImageMode::from_str(
                        &get_attr_str(e, b"bg-image-mode")?);
                    let grid_size        = get_attr(e, b"grid-size")?
                                              .and_then(|v| v.parse::<u8>().ok())
                                              .unwrap_or(8);
                    let snap_to_grid     = get_attr(e, b"snap-to-grid")?
                                              .map(|v| v != "false" && v != "0")
                                              .unwrap_or(true);
                    let target           = get_attr_str(e, b"target")
                                              .unwrap_or_else(|_| "Custom".to_owned());
                    let theme            = get_attr(e, b"theme")?
                                              .map(|s| s.trim().to_owned())
                                              .filter(|s| !s.is_empty());
                    let use_theme_background = get_attr(e, b"use-theme-background")?
                                              .map(|v| v == "true" || v == "1")
                                              .unwrap_or(false);
                    let glass_style = get_attr(e, b"glass-style")?
                                              .map(|v| crate::model::GlassStyle::from_str(&v))
                                              .unwrap_or_default();
                    Ok(OwnedEvent::FormStart {
                        name, title, width, height, background,
                        transparency, background_image, bg_image_mode,
                        grid_size, snap_to_grid, target,
                        theme, use_theme_background, glass_style,
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
                b"Children"         => Ok(OwnedEvent::ChildrenStart),
                b"working-storage"  => Ok(OwnedEvent::WorkingStorageStart),
                b"special-names"    => Ok(OwnedEvent::SpecialNamesStart),
                b"repository"       => Ok(OwnedEvent::RepositoryStart),
                b"file-control"     => Ok(OwnedEvent::FileControlStart),
                b"file-section"     => Ok(OwnedEvent::FileSectionStart),
                b"user-procedures"  => Ok(OwnedEvent::UserProceduresStart),
                b"form-events"      => Ok(OwnedEvent::FormEventsStart),
                b"deleted-controls" => Ok(OwnedEvent::DeletedControlsStart),
                b"DeletedControl"   => {
                    let id         = get_attr_str(e, b"id")?;
                    let deleted_at = get_attr_str(e, b"deleted-at")?;
                    Ok(OwnedEvent::DeletedControlStart(id, deleted_at))
                }
                // <Event> as a start tag (v1.0 — has CDATA body)
                b"Event" => {
                    let ev_name   = get_attr_str(e, b"name")?;
                    let paragraph = get_attr_str(e, b"paragraph")?;
                    Ok(OwnedEvent::EventStart(ev_name, paragraph))
                }
                // Generic start tag — returned as StartTag so specialised parsers
                // (e.g. collect_event_body) can match on it.
                other => Ok(OwnedEvent::StartTag(other.to_vec())),
            }
        }

        // ── Empty / self-closing tags ─────────────────────────────────────────
        Event::Empty(e) => {
            match e.local_name().as_ref() {
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
            }
        }

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
    let mut buf  = Vec::new();
    let mut form: Option<Form> = None;

    loop {
        match next_owned(reader, &mut buf)? {
            OwnedEvent::FormStart {
                name, title, width, height, background,
                transparency, background_image, bg_image_mode,
                grid_size, snap_to_grid, target,
                theme, use_theme_background, glass_style,
            } => {
                // Build a base Form using Form::new (populates default form_events)
                let mut f = Form::new(&name, &title, width, height);
                f.background_color = background;
                f.transparency     = transparency;
                f.background_image = background_image;
                f.bg_image_mode    = bg_image_mode;
                f.grid_size        = grid_size;
                f.snap_to_grid     = snap_to_grid;
                f.target           = target;
                f.theme            = theme;
                f.use_theme_background = use_theme_background;
                f.glass_style      = glass_style;
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
    form.seed_repository_if_empty();
    // Containers (spec 012): flatten any legacy <Children> nesting into the flat
    // editing list with `parent` links, and migrate the old Panel `Scrollable`
    // flag to the unified `AutoScroll` property.
    normalize_containers(&mut form);
    seed_missing_props(&mut form);
    Ok(form)
}

/// Seed properties that were added after a control was first created, so that
/// existing .cfrm files gain the new UI without a manual re-create.
fn seed_missing_props(form: &mut Form) {
    use crate::model::{ControlType, PropValue};
    for c in &mut form.controls {
        match c.control_type {
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
            _ => {}
        }
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
        if c.is_container() && c.get_prop("AutoScroll").is_none() {
            if let Some(scroll) = c.get_prop("Scrollable").map(|v| v.as_bool()) {
                c.set_prop("AutoScroll", PropValue::Bool(scroll));
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
    buf:    &mut Vec<u8>,
    form:   &mut Form,
) -> Result<(), FormError> {
    loop {
        match next_owned(reader, buf)? {
            // ── Controls ──────────────────────────────────────────────────────
            OwnedEvent::ControlStart(attrs) => {
                form.controls.push(parse_control(reader, buf, attrs)?);
            }

            // ── <working-storage> ─────────────────────────────────────────────
            OwnedEvent::WorkingStorageStart => {
                form.user_ws_source = collect_cdata_body(reader, buf, b"working-storage")?;
            }

            // ── COBOL structure blocks (spec 005) ─────────────────────────────
            OwnedEvent::SpecialNamesStart => {
                form.cobol_structure.special_names = collect_cdata_body(reader, buf, b"special-names")?;
            }
            OwnedEvent::RepositoryStart => {
                form.cobol_structure.repository = collect_cdata_body(reader, buf, b"repository")?;
            }
            OwnedEvent::FileControlStart => {
                form.cobol_structure.file_control = collect_cdata_body(reader, buf, b"file-control")?;
            }
            OwnedEvent::FileSectionStart => {
                form.cobol_structure.file_section = collect_cdata_body(reader, buf, b"file-section")?;
            }

            // ── <user-procedures> (spec 005) — reuse the Event-list machinery ──
            OwnedEvent::UserProceduresStart => {
                form.user_procedures = parse_event_list(reader, buf, b"user-procedures")?
                    .into_iter()
                    .map(|e| UserProcedure { name: e.event, code: e.code })
                    .collect();
            }

            // ── <form-events> ─────────────────────────────────────────────────
            OwnedEvent::FormEventsStart => {
                form.form_events = parse_event_list(reader, buf, b"form-events")?;
                // Ensure onLoad / onClose stubs exist even if file omits them.
                for ev_name in &["onLoad", "onClose"] {
                    if !form.form_events.iter().any(|e| e.event == *ev_name) {
                        form.form_events.push(EventBinding {
                            event:     ev_name.to_string(),
                            paragraph: derive_paragraph_name(&form.name, ev_name),
                            code:      String::new(),
                        });
                    }
                }
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
    reader:  &mut Reader<R>,
    buf:     &mut Vec<u8>,
    end_tag: &[u8],
) -> Result<String, FormError> {
    let mut body = String::new();
    loop {
        match next_owned(reader, buf)? {
            OwnedEvent::Text(t)  => body.push_str(&t),
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
    reader:  &mut Reader<R>,
    buf:     &mut Vec<u8>,
    end_tag: &[u8],
) -> Result<Vec<EventBinding>, FormError> {
    let mut events = Vec::new();
    loop {
        match next_owned(reader, buf)? {
            OwnedEvent::EventStart(ev_name, paragraph) => {
                let (code, local_ws) = collect_event_body(reader, buf)?;
                if !ev_name.is_empty() {
                    let code = migrate_handler_source(code, local_ws);
                    events.push(EventBinding { event: ev_name, paragraph, code });
                }
            }
            OwnedEvent::EndTag(tag) if tag.as_slice() == end_tag => break,
            OwnedEvent::Eof => break,
            _ => {}
        }
    }
    Ok(events)
}

/// Read everything inside an `<Event>...</Event>` block and return
/// `(procedure_body_code, local_ws)`.
///
/// Handles both the legacy bare-CDATA format and the new format that includes
/// a `<LocalWS>` child element.
fn collect_event_body<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    buf:    &mut Vec<u8>,
) -> Result<(String, String), FormError> {
    let mut code     = String::new();
    let mut local_ws = String::new();
    loop {
        match next_owned(reader, buf)? {
            // <LocalWS> child — read its CDATA content
            OwnedEvent::StartTag(tag) if tag.as_slice() == b"LocalWS" => {
                local_ws = collect_cdata_body(reader, buf, b"LocalWS")?;
            }
            // Top-level text / CDATA = procedure body
            OwnedEvent::Text(t)  => code.push_str(&t),
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
    buf:    &mut Vec<u8>,
) -> Result<Vec<DeletedControlCode>, FormError> {
    let mut deleted = Vec::new();
    loop {
        match next_owned(reader, buf)? {
            OwnedEvent::DeletedControlStart(control_id, deleted_at) => {
                let events = parse_event_list(reader, buf, b"DeletedControl")?;
                deleted.push(DeletedControlCode { control_id, deleted_at, events });
            }
            OwnedEvent::EndTag(tag) if tag.as_slice() == b"deleted-controls" => break,
            OwnedEvent::Eof => break,
            _ => {}
        }
    }
    Ok(deleted)
}

fn parse_control_list<R: std::io::BufRead>(
    reader:  &mut Reader<R>,
    buf:     &mut Vec<u8>,
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
    buf:    &mut Vec<u8>,
    attrs:  AttrPairs,
) -> Result<Control, FormError> {
    // ── Decode attributes ────────────────────────────────────────────────────
    let mut id          = String::new();
    let mut type_str    = String::new();
    let mut x           = 0i32;
    let mut y           = 0i32;
    let mut w: Option<i32> = None;
    let mut h: Option<i32> = None;
    let mut tab_order   = 0u32;
    let mut z_order     = 0i32;
    let mut visible     = true;
    let mut enabled     = true;
    let mut parent: Option<String> = None;
    let mut tab:    Option<u32>    = None;

    for (key, val) in attrs {
        match key.as_slice() {
            b"id"        => id        = val,
            b"type"      => type_str  = val,
            b"x"         => x         = val.parse().unwrap_or(0),
            b"y"         => y         = val.parse().unwrap_or(0),
            b"w"         => w         = val.parse().ok(),
            b"h"         => h         = val.parse().ok(),
            b"tab-order" => tab_order = val.parse().unwrap_or(0),
            b"z-order"   => z_order   = val.parse().unwrap_or(0),
            b"visible"   => visible   = val != "false" && val != "0",
            b"enabled"   => enabled   = val != "false" && val != "0",
            // Container membership (spec 012). Empty `parent` = direct form child.
            b"parent"    => parent    = if val.is_empty() { None } else { Some(val) },
            b"tab"       => tab       = val.parse().ok(),
            _ => {}
        }
    }

    let control_type = ControlType::from_str(&type_str);
    let mut ctrl = Control::new(id, control_type, x, y);
    if let Some(wv) = w { ctrl.rect.w = wv; }
    if let Some(hv) = h { ctrl.rect.h = hv; }
    ctrl.tab_order = tab_order;
    ctrl.z_order   = z_order;
    ctrl.visible   = visible;
    ctrl.enabled   = enabled;
    ctrl.parent    = parent;
    ctrl.tab       = tab;
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
                    ctrl.properties.insert(pname.clone(), parse_prop_value(&text));
                }
            }
            OwnedEvent::CData(text) => {
                // CDATA inside a <Property> (unlikely but handle gracefully)
                if let Some(ref pname) = current_prop {
                    ctrl.properties.insert(pname.clone(), parse_prop_value(&text));
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
                    ctrl.events.push(EventBinding { event: ev_name, paragraph, code });
                }
            }
            OwnedEvent::AnimationEmpty(attrs) => {
                let mut name       = String::new();
                let mut trigger    = "OnLoad".to_owned();
                let mut kind       = "FadeIn".to_owned();
                let mut duration   = 400u64;
                let mut delay      = 0u64;
                let mut easing     = "EaseInOut".to_owned();
                let mut repeat     = "Once".to_owned();
                let mut slide_dx   = 0i32;
                let mut slide_dy   = 0i32;
                for (key, val) in attrs {
                    match key.as_slice() {
                        b"name"       => name      = val,
                        b"trigger"    => trigger   = val,
                        b"kind"       => kind      = val,
                        b"duration"   => duration  = val.parse().unwrap_or(400),
                        b"delay"      => delay     = val.parse().unwrap_or(0),
                        b"easing"     => easing    = val,
                        b"repeat"     => repeat    = val,
                        b"slide-dx"   => slide_dx  = val.parse().unwrap_or(0),
                        b"slide-dy"   => slide_dy  = val.parse().unwrap_or(0),
                        _ => {}
                    }
                }
                if !name.is_empty() {
                    let mut anim = AnimationDef::new(&name);
                    anim.trigger     = AnimTrigger::from_str(&trigger);
                    anim.kind        = AnimKind::from_str(&kind);
                    anim.duration_ms = duration;
                    anim.delay_ms    = delay;
                    anim.easing      = EasingKind::from_str(&easing);
                    anim.repeat      = match repeat.as_str() {
                        "Loop"     => AnimRepeat::Loop,
                        "PingPong" => AnimRepeat::PingPong,
                        "Count"    => AnimRepeat::Count(3),
                        _          => AnimRepeat::Once,
                    };
                    anim.slide_dx    = slide_dx;
                    anim.slide_dy    = slide_dy;
                    ctrl.animations.push(anim);
                }
            }
            OwnedEvent::EndTag(tag) => {
                match tag.as_slice() {
                    b"Property" => { current_prop = None; }
                    b"Control"  => break,
                    _ => {}
                }
            }
            OwnedEvent::Eof => break,
            _ => {}
        }
    }

    Ok(ctrl)
}

fn parse_prop_value(s: &str) -> PropValue {
    let trimmed = s.trim();
    if trimmed == "true"  { return PropValue::Bool(true); }
    if trimmed == "false" { return PropValue::Bool(false); }
    if let Ok(n) = trimmed.parse::<i64>() { return PropValue::Int(n); }
    PropValue::String(trimmed.to_owned())
}

// ── Save ──────────────────────────────────────────────────────────────────────

pub fn save_form(form: &Form, path: &Path) -> Result<(), FormError> {
    let mut output = Vec::new();
    {
        let mut w = Writer::new_with_indent(&mut output, b' ', 2);

        w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

        // ── <Form ...> ────────────────────────────────────────────────────────
        let mut elem = BytesStart::new("Form");
        elem.push_attribute(("name",          form.name.as_str()));
        elem.push_attribute(("title",         form.title.as_str()));
        elem.push_attribute(("width",         form.width.to_string().as_str()));
        elem.push_attribute(("height",        form.height.to_string().as_str()));
        elem.push_attribute(("background",    form.background_color.as_str()));
        elem.push_attribute(("transparency",  form.transparency.to_string().as_str()));
        elem.push_attribute(("grid-size",     form.grid_size.to_string().as_str()));
        elem.push_attribute(("snap-to-grid",  if form.snap_to_grid { "true" } else { "false" }));
        elem.push_attribute(("target",        form.target.as_str()));
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
            elem.push_attribute(("bg-image-mode",    form.bg_image_mode.as_str()));
        }
        w.write_event(Event::Start(elem))?;

        // ── <working-storage> ─────────────────────────────────────────────────
        if !form.user_ws_source.trim().is_empty() {
            w.write_event(Event::Start(BytesStart::new("working-storage")))?;
            w.write_event(Event::CData(BytesCData::new(form.user_ws_source.as_str())))?;
            w.write_event(Event::End(BytesEnd::new("working-storage")))?;
        }

        // ── COBOL structure blocks (spec 005) ─────────────────────────────────
        for (tag, body) in [
            ("special-names", form.cobol_structure.special_names.as_str()),
            ("repository",    form.cobol_structure.repository.as_str()),
            ("file-control",  form.cobol_structure.file_control.as_str()),
            ("file-section",  form.cobol_structure.file_section.as_str()),
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
                    event:     up.name.clone(),
                    paragraph: up.name.clone(),
                    code:      up.code.clone(),
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

        // ── Controls ─────────────────────────────────────────────────────────
        for ctrl in &form.controls {
            write_control(&mut w, ctrl)?;
        }

        // ── <deleted-controls> ────────────────────────────────────────────────
        if !form.deleted_code.is_empty() {
            w.write_event(Event::Start(BytesStart::new("deleted-controls")))?;
            for dc in &form.deleted_code {
                let mut de = BytesStart::new("DeletedControl");
                de.push_attribute(("id",         dc.control_id.as_str()));
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
    fs::write(path, &output)?;
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
    w:  &mut Writer<W>,
    ev: &EventBinding,
) -> Result<(), FormError> {
    let mut ee = BytesStart::new("Event");
    ee.push_attribute(("name",      ev.event.as_str()));
    ee.push_attribute(("paragraph", ev.paragraph.as_str()));
    w.write_event(Event::Start(ee))?;
    // Write the full handler source as CDATA (single-source format).
    if !ev.code.is_empty() {
        w.write_event(Event::CData(BytesCData::new(ev.code.as_str())))?;
    }
    w.write_event(Event::End(BytesEnd::new("Event")))?;
    Ok(())
}

fn write_control<W: std::io::Write>(
    w:    &mut Writer<W>,
    ctrl: &Control,
) -> Result<(), FormError> {
    let mut elem = BytesStart::new("Control");
    elem.push_attribute(("id",        ctrl.id.as_str()));
    elem.push_attribute(("type",      ctrl.control_type.as_str()));
    elem.push_attribute(("x",         ctrl.rect.x.to_string().as_str()));
    elem.push_attribute(("y",         ctrl.rect.y.to_string().as_str()));
    elem.push_attribute(("w",         ctrl.rect.w.to_string().as_str()));
    elem.push_attribute(("h",         ctrl.rect.h.to_string().as_str()));
    elem.push_attribute(("tab-order", ctrl.tab_order.to_string().as_str()));
    elem.push_attribute(("z-order",   ctrl.z_order.to_string().as_str()));
    elem.push_attribute(("visible",   if ctrl.visible { "true" } else { "false" }));
    elem.push_attribute(("enabled",   if ctrl.enabled { "true" } else { "false" }));
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
        ae.push_attribute(("name",      anim.name.as_str()));
        ae.push_attribute(("trigger",   anim.trigger.as_str()));
        ae.push_attribute(("kind",      anim.kind.as_str()));
        ae.push_attribute(("duration",  anim.duration_ms.to_string().as_str()));
        ae.push_attribute(("delay",     anim.delay_ms.to_string().as_str()));
        ae.push_attribute(("easing",    anim.easing.as_str()));
        ae.push_attribute(("repeat",    anim.repeat.as_str()));
        ae.push_attribute(("slide-dx",  anim.slide_dx.to_string().as_str()));
        ae.push_attribute(("slide-dy",  anim.slide_dy.to_string().as_str()));
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
        PropValue::Int(n)    => n.to_string(),
        PropValue::Bool(b)   => if *b { "true" } else { "false" }.to_string(),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_form() -> Form {
        let mut form = Form::new("MAIN-FORM", "Test App", 800, 600);
        form.background_color = "#F0F0F0".into();
        form.background_image = "/tmp/wallpaper.png".into();
        form.bg_image_mode    = BgImageMode::Fit;
        // Set OnLoad code
        if let Some(ev) = form.form_events.iter_mut().find(|e| e.event == "onLoad") {
            ev.code = "    MOVE 0 TO WS-COUNTER".into();
        }
        form.user_ws_source = "       01 WS-COUNTER  PIC 9(8) VALUE 0 GLOBAL.\n".into();

        let mut btn = Control::new("BTN-OK", ControlType::Button, 10, 10);
        btn.rect.w = 80;
        btn.rect.h = 30;
        btn.properties.insert("Caption".into(), PropValue::String("OK".into()));
        btn.properties.insert("FontSize".into(), PropValue::Int(12));
        btn.events.push(EventBinding {
            event:     "onClick".into(),
            paragraph: "BTN-OK--CLICK".into(),
            code:      "    MOVE 1 TO WS-COUNTER".into(),
        });
        form.controls.push(btn);

        form
    }

    #[test]
    fn roundtrip_in_memory() {
        let form = sample_form();
        let dir  = std::env::temp_dir();
        let path: PathBuf = dir.join("cobolt_test_roundtrip.cfrm");
        save_form(&form, &path).expect("save_form failed");
        let loaded = load_form(&path).expect("load_form failed");

        assert_eq!(loaded.name,             form.name);
        assert_eq!(loaded.title,            form.title);
        assert_eq!(loaded.width,            form.width);
        assert_eq!(loaded.height,           form.height);
        assert_eq!(loaded.background_color, form.background_color);
        // Background image path + mode must survive the save/reload the inline
        // inspector performs immediately after a pick.
        assert_eq!(loaded.background_image, "/tmp/wallpaper.png");
        assert_eq!(loaded.bg_image_mode,    BgImageMode::Fit);

        // User WS preserved
        assert!(loaded.user_ws_source.contains("WS-COUNTER"));

        // Form events with code
        let on_load = loaded.form_events.iter().find(|e| e.event == "onLoad");
        assert!(on_load.is_some());
        assert!(on_load.unwrap().code.contains("WS-COUNTER"));

        // Controls
        assert_eq!(loaded.controls.len(), 1);
        let btn = &loaded.controls[0];
        assert_eq!(btn.id,             "BTN-OK");
        assert_eq!(btn.control_type,   ControlType::Button);
        assert_eq!(btn.rect.x,         10);
        assert_eq!(btn.rect.w,         80);
        assert_eq!(btn.events.len(),   1);
        assert_eq!(btn.events[0].event,     "onClick");
        assert_eq!(btn.events[0].paragraph, "BTN-OK--CLICK");
        assert!(btn.events[0].code.contains("WS-COUNTER"));

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
    fn roundtrip_nested_containers_012() {
        // A deeply nested form round-trips parent/tab links and the new container
        // props (spec 012).
        let mut form = Form::new("F", "F", 640, 480);
        let mut pnl = Control::new("Pnl", ControlType::Panel, 10, 10);
        pnl.set_prop("BorderRadius", PropValue::Int(8));
        pnl.set_prop("AutoScroll",   PropValue::Bool(true));
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
        assert!(find("Pnl").get_prop("AutoScroll").unwrap().as_bool());
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
        assert_eq!(loaded.controls[0].get_prop("CornerRadius").unwrap().as_i64(), 16);
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
        assert!(pnl.get_prop("CornerRadius").is_none(), "legacy file has no CornerRadius");
    }

    #[test]
    fn roundtrip_repeating_groupbox_015() {
        // Spec 015: GroupBox visual + repeating-group metadata round-trips.
        let mut form = Form::new("F", "F", 640, 480);
        let mut g = Control::new("CustomerCard", ControlType::GroupBox, 10, 10);
        g.set_prop("HideCaption", PropValue::Bool(true));
        g.set_prop("HideBackground", PropValue::Bool(true));
        g.set_prop("BackgroundGradientEnabled", PropValue::Bool(true));
        g.set_prop("BackgroundGradientDirection", PropValue::String("DiagonalDown".into()));
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
        let c = loaded.controls.iter().find(|c| c.id == "CustomerCard").unwrap();
        assert!(c.get_prop("HideCaption").unwrap().as_bool());
        assert!(c.get_prop("HideBackground").unwrap().as_bool());
        assert!(c.get_prop("BackgroundGradientEnabled").unwrap().as_bool());
        assert_eq!(c.get_prop("BackgroundGradientDirection").unwrap().as_str(), "DiagonalDown");
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
        let inner = loaded.controls.iter().find(|c| c.id == "Inner").expect("Inner flattened");
        assert_eq!(inner.parent.as_deref(), Some("Pnl"), "legacy child reparented to Pnl");
        let pnl = loaded.controls.iter().find(|c| c.id == "Pnl").unwrap();
        assert!(pnl.get_prop("AutoScroll").map(|v| v.as_bool()).unwrap_or(false),
            "Scrollable must migrate to AutoScroll");
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
        assert!(!xml.contains("theme="), "theme attr must be omitted when unset");
        let loaded = load_form(&path).expect("load");
        assert_eq!(loaded.theme, None);
        assert!(!loaded.use_theme_background);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn roundtrip_cobol_structure_005() {
        let mut form = sample_form();
        form.cobol_structure.special_names = "       DECIMAL-POINT IS COMMA.".into();
        form.cobol_structure.repository    = "       FUNCTION ALL INTRINSIC.".into();
        form.cobol_structure.file_control  = "           SELECT F ASSIGN TO \"f.dat\".".into();
        form.cobol_structure.file_section  = "       FD  F.\n       01 F-REC PIC X(80).".into();
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
        assert!(loaded.user_procedures[0].code.contains("ADD 1 TO WS-COUNTER"));
        // Existing fields still survive alongside the new ones.
        assert!(loaded.user_ws_source.contains("WS-COUNTER"));
        assert_eq!(loaded.controls.len(), 1);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn xml_output_contains_expected_tags() {
        let form = sample_form();
        let dir  = std::env::temp_dir();
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
