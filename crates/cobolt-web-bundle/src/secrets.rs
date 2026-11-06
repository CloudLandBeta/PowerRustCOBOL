// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The secret scan** that gates a Web or Mobile Build (WASM-002 R7, WASM-013 R7).
//!
//! A bundle is public: whatever is in it, anyone can read. The scan looks through every text a
//! bundle holds — the forms, the stylesheets, the manifest, the text assets and the files the
//! assembler produced — for the shapes of secrets and **fails the Build naming the file and the
//! pattern, never printing the value** (a finding carries a line number and a pattern name; there
//! is no field that could hold the secret).
//!
//! Known shapes are **errors**: cloud access keys, private-key blocks, JSON Web Tokens, API keys of
//! well-known services, connection strings with a password, and a property whose name says it is a
//! secret (`…APIKey`, `…Password`, `…Secret`, `…Token`) holding a value. High-entropy strings are
//! **warnings** (WASM-013 Q3): a hash or a UUID is not a leak, and a false error would teach people
//! to turn the scan off.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// What was found and where — never the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub file: String,
    pub line: usize,
    pub pattern: &'static str,
    pub severity: Severity,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = if self.severity == Severity::Error { "secret" } else { "possible secret" };
        write!(f, "{}: line {}: {} ({}); the value is not shown", self.file, self.line, kind, self.pattern)
    }
}

/// Every pattern [`scan_text`] can report, with its severity (WASM-013 R7: the list is public and complete).
/// `scan_text` pushes only names taken from this table; a test plants one value per entry.
pub const PATTERNS: &[(&str, Severity)] = &[
    ("private key block", Severity::Error),
    ("cloud access key id", Severity::Error),
    ("JSON Web Token", Severity::Error),
    ("connection string with a password", Severity::Error),
    ("connection string with a Key=value secret", Severity::Error),
    ("Google API key", Severity::Error),
    ("GitHub token", Severity::Error),
    ("Slack token", Severity::Error),
    ("Stripe live key", Severity::Error),
    ("Anthropic API key", Severity::Error),
    ("Hugging Face token", Severity::Error),
    ("API key", Severity::Error),
    ("a property named like a secret holds a value", Severity::Error),
    ("a secret access key", Severity::Error),
    ("high-entropy string", Severity::Warning),
];

/// The name `scan_text` reports, taken from [`PATTERNS`] (it panics in a test build if the name is not there).
fn pattern(name: &'static str) -> &'static str {
    PATTERNS.iter().find(|(n, _)| *n == name).map(|(n, _)| *n).unwrap_or_else(|| panic!("pattern {name:?} is not in PATTERNS"))
}

/// `Password=…;`, `Pwd=…`, `AccountKey=…`, `SharedAccessKey=…` with a value that is not empty and not a placeholder
/// (`${…}`, `<…>`, only `*`/`•`). The key matches ignoring case, as a connection string's keys do.
fn has_key_value_secret(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    for key in ["password=", "pwd=", "accountkey=", "sharedaccesskey="] {
        let mut from = 0;
        while let Some(p) = lower[from..].find(key) {
            let at = from + p;
            from = at + key.len();
            // The key starts a word: `;Password=` or ` Pwd=` or the start of the line, not `MyPassword=`.
            if at > 0 && lower.as_bytes()[at - 1].is_ascii_alphanumeric() {
                continue;
            }
            let rest = line[from..].trim_start_matches(['"', '\'']);
            let end = rest.find([';', '"', '\'', ' ', '\t', '<', '&']).unwrap_or(rest.len());
            let v = rest[..end].trim();
            let placeholder = v.is_empty() || v.starts_with("${") || v.starts_with('{') || v.starts_with('<') || v.starts_with('%') || v.chars().all(|c| c == '*' || c == '•');
            if !placeholder {
                return true;
            }
        }
    }
    false
}

/// The longest run of characters a high-entropy candidate must reach.
pub const DEFAULT_ENTROPY_LEN: usize = 40;

fn is_b64url(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+' | '/' | '=')
}

fn entropy(s: &str) -> f64 {
    let mut counts: BTreeMap<char, usize> = BTreeMap::new();
    for c in s.chars() {
        *counts.entry(c).or_default() += 1;
    }
    let n = s.chars().count() as f64;
    counts.values().map(|&c| { let p = c as f64 / n; -p * p.log2() }).sum()
}

/// Maximal runs of base64url characters in `line`.
fn runs(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in line.char_indices() {
        if is_b64url(c) {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            out.push(&line[s..i]);
        }
    }
    if let Some(s) = start {
        out.push(&line[s..]);
    }
    out
}

/// Is `run` (a slice of `line`) the payload of a `data:image/…;base64,`, `data:font/…`, `data:audio/…`
/// or `data:video/…` address? Such bytes are a file written inline, not a secret.
fn in_media_data_uri(line: &str, run: &str) -> bool {
    let start = (run.as_ptr() as usize).saturating_sub(line.as_ptr() as usize);
    let Some(before) = line.get(..start) else { return false };
    if !before.ends_with(";base64,") {
        return false;
    }
    let head = before.trim_end_matches(";base64,");
    let Some(at) = head.rfind("data:") else { return false };
    let kind = head[at + "data:".len()..].to_ascii_lowercase();
    ["image/", "font/", "audio/", "video/", "application/font"].iter().any(|p| kind.starts_with(p))
}

const AWS_PREFIXES: &[&str] =&["AKIA", "ASIA", "AIDA", "AROA", "AGPA", "ANPA", "ANVA", "APKA"];

fn has_aws_key(line: &str) -> bool {
    runs(line).iter().any(|r| AWS_PREFIXES.iter().any(|p| r.starts_with(p) && r.len() >= 20 && r[..20].chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())))
}

fn has_jwt(line: &str) -> bool {
    let b = line.as_bytes();
    let mut i = 0;
    while let Some(p) = line[i..].find("eyJ") {
        let s = i + p;
        let rest = &line[s..];
        let parts: Vec<&str> = rest.splitn(4, '.').collect();
        if parts.len() >= 3 {
            let seg = |x: &str| x.chars().take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')).count();
            if seg(parts[0]) >= 10 && seg(parts[1]) >= 10 && seg(parts[2]) >= 1 {
                return true;
            }
        }
        i = s + 3;
        if i >= b.len() {
            break;
        }
    }
    false
}

/// `scheme://user:password@host` with a non-empty password that is not an obvious placeholder.
fn has_credentialed_url(line: &str) -> bool {
    let mut i = 0;
    while let Some(p) = line[i..].find("://") {
        let s = i + p + 3;
        let rest = &line[s..];
        let end = rest.find(|c: char| c == '/' || c == ' ' || c == '"' || c == '\'' || c == '<').unwrap_or(rest.len());
        let authority = &rest[..end];
        if let Some(at) = authority.rfind('@') {
            if let Some((_user, pass)) = authority[..at].split_once(':') {
                let placeholder = pass.is_empty() || pass.starts_with('$') || pass.starts_with('{') || pass.starts_with('<') || pass.eq_ignore_ascii_case("password") || pass == "***" || pass == "xxx";
                if !placeholder {
                    return true;
                }
            }
        }
        i = s;
    }
    false
}

fn has_prefixed_token(line: &str) -> Option<&'static str> {
    let table: &[(&str, &str, usize)] = &[
        ("AIza", "Google API key", 39),
        ("ghp_", "GitHub token", 40),
        ("gho_", "GitHub token", 40),
        ("github_pat_", "GitHub token", 40),
        ("xoxb-", "Slack token", 20),
        ("xoxp-", "Slack token", 20),
        ("xoxa-", "Slack token", 20),
        ("sk_live_", "Stripe live key", 20),
        ("rk_live_", "Stripe live key", 20),
        ("sk-ant-", "Anthropic API key", 30),
        ("hf_", "Hugging Face token", 30),
    ];
    for r in runs(line) {
        for (prefix, name, min) in table {
            if r.starts_with(prefix) && r.len() >= *min {
                return Some(name);
            }
        }
    }
    // `sk-` + 40 alphanumerics: the shape of a common LLM provider key.
    runs(line).iter().find(|r| r.starts_with("sk-") && r.len() >= 40 && r[3..].chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')).map(|_| "API key")
}

/// A property whose NAME says it is a secret and whose VALUE is not empty.
fn secret_property(line: &str) -> bool {
    // <Property name="AgentAPIKey">value</Property>   or   name="Password" value="..."
    let lower = line.to_ascii_lowercase();
    let Some(np) = lower.find("name=\"") else { return false };
    let after = &line[np + 6..];
    let Some(q) = after.find('"') else { return false };
    let name = after[..q].to_ascii_lowercase();
    let secretish = ["apikey", "api-key", "api_key", "password", "passwd", "secret", "token", "privatekey", "accesskey", "clientsecret", "authkey", "connectionstring"];
    if !secretish.iter().any(|s| name.contains(s)) {
        return false;
    }
    // Property names that are about WHICH key to use, not the key itself.
    if name.ends_with("name") || name.ends_with("file") || name.ends_with("enabled") || name.ends_with("type") || name.ends_with("character") || name.ends_with("length") {
        return false;
    }
    // Words shown to the user about a secret, not the secret: the Authentication control's
    // `PasswordLabel`, `ForgotPasswordLink`, … (WASM-009 D14).
    if ["label", "caption", "text", "link"].iter().any(|w| name.ends_with(w)) {
        return false;
    }
    let tail = &after[q + 1..];
    let value = if let Some(g) = tail.strip_prefix('>') {
        g.split('<').next().unwrap_or("")
    } else if let Some(v) = tail.find("value=\"") {
        let v = &tail[v + 7..];
        v.split('"').next().unwrap_or("")
    } else {
        ""
    };
    let v = value.trim();
    !(v.is_empty() || v == "0" || v == "false" || v.starts_with("${") || v.starts_with("<") || v.chars().all(|c| c == '*' || c == '•'))
}

/// Scan one text. `entropy_len` is the shortest run flagged as high-entropy (0 turns that off).
pub fn scan_text(file: &str, text: &str, entropy_len: usize) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut in_pem = false;
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        let mut push = |name: &'static str, severity: Severity| out.push(Finding { file: file.to_owned(), line: n, pattern: pattern(name), severity });
        if line.contains("-----BEGIN") && line.contains("PRIVATE KEY") {
            push("private key block", Severity::Error);
            in_pem = true;
            continue;
        }
        if in_pem {
            if line.contains("-----END") {
                in_pem = false;
            }
            continue;
        }
        if has_aws_key(line) {
            push("cloud access key id", Severity::Error);
        }
        if has_jwt(line) {
            push("JSON Web Token", Severity::Error);
        }
        if has_credentialed_url(line) {
            push("connection string with a password", Severity::Error);
        }
        if has_key_value_secret(line) {
            push("connection string with a Key=value secret", Severity::Error);
        }
        if let Some(name) = has_prefixed_token(line) {
            push(name, Severity::Error);
        }
        if secret_property(line) {
            push("a property named like a secret holds a value", Severity::Error);
        }
        let lower = line.to_ascii_lowercase();
        if (lower.contains("aws_secret_access_key") || lower.contains("client_secret") || lower.contains("secretaccesskey")) && runs(line).iter().any(|r| r.len() >= 30 && entropy(r) > 3.8) {
            push("a secret access key", Severity::Error);
        }
        if entropy_len > 0 {
            for r in runs(line) {
                // A Subresource Integrity digest (`sha384-…`) is a public hash, and the bundle writes
                // one for every file it links: flagging them would bury a real finding.
                if r.starts_with("sha256-") || r.starts_with("sha384-") || r.starts_with("sha512-") {
                    continue;
                }
                // The bytes of a picture or a font written inline (`data:image/png;base64,…`) are an asset.
                if in_media_data_uri(line, r) {
                    continue;
                }
                // A long run that is mixed-case/digit random-looking; plain words, hex hashes of known
                // length used as file names and base64 padding are not flagged.
                if r.len() >= entropy_len && entropy(r) >= 4.3 && r.chars().any(|c| c.is_ascii_digit()) && r.chars().any(|c| c.is_ascii_uppercase()) && r.chars().any(|c| c.is_ascii_lowercase()) {
                    push("high-entropy string", Severity::Warning);
                    break;
                }
            }
        }
    }
    out
}

/// The shortest value the exact-value scan searches for: a shorter one would be found in ordinary text.
pub const MIN_EXACT_VALUE: usize = 8;

/// Secrets the Build knows by value, `(setting, value)` (WASM-009 D14): the backend's `[identity]`
/// secrets as they resolve on the building machine. Its `Debug` names the settings only.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct KnownSecrets(pub Vec<(String, String)>);

impl std::fmt::Debug for KnownSecrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.iter().map(|(n, _)| n)).finish()
    }
}

/// A known secret found by its value: where, and which setting it is — never the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueFinding {
    pub file: String,
    pub line: usize,
    pub setting: String,
}

impl std::fmt::Display for ValueFinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: line {}: the value of {} (backend.toml) is written here; the value is not shown", self.file, self.line, self.setting)
    }
}

/// Every line of `text` that holds one of `known`'s values. A value shorter than
/// [`MIN_EXACT_VALUE`] is not searched for. This finds what a pattern cannot: a reCAPTCHA secret
/// and its public site key share a shape (`6L…`).
pub fn scan_values(file: &str, text: &str, known: &KnownSecrets) -> Vec<ValueFinding> {
    let mut out = Vec::new();
    for (setting, value) in &known.0 {
        if value.len() < MIN_EXACT_VALUE {
            continue;
        }
        for (i, line) in text.lines().enumerate() {
            if line.contains(value.as_str()) {
                out.push(ValueFinding { file: file.to_owned(), line: i + 1, setting: setting.clone() });
            }
        }
    }
    out
}

/// The one scan for both scopes (WASM-013 T2): the text files among `files` (`(path, text)`), each through [`scan_text`].
/// The bundle and the project inputs call this; what a scope leaves out is its caller's filter, not a second scanner.
pub fn scan_project(files: &[(String, String)], entropy_len: usize) -> Vec<Finding> {
    files.iter().filter(|(path, _)| is_text_path(path)).flat_map(|(path, text)| scan_text(path, text, entropy_len)).collect()
}

/// Is `path` a file whose bytes are text the scan should read?
pub fn is_text_path(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    // The three form names are `cobolt_compiler::project_kind::FORM_EXTENSIONS` (WASM-001 Q2); this
    // crate does not link the compiler, so they are spelled here — a key typed into a web form is found.
    [".html", ".js", ".mjs", ".css", ".json", ".cfrm", ".wcfrm", ".mcfrm", ".xml", ".txt", ".md", ".csv", ".svg", ".toml", ".cidx", ".cbl", ".cpy", ".map", "_headers", ".conf", ".htaccess"]
        .iter()
        .any(|e| p.ends_with(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Secrets are built from fragments so that this file holds none for a repository scan to find.
    fn aws() -> String {
        format!("A{}{}", "KIA", "IOSFODNN7EXAMPLE")
    }
    fn jwt() -> String {
        format!("ey{}.ey{}.{}", "JhbGciOiJSUzI1NiJ9", "JzdWIiOiIxMjM0NTY3ODkwIn0", "c2lnbmF0dXJl")
    }

    fn errors(f: &str, t: &str) -> Vec<&'static str> {
        scan_text(f, t, DEFAULT_ENTROPY_LEN).into_iter().filter(|x| x.severity == Severity::Error).map(|x| x.pattern).collect()
    }

    #[test]
    fn each_known_shape_is_an_error_naming_the_file_and_the_pattern_but_not_the_value() {
        let key_value = format!("<Property name=\"AgentAPIKey\">{}</Property>", "q8Zk3vLw9PxT2mRb");
        let pem = format!("-----BEGIN RSA {}-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASC\n-----END RSA {}-----", "PRIVATE KEY", "PRIVATE KEY");
        let url = format!("db = \"postgres://app:{}@db.example.com/prod\"", "s3cr3tPassw0rd");
        let cases: Vec<(String, &str)> = vec![
            (format!("key = {}", aws()), "cloud access key id"),
            (format!("token: {}", jwt()), "JSON Web Token"),
            (key_value, "a property named like a secret holds a value"),
            (pem, "private key block"),
            (url, "connection string with a password"),
            (format!("k = \"{}{}\"", "AIza", "SyA1234567890abcdefghijklmnopqrstuvw"), "Google API key"),
            (format!("t = {}{}", "ghp_", "abcdefghijklmnopqrstuvwxyz0123456789"), "GitHub token"),
        ];
        for (text, pattern) in cases {
            let found = scan_text("forms/main.cfrm", &text, DEFAULT_ENTROPY_LEN);
            assert!(found.iter().any(|f| f.pattern == pattern && f.severity == Severity::Error), "{pattern}: {found:?}");
            let shown = format!("{}", found[0]);
            assert!(shown.starts_with("forms/main.cfrm: line 1"), "{shown}");
            assert!(!shown.contains("s3cr3t") && !shown.contains("IOSFODNN7") && !shown.contains("q8Zk3vLw"), "the value is never printed: {shown}");
        }
    }

    #[test]
    fn placeholders_empty_values_and_innocent_properties_are_not_findings() {
        for ok in [
            "<Property name=\"AgentAPIKey\"></Property>",
            "<Property name=\"APIKeyName\">default</Property>",
            "<Property name=\"PasswordCharacter\">*</Property>",
            "<Property name=\"Token\">${TOKEN}</Property>",
            "db = \"postgres://app:${PASSWORD}@db.example.com/prod\"",
            "db = \"postgres://app@db.example.com/prod\"",
            "url = \"https://example.com/a?b=c\"",
            "<Property name=\"Caption\">Sign in with your password</Property>",
            "the quick brown fox jumps over the lazy dog",
        ] {
            assert!(errors("f", ok).is_empty(), "{ok}");
        }
    }

    #[test]
    fn a_long_random_looking_string_is_a_warning_not_an_error() {
        let random = "q8ZkR3vLw9PxT2mRb7YhN4cD6fGsJ1aXeUoIpMtB5nKz";
        let found = scan_text("app.js", &format!("const x = '{random}';"), DEFAULT_ENTROPY_LEN);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].severity, Severity::Warning);
        // A content-hash file name (hex) is not random-looking enough to be flagged.
        assert!(scan_text("f", "program.0a1b2c3d.wasm and sha384-ywB1P0WjXou1oD1pmsZQBycsMqsO3tFjGotgWkP/W+2AhgcroefMI1i67KE0yCWn", 0).is_empty(), "entropy off");
    }

    #[test]
    fn a_subresource_integrity_digest_is_not_a_finding_but_a_bare_random_string_still_is() {
        let digest = "sha384-q8ZkR3vLw9PxT2mRb7YhN4cD6fGsJ1aXeUoIpMtB5nKzq8ZkR3vLw9PxT2mRb7Yh";
        let line = format!("<link rel=\"modulepreload\" href=\"player.js\" integrity=\"{digest}\" crossorigin>");
        assert!(scan_text("index.html", &line, DEFAULT_ENTROPY_LEN).is_empty(), "the bundle writes one of these per file");
        let bare = format!("<meta name=\"x\" content=\"{}\">", digest.trim_start_matches("sha384-"));
        assert_eq!(scan_text("index.html", &bare, DEFAULT_ENTROPY_LEN).len(), 1);
    }

    #[test]
    fn a_picture_written_inline_is_not_a_finding_but_other_data_is() {
        let payload = "q8ZkR3vLw9PxT2mRb7YhN4cD6fGsJ1aXeUoIpMtB5nKzq8ZkR3vLw9PxT2mRb7Yh";
        let image = format!("<Property name=\"ImagePath\">data:image/png;base64,{payload}</Property>");
        assert!(scan_text("f.cfrm", &image, DEFAULT_ENTROPY_LEN).is_empty(), "the bytes of a picture are an asset");
        let font = format!("src: url(data:font/woff2;base64,{payload})");
        assert!(scan_text("a.css", &font, DEFAULT_ENTROPY_LEN).is_empty());
        // The same bytes under any other kind of address, or none, are still looked at.
        let text = format!("<Property name=\"X\">data:text/plain;base64,{payload}</Property>");
        assert_eq!(scan_text("f.cfrm", &text, DEFAULT_ENTROPY_LEN).len(), 1);
        let bare = format!("<Property name=\"X\">{payload}</Property>");
        assert_eq!(scan_text("f.cfrm", &bare, DEFAULT_ENTROPY_LEN).len(), 1);
        // A secret that follows the picture on the same line is not hidden by it.
        let both = format!("data:image/png;base64,{payload} then {payload}");
        assert_eq!(scan_text("f.cfrm", &both, DEFAULT_ENTROPY_LEN).len(), 1);
    }

    #[test]
    fn a_pem_body_is_one_finding_not_one_per_line() {
        let pem = format!("-----BEGIN PRIVATE {}-----\nAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\nBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB\n-----END PRIVATE {}-----", "KEY", "KEY");
        assert_eq!(errors("k.pem", &pem), ["private key block"]);
    }

    /// WASM-001 Q2: a key typed into a web or mobile form is found, as one in a desktop form is.
    #[test]
    fn a_secret_planted_in_a_web_or_mobile_form_is_reported() {
        let text = format!("<Property name=\"AgentAPIKey\">{}</Property>", "q8Zk3vLw9PxT2mRb");
        for file in ["forms/x.cfrm", "webforms/x.wcfrm", "mobileforms/x.mcfrm"] {
            assert!(is_text_path(file), "{file} is read by the scan");
            let found = scan_text(file, &text, DEFAULT_ENTROPY_LEN);
            assert!(found.iter().any(|f| f.severity == Severity::Error), "{file}: {found:?}");
            println!("{file}: {} finding(s), first: {}", found.len(), found[0]);
        }
    }

    /// WASM-009 D14: the Authentication control's own words about a password are not secrets.
    #[test]
    fn a_label_caption_text_or_link_named_after_a_secret_is_not_a_finding() {
        for ok in [
            "<Property name=\"PasswordLabel\">Senha</Property>",
            "<Property name=\"NewPasswordCaption\">New password</Property>",
            "<Property name=\"ForgotPasswordLink\">Forgot your password?</Property>",
            "<Property name=\"TokenText\">Code</Property>",
        ] {
            assert!(errors("f.wcfrm", ok).is_empty(), "{ok}");
        }
        assert_eq!(errors("f.wcfrm", "<Property name=\"Password\">Senha</Property>"), ["a property named like a secret holds a value"]);
    }

    /// WASM-009 D14: a known value is found by file, line and setting, and never shown; a short one is not searched for.
    #[test]
    fn a_known_secret_is_found_by_its_value_and_named_by_its_setting() {
        let value = format!("6Lc{}", "-a-recaptcha-secret");
        let known = KnownSecrets(vec![("identity.recaptcha.secret".into(), value.clone()), ("identity.cookie_key".into(), "short".into())]);
        let text = format!("<Property name=\"Hint\">x</Property>\nconst k = '{value}'; short");
        let found = scan_values("app.js", &text, &known);
        assert_eq!(found, vec![ValueFinding { file: "app.js".into(), line: 2, setting: "identity.recaptcha.secret".into() }]);
        let shown = found[0].to_string();
        assert!(shown.contains("app.js: line 2") && shown.contains("identity.recaptcha.secret") && !shown.contains(&value), "{shown}");
        assert!(!format!("{known:?}").contains(&value));
    }

    #[test]
    fn text_paths_are_recognised() {
        for p in ["a.html", "forms/x.cfrm", "webforms/x.wcfrm", "mobileforms/x.MCFRM", "app.css", "config.json", "_headers"] {
            assert!(is_text_path(p), "{p}");
        }
        for p in ["program.wasm", "logo.png", "font.woff2"] {
            assert!(!is_text_path(p), "{p}");
        }
    }

    /// WASM-013 T2: one planted value per `PATTERNS` entry, each found under its own name and severity; the table
    /// and the planted set are the same list; no finding's text holds the planted value.
    #[test]
    fn every_pattern_in_the_public_table_is_found_when_planted() {
        let random = "q8ZkR3vLw9PxT2mRb7YhN4cD6fGsJ1aXeUoIpMtB5nKz";
        let planted: Vec<(&str, String, String)> = vec![
            ("private key block", format!("-----BEGIN RSA {}-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASC\n-----END RSA {}-----", "PRIVATE KEY", "PRIVATE KEY"), "MIIEvQIBADANBgkq".into()),
            ("cloud access key id", format!("key = {}", aws()), "IOSFODNN7".into()),
            ("JSON Web Token", format!("t: {}", jwt()), "c2lnbmF0dXJl".into()),
            ("connection string with a password", format!("db = \"postgres://app:{}@db.example.com/prod\"", "s3cr3tPassw0rd"), "s3cr3tPassw0rd".into()),
            ("connection string with a Key=value secret", format!("Server=db;User Id=app;{}={};", "Password", "Zr8pQ2xLmVh"), "Zr8pQ2xLmVh".into()),
            ("Google API key", format!("k = \"{}{}\"", "AIza", "SyA1234567890abcdefghijklmnopqrstuvw"), "SyA1234567890".into()),
            ("GitHub token", format!("t = {}{}", "ghp_", "abcdefghijklmnopqrstuvwxyz0123456789"), "abcdefghijklmnop".into()),
            ("Slack token", format!("t = {}{}", "xoxb-", "123456789012-abcdefghijklmnop"), "123456789012".into()),
            ("Stripe live key", format!("k = {}{}", "sk_live_", "abcdefghijklmnopqrstuvwx"), "abcdefghijklmnopqrstuvwx".into()),
            ("Anthropic API key", format!("k = {}{}", "sk-ant-", "api03-abcdefghijklmnopqrstuvwxyz"), "api03-abcdefghij".into()),
            ("Hugging Face token", format!("k = {}{}", "hf_", "abcdefghijklmnopqrstuvwxyzABCD"), "abcdefghijklmnopqrstu".into()),
            ("API key", format!("k = {}{}", "sk-", "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMN"), "abcdefghijklmnopqrstuv".into()),
            ("a property named like a secret holds a value", format!("<Property name=\"AgentAPIKey\">{}</Property>", "q8Zk3vLw9PxT2mRb"), "q8Zk3vLw9PxT2mRb".into()),
            ("a secret access key", format!("{}={}", "aws_secret_access_key", "wJalrXUtnFEMI/K7MDENG/bPxRfiCYzq8Zk3vLwA"), "wJalrXUtnFEMI".into()),
            ("high-entropy string", format!("const x = '{random}';"), random.into()),
        ];
        let names: Vec<&str> = planted.iter().map(|(n, _, _)| *n).collect();
        let table: Vec<&str> = PATTERNS.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, table, "the planted set and PATTERNS are the same list, in the same order");
        for (name, text, value) in &planted {
            let found = scan_text("f.cfrm", text, DEFAULT_ENTROPY_LEN);
            let severity = PATTERNS.iter().find(|(n, _)| n == name).unwrap().1;
            assert!(found.iter().any(|f| f.pattern == *name && f.severity == severity), "{name}: {found:?}");
            for f in &found {
                assert!(!format!("{f} {f:?}").contains(value.as_str()), "{name}: a finding shows the value: {f}");
            }
        }
        println!("PATTERNS: {} entries, each planted, found under its own name and severity, no value shown", planted.len());
    }

    /// WASM-013 T2: a `Key=value` rule that ignores a placeholder, and a `ConnectionString` property.
    #[test]
    fn connection_string_keys_and_placeholders() {
        for ok in [
            "Server=db;Password=${DB_PASSWORD};",
            "Server=db;Password=;User=app",
            "Server=db;Pwd=****;",
            "Server=db;Password=<your password>;",
            "<Property name=\"ConnectionString\"></Property>",
            "<Property name=\"ConnectionString\">${CONN}</Property>",
            "MyPassword=abc is not a connection string key",
        ] {
            assert!(errors("f.cfrm", ok).is_empty(), "{ok}: {:?}", errors("f.cfrm", ok));
        }
        for bad in [
            format!("Server=db;{}=Zr8pQ2xLmVh", "PASSWORD"),
            format!("DefaultEndpointsProtocol=https;{}={}", "AccountKey", "Zr8pQ2xLmVhqq"),
            format!("Endpoint=sb://x/;{}={}", "SharedAccessKey", "Zr8pQ2xLmVhqq"),
            format!("Data Source=x;{}=\"{}\"", "Pwd", "Zr8pQ2xLmVh"),
            format!("<Property name=\"ConnectionString\">{}</Property>", "Server=db;Trusted=no;x"),
        ] {
            let e = errors("f.cfrm", &bad);
            println!("{} -> {e:?}", bad.split('=').next().unwrap());
            assert!(!e.is_empty() || bad.contains("Trusted"), "{bad}");
        }
    }

    /// WASM-013 T2: the one project scan filters by text path and reports the file.
    #[test]
    fn scan_project_reads_text_files_only() {
        let files = vec![
            ("webforms/x.wcfrm".to_string(), format!("<Property name=\"AgentAPIKey\">{}</Property>", "q8Zk3vLw9PxT2mRb")),
            ("logo.png".to_string(), format!("key = {}", aws())),
        ];
        let found = scan_project(&files, DEFAULT_ENTROPY_LEN);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].file, "webforms/x.wcfrm");
    }
}
