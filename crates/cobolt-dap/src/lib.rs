// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The PowerRustCOBOL Debug Adapter Protocol.**
//!
//! A wire-compatible DAP implementation: framing, message envelope, typed
//! bodies, a client, and an adapter server loop. It knows nothing about COBOL
//! beyond the shape of the extension fields — no interpreter, no egui, no
//! filesystem — which is what lets the debugger UI, the debuggee and a test all
//! link the same protocol and none of them link each other.
//!
//! ```text
//!  IDE debugger window            │            debuggee process
//!  ─────────────────────          │            ────────────────────────────
//!  DapClient ──── request ────────┼──────────► serve() ──► DapHandler
//!            ◄─── response ───────┼──────────                  │
//!            ◄─── event ──────────┼──────────  FrameWriter ◄────┘
//!                                 │
//!                       Content-Length framing
//! ```
//!
//! **Why wire-compatible and not merely DAP-shaped.** The debuggee is not
//! always ours to co-design: a compiled application, a form host, and one day a
//! runtime on another device all sit at the far end. Speaking the real protocol
//! means the boundary is specified by someone other than us — and any DAP
//! client can drive a RustCOBOL program, which is not true of a private
//! protocol however well shaped.
//!
//! **Where COBOL enters.** DAP has no word for a PICTURE, a level number, an
//! OCCURS bound or a paragraph. Those travel as extra `cobol*` fields on the
//! standard structures ([`types`]), which a stock client ignores and ours
//! reads. No standard field is ever repurposed to carry COBOL meaning.

pub mod adapter;
pub mod client;
pub mod hits;
pub mod link;
pub mod protocol;
pub mod transport;
pub mod types;

pub use adapter::{serve, DapHandler, Reply};
pub use client::{ClientUpdate, DapClient, SessionState};
pub use hits::{HitCondition, HitConditionError};
pub use link::{spawn_reader, FrameWriter, LinkEvent};
pub use protocol::{Event, ProtocolMessage, Request, Response, SeqCounter};
pub use transport::{read_frame, write_frame};
pub use types::{
    Breakpoint, Capabilities, CobolScope, CobolUnit, DataAccess, DataBreakpoint, FrameKind, Scope,
    Source, SourceBreakpoint, SpecialValue, StackFrame, StopReason, Thread, ValueView, Variable,
    PROTOCOL_VERSION,
};
