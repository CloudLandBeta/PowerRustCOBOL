// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **WebLink** (WASM-006): the policy that decides what a click on a link does.
//!
//! The control is link text that behaves like an `<a>` element — including the browser's own
//! conventions for modifier keys — and exposes every `<a>` capability as a property **except
//! `ping`** (operator, 2026-10-20), which is the one attribute that makes the browser send a
//! request to an address the page never fetches. Nothing here knows the DOM; the renderer asks
//! [`plan`] what to do and carries it out, so the whole policy is tested natively.
//!
//! Properties this module reads (the names WASM-006 R3 proposes; they are the contract the
//! model and the Properties panel must match): `Url`, `Target` (`Self` | `NewTab` | `NewWindow` |
//! `Parent` | `Top`), `WindowFeatures` (the `window.open` feature string, for `NewWindow`), `Rel`
//! (a space-separated list), `Download` (empty = off, else the suggested file name),
//! `HrefLang`, `MimeType`, `ReferrerPolicy`, `Underline` (`Always` | `Hover` | `Never`),
//! `AccessibleLabel`, and the colours `LinkColor`, `HoverColor`, `ActiveColor`, `VisitedColor`.
//! `Cancel` is the flag a handler sets during the click event to stop the navigation.

use crate::methods::{MethodError, Outcome};
use crate::tree::{FormTree, Kind};
use crate::value::truthy;

/// What kind of address a link holds, after the URL parser's own rules (WHATWG: leading and
/// trailing C0 controls and spaces are stripped, tabs and newlines are removed anywhere).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlKind {
    Http,
    Https,
    Mailto,
    Tel,
    Sms,
    /// `#name` — a place on this page.
    Fragment,
    /// No scheme: relative to the page (or `//host/path`, which keeps the page's scheme).
    Relative,
    /// Any other scheme, named.
    Other(String),
}

/// The URL as the browser's parser would see it before it looks for a scheme.
pub fn normalize(url: &str) -> String {
    let t = url.trim_matches(|c: char| c <= ' ');
    t.chars().filter(|c| !matches!(c, '\t' | '\n' | '\r')).collect()
}

/// The scheme of `url`, lower-cased, if it has one.
pub fn scheme_of(url: &str) -> Option<String> {
    let n = normalize(url);
    let colon = n.find(':')?;
    let s = &n[..colon];
    let mut chars = s.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() || !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        return None;
    }
    Some(s.to_ascii_lowercase())
}

pub fn kind_of(url: &str) -> UrlKind {
    let n = normalize(url);
    if n.starts_with('#') {
        return UrlKind::Fragment;
    }
    match scheme_of(&n).as_deref() {
        None => UrlKind::Relative,
        Some("http") => UrlKind::Http,
        Some("https") => UrlKind::Https,
        Some("mailto") => UrlKind::Mailto,
        Some("tel") => UrlKind::Tel,
        Some("sms") => UrlKind::Sms,
        Some(other) => UrlKind::Other(other.to_owned()),
    }
}

/// Why an address is refused (R7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub scheme: String,
    pub message: String,
}

/// The schemes a project may widen the allow-list with, beyond the defaults. The defaults are
/// never widened by default (R7): a project names what it adds.
#[derive(Debug, Clone, Default)]
pub struct Policy {
    pub extra_schemes: Vec<String>,
}

/// Check an address against R7: `http`, `https`, `mailto`, `tel`, `sms`, relative and fragment are
/// accepted; `javascript:`, `vbscript:` and `data:` are refused whatever else is configured (a
/// `data:` address is allowed only for a `Download` of a generated file, `allow_data`); any other
/// scheme is refused unless the project listed it.
pub fn check(url: &str, allow_data: bool, policy: &Policy) -> Result<UrlKind, Refused> {
    let kind = kind_of(url);
    match &kind {
        UrlKind::Other(s) => {
            let always = matches!(s.as_str(), "javascript" | "vbscript");
            if always || (s == "data" && !allow_data) {
                return Err(Refused {
                    scheme: s.clone(),
                    message: format!("the address uses the `{s}:` scheme, which a WebLink never follows"),
                });
            }
            if s == "data" || s == "blob" {
                // Only reachable with allow_data (a Download of a generated file).
                return Ok(kind);
            }
            if policy.extra_schemes.iter().any(|x| x.eq_ignore_ascii_case(s)) {
                return Ok(kind);
            }
            Err(Refused {
                scheme: s.clone(),
                message: format!("the `{s}:` scheme is not on this project's list of link schemes"),
            })
        }
        _ => Ok(kind),
    }
}

/// How the new page is opened (WASM-006 R3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    SelfFrame,
    NewTab,
    NewWindow,
    Parent,
    Top,
}

impl Target {
    pub fn parse(s: &str) -> Target {
        match s.trim().to_ascii_lowercase().as_str() {
            "newtab" | "_blank" | "blank" => Target::NewTab,
            "newwindow" => Target::NewWindow,
            "parent" | "_parent" => Target::Parent,
            "top" | "_top" => Target::Top,
            _ => Target::SelfFrame,
        }
    }
    /// The `target` attribute / `window.open` name.
    pub fn html(self) -> &'static str {
        match self {
            Target::SelfFrame => "_self",
            Target::NewTab | Target::NewWindow => "_blank",
            Target::Parent => "_parent",
            Target::Top => "_top",
        }
    }
}

/// What the user pressed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Click {
    pub ctrl: bool,
    pub meta: bool,
    pub shift: bool,
    pub alt: bool,
    /// 0 primary, 1 middle.
    pub button: u8,
}

impl Click {
    /// The modifier keys as the click event reports them to the handler (WASM-006 R5).
    pub fn modifiers_text(&self) -> String {
        let mut v = Vec::new();
        if self.ctrl {
            v.push("Ctrl");
        }
        if self.meta {
            v.push("Meta");
        }
        if self.shift {
            v.push("Shift");
        }
        if self.alt {
            v.push("Alt");
        }
        v.join(",")
    }
}

/// What to do about a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavPlan {
    pub url: String,
    pub target: Target,
    /// The effective `rel`: `noopener noreferrer` when a new context opens and `Rel` is empty (R8).
    pub rel: String,
    pub features: String,
    /// `Some(name)` to download instead of navigating.
    pub download: Option<String>,
    pub referrer_policy: String,
}

/// Everything about a link a click needs, read from the control's properties.
#[derive(Debug, Clone, Default)]
pub struct LinkProps {
    pub url: String,
    pub target: String,
    pub features: String,
    pub rel: String,
    pub download: String,
    pub referrer_policy: String,
}

impl LinkProps {
    pub fn read(tree: &FormTree, ctl: usize) -> LinkProps {
        let g = |k: &str| tree.get(ctl, k);
        LinkProps {
            url: g("Url"),
            target: g("Target"),
            features: g("WindowFeatures"),
            rel: g("Rel"),
            download: g("Download"),
            referrer_policy: g("ReferrerPolicy"),
        }
    }
}

/// The file name a download suggests: the `Download` property, else the address's last path segment.
fn download_name(props: &LinkProps) -> String {
    let d = props.download.trim();
    if !d.is_empty() && !truthy(d) {
        return d.to_owned();
    }
    let n = normalize(&props.url);
    let path = n.split(['?', '#']).next().unwrap_or("");
    path.rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or("download").to_owned()
}

/// Decide what a click does, or `None` when nothing must happen (an empty or refused address).
///
/// A plain click follows `Target`; Ctrl/Cmd-click and middle-click open a new tab; Shift-click a
/// new window; Alt-click downloads (the browser's convention). `Download` set on the link makes a
/// plain click download too.
pub fn plan(props: &LinkProps, click: Click, policy: &Policy) -> Result<Option<NavPlan>, Refused> {
    if normalize(&props.url).is_empty() {
        return Ok(None);
    }
    let wants_download = !props.download.trim().is_empty() && props.download.trim() != "0";
    check(&props.url, wants_download, policy)?;
    let mut target = Target::parse(&props.target);
    let mut download = wants_download.then(|| download_name(props));
    if click.button == 1 || click.ctrl || click.meta {
        target = Target::NewTab;
    } else if click.shift {
        target = Target::NewWindow;
    } else if click.alt {
        download = Some(download_name(props));
    }
    let opens_new = matches!(target, Target::NewTab | Target::NewWindow);
    let rel = if props.rel.trim().is_empty() && opens_new { "noopener noreferrer".to_owned() } else { props.rel.trim().to_owned() };
    Ok(Some(NavPlan {
        url: normalize(&props.url),
        target,
        rel,
        features: if target == Target::NewWindow { props.features.trim().to_owned() } else { String::new() },
        download,
        referrer_policy: props.referrer_policy.trim().to_owned(),
    }))
}

/// The methods of the control (WASM-006 R12): `SetUrl`, `GetUrl`, `Open`, `OpenIn(target)`.
pub fn method(tree: &mut FormTree, ctl: usize, m: &str, args: &[&str]) -> Result<Outcome, MethodError> {
    debug_assert!(matches!(tree.ctls[ctl].kind, Kind::Control(_)));
    let arg = |i: usize| args.get(i).map(|s| s.trim().to_owned()).unwrap_or_default();
    let mut out = Outcome::default();
    match m {
        "GETURL" => out.value = Some(tree.get(ctl, "Url")),
        "SETURL" => {
            let url = arg(0);
            let allow_data = !tree.get(ctl, "Download").trim().is_empty();
            if let Err(r) = check(&url, allow_data, &Policy::default()) {
                return Err(MethodError::Refused(format!("SetUrl refused: {}", r.message)));
            }
            out.events.extend(tree.set(ctl, "Url", &url));
        }
        "OPEN" => tree.command(ctl, "open", Vec::new()),
        "OPENIN" => tree.command(ctl, "open-in", vec![arg(0)]),
        _ => unreachable!("routed by the caller"),
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(url: &str) -> LinkProps {
        LinkProps { url: url.into(), ..Default::default() }
    }

    #[test]
    fn dangerous_schemes_are_refused_however_they_are_spelled() {
        let p = Policy::default();
        for bad in [
            "javascript:alert(1)",
            "  JavaScript:alert(1)",
            "jav\tascript:alert(1)",
            "java\nscript:alert(1)",
            "\u{1}javascript:alert(1)",
            "vbscript:msgbox(1)",
            "data:text/html,<script>alert(1)</script>",
            "DATA:text/html;base64,AAAA",
            "file:///etc/passwd",
            "intent://x#Intent;end",
        ] {
            assert!(check(bad, false, &p).is_err(), "{bad:?} must be refused");
        }
        for ok in ["https://example.com", "http://example.com/a?b=c#d", "mailto:a@b.c?subject=Hi&body=x", "tel:+5511999999999", "sms:+5511999999999", "relative/page.html", "../x", "#section", "//cdn.example.com/x", "HTTPS://EXAMPLE.COM"] {
            assert!(check(ok, false, &p).is_ok(), "{ok:?} must be accepted");
        }
        // A `data:` address is allowed for a Download of a generated file, never otherwise.
        assert!(check("data:text/plain,hello", true, &p).is_ok());
        assert!(check("javascript:1", true, &p).is_err(), "never, not even for a download");
    }

    #[test]
    fn a_project_can_widen_the_list_but_not_for_the_dangerous_schemes() {
        let p = Policy { extra_schemes: vec!["ftp".into(), "javascript".into()] };
        assert!(check("ftp://x/y", false, &p).is_ok());
        assert!(check("javascript:1", false, &p).is_err());
        assert!(check("ftp://x/y", false, &Policy::default()).is_err(), "never widened by default");
    }

    #[test]
    fn a_plain_click_follows_the_target_and_new_contexts_get_noopener_noreferrer() {
        let mut p = props("https://example.com/");
        p.target = "NewTab".into();
        let plan_ = plan(&p, Click::default(), &Policy::default()).unwrap().unwrap();
        assert_eq!(plan_.target, Target::NewTab);
        assert_eq!(plan_.rel, "noopener noreferrer", "R8: empty Rel on a new tab");
        // Set explicitly, Rel is as set (the developer can loosen it).
        p.rel = "external".into();
        assert_eq!(plan(&p, Click::default(), &Policy::default()).unwrap().unwrap().rel, "external");
        // A same-context navigation carries no rel of its own.
        p.target = "Self".into();
        p.rel = String::new();
        let q = plan(&p, Click::default(), &Policy::default()).unwrap().unwrap();
        assert_eq!((q.target, q.rel.as_str()), (Target::SelfFrame, ""));
    }

    #[test]
    fn modifier_keys_follow_the_browsers_conventions() {
        let p = props("https://example.com/files/report.pdf");
        let pol = Policy::default();
        let ctrl = plan(&p, Click { ctrl: true, ..Default::default() }, &pol).unwrap().unwrap();
        assert_eq!(ctrl.target, Target::NewTab);
        let meta = plan(&p, Click { meta: true, ..Default::default() }, &pol).unwrap().unwrap();
        assert_eq!(meta.target, Target::NewTab, "Command on macOS");
        let middle = plan(&p, Click { button: 1, ..Default::default() }, &pol).unwrap().unwrap();
        assert_eq!(middle.target, Target::NewTab);
        let shift = plan(&p, Click { shift: true, ..Default::default() }, &pol).unwrap().unwrap();
        assert_eq!(shift.target, Target::NewWindow);
        let alt = plan(&p, Click { alt: true, ..Default::default() }, &pol).unwrap().unwrap();
        assert_eq!(alt.download.as_deref(), Some("report.pdf"), "Alt-click downloads, named from the path");
        assert_eq!(Click { ctrl: true, shift: true, ..Default::default() }.modifiers_text(), "Ctrl,Shift");
    }

    #[test]
    fn download_and_window_features_and_referrer_policy_are_carried_through() {
        let mut p = props("https://example.com/a.csv");
        p.download = "export.csv".into();
        p.referrer_policy = "no-referrer".into();
        let plan_ = plan(&p, Click::default(), &Policy::default()).unwrap().unwrap();
        assert_eq!(plan_.download.as_deref(), Some("export.csv"));
        assert_eq!(plan_.referrer_policy, "no-referrer");
        let mut w = props("https://example.com/");
        w.target = "NewWindow".into();
        w.features = "width=400,height=300".into();
        let plan_ = plan(&w, Click::default(), &Policy::default()).unwrap().unwrap();
        assert_eq!((plan_.target, plan_.features.as_str()), (Target::NewWindow, "width=400,height=300"));
    }

    #[test]
    fn an_empty_address_does_nothing_and_a_refused_one_is_reported() {
        assert_eq!(plan(&props("  "), Click::default(), &Policy::default()), Ok(None));
        let e = plan(&props("javascript:alert(1)"), Click::default(), &Policy::default()).unwrap_err();
        assert_eq!(e.scheme, "javascript");
    }

    #[test]
    fn set_url_refuses_javascript_and_leaves_the_control_unchanged() {
        let mut t = crate::tree::testing::tree();
        let ctl = t.by_name("TXT-IN").unwrap();
        t.set(ctl, "Url", "https://ok.example/");
        let r = method(&mut t, ctl, "SETURL", &["javascript:alert(1)"]);
        assert!(matches!(r, Err(MethodError::Refused(_))));
        assert_eq!(t.get(ctl, "Url"), "https://ok.example/");
        method(&mut t, ctl, "SETURL", &["https://other.example/"]).unwrap();
        assert_eq!(method(&mut t, ctl, "GETURL", &[]).unwrap().value.unwrap(), "https://other.example/");
    }
}
