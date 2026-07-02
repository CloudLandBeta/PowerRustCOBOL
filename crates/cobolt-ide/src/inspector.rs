// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Run-Form process inspector.
//!
//! Samples **the IDE process itself (and its child processes)** while the Live
//! Interpreter (Run Form) is running, feeds real-time charts, and — when it sees
//! suspicious behaviour (sustained memory growth while idle, CPU pegged while
//! idle, or a burst of child processes) — dumps process/memory detail to the
//! console **and** a configurable dump file.
//!
//! The Live Interpreter runs on a background *thread* inside the IDE process, so
//! there is no separate OS process to isolate; the metrics are therefore
//! process-wide. Because the inspector only samples while a form is running, an
//! idle form shows a flat line and a runaway handler shows growth — which is
//! exactly the leak signal we want.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// Samples retained for the rolling charts (~90 s at one sample per 500 ms).
pub const HISTORY: usize = 180;

/// The minimum gap between two dumps, so a persistent anomaly does not spam.
const DUMP_COOLDOWN: Duration = Duration::from_secs(20);

/// One point on the inspector's timeline.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    /// Process CPU %, normalised to one core (can exceed 100 on multi-core).
    pub cpu_pct: f32,
    /// Resident memory (RSS) of the IDE process, in bytes.
    pub rss_bytes: u64,
    /// Number of child processes of the IDE process.
    pub children: usize,
    /// System-wide CPU %.
    pub sys_cpu_pct: f32,
    /// System memory used / total, in bytes.
    pub sys_mem_used: u64,
    pub sys_mem_total: u64,
    /// Whether the interpreter had queued work when this sample was taken
    /// (used to distinguish "growth while idle" from legitimate processing).
    pub processing: bool,
}

/// Per-project inspector configuration (persisted in `cobolt.toml`).
#[derive(Clone, Debug)]
pub struct InspectorConfig {
    /// Write a dump (console + file) when a suspicious pattern is detected.
    pub dump_enabled: bool,
    /// Where the dump file is written.
    pub dump_path: String,
    /// Sustained RSS growth (MB) across the window, **while idle**, that trips a dump.
    pub rss_growth_mb: f32,
    /// Process CPU % that counts as "busy" while the interpreter reports idle.
    pub cpu_idle_pct: f32,
    /// More child processes than this is considered suspicious.
    pub max_children: usize,
}

impl Default for InspectorConfig {
    fn default() -> Self {
        Self {
            dump_enabled: true,
            dump_path: "/tmp/prc_inspector_dump.txt".to_string(),
            rss_growth_mb: 50.0,
            cpu_idle_pct: 25.0,
            max_children: 8,
        }
    }
}

/// Samples process/system metrics and detects suspicious behaviour.
pub struct ProcessInspector {
    sys: System,
    pid: Pid,
    hist: VecDeque<Sample>,
    last_sample: Option<Instant>,
    interval: Duration,
    /// RSS at the start of the current idle stretch, for growth detection.
    idle_baseline_rss: Option<u64>,
    last_dump: Option<Instant>,
    pub config: InspectorConfig,
    /// Human-readable description of the most recent anomaly, shown in the panel.
    pub last_anomaly: Option<String>,
}

impl ProcessInspector {
    pub fn new(config: InspectorConfig) -> Self {
        let pid = sysinfo::get_current_pid().unwrap_or(Pid::from(0));
        Self {
            sys: System::new(),
            pid,
            hist: VecDeque::with_capacity(HISTORY),
            last_sample: None,
            interval: Duration::from_millis(500),
            idle_baseline_rss: None,
            last_dump: None,
            config,
            last_anomaly: None,
        }
    }

    /// Clear the timeline (call when a new Run Form starts).
    pub fn reset(&mut self) {
        self.hist.clear();
        self.idle_baseline_rss = None;
        self.last_anomaly = None;
        self.last_sample = None;
    }

    pub fn history(&self) -> &VecDeque<Sample> {
        &self.hist
    }

    pub fn latest(&self) -> Option<Sample> {
        self.hist.back().copied()
    }

    /// Sample now if the interval has elapsed. `processing` reflects whether the
    /// interpreter currently has queued work (so we can flag growth *while idle*).
    pub fn maybe_sample(&mut self, processing: bool) {
        let now = Instant::now();
        if let Some(last) = self.last_sample {
            if now.duration_since(last) < self.interval {
                return;
            }
        }
        self.last_sample = Some(now);
        let sample = self.collect(processing);
        if self.hist.len() >= HISTORY {
            self.hist.pop_front();
        }
        self.hist.push_back(sample);
        self.detect(sample);
    }

    /// Read the current process/system metrics from `sysinfo`.
    fn collect(&mut self, processing: bool) -> Sample {
        // CPU % needs two refreshes separated by ≥200 ms; our 500 ms cadence
        // satisfies that, so each refresh yields usage since the previous one.
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            ProcessRefreshKind::new().with_cpu().with_memory(),
        );

        let (cpu_pct, rss_bytes) = self
            .sys
            .process(self.pid)
            .map(|p| (p.cpu_usage(), p.memory()))
            .unwrap_or((0.0, 0));

        let children = self
            .sys
            .processes()
            .values()
            .filter(|p| p.parent() == Some(self.pid))
            .count();

        Sample {
            cpu_pct,
            rss_bytes,
            children,
            sys_cpu_pct: self.sys.global_cpu_usage(),
            sys_mem_used: self.sys.used_memory(),
            sys_mem_total: self.sys.total_memory(),
            processing,
        }
    }

    /// Suspicious-behaviour heuristics → console + file dump (rate-limited).
    fn detect(&mut self, sample: Sample) {
        // Track a baseline RSS across each idle stretch. Reset it whenever the
        // interpreter is processing (legitimate growth is expected then).
        if sample.processing {
            self.idle_baseline_rss = None;
        } else {
            let base = *self.idle_baseline_rss.get_or_insert(sample.rss_bytes);
            let growth_mb = (sample.rss_bytes.saturating_sub(base)) as f32 / (1024.0 * 1024.0);
            if growth_mb >= self.config.rss_growth_mb {
                self.flag(format!(
                    "memory grew {growth_mb:.1} MB while idle (possible leak)"
                ));
            } else if sample.cpu_pct >= self.config.cpu_idle_pct {
                self.flag(format!(
                    "CPU at {:.0}% while idle (possible runaway loop)",
                    sample.cpu_pct
                ));
            }
        }
        if sample.children > self.config.max_children {
            self.flag(format!(
                "{} child processes (possible rogue subprocesses)",
                sample.children
            ));
        }
    }

    fn flag(&mut self, reason: String) {
        self.last_anomaly = Some(reason.clone());
        // Rate-limit dumps so a persistent condition writes at most one per cooldown.
        let now = Instant::now();
        if let Some(last) = self.last_dump {
            if now.duration_since(last) < DUMP_COOLDOWN {
                return;
            }
        }
        self.last_dump = Some(now);
        self.dump(&reason);
    }

    /// Emit a full process/memory dump to the console and (if enabled) the file.
    fn dump(&self, reason: &str) {
        let report = self.build_report(reason);
        // Console — always, so it is visible in the Output/terminal.
        eprintln!("{report}");
        if self.config.dump_enabled && !self.config.dump_path.trim().is_empty() {
            use std::io::Write as _;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.config.dump_path)
            {
                let _ = writeln!(f, "{report}\n");
            }
        }
    }

    /// Build the dump text: header + latest metrics + child-process breakdown.
    pub fn build_report(&self, reason: &str) -> String {
        let s = self.latest().unwrap_or_default();
        let mb = |b: u64| b as f64 / (1024.0 * 1024.0);
        let mut out = String::new();
        out.push_str(&format!(
            "── PowerRustCOBOL inspector dump ──\nreason: {reason}\npid: {}\n\
             process CPU: {:.1}%   RSS: {:.1} MB   children: {}\n\
             system CPU: {:.1}%   mem: {:.0}/{:.0} MB\n",
            self.pid,
            s.cpu_pct,
            mb(s.rss_bytes),
            s.children,
            s.sys_cpu_pct,
            mb(s.sys_mem_used),
            mb(s.sys_mem_total),
        ));
        // Child-process breakdown (name, pid, CPU, RSS).
        let mut kids: Vec<_> = self
            .sys
            .processes()
            .values()
            .filter(|p| p.parent() == Some(self.pid))
            .collect();
        kids.sort_by(|a, b| b.memory().cmp(&a.memory()));
        for p in kids {
            out.push_str(&format!(
                "  child {} '{}'  CPU {:.1}%  RSS {:.1} MB\n",
                p.pid(),
                p.name().to_string_lossy(),
                p.cpu_usage(),
                mb(p.memory()),
            ));
        }
        out
    }

    /// The application's process subtree, pre-order (depth-first) from the IDE
    /// process down through its descendants: `(depth, pid, name, cpu%, rss_bytes)`.
    /// Uses the process list captured at the last sample.
    pub fn process_tree(&self) -> Vec<(usize, String, String, f32, u64)> {
        use std::collections::{HashMap, HashSet};
        let mut kids: HashMap<Pid, Vec<Pid>> = HashMap::new();
        for (pid, p) in self.sys.processes() {
            if let Some(parent) = p.parent() {
                kids.entry(parent).or_default().push(*pid);
            }
        }
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        // DFS with a stack; push children reversed so they pop in ascending order.
        let mut stack = vec![(self.pid, 0usize)];
        while let Some((pid, depth)) = stack.pop() {
            if !seen.insert(pid) || depth > 6 {
                continue;
            }
            if let Some(p) = self.sys.process(pid) {
                out.push((
                    depth,
                    pid.to_string(),
                    p.name().to_string_lossy().to_string(),
                    p.cpu_usage(),
                    p.memory(),
                ));
                if let Some(children) = kids.get(&pid) {
                    let mut cs = children.clone();
                    cs.sort_unstable();
                    for c in cs.into_iter().rev() {
                        stack.push((c, depth + 1));
                    }
                }
            }
        }
        out
    }

    /// Test-only: feed a synthetic sample through the anomaly heuristics without
    /// touching `sysinfo`, so the detection logic is deterministic.
    #[cfg(test)]
    fn test_feed(&mut self, s: Sample) {
        if self.hist.len() >= HISTORY {
            self.hist.pop_front();
        }
        self.hist.push_back(s);
        self.detect(s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mb(x: f32) -> u64 {
        (x * 1024.0 * 1024.0) as u64
    }

    fn idle(rss_mb: f32, cpu: f32, children: usize) -> Sample {
        Sample {
            cpu_pct: cpu,
            rss_bytes: mb(rss_mb),
            children,
            processing: false,
            ..Default::default()
        }
    }

    fn inspector() -> ProcessInspector {
        let mut cfg = InspectorConfig::default();
        cfg.dump_enabled = false; // don't write a file during tests
        cfg.rss_growth_mb = 50.0;
        cfg.cpu_idle_pct = 25.0;
        cfg.max_children = 4;
        ProcessInspector::new(cfg)
    }

    #[test]
    fn sustained_idle_memory_growth_is_flagged_as_leak() {
        let mut insp = inspector();
        insp.test_feed(idle(100.0, 1.0, 0)); // baseline
        assert!(insp.last_anomaly.is_none(), "flat baseline is healthy");
        insp.test_feed(idle(120.0, 1.0, 0)); // +20 MB, below 50 threshold
        assert!(insp.last_anomaly.is_none(), "small growth is not yet a leak");
        insp.test_feed(idle(170.0, 1.0, 0)); // +70 MB from baseline → leak
        let a = insp.last_anomaly.as_deref().unwrap_or("");
        assert!(a.contains("memory grew"), "expected a leak flag, got {a:?}");
    }

    #[test]
    fn idle_form_with_flat_memory_stays_healthy() {
        let mut insp = inspector();
        for _ in 0..20 {
            insp.test_feed(idle(150.0, 0.5, 0)); // idle: needle must not move
        }
        assert!(
            insp.last_anomaly.is_none(),
            "an idle form with flat RSS must not trip any anomaly"
        );
    }

    #[test]
    fn cpu_pegged_while_idle_is_flagged() {
        let mut insp = inspector();
        insp.test_feed(idle(100.0, 90.0, 0)); // 90% CPU while idle
        let a = insp.last_anomaly.as_deref().unwrap_or("");
        assert!(a.contains("CPU"), "expected a runaway-CPU flag, got {a:?}");
    }

    #[test]
    fn processing_growth_is_not_a_leak() {
        let mut insp = inspector();
        // While the interpreter is processing, growth is expected — no leak flag.
        let mut s = idle(100.0, 5.0, 0);
        s.processing = true;
        insp.test_feed(s);
        let mut s2 = idle(400.0, 5.0, 0);
        s2.processing = true;
        insp.test_feed(s2);
        assert!(
            insp.last_anomaly.is_none(),
            "growth while processing must not be flagged as a leak"
        );
    }

    #[test]
    fn too_many_children_is_flagged() {
        let mut insp = inspector();
        insp.test_feed(idle(100.0, 1.0, 9)); // 9 > max_children(4)
        let a = insp.last_anomaly.as_deref().unwrap_or("");
        assert!(a.contains("child processes"), "expected a rogue-subprocess flag, got {a:?}");
    }
}
