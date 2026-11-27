// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The instances one connection keeps under `[backend] state = "per-connection"` (spec 114 T37, ABI A61).
//!
//! A connection that keeps its state has **one instance of the module per exported program it has called**, for as long as
//! the connection lasts: the program's `WORKING-STORAGE`, a file it left open and a database connection it holds are what
//! the next call finds, as for a program `CALL`ed again in one COBOL run unit. A call that is cut off, cancelled, fails with
//! a run-time error or whose caller asked it to stop ends that program's instance, and its next call starts fresh (the
//! interpreter's engine does the same: `cobolt_server::exec::Session`). Two connections never share an instance.
//!
//! **Where the instances live.** A wasmtime store holds the provider state of the module's imports, which is not `Send`
//! (it owns the open files and the data connections through `Rc`s), and an instance that never changes thread needs no
//! argument about when it may. So a session owns **one worker thread**, started at its first call, and every instance of
//! the connection is created, called and dropped on that thread; a call hands the worker its request and waits for the
//! answer. The server runs a connection's calls in turn (it holds the session's lock), so the worker is never asked for two
//! at once. When the session ends its worker's queue closes and the thread drops the instances and returns.

use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;

use cobolt_runner::backend::{BackendModule, CallControl, Called, Resident};
use cobolt_runner::verbs::CallHost;

/// What the worker is asked.
enum Job {
    /// Run a call on the instance kept for `program`, making the instance first when there is none.
    Call(Call),
    /// End the instance kept for `program`: its next call starts afresh.
    Forget(String),
}

struct Call {
    program: String,
    args: Vec<String>,
    control: CallControl,
    host: Box<dyn CallHost>,
    reply: mpsc::SyncSender<Result<Called, String>>,
}

/// The worker thread of one session and the queue that feeds it.
pub(crate) struct Worker {
    jobs: Option<Sender<Job>>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    /// Start the worker of a session on `module`.
    pub(crate) fn start(module: Arc<BackendModule>) -> Result<Worker, String> {
        let (jobs, queue) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("prc-session".into())
            .spawn(move || serve(&module, queue))
            .map_err(|e| format!("cannot start the session's worker: {e}"))?;
        Ok(Worker { jobs: Some(jobs), thread: Some(thread) })
    }

    /// Run one call on the instance kept for `program` and wait for it. `Err` is the runner's own failure (the worker is gone,
    /// or the module could not be started), not the program's.
    pub(crate) fn call(&self, program: &str, args: &[&str], control: &CallControl, host: Box<dyn CallHost>) -> Result<Called, String> {
        let (reply, answer) = mpsc::sync_channel(1);
        let job = Job::Call(Call { program: program.to_owned(), args: args.iter().map(|a| (*a).to_owned()).collect(), control: control.clone(), host, reply });
        self.jobs.as_ref().ok_or("the session has ended")?.send(job).map_err(|_| "the session's worker has stopped".to_string())?;
        answer.recv().map_err(|_| "the session's worker stopped while the call was running".to_string())?
    }

    /// End the instance kept for `program`.
    pub(crate) fn forget(&self, program: &str) {
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(Job::Forget(program.to_owned()));
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Closing the queue ends the worker's loop, which drops its instances.
        self.jobs.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The worker's loop: the instances of the session, by program, and the jobs in the order they were sent.
fn serve(module: &BackendModule, queue: Receiver<Job>) {
    let mut kept: HashMap<String, Resident> = HashMap::new();
    for job in queue {
        match job {
            Job::Forget(program) => {
                kept.remove(&program);
            }
            Job::Call(call) => {
                let Call { program, args, control, host, reply } = call;
                let texts: Vec<&str> = args.iter().map(String::as_str).collect();
                // A panic of the host's own code ends the instance, not the thread: the next call starts afresh.
                let ran = std::panic::catch_unwind(AssertUnwindSafe(|| -> Result<Called, String> {
                    if !kept.contains_key(&program) {
                        kept.insert(program.clone(), module.resident()?);
                    }
                    let resident = kept.get_mut(&program).expect("the instance was just made");
                    module.call_resident(resident, &program, &texts, &control, Some(host))
                }));
                let result = match ran {
                    Ok(r) => r,
                    Err(_) => {
                        kept.remove(&program);
                        Err("the host panicked while the call was running".to_string())
                    }
                };
                let _ = reply.send(result);
            }
        }
    }
}
