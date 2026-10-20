// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The New Project templates, as data** (WASM-001 R2, R4, R5).
//!
//! The IDE's New Project dialog and the `create_project` tool both read this
//! catalogue; neither names a template in code. A template is a
//! [`TemplateDescriptor`]: an id, the project kind it makes, the platform and
//! device class for a Mobile one, whether it can be created yet, and the
//! options it asks for with their defaults. Adding a template adds one
//! descriptor here — the dialog draws whatever options the descriptor lists.
//!
//! The wording here is English: it is what the tools say. The IDE shows the
//! same templates in the developer's language through its own string table,
//! keyed by the ids below, and falls back to this English for an id it has no
//! string for (a template registered by a test, say).

use std::collections::BTreeMap;

use cobolt_compiler::project_kind::{DeviceClass, MobilePlatform, ProjectKind};

/// One value of a choice option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice {
    /// What the manifest and the tools carry.
    pub id: &'static str,
    /// The English wording.
    pub label: &'static str,
}

/// What kind of answer an option takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionKind {
    /// One of a list.
    Choice(&'static [Choice]),
    /// On or off.
    Flag,
}

/// One option a template asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptionDescriptor {
    pub id: &'static str,
    /// The English wording of the option's label.
    pub label: &'static str,
    pub kind: OptionKind,
    /// The choice id, or `"true"` / `"false"` for a flag.
    pub default: &'static str,
}

impl OptionDescriptor {
    /// Is `value` one this option accepts?
    pub fn accepts(&self, value: &str) -> bool {
        match self.kind {
            OptionKind::Choice(choices) => choices.iter().any(|c| c.id == value),
            OptionKind::Flag => matches!(value, "true" | "false"),
        }
    }
}

/// What a created project builds into — shown as *Output type* in the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputType {
    /// A native executable application.
    Executable,
    /// A static WebAssembly bundle.
    WebBundle,
    /// An Android package.
    AndroidPackage,
    /// An iOS archive.
    IosArchive,
}

/// One entry of the New Project list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemplateDescriptor {
    /// Stable id: `desktop`, `web`, `android-phone`, …
    pub id: &'static str,
    /// English name, as the list shows it.
    pub name: &'static str,
    /// The noun of the dialog's title (*New* **Desktop** *Project*).
    pub title_noun: &'static str,
    /// The platforms the project runs on, as one line.
    pub platforms: &'static str,
    /// One sentence on what the template makes.
    pub summary: &'static str,
    /// What the template makes.
    pub kind: ProjectKind,
    /// The operating system and device class of a Mobile template that can be
    /// created; `None` for the rest.
    pub mobile: Option<(MobilePlatform, DeviceClass)>,
    /// Can *Create* run on it? An unavailable template is listed all the same
    /// (WASM-001 R5, Q3).
    pub creatable: bool,
    /// The options it asks for, in the order the dialog shows them.
    pub options: &'static [OptionDescriptor],
}

impl TemplateDescriptor {
    /// The option named `id`, if the template asks for it.
    pub fn option(&self, id: &str) -> Option<&OptionDescriptor> {
        self.options.iter().find(|o| o.id == id)
    }

    /// What this template's project builds into.
    pub fn output_type(&self) -> OutputType {
        match (self.kind, self.mobile) {
            (ProjectKind::Desktop, _) => OutputType::Executable,
            (ProjectKind::Web, _) => OutputType::WebBundle,
            (ProjectKind::Mobile, Some((MobilePlatform::Ios, _))) => OutputType::IosArchive,
            (ProjectKind::Mobile, _) => OutputType::AndroidPackage,
        }
    }

    /// Does the template's name, platforms or summary contain `needle`
    /// (case-blind)? The list's search box.
    pub fn matches(&self, needle: &str) -> bool {
        let needle = needle.trim().to_lowercase();
        needle.is_empty()
            || [self.name, self.platforms, self.summary, self.title_noun]
                .iter()
                .any(|s| s.to_lowercase().contains(&needle))
    }

    /// The default value of every option, by id.
    pub fn default_options(&self) -> BTreeMap<String, String> {
        self.options.iter().map(|o| (o.id.to_owned(), o.default.to_owned())).collect()
    }
}

// ── The options ─────────────────────────────────────────────────────────────

const WINDOW_STYLES: &[Choice] = &[
    Choice { id: "tabbed", label: "Tabbed Document" },
    Choice { id: "single", label: "Single Window" },
];
const UI_FRAMEWORKS: &[Choice] = &[Choice { id: "powerui", label: "PowerUI" }];
const APP_STYLES: &[Choice] = &[
    Choice { id: "spa", label: "Single-page application (SPA)" },
    Choice { id: "standalone", label: "Standalone forms" },
];
const ORIENTATIONS: &[Choice] = &[
    Choice { id: "auto", label: "Follow the device" },
    Choice { id: "portrait", label: "Portrait" },
    Choice { id: "landscape", label: "Landscape" },
];
const ANDROID_MIN_OS: &[Choice] = &[
    Choice { id: "auto", label: "Auto / Latest" },
    Choice { id: "24", label: "Android 7.0 (API 24)" },
    Choice { id: "26", label: "Android 8.0 (API 26)" },
    Choice { id: "29", label: "Android 10 (API 29)" },
    Choice { id: "31", label: "Android 12 (API 31)" },
    Choice { id: "34", label: "Android 14 (API 34)" },
];
const IOS_MIN_OS: &[Choice] = &[
    Choice { id: "auto", label: "Auto / Latest" },
    Choice { id: "15.0", label: "iOS 15.0" },
    Choice { id: "16.0", label: "iOS 16.0" },
    Choice { id: "17.0", label: "iOS 17.0" },
    Choice { id: "18.0", label: "iOS 18.0" },
];

const fn flag(id: &'static str, label: &'static str, default: bool) -> OptionDescriptor {
    OptionDescriptor { id, label, kind: OptionKind::Flag, default: if default { "true" } else { "false" } }
}

const fn choice(id: &'static str, label: &'static str, choices: &'static [Choice], default: &'static str) -> OptionDescriptor {
    OptionDescriptor { id, label, kind: OptionKind::Choice(choices), default }
}

const DESKTOP_OPTIONS: &[OptionDescriptor] = &[
    choice("window_style", "Window style", WINDOW_STYLES, "tabbed"),
    choice("ui_framework", "UI framework", UI_FRAMEWORKS, "powerui"),
    flag("high_dpi", "Enable High DPI support", true),
    flag("gpu_acceleration", "Enable GPU acceleration", true),
    flag("sample", "Include sample main program", true),
];

const WEB_OPTIONS: &[OptionDescriptor] = &[
    choice("style", "Application style", APP_STYLES, "spa"),
    flag("backend", "Create the Backend part", true),
    flag("sample", "Include sample main form", true),
];

const ANDROID_PHONE_OPTIONS: &[OptionDescriptor] = &[
    choice("orientation", "Default orientation", ORIENTATIONS, "portrait"),
    choice("min_os", "Minimum OS", ANDROID_MIN_OS, "auto"),
    choice("style", "Application style", APP_STYLES, "spa"),
    flag("backend", "Create the Backend part", true),
    flag("sample", "Include sample main form", true),
];
const ANDROID_TABLET_OPTIONS: &[OptionDescriptor] = &[
    choice("orientation", "Default orientation", ORIENTATIONS, "auto"),
    choice("min_os", "Minimum OS", ANDROID_MIN_OS, "auto"),
    choice("style", "Application style", APP_STYLES, "spa"),
    flag("backend", "Create the Backend part", true),
    flag("sample", "Include sample main form", true),
];
const IPHONE_OPTIONS: &[OptionDescriptor] = &[
    choice("orientation", "Default orientation", ORIENTATIONS, "portrait"),
    choice("min_os", "Minimum OS", IOS_MIN_OS, "auto"),
    choice("style", "Application style", APP_STYLES, "spa"),
    flag("backend", "Create the Backend part", true),
    flag("sample", "Include sample main form", true),
];
const IPAD_OPTIONS: &[OptionDescriptor] = &[
    choice("orientation", "Default orientation", ORIENTATIONS, "auto"),
    choice("min_os", "Minimum OS", IOS_MIN_OS, "auto"),
    choice("style", "Application style", APP_STYLES, "spa"),
    flag("backend", "Create the Backend part", true),
    flag("sample", "Include sample main form", true),
];

/// The template `create_project` uses when none is named.
pub const DEFAULT_TEMPLATE: &str = "desktop";

/// The catalogue: the twelve entries of the operator's mock-up, in its order
/// (WASM-001 §"The mock-up"). In 2.0 the creatable ones are Desktop, Web
/// (WebAssembly), Android (Cellphone), Android (Tablet), iPhone and iPad
/// (Q3, operator 2026-10-19); the rest are listed and cannot be created yet.
pub fn builtin() -> Vec<TemplateDescriptor> {
    use DeviceClass::{Phone, Tablet};
    use MobilePlatform::{Android, Ios};
    let unavailable = |id, name, title_noun, platforms, summary, kind| TemplateDescriptor {
        id,
        name,
        title_noun,
        platforms,
        summary,
        kind,
        mobile: None,
        creatable: false,
        options: &[],
    };
    vec![
        TemplateDescriptor {
            id: "desktop",
            name: "Desktop",
            title_noun: "Desktop",
            platforms: "macOS, Windows, Linux",
            summary: "A native desktop application with forms, indexed files and a starter program.",
            kind: ProjectKind::Desktop,
            mobile: None,
            creatable: true,
            options: DESKTOP_OPTIONS,
        },
        unavailable(
            "chatbot-desktop",
            "Agentic AI Chatbot",
            "Agentic AI Chatbot",
            "macOS, Windows, Linux",
            "A desktop application with an AI chat built in.",
            ProjectKind::Desktop,
        ),
        unavailable(
            "chatbot-web",
            "Agentic AI Chatbot (Web)",
            "Agentic AI Chatbot (Web)",
            "Web",
            "A web application with an AI chat whose model key stays on the Backend.",
            ProjectKind::Web,
        ),
        TemplateDescriptor {
            id: "web",
            name: "Web (WebAssembly)",
            title_noun: "Web",
            platforms: "Chrome, Edge, Firefox, Safari",
            summary: "A WebAssembly application for the browser, with a Backend part it talks to.",
            kind: ProjectKind::Web,
            mobile: None,
            creatable: true,
            options: WEB_OPTIONS,
        },
        unavailable(
            "usercontrol-desktop",
            "UserControl (Desktop)",
            "UserControl",
            "macOS, Windows, Linux",
            "A reusable composite control for desktop forms.",
            ProjectKind::Desktop,
        ),
        unavailable(
            "usercontrol-web",
            "UserControl (Web)",
            "UserControl (Web)",
            "Web",
            "A reusable composite control for web forms.",
            ProjectKind::Web,
        ),
        TemplateDescriptor {
            id: "android-phone",
            name: "Android (Cellphone)",
            title_noun: "Android",
            platforms: "Android",
            summary: "An Android phone application: the web frontend in a Tauri shell, with a Backend part.",
            kind: ProjectKind::Mobile,
            mobile: Some((Android, Phone)),
            creatable: true,
            options: ANDROID_PHONE_OPTIONS,
        },
        TemplateDescriptor {
            id: "android-tablet",
            name: "Android (Tablet)",
            title_noun: "Android Tablet",
            platforms: "Android",
            summary: "An Android tablet application: the web frontend in a Tauri shell, with a Backend part.",
            kind: ProjectKind::Mobile,
            mobile: Some((Android, Tablet)),
            creatable: true,
            options: ANDROID_TABLET_OPTIONS,
        },
        unavailable(
            "android-watch",
            "Android (Smartwatch)",
            "Android Smartwatch",
            "Android",
            "A smartwatch application.",
            ProjectKind::Mobile,
        ),
        TemplateDescriptor {
            id: "iphone",
            name: "iPhone (iOS)",
            title_noun: "iPhone",
            platforms: "iOS",
            summary: "An iPhone application: the web frontend in a Tauri shell, with a Backend part.",
            kind: ProjectKind::Mobile,
            mobile: Some((Ios, Phone)),
            creatable: true,
            options: IPHONE_OPTIONS,
        },
        TemplateDescriptor {
            id: "ipad",
            name: "iPad (iOS)",
            title_noun: "iPad",
            platforms: "iOS",
            summary: "An iPad application: the web frontend in a Tauri shell, with a Backend part.",
            kind: ProjectKind::Mobile,
            mobile: Some((Ios, Tablet)),
            creatable: true,
            options: IPAD_OPTIONS,
        },
        unavailable(
            "apple-watch",
            "Apple SmartWatch (watchOS)",
            "Apple SmartWatch",
            "watchOS",
            "A watchOS application.",
            ProjectKind::Mobile,
        ),
    ]
}

/// The template named `id` in `catalogue`.
pub fn find<'a>(catalogue: &'a [TemplateDescriptor], id: &str) -> Option<&'a TemplateDescriptor> {
    catalogue.iter().find(|t| t.id.eq_ignore_ascii_case(id.trim()))
}

/// The ids of the templates that can be created, for a message.
pub fn creatable_ids(catalogue: &[TemplateDescriptor]) -> Vec<&'static str> {
    catalogue.iter().filter(|t| t.creatable).map(|t| t.id).collect()
}

/// The template of a kind with the platform and device class the caller asks
/// for — what `create_project`'s `kind` argument resolves to.
pub fn for_kind<'a>(
    catalogue: &'a [TemplateDescriptor],
    kind: ProjectKind,
    mobile: Option<(MobilePlatform, DeviceClass)>,
) -> Option<&'a TemplateDescriptor> {
    catalogue
        .iter()
        .find(|t| t.creatable && t.kind == kind && (kind != ProjectKind::Mobile || t.mobile == mobile))
}

// ── A request to create a project ──────────────────────────────────────────

/// Everything New Project and `create_project` take, the same for both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRequest {
    /// The project's name, with or without the `.project` suffix.
    pub name: String,
    /// A template id; empty means [`DEFAULT_TEMPLATE`].
    pub template: String,
    /// `major.minor.fix`; empty means `1.0.0`.
    pub version: String,
    /// The main file, project-relative; empty means the template's own
    /// (`src/main.cbl`, or the main form of a Web or Mobile project).
    pub main: String,
    pub description: String,
    /// An IDE theme id; empty means the default. A theme the form catalogue
    /// knows is also the default form theme.
    pub theme: String,
    /// A locale such as `pt-BR`; empty means none chosen.
    pub locale: String,
    /// Option ids to values; an option left out takes the template's default.
    pub options: BTreeMap<String, String>,
    /// Make the project wear Spatial, for the IDE and for its forms — what a
    /// project a coding agent creates does (the golden rules).
    pub spatial: bool,
}

impl CreateRequest {
    /// A request for a Desktop project named `name`, all defaults — what the
    /// tool made before there were kinds.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            template: String::new(),
            version: String::new(),
            main: String::new(),
            description: String::new(),
            theme: String::new(),
            locale: String::new(),
            options: BTreeMap::new(),
            spatial: false,
        }
    }

    pub fn template(mut self, id: impl Into<String>) -> Self {
        self.template = id.into();
        self
    }

    pub fn option(mut self, id: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(id.into(), value.into());
        self
    }

    /// The version to write: the request's, or `1.0.0`.
    pub fn version_or_default(&self) -> &str {
        let v = self.version.trim();
        if v.is_empty() {
            "1.0.0"
        } else {
            v
        }
    }
}

/// A request checked against its template: the descriptor and every option's
/// final value.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub template: TemplateDescriptor,
    pub options: BTreeMap<String, String>,
}

impl Resolved {
    /// A flag option's value.
    pub fn flag(&self, id: &str) -> bool {
        self.options.get(id).is_some_and(|v| v == "true")
    }

    /// A choice option's value.
    pub fn choice(&self, id: &str) -> &str {
        self.options.get(id).map(String::as_str).unwrap_or("")
    }
}

/// Check `request` against `catalogue`: the template exists and can be created,
/// every option is one the template asks for and carries a value it accepts.
/// An option left out takes the template's default.
pub fn resolve(request: &CreateRequest, catalogue: &[TemplateDescriptor]) -> Result<Resolved, String> {
    let id = if request.template.trim().is_empty() { DEFAULT_TEMPLATE } else { request.template.trim() };
    let template = find(catalogue, id).ok_or_else(|| {
        format!("no template '{id}'; the templates are: {}", catalogue.iter().map(|t| t.id).collect::<Vec<_>>().join(", "))
    })?;
    if !template.creatable {
        return Err(format!(
            "the template '{}' ({}) cannot be created yet; the templates that can are: {}",
            template.id,
            template.name,
            creatable_ids(catalogue).join(", ")
        ));
    }
    let mut options = template.default_options();
    for (key, value) in &request.options {
        let option = template.option(key).ok_or_else(|| {
            format!(
                "the template '{}' has no option '{key}'; its options are: {}",
                template.id,
                template.options.iter().map(|o| o.id).collect::<Vec<_>>().join(", ")
            )
        })?;
        if !option.accepts(value) {
            let accepted = match option.kind {
                OptionKind::Choice(choices) => choices.iter().map(|c| c.id).collect::<Vec<_>>().join(", "),
                OptionKind::Flag => "true, false".to_owned(),
            };
            return Err(format!("'{value}' is not a value of the option '{key}' of '{}'; use one of: {accepted}", template.id));
        }
        options.insert(key.clone(), value.clone());
    }
    Ok(Resolved { template: *template, options })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalogue_is_the_twelve_entries_of_the_mock_up_in_its_order() {
        let ids: Vec<&str> = builtin().iter().map(|t| t.id).collect();
        assert_eq!(
            ids,
            [
                "desktop",
                "chatbot-desktop",
                "chatbot-web",
                "web",
                "usercontrol-desktop",
                "usercontrol-web",
                "android-phone",
                "android-tablet",
                "android-watch",
                "iphone",
                "ipad",
                "apple-watch"
            ]
        );
    }

    /// WASM-001 R5 / Q3: Desktop, Web, Android phone and tablet, iPhone and
    /// iPad can be created in 2.0; the rest are listed and cannot.
    #[test]
    fn exactly_the_six_templates_of_the_operators_ruling_can_be_created() {
        let catalogue = builtin();
        assert_eq!(creatable_ids(&catalogue), ["desktop", "web", "android-phone", "android-tablet", "iphone", "ipad"]);
        for t in catalogue.iter().filter(|t| !t.creatable) {
            assert!(t.options.is_empty(), "{}: an unavailable template asks for nothing", t.id);
        }
    }

    #[test]
    fn every_option_default_is_a_value_the_option_accepts() {
        for t in builtin() {
            for o in t.options {
                assert!(o.accepts(o.default), "{}.{}: default {:?}", t.id, o.id, o.default);
            }
            let mut seen = std::collections::BTreeSet::new();
            for o in t.options {
                assert!(seen.insert(o.id), "{}: option {} twice", t.id, o.id);
            }
        }
    }

    /// WASM-001 R4: an option that does not apply to a template is not shown for it.
    #[test]
    fn the_options_belong_to_their_templates() {
        let catalogue = builtin();
        let desktop = find(&catalogue, "desktop").unwrap();
        for id in ["window_style", "ui_framework", "high_dpi", "gpu_acceleration", "sample"] {
            assert!(desktop.option(id).is_some(), "desktop asks for {id}");
        }
        for id in ["orientation", "min_os", "style", "backend"] {
            assert!(desktop.option(id).is_none(), "desktop does not ask for {id}");
        }
        for id in ["web", "android-phone", "android-tablet", "iphone", "ipad"] {
            let t = find(&catalogue, id).unwrap();
            assert!(t.option("style").is_some() && t.option("backend").is_some(), "{id}: application style and Backend");
            assert!(t.option("high_dpi").is_none() && t.option("gpu_acceleration").is_none(), "{id}: no desktop-only options");
        }
        assert!(find(&catalogue, "web").unwrap().option("orientation").is_none(), "a web page has no orientation option");
        for id in ["android-phone", "android-tablet", "iphone", "ipad"] {
            let t = find(&catalogue, id).unwrap();
            assert!(t.option("orientation").is_some() && t.option("min_os").is_some(), "{id}: orientation and minimum OS");
        }
    }

    #[test]
    fn the_defaults_follow_the_device_and_the_platform() {
        let catalogue = builtin();
        let default = |id: &str, opt: &str| find(&catalogue, id).unwrap().option(opt).unwrap().default;
        assert_eq!(default("android-phone", "orientation"), "portrait");
        assert_eq!(default("android-tablet", "orientation"), "auto");
        assert_eq!(default("iphone", "orientation"), "portrait");
        assert_eq!(default("ipad", "orientation"), "auto");
        assert_eq!(default("web", "style"), "spa", "SPA is the default (WASM-011 R1)");
        assert_eq!(default("web", "backend"), "true", "the Backend is created unless cleared (Q5)");
        // The minimum-OS choices are the platform's own.
        let android = find(&catalogue, "android-phone").unwrap().option("min_os").unwrap();
        let ios = find(&catalogue, "iphone").unwrap().option("min_os").unwrap();
        assert!(android.accepts("26") && !android.accepts("16.0"));
        assert!(ios.accepts("16.0") && !ios.accepts("26"));
    }

    #[test]
    fn output_types_follow_the_kind_and_platform() {
        let catalogue = builtin();
        let out = |id: &str| find(&catalogue, id).unwrap().output_type();
        assert_eq!(out("desktop"), OutputType::Executable);
        assert_eq!(out("web"), OutputType::WebBundle);
        assert_eq!(out("android-tablet"), OutputType::AndroidPackage);
        assert_eq!(out("ipad"), OutputType::IosArchive);
    }

    #[test]
    fn the_search_finds_a_template_by_name_platform_or_summary() {
        let catalogue = builtin();
        let hits = |q: &str| catalogue.iter().filter(|t| t.matches(q)).map(|t| t.id).collect::<Vec<_>>();
        assert_eq!(hits("").len(), catalogue.len(), "an empty search shows everything");
        assert_eq!(hits("WEBASSEMBLY"), ["web"], "case-blind, by name");
        assert!(hits("ios").contains(&"iphone") && hits("ios").contains(&"ipad"), "by platform");
        assert!(hits("watch").contains(&"android-watch") && hits("watch").contains(&"apple-watch"));
        assert!(hits("no such template").is_empty());
    }

    #[test]
    fn a_request_is_resolved_against_the_catalogue() {
        let catalogue = builtin();
        // All defaults: a Desktop project.
        let r = resolve(&CreateRequest::new("X"), &catalogue).unwrap();
        assert_eq!(r.template.id, "desktop");
        assert!(r.flag("sample") && r.flag("high_dpi") && r.choice("window_style") == "tabbed");
        // A named template with an override.
        let r = resolve(&CreateRequest::new("X").template("android-phone").option("min_os", "29"), &catalogue).unwrap();
        assert_eq!(r.template.mobile, Some((MobilePlatform::Android, DeviceClass::Phone)));
        assert_eq!(r.choice("min_os"), "29");
        assert_eq!(r.choice("orientation"), "portrait", "the rest keep their defaults");
        // Refusals say what would have worked.
        let e = resolve(&CreateRequest::new("X").template("nope"), &catalogue).unwrap_err();
        assert!(e.contains("no template 'nope'") && e.contains("android-phone"), "{e}");
        let e = resolve(&CreateRequest::new("X").template("android-watch"), &catalogue).unwrap_err();
        assert!(e.contains("cannot be created yet") && e.contains("iphone"), "{e}");
        let e = resolve(&CreateRequest::new("X").template("web").option("orientation", "portrait"), &catalogue).unwrap_err();
        assert!(e.contains("has no option 'orientation'") && e.contains("style"), "{e}");
        let e = resolve(&CreateRequest::new("X").template("web").option("style", "tabs"), &catalogue).unwrap_err();
        assert!(e.contains("spa") && e.contains("standalone"), "{e}");
        let e = resolve(&CreateRequest::new("X").option("high_dpi", "yes"), &catalogue).unwrap_err();
        assert!(e.contains("true, false"), "{e}");
    }

    /// WASM-001 AC5: a template is added by adding a descriptor, and the
    /// catalogue's consumers take it as it is.
    #[test]
    fn a_new_template_is_one_descriptor() {
        const FAKE_OPTIONS: &[OptionDescriptor] = &[flag("shiny", "Shiny", false), choice("size", "Size", UI_FRAMEWORKS, "powerui")];
        let mut catalogue = builtin();
        catalogue.push(TemplateDescriptor {
            id: "fake",
            name: "Fake",
            title_noun: "Fake",
            platforms: "Nowhere",
            summary: "A template that exists only in a test.",
            kind: ProjectKind::Web,
            mobile: None,
            creatable: true,
            options: FAKE_OPTIONS,
        });
        let t = find(&catalogue, "fake").unwrap();
        assert_eq!(t.default_options().len(), 2);
        let r = resolve(&CreateRequest::new("X").template("fake").option("shiny", "true"), &catalogue).unwrap();
        assert!(r.flag("shiny"));
        assert!(creatable_ids(&catalogue).contains(&"fake"));
    }

    #[test]
    fn a_kind_resolves_to_its_template() {
        let catalogue = builtin();
        assert_eq!(for_kind(&catalogue, ProjectKind::Desktop, None).unwrap().id, "desktop");
        assert_eq!(for_kind(&catalogue, ProjectKind::Web, None).unwrap().id, "web");
        assert_eq!(
            for_kind(&catalogue, ProjectKind::Mobile, Some((MobilePlatform::Ios, DeviceClass::Tablet))).unwrap().id,
            "ipad"
        );
        assert!(for_kind(&catalogue, ProjectKind::Mobile, None).is_none(), "a Mobile project needs its platform and device");
    }
}
