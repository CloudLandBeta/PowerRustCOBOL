// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Indexed-file definition model for the PowerRustCOBOL IDE (`.cidx` XML files).

pub mod control_defaults;
pub mod model;
pub mod paths;
pub mod pic;
pub mod raw_text;
pub mod schema_support;
pub mod structure;
pub mod xml;

pub use control_defaults::{apply_default_controls, default_control_for_field};
pub use model::{
    AccessMode, FieldUsage, IndexedDefinition, IndexedField, KeyDef, KeyEncodingDef,
    KeyOrderingDef, KeyPartDef, KeySchema, RecordFormatDef, StorageMode,
};
pub use paths::{resolve_path, store_path};
pub use pic::{
    encode_field_display, encode_indicator_bool, format_field_display, indicator_bool, parse_pic,
    FieldEncodeError, ParsedPic, PicCategory,
};
pub use raw_text::{fix_record_text, record_to_text, text_to_record};
pub use schema_support::{finalize_warnings, structural_fingerprint};
pub use structure::{
    apply_flat, flatten_record, indent_entry, level_from_depth, outdent_allowed, outdent_entry,
    rebuild_record, validate_definition, validate_flat_indent, FlatEntry,
};
pub use xml::{
    load_indexed, load_indexed_from_str, save_indexed, save_indexed_to_string, IndexedError,
};
