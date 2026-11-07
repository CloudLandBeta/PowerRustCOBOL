// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The device features of a Mobile application (WASM-012 R7, R9, R13; D-Scan).
//!
//! A feature is something of the phone a program reaches through the shell:
//! the operating system's secure storage, the system browser and dialler, a
//! deep link that opens a form. Each one is a row of [`FEATURES`] that says
//! which `COBOL::` verbs use it, what the shell has to ask Android and iOS for,
//! which Tauri plugin and capability permissions it needs and what the store
//! listing has to say about it. A project declares the features it uses in
//! `[mobile] features`; [`scan`] finds the uses in the programs and
//! [`undeclared`] refuses the build at the first use of a feature that is not
//! declared — so the shell is granted exactly what the application uses.
//!
//! The parser turns both spellings of a device verb, the inline
//! `COBOL::SecureStore( K V )` and `INVOKE COBOL::SecureStore( K V )`, into
//! `CALL "COBOL-SECURESTORE"` (`cobol_object_call`), which is what the scan
//! looks for. The five features of the spec's Q1 (camera, location,
//! notifications, biometric unlock, barcode scanning) are not rows yet: they
//! wait for the operator (WASM-012 O1).

use cobolt_ast::program::{ProcedureBody, Program};
use cobolt_ast::stmt::Stmt;
use std::collections::BTreeSet;

/// The version of the feature table. A row added or changed raises it, so a
/// staging folder generated from an older table is regenerated.
pub const FEATURE_SET_VERSION: u32 = 1;

/// One device feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Feature {
    /// The name a project declares in `[mobile] features`.
    pub name: &'static str,
    /// What it is for, in one line (English: the tools' and the KB's text).
    pub purpose: &'static str,
    /// The `COBOL::` verbs that use it, upper case without the `COBOL-` prefix.
    pub verbs: &'static [&'static str],
    /// The Android permissions the manifest must carry (`android.permission.…`).
    pub android_permissions: &'static [&'static str],
    /// The iOS `Info.plist` usage key whose sentence is the permission prompt,
    /// when the feature prompts.
    pub ios_usage_key: Option<&'static str>,
    /// The Tauri plugin the shell registers for it (the crate's short name),
    /// when the shell's own commands are not enough.
    pub plugin: Option<&'static str>,
    /// The capability permissions the main window is granted for it: the
    /// shell's own commands (`allow-…`) and the plugin's.
    pub permissions: &'static [&'static str],
    /// What the store listing has to say (R13): iOS privacy, Android data
    /// safety. Printed after a build, never submitted.
    pub checklist: &'static [&'static str],
}

/// The first set (WASM-012 R9).
pub const FEATURES: &[Feature] = &[
    Feature {
        name: "secure-storage",
        purpose: "keeps small secrets (a session, a token) in the phone's own secure store: the Android Keystore, the iOS Keychain",
        verbs: &["SECURESTORE", "SECURELOAD", "SECUREDELETE"],
        android_permissions: &[],
        ios_usage_key: None,
        plugin: None,
        permissions: &["allow-secure-store", "allow-secure-load", "allow-secure-delete"],
        checklist: &[
            "iOS privacy: data kept in the Keychain on the device is not \"collected\" unless it is sent off the device; say so if your backend receives it",
            "Android data safety: data encrypted with the Android Keystore stays on the device; declare it only if it is shared or sent",
        ],
    },
    Feature {
        name: "open-address",
        purpose: "opens an address outside the application in the system browser, and mailto:, tel: and sms: in the phone's own apps",
        verbs: &["NAVIGATE", "OPENURL"],
        android_permissions: &[],
        ios_usage_key: None,
        plugin: Some("opener"),
        permissions: &["opener:allow-open-url"],
        checklist: &["no store declaration: opening an address sends nothing on the application's behalf"],
    },
    Feature {
        name: "deep-link",
        purpose: "an address of the application's own scheme opens a form with its parameters (me::GetParam)",
        verbs: &[],
        android_permissions: &[],
        ios_usage_key: None,
        plugin: Some("deep-link"),
        permissions: &["deep-link:default"],
        checklist: &["no store declaration; the scheme is the application id, so a link from another application can open your forms: check every parameter a form reads"],
    },
];

/// The feature called `name`.
pub fn feature(name: &str) -> Option<&'static Feature> {
    FEATURES.iter().find(|f| f.name.eq_ignore_ascii_case(name.trim()))
}

/// The feature a verb belongs to (any case, with or without `COBOL-`).
pub fn feature_of_verb(verb: &str) -> Option<&'static Feature> {
    let v = verb.trim().trim_matches('"').to_ascii_uppercase();
    let v = v.strip_prefix("COBOL-").unwrap_or(&v);
    FEATURES.iter().find(|f| f.verbs.contains(&v))
}

/// Is `verb` one of the device verbs only a Mobile application has (the
/// secure-storage verbs; `Navigate`/`OpenUrl` exist on the Web too)?
pub fn is_mobile_only_verb(verb: &str) -> bool {
    feature_of_verb(verb).is_some_and(|f| f.name == "secure-storage")
}

/// A use of a feature in a program.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FeatureUse {
    pub feature: &'static str,
    /// The verb as the program wrote it, in the `COBOL::` spelling (`SecureStore`).
    pub verb: String,
    /// The program's file, project-relative.
    pub file: String,
    /// The line of the statement.
    pub line: u32,
}

/// The display spelling of a verb: `SECURESTORE` → `SecureStore`.
fn verb_spelling(verb: &str) -> String {
    match verb {
        "SECURESTORE" => "SecureStore".into(),
        "SECURELOAD" => "SecureLoad".into(),
        "SECUREDELETE" => "SecureDelete".into(),
        "NAVIGATE" => "Navigate".into(),
        "OPENURL" => "OpenUrl".into(),
        other => other.to_owned(),
    }
}

/// The feature uses in one parsed program (and its nested programs).
pub fn scan_program(file: &str, program: &Program, out: &mut Vec<FeatureUse>) {
    let mut visit = |s: &Stmt| {
        if let Stmt::Call { program: cobolt_ast::expr::Expr::Literal(cobolt_ast::expr::Literal::String(name), _), span, .. } = s {
            if let Some(f) = feature_of_verb(name) {
                let v = name.trim().to_ascii_uppercase();
                let v = v.strip_prefix("COBOL-").unwrap_or(&v).to_owned();
                out.push(FeatureUse { feature: f.name, verb: verb_spelling(&v), file: file.to_owned(), line: span.line });
            }
        }
    };
    let mut lists: Vec<&Vec<Stmt>> = Vec::new();
    match &program.procedure.body {
        ProcedureBody::Sections(sections) => {
            for s in sections {
                for p in &s.paragraphs {
                    lists.push(&p.stmts);
                }
            }
        }
        ProcedureBody::Paragraphs(paragraphs) => {
            for p in paragraphs {
                lists.push(&p.stmts);
            }
        }
    }
    for list in lists {
        for s in list {
            s.walk(&mut visit);
        }
    }
    for nested in &program.nested_programs {
        scan_program(file, nested, out);
    }
}

/// The feature uses in `programs` (`(file, source)` pairs), sorted by file
/// and line. A program that does not parse contributes what its partial tree
/// holds; Check reports its errors.
pub fn scan(programs: &[(String, String)]) -> Vec<FeatureUse> {
    let mut out = Vec::new();
    for (file, src) in programs {
        let tokens = cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::detect(src));
        if let Some(program) = cobolt_parser::parse(tokens).program {
            scan_program(file, &program, &mut out);
        }
    }
    out.sort_by(|a, b| (&a.file, a.line, a.feature, &a.verb).cmp(&(&b.file, b.line, b.feature, &b.verb)));
    out.dedup();
    out
}

/// The features `uses` names, each once, in table order.
pub fn used_features(uses: &[FeatureUse]) -> Vec<&'static Feature> {
    let names: BTreeSet<&str> = uses.iter().map(|u| u.feature).collect();
    FEATURES.iter().filter(|f| names.contains(f.name)).collect()
}

/// `N uses of M features in K programs`.
pub fn summary(uses: &[FeatureUse]) -> String {
    let programs: BTreeSet<&str> = uses.iter().map(|u| u.file.as_str()).collect();
    let features = used_features(uses).len();
    format!(
        "{} use{} of {} feature{} in {} program{}",
        uses.len(),
        if uses.len() == 1 { "" } else { "s" },
        features,
        if features == 1 { "" } else { "s" },
        programs.len(),
        if programs.len() == 1 { "" } else { "s" }
    )
}

/// One line per use of a feature `declared` does not name (R7):
/// `file:line: uses <feature> (COBOL::<verb>); add it to the project's device features`.
pub fn undeclared(uses: &[FeatureUse], declared: &[String]) -> Vec<String> {
    uses.iter()
        .filter(|u| !declared.iter().any(|d| d.trim().eq_ignore_ascii_case(u.feature)))
        .map(|u| format!("{}:{}: uses {} (COBOL::{}); add it to the project's device features", u.file, u.line, u.feature, u.verb))
        .collect()
}

/// The names in `declared` that are not features, each as a line.
pub fn unknown_declared(declared: &[String]) -> Vec<String> {
    let known: Vec<&str> = FEATURES.iter().map(|f| f.name).collect();
    declared
        .iter()
        .filter(|d| feature(d).is_none())
        .map(|d| format!("[mobile] features names '{}', which is not a device feature; the features are {}", d.trim(), known.join(", ")))
        .collect()
}

/// The declared features, each once, in table order (unknown names dropped:
/// [`unknown_declared`] reports them).
pub fn declared_features(declared: &[String]) -> Vec<&'static Feature> {
    FEATURES.iter().filter(|f| declared.iter().any(|d| d.trim().eq_ignore_ascii_case(f.name))).collect()
}

/// The R13 checklist of the declared features, one line each, headed by the feature.
pub fn checklist(declared: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for f in declared_features(declared) {
        for line in f.checklist {
            out.push(format!("{}: {line}", f.name));
        }
    }
    out
}

/// The features whose verbs appear in a module's `builtins` (D-Scan: the
/// generator's record of what the program calls on the application object).
pub fn features_of_builtins<'a>(builtins: impl IntoIterator<Item = &'a str>) -> BTreeSet<&'static str> {
    builtins.into_iter().filter_map(feature_of_verb).map(|f| f.name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_of_the_table_is_complete() {
        for f in FEATURES {
            assert!(!f.name.is_empty() && !f.purpose.is_empty(), "{f:?}");
            assert!(!f.permissions.is_empty(), "{}: the window must be granted something", f.name);
            assert!(!f.checklist.is_empty(), "{}: R13 needs a line", f.name);
            for v in f.verbs {
                assert_eq!(*v, v.to_ascii_uppercase(), "{}: verbs are upper case", f.name);
                assert_eq!(feature_of_verb(v).map(|x| x.name), Some(f.name));
            }
            if let Some(p) = f.plugin {
                assert!(f.permissions.iter().any(|x| x.starts_with(&format!("{p}:"))), "{}: the plugin's permission is granted", f.name);
            }
        }
        let names: BTreeSet<&str> = FEATURES.iter().map(|f| f.name).collect();
        assert_eq!(names.len(), FEATURES.len(), "names are unique");
        for gated in ["camera", "location", "notifications", "biometric", "barcode"] {
            assert!(feature(gated).is_none(), "{gated} waits for the operator (O1)");
        }
        assert!(is_mobile_only_verb("COBOL-SECURELOAD") && is_mobile_only_verb("securestore"));
        assert!(!is_mobile_only_verb("NAVIGATE"), "Navigate is a Web verb too");
        println!("feature table v{FEATURE_SET_VERSION}: {} rows, {} verbs, every row complete", FEATURES.len(), FEATURES.iter().map(|f| f.verbs.len()).sum::<usize>());
    }

    const PROGRAM: &str = "       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-KEY   PIC X(20) VALUE \"session\".
       01 WS-VALUE PIC X(80).
       PROCEDURE DIVISION.
       MAIN-PARA.
      *> COBOL::SecureStore( WS-KEY WS-VALUE ) in a comment is not a use
           DISPLAY \"COBOL::SecureDelete( WS-KEY ) in a string is not a use\"
           COBOL::SecureStore( WS-KEY WS-VALUE )
           INVOKE COBOL::SecureLoad( WS-KEY )
           IF WS-VALUE = SPACES
               COBOL::OpenUrl( \"https://example.com\" )
           END-IF
           GOBACK.
";

    #[test]
    fn the_scan_finds_both_spellings_at_their_lines_and_ignores_comments_and_strings() {
        let uses = scan(&[("generated/main-form.cbl".to_owned(), PROGRAM.to_owned())]);
        let got: Vec<(&str, &str, u32)> = uses.iter().map(|u| (u.feature, u.verb.as_str(), u.line)).collect();
        assert_eq!(got, [("secure-storage", "SecureStore", 11), ("secure-storage", "SecureLoad", 12), ("open-address", "OpenUrl", 14)]);
        assert_eq!(summary(&uses), "3 uses of 2 features in 1 program");
        let none = scan(&[("x.cbl".to_owned(), PROGRAM.replace("COBOL::SecureStore( WS-KEY WS-VALUE )\n", "").replace("INVOKE COBOL::SecureLoad( WS-KEY )\n", ""))]);
        assert_eq!(none.len(), 1, "only the OpenUrl is left: {none:?}");
        println!("scan: {} — inline and INVOKE found at lines 11, 12, 14; the comment and the string literal ignored", summary(&uses));
    }

    #[test]
    fn an_undeclared_use_is_refused_with_its_line() {
        let uses = scan(&[("generated/main-form.cbl".to_owned(), PROGRAM.to_owned())]);
        let lines = undeclared(&uses, &["open-address".to_owned()]);
        assert_eq!(
            lines,
            [
                "generated/main-form.cbl:11: uses secure-storage (COBOL::SecureStore); add it to the project's device features",
                "generated/main-form.cbl:12: uses secure-storage (COBOL::SecureLoad); add it to the project's device features",
            ]
        );
        assert!(undeclared(&uses, &["secure-storage".into(), "OPEN-ADDRESS".into()]).is_empty(), "declared in any case");
        let unknown = unknown_declared(&["camera".into(), "deep-link".into()]);
        assert_eq!(unknown.len(), 1);
        assert!(unknown[0].contains("'camera'") && unknown[0].contains("secure-storage, open-address, deep-link"), "{}", unknown[0]);
        assert_eq!(features_of_builtins(["SECURESTORE", "MESSAGEBOX", "OPENURL"]).into_iter().collect::<Vec<_>>(), ["open-address", "secure-storage"]);
        let list = checklist(&["deep-link".into(), "secure-storage".into()]);
        assert_eq!(list.len(), 3);
        assert!(list[0].starts_with("secure-storage: iOS privacy"), "table order: {list:?}");
        println!("undeclared: 2 lines with file:line and the verb; unknown 'camera' named; checklist of 2 features = 3 lines");
    }
}
