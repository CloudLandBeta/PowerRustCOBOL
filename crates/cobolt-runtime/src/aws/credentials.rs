// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! AWS keys typed into the running application (spec 092, amendment A6 to
//! spec 078 R11).
//!
//! A program hands a connection an access key id, a secret access key and an
//! optional session token with `COBOL::"AWS-SET-CREDENTIALS"`. They are kept
//! here, in this process's memory only, per connection, and leave by one road:
//! the environment of an AWS server started (or restarted) for that
//! connection, which [`super::process`] builds — with no profile, since the
//! keys sign instead. They are never a property, never written to a file or a
//! log, never shown in a message, and no call returns them:
//! `COBOL::"AWS-CREDENTIALS-SET"` answers only whether there are some.
//!
//! Setting or clearing a connection's keys ends every server it has running,
//! so the process holding the old keys in its environment goes at once and the
//! next call starts a server with what is set now.
//!
//! The key store (`key_store.rs`) is deliberately not used: it keeps what it
//! is given on disk.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use super::connections::{self, AwsConnection};
use super::pool;
use super::process::{wipe, Credentials};

/// The keys each connection holds, by connection id.
fn store() -> &'static Mutex<HashMap<String, Arc<Credentials>>> {
    static S: OnceLock<Mutex<HashMap<String, Arc<Credentials>>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

fn held() -> std::sync::MutexGuard<'static, HashMap<String, Arc<Credentials>>> {
    store().lock().unwrap_or_else(|e| e.into_inner())
}

/// The connection `name` names — by name or id, as a control's `Connection`
/// does — or why there is none.
fn resolve(name: &str) -> Result<AwsConnection, String> {
    connections::find(name).ok_or_else(|| {
        let known = connections::names();
        if name.trim().is_empty() {
            format!("no AWS connection was named, and the project has {} — name one", known.len())
        } else {
            format!("no AWS connection named \"{}\" (the project has: {})", name.trim(), known.join(", "))
        }
    })
}

/// Why these values cannot be keys, when they cannot. Reads the caller's
/// text, before anything is kept; never quotes it.
fn refusal(key_id: &str, secret: &str, token: &str) -> Option<&'static str> {
    if key_id.trim().is_empty() || secret.trim().is_empty() {
        return Some("an access key id and a secret access key are both needed: nothing was stored");
    }
    let odd = |t: &str| t.trim().chars().any(|c| c.is_whitespace() || c.is_control());
    if odd(key_id) || odd(secret) || odd(token) {
        return Some("an AWS key never holds a space or a control character: nothing was stored");
    }
    None
}

/// Keep `key_id`, `secret` and `token` (empty for none) for the connection
/// `connection` names, replacing what it held, and end its running servers.
/// The texts handed in are overwritten whatever happens.
pub fn set(connection: &str, mut key_id: String, mut secret: String, mut token: String) -> Result<(), String> {
    let checked = resolve(connection).and_then(|conn| match refusal(&key_id, &secret, &token) {
        Some(why) => Err(why.to_owned()),
        None => Ok(conn),
    });
    let conn = match checked {
        Ok(conn) => conn,
        Err(why) => {
            wipe(&mut key_id);
            wipe(&mut secret);
            wipe(&mut token);
            return Err(why);
        }
    };
    let keys = Arc::new(Credentials::new(key_id, secret, token));
    held().insert(conn.id.clone(), keys);
    pool::stop_connection(&conn.id);
    Ok(())
}

/// Forget the keys of the connection `connection` names, and end its running
/// servers: the next call starts one on the connection's profile.
pub fn clear(connection: &str) -> Result<(), String> {
    let conn = resolve(connection)?;
    held().remove(&conn.id);
    pool::stop_connection(&conn.id);
    Ok(())
}

/// Whether the connection `connection` names holds keys. An unknown
/// connection holds none.
pub fn is_set(connection: &str) -> bool {
    resolve(connection).is_ok_and(|conn| held().contains_key(&conn.id))
}

/// The keys the connection with this id holds — for a launch, which hands
/// them to the server's environment and nowhere else.
pub(crate) fn for_connection(id: &str) -> Option<Arc<Credentials>> {
    held().get(id).cloned()
}
