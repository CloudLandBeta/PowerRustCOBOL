// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The SQL connection editor (spec 087 R33): one of the project's named SQL
//! connections, opened in the main pane from the project tree's SQL
//! Connections item.
//!
//! The project file keeps the definition — name, backend, the SQLite path or
//! the server's host, port and database, the default mark and
//! create-if-missing. The user name and password NEVER go there (R52): the
//! app keeps them in the IDE's credential vault, under
//! [`cobolt_forms::connections::sql_credential_slot`], the store and policy
//! of the model API keys. This panel only edits; the app saves, removes,
//! moves credentials on a rename and refreshes the tree.
//!
//! **Test connection** connects with the values in the editor — saved or
//! not — on a worker thread, and shows the database's own message.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

use cobolt_forms::connections::{sql_env_name, SqlConnection};
use egui::{Color32, RichText, Ui};

use crate::i18n::Tr;

/// What the developer asked for this frame.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlEditorAction {
    None,
    /// Save the draft: the validated definition and the credentials typed.
    Save { def: SqlConnection, user: String, password: String },
    /// Remove the connection being edited (already confirmed).
    Remove,
    /// Close the editor without saving.
    Close,
}

/// The editor's state: a draft of one SQL connection.
pub struct SqlConnectionEditor {
    /// The name the connection is saved under; `None` for a new one.
    pub original: Option<String>,
    pub draft: SqlConnection,
    port_text: String,
    pub user: String,
    pub password: String,
    /// Whether the vault already holds a password (shown, never read back).
    pub password_stored: bool,
    confirm_remove: bool,
    error: Option<String>,
    test: TestState,
    /// What a relative SQLite path is relative to: the project folder.
    base_dir: PathBuf,
}

enum TestState {
    Idle,
    Running(Receiver<Result<String, String>>),
    Done(Result<String, String>),
}

impl SqlConnectionEditor {
    /// An editor over `existing`, or a new connection when `None`.
    pub fn new(existing: Option<&SqlConnection>, user: String, password_stored: bool, base_dir: &Path) -> Self {
        let draft = existing.cloned().unwrap_or_else(|| {
            let mut c = SqlConnection::new("");
            c.path = "data/".into();
            c
        });
        Self {
            original: existing.map(|c| c.name.clone()),
            port_text: draft.port.map(|p| p.to_string()).unwrap_or_default(),
            draft,
            user,
            password: String::new(),
            password_stored,
            confirm_remove: false,
            error: None,
            test: TestState::Idle,
            base_dir: base_dir.to_path_buf(),
        }
    }

    fn is_sqlite(&self) -> bool {
        is_sqlite(&self.draft.backend)
    }

    /// The draft as a definition, checked against the project's `others`:
    /// a name, unique after the environment-variable normalization (two
    /// names that map to the same `<APP>_SQL_<NAME>_*` variables would share
    /// credentials), and a valid port.
    pub fn validated(&self, others: &[SqlConnection], tr: &Tr) -> Result<SqlConnection, String> {
        let mut def = self.draft.clone();
        def.name = def.name.trim().to_string();
        if def.name.is_empty() {
            return Err(tr.sqlc_err_name_empty.to_string());
        }
        let key = sql_env_name(&def.name);
        let taken = others.iter().any(|o| {
            let same_entry = self.original.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(&o.name));
            !same_entry && sql_env_name(&o.name) == key
        });
        if taken {
            return Err(tr.sqlc_err_name_taken.to_string());
        }
        def.port = match self.port_text.trim() {
            "" => None,
            t => Some(t.parse::<u16>().ok().filter(|p| *p > 0).ok_or_else(|| tr.sqlc_err_port.to_string())?),
        };
        def.user.clear();
        if is_sqlite(&def.backend) {
            def.backend = "sqlite".into();
            def.host.clear();
            def.port = None;
            def.database.clear();
        } else {
            def.path.clear();
            def.create_if_missing = false;
        }
        Ok(def)
    }

    /// Collect a finished test, if one is running.
    fn poll_test(&mut self) {
        if let TestState::Running(rx) = &self.test {
            if let Ok(r) = rx.try_recv() {
                self.test = TestState::Done(r);
            }
        }
    }

    /// Draw the editor. `others` is the project's SQL connections.
    pub fn show(&mut self, ui: &mut Ui, others: &[SqlConnection], tr: &Tr) -> SqlEditorAction {
        self.poll_test();
        let mut action = SqlEditorAction::None;
        let title = match &self.original {
            Some(n) => format!("{} — {n}", tr.sqlc_title_edit),
            None => tr.sqlc_title_new.to_string(),
        };
        ui.heading(title);
        ui.add_space(8.0);

        egui::Grid::new("sql_connection_editor").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label(tr.sqlc_name);
            ui.add(egui::TextEdit::singleline(&mut self.draft.name).desired_width(280.0));
            ui.end_row();

            ui.label(tr.sqlc_backend);
            egui::ComboBox::from_id_salt("sqlc_backend")
                .selected_text(backend_label(&self.draft.backend))
                .show_ui(ui, |ui| {
                    for b in ["sqlite", "postgresql", "mysql"] {
                        if ui.selectable_label(self.draft.backend.eq_ignore_ascii_case(b), backend_label(b)).clicked() {
                            self.draft.backend = b.to_string();
                        }
                    }
                });
            ui.end_row();

            if self.is_sqlite() {
                ui.label(tr.sqlc_path);
                ui.add(egui::TextEdit::singleline(&mut self.draft.path).desired_width(280.0));
                ui.end_row();
                ui.label("");
                ui.checkbox(&mut self.draft.create_if_missing, tr.sqlc_create_if_missing);
                ui.end_row();
            } else {
                ui.label(tr.sqlc_host);
                ui.add(egui::TextEdit::singleline(&mut self.draft.host).desired_width(280.0));
                ui.end_row();
                ui.label(tr.sqlc_port);
                ui.add(egui::TextEdit::singleline(&mut self.port_text).desired_width(80.0));
                ui.end_row();
                ui.label(tr.sqlc_database);
                ui.add(egui::TextEdit::singleline(&mut self.draft.database).desired_width(280.0));
                ui.end_row();
                ui.label(tr.sqlc_user);
                ui.add(egui::TextEdit::singleline(&mut self.user).desired_width(200.0));
                ui.end_row();
                ui.label(tr.sqlc_password);
                let hint = if self.password_stored { tr.sqlc_password_stored } else { "" };
                ui.add(
                    egui::TextEdit::singleline(&mut self.password)
                        .password(true)
                        .hint_text(hint)
                        .desired_width(200.0),
                );
                ui.end_row();
            }

            ui.label("");
            ui.checkbox(&mut self.draft.default, tr.sqlc_default);
            ui.end_row();
        });
        if !self.is_sqlite() {
            ui.label(RichText::new(tr.sqlc_credentials_note).small().italics());
        }

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button(tr.sqlc_save).clicked() {
                match self.validated(others, tr) {
                    Ok(def) => {
                        self.error = None;
                        action = SqlEditorAction::Save {
                            def,
                            user: self.user.trim().to_string(),
                            password: std::mem::take(&mut self.password),
                        };
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            let running = matches!(self.test, TestState::Running(_));
            if ui.add_enabled(!running, egui::Button::new(tr.sqlc_test)).clicked() {
                match self.validated(others, tr) {
                    Ok(def) => {
                        self.error = None;
                        let (tx, rx) = channel();
                        let (base, user, password) = (self.base_dir.clone(), self.user.clone(), self.password.clone());
                        std::thread::spawn(move || {
                            let _ = tx.send(test_connection_with(&def, &base, &user, &password));
                        });
                        self.test = TestState::Running(rx);
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            if self.original.is_some() && ui.button(tr.sqlc_remove).clicked() {
                self.confirm_remove = true;
            }
            if ui.button(tr.btn_cancel).clicked() {
                action = SqlEditorAction::Close;
            }
        });

        if let Some(e) = &self.error {
            ui.colored_label(Color32::from_rgb(220, 90, 80), e);
        }
        match &self.test {
            TestState::Idle => {}
            TestState::Running(_) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(tr.sqlc_testing);
                });
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
            }
            TestState::Done(Ok(msg)) => {
                ui.colored_label(Color32::from_rgb(80, 170, 100), format!("{} {msg}", tr.sqlc_test_ok));
            }
            TestState::Done(Err(msg)) => {
                ui.colored_label(Color32::from_rgb(220, 90, 80), tr.sqlc_test_failed);
                ui.label(RichText::new(msg).monospace().small());
            }
        }

        if self.confirm_remove {
            ui.add_space(8.0);
            ui.label(tr.sqlc_remove_confirm);
            ui.horizontal(|ui| {
                if ui.button(tr.sqlc_remove).clicked() {
                    self.confirm_remove = false;
                    action = SqlEditorAction::Remove;
                }
                if ui.button(tr.btn_cancel).clicked() {
                    self.confirm_remove = false;
                }
            });
        }
        action
    }
}

fn is_sqlite(backend: &str) -> bool {
    matches!(backend.trim().to_ascii_lowercase().as_str(), "" | "sqlite")
}

fn backend_label(backend: &str) -> &'static str {
    match backend.trim().to_ascii_lowercase().as_str() {
        "postgresql" | "postgres" => "PostgreSQL",
        "mysql" => "MySQL",
        _ => "SQLite",
    }
}

/// Connect with the editor's values — saved or not — and run `SELECT 1`
/// (R33): `Ok` with what was reached, or the database's own message. A
/// relative SQLite path is the project folder's; a SQLite file that does not
/// exist is reported, never created.
pub fn test_connection_with(def: &SqlConnection, base_dir: &Path, user: &str, password: &str) -> Result<String, String> {
    use cobolt_runtime::esql::backend::BackendKind;
    use cobolt_runtime::esql::catalog::{Secret, Target};
    let target = if is_sqlite(&def.backend) {
        Target::Sqlite { path: base_dir.join(def.path.trim()), create: false }
    } else {
        Target::Server {
            kind: if def.backend.eq_ignore_ascii_case("mysql") { BackendKind::MySql } else { BackendKind::Postgres },
            host: def.host.trim().to_string(),
            port: def.port,
            database: def.database.trim().to_string(),
            user: (!user.trim().is_empty()).then(|| user.trim().to_string()),
            password: (!password.is_empty()).then(|| Secret::new(password)),
        }
    };
    cobolt_runtime::esql::catalog::test_connection(&target)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R33 — the function behind Test connection: a SQLite file it reaches,
    /// a missing file it reports without creating, and a PostgreSQL server
    /// on a port nothing listens on — the driver's own connection error.
    #[test]
    fn test_connection_sqlite_ok_pg_closed_port_driver_error() {
        let dir = std::env::temp_dir().join(format!("prc087-testconn-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("data")).unwrap();
        cobolt_runtime::esql::catalog::test_connection(&cobolt_runtime::esql::catalog::Target::Sqlite {
            path: dir.join("data/sales.db"),
            create: true,
        })
        .unwrap();
        let mut sales = SqlConnection::new("SALES");
        sales.path = "data/sales.db".into();
        let ok = test_connection_with(&sales, &dir, "", "").unwrap();
        assert!(ok.contains("SQLite"), "{ok}");

        sales.path = "data/missing.db".into();
        let missing = test_connection_with(&sales, &dir, "", "").unwrap_err();
        assert!(missing.contains("does not exist"), "{missing}");
        assert!(!dir.join("data/missing.db").exists(), "never created");

        // A port nothing listens on: bind one, learn its number, close it.
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let mut pg = SqlConnection::new("WAREHOUSE");
        pg.backend = "postgresql".into();
        pg.host = "127.0.0.1".into();
        pg.port = Some(port);
        pg.database = "stock".into();
        let refused = test_connection_with(&pg, &dir, "someone", "the-password").unwrap_err();
        assert!(refused.to_lowercase().contains("refused"), "the driver's own reason: {refused}");
        assert!(!refused.contains("the-password"), "the password is never shown: {refused}");
        let _ = std::fs::remove_dir_all(&dir);
        println!("Test connection: SQLite → '{ok}'; missing file → '{missing}'; closed PostgreSQL port {port} → '{refused}'");
    }

    /// Names must stay unique after the environment-variable normalization.
    #[test]
    fn names_are_unique_after_normalization() {
        let tr = &crate::i18n::Language::English.tr();
        let mut sales = SqlConnection::new("Sales-DB");
        sales.path = "a.db".into();
        let mut ed = SqlConnectionEditor::new(None, String::new(), false, Path::new("."));
        ed.draft.name = "SALES_db".into();
        assert_eq!(ed.validated(std::slice::from_ref(&sales), tr).unwrap_err(), tr.sqlc_err_name_taken);
        ed.draft.name = " ".into();
        assert_eq!(ed.validated(&[], tr).unwrap_err(), tr.sqlc_err_name_empty);
        // Editing an entry may keep its own name.
        let mut ed = SqlConnectionEditor::new(Some(&sales), String::new(), false, Path::new("."));
        ed.draft.name = "SALES-DB".into();
        assert!(ed.validated(std::slice::from_ref(&sales), tr).is_ok());
        ed.draft.backend = "postgresql".into();
        ed.port_text = "99999".into();
        assert_eq!(ed.validated(&[], tr).unwrap_err(), tr.sqlc_err_port);
    }
}
