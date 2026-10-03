// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The PowerRustCOBOL plugin for Claude Code** (spec 084 R6–R10), written
//! from the agent-neutral content into a local marketplace in the IDE's own
//! data folder — never into a project.
//!
//! Layout (settled by the T1 spike against Claude Code 2.1.158):
//!
//! ```text
//! <data>/cobolt/claude-code/marketplace/
//!   .claude-plugin/marketplace.json
//!   powerrustcobol/.claude-plugin/plugin.json   (userConfig: port, token)
//!   powerrustcobol/.mcp.json                    (IDE over HTTP, rcrun over stdio)
//!   powerrustcobol/skills/<name>/SKILL.md
//!   powerrustcobol/agents/powerrustcobol-reviewer.md
//! ```
//!
//! No `CLAUDE.md` (R9): the rules travel as the servers' instructions. No
//! token and no home folder in any file (R10): the token is a `userConfig`
//! option Claude Code keeps in its secure storage, and `rcrun` is named
//! `${HOME}`-relative.

use std::path::{Path, PathBuf};

use cobolt_project_tools::content::{self, gap_template_markdown, KitContent, RcrunLocation, Skill};
use serde_json::{json, Value};


/// The command that starts `rcrun`, using Claude Code's `${HOME}` expansion so
/// the home folder is never written into the plugin (R10).
pub fn rcrun_command(at: &RcrunLocation) -> String {
    match at {
        RcrunLocation::UnderHome(rest) => format!("${{HOME}}/{rest}"),
        RcrunLocation::Absolute(path) => path.clone(),
        RcrunLocation::OnPath => "rcrun".into(),
    }
}

/// The marketplace's name, and the plugin's.
pub const MARKETPLACE: &str = "powerrustcobol";
pub const PLUGIN: &str = "powerrustcobol";
/// `plugin@marketplace`, as `claude plugin install` takes it.
pub fn plugin_ref() -> String {
    format!("{PLUGIN}@{MARKETPLACE}")
}

/// Where the IDE keeps the marketplace it hands Claude Code.
pub fn bundle_dir() -> PathBuf {
    crate::llm::base_dir().join("claude-code").join("marketplace")
}

/// One file of the bundle, relative to the marketplace folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleFile {
    pub rel: String,
    pub text: String,
}

/// The content the plugin carries, for this IDE version.
fn kit(version: &str) -> KitContent {
    content::build(content::BuildInput {
        ide_version: version.to_owned(),
        project_name: String::new(),
        ide_url: String::new(),
        rcrun: RcrunLocation::OnPath,
        tools: cobolt_project_tools::ProjectTools::<cobolt_project_tools::HeadlessHost>::tool_list()
            .into_iter()
            .map(|t| content::ToolInfo { name: t.name, description: t.description.unwrap_or_default() })
            .collect(),
        reference: Vec::new(),
    })
}

fn pretty(v: &Value) -> String {
    let mut s = serde_json::to_string_pretty(v).unwrap_or_default();
    s.push('\n');
    s
}

fn yaml_str(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

const WHERE_THE_RULES_ARE: &str = "The standing rules come with the PowerRustCOBOL tool servers (their \
     instructions); the reference is the `powerrustcobol://reference/` resources, and `kb_lookup` / \
     `kb_search` find a name or a subject in it.";

fn skill_file(c: &KitContent, s: &Skill) -> String {
    let mut out = format!(
        "---\nname: {}\ndescription: {}\n---\n\n# {}\n\n{}\n\n## Steps\n\n",
        s.name,
        yaml_str(&s.summary),
        s.name,
        s.summary
    );
    for (i, step) in s.steps.iter().enumerate() {
        out.push_str(&format!("{}. {}\n", i + 1, step));
    }
    let tools: Vec<String> = s.uses_tools.iter().map(|t| format!("`{t}`")).collect();
    out.push_str(&format!("\n## Tools\n\n{}.\n", tools.join(", ")));
    if let Some(example) = &s.example {
        out.push_str(&format!("\n## Example\n\n{example}\n"));
    }
    if s.name == "powerrustcobol-gap-report" {
        out.push_str(&format!(
            "\n## Template\n\nSave it as `{}`.\n\n{}",
            c.gap_template.path_rule,
            gap_template_markdown(&c.gap_template)
        ));
    }
    out.push_str(&format!("\n{WHERE_THE_RULES_ARE}\n"));
    out
}

fn reviewer_file(c: &KitContent) -> String {
    let r = &c.reviewer;
    let mut out = format!(
        "---\nname: {}\ndescription: {}\n---\n\n\
         You review a change to a PowerRustCOBOL project before it is reported done. You do not \
         change files: you report what is wrong, file by file, or that the change passes.\n\n\
         ## Checks\n\n",
        r.name,
        yaml_str(&r.summary)
    );
    for (i, check) in r.checks.iter().enumerate() {
        out.push_str(&format!("{}. {}\n", i + 1, check));
    }
    let tools: Vec<String> = r.uses_tools.iter().map(|t| format!("`{t}`")).collect();
    out.push_str(&format!("\n## Tools\n\n{}.\n\n{WHERE_THE_RULES_ARE}\n", tools.join(", ")));
    out
}

/// Every file of the plugin bundle for IDE `version`, with `rcrun` where
/// `rcrun` says it is.
pub fn files(version: &str, rcrun: &RcrunLocation) -> Vec<BundleFile> {
    let c = kit(version);
    let description = "PowerRustCOBOL AI: build COBOL desktop applications with the IDE's own tools and knowledge";
    let mut out = vec![
        BundleFile {
            rel: ".claude-plugin/marketplace.json".into(),
            text: pretty(&json!({
                "name": MARKETPLACE,
                "owner": { "name": "PowerRustCOBOL" },
                "description": "The PowerRustCOBOL AI plugin, installed by the IDE's Configure Claude Code",
                "plugins": [{ "name": PLUGIN, "source": format!("./{PLUGIN}"), "description": description, "version": version }]
            })),
        },
        BundleFile {
            rel: format!("{PLUGIN}/.claude-plugin/plugin.json"),
            text: pretty(&json!({
                "name": PLUGIN,
                "version": version,
                "description": description,
                "author": { "name": "PowerRustCOBOL" },
                "userConfig": {
                    "port": {
                        "type": "number",
                        "title": "PowerRustCOBOL AI port",
                        "description": "The port PowerRustCOBOL AI serves its tools on (Help → Claude Code Settings).",
                        "default": 5720
                    },
                    "token": {
                        "type": "string",
                        "title": "PowerRustCOBOL AI access token",
                        "description": "Generated by PowerRustCOBOL AI's Configure Claude Code.",
                        "sensitive": true
                    }
                }
            })),
        },
        BundleFile {
            rel: format!("{PLUGIN}/.mcp.json"),
            text: pretty(&json!({
                "mcpServers": {
                    "powerrustcobol-ide": {
                        "type": "http",
                        "url": "http://127.0.0.1:${user_config.port}/mcp",
                        "headers": { "Authorization": "Bearer ${user_config.token}" }
                    },
                    "powerrustcobol": {
                        "type": "stdio",
                        "command": rcrun_command(rcrun),
                        "args": ["mcp"]
                    }
                }
            })),
        },
    ];
    for skill in &c.skills {
        out.push(BundleFile { rel: format!("{PLUGIN}/skills/{}/SKILL.md", skill.name), text: skill_file(&c, skill) });
    }
    out.push(BundleFile { rel: format!("{PLUGIN}/agents/{}.md", c.reviewer.name), text: reviewer_file(&c) });
    out
}

/// Write the bundle into `dir`, replacing whatever an earlier version left.
pub fn write(dir: &Path, files: &[BundleFile]) -> Result<(), String> {
    if dir.exists() {
        std::fs::remove_dir_all(dir).map_err(|e| format!("cannot clear {}: {e}", dir.display()))?;
    }
    for f in files {
        let path = dir.join(&f.rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, &f.text).map_err(|e| format!("cannot write {}: {e}", f.rel))?;
    }
    Ok(())
}

/// Spec 084 R10 — refuse a bundle carrying a stored API key or a personal
/// detail of this machine; names the file and what was found, never the value.
pub fn check(files: &[BundleFile], personal: &crate::ai_bundle::Personal, llm: &crate::llm::LlmConfig) -> Result<(), String> {
    for f in files {
        if let Some(slot) = crate::ai_bundle::find_key(&f.text, llm) {
            return Err(format!("an API key ({slot}) was found in {}", f.rel));
        }
        if let Some(kind) = personal.find(&f.text) {
            return Err(format!("a personal detail ({kind}) was found in {}", f.rel));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 084 AC4 / AC19: ten skills, the reviewer, both servers, the
    /// `userConfig` port and token; no `CLAUDE.md`, no settings, no token, no
    /// `docs/powerrustcobol/` path; every file passes the redaction check.
    #[test]
    fn the_bundle_holds_the_plugin_and_nothing_personal() {
        let files = files("1.80.test", &RcrunLocation::UnderHome("Apps/PowerRustCOBOL/rcrun".into()));
        let rels: Vec<&str> = files.iter().map(|f| f.rel.as_str()).collect();
        let skills = rels.iter().filter(|r| r.ends_with("/SKILL.md")).count();
        assert_eq!(skills, 10, "{rels:?}");
        for skill in ["build-an-application", "shell-and-navigation", "layout-and-themes"] {
            let rel = format!("{PLUGIN}/skills/powerrustcobol-{skill}/SKILL.md");
            let text = &files.iter().find(|f| f.rel == rel).unwrap_or_else(|| panic!("{rel}")).text;
            assert!(text.contains("render_form"), "{rel} names render_form");
        }
        assert!(rels.contains(&"powerrustcobol/agents/powerrustcobol-reviewer.md"));
        assert!(!rels.iter().any(|r| r.ends_with("CLAUDE.md") || r.ends_with("settings.json")), "{rels:?}");

        let get = |rel: &str| -> Value { serde_json::from_str(&files.iter().find(|f| f.rel == rel).unwrap().text).unwrap() };
        let plugin = get("powerrustcobol/.claude-plugin/plugin.json");
        assert_eq!(plugin["version"], "1.80.test");
        assert_eq!(plugin["userConfig"]["token"]["sensitive"], true);
        let mcp = get("powerrustcobol/.mcp.json");
        assert_eq!(mcp["mcpServers"]["powerrustcobol-ide"]["url"], "http://127.0.0.1:${user_config.port}/mcp");
        assert_eq!(mcp["mcpServers"]["powerrustcobol-ide"]["headers"]["Authorization"], "Bearer ${user_config.token}");
        assert_eq!(mcp["mcpServers"]["powerrustcobol"]["command"], "${HOME}/Apps/PowerRustCOBOL/rcrun");
        assert_eq!(mcp["mcpServers"]["powerrustcobol"]["args"], json!(["mcp"]));
        assert!(mcp["mcpServers"].get(content::IDE_SERVER).is_some() && mcp["mcpServers"].get(content::STDIO_SERVER).is_some());
        let market = get(".claude-plugin/marketplace.json");
        assert_eq!(market["plugins"][0]["source"], "./powerrustcobol");

        for f in &files {
            assert!(!f.text.contains("docs/powerrustcobol"), "{} still points at the per-project pack", f.rel);
            assert!(!f.text.contains("CLAUDE.md"), "{} mentions CLAUDE.md", f.rel);
        }
        let llm = crate::llm::LlmConfig::load_defaults_for_test();
        let personal = crate::ai_bundle::Personal::from_environment();
        check(&files, &personal, &llm).expect("nothing personal, no key");
        println!("plugin bundle: {} files — 10 skills (3 for building an application), the reviewer, 2 servers, userConfig port + sensitive token; no CLAUDE.md, settings or home path", files.len());
    }

    /// The written bundle passes Claude Code's own `claude plugin validate`
    /// (marketplace and plugin), where `claude` is installed; skipped — and
    /// said so — where it is not. Validation only reads the files, and runs
    /// against a throwaway configuration folder.
    #[test]
    fn claude_validates_the_written_bundle() {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let dirs = super::super::configure::candidate_dirs(home.as_deref());
        let Some(claude) = super::super::configure::find_claude(&dirs) else {
            println!("claude plugin validate: no claude on this machine — skipped");
            return;
        };
        let root = std::env::temp_dir().join(format!("prc-084-bundle-{}", std::process::id()));
        let dir = root.join("marketplace");
        write(&dir, &files(crate::version::VERSION, &RcrunLocation::OnPath)).unwrap();
        for target in [dir.clone(), dir.join(PLUGIN)] {
            let out = std::process::Command::new(&claude)
                .args(["plugin", "validate"])
                .arg(&target)
                .env("CLAUDE_CONFIG_DIR", root.join("config"))
                .output()
                .unwrap();
            let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
            assert!(out.status.success() && !text.contains('✘'), "{}: {text}", target.display());
        }
        let _ = std::fs::remove_dir_all(&root);
        println!("claude plugin validate: marketplace and plugin both pass");
    }

    /// A key stored on this machine, planted in the bundle, refuses it.
    #[test]
    fn a_planted_key_refuses_the_bundle() {
        let mut llm = crate::llm::LlmConfig::load_defaults_for_test();
        llm.store_api_key(crate::llm::provider_key_slot("openrouter"), "sk-or-PLANTED-0123456789");
        let mut files = files("1.80.test", &RcrunLocation::OnPath);
        files[0].text.push_str("sk-or-PLANTED-0123456789");
        let why = check(&files, &crate::ai_bundle::Personal::from_environment(), &llm).unwrap_err();
        assert!(why.contains("providerkey::openrouter") && !why.contains("PLANTED"), "{why}");
        println!("plugin bundle: a planted stored key is refused, named by its slot only");
    }
}
