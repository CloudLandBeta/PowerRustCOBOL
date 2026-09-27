// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Where the built-in semantic model lives, and fetching it (spec 068 R23,
//! R23a).
//!
//! The model is `intfloat/multilingual-e5-small`: about 470 MB, cached **per
//! application** in the application's own folder — `<app>/models/`, beside
//! `assets/`, which is where a build that links the model puts it
//! (operator, 2026-09-27) — shared by every user of that installation. See
//! [`app_models_dir`] for every place it is looked for. It is fetched only when the
//! application asks, never on start-up. Each file is written as `<file>.part`
//! and renamed when complete, so another user of the installation never sees
//! half a file. This module needs no model code, so fetching works in any
//! build; running the model needs the `semantic` feature.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::embed::Transport;

pub const MODEL_ID: &str = "intfloat/multilingual-e5-small";
/// The stamp the built-in model's vectors carry — named here, outside the
/// `semantic` feature, so a build without the model can still recognise them.
pub const BUILTIN_STAMP: &str = "builtin:multilingual-e5-small";
const MODEL_NAME: &str = "multilingual-e5-small";
pub(crate) const MODEL_FILES: [&str; 3] = ["config.json", "tokenizer.json", "model.safetensors"];
const HF_RESOLVE_BASE: &str = "https://huggingface.co";

/// The variable a host sets to a folder that already holds the model — Run
/// Form points it at the IDE's own copy, so running from the IDE never
/// downloads the model a second time.
pub const MODELS_ENV: &str = "COBOL_KB_MODELS";

/// Where an application's built-in model is: the first of these that holds
/// every model file —
///
/// 1. `$COBOL_KB_MODELS`, when a host set it;
/// 2. `<app>/models` — beside `assets/`, where a build puts it;
/// 3. `<app>/assets/models` — where applications kept it before.
///
/// None holds it: `<app>/models`, which is where `FetchModel` then puts it.
pub fn app_models_dir(app: &Path) -> PathBuf {
    let own = app.join("models");
    let from_env = std::env::var_os(MODELS_ENV).map(PathBuf::from);
    [from_env, Some(own.clone()), Some(app.join("assets").join("models"))]
        .into_iter()
        .flatten()
        .find(|d| model_is_cached(d))
        .unwrap_or(own)
}

/// The model's folder inside `models_dir` (see [`app_models_dir`]).
pub fn model_dir(models_dir: &Path) -> PathBuf {
    models_dir.join(MODEL_NAME)
}

/// Whether every model file is present.
pub fn model_is_cached(models_dir: &Path) -> bool {
    let dir = model_dir(models_dir);
    MODEL_FILES.iter().all(|f| dir.join(f).is_file())
}

/// Fetch the files that are missing. Reports `(file, received, total)`; stops
/// between and within files when `cancel` is raised. Files already present are
/// never fetched again.
pub fn fetch_model(
    models_dir: &Path,
    transport: &dyn Transport,
    progress: &mut dyn FnMut(&str, u64, Option<u64>),
    cancel: &AtomicBool,
) -> Result<PathBuf, String> {
    let dir = model_dir(models_dir);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("could not create the model folder {}: {e}", dir.display()))?;
    for file in MODEL_FILES {
        let dest = dir.join(file);
        if dest.is_file() {
            continue;
        }
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        let part = dir.join(format!("{file}.part"));
        let url = format!("{HF_RESOLVE_BASE}/{MODEL_ID}/resolve/main/{file}");
        transport.get_to_file(&url, &part, &mut |got, total| progress(file, got, total), cancel)?;
        std::fs::rename(&part, &dest)
            .map_err(|e| format!("could not finish {}: {e}", dest.display()))?;
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Operator (2026-09-27): a build puts the model in `models/`, beside
    /// `assets/`, and the application must find it there — while one that
    /// kept it in `assets/models` still finds it.
    #[test]
    fn the_model_is_found_beside_assets_or_where_it_used_to_be() {
        let plant = |d: &Path| {
            let m = model_dir(d);
            std::fs::create_dir_all(&m).unwrap();
            for f in MODEL_FILES {
                std::fs::write(m.join(f), b"x").unwrap();
            }
        };
        let app = tempfile::tempdir().unwrap();
        let app = app.path();
        assert_eq!(app_models_dir(app), app.join("models"), "nothing yet: fetched into models/");
        plant(&app.join("assets").join("models"));
        assert_eq!(app_models_dir(app), app.join("assets").join("models"), "the old place still works");
        plant(&app.join("models"));
        assert_eq!(app_models_dir(app), app.join("models"), "beside assets/ wins");
        println!("model lookup — empty → models/; assets/models only → assets/models; both → models/");
    }

    struct Files(Mutex<Vec<String>>);
    impl Transport for Files {
        fn post_json(&self, _: &str, _: &[(String, String)], _: &str, _: u64) -> Result<(u16, String), String> {
            Err("unused".into())
        }
        fn get_to_file(
            &self,
            url: &str,
            dest: &Path,
            progress: &mut dyn FnMut(u64, Option<u64>),
            _cancel: &AtomicBool,
        ) -> Result<(), String> {
            self.0.lock().unwrap().push(url.to_string());
            std::fs::write(dest, b"x").map_err(|e| e.to_string())?;
            progress(1, Some(1));
            Ok(())
        }
    }

    /// Fetched once into the application's folder; a second instance finds it
    /// and fetches nothing (AC13b).
    #[test]
    fn the_model_is_fetched_once_per_installation() {
        let app = tempfile::tempdir().unwrap();
        let models = app.path().join("assets").join("models");
        let t = Files(Mutex::new(Vec::new()));
        let cancel = AtomicBool::new(false);
        assert!(!model_is_cached(&models));
        fetch_model(&models, &t, &mut |_, _, _| {}, &cancel).unwrap();
        assert!(model_is_cached(&models));
        assert_eq!(t.0.lock().unwrap().len(), 3);
        assert!(t.0.lock().unwrap()[2].ends_with("/intfloat/multilingual-e5-small/resolve/main/model.safetensors"));
        let second = Files(Mutex::new(Vec::new()));
        fetch_model(&models, &second, &mut |_, _, _| {}, &cancel).unwrap();
        assert!(second.0.lock().unwrap().is_empty(), "nothing fetched again");
        assert!(!model_dir(&models).join("model.safetensors.part").exists());
    }
}
