// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Where a named SQL connection points (spec 087 R33–R35, R39, R52).
//!
//! The definitions come from one place per kind of run: the project file's
//! `[[sql-connections]]` under the IDE and `rcrun`, the deployment file
//! (`sql-connections.toml`) beside a built binary, or whatever the IDE's
//! in-process runner injects. On top of a definition, for application `APP`
//! and connection `NAME`:
//!
//! * `APP_SQL_NAME_URL` replaces the connection target, `APP_SQL_NAME_USER`
//!   the user name;
//! * the password comes ONLY from `APP_SQL_NAME_PASSWORD` or — in a built
//!   application — the application's key store entry `SQL:NAME`. A password
//!   written in the file is refused (`28000`) with a message that names the
//!   file and the two permitted sources.
//!
//! Credentials never travel inside a URL this module builds, and a
//! [`Secret`] prints as `***` whichever way it is formatted.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use cobolt_forms::connections::{sql_env_var, SqlConnection};

use super::backend::BackendKind;
use super::state::{code, SqlError};

/// The reserved connection-string prefix that names a project SQL connection
/// (spec 087 R40): `'sql-connection:SALES'` is what the generated
/// `<id>-CONNECT` of an `SqlDatabase` with `SqlConnection = SALES` opens.
pub const SQL_CONNECTION_PREFIX: &str = "sql-connection:";

/// The SQL connection `conn` names through [`SQL_CONNECTION_PREFIX`], if it
/// does (the prefix compared without regard to case).
pub fn named_connection(conn: &str) -> Option<&str> {
    let t = conn.trim();
    let n = SQL_CONNECTION_PREFIX.len();
    let head = t.get(..n)?;
    head.eq_ignore_ascii_case(SQL_CONNECTION_PREFIX).then(|| t[n..].trim())
}

/// A password. It cannot be printed: `Debug` and `Display` show `***`.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    /// The secret itself — for the driver's login only.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

impl std::fmt::Display for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

/// Where a connection goes, resolved.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// A SQLite file; `create` allows making a missing one.
    Sqlite { path: PathBuf, create: bool },
    /// A connection string, as written (from `_URL` or `CONNECT TO '…'`).
    ConnString(String),
    /// A server, field by field.
    Server {
        kind: BackendKind,
        host: String,
        port: Option<u16>,
        database: String,
        user: Option<String>,
        password: Option<Secret>,
    },
}

/// Which file the definitions came from — named in messages.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Project(PathBuf),
    Deployment(PathBuf),
    Injected,
}

/// The SQL connections a run can name.
#[derive(Debug, Clone)]
pub struct SqlCatalog {
    /// The application's name, for the environment variables.
    pub app: String,
    pub connections: Vec<SqlConnection>,
    /// What a relative SQLite path is relative to.
    pub base_dir: PathBuf,
    pub source: Source,
    /// Names whose entry in the file carries a password (refused).
    pub password_in_file: Vec<String>,
    /// A built application: its key store may hold the password.
    pub key_store: bool,
}

impl SqlCatalog {
    pub fn new(app: impl Into<String>, connections: Vec<SqlConnection>, base_dir: PathBuf, source: Source) -> Self {
        Self { app: app.into(), connections, base_dir, source, password_in_file: Vec::new(), key_store: false }
    }

    /// The project's catalog, from its project file.
    pub fn from_project(manifest: &Path, app: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
        let (connections, password_in_file) = cobolt_forms::connections::parse_sql_connections(&text, "sql-connections")?;
        let base = manifest.parent().map(Path::to_path_buf).unwrap_or_default();
        Ok(Self { password_in_file, ..Self::new(app, connections, base, Source::Project(manifest.to_path_buf())) })
    }

    /// A built application's catalog, from the deployment file beside it.
    pub fn from_deployment_file(file: &Path, app: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
        let (connections, password_in_file) = cobolt_forms::connections::parse_sql_connections(&text, "connection")?;
        let base = file.parent().map(Path::to_path_buf).unwrap_or_default();
        Ok(Self {
            password_in_file,
            key_store: true,
            ..Self::new(app, connections, base, Source::Deployment(file.to_path_buf()))
        })
    }

    pub fn find(&self, name: &str) -> Option<&SqlConnection> {
        self.connections.iter().find(|c| c.name.trim().eq_ignore_ascii_case(name.trim()))
    }

    /// The connection SQL uses when none was made (R35).
    pub fn default_connection(&self) -> Option<&SqlConnection> {
        self.connections.iter().find(|c| c.default)
    }

    fn file_name(&self) -> String {
        match &self.source {
            Source::Project(p) | Source::Deployment(p) => p.display().to_string(),
            Source::Injected => "the IDE's project settings".into(),
        }
    }

    /// Resolve SQL connection `name`, reading the environment through `env`.
    pub fn resolve(&self, name: &str, env: &dyn Fn(&str) -> Option<String>) -> Result<Target, SqlError> {
        let def = self.find(name).ok_or_else(|| {
            SqlError::new(code::CANNOT_CONNECT, format!("there is no SQL connection named '{}' in this project", name.trim()))
        })?;
        let var = |part: &str| sql_env_var(&self.app, &def.name, part);
        if self.password_in_file.iter().any(|n| n.eq_ignore_ascii_case(&def.name)) {
            return Err(SqlError::new(
                code::INVALID_AUTHORIZATION,
                format!(
                    "{} holds a password for SQL connection {}; it is not used — remove it, and supply the password \
                     through the environment variable {} or the application's key store entry SQL:{}",
                    self.file_name(),
                    def.name,
                    var("PASSWORD"),
                    def.name.to_ascii_uppercase()
                ),
            ));
        }
        let user = env(&var("USER")).filter(|u| !u.is_empty()).or_else(|| (!def.user.is_empty()).then(|| def.user.clone()));
        let password = env(&var("PASSWORD")).filter(|p| !p.is_empty()).map(Secret::new).or_else(|| {
            self.key_store
                .then(|| crate::key_store::key_store().get(&format!("SQL:{}", def.name.to_ascii_uppercase())))
                .flatten()
                .map(Secret::new)
        });
        if let Some(url) = env(&var("URL")).filter(|u| !u.trim().is_empty()) {
            let kind = BackendKind::of(&url);
            if kind == BackendKind::Sqlite {
                let path = url.trim().strip_prefix("sqlite:").unwrap_or(url.trim()).to_string();
                return Ok(Target::Sqlite { path: self.base_dir.join(path), create: def.create_if_missing });
            }
            return Ok(Target::ConnString(url));
        }
        match def.backend.trim().to_ascii_lowercase().as_str() {
            "sqlite" | "" => Ok(Target::Sqlite { path: self.base_dir.join(&def.path), create: def.create_if_missing }),
            b @ ("postgresql" | "postgres" | "mysql") => Ok(Target::Server {
                kind: if b == "mysql" { BackendKind::MySql } else { BackendKind::Postgres },
                host: def.host.clone(),
                port: def.port,
                database: def.database.clone(),
                user,
                password,
            }),
            other => Err(SqlError::new(
                code::CANNOT_CONNECT,
                format!("SQL connection {} names an unknown database kind '{other}'", def.name),
            )),
        }
    }
}

fn installed() -> &'static RwLock<Option<Arc<SqlCatalog>>> {
    static SLOT: std::sync::OnceLock<RwLock<Option<Arc<SqlCatalog>>>> = std::sync::OnceLock::new();
    SLOT.get_or_init(|| RwLock::new(None))
}

/// Install the process's catalog — before any interpreter starts (a built
/// binary's `main`, `rcrun run`, `rcrun run-form`).
pub fn install(catalog: SqlCatalog) {
    *installed().write().unwrap_or_else(|p| p.into_inner()) = Some(Arc::new(catalog));
}

/// The process's catalog, if one was installed.
pub fn current() -> Option<Arc<SqlCatalog>> {
    installed().read().unwrap_or_else(|p| p.into_inner()).clone()
}

/// The environment, as [`SqlCatalog::resolve`] reads it.
pub fn process_env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Try a connection with the values given (the IDE's Test connection button,
/// R33): `Ok` with what it reached, or the database's own message.
pub fn test_connection(target: &Target) -> Result<String, String> {
    let mut backend = super::backend::open_target(target).map_err(|e| e.message)?;
    backend.query("SELECT 1", &[]).map_err(|e| e.message)?;
    Ok(format!("connected to {}", backend.kind().name()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn catalog() -> SqlCatalog {
        let mut sales = SqlConnection::new("Sales");
        sales.path = "data/sales.db".into();
        sales.default = true;
        let mut wh = SqlConnection::new("WAREHOUSE");
        wh.backend = "postgresql".into();
        wh.host = "db.local".into();
        wh.port = Some(5432);
        wh.database = "stock".into();
        wh.user = "file-user".into();
        SqlCatalog::new("Shop App", vec![sales, wh], PathBuf::from("/srv/shop"), Source::Injected)
    }

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |k: &str| m.get(k).cloned()
    }

    /// Each precedence case: the file, then `_URL`, `_USER`, `_PASSWORD`.
    #[test]
    fn the_environment_overrides_the_file_in_its_order() {
        let c = catalog();
        // The file alone.
        assert_eq!(
            c.resolve("sales", &env(&[])).unwrap(),
            Target::Sqlite { path: PathBuf::from("/srv/shop/data/sales.db"), create: false }
        );
        // _URL replaces the target.
        assert_eq!(
            c.resolve("SALES", &env(&[("SHOP_APP_SQL_SALES_URL", "sqlite:other.db")])).unwrap(),
            Target::Sqlite { path: PathBuf::from("/srv/shop/other.db"), create: false }
        );
        // _USER over the file's user; _PASSWORD supplies the password.
        match c.resolve("warehouse", &env(&[("SHOP_APP_SQL_WAREHOUSE_USER", "env-user"), ("SHOP_APP_SQL_WAREHOUSE_PASSWORD", "pw")])).unwrap() {
            Target::Server { kind, host, port, database, user, password } => {
                assert_eq!((kind, host.as_str(), port, database.as_str()), (BackendKind::Postgres, "db.local", Some(5432), "stock"));
                assert_eq!(user.as_deref(), Some("env-user"));
                assert_eq!(password.as_ref().map(Secret::expose), Some("pw"));
            }
            other => panic!("{other:?}"),
        }
        // Without them: the file's user, no password.
        match c.resolve("warehouse", &env(&[])).unwrap() {
            Target::Server { user, password, .. } => {
                assert_eq!(user.as_deref(), Some("file-user"));
                assert!(password.is_none());
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(c.resolve("NOPE", &env(&[])).unwrap_err().sqlstate, "08001");
        assert_eq!(c.default_connection().map(|d| d.name.as_str()), Some("Sales"));
    }

    /// A password in the file is refused with a message naming the file and
    /// both permitted sources.
    #[test]
    fn a_password_in_the_file_fails_with_28000() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sql-connections.toml");
        std::fs::write(&file, "[[connection]]\nname = \"SALES\"\nbackend = \"postgresql\"\nhost = \"h\"\npassword = \"oops\"\n").unwrap();
        let c = SqlCatalog::from_deployment_file(&file, "shop").unwrap();
        let e = c.resolve("SALES", &env(&[])).unwrap_err();
        println!("{}", e.message);
        assert_eq!(e.sqlstate, "28000");
        assert!(e.message.contains(&file.display().to_string()));
        assert!(e.message.contains("SHOP_SQL_SALES_PASSWORD"));
        assert!(e.message.contains("SQL:SALES"));
        assert!(!e.message.contains("oops"), "the password itself is never repeated");
    }

    #[test]
    fn a_secret_never_prints() {
        let s = Secret::new("hunter2");
        assert_eq!(format!("{s}"), "***");
        assert_eq!(format!("{s:?}"), "***");
        let t = Target::Server {
            kind: BackendKind::MySql,
            host: "h".into(),
            port: None,
            database: "d".into(),
            user: Some("u".into()),
            password: Some(s),
        };
        assert!(!format!("{t:?}").contains("hunter2"));
    }
}
