// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! An independent SigV4 verifier (spec 103 AC1).
//!
//! The runtime signs with the official AWS crates. A test that checked those
//! signatures with the same crates would prove only that they agree with
//! themselves, so this is the algorithm written out from AWS's published
//! description — canonical request, string to sign, the HMAC chain — over `hmac`
//! and `sha2`, and checked first against AWS's own published example
//! (`get-vanilla`, whose expected signature is printed in AWS's test suite).
//!
//! Test code only: it takes the secret as an argument, never from the product.

use hmac::{Hmac, Mac};
use sha2_010::{Digest, Sha256};

use super::http_fake::Logged;

type HmacSha256 = Hmac<Sha256>;

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut m = HmacSha256::new_from_slice(key).expect("hmac takes a key of any length");
    m.update(data);
    m.finalize().into_bytes().to_vec()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha256_hex(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

/// The canonical request: method, path, query, the signed headers (name:value,
/// one per line), their names, and the hash of the body.
pub fn canonical_request(method: &str, path: &str, query: &str, headers: &[(String, String)], signed: &[String], body: &[u8]) -> String {
    let mut canonical_headers = String::new();
    for name in signed {
        let value = headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join(",");
        canonical_headers.push_str(&format!("{}:{}\n", name.to_ascii_lowercase(), value));
    }
    format!("{method}\n{path}\n{query}\n{canonical_headers}\n{}\n{}", signed.join(";"), sha256_hex(body))
}

/// The signature for a canonical request (`date` is `YYYYMMDD`, `stamp` is
/// `YYYYMMDDTHHMMSSZ`).
pub fn signature(secret: &str, stamp: &str, region: &str, service: &str, canonical: &str) -> String {
    let date = &stamp[..8];
    let scope = format!("{date}/{region}/{service}/aws4_request");
    let to_sign = format!("AWS4-HMAC-SHA256\n{stamp}\n{scope}\n{}", sha256_hex(canonical.as_bytes()));
    let k_date = hmac(format!("AWS4{secret}").as_bytes(), date.as_bytes());
    let k_region = hmac(&k_date, region.as_bytes());
    let k_service = hmac(&k_region, service.as_bytes());
    let k_signing = hmac(&k_service, b"aws4_request");
    hex(&hmac(&k_signing, to_sign.as_bytes()))
}

/// What the `Authorization` header of a signed request says.
pub struct Authorization {
    pub key_id: String,
    pub date: String,
    pub region: String,
    pub service: String,
    pub signed_headers: Vec<String>,
    pub signature: String,
}

pub fn parse_authorization(value: &str) -> Result<Authorization, String> {
    let rest = value.strip_prefix("AWS4-HMAC-SHA256 ").ok_or("not an AWS4-HMAC-SHA256 header")?;
    let (mut credential, mut signed, mut sig) = (None, None, None);
    for part in rest.split(',').map(str::trim) {
        if let Some(v) = part.strip_prefix("Credential=") {
            credential = Some(v);
        } else if let Some(v) = part.strip_prefix("SignedHeaders=") {
            signed = Some(v);
        } else if let Some(v) = part.strip_prefix("Signature=") {
            sig = Some(v);
        }
    }
    let credential = credential.ok_or("no Credential")?;
    let scope: Vec<&str> = credential.split('/').collect();
    if scope.len() != 5 || scope[4] != "aws4_request" {
        return Err(format!("the credential scope is malformed: {} parts", scope.len()));
    }
    Ok(Authorization {
        key_id: scope[0].to_owned(),
        date: scope[1].to_owned(),
        region: scope[2].to_owned(),
        service: scope[3].to_owned(),
        signed_headers: signed.ok_or("no SignedHeaders")?.split(';').map(str::to_owned).collect(),
        signature: sig.ok_or("no Signature")?.to_owned(),
    })
}

/// Check one logged request: it carries a signature for `service` and `region`,
/// made with `secret`, over exactly the method, path, headers and body it was
/// sent with. Returns what was signed, for a test to report.
pub fn verify(req: &Logged, secret: &str, service: &str, region: &str) -> Result<Authorization, String> {
    let auth = parse_authorization(req.header("authorization").ok_or("no Authorization header")?)?;
    if auth.service != service || auth.region != region {
        return Err(format!("signed for {}/{}, expected {service}/{region}", auth.region, auth.service));
    }
    let stamp = req.header("x-amz-date").ok_or("no X-Amz-Date header")?;
    if stamp.len() != 16 || !stamp.starts_with(&auth.date) {
        return Err(format!("X-Amz-Date {stamp} does not match the credential date {}", auth.date));
    }
    for name in &auth.signed_headers {
        if req.header(name).is_none() {
            return Err(format!("the signature names a header that was not sent: {name}"));
        }
    }
    for must in ["host", "x-amz-date"] {
        if !auth.signed_headers.iter().any(|h| h == must) {
            return Err(format!("{must} is not among the signed headers"));
        }
    }
    let (path, query) = req.path.split_once('?').unwrap_or((req.path.as_str(), ""));
    let canonical = canonical_request(&req.method, path, query, &req.headers, &auth.signed_headers, &req.body);
    let expected = signature(secret, stamp, region, service, &canonical);
    if expected == auth.signature {
        Ok(auth)
    } else {
        Err(format!("the signature does not verify (expected {}…, got {}…)", &expected[..8], &auth.signature[..8.min(auth.signature.len())]))
    }
}

/// AWS's published example, `get-vanilla` of its SigV4 test suite: the verifier
/// must reproduce the signature AWS prints before it is allowed to judge ours.
/// (The credentials are AWS's documented examples, not anyone's.)
pub fn reproduces_the_published_example() -> Result<String, String> {
    let headers = vec![
        ("host".to_owned(), "example.amazonaws.com".to_owned()),
        ("x-amz-date".to_owned(), "20150830T123600Z".to_owned()),
    ];
    let signed = vec!["host".to_owned(), "x-amz-date".to_owned()];
    let canonical = canonical_request("GET", "/", "", &headers, &signed, b"");
    let got = signature("wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY", "20150830T123600Z", "us-east-1", "service", &canonical);
    let want = "5fa00fa31553b73ebf1942676e86291e8372ff2a2260956d9b8aae1d763fbf31";
    if got == want {
        Ok(got)
    } else {
        Err(format!("the verifier computed {got}, AWS publishes {want}"))
    }
}
