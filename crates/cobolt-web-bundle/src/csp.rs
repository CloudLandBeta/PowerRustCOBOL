// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Content-Security-Policy and the response headers of a Web bundle (WASM-013 R2–R4, WASM-002
//! R14/R20, WASM-017 R19c).
//!
//! The policy is **default-deny**: scripts from the application's own origin plus
//! `'wasm-unsafe-eval'` (compiling WebAssembly) and nothing else — no `'unsafe-eval'`, no
//! `'unsafe-inline'` for scripts or styles (the page uses constructed stylesheets and CSSOM, never
//! an inline `style` attribute or `<style>` element), `connect-src` limited to the origin and the
//! configured backend addresses, `base-uri`, `form-action` and `object-src` closed, Trusted Types
//! enforced for scripts.
//!
//! A static host may not let the application set headers, so the policy is delivered twice: as a
//! `<meta>` element (what a `<meta>` can express) and as a ready-to-use header set for the common
//! hosts. [`Policy::header_only_directives`] lists what only a header can deliver
//! (`frame-ancestors`, and report-only mode), so the Build can print which is which.

use crate::{Embed, Isolation};

/// The knobs of the policy.
#[derive(Debug, Clone, Default)]
pub struct CspOptions {
    /// Extra origins the page may fetch from (`connect-src`).
    pub extra_connect: Vec<String>,
    pub extra_img: Vec<String>,
    pub extra_font: Vec<String>,
    pub extra_script: Vec<String>,
    pub extra_frame: Vec<String>,
    /// Enforce Trusted Types for scripts (default on).
    pub trusted_types: bool,
    /// Deliver the header as report-only (a rollout aid; a release build is enforcing).
    pub report_only: bool,
    /// Where violations are reported (header only).
    pub report_uri: Option<String>,
}

impl CspOptions {
    pub fn strict() -> CspOptions {
        CspOptions { trusted_types: true, ..CspOptions::default() }
    }
}

/// The built policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    /// Directives deliverable in a `<meta>` element.
    pub meta: Vec<(String, String)>,
    /// Directives only a response header can carry.
    pub header_only: Vec<(String, String)>,
    pub report_only: bool,
}

fn join(base: &[&str], extra: &[String]) -> String {
    let mut v: Vec<String> = base.iter().map(|s| (*s).to_owned()).collect();
    for e in extra {
        if !v.contains(e) {
            v.push(e.clone());
        }
    }
    v.join(" ")
}

/// Build the policy for a bundle whose backend addresses are `backends` (`wss://…`).
pub fn build(opts: &CspOptions, backends: &[String], embed: &Embed) -> Policy {
    let mut connect: Vec<String> = backends.to_vec();
    connect.extend(opts.extra_connect.iter().cloned());
    let mut meta: Vec<(String, String)> = vec![
        ("default-src".into(), "'none'".into()),
        ("script-src".into(), join(&["'self'", "'wasm-unsafe-eval'"], &opts.extra_script)),
        ("worker-src".into(), "'self'".into()),
        // No 'unsafe-inline': the page sets styles through the CSSOM and constructed stylesheets only.
        ("style-src".into(), "'self'".into()),
        ("img-src".into(), join(&["'self'", "data:", "blob:"], &opts.extra_img)),
        ("font-src".into(), join(&["'self'"], &opts.extra_font)),
        ("media-src".into(), "'self' blob: data:".into()),
        ("connect-src".into(), join(&["'self'"], &connect)),
        ("manifest-src".into(), "'self'".into()),
        ("frame-src".into(), if opts.extra_frame.is_empty() { "'none'".into() } else { join(&[], &opts.extra_frame) }),
        ("object-src".into(), "'none'".into()),
        ("base-uri".into(), "'none'".into()),
        ("form-action".into(), "'none'".into()),
    ];
    if opts.trusted_types {
        meta.push(("require-trusted-types-for".into(), "'script'".into()));
        meta.push(("trusted-types".into(), "default".into()));
    }
    let mut header_only: Vec<(String, String)> = Vec::new();
    match embed {
        Embed::Window => header_only.push(("frame-ancestors".into(), "'none'".into())),
        Embed::Element { .. } => header_only.push(("frame-ancestors".into(), "'self'".into())),
    }
    if let Some(u) = &opts.report_uri {
        header_only.push(("report-uri".into(), u.clone()));
    }
    Policy { meta, header_only, report_only: opts.report_only }
}

impl Policy {
    fn render(dirs: &[(String, String)]) -> String {
        dirs.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join("; ")
    }

    /// The text of the `<meta http-equiv="Content-Security-Policy">` (what a meta can say).
    pub fn meta_content(&self) -> String {
        Self::render(&self.meta)
    }

    /// The full header value: everything, including what only a header can deliver.
    pub fn header_value(&self) -> String {
        let mut all = self.meta.clone();
        all.extend(self.header_only.iter().cloned());
        Self::render(&all)
    }

    pub fn header_name(&self) -> &'static str {
        if self.report_only {
            "Content-Security-Policy-Report-Only"
        } else {
            "Content-Security-Policy"
        }
    }

    /// The names of the directives a `<meta>` cannot carry.
    pub fn header_only_directives(&self) -> Vec<String> {
        let mut v: Vec<String> = self.header_only.iter().map(|(k, _)| k.clone()).collect();
        if self.report_only {
            v.push("(report-only mode)".into());
        }
        v
    }
}

/// The headers every response of the bundle should carry.
pub fn security_headers(policy: &Policy, isolation: Isolation) -> Vec<(String, String)> {
    let mut h = vec![
        (policy.header_name().to_owned(), policy.header_value()),
        ("X-Content-Type-Options".into(), "nosniff".into()),
        ("Referrer-Policy".into(), "no-referrer".into()),
        ("Permissions-Policy".into(), "camera=(), microphone=(), geolocation=(), payment=(), usb=(), interest-cohort=()".into()),
    ];
    match isolation {
        Isolation::Off => {}
        Isolation::RequireCorp => {
            h.push(("Cross-Origin-Opener-Policy".into(), "same-origin".into()));
            h.push(("Cross-Origin-Embedder-Policy".into(), "require-corp".into()));
            h.push(("Cross-Origin-Resource-Policy".into(), "same-origin".into()));
        }
        Isolation::Credentialless => {
            h.push(("Cross-Origin-Opener-Policy".into(), "same-origin".into()));
            h.push(("Cross-Origin-Embedder-Policy".into(), "credentialless".into()));
            h.push(("Cross-Origin-Resource-Policy".into(), "same-origin".into()));
        }
    }
    h
}

/// The long-cache headers of the content-hashed files.
pub const IMMUTABLE_CACHE: &str = "public, max-age=31536000, immutable";

/// One file per documented static host. `(path, content)`.
pub fn host_files(headers: &[(String, String)], hashed_globs: &[&str]) -> Vec<(String, String)> {
    let mut out = Vec::new();

    // Netlify and Cloudflare Pages: `_headers`.
    let mut netlify = String::from("# Headers for Netlify and Cloudflare Pages (the same file format).\n/*\n");
    for (k, v) in headers {
        netlify.push_str(&format!("  {k}: {v}\n"));
    }
    netlify.push_str("/*.wasm\n  Content-Type: application/wasm\n");
    for g in hashed_globs {
        netlify.push_str(&format!("{g}\n  Cache-Control: {IMMUTABLE_CACHE}\n"));
    }
    netlify.push_str("/index.html\n  Cache-Control: no-cache\n/config.json\n  Cache-Control: no-cache\n");
    out.push(("_headers".to_owned(), netlify));

    // nginx: a snippet to `include` inside the server block.
    let mut nginx = String::from("# Include inside the `server { ... }` block that serves this folder.\ntypes { application/wasm wasm; }\n");
    for (k, v) in headers {
        nginx.push_str(&format!("add_header {k} \"{}\" always;\n", v.replace('"', "\\\"")));
    }
    nginx.push_str("location ~* \\.[0-9a-f]{8}\\.(js|css|wasm|cfrm|json)$ { add_header Cache-Control \"" );
    nginx.push_str(IMMUTABLE_CACHE);
    nginx.push_str("\"; }\nlocation = /index.html { add_header Cache-Control \"no-cache\"; }\n");
    out.push(("hosting/nginx.conf".to_owned(), nginx));

    // Apache: `.htaccess` (mod_headers, mod_mime).
    let mut apache = String::from("# Place beside index.html (needs mod_headers and mod_mime).\nAddType application/wasm .wasm\n<IfModule mod_headers.c>\n");
    for (k, v) in headers {
        apache.push_str(&format!("  Header always set {k} \"{}\"\n", v.replace('"', "\\\"")));
    }
    apache.push_str("  <FilesMatch \"\\.[0-9a-f]{8}\\.(js|css|wasm|cfrm|json)$\">\n    Header set Cache-Control \"");
    apache.push_str(IMMUTABLE_CACHE);
    apache.push_str("\"\n  </FilesMatch>\n</IfModule>\n");
    out.push(("hosting/.htaccess".to_owned(), apache));

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> Policy {
        build(&CspOptions::strict(), &["wss://api.example.com/ws".into()], &Embed::Window)
    }

    #[test]
    fn the_policy_is_default_deny_and_never_allows_eval_or_inline() {
        let p = policy();
        let text = p.header_value();
        assert!(text.starts_with("default-src 'none'"), "{text}");
        assert!(text.contains("script-src 'self' 'wasm-unsafe-eval'"));
        assert!(!text.contains("unsafe-eval'") || text.contains("'wasm-unsafe-eval'"));
        assert!(!text.contains("'unsafe-eval'"), "no 'unsafe-eval' (WASM-002 R14)");
        assert!(!text.contains("'unsafe-inline'"), "no 'unsafe-inline' for scripts or styles");
        assert!(text.contains("connect-src 'self' wss://api.example.com/ws"), "connect-src is the origin and the configured backend only");
        assert!(text.contains("object-src 'none'") && text.contains("base-uri 'none'") && text.contains("form-action 'none'"));
        assert!(text.contains("require-trusted-types-for 'script'") && text.contains("trusted-types default"));
        assert!(text.contains("frame-ancestors 'none'"));
    }

    #[test]
    fn the_meta_carries_everything_a_meta_can_and_the_header_adds_the_rest() {
        let p = policy();
        assert!(!p.meta_content().contains("frame-ancestors"), "a <meta> cannot deliver frame-ancestors");
        assert_eq!(p.header_only_directives(), ["frame-ancestors"]);
        assert!(p.header_value().contains("frame-ancestors 'none'"));
        let embedded = build(&CspOptions::strict(), &[], &Embed::Element { selector: "#app".into() });
        assert!(embedded.header_value().contains("frame-ancestors 'self'"));
        let ro = build(&CspOptions { report_only: true, trusted_types: true, ..Default::default() }, &[], &Embed::Window);
        assert_eq!(ro.header_name(), "Content-Security-Policy-Report-Only");
        assert!(ro.header_only_directives().contains(&"(report-only mode)".to_owned()));
    }

    #[test]
    fn extra_origins_are_added_to_the_directive_that_needs_them_and_nowhere_else() {
        let p = build(
            &CspOptions {
                extra_img: vec!["https://tile.openstreetmap.org".into()],
                extra_script: vec!["https://www.google.com/recaptcha/".into()],
                extra_frame: vec!["https://www.google.com/".into()],
                trusted_types: true,
                ..Default::default()
            },
            &[],
            &Embed::Window,
        );
        let t = p.header_value();
        assert!(t.contains("img-src 'self' data: blob: https://tile.openstreetmap.org"));
        assert!(t.contains("script-src 'self' 'wasm-unsafe-eval' https://www.google.com/recaptcha/"));
        assert!(t.contains("frame-src https://www.google.com/"));
        assert!(t.contains("connect-src 'self';") || t.contains("connect-src 'self'"));
        assert!(!t.contains("connect-src 'self' https://tile"));
    }

    #[test]
    fn isolation_headers_depend_on_the_mode_and_carry_the_trade_off() {
        let p = policy();
        let off = security_headers(&p, Isolation::Off);
        assert!(off.iter().all(|(k, _)| !k.starts_with("Cross-Origin")));
        let corp = security_headers(&p, Isolation::RequireCorp);
        assert!(corp.contains(&("Cross-Origin-Opener-Policy".into(), "same-origin".into())));
        assert!(corp.contains(&("Cross-Origin-Embedder-Policy".into(), "require-corp".into())));
        let cl = security_headers(&p, Isolation::Credentialless);
        assert!(cl.contains(&("Cross-Origin-Embedder-Policy".into(), "credentialless".into())));
    }

    #[test]
    fn host_files_exist_for_each_documented_host() {
        let h = security_headers(&policy(), Isolation::RequireCorp);
        let files = host_files(&h, &["/*.js"]);
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["_headers", "hosting/nginx.conf", "hosting/.htaccess"]);
        let netlify = &files[0].1;
        assert!(netlify.contains("Cross-Origin-Embedder-Policy: require-corp") && netlify.contains("/*.wasm\n  Content-Type: application/wasm"));
        assert!(files[1].1.contains("add_header Cross-Origin-Opener-Policy \"same-origin\" always;"));
        assert!(files[2].1.contains("Header always set Cross-Origin-Opener-Policy \"same-origin\""));
    }
}
