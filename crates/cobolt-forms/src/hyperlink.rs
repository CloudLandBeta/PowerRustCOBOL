// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Hyperlink** (WASM-006): the policy that decides what a click on a link does.
//!
//! The control is link text that behaves like an `<a>` element — including the browser's own
//! conventions for modifier keys — and exposes every `<a>` capability as a property **except
//! `ping`** (operator, 2026-10-20), which is the one attribute that makes the browser send a
//! request to an address the page never fetches.
//!
//! One policy for every writer and every host (plan D4): the browser player, the desktop form host,
//! the interpreter's `SetUrl` and the Designer's Properties panel all call [`check`] and [`plan`].
//! Pure functions — no egui, no I/O — so the module builds for `wasm32` and is tested natively.
//!
//! Properties the policy reads: `Url`, `Target` (`Self` | `NewTab` | `NewWindow` | `Parent` |
//! `Top`), `WindowFeatures` (the `window.open` feature string, for `NewWindow`), `Rel` (a
//! space-separated list), `Download` (empty = off, else the suggested file name), `ReferrerPolicy`.
//!
//! Addresses go through the WHATWG URL parser (the `url` crate, Q6): percent-encoding and IDN are the
//! parser's, and the parsed form is what is checked and followed ([`parse`]).

/// The properties a Hyperlink adds to the Label's, with their seeds (R3), in the order the
/// Properties panel shows them. **No `Ping`** (operator, 2026-10-20).
///
/// `LinkUnderline` and not `Underline`: every control already has a Boolean `Underline` (the font's),
/// which the painters and the themes read as a font flag; the link's three-state rule is its own
/// property so the two never collide.
pub const SEEDS: &[(&str, &str)] = &[
    ("Url", ""),
    ("Target", "Self"),
    ("WindowFeatures", ""),
    ("Rel", ""),
    ("Download", ""),
    ("HrefLang", ""),
    ("MimeType", ""),
    ("ReferrerPolicy", ""),
    ("LinkUnderline", "Always"),
    ("AccessibleLabel", ""),
    ("LinkColor", ""),
    ("HoverColor", ""),
    ("ActiveColor", ""),
    ("VisitedColor", ""),
];

/// The values of `Target`.
pub const TARGETS: &[&str] = &["Self", "NewTab", "NewWindow", "Parent", "Top"];
/// The values of `LinkUnderline`.
pub const UNDERLINES: &[&str] = &["Always", "Hover", "Never"];
/// The values of `ReferrerPolicy` (HTML's eight, and empty for the browser's default).
pub const REFERRER_POLICIES: &[&str] = &[
    "",
    "no-referrer",
    "no-referrer-when-downgrade",
    "origin",
    "origin-when-cross-origin",
    "same-origin",
    "strict-origin",
    "strict-origin-when-cross-origin",
    "unsafe-url",
];

/// When the link text is underlined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Underline {
    Always,
    Hover,
    Never,
}

impl Underline {
    pub fn parse(s: &str) -> Underline {
        match s.trim().to_ascii_lowercase().as_str() {
            "hover" => Underline::Hover,
            "never" | "none" | "0" | "false" => Underline::Never,
            _ => Underline::Always,
        }
    }
    /// Is the text underlined in this state?
    pub fn shows(self, hovered: bool) -> bool {
        match self {
            Underline::Always => true,
            Underline::Hover => hovered,
            Underline::Never => false,
        }
    }
}

/// The colours of a link's states when its own are empty — the theme's link colours, one pair for a
/// light surface and one for a dark one: `(link, hover, active, visited)`.
pub fn link_colors(dark_surface: bool) -> (&'static str, &'static str, &'static str, &'static str) {
    if dark_surface {
        ("#6AA6FF", "#9CC3FF", "#FF7A7A", "#B58CFF")
    } else {
        ("#1A5FD0", "#0B3F99", "#C62828", "#6A2EB8")
    }
}

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
    t.chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect()
}

/// The address in the URL parser's form (Q6): an absolute address is parsed and serialised
/// (percent-encoded, an internationalised host in its ASCII form); a relative one is joined to
/// `base` when there is one, otherwise kept as normalised text for the platform to resolve; a
/// fragment (`#name`) is kept as written. An address the parser rejects is kept as normalised text,
/// so [`check`] still sees its scheme.
pub fn parse(url: &str, base: Option<&str>) -> String {
    let n = normalize(url);
    if n.is_empty() || n.starts_with('#') {
        return n;
    }
    match url::Url::parse(&n) {
        Ok(u) => u.to_string(),
        Err(url::ParseError::RelativeUrlWithoutBase) => match base
            .and_then(|b| url::Url::parse(b).ok())
            .and_then(|b| b.join(&n).ok())
        {
            Some(u) => u.to_string(),
            None => n,
        },
        Err(_) => n,
    }
}

/// The scheme of `url`, lower-cased, if it has one.
pub fn scheme_of(url: &str) -> Option<String> {
    let n = normalize(url);
    let colon = n.find(':')?;
    let s = &n[..colon];
    let mut chars = s.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic()
        || !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Policy {
    pub extra_schemes: Vec<String>,
}

/// The project's extra schemes for every Hyperlink this process hosts (`[links] schemes`, WASM-006
/// R7): published once by whoever knows the project — `rcrun run-form`, a built application's main —
/// and read by the interpreter, the run-form renderer and the opener alike. First call wins.
static PROJECT_SCHEMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

/// Publish the project's extra link schemes (first call wins; a child form cannot replace them).
pub fn publish_project_schemes(schemes: Vec<String>) {
    let _ = PROJECT_SCHEMES.set(schemes);
}

/// The published extra schemes; none when the process never had a project.
pub fn project_schemes() -> &'static [String] {
    PROJECT_SCHEMES.get().map(|v| v.as_slice()).unwrap_or(&[])
}

/// The policy every Hyperlink of this process is checked against.
pub fn project_policy() -> Policy {
    Policy {
        extra_schemes: project_schemes().to_vec(),
    }
}

/// Check an address against R7: `http`, `https`, `mailto`, `tel`, `sms`, relative and fragment are
/// accepted; `javascript:`, `vbscript:` and `data:` are refused whatever else is configured (a
/// `data:` address is allowed only for a `Download` of a generated file, `allow_data`); any other
/// scheme is refused unless the project listed it. The address is checked in its parsed form.
pub fn check(url: &str, allow_data: bool, policy: &Policy) -> Result<UrlKind, Refused> {
    let kind = kind_of(&parse(url, None));
    match &kind {
        UrlKind::Other(s) => {
            let always = matches!(s.as_str(), "javascript" | "vbscript");
            if always || (s == "data" && !allow_data) {
                return Err(Refused {
                    scheme: s.clone(),
                    message: format!(
                        "the address uses the `{s}:` scheme, which a Hyperlink never follows"
                    ),
                });
            }
            if s == "data" || s == "blob" {
                // Only reachable with allow_data (a Download of a generated file).
                return Ok(kind);
            }
            if policy
                .extra_schemes
                .iter()
                .any(|x| x.eq_ignore_ascii_case(s))
            {
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
    /// The property value (`Self`, `NewTab`, …).
    pub fn name(self) -> &'static str {
        match self {
            Target::SelfFrame => "Self",
            Target::NewTab => "NewTab",
            Target::NewWindow => "NewWindow",
            Target::Parent => "Parent",
            Target::Top => "Top",
        }
    }
}

/// Whose modifier-key conventions apply (Q2, operator 2026-10-22): Apple platforms follow Safari,
/// Linux and Windows follow Chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// macOS, iOS, iPadOS — Safari's table.
    Apple,
    /// Linux, Windows and the rest — Chrome's table.
    Other,
}

impl Platform {
    /// From an operating-system name as `std::env::consts::OS` spells it.
    pub fn from_os(os: &str) -> Platform {
        match os.to_ascii_lowercase().as_str() {
            "macos" | "ios" | "ipados" => Platform::Apple,
            _ => Platform::Other,
        }
    }
    /// From the browser's `navigator.userAgentData.platform` / `navigator.platform`
    /// (`macOS`, `MacIntel`, `iPhone`, `iPad`, `Win32`, `Linux x86_64`, …).
    pub fn from_navigator(platform: &str) -> Platform {
        let p = platform.to_ascii_lowercase();
        if p.contains("mac")
            || p.contains("iphone")
            || p.contains("ipad")
            || p.contains("ipod")
            || p == "ios"
        {
            Platform::Apple
        } else {
            Platform::Other
        }
    }
    /// The platform this program runs on.
    pub fn native() -> Platform {
        Platform::from_os(std::env::consts::OS)
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

/// What a click asks for once the platform's table has read the modifier keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Intent {
    Follow,
    NewTab,
    NewWindow,
    Download,
}

/// The modifier-key table (plan D3).
///
/// * **Chrome** (Linux, Windows): Ctrl-click and middle-click → new tab; Shift-click → new window;
///   Alt-click → download.
/// * **Safari** (macOS, iOS, iPadOS), from Apple's *Safari keyboard and other shortcuts* page
///   (checked 2026-10-23): ⌘-click → new tab; ⇧⌘-click → new tab, made active; ⌥-click → download;
///   Shift-click is Safari's "add to Reading List", which a page cannot do → a plain click; ⌘⌥-click
///   is read as ⌘-click. Middle-click keeps Chrome's row (not on Apple's page). Ctrl-click is the
///   secondary click on a Mac and never reaches a link as a click.
fn intent(click: Click, platform: Platform) -> Intent {
    match platform {
        Platform::Other => {
            if click.button == 1 || click.ctrl {
                Intent::NewTab
            } else if click.shift {
                Intent::NewWindow
            } else if click.alt {
                Intent::Download
            } else {
                Intent::Follow
            }
        }
        Platform::Apple => {
            if click.button == 1 || click.meta {
                Intent::NewTab
            } else if click.alt {
                Intent::Download
            } else {
                Intent::Follow
            }
        }
    }
}

/// What to do about a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavPlan {
    /// The address in its parsed form ([`parse`]).
    pub url: String,
    pub target: Target,
    /// The effective `rel`: `noopener noreferrer` when a new context opens and `Rel` is empty (R8).
    pub rel: String,
    pub features: String,
    /// `Some(name)` to download instead of navigating.
    pub download: Option<String>,
    pub referrer_policy: String,
}

impl NavPlan {
    /// Does the plan open a new browsing context (a tab or a window)?
    pub fn opens_new(&self) -> bool {
        matches!(self.target, Target::NewTab | Target::NewWindow)
    }
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
    /// Read the properties through `get` (a property name in, its value as text out).
    pub fn read_with(get: impl Fn(&str) -> String) -> LinkProps {
        LinkProps {
            url: get("Url"),
            target: get("Target"),
            features: get("WindowFeatures"),
            rel: get("Rel"),
            download: get("Download"),
            referrer_policy: get("ReferrerPolicy"),
        }
    }
    /// Read them from a model control.
    pub fn of(ctrl: &crate::model::Control) -> LinkProps {
        LinkProps::read_with(|k| {
            ctrl.get_prop(k)
                .map(|v| v.as_str().to_string())
                .unwrap_or_default()
        })
    }
}

/// `Download` set to a yes-word asks for a download named from the address.
fn truthy(s: &str) -> bool {
    let t = s.trim();
    t == "1"
        || t.eq_ignore_ascii_case("true")
        || t.eq_ignore_ascii_case("yes")
        || t.eq_ignore_ascii_case("on")
}

/// Is a download asked for (a `Download` that is neither empty nor `0`)?
pub fn wants_download(download: &str) -> bool {
    let d = download.trim();
    !d.is_empty() && d != "0" && !d.eq_ignore_ascii_case("false")
}

/// The file name a download suggests: the `Download` property, else the address's last path segment.
fn download_name(props: &LinkProps) -> String {
    let d = props.download.trim();
    if !d.is_empty() && !truthy(d) {
        return d.to_owned();
    }
    let n = normalize(&props.url);
    let path = n.split(['?', '#']).next().unwrap_or("");
    path.rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("download")
        .to_owned()
}

/// Decide what a click does, or `None` when nothing must happen (an empty address).
///
/// A plain click follows `Target`; the modifier keys are read by `platform`'s table ([`intent`]).
/// `Download` set on the link makes a plain click download too.
pub fn plan(
    props: &LinkProps,
    click: Click,
    platform: Platform,
    policy: &Policy,
) -> Result<Option<NavPlan>, Refused> {
    if normalize(&props.url).is_empty() {
        return Ok(None);
    }
    let wants = wants_download(&props.download);
    check(&props.url, wants, policy)?;
    let mut target = Target::parse(&props.target);
    let mut download = wants.then(|| download_name(props));
    match intent(click, platform) {
        Intent::Follow => {}
        Intent::NewTab => target = Target::NewTab,
        Intent::NewWindow => target = Target::NewWindow,
        Intent::Download => download = Some(download_name(props)),
    }
    let opens_new = matches!(target, Target::NewTab | Target::NewWindow);
    let rel = if props.rel.trim().is_empty() && opens_new {
        "noopener noreferrer".to_owned()
    } else {
        props.rel.trim().to_owned()
    };
    Ok(Some(NavPlan {
        url: parse(&props.url, None),
        target,
        rel,
        features: if target == Target::NewWindow {
            props.features.trim().to_owned()
        } else {
            String::new()
        },
        download,
        referrer_policy: props.referrer_policy.trim().to_owned(),
    }))
}

/// Does `rel` keep the opened page from reaching back to this one? (`noopener` or `noreferrer`
/// in the list — `noreferrer` implies `noopener` in every browser.)
pub fn rel_isolates(rel: &str) -> bool {
    rel.split_ascii_whitespace()
        .any(|t| t.eq_ignore_ascii_case("noopener") || t.eq_ignore_ascii_case("noreferrer"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(url: &str) -> LinkProps {
        LinkProps {
            url: url.into(),
            ..Default::default()
        }
    }

    fn other(p: &LinkProps, c: Click) -> NavPlan {
        plan(p, c, Platform::Other, &Policy::default())
            .unwrap()
            .unwrap()
    }

    fn apple(p: &LinkProps, c: Click) -> NavPlan {
        plan(p, c, Platform::Apple, &Policy::default())
            .unwrap()
            .unwrap()
    }

    #[test]
    fn dangerous_schemes_are_refused_however_they_are_spelled() {
        let p = Policy::default();
        for bad in [
            "javascript:alert(1)",
            "  JavaScript:alert(1)",
            "JaVaScRiPt:alert(1)",
            "\tjavascript:alert(1)",
            "jav\tascript:alert(1)",
            "java\nscript:alert(1)",
            "\u{1}javascript:alert(1)",
            "vbscript:msgbox(1)",
            "VBScript:msgbox(1)",
            "data:text/html,<script>alert(1)</script>",
            "DATA:text/html;base64,AAAA",
            "file:///etc/passwd",
            "intent://x#Intent;end",
        ] {
            let r = check(bad, false, &p);
            println!("{bad:?} -> parsed {:?} -> {r:?}", parse(bad, None));
            assert!(r.is_err(), "{bad:?} must be refused");
        }
        for ok in [
            "https://example.com",
            "http://example.com/a?b=c#d",
            "mailto:a@b.c?subject=Hi&body=x",
            "tel:+5511999999999",
            "sms:+5511999999999",
            "relative/page.html",
            "../x",
            "#section",
            "//cdn.example.com/x",
            "HTTPS://EXAMPLE.COM",
        ] {
            assert!(check(ok, false, &p).is_ok(), "{ok:?} must be accepted");
        }
        // A `data:` address is allowed for a Download of a generated file, never otherwise.
        assert!(check("data:text/plain,hello", true, &p).is_ok());
        assert!(
            check("javascript:1", true, &p).is_err(),
            "never, not even for a download"
        );
    }

    #[test]
    fn a_project_can_widen_the_list_but_not_for_the_dangerous_schemes() {
        let p = Policy {
            extra_schemes: vec!["ftp".into(), "javascript".into()],
        };
        assert!(check("ftp://x/y", false, &p).is_ok());
        assert!(check("javascript:1", false, &p).is_err());
        assert!(
            check("ftp://x/y", false, &Policy::default()).is_err(),
            "never widened by default"
        );
    }

    #[test]
    fn a_plain_click_follows_the_target_and_new_contexts_get_noopener_noreferrer() {
        let mut p = props("https://example.com/");
        p.target = "NewTab".into();
        let plan_ = other(&p, Click::default());
        assert_eq!(plan_.target, Target::NewTab);
        assert_eq!(
            plan_.rel, "noopener noreferrer",
            "R8: empty Rel on a new tab"
        );
        assert!(rel_isolates(&plan_.rel));
        // Set explicitly, Rel is as set (the developer can loosen it).
        p.rel = "external".into();
        let loose = other(&p, Click::default());
        assert_eq!(loose.rel, "external");
        assert!(!rel_isolates(&loose.rel));
        // A same-context navigation carries no rel of its own.
        p.target = "Self".into();
        p.rel = String::new();
        let q = other(&p, Click::default());
        assert_eq!((q.target, q.rel.as_str()), (Target::SelfFrame, ""));
        for t in [
            Target::SelfFrame,
            Target::NewTab,
            Target::NewWindow,
            Target::Parent,
            Target::Top,
        ] {
            assert_eq!(Target::parse(t.name()), t);
        }
    }

    #[test]
    fn modifier_keys_follow_chrome_on_linux_and_windows() {
        for os in ["linux", "windows"] {
            assert_eq!(Platform::from_os(os), Platform::Other);
        }
        let p = props("https://example.com/files/report.pdf");
        assert_eq!(
            other(
                &p,
                Click {
                    ctrl: true,
                    ..Default::default()
                }
            )
            .target,
            Target::NewTab
        );
        assert_eq!(
            other(
                &p,
                Click {
                    ctrl: true,
                    shift: true,
                    ..Default::default()
                }
            )
            .target,
            Target::NewTab,
            "Ctrl+Shift: a new tab, made active"
        );
        assert_eq!(
            other(
                &p,
                Click {
                    button: 1,
                    ..Default::default()
                }
            )
            .target,
            Target::NewTab
        );
        assert_eq!(
            other(
                &p,
                Click {
                    shift: true,
                    ..Default::default()
                }
            )
            .target,
            Target::NewWindow
        );
        let alt = other(
            &p,
            Click {
                alt: true,
                ..Default::default()
            },
        );
        assert_eq!(
            alt.download.as_deref(),
            Some("report.pdf"),
            "Alt-click downloads, named from the path"
        );
        assert_eq!(
            Click {
                ctrl: true,
                shift: true,
                ..Default::default()
            }
            .modifiers_text(),
            "Ctrl,Shift"
        );
    }

    #[test]
    fn modifier_keys_follow_safari_on_apple_platforms() {
        for os in ["macos", "ios"] {
            assert_eq!(Platform::from_os(os), Platform::Apple);
        }
        for nav in ["MacIntel", "macOS", "iPhone", "iPad"] {
            assert_eq!(Platform::from_navigator(nav), Platform::Apple, "{nav}");
        }
        for nav in ["Win32", "Windows", "Linux x86_64", "Android", ""] {
            assert_eq!(Platform::from_navigator(nav), Platform::Other, "{nav}");
        }
        let p = props("https://example.com/files/report.pdf");
        assert_eq!(
            apple(
                &p,
                Click {
                    meta: true,
                    ..Default::default()
                }
            )
            .target,
            Target::NewTab,
            "⌘-click"
        );
        assert_eq!(
            apple(
                &p,
                Click {
                    meta: true,
                    shift: true,
                    ..Default::default()
                }
            )
            .target,
            Target::NewTab,
            "⇧⌘-click: a new active tab"
        );
        let cmd_opt = apple(
            &p,
            Click {
                meta: true,
                alt: true,
                ..Default::default()
            },
        );
        assert_eq!(
            (cmd_opt.target, cmd_opt.download.as_deref()),
            (Target::NewTab, None),
            "⌘⌥-click is a new tab, not a new window"
        );
        let shift = apple(
            &p,
            Click {
                shift: true,
                ..Default::default()
            },
        );
        assert_eq!(
            (shift.target, shift.download.as_deref()),
            (Target::SelfFrame, None),
            "Shift-click on Apple = a plain click"
        );
        assert_eq!(
            apple(
                &p,
                Click {
                    alt: true,
                    ..Default::default()
                }
            )
            .download
            .as_deref(),
            Some("report.pdf"),
            "⌥-click downloads"
        );
        assert_eq!(
            apple(
                &p,
                Click {
                    button: 1,
                    ..Default::default()
                }
            )
            .target,
            Target::NewTab,
            "middle-click keeps Chrome's row"
        );
    }

    #[test]
    fn download_and_window_features_and_referrer_policy_are_carried_through() {
        let mut p = props("https://example.com/a.csv");
        p.download = "export.csv".into();
        p.referrer_policy = "no-referrer".into();
        let plan_ = other(&p, Click::default());
        assert_eq!(plan_.download.as_deref(), Some("export.csv"));
        assert_eq!(plan_.referrer_policy, "no-referrer");
        let mut w = props("https://example.com/");
        w.target = "NewWindow".into();
        w.features = "width=400,height=300".into();
        let plan_ = other(&w, Click::default());
        assert_eq!(
            (plan_.target, plan_.features.as_str()),
            (Target::NewWindow, "width=400,height=300")
        );
    }

    #[test]
    fn an_empty_address_does_nothing_and_a_refused_one_is_reported() {
        assert_eq!(
            plan(
                &props("  "),
                Click::default(),
                Platform::Other,
                &Policy::default()
            ),
            Ok(None)
        );
        let e = plan(
            &props("javascript:alert(1)"),
            Click::default(),
            Platform::Other,
            &Policy::default(),
        )
        .unwrap_err();
        assert_eq!(e.scheme, "javascript");
    }

    #[test]
    fn addresses_go_through_the_url_parser() {
        let pct = parse("https://example.com/a b?q=ä", None);
        println!("percent-encoded: {pct}");
        assert_eq!(pct, "https://example.com/a%20b?q=%C3%A4");
        let idn = parse("https://bücher.example/", None);
        println!("IDN host: {idn}");
        assert_eq!(
            idn, "https://xn--bcher-kva.example/",
            "the host in its parsed form"
        );
        assert_eq!(parse("HTTPS://EXAMPLE.COM", None), "https://example.com/");
        let rel = parse(
            "../docs/help.html",
            Some("https://app.example/forms/main/index.html"),
        );
        println!("relative against a base: {rel}");
        assert_eq!(rel, "https://app.example/forms/docs/help.html");
        assert_eq!(
            parse("relative/page.html", None),
            "relative/page.html",
            "no base: kept for the platform"
        );
        assert_eq!(parse("  #section ", None), "#section");
        assert_eq!(
            parse("mailto:a@b.c?subject=Hello World", None),
            "mailto:a@b.c?subject=Hello%20World"
        );
        // The plan carries the parsed form.
        assert_eq!(
            other(&props(" https://bücher.example/a b "), Click::default()).url,
            "https://xn--bcher-kva.example/a%20b"
        );
    }

    // ── the model type (T6) ─────────────────────────────────────────────────

    use crate::model::{Control, ControlType, Form, PropValue};
    use crate::xml::{form_to_string, load_form_from_str};

    #[test]
    fn a_new_hyperlink_has_the_hand_cursor_every_link_property_and_no_ping() {
        let c = Control::new("LNK-1", ControlType::Hyperlink, 0, 0);
        assert_eq!(
            c.get_prop("Cursor")
                .map(|v| v.as_str().to_string())
                .as_deref(),
            Some("Hand")
        );
        assert_eq!(
            c.get_prop("Caption")
                .map(|v| v.as_str().to_string())
                .as_deref(),
            Some("LNK-1")
        );
        for (k, v) in SEEDS {
            assert_eq!(
                c.get_prop(k).map(|p| p.as_str().to_string()).as_deref(),
                Some(*v),
                "{k}"
            );
        }
        assert!(c.get_prop("Ping").is_none(), "no ping in v1");
        assert_eq!(
            ControlType::Hyperlink.default_size(),
            ControlType::Label.default_size()
        );
        assert_eq!(ControlType::Hyperlink.primary_event(), "onClick");
        let ev = ControlType::Hyperlink.supported_events();
        for e in ControlType::Label.supported_events() {
            assert!(ev.contains(e), "the Label's {e}");
        }
        assert!(ev.contains(&"onBeforeNavigate") && ev.contains(&"onNavigated"));
        assert!(!ControlType::Hyperlink.is_non_visual());
        assert!(ControlType::Hyperlink.takes_tab_order() && ControlType::Hyperlink.is_tab_stop());
        assert!(!ControlType::Hyperlink.has_perspective(), "spec 096: a control that takes input is never tilted");
        assert_eq!(
            crate::model::runtime_property_names_for("Hyperlink"),
            &["Cancel"]
        );
        assert!(ControlType::ALL.contains(&ControlType::Hyperlink));
    }

    fn form_with(c: Control) -> Form {
        let mut f = Form::new("LINKS", "Links", 400, 300);
        f.controls.push(c);
        f
    }

    #[test]
    fn a_hyperlink_round_trips_through_cfrm_with_every_property_at_a_distinct_value() {
        let mut c = Control::new("LNK-1", ControlType::Hyperlink, 10, 20);
        let values: Vec<(&str, String)> = SEEDS
            .iter()
            .enumerate()
            .map(|(i, (k, _))| (*k, format!("v{i}-{k}")))
            .collect();
        for (k, v) in &values {
            c.set_prop(*k, PropValue::String(v.clone()));
        }
        c.set_prop("Caption", PropValue::String("Example".into()));
        let text = form_to_string(&form_with(c)).unwrap();
        assert!(text.contains("type=\"Hyperlink\""), "saved as Hyperlink");
        let back = load_form_from_str(&text).unwrap();
        let l = back.find_control("LNK-1").unwrap();
        assert_eq!(l.control_type, ControlType::Hyperlink);
        for (k, v) in &values {
            let got = l
                .get_prop(k)
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            println!("{k} = {got}");
            assert_eq!(&got, v, "{k}");
        }
        assert_eq!(
            form_to_string(&back).unwrap(),
            text,
            "stable on a second save"
        );
    }

    #[test]
    fn a_form_written_with_the_interim_weblink_name_loads_as_a_hyperlink_and_saves_as_one() {
        let text = form_to_string(&form_with(Control::new(
            "LNK-1",
            ControlType::Hyperlink,
            0,
            0,
        )))
        .unwrap();
        let old = text.replace("type=\"Hyperlink\"", "type=\"WebLink\"");
        assert!(old.contains("type=\"WebLink\""));
        let back = load_form_from_str(&old).unwrap();
        assert_eq!(
            back.find_control("LNK-1").unwrap().control_type,
            ControlType::Hyperlink
        );
        let saved = form_to_string(&back).unwrap();
        assert!(saved.contains("type=\"Hyperlink\"") && !saved.contains("WebLink"));
    }

    #[test]
    fn a_stored_refused_url_survives_load_and_save_unchanged() {
        // User data is sacred (D5): the refusal is reported, the text is never cleared.
        let mut c = Control::new("LNK-1", ControlType::Hyperlink, 0, 0);
        c.set_prop("Url", PropValue::String("javascript:alert(1)".into()));
        let text = form_to_string(&form_with(c)).unwrap();
        let back = load_form_from_str(&text).unwrap();
        let l = back.find_control("LNK-1").unwrap();
        assert_eq!(
            l.get_prop("Url").map(|p| p.as_str().to_string()).as_deref(),
            Some("javascript:alert(1)")
        );
        assert!(
            check(&LinkProps::of(l).url, false, &Policy::default()).is_err(),
            "still refused"
        );
        assert_eq!(form_to_string(&back).unwrap(), text);
    }

    #[test]
    fn the_underline_rule_reads_its_three_values() {
        assert_eq!(Underline::parse("Always"), Underline::Always);
        assert_eq!(Underline::parse("hover"), Underline::Hover);
        assert_eq!(Underline::parse("Never"), Underline::Never);
        assert!(
            Underline::Always.shows(false)
                && Underline::Hover.shows(true)
                && !Underline::Hover.shows(false)
                && !Underline::Never.shows(true)
        );
    }
}
