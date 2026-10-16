<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Baseline — before any code change (task T1)

Measured 2026-10-16 on `features-aws-native` at `main` 2e57112f (version 1.91.179), debug profile, macOS, rustc 1.95.0,
in an APFS-cloned `target/` of this worktree. Every number below is a `test result:` line this run produced.

| Suite | Command (abridged) | Result |
|---|---|---|
| runtime, inline `aws::` | `cargo test -p cobolt-runtime --lib aws::` | 22 passed |
| runtime, integration | `--test aws_controls` … `aws_routes` (11 files) | 44 passed (5 + 7 + 6 + 9 + 3 + 1 + 1 + 6 + 1 + 1 + 4) |
| cli, `aws_hosts` | `cargo test -p cobolt-cli --test aws_hosts` | 2 passed, 1 ignored (the built-binary run) — 129.9 s |
| cli, `powercloud_hosts` | `--test powercloud_hosts` | 7 passed — 71.8 s |
| compiler, dependency guards | `--test dependency_guards_063` | 2 passed |
| compiler, feature selection | `--lib runtime_features` | 25 passed |
| forms, AWS catalogue | `--test aws_controls_078` | 1 passed |

Totals: **53 AWS integration tests pass by default** (44 runtime + 2 + 7 cli), plus 22 inline, plus 1 ignored built-binary run
and the ignored live test `aws_live.rs`. Spec AC11 compares against these numbers.

Not yet measured: `cargo test -p cobolt-ide --bin cobolt-ide` (taken before the first IDE change, in P4; the handoff lists seven
failures that are not this work's).
