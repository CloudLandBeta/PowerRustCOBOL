// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! TLS for the channel — the operating system's stack through `native-tls`, on
//! both ends (WASM-003 R5; spec 063 AC8 as amended by 103 bars rustls, `ring`
//! and `aws-lc-rs`, which compile C).
//!
//! The floor is TLS 1.2 (WASM-013 R4). The server's identity is a PEM chain
//! with a PKCS#8 key, or a PKCS#12 file; the client trusts the system roots
//! and, when told, one more (a private CA — the loopback tests use it, a
//! deployment behind an internal CA needs it).

use native_tls::{Certificate, Identity, Protocol, TlsAcceptor, TlsConnector};

#[cfg(feature = "server")]
use crate::config::TlsSource;

/// The server side: an acceptor for the configured identity.
#[cfg(feature = "server")]
pub fn acceptor(source: &TlsSource) -> Result<tokio_native_tls::TlsAcceptor, String> {
    let identity = match source {
        TlsSource::Pem { cert_chain, key } => Identity::from_pkcs8(cert_chain, key).map_err(|e| format!("the TLS certificate and key do not form an identity: {e}"))?,
        TlsSource::Pkcs12 { der, password } => Identity::from_pkcs12(der, password.expose()).map_err(|e| format!("the PKCS#12 identity cannot be read: {e}"))?,
    };
    let acceptor = TlsAcceptor::builder(identity)
        .min_protocol_version(Some(Protocol::Tlsv12))
        .build()
        .map_err(|e| format!("TLS: {e}"))?;
    Ok(tokio_native_tls::TlsAcceptor::from(acceptor))
}

/// The client side: the system roots, plus `extra_root_pem` when given.
pub fn connector(extra_root_pem: Option<&[u8]>) -> Result<tokio_native_tls::TlsConnector, String> {
    let mut b = TlsConnector::builder();
    b.min_protocol_version(Some(Protocol::Tlsv12));
    if let Some(pem) = extra_root_pem {
        let cert = Certificate::from_pem(pem).map_err(|e| format!("the extra root certificate is not PEM: {e}"))?;
        b.add_root_certificate(cert);
    }
    let c = b.build().map_err(|e| format!("TLS: {e}"))?;
    Ok(tokio_native_tls::TlsConnector::from(c))
}
