// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `smb://` for registered files (spec 075 R9, R11–R13), read without the
//! share being mounted, through the pure-Rust `smb2` client.
//!
//! Each request is one short session on its own thread — connect, open the
//! share, `stat` or read, disconnect — inside a private current-thread tokio
//! runtime (as `maps_bridge` does), and the caller waits at most
//! [`DEADLINE`]. Only reading is ever asked of the share. Credentials come from
//! the address (R11); none means a guest login (R12). The password is never
//! shown, logged or kept: every message is built from the masked address.

use crate::registered_file::Fetch;

/// The `smb://` reader, or `None` in a build without the `smb` feature.
pub fn fetcher() -> Option<&'static dyn Fetch> {
    #[cfg(feature = "smb")]
    {
        Some(&real::RealSmb)
    }
    #[cfg(not(feature = "smb"))]
    {
        None
    }
}

/// The longest a registration waits on a share.
pub const DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);

#[cfg(feature = "smb")]
mod real {
    use super::DEADLINE;
    use crate::registered_file::{code, Fetch, Location, Refusal, SmbUrl};

    pub struct RealSmb;

    enum Ask {
        Size,
        Read,
    }

    enum Answer {
        Size(Option<u64>),
        Bytes(Vec<u8>),
    }

    fn refusal(url: &SmbUrl, e: &smb2::Error) -> Refusal {
        use smb2::ErrorKind as K;
        let at = url.to_string();
        let mut detail = e.to_string();
        if let Some(p) = url.password.as_deref().filter(|p| !p.is_empty()) {
            detail = detail.replace(p, "****");
        }
        match e.kind() {
            K::AuthRequired | K::SigningRequired | K::AccessDenied => Refusal::new(
                code::ACCESS_DENIED,
                format!("{at}: the share refused the login or the file ({detail})"),
            ),
            K::NotFound => Refusal::new(code::NOT_FOUND, format!("{at} does not exist")),
            _ => Refusal::new(code::UNREACHABLE, format!("{at} cannot be reached ({detail})")),
        }
    }

    /// One session: connect, open the share, answer, disconnect.
    fn ask(url: &SmbUrl, what: Ask, max: u64) -> Result<Answer, Refusal> {
        let (tx, rx) = std::sync::mpsc::channel();
        let job = url.clone();
        std::thread::spawn(move || {
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| Refusal::new(code::UNREACHABLE, format!("no network runtime: {e}")))
                .and_then(|rt| rt.block_on(session(&job, what, max)));
            let _ = tx.send(result);
        });
        rx.recv_timeout(DEADLINE).unwrap_or_else(|_| {
            Err(Refusal::new(
                code::UNREACHABLE,
                format!("{url} did not answer within {} s", DEADLINE.as_secs()),
            ))
        })
    }

    async fn session(url: &SmbUrl, what: Ask, max: u64) -> Result<Answer, Refusal> {
        let config = smb2::ClientConfig {
            addr: url.server(),
            timeout: std::time::Duration::from_secs(15),
            // Empty is a guest login (R12).
            username: url.user.clone().unwrap_or_default(),
            password: url.password.clone().unwrap_or_default(),
            domain: url.domain.clone().unwrap_or_default(),
            auto_reconnect: false,
            compression: true,
            dfs_enabled: true,
            dfs_target_overrides: Default::default(),
            connect_options: None,
        };
        let mut client = smb2::SmbClient::connect(config).await.map_err(|e| refusal(url, &e))?;
        let mut tree = client.connect_share(&url.share).await.map_err(|e| refusal(url, &e))?;
        let answer = match what {
            Ask::Size => match client.stat(&mut tree, &url.path).await {
                Ok(info) if info.is_directory => Err(Refusal::new(
                    code::NOT_FOUND,
                    format!("{url} is a folder, not a file"),
                )),
                Ok(info) => Ok(Answer::Size(Some(info.size))),
                Err(e) if e.kind() == smb2::ErrorKind::NotFound => Ok(Answer::Size(None)),
                Err(e) => Err(refusal(url, &e)),
            },
            Ask::Read => match client.stat(&mut tree, &url.path).await {
                Err(e) => Err(refusal(url, &e)),
                Ok(info) if info.size > max => Err(Refusal {
                    code: code::TOO_LARGE_FOR_LIMIT,
                    message: format!("{url} is {} bytes, over {max}", info.size),
                    size: info.size,
                    bound: max,
                }),
                Ok(_) => client
                    .read_file_pipelined(&mut tree, &url.path)
                    .await
                    .map(Answer::Bytes)
                    .map_err(|e| refusal(url, &e)),
            },
        };
        let _ = client.disconnect_share(&tree).await;
        answer
    }

    fn url(at: &Location) -> &SmbUrl {
        match at {
            Location::Smb(u) => u,
            Location::Fs(_) => unreachable!("the smb reader is asked only for smb:// locations"),
        }
    }

    impl Fetch for RealSmb {
        fn size(&self, at: &Location) -> Result<Option<u64>, Refusal> {
            match ask(url(at), Ask::Size, 0)? {
                Answer::Size(s) => Ok(s),
                Answer::Bytes(_) => unreachable!(),
            }
        }

        fn read(&self, at: &Location, max: u64) -> Result<Vec<u8>, Refusal> {
            match ask(url(at), Ask::Read, max)? {
                Answer::Bytes(b) => Ok(b),
                Answer::Size(_) => unreachable!(),
            }
        }
    }
}
