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
//!     <Event name="Click" paragraph="BTN-OK-CLICK"/>
//!   </Control>
//! </Form>
//! ```

pub mod model;
pub mod xml;

pub use model::{Control, ControlType, EventBinding, Form, PropValue, Rect};
pub use xml::{load_form, load_form_from_str, save_form, FormError};
