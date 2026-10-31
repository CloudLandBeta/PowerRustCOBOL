// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Hyperlink** (WASM-006): the player's half of the control — its properties read from the form's
//! state, and its methods. The policy itself (what is followed, how a click is planned) is
//! `cobolt_forms::hyperlink`, shared with the desktop host, the interpreter and the Designer.
//!
//! The control is link text that behaves like an `<a>` element — including the browser's own
//! conventions for modifier keys — and exposes every `<a>` capability as a property **except
//! `ping`** (operator, 2026-10-20), which is the one attribute that makes the browser send a
//! request to an address the page never fetches. Nothing here knows the DOM; the renderer asks
//! [`plan`] what to do and carries it out, so the whole policy is tested natively.
//!
//! **A click** (plan D1, D2): a link with no handler bound to `onClick`, `onBeforeNavigate` or
//! `onNavigated` stays a native anchor and the browser navigates. A link with one is
//! default-prevented and walks [`start`] → [`advance`]: `Cancel` reset to false, `onClick` (the
//! modifier keys) → `onBeforeNavigate` (`target⇥url`) → the navigation, each step queued behind the
//! handlers of the one before; a true `Cancel` after either handler stops it.
//!
//! **Page commands** (C9): `Open` and `OpenIn` queue `open` / `open-in` with the planned navigation
//! as arguments ([`plan_args`]); a fragment address queues `scroll-to` with the place it names.

use crate::methods::{MethodError, Outcome};
use crate::tree::{FormTree, Kind};
use crate::value::turned_on;

// The policy lives in `cobolt-forms` (WASM-006 T5, plan D4): the desktop host, the interpreter and the
// Designer check an address with the same functions.
pub use cobolt_forms::hyperlink::{
    check, kind_of, normalize, parse, plan, rel_isolates, scheme_of, Click, LinkProps, NavPlan,
    Platform, Policy, Refused, Target, UrlKind,
};

/// Everything about a link a click needs, read from the control's properties in the form's state.
pub fn read_props(tree: &FormTree, ctl: usize) -> LinkProps {
    LinkProps::read_with(|k| tree.get(ctl, k))
}

/// The methods of the control (WASM-006 R12): `SetUrl`, `GetUrl`, `Open`, `OpenIn(target)`.
pub fn method(
    tree: &mut FormTree,
    ctl: usize,
    m: &str,
    args: &[&str],
) -> Result<Outcome, MethodError> {
    debug_assert!(matches!(tree.ctls[ctl].kind, Kind::Control(_)));
    let arg = |i: usize| args.get(i).map(|s| s.trim().to_owned()).unwrap_or_default();
    let mut out = Outcome::default();
    match m {
        "GETURL" => out.value = Some(tree.get(ctl, "Url")),
        "SETURL" => {
            let url = arg(0);
            let allow_data = cobolt_forms::hyperlink::wants_download(&tree.get(ctl, "Download"));
            if let Err(r) = check(&url, allow_data, &tree.policy.clone()) {
                return Err(MethodError::Refused(format!(
                    "SetUrl refused: {}",
                    r.message
                )));
            }
            out.events.extend(tree.set(ctl, "Url", &url));
        }
        "OPEN" | "OPENIN" => {
            // `Open` acts as a plain click honouring `Target`; `OpenIn` as a plain click with the target it
            // is given. A program's call is not a user's gesture: a new tab or window it opens may be
            // blocked by the browser, which the page reports with its pop-up notice.
            let mut props = read_props(tree, ctl);
            let cmd = if m == "OPENIN" {
                props.target = arg(0);
                "open-in"
            } else {
                "open"
            };
            match plan(&props, Click::default(), tree.platform, &tree.policy) {
                Ok(Some(np)) => match fragment_of(&np.url) {
                    Some(name) => tree.command(ctl, "scroll-to", vec![name]),
                    None => tree.command(ctl, cmd, plan_args(&np)),
                },
                Ok(None) => {}
                Err(r) => {
                    return Err(MethodError::Refused(format!(
                        "{} refused: {}",
                        if m == "OPENIN" { "OpenIn" } else { "Open" },
                        r.message
                    )))
                }
            }
        }
        _ => unreachable!("routed by the caller"),
    }
    Ok(out)
}

/// The page commands the player carries out for a link (`dom/instance.rs::run_command`).
pub const PAGE_COMMANDS: &[&str] = &["open", "open-in", "scroll-to"];

/// The arguments of an `open` / `open-in` command: url, target (the property value), features, rel,
/// download name (empty = none), referrer policy.
pub fn plan_args(np: &NavPlan) -> Vec<String> {
    vec![
        np.url.clone(),
        np.target.name().to_owned(),
        np.features.clone(),
        np.rel.clone(),
        np.download.clone().unwrap_or_default(),
        np.referrer_policy.clone(),
    ]
}

/// The plan an `open` / `open-in` command carries.
pub fn plan_from_args(args: &[String]) -> Option<NavPlan> {
    let a = |i: usize| args.get(i).cloned().unwrap_or_default();
    let url = a(0);
    if url.trim().is_empty() {
        return None;
    }
    let download = a(4);
    Some(NavPlan {
        url,
        target: Target::parse(&a(1)),
        features: a(2),
        rel: a(3),
        download: (!download.is_empty()).then_some(download),
        referrer_policy: a(5),
    })
}

/// The place a fragment address names (`#Terms` → `Terms`), or `None` for any other address.
pub fn fragment_of(url: &str) -> Option<String> {
    let n = normalize(url);
    n.strip_prefix('#').map(|f| f.to_owned())
}

/// Where a fragment link goes on this form (R10): a control named so, whatever the case, else `None`
/// (the page then looks for an element with that id).
pub fn fragment_target(tree: &FormTree, name: &str) -> Option<usize> {
    tree.by_name(name).filter(|&i| i != 0)
}

/// Does a click on this link go through the program's handlers (plan D1)? Only then is the browser's
/// own navigation prevented.
pub fn intercepted(tree: &FormTree, ctl: usize) -> bool {
    let c = tree.ctl(ctl);
    ["onClick", "onBeforeNavigate", "onNavigated"]
        .iter()
        .any(|e| c.has_handler(e))
}

// ── the click sequence (R5, R6, R13) ─────────────────────────────────────────

/// Where an intercepted click is: the handlers of the step before have run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// `onClick` has run: raise `onBeforeNavigate` unless it cancelled.
    BeforeNavigate,
    /// `onBeforeNavigate` has run: navigate unless it cancelled.
    Follow,
}

/// An intercepted click waiting for the handlers before it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkStep {
    pub inst: u32,
    pub ctl: usize,
    pub stage: Stage,
    pub plan: NavPlan,
}

/// What comes after a step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// A handler set `Cancel`: nothing happens.
    Stop,
    /// Raise `event` with `arg`, then wait for its handler before `then`.
    Raise {
        event: &'static str,
        arg: String,
        then: LinkStep,
    },
    /// Navigate.
    Follow(NavPlan),
}

/// The start of an intercepted click: reset `Cancel`, raise `onClick` with the modifier keys, and wait.
/// Returns the property write, the event to raise and the step that follows the handler.
pub fn start(
    inst: u32,
    ctl: usize,
    plan: NavPlan,
    click: Click,
) -> (
    (&'static str, &'static str),
    (&'static str, String),
    LinkStep,
) {
    (
        ("Cancel", "0"),
        ("onClick", click.modifiers_text()),
        LinkStep {
            inst,
            ctl,
            stage: Stage::BeforeNavigate,
            plan,
        },
    )
}

/// The step after the handlers of `step` have run, `cancelled` being the `Cancel` they left.
pub fn advance(step: LinkStep, cancelled: bool) -> Next {
    if cancelled {
        return Next::Stop;
    }
    match step.stage {
        Stage::BeforeNavigate => {
            let arg = format!("{}\t{}", step.plan.target.name(), step.plan.url);
            Next::Raise {
                event: "onBeforeNavigate",
                arg,
                then: LinkStep {
                    stage: Stage::Follow,
                    ..step
                },
            }
        }
        Stage::Follow => Next::Follow(step.plan),
    }
}

/// Is the `Cancel` a handler left true?
pub fn cancelled(tree: &FormTree, ctl: usize) -> bool {
    turned_on(&tree.get(ctl, "Cancel"))
}

/// Does `onNavigated` follow this navigation (OQ1, working assumption)? Only where the application is
/// still loaded afterwards: a fragment link and a new tab or window. A download is not a navigation.
pub fn raises_navigated(np: &NavPlan) -> bool {
    np.download.is_none() && (fragment_of(&np.url).is_some() || np.opens_new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_url_refuses_javascript_and_leaves_the_control_unchanged() {
        let mut t = crate::tree::testing::tree();
        let ctl = t.by_name("TXT-IN").unwrap();
        t.set(ctl, "Url", "https://ok.example/");
        let r = method(&mut t, ctl, "SETURL", &["javascript:alert(1)"]);
        assert!(matches!(r, Err(MethodError::Refused(_))));
        assert_eq!(t.get(ctl, "Url"), "https://ok.example/");
        method(&mut t, ctl, "SETURL", &["https://other.example/"]).unwrap();
        assert_eq!(
            method(&mut t, ctl, "GETURL", &[]).unwrap().value.unwrap(),
            "https://other.example/"
        );
    }

    fn link_tree() -> (FormTree, usize) {
        crate::tree::testing::tree_with(
            "Hyperlink",
            "LNK-1",
            &[
                ("Url", "https://example.com/a?x=1&y=2"),
                ("Target", "NewTab"),
            ],
        )
    }

    /// C9 regression: `Open` and `OpenIn` queue page commands that `run_command` handles, never one that
    /// falls to its `_ => {}`.
    #[test]
    fn open_and_open_in_queue_commands_the_page_handles() {
        let src = include_str!("dom/instance.rs");
        let body = &src[src.find("fn run_command").expect("run_command")..];
        let body = &body[..body.find("_ => {}").expect("the catch-all")];
        for c in PAGE_COMMANDS {
            assert!(
                body.contains(&format!("\"{c}\"")),
                "run_command has no arm for {c}"
            );
        }
        let (mut t, i) = link_tree();
        method(&mut t, i, "OPEN", &[]).unwrap();
        method(&mut t, i, "OPENIN", &["NewWindow"]).unwrap();
        t.set(i, "Url", "#lnk-1");
        method(&mut t, i, "OPEN", &[]).unwrap();
        let cmds: Vec<(String, Vec<String>)> = t
            .take_changes()
            .into_iter()
            .filter_map(|c| match c {
                crate::tree::Change::Command { name, args, .. } => Some((name, args)),
                _ => None,
            })
            .collect();
        println!("{cmds:?}");
        assert_eq!(
            cmds.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            ["open", "open-in", "scroll-to"]
        );
        for (n, _) in &cmds {
            assert!(PAGE_COMMANDS.contains(&n.as_str()));
        }
        let open = plan_from_args(&cmds[0].1).unwrap();
        assert_eq!(
            (open.url.as_str(), open.target, open.rel.as_str()),
            (
                "https://example.com/a?x=1&y=2",
                Target::NewTab,
                "noopener noreferrer"
            )
        );
        assert_eq!(
            plan_from_args(&cmds[1].1).unwrap().target,
            Target::NewWindow,
            "OpenIn takes its argument"
        );
        assert_eq!(cmds[2].1, ["lnk-1"]);
        assert_eq!(
            fragment_target(&t, "lnk-1"),
            Some(i),
            "a fragment names a control whatever the case"
        );
        // A refused address is refused by Open too.
        t.set(i, "Url", "javascript:alert(1)");
        assert!(matches!(
            method(&mut t, i, "OPEN", &[]),
            Err(MethodError::Refused(_))
        ));
    }

    #[test]
    fn cancel_in_on_click_or_in_on_before_navigate_stops_the_navigation() {
        let np = plan(
            &LinkProps {
                url: "https://example.com/".into(),
                target: "NewTab".into(),
                ..Default::default()
            },
            Click::default(),
            Platform::Other,
            &Policy::default(),
        )
        .unwrap()
        .unwrap();
        let (reset, click, step) = start(
            7,
            3,
            np.clone(),
            Click {
                ctrl: true,
                ..Default::default()
            },
        );
        assert_eq!(reset, ("Cancel", "0"), "reset before onClick");
        assert_eq!(
            click,
            ("onClick", "Ctrl".to_owned()),
            "the click reports the modifiers"
        );
        // Cancel set in onClick: nothing more, not even onBeforeNavigate.
        assert_eq!(advance(step.clone(), true), Next::Stop);
        // Not cancelled: onBeforeNavigate with target⇥url, then the navigation unless it cancels.
        let Next::Raise { event, arg, then } = advance(step, false) else {
            panic!("onBeforeNavigate")
        };
        assert_eq!(
            (event, arg.as_str()),
            ("onBeforeNavigate", "NewTab\thttps://example.com/")
        );
        assert_eq!(
            advance(then.clone(), true),
            Next::Stop,
            "Cancel set in onBeforeNavigate"
        );
        assert_eq!(advance(then, false), Next::Follow(np.clone()));
        assert!(
            raises_navigated(&np),
            "a new tab: the application is still here"
        );
        let same = NavPlan {
            target: Target::SelfFrame,
            ..np
        };
        assert!(
            !raises_navigated(&same),
            "a same-context navigation unloads the application (OQ1)"
        );
    }

    #[test]
    fn a_link_is_intercepted_only_when_a_navigation_event_has_a_handler() {
        let (t, i) = link_tree();
        assert!(
            !intercepted(&t, i),
            "nobody handles it: the anchor stays native"
        );
        // The demo manifest binds BTN-GO's onClick (controls.rs tests); the same predicate answers for it.
        let bound = crate::tree::testing::tree();
        assert!(intercepted(&bound, bound.by_name("BTN-GO").unwrap()));
    }
}
