// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Claude Code writer** (spec 080 R2, R15, R20, T4.3).
//!
//! Turns the target-neutral [`KitContent`] into Claude Code's files:
//! `CLAUDE.md` (the kit's section), `.claude/settings.json`,
//! `.claude/skills/<name>/SKILL.md`, `.claude/agents/<reviewer>.md`,
//! `.mcp.json` and the reference pack under `docs/powerrustcobol/`.
//!
//! What this file assumes about Claude Code is plan §8 (A1, A3, A4): the
//! `.mcp.json` shape and its `${VAR}` expansion, `permissions.allow/deny`
//! with `Tool(pattern)` rules and `mcp__<server>__<tool>` names, and
//! frontmatter that opens every skill and agent file. A wrong assumption
//! changes this file only, never the content.

use serde_json::{json, Value};

use super::content::{
    gap_template_markdown, KitContent, KitFile, KitFileKind, KitWriter, RcrunLocation, Skill, REFERENCE_DIR,
};

/// The file Claude Code reads the project's instructions from.
pub const BRIEF_FILE: &str = "CLAUDE.md";

pub struct ClaudeCodeWriter;

impl KitWriter for ClaudeCodeWriter {
    fn target(&self) -> &'static str {
        "claude-code"
    }

    fn files(&self, c: &KitContent) -> Vec<KitFile> {
        let mut out = vec![KitFile {
            rel: BRIEF_FILE.into(),
            body: brief(c),
            kind: KitFileKind::Section,
        }];
        out.push(KitFile {
            rel: ".claude/settings.json".into(),
            body: pretty(&settings(c)),
            kind: KitFileKind::Json,
        });
        for skill in &c.skills {
            out.push(KitFile {
                rel: format!(".claude/skills/{}/SKILL.md", skill.name),
                body: skill_file(c, skill),
                kind: KitFileKind::FrontMatterMarkdown,
            });
        }
        out.push(KitFile {
            rel: format!(".claude/agents/{}.md", c.reviewer.name),
            body: reviewer_file(c),
            kind: KitFileKind::FrontMatterMarkdown,
        });
        out.push(KitFile {
            rel: ".mcp.json".into(),
            body: pretty(&mcp_json(c)),
            kind: KitFileKind::Json,
        });
        for doc in &c.reference {
            out.push(KitFile {
                rel: format!("{REFERENCE_DIR}/{}", doc.name),
                body: doc.body.clone(),
                kind: KitFileKind::Markdown,
            });
        }
        out
    }
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default() + "\n"
}

/// `mcp__<server>__<tool>` for every tool, on both servers (plan D10: listed
/// by full name — the `mcp__<server>__*` wildcard has a reported regression).
fn tool_rules(c: &KitContent, tools: &[String]) -> Vec<String> {
    [&c.servers.ide_name, &c.servers.stdio_name]
        .iter()
        .flat_map(|server| tools.iter().map(move |t| format!("mcp__{server}__{t}")))
        .collect()
}

/// The permissions (R15, plan D10): edit and read inside the project, the
/// project tools, and no shell. Nothing that reaches outside the project.
fn settings(c: &KitContent) -> Value {
    let p = &c.permissions;
    let mut allow: Vec<String> = Vec::new();
    if p.edit_inside_project {
        allow.push("Edit(./**)".into());
    }
    if p.read_inside_project {
        allow.push("Read(./**)".into());
    }
    if p.allow_project_tools {
        let names: Vec<String> = c.tools.iter().map(|t| t.name.clone()).collect();
        allow.extend(tool_rules(c, &names));
    }
    let deny: Vec<String> = if p.deny_shell { vec!["Bash".into()] } else { Vec::new() };
    json!({ "permissions": { "allow": allow, "deny": deny } })
}

/// The command that starts `rcrun`, using Claude Code's `${HOME}` expansion
/// so the home folder is never written (plan §8 A1, §9 F3).
pub fn rcrun_command(at: &RcrunLocation) -> String {
    match at {
        RcrunLocation::UnderHome(rest) => format!("${{HOME}}/{rest}"),
        RcrunLocation::Absolute(path) => path.clone(),
        RcrunLocation::OnPath => "rcrun".into(),
    }
}

/// The two servers (R11a): the IDE over HTTP, `rcrun mcp` over stdio with the
/// project folder Claude Code was started in (A1, A5).
fn mcp_json(c: &KitContent) -> Value {
    let s = &c.servers;
    let mut args = s.stdio_args.clone();
    args.push("${CLAUDE_PROJECT_DIR}".into());
    json!({
        "mcpServers": {
            s.ide_name.clone(): { "type": "http", "url": s.ide_url },
            s.stdio_name.clone(): { "type": "stdio", "command": rcrun_command(&s.rcrun), "args": args }
        }
    })
}

/// A YAML double-quoted scalar.
fn yaml_str(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn brief(c: &KitContent) -> String {
    let mut out = String::from("# PowerRustCOBOL — coding-agent brief\n\n");
    out.push_str(&format!(
        "Written by {} {} for this project (the kit and IDE version a gap report quotes).\n\n",
        c.product, c.ide_version
    ));
    for p in &c.brief_intro {
        out.push_str(p);
        out.push_str("\n\n");
    }
    out.push_str("## Project layout\n\n| Path | What lives there |\n|---|---|\n");
    for (path, what) in &c.layout {
        out.push_str(&format!("| `{path}` | {what} |\n"));
    }
    out.push_str("\n## Rules\n\n");
    for (i, r) in c.brief_rules.iter().enumerate() {
        out.push_str(&format!("{}. {}\n", i + 1, r.text));
    }
    out.push_str(&format!(
        "\n## Project tools\n\n\
         Two MCP servers serve the same tools, and every answer is JSON:\n\n\
         - `{}` — PowerRustCOBOL AI itself, while it runs with this project open. Use it whenever \
         it answers.\n\
         - `{}` — `rcrun mcp`, started by you, for when the IDE is closed.\n\n",
        c.servers.ide_name, c.servers.stdio_name
    ));
    for t in &c.tools {
        out.push_str(&format!("- `{}` — {}\n", t.name, t.description));
    }
    out.push_str("\n## Skills\n\n");
    for s in &c.skills {
        out.push_str(&format!("- `{}` — {}\n", s.name, s.summary));
    }
    out.push_str(&format!(
        "\n## Review\n\nBefore you report a change done, have the `{}` subagent review it.\n",
        c.reviewer.name
    ));
    out.push_str(&format!(
        "\n## Reference\n\nStart at `{REFERENCE_DIR}/README.md`. It is generated by the version of \
         PowerRustCOBOL that wrote this brief and lists everything that exists; `kb_lookup` finds \
         one name in it.\n"
    ));
    out.push_str(&format!(
        "\n## Gap reports\n\nWhen a request needs something the reference does not list, write \
         `{}` with these sections (the `powerrustcobol-gap-report` skill has the steps):\n\n{}",
        c.gap_template.path_rule,
        gap_template_markdown(&c.gap_template)
    ));
    out
}

fn tool_line(c: &KitContent, tools: &[String]) -> String {
    tools
        .iter()
        .map(|t| format!("`{t}` (`mcp__{}__{t}`, or `mcp__{}__{t}` with the IDE closed)", c.servers.ide_name, c.servers.stdio_name))
        .collect::<Vec<_>>()
        .join(", ")
}

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
    out.push_str(&format!("\n## Tools\n\n{}.\n", tool_line(c, &s.uses_tools)));
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
    out.push_str("\nThe standing rules are in `CLAUDE.md`; the reference is in `docs/powerrustcobol/`.\n");
    out
}

fn reviewer_file(c: &KitContent) -> String {
    let r = &c.reviewer;
    let mut tools: Vec<String> = vec!["Read".into(), "Grep".into(), "Glob".into()];
    tools.extend(tool_rules(c, &r.uses_tools));
    let mut out = format!(
        "---\nname: {}\ndescription: {}\ntools: {}\n---\n\n\
         You review a change to this PowerRustCOBOL project before it is reported done. You do not \
         change files: you report what is wrong, file by file, or that the change passes.\n\n\
         ## Checks\n\n",
        r.name,
        yaml_str(&r.summary),
        tools.join(", ")
    );
    for (i, check) in r.checks.iter().enumerate() {
        out.push_str(&format!("{}. {}\n", i + 1, check));
    }
    out.push_str(&format!(
        "\n## Tools\n\n{}.\n\nThe standing rules are in `CLAUDE.md`; the reference is in `{REFERENCE_DIR}/`.\n",
        tool_line(c, &r.uses_tools)
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_kit::content::{locate_rcrun, tests::sample, RefDoc};
    use std::path::Path;

    /// Renders the same content into a different layout: one flat text file
    /// per piece. Exists only to prove the content needs no change for a
    /// second target (AC9).
    struct FlatWriter;

    impl KitWriter for FlatWriter {
        fn target(&self) -> &'static str {
            "flat-test"
        }
        fn files(&self, c: &KitContent) -> Vec<KitFile> {
            let mut out = vec![KitFile {
                rel: "agent/RULES.txt".into(),
                body: c.brief_rules.iter().map(|r| format!("{}: {}\n", r.id, r.text)).collect(),
                kind: KitFileKind::Markdown,
            }];
            for s in &c.skills {
                out.push(KitFile {
                    rel: format!("agent/skill-{}.txt", s.name),
                    body: s.steps.join("\n"),
                    kind: KitFileKind::Markdown,
                });
            }
            out.push(KitFile {
                rel: "agent/servers.json".into(),
                body: serde_json::to_string(&c.servers).unwrap(),
                kind: KitFileKind::Json,
            });
            out
        }
    }

    fn content_with_pack() -> KitContent {
        let mut c = sample();
        c.reference = vec![
            RefDoc { name: "README.md".into(), title: "Index".into(), body: "# Index\n".into() },
            RefDoc { name: "developers-guide.md".into(), title: "Guide".into(), body: "# Guide\n".into() },
        ];
        c
    }

    /// AC7: settings.json parses, holds the R15 rules and nothing that grants
    /// access outside the project.
    #[test]
    fn settings_permissions() {
        let c = content_with_pack();
        let files = ClaudeCodeWriter.files(&c);
        let settings = files.iter().find(|f| f.rel == ".claude/settings.json").expect("settings.json written");
        let v: Value = serde_json::from_str(&settings.body).expect("settings.json parses");
        let allow: Vec<String> = serde_json::from_value(v["permissions"]["allow"].clone()).unwrap();
        let deny: Vec<String> = serde_json::from_value(v["permissions"]["deny"].clone()).unwrap();
        let tools: Vec<String> = cobolt_project_tools::ProjectTools::<cobolt_project_tools::HeadlessHost>::tool_list()
            .into_iter()
            .map(|t| t.name)
            .collect();
        for rule in ["Edit(./**)", "Read(./**)"] {
            assert!(allow.contains(&rule.to_string()), "missing {rule}");
        }
        let mut mcp = 0;
        for server in ["powerrustcobol-ide", "powerrustcobol"] {
            for t in &tools {
                assert!(allow.contains(&format!("mcp__{server}__{t}")), "missing mcp__{server}__{t}");
                mcp += 1;
            }
        }
        assert_eq!(deny, vec!["Bash".to_string()]);
        for rule in allow.iter().chain(deny.iter()) {
            for bad in ["//", "~", ".."] {
                assert!(!rule.contains(bad), "{rule} reaches outside the project ({bad})");
            }
            assert!(!rule.contains("(/"), "{rule} names an absolute path");
        }
        for bare in ["Glob", "Grep", "Edit", "Read", "Write"] {
            assert!(!allow.contains(&bare.to_string()), "bare {bare} would grant it everywhere");
        }
        let perms = v["permissions"].as_object().unwrap();
        assert!(!perms.contains_key("additionalDirectories") && !perms.contains_key("defaultMode"));
        println!(
            "settings.json: {} allow rules ({mcp} project-tool rules = {} tools × 2 servers), {} deny; \
             no rule outside the project",
            allow.len(),
            tools.len(),
            deny.len()
        );
    }

    /// AC9 + R2 + plan §8 A4: the content is identical before and after two
    /// writers render it, the layouts differ, every skill and agent file opens
    /// with frontmatter, and `.mcp.json` names both servers without the home
    /// folder when rcrun lives under it.
    #[test]
    fn a_second_writer_needs_no_content_change() {
        let mut c = content_with_pack();
        let home = Path::new("/home/fake-user");
        c.servers.rcrun = locate_rcrun(Some(&home.join("Apps/PowerRustCOBOL/rcrun")), Some(home));
        let before = serde_json::to_string(&c).unwrap();
        let claude = ClaudeCodeWriter.files(&c);
        let flat = FlatWriter.files(&c);
        let after = serde_json::to_string(&c).unwrap();
        assert_eq!(before, after, "rendering changed the content");

        let a: std::collections::BTreeSet<&str> = claude.iter().map(|f| f.rel.as_str()).collect();
        let b: std::collections::BTreeSet<&str> = flat.iter().map(|f| f.rel.as_str()).collect();
        assert!(a.is_disjoint(&b), "the two layouts share a path");

        for want in ["CLAUDE.md", ".claude/settings.json", ".mcp.json", ".claude/agents/powerrustcobol-reviewer.md",
                     "docs/powerrustcobol/README.md", "docs/powerrustcobol/developers-guide.md"] {
            assert!(a.contains(want), "{want} not written");
        }
        let fm: Vec<&KitFile> = claude.iter().filter(|f| f.kind == KitFileKind::FrontMatterMarkdown).collect();
        assert_eq!(fm.len(), 8, "seven skills and one agent");
        for f in &fm {
            assert!(f.body.starts_with("---\nname: "), "{} does not open with frontmatter", f.rel);
            assert!(f.body.contains("\ndescription: \""), "{} lacks a description", f.rel);
        }
        let skills = claude.iter().filter(|f| f.rel.starts_with(".claude/skills/") && f.rel.ends_with("/SKILL.md")).count();
        assert_eq!(skills, 7);

        let mcp = claude.iter().find(|f| f.rel == ".mcp.json").unwrap();
        let v: Value = serde_json::from_str(&mcp.body).unwrap();
        assert_eq!(v["mcpServers"]["powerrustcobol-ide"]["type"], "http");
        assert_eq!(v["mcpServers"]["powerrustcobol-ide"]["url"], "http://127.0.0.1:5720/mcp/k-test");
        assert_eq!(v["mcpServers"]["powerrustcobol"]["command"], "${HOME}/Apps/PowerRustCOBOL/rcrun");
        assert_eq!(v["mcpServers"]["powerrustcobol"]["args"], json!(["mcp", "--project", "${CLAUDE_PROJECT_DIR}"]));
        assert!(!mcp.body.contains("fake-user"), "the home folder leaked into .mcp.json");
        assert_eq!(locate_rcrun(Some(Path::new("/opt/prc/rcrun")), Some(home)), RcrunLocation::Absolute("/opt/prc/rcrun".into()));
        assert_eq!(rcrun_command(&locate_rcrun(None, Some(home))), "rcrun");
        println!(
            "writers: claude-code {} files, flat-test {} files, no path shared; content JSON {} bytes, \
             identical before and after; {} frontmatter files; .mcp.json without the home folder",
            claude.len(),
            flat.len(),
            before.len(),
            fm.len()
        );
    }
}
