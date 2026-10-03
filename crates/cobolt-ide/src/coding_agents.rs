// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The coding agents the IDE can connect** (spec 084, first-run wizard).
//!
//! Claude Code is the first and, for now, the only one. The wizard and the
//! Coding Agent Settings window name the agent from here and dispatch on its
//! [`AgentId`], so a second agent adds an entry and its detect/configure
//! arm — never a second wizard.

/// Which agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentId {
    ClaudeCode,
}

/// One agent the IDE can connect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodingAgent {
    pub id: AgentId,
    /// Stable key, stored in the IDE's settings (e.g. "don't ask again").
    pub key: &'static str,
    /// Product name, shown as is in every language.
    pub name: &'static str,
}

/// Every agent the IDE knows, in the order the wizard lists them.
pub const AGENTS: &[CodingAgent] = &[CodingAgent { id: AgentId::ClaudeCode, key: "claude-code", name: "Claude Code" }];

/// Whether the IDE is set up with an agent, as the wizard needs to know it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Connection {
    /// Not checked yet.
    Unknown,
    /// The agent is not installed on this machine.
    NotInstalled,
    /// Installed at `path`, without the PowerRustCOBOL integration.
    NotConnected { path: String },
    /// Installed and connected.
    Connected,
}

/// Spec 084 (wizard): offer to connect at start when the agent is not
/// connected and the developer has not said "don't show again" for it.
pub fn should_offer(agent: &CodingAgent, connection: &Connection, dismissed: &[String]) -> bool {
    !dismissed.iter().any(|k| k == agent.key)
        && matches!(connection, Connection::NotInstalled | Connection::NotConnected { .. })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wizard_is_offered_only_when_not_connected_and_not_dismissed() {
        let claude = AGENTS[0];
        let none: Vec<String> = Vec::new();
        let not_connected = Connection::NotConnected { path: "/opt/homebrew/bin/claude".into() };
        assert!(should_offer(&claude, &not_connected, &none));
        assert!(should_offer(&claude, &Connection::NotInstalled, &none));
        assert!(!should_offer(&claude, &Connection::Connected, &none));
        assert!(!should_offer(&claude, &Connection::Unknown, &none), "never before the check answers");
        assert!(!should_offer(&claude, &not_connected, &["claude-code".into()]), "don't show again");
        println!("wizard offer: not connected / not installed -> offered; connected, unchecked or dismissed -> not");
    }
}
