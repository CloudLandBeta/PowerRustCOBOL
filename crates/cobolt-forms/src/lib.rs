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

pub mod containers;
pub mod datagrid;
pub mod diagnostics;
pub mod icons;
pub mod menu;
pub mod model;
pub mod theme;
pub mod theme_pack;
pub mod xml;

pub use model::{
    ApprovedBindingTargetKind, BindingChartKind, BindingDataType, BindingField, BindingMode,
    BindingSourceDescriptor, BindingSourceKind, BindingSourceMetadata, BindingTargetDescriptor,
    BindingTargetPath, BindingUpdateMetadata, BindingValidationSnapshot, Control, ControlType,
    DataBindingDef, DataGridAdvanced, DataGridColumn, DataGridFilter, EventBinding, FieldMapping,
    Form, GlassStyle, GuardianFinding, GuardianSeverity, MappingCompatibility, PropValue, Rect,
    DATAGRID_ADVANCED_PROP, DATA_BINDING_SCHEMA_VERSION,
};
pub use xml::{load_form, load_form_from_str, save_form, FormError};

#[cfg(feature = "render")]
pub mod anim;

#[cfg(feature = "render")]
pub mod paint;

#[cfg(feature = "render")]
pub mod render;

#[cfg(feature = "render")]
pub mod fonts;
