// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! The free-space rule (R4, amendment A2).
//!
//! The setup writes to two places that are often, but not always, the same
//! volume: the developer's build folder (the compiled platform, and Rust in the
//! user's profile) and the system volume (the Visual Studio Build Tools). The
//! rule is checked on each volume it writes to, and the volumes are added
//! together when they are one.

use crate::figures::Figures;

/// A volume and what is free on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volume {
    /// How the page names it: `C:`.
    pub name: String,
    /// Free bytes.
    pub free_bytes: u64,
}

/// The two volumes the preparation writes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Free {
    /// Where the build folder, and the user's Rust, are.
    pub build: Volume,
    /// Where the Visual Studio Build Tools go.
    pub system: Volume,
}

/// How much each volume must hold, before the margin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Needs {
    /// On the build folder's volume.
    pub build_bytes: u64,
    /// On the system volume.
    pub system_bytes: u64,
}

/// A volume that cannot hold what is asked of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortfall {
    /// The volume, as the page names it.
    pub volume: String,
    /// What the preparation needs there, margin included.
    pub need_bytes: u64,
    /// What is free there.
    pub free_bytes: u64,
}

fn same_volume(a: &str, b: &str) -> bool {
    a.trim_end_matches(['\\', '/'])
        .eq_ignore_ascii_case(b.trim_end_matches(['\\', '/']))
}

/// The volumes that fall short. Empty means there is room. A volume that is asked for nothing never falls short,
/// whatever is free on it, and the margin is added only where something is asked for.
pub fn check(figures: &Figures, free: &Free, needs: &Needs) -> Vec<Shortfall> {
    let margin = figures.margin_bytes;
    let mut out = Vec::new();
    let mut test = |volume: &str, need: u64, have: u64| {
        if need == 0 {
            return;
        }
        let required = need.saturating_add(margin);
        if have < required {
            out.push(Shortfall {
                volume: volume.to_owned(),
                need_bytes: required,
                free_bytes: have,
            });
        }
    };
    if same_volume(&free.build.name, &free.system.name) {
        // One volume: it holds both, and it is one volume, so its free space is counted once.
        test(
            &free.build.name,
            needs.build_bytes.saturating_add(needs.system_bytes),
            free.build.free_bytes,
        );
    } else {
        test(&free.build.name, needs.build_bytes, free.build.free_bytes);
        test(
            &free.system.name,
            needs.system_bytes,
            free.system.free_bytes,
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::{FIGURES, GB};

    fn vol(name: &str, gb: u64) -> Volume {
        Volume {
            name: name.to_owned(),
            free_bytes: gb * GB,
        }
    }

    #[test]
    fn the_rule_holds_at_its_edges() {
        let m = FIGURES.margin_bytes;
        let free = |bytes: u64| Free {
            build: Volume {
                name: "C:".into(),
                free_bytes: bytes,
            },
            system: vol("C:", 0),
        };
        let need = Needs {
            build_bytes: 5 * GB,
            system_bytes: 0,
        };
        let mut cases = 0;
        // One byte under, exactly enough, one byte over.
        assert_eq!(
            check(&FIGURES, &free(5 * GB + m - 1), &need).len(),
            1,
            "one byte under falls short"
        );
        assert!(
            check(&FIGURES, &free(5 * GB + m), &need).is_empty(),
            "exactly enough is enough"
        );
        assert!(
            check(&FIGURES, &free(5 * GB + m + 1), &need).is_empty(),
            "one byte over is enough"
        );
        cases += 3;
        // Nothing asked: nothing falls short, even on an empty disk, and no margin is invented.
        assert!(check(&FIGURES, &free(0), &Needs::default()).is_empty());
        cases += 1;
        let s = &check(&FIGURES, &free(GB), &need)[0];
        assert_eq!(
            (s.volume.as_str(), s.need_bytes, s.free_bytes),
            ("C:", 5 * GB + m, GB)
        );
        cases += 1;
        println!("disk rule: {cases} cases (under, exact, over, nothing asked, the shortfall's own figures) all hold");
    }

    #[test]
    fn two_volumes_are_checked_each_and_one_volume_is_counted_once() {
        let m = FIGURES.margin_bytes;
        let need = Needs {
            build_bytes: 5 * GB,
            system_bytes: 6 * GB,
        };

        // D: holds the build folder, C: the Build Tools: each is judged on its own.
        let apart = Free {
            build: vol("D:", 100),
            system: vol("C:", 6),
        };
        let short = check(&FIGURES, &apart, &need);
        assert_eq!(short.len(), 1);
        assert_eq!(short[0].volume, "C:", "only the system volume falls short");
        assert_eq!(short[0].need_bytes, 6 * GB + m);

        // The same volume, said two ways: its space is counted once against the sum.
        let together = Free {
            build: vol("C:", 12),
            system: vol("c:\\", 12),
        };
        let short = check(&FIGURES, &together, &need);
        assert_eq!(
            short.len(),
            1,
            "12 GB free cannot hold 5 + 6 GB and the margin"
        );
        assert_eq!(short[0].need_bytes, 11 * GB + m);
        let together = Free {
            build: vol("C:", 14),
            system: vol("C:", 14),
        };
        assert!(check(&FIGURES, &together, &need).is_empty());
        println!("disk rule: apart (one short), together (counted once), enough: 3 cases hold");
    }
}
