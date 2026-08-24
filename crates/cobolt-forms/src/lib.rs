// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Form and control data model for the Cobolt IDE.
//!
//! A `Form` is the Cobolt equivalent of a PowerCOBOL `.pco` file — a
//! structured description of a visual screen together with its controls and
//! event bindings.  Forms are serialised to/from XML with a `.cfrm` extension.
//!
//! # Example `.cfrm` file
//!
//! ```xml
//! <Form name="MAIN-FORM" title="My App" width="800" height="600">
//!   <Control id="BTN-OK" type="Button" x="10" y="10" w="80" h="30">
//!     <Property name="Caption">OK</Property>
//!     <Event name="onClick" paragraph="BTN-OK-CLICK"/>
//!   </Control>
//! </Form>
//! ```

pub mod code_site;
pub mod containers;
pub mod datagrid;
pub mod dropzone;
pub mod diagnostics;
pub mod icons;
pub mod menu;
pub mod model;
pub mod theme;
pub mod theme_pack;
pub mod xml;

pub use model::{
    parse_map_markers, serialize_map_markers, ApprovedBindingTargetKind, BindingChartKind,
    BindingDataType, BindingField, BindingMode, BindingSourceDescriptor, BindingSourceKind,
    BindingSourceMetadata, BindingTargetDescriptor, BindingTargetPath, BindingUpdateMetadata,
    BindingValidationSnapshot, Control, ControlType, DataBindingDef, DataGridAdvanced,
    DataGridColumn, DataGridFilter, EventBinding, FieldMapping, Form, GlassStyle, GuardianFinding,
    GuardianSeverity, MapMarkerField, MapMarkerRecord, MappingCompatibility, PropValue, Rect,
    DATAGRID_ADVANCED_PROP, DATA_BINDING_SCHEMA_VERSION,
};
pub use code_site::{code_sites, resolve_display_path, site_text, CodeSite, StructureSection};
pub use xml::{form_to_string, load_form, load_form_from_str, save_form, FormError};

#[cfg(feature = "render")]
pub mod anim;

#[cfg(feature = "render")]
pub mod paint;

// What a form theme IS (spec 050): an implementation the painters ask, not an
// identity they test against. Registering a theme touches no painter, and a
// theme that owns the whole look says so instead of every painter guessing.
#[cfg(feature = "render")]
pub mod surface_theme;

#[cfg(feature = "render")]
pub mod render;

#[cfg(feature = "render")]
pub mod fonts;

// The ONE sidebar renderer (spec 049). Designer canvas, preview, Run Form and
// the shell MenuPane all draw through it, so the rail cannot look different
// depending on which surface you are looking at.
#[cfg(feature = "render")]
pub mod sidebar;

// The ONE breadcrumb renderer (spec 049) — the sidebar's sibling, and shared
// for the same reason: the strip lives in the running shell, but the designer
// and the preview have to show it too, and the IDE takes no runtime dependency
// on the form host.
#[cfg(feature = "render")]
pub mod breadcrumb;

// What a ToolBar is made of: groups of buttons, each with its own frame, and an
// action per button. Model only — no egui — so the designer's editor, the
// renderer and the running host all read one definition.
pub mod toolbar;

// The ONE toolbar renderer — the sidebar's and breadcrumb's sibling, shared for
// the same reason: a bar that looks different on the canvas than it does running
// is a bar you cannot design against.
#[cfg(feature = "render")]
pub mod toolbar_paint;

/// The TREE itself — parsing `Items` into nodes, and walking them. Deliberately
/// NOT behind `render`: the interpreter answers a handler's `NodeParent` while
/// knowing nothing about egui, and it must read the same tree the canvas draws
/// rather than a second parser that would drift from it.
pub mod treenodes;

/// The TreeView's layout and paint, shared by the canvas and the running form
/// for the same reason the toolbar's is: a tree that looks different where you
/// design it is a tree you cannot design against.
#[cfg(feature = "render")]
pub mod treeview;

/// The Splitter's geometry — where its two panes, its division line and its
/// grip sit — plus the paint the canvas and the running form share. The
/// geometry is not gated: the model pins the pane Panels from it, and the model
/// builds without `render`.
pub mod splitter;

// Carrying out a toolbar button's PLATFORM action: printing, sharing, the
// clipboard, a window capture, another process. Beside `toolbar_paint` for the
// same reason — every surface that DRAWS a toolbar must also be able to PRESS
// one, so the running host and the designer's Preview share this.
#[cfg(feature = "render")]
pub mod toolbar_actions;

// Window entrance/exit effects (spec 038). Needs egui types only, so it is
// gated with the other render modules.
#[cfg(feature = "render")]
pub mod window_fx;

// Hand-rolled OpenStreetMap tile rendering for the Maps control (spec 039).
// Route/region geometry: polyline decoding, point parsing, triangulation.
// Pure math with no egui in it, so it builds and is tested without `render`.
pub mod map_geometry;

#[cfg(feature = "render")]
pub mod map_tiles;
