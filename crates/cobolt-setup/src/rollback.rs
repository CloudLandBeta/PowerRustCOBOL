// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! What a cancelled setup leaves in the developer's build folder (R14).
//!
//! The compile marks the compiled-crate cache with [`CREATED_BY_SETUP`] **when
//! its own run created that cache**, before it compiles anything. If the install
//! is rolled back, the cache is removed only when that mark is there: a cache
//! the developer already had (from an earlier version, or from their own
//! builds) is theirs and stays, whatever the setup did to it.

use crate::facts::STAMP_FOLDER;
use crate::prepare::CREATED_BY_SETUP;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How many times a removal is tried: the processes of a step that was just stopped may still hold a file for a moment.
const ATTEMPTS: u32 = 10;
/// The wait between two tries.
const WAIT: Duration = Duration::from_millis(300);

/// What the rollback did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Removal {
    /// The cache was the setup's and is gone.
    Removed(PathBuf),
    /// There is no cache in this build folder: nothing was made, so nothing is removed.
    Absent,
    /// There is a cache and the setup did not create it: it stays.
    NotOurs(PathBuf),
    /// The build folder given is not a folder this action may act on, or the removal did not finish.
    Failed(String),
}

/// The compiled-crate cache of a build folder.
pub fn cache_of(build_folder: &Path) -> PathBuf {
    build_folder.join(STAMP_FOLDER[0])
}

/// Remove the cache of `build_folder` if, and only if, the setup created it.
pub fn remove_if_created_by_setup(build_folder: &Path) -> Removal {
    // Never act on a path that is not complete: an empty one would be read against whatever folder the installer
    // happens to be running in.
    if !build_folder.is_absolute() {
        return Removal::Failed(format!(
            "'{}' is not a full path, so nothing was touched",
            build_folder.display()
        ));
    }
    let cache = cache_of(build_folder);
    if !cache.is_dir() {
        return Removal::Absent;
    }
    if !cache.join(CREATED_BY_SETUP).is_file() {
        return Removal::NotOurs(cache);
    }
    let mut last = String::new();
    for attempt in 0..ATTEMPTS {
        match std::fs::remove_dir_all(&cache) {
            Ok(()) => return Removal::Removed(cache),
            Err(_) if !cache.exists() => return Removal::Removed(cache),
            Err(e) => last = e.to_string(),
        }
        if attempt + 1 < ATTEMPTS {
            std::thread::sleep(WAIT);
        }
    }
    Removal::Failed(format!(
        "{} could not be removed: {last}",
        cache.display()
    ))
}

impl Removal {
    /// The line the install log gets.
    pub fn describe(&self) -> String {
        match self {
            Removal::Removed(cache) => format!(
                "the setup created {} and removed it, as the install was cancelled",
                cache.display()
            ),
            Removal::Absent => "the setup made no compiled-crate cache; nothing to remove".to_owned(),
            Removal::NotOurs(cache) => format!(
                "{} was there before this setup and stays",
                cache.display()
            ),
            Removal::Failed(why) => format!("the compiled-crate cache was not removed: {why}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A fresh empty folder under the system's temporary folder, removed by the caller.
    fn scratch(name: &str) -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "prc-rollback-{name}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fill(cache: &Path) {
        let warm = cache.join("warm");
        std::fs::create_dir_all(&warm).unwrap();
        std::fs::write(warm.join("2.1.31"), "").unwrap();
        std::fs::create_dir_all(cache.join("release").join("deps")).unwrap();
        std::fs::write(cache.join("release").join("deps").join("libx.rlib"), "x").unwrap();
    }

    #[test]
    fn a_cache_the_setup_created_is_removed_with_everything_in_it() {
        let build = scratch("ours");
        let cache = cache_of(&build);
        fill(&cache);
        std::fs::write(cache.join(CREATED_BY_SETUP), "2.1.31").unwrap();
        // Something else in the build folder, beside the cache, is not the cache's and stays.
        std::fs::write(build.join("a-project-build.txt"), "keep").unwrap();

        let removal = remove_if_created_by_setup(&build);
        assert_eq!(removal, Removal::Removed(cache.clone()), "{}", removal.describe());
        assert!(!cache.exists());
        assert!(build.join("a-project-build.txt").is_file());
        let _ = std::fs::remove_dir_all(&build);
    }

    #[test]
    fn a_cache_the_developer_already_had_stays_whole() {
        let build = scratch("theirs");
        let cache = cache_of(&build);
        fill(&cache);

        let removal = remove_if_created_by_setup(&build);
        assert_eq!(removal, Removal::NotOurs(cache.clone()));
        assert!(cache.join("warm").join("2.1.31").is_file());
        assert!(cache.join("release").join("deps").join("libx.rlib").is_file());
        let _ = std::fs::remove_dir_all(&build);
    }

    #[test]
    fn no_cache_means_nothing_to_remove() {
        let build = scratch("none");
        assert_eq!(remove_if_created_by_setup(&build), Removal::Absent);
        // A build folder that does not exist at all is the same.
        assert_eq!(
            remove_if_created_by_setup(&build.join("not-there")),
            Removal::Absent
        );
        let _ = std::fs::remove_dir_all(&build);
    }

    #[test]
    fn a_path_that_is_not_complete_is_refused_and_touches_nothing() {
        for path in ["", "builds", "..\\builds"] {
            match remove_if_created_by_setup(Path::new(path)) {
                Removal::Failed(why) => assert!(why.contains("not a full path"), "{why}"),
                other => panic!("'{path}' gave {other:?}"),
            }
        }
    }

    #[test]
    fn the_cache_folder_is_the_one_the_compile_stamps_in() {
        // The stamp folder is `<build>/shared-target/warm`: the cache is its parent, as the compile spells it.
        assert_eq!(STAMP_FOLDER[0], "shared-target");
        assert_eq!(
            cache_of(Path::new("/b")),
            Path::new("/b").join("shared-target")
        );
    }

    #[test]
    fn every_removal_has_a_line_for_the_log() {
        let cache = PathBuf::from("/b/shared-target");
        for removal in [
            Removal::Removed(cache.clone()),
            Removal::Absent,
            Removal::NotOurs(cache),
            Removal::Failed("busy".into()),
        ] {
            assert!(!removal.describe().is_empty());
        }
    }
}
