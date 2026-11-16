// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **What runs a form's program** (DESKTOP-WASM-001 T29; R9; C5).
//!
//! The form host speaks to a form's program over one set of channels, whatever runs it: events
//! in ([`FormEvent`]), property changes and `DISPLAY` lines out ([`StateUpdate`], text), the
//! values the user edits in, closed handles in, and requests to the host (open a form, drive a
//! handle) out. A window therefore behaves the same whichever engine runs its handlers.
//!
//! * **[`InterpreterEngine`]** is today's engine: the program's syntax tree run by the
//!   interpreter, on a thread of its own per form instance. Every glue uses it unless it says
//!   otherwise (`rcrun run-form`, a program built on the Cargo path, the IDE).
//! * **A [`ProgramEngine`]** a glue gives through [`crate::HostHooks::program_engine`] replaces it
//!   for the forms the host opens: the desktop runner's, which runs the forms of a WebAssembly
//!   module (T30).
//!
//! An engine answers a form's design ([`ProgramEngine::form`]) and opens an instance of it
//! ([`ProgramEngine::open`]): the design, and the start of the instance's program, which the host
//! calls once it has made the instance's channels ([`ProgramRun`]).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use cobolt_runtime::form_host::FormRequest;
use cobolt_runtime::{FormEvent, StateUpdate};

use crate::debug_link::DebugWiring;

/// Everything a form instance's program is given by the host that opened it.
pub struct ProgramRun {
    /// The instance's handle in the host's supervisor.
    pub handle: String,
    /// The form's name in capitals: the object its handlers' `SELF` names.
    pub form_object: String,
    /// How the form spells its control ids, so an event the program queues itself is dispatched
    /// under the literal its handlers compare against.
    pub control_ids: Vec<String>,
    /// Events from the window.
    pub events: mpsc::Receiver<FormEvent>,
    /// Property changes to the window.
    pub state: mpsc::Sender<StateUpdate>,
    /// `DISPLAY` lines, and a run-time error's report, to the window.
    pub display: mpsc::Sender<String>,
    /// The values the user changes in the window (a slider dragged, a text typed).
    pub input: mpsc::Receiver<StateUpdate>,
    /// Handles that were closed, so a handle the program holds becomes NULL (037 R24).
    pub closed: mpsc::Receiver<String>,
    /// Requests to the host: open a form, drive a handle (051).
    pub requests: mpsc::Sender<FormRequest>,
    /// Events queued for the program and not yet taken.
    pub pending: Arc<AtomicUsize>,
    /// Set when the program has ended.
    pub finished: Arc<AtomicBool>,
    /// The handle of the form that opened this one, which `super::` names (049 R28).
    pub super_handle: Option<String>,
    /// The form's objects, seeded before the program starts: `(control, kind, properties)`.
    pub seed: Vec<(String, String, Vec<(String, String)>)>,
    /// The SideMenus' designed rows, which the program may not edit (spec 066).
    pub designed_menus: Vec<(String, cobolt_forms::menu::MenuDefinition)>,
    /// The debugger's channels when this process is being debugged (061).
    pub debug: Option<DebugWiring>,
}

/// The start of one form instance's program, called once with the instance's channels.
pub type ProgramStart = Box<dyn FnOnce(ProgramRun) -> Result<(), String> + Send>;

/// What runs the programs of the forms a host opens.
pub trait ProgramEngine: Send {
    /// The design of the form `form_id`.
    fn form(&self, form_id: &str) -> Result<cobolt_forms::Form, String>;
    /// The design of the form `form_id`, and the start of a new instance's program.
    fn open(&self, form_id: &str) -> Result<(cobolt_forms::Form, ProgramStart), String>;
}

/// A per-interpreter setup every form's interpreter is given before it runs: the glue joins the
/// main form's SQL run unit there, and a built program registers its `EXEC RUST` blocks.
pub type InterpreterSetup = Arc<dyn Fn(&mut cobolt_runtime::interpreter::Interpreter) + Send + Sync>;

/// The interpreter's object bridge, one per process (051 Q1).
pub type SharedBridge = Arc<Mutex<cobolt_runtime::rust_bridge::RustBridge>>;

/// Today's engine: each form instance's program run by the interpreter on a thread of its own.
#[derive(Clone)]
pub struct InterpreterEngine {
    /// Run on each interpreter before it starts.
    pub setup: Option<InterpreterSetup>,
    /// The indexed-file engine new files are created with: the root form's, so a file created
    /// from a child form has the same format.
    pub indexed_engine: cobolt_runtime::indexed::IndexedEngine,
    /// The process's one object bridge, adopted by every interpreter.
    pub bridge: Option<SharedBridge>,
}

impl InterpreterEngine {
    /// The start of a form instance's program: `program` run by an interpreter on a program
    /// thread (`stack_guard`), **running**, never paused at its first statement, also under a
    /// debugger: this is not the form the developer pressed Debug on, so it stops only where
    /// they put a breakpoint.
    pub fn start(&self, program: cobolt_ast::program::Program) -> ProgramStart {
        let engine = self.clone();
        Box::new(move |run: ProgramRun| {
            let ProgramRun {
                handle,
                form_object,
                control_ids,
                events,
                state,
                display,
                input,
                closed,
                requests,
                pending,
                finished,
                super_handle,
                seed,
                designed_menus,
                debug,
            } = run;
            let err_tx = display.clone();
            cobolt_runtime::stack_guard::spawn_program_thread("form", move || {
                // 051 Q1 — one object bridge per process, adopted from construction: seeding
                // this form's own EXEC RUST objects into the shared bridge directly (rather than
                // building with a private one and swapping it in after) is what keeps their
                // handles valid — see `new_with_channels_and_bridge`'s doc.
                let mut interp = cobolt_runtime::interpreter::Interpreter::new_with_channels_and_bridge(
                    program,
                    events,
                    state,
                    display,
                    engine.bridge,
                );
                interp.set_control_ids(control_ids);
                // The parent's engine: a child form creates files in the same format as the
                // form that opened it.
                interp.set_indexed_engine(engine.indexed_engine);
                interp.set_input_channel(input);
                interp.set_event_counter(pending);
                interp.set_form_host(requests, &handle, &form_object, closed);
                if let Some(sh) = &super_handle {
                    interp.set_super_form(sh);
                }
                if let Some((cmd_rx, ev_tx, bps, scope)) = debug {
                    interp.attach_debug_channels_running(cmd_rx, ev_tx, bps);
                    interp.set_debug_user_scope(scope);
                }
                if let Some(setup) = engine.setup {
                    setup(&mut interp);
                }
                interp.seed_objects(seed);
                for (id, def) in &designed_menus {
                    interp.set_designed_menu(id, def);
                }
                match interp.run() {
                    Ok(()) => {}
                    Err(e) if e.is_exit_signal() => {}
                    Err(e) => {
                        eprintln!("Runtime error: {e}");
                        let _ = err_tx.send(format!("Runtime error: {e}"));
                    }
                }
                finished.store(true, Ordering::Relaxed);
            })
            .expect("cannot start the form's program thread");
            Ok(())
        })
    }
}
