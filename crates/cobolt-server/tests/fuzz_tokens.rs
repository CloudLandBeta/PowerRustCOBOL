// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Fuzzing of the token validator (WASM-013 R15, T20; AC11, tokens).
//!
//! `TokenValidator::validate` reads a string a stranger chose. The property is one sentence: **no token that the
//! configured key did not sign is accepted**, and nothing else a stranger sends panics, hangs or is answered with
//! anything but a named authentication error. The oracle for the first half is exact: every token the test signs, with
//! the real keys, has its *signing input* (`header.payload`, the bytes the signature covers) recorded; a token the
//! validator accepts must carry one of them. Anything else that is accepted has been accepted on a signature nobody
//! made.
//!
//! The tokens are:
//!
//! * **mutations of genuine tokens** (ES256 and RS256 from the issuer, HS256 from the development secret): bits, bytes,
//!   ranges, segments (dropped, doubled, swapped, emptied, a fourth), padding, a segment taken from another token, the
//!   last character of the signature in every one of the 64 base64 values (a non-canonical encoding of a genuine
//!   signature), a `Bearer ` prefix, a newline;
//! * **header forgeries**: `alg` `none` in four spellings, `HS256`/`HS384`/`HS512` signed with the issuer's published key
//!   document, with the empty string and with a guess, `RS256` carrying an ES256 signature, `ES256` carrying an HMAC,
//!   algorithms that no verifier knows, a numeric or array `alg`; a `kid` that is wrong, absent, empty, huge, a path, an
//!   array, null or Unicode; and `jwk`, `jku`, `x5u`, `x5c` and `crit` members in the header, the first signed with an
//!   attacker's own key (the embedded-key attack);
//! * **a wrong key**: the same `kid`, the same claims, another key;
//! * **claims** nobody should honour, correctly signed by the real key: a wrong or absent issuer, audience, subject,
//!   expiry or not-before, an identity token's marks, in the wrong types and the extreme values. Here the oracle is the
//!   documented rules: a token whose claims break one of them with a margin the leeway cannot cover is refused;
//! * **size**: a token past 16 KiB, a header of 70 KB, ten thousand dots, a megabyte of one letter.
//!
//! Both configurations are driven: a release-like one with no development secret, and a development one with it. A
//! fixed seed makes a failure reproduce; `PR_FUZZ_ITERS=<n>` multiplies the work (default 1). The run prints its
//! iterations, its outcomes by code, and its seconds.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Instant;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use cobolt_server::config::{RunMode, ServerConfig};
use cobolt_server::dev::mint_dev_token;
use cobolt_server::jwks::Jwks;
use cobolt_server::jwt::{now, TokenValidator};
use cobolt_server::testkit::*;
use cobolt_wire::error::{codes, ErrorClass};
use serde_json::{json, Value};

const ISS: &str = "https://idp.example.test/pool";
const AUD: &str = "prc-api";
const CLIENT: &str = "app-client-1";
const KID: &str = "k1";
const RKID: &str = "r1";
const DEV_SECRET: &str = "dev-only-secret-for-this-test-0123456789";
const LEEWAY: i64 = 30;
/// Seconds of slack the oracle allows for the clock moving between minting and judging.
const MARGIN: i64 = 5;
const SEED: u64 = 0x70C3_7E57_D15C_0DE5;
/// Tokens judged per configuration at `PR_FUZZ_ITERS=1`.
const RANDOM: u64 = 6_000;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn chance(&mut self, one_in: u64) -> bool {
        self.below(one_in) == 0
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len() as u64) as usize]
    }
}

fn iters() -> u64 {
    std::env::var("PR_FUZZ_ITERS").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(1).max(1)
}

fn b64(v: &Value) -> String {
    URL_SAFE_NO_PAD.encode(v.to_string())
}

fn claims(extra: Value) -> Value {
    let t = now();
    let mut c = json!({"iss": ISS, "sub": "user-1", "aud": AUD, "iat": t, "exp": t + 600});
    for (k, v) in extra.as_object().unwrap() {
        if v.is_null() {
            c.as_object_mut().unwrap().remove(k);
        } else {
            c[k] = v.clone();
        }
    }
    c
}

/// The issuer's published keys: the ES256 key and the RS256 key in one document.
fn jwks_doc(es: &Issuer, rsa: &RsaIssuer) -> String {
    let mut keys = Vec::new();
    for doc in [es.jwks(), rsa.jwks()] {
        let v: Value = serde_json::from_str(&doc).unwrap();
        keys.extend(v["keys"].as_array().unwrap().iter().cloned());
    }
    json!({ "keys": keys }).to_string()
}

fn validator(doc: String, dev_secret: Option<&str>) -> TokenValidator {
    let text = format!("[auth]\nissuer = [\"{ISS}\"]\naudiences = [\"{AUD}\"]\nclient_ids = [\"{CLIENT}\"]\nalgorithms = [\"RS256\", \"ES256\"]\nleeway_seconds = {LEEWAY}\n");
    let cfg = ServerConfig::from_toml(&text).expect("config");
    let resolved = cfg.resolve(RunMode::Development, std::path::Path::new(".")).expect("resolve");
    let fetcher = StaticJwks::new(doc);
    let jwks = Arc::new(Jwks::new(format!("{ISS}/.well-known/jwks.json"), resolved.auth.jwks_ttl, resolved.auth.jwks_min_refresh, fetcher));
    TokenValidator::new(resolved.auth, Some(jwks), dev_secret.map(str::to_owned))
}

/// The two segments a signature covers.
fn signing_input(token: &str) -> &str {
    token.rsplit_once('.').map(|(i, _)| i).unwrap_or(token)
}

/// The keys of the test, the genuine tokens, and what the validator may accept.
struct Env {
    es: Issuer,
    rsa: RsaIssuer,
    /// Another key under the same `kid`: an attacker's.
    attacker: Issuer,
    /// Genuine tokens (the seeds of the mutations): ES256, RS256, development.
    genuine: Vec<String>,
    /// Signing inputs of every token signed by the issuer's keys, each with the token the test minted for it.
    known: HashMap<String, String>,
    /// The same for the development secret: genuine in the development configuration only.
    dev_known: HashMap<String, String>,
}

impl Env {
    fn new() -> Env {
        let es = Issuer::new(ISS, KID);
        let rsa = RsaIssuer::new(ISS, RKID);
        let attacker = Issuer::new(ISS, KID);
        let a = es.mint(&claims(json!({"roles": ["admin"]})));
        let b = rsa.mint(&claims(json!({})));
        let d = mint_dev_token(DEV_SECRET, "dev-user", &["admin"], 600);
        let known: HashMap<String, String> = [&a, &b].iter().map(|t| (signing_input(t).to_owned(), (*t).clone())).collect();
        let dev_known: HashMap<String, String> = [(signing_input(&d).to_owned(), d.clone())].into_iter().collect();
        Env { es, rsa, attacker, genuine: vec![a, b, d], known, dev_known }
    }

    /// The token the test minted with this signing input, if a configuration may accept one: the issuer's keys signed it, or,
    /// where the development secret is set, that secret did.
    fn minted_for(&self, dev: bool, input: &str) -> Option<&String> {
        self.known.get(input).or_else(|| if dev { self.dev_known.get(input) } else { None })
    }

    /// A token the real ES256 key signs: recorded.
    fn sign_es(&mut self, c: &Value) -> String {
        let t = self.es.mint(c);
        self.known.insert(signing_input(&t).to_owned(), t.clone());
        t
    }
}

// ── the mutations ───────────────────────────────────────────────────────────

const SPECIAL: &[u8] = b".-_=+/ \t\n\r\"{}[]:,\\\x00\x7f\xff";
const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn mutate(rng: &mut Rng, env: &Env) -> String {
    let seed = rng.pick(&env.genuine).clone();
    let mut b = seed.as_bytes().to_vec();
    for _ in 0..1 + rng.below(3) {
        if b.is_empty() {
            b.push(b'.');
        }
        let at = rng.below(b.len() as u64) as usize;
        match rng.below(16) {
            0 => b[at] ^= 1 << rng.below(8),
            1 => b[at] = rng.next() as u8,
            2 => b[at] = *rng.pick(SPECIAL),
            3 => {
                let n = (1 + rng.below(12) as usize).min(b.len() - at);
                b.drain(at..at + n);
            }
            4 => {
                let n = 1 + rng.below(12) as usize;
                let bytes: Vec<u8> = (0..n).map(|_| rng.next() as u8).collect();
                b.splice(at..at, bytes);
            }
            5 => {
                let n = (1 + rng.below(40) as usize).min(b.len() - at);
                let piece = b[at..at + n].to_vec();
                b.splice(at..at, piece);
            }
            6 => b.truncate(at),
            7 => {
                // A segment is dropped, doubled, emptied, swapped with another, or a fourth is added.
                let text = String::from_utf8_lossy(&b).into_owned();
                let mut parts: Vec<String> = text.split('.').map(str::to_owned).collect();
                let i = rng.below(parts.len() as u64) as usize;
                match rng.below(5) {
                    0 => {
                        parts.remove(i);
                    }
                    1 => parts.insert(i, parts[i].clone()),
                    2 => parts[i].clear(),
                    3 => {
                        let j = rng.below(parts.len() as u64) as usize;
                        parts.swap(i, j);
                    }
                    _ => parts.push(parts[i].clone()),
                }
                b = parts.join(".").into_bytes();
            }
            8 => {
                // A segment from another genuine token.
                let other = rng.pick(&env.genuine).clone();
                let theirs: Vec<&str> = other.split('.').collect();
                let text = String::from_utf8_lossy(&b).into_owned();
                let mut parts: Vec<String> = text.split('.').map(str::to_owned).collect();
                let i = rng.below(parts.len() as u64) as usize;
                parts[i] = theirs[rng.below(theirs.len() as u64) as usize].to_owned();
                b = parts.join(".").into_bytes();
            }
            9 => b.extend(std::iter::repeat_n(b'=', 1 + rng.below(4) as usize)),
            10 => {
                // The last character of the signature in another of the 64 base64 values: a non-canonical encoding.
                if let Some(last) = b.last_mut() {
                    *last = *rng.pick(B64);
                }
            }
            11 => {
                b.splice(0..0, b"Bearer ".iter().copied());
            }
            12 => b.push(b'\n'),
            13 => b.insert(at, b'.'),
            14 => {
                let t = *rng.pick(&["é", "日", "😀", "\u{FEFF}"]);
                b.splice(at..at, t.bytes());
            }
            _ => {
                let other = rng.below(b.len() as u64) as usize;
                b.swap(at, other);
            }
        }
    }
    String::from_utf8_lossy(&b).into_owned()
}

// ── forgeries ───────────────────────────────────────────────────────────────

/// A header with this `alg` and `kid` and the extras, as JSON.
fn header(alg: Value, kid: Value, extra: &[(&str, Value)]) -> Value {
    let mut h = json!({"typ": "JWT"});
    if !alg.is_null() {
        h["alg"] = alg;
    }
    if !kid.is_null() {
        h["kid"] = kid;
    }
    for (k, v) in extra {
        h[*k] = v.clone();
    }
    h
}

/// A forged token, and whether the header alone makes it one that must be refused even though a configured key signed it
/// (RFC 7515 section 4.1.11: a `crit` the verifier does not understand).
fn forged(rng: &mut Rng, env: &mut Env, doc: &str) -> (String, bool) {
    let good = claims(json!({}));
    let algs = [
        json!("none"), json!("None"), json!("NONE"), json!("nOnE"), json!("HS256"), json!("HS384"), json!("HS512"), json!("RS256"), json!("RS384"), json!("RS512"), json!("ES256"), json!("ES384"),
        json!("PS256"), json!("EdDSA"), json!(""), json!("RS256 "), json!(null), json!(5), json!(["RS256"]), json!({"a": 1}), json!(true),
    ];
    let long_kid = "k".repeat(5000);
    let kids = [json!(KID), json!(RKID), json!("other"), json!(null), json!(""), json!(long_kid), json!("../../etc/passwd"), json!([KID]), json!("日本語"), json!(7), json!("k1 "), json!("K1")];
    let alg = rng.pick(&algs).clone();
    let kid = rng.pick(&kids).clone();
    let mut crit = false;
    let extras: Vec<(&str, Value)> = match rng.below(8) {
        0 => vec![("jwk", env.attacker.jwk())],
        1 => vec![("jku", json!("https://attacker.example.test/jwks.json"))],
        2 => vec![("x5u", json!("https://attacker.example.test/cert.pem"))],
        3 => vec![("x5c", json!(["MIIB"]))],
        4 => {
            crit = true;
            vec![("crit", json!(["exp", "nope"])), ("exp", json!(1))]
        }
        5 => vec![("cty", json!("JWT")), ("zip", json!("DEF"))],
        _ => vec![],
    };
    let h = header(alg, kid.clone(), &extras);
    // Who signs it, and how.
    let token = match rng.below(9) {
        // An attacker's own key, over exactly this header (the embedded-key attack when the header carries a `jwk`).
        0 | 1 => env.attacker.mint_with_header(&h, &good),
        // The real key signs a header with extras: a genuine signature, a header the validator must still judge alone.
        2 => {
            let t = env.es.mint_with_header(&h, &good);
            env.known.insert(signing_input(&t).to_owned(), t.clone());
            return (t, crit);
        }
        // No signature, an empty one, or noise.
        3 => format!("{}.{}.", b64(&h), b64(&good)),
        4 => format!("{}.{}.{}", b64(&h), b64(&good), URL_SAFE_NO_PAD.encode((0..64).map(|_| rng.next() as u8).collect::<Vec<u8>>())),
        // HMAC with a key the attacker knows: the published key document, the empty string, a guess.
        5..=7 => {
            let alg = *rng.pick(&[jsonwebtoken::Algorithm::HS256, jsonwebtoken::Algorithm::HS384, jsonwebtoken::Algorithm::HS512]);
            let secret: Vec<u8> = match rng.below(4) {
                0 => doc.as_bytes().to_vec(),
                1 => Vec::new(),
                2 => b"secret".to_vec(),
                _ => env.es.jwks().into_bytes(),
            };
            let mut jh = jsonwebtoken::Header::new(alg);
            jh.kid = kid.as_str().map(str::to_owned);
            jsonwebtoken::encode(&jh, &good, &jsonwebtoken::EncodingKey::from_secret(&secret)).unwrap_or_default()
        }
        // The development secret itself: genuine where the secret is configured, with the header the test picks.
        _ => {
            let mut jh = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
            jh.kid = kid.as_str().map(str::to_owned);
            let t = jsonwebtoken::encode(&jh, &good, &jsonwebtoken::EncodingKey::from_secret(DEV_SECRET.as_bytes())).unwrap_or_default();
            env.dev_known.insert(signing_input(&t).to_owned(), t.clone());
            t
        }
    };
    (token, false)
}

/// A token the attacker signs with a key of their own under the issuer's `kid`, with good claims.
fn wrong_key(env: &Env, rng: &mut Rng) -> String {
    let c = claims(json!({"roles": ["admin"], "sub": format!("victim-{}", rng.below(100))}));
    env.attacker.mint(&c)
}

// ── claims ──────────────────────────────────────────────────────────────────

fn odd(rng: &mut Rng) -> Value {
    let t = now();
    let long = "x".repeat(40_000);
    rng.pick(&[
        json!(null), json!(""), json!("wrong"), json!(0), json!(-1), json!(t), json!(t - 100_000), json!(t + 100_000), json!(t as f64 + 0.5), json!(9_007_199_254_740_993i64), json!(u64::MAX), json!(1e308), json!(true),
        json!([]), json!([AUD]), json!(["wrong", AUD]), json!([1, 2]), json!({"a": AUD}), json!(long), json!(ISS), json!(AUD), json!(CLIENT), json!("access"), json!("id"), json!("ID"),
    ])
    .clone()
}

/// Claims with some members replaced by odd values or removed.
fn weird_claims(rng: &mut Rng) -> Value {
    let mut c = claims(json!({}));
    let names = ["iss", "sub", "aud", "exp", "nbf", "iat", "client_id", "azp", "at_hash", "nonce", "typ", "token_use", "scp", "roles", "groups"];
    for _ in 0..1 + rng.below(4) {
        let k = *rng.pick(&names);
        if rng.chance(4) {
            c.as_object_mut().unwrap().remove(k);
        } else {
            c[k] = odd(rng);
        }
    }
    c
}

/// Claims that break a documented rule by more than the leeway can forgive: the token must be refused. Read from
/// `jwt.rs` (the generic profile of this configuration); it says "refuse" only where the rule is plain.
fn must_refuse(c: &Value) -> Option<&'static str> {
    let t = now();
    match c.get("iss") {
        Some(Value::String(s)) if s == ISS => {}
        Some(Value::Array(a)) if a.iter().any(|v| v.as_str() == Some(ISS)) => {}
        _ => return Some("the issuer is not the configured one"),
    }
    // An expiry is a whole number of seconds, and not in the past beyond the leeway.
    match c.get("exp").and_then(Value::as_i64) {
        Some(e) if e + LEEWAY + MARGIN >= t => {}
        Some(_) => return Some("expired"),
        None => return Some("no exp that is a whole number of seconds"),
    }
    if matches!(c.get("nbf").and_then(Value::as_i64), Some(n) if n - LEEWAY - MARGIN > t) {
        return Some("not valid yet");
    }
    if c.get("sub").and_then(Value::as_str).is_none_or(str::is_empty) {
        return Some("no subject");
    }
    let aud_ok = match c.get("aud") {
        Some(Value::String(s)) => s == AUD,
        Some(Value::Array(a)) => a.iter().any(|v| v.as_str() == Some(AUD)),
        _ => false,
    };
    let client_ok = ["client_id", "azp", "appid"].iter().any(|k| c.get(*k).and_then(Value::as_str) == Some(CLIENT));
    if !aud_ok && !client_ok {
        return Some("not for this application");
    }
    if c.get("at_hash").is_some() || c.get("nonce").is_some() {
        return Some("an identity token's mark");
    }
    if matches!(c.get("token_use").and_then(Value::as_str), Some(u) if u != "access") {
        return Some("token_use is not access");
    }
    if matches!(c.get("typ").and_then(Value::as_str), Some(t) if t.eq_ignore_ascii_case("ID") || t.eq_ignore_ascii_case("Refresh")) {
        return Some("an ID or refresh token");
    }
    None
}

// ── the oracle ──────────────────────────────────────────────────────────────

#[derive(Default)]
struct Tally {
    judged: u64,
    accepted: u64,
    /// Accepted although the bytes are not those of the token the test minted for that signing input (a non-canonical
    /// encoding of a genuine signature).
    accepted_reencoded: u64,
    by_code: BTreeMap<u32, u64>,
    forged: u64,
    crit_refused: u64,
    claims_must_refuse: u64,
}

async fn judge(v: &TokenValidator, token: &str, env: &Env, dev: bool, t: &mut Tally) -> bool {
    t.judged += 1;
    match v.validate(token).await {
        Ok(c) => {
            let Some(minted) = env.minted_for(dev, signing_input(token)) else {
                panic!("ACCEPTED A TOKEN NO CONFIGURED KEY SIGNED (sub {:?}): {}", c.sub, clip(token));
            };
            t.accepted += 1;
            if minted != token {
                t.accepted_reencoded += 1;
            }
            true
        }
        Err(e) => {
            assert_eq!(e.class, ErrorClass::Authentication, "a refused token is an authentication error: {e:?}");
            assert!([codes::TOKEN_MISSING, codes::TOKEN_INVALID, codes::TOKEN_EXPIRED].contains(&e.code), "a named code: {e:?}");
            assert!(!e.message.is_empty() && e.message.len() < 300, "a named, short reason: {e:?}");
            // The reason never carries a piece of the token.
            for seg in token.split('.').filter(|s| s.chars().count() >= 16) {
                let piece: String = seg.chars().take(16).collect();
                assert!(!e.message.contains(&piece), "the refusal quotes the token: {}", e.message);
            }
            *t.by_code.entry(e.code).or_default() += 1;
            false
        }
    }
}

fn clip(s: &str) -> String {
    s.chars().take(120).collect()
}

async fn run(name: &str, v: &TokenValidator, dev: bool, env: &mut Env, doc: &str) -> String {
    let started = Instant::now();
    let mut rng = Rng(SEED ^ if dev { 7 } else { 3 });
    let mut t = Tally::default();
    let n = RANDOM * iters();
    for i in 0..n {
        match rng.below(10) {
            0..=4 => {
                let token = mutate(&mut rng, env);
                judge(v, &token, env, dev, &mut t).await;
            }
            5 | 6 => {
                let (token, flawed) = forged(&mut rng, env, doc);
                t.forged += 1;
                let accepted = judge(v, &token, env, dev, &mut t).await;
                if flawed {
                    t.crit_refused += 1;
                    assert!(!accepted, "a token with a `crit` header the verifier does not understand was accepted: {}", clip(&token));
                }
            }
            7 => {
                let token = wrong_key(env, &mut rng);
                let accepted = judge(v, &token, env, dev, &mut t).await;
                assert!(!accepted, "a token signed by another key under the issuer's kid was accepted");
            }
            8 => {
                let c = weird_claims(&mut rng);
                // The real key signs it: only the rules of the claims can refuse it.
                let token = env.sign_es(&c);
                let accepted = judge(v, &token, env, dev, &mut t).await;
                if let Some(rule) = must_refuse(&c) {
                    t.claims_must_refuse += 1;
                    assert!(!accepted, "a correctly signed token that breaks a rule was accepted ({rule}): {}", clip(&c.to_string()));
                }
            }
            _ => {
                // Size.
                let big = match i % 5 {
                    0 => format!("{}.{}.{}", "A".repeat(16 * 1024), "B", "C"),
                    1 => format!("{}.{}.{}", URL_SAFE_NO_PAD.encode(vec![b'{'; 70_000]), "e30", "AAAA"),
                    2 => ".".repeat(10_000),
                    3 => "A".repeat(1_000_000),
                    _ => format!("{}.{}.{}", "e30", "e30", "A".repeat(20_000)),
                };
                let accepted = judge(v, &big, env, dev, &mut t).await;
                assert!(!accepted);
            }
        }
    }
    // The genuine tokens still pass (the validator was not broken by what it was fed), where each configuration allows.
    for (i, expected) in [(0usize, true), (1, true), (2, dev)] {
        let ok = v.validate(&env.genuine[i]).await.is_ok();
        assert_eq!(ok, expected, "the genuine token {i} in the {name} configuration");
    }
    let by_code = t.by_code.iter().map(|(c, n)| format!("{c}: {n}")).collect::<Vec<_>>().join(", ");
    format!(
        "{name}: {} tokens judged ({} forged, {} with a crit header, {} signed by the real key with claims that break a rule); {} accepted, every one on a signing input a configured key made ({} re-encoded); refusals by code {{{by_code}}}; {:.2} s",
        t.judged,
        t.forged,
        t.crit_refused,
        t.claims_must_refuse,
        t.accepted,
        t.accepted_reencoded,
        started.elapsed().as_secs_f64()
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn no_token_the_configured_key_did_not_sign_is_accepted_and_nothing_a_stranger_sends_gets_past_a_named_refusal() {
    let started = Instant::now();
    let mut env = Env::new();
    let doc = jwks_doc(&env.es, &env.rsa);
    let release = validator(doc.clone(), None);
    let dev = validator(doc.clone(), Some(DEV_SECRET));
    let a = run("release-like (no development secret)", &release, false, &mut env, &doc).await;
    let b = run("development (secret configured)", &dev, true, &mut env, &doc).await;
    println!("\n  fuzz_tokens (PR_FUZZ_ITERS={}):\n    {a}\n    {b}\n    total {:.2} s", iters(), started.elapsed().as_secs_f64());
}
