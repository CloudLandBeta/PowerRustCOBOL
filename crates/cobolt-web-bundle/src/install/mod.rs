//! The install package's script and the site it writes (spec 113, R14 to R23).
//!
//! A Web project's package is two files: `<name>.zip` (the page, the Backend, its settings and its
//! seed data) and `<name>-install.sh` or `<name>-install.ps1`, which installs it. The script is plain
//! text made here, like the deployment kit's scripts in [`crate::deploy`]: nothing in it is a secret,
//! and its messages are in the language the IDE was in (see [`messages`]).

mod messages;
mod unix;
mod windows;

pub use messages::{msg, Lang, TABLE};

/// The system the script installs on: the one of the machine that packaged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOs {
    Linux,
    MacOs,
    Windows,
}

impl TargetOs {
    /// The system this code was compiled for.
    pub fn host() -> TargetOs {
        if cfg!(windows) {
            TargetOs::Windows
        } else if cfg!(target_os = "macos") {
            TargetOs::MacOs
        } else {
            TargetOs::Linux
        }
    }

    /// `sh` or `ps1`.
    pub fn script_ext(self) -> &'static str {
        match self {
            TargetOs::Windows => "ps1",
            _ => "sh",
        }
    }
}

/// What the script needs to know about the Backend.
#[derive(Debug, Clone)]
pub struct BackendFacts {
    /// The executable's file name (`app-backend`, `app-backend.exe`).
    pub exe: String,
    /// Where it listens, as nginx writes it: `127.0.0.1:8443`, `[::1]:8443`.
    pub addr: String,
    pub port: u16,
    /// The names the Backend reads as `env:NAME`; the script requires each in the environment file.
    pub required_secrets: Vec<String>,
    /// The package carries `assets/KB`, which the service may write (its index).
    pub has_kb: bool,
}

/// The facts a script is written from.
#[derive(Debug, Clone)]
pub struct Install<'a> {
    /// The project's name, as the developer sees it.
    pub project: &'a str,
    /// The file stem of the two files: `<stem>.zip` and `<stem>-install.<ext>`.
    pub stem: String,
    /// Lowercase letters, digits and dashes ([`crate::deploy::slug`]): the service and the site.
    pub slug: String,
    pub os: TargetOs,
    pub lang: Lang,
    /// The SHA-256 of the zip, lowercase hex: the script refuses a zip that does not match.
    pub zip_sha256: String,
    /// The security headers every response carries ([`crate::csp`]).
    pub headers: &'a [(String, String)],
    /// `None`: a project with no Backend; the script installs the site only.
    pub backend: Option<BackendFacts>,
    /// The name nginx answers to unless the operator gives another.
    pub server_name: String,
    /// The address the page opens is `wss://`: TLS ends in nginx, which needs a certificate and a key.
    pub wss: bool,
}

impl Install<'_> {
    pub fn zip_name(&self) -> String {
        format!("{}.zip", self.stem)
    }

    pub fn script_name(&self) -> String {
        format!("{}-install.{}", self.stem, self.os.script_ext())
    }

    /// The service's name: the slug; launchd wants a reverse-domain label.
    pub fn service(&self) -> String {
        match self.os {
            TargetOs::MacOs => format!("app.{}", self.slug),
            _ => self.slug.clone(),
        }
    }

    /// The account the service runs as; macOS service accounts begin with an underscore.
    pub fn account(&self) -> String {
        match self.os {
            TargetOs::MacOs => format!("_{}", self.slug),
            _ => self.slug.clone(),
        }
    }

    /// Where it installs unless told another place.
    pub fn default_install_dir(&self) -> String {
        match self.os {
            TargetOs::Linux => format!("/opt/{}", self.slug),
            TargetOs::MacOs => format!("/usr/local/{}", self.slug),
            TargetOs::Windows => format!("%ProgramFiles%\\{}", self.folder_name()),
        }
    }

    /// The project's name as a Windows folder name: the characters a path cannot hold become `-`.
    pub fn folder_name(&self) -> String {
        let s: String = self.project.chars().map(|c| if c.is_control() || "\\/:*?\"<>|".contains(c) { '-' } else { c }).collect();
        let s = s.trim_matches(|c: char| c == ' ' || c == '.' || c == '-').to_owned();
        if s.is_empty() {
            self.slug.clone()
        } else {
            s
        }
    }

    /// The script: `sh` for Linux and macOS, PowerShell for Windows.
    pub fn script(&self) -> String {
        match self.os {
            TargetOs::Windows => windows::script(self),
            _ => unix::script(self),
        }
    }

    /// The nginx site, with `@SERVER_NAME@`, `@WEB_ROOT@`, `@CERT@` and `@KEY@` where the script puts
    /// what the operator chose. The static files carry the headers of [`crate::csp::nginx_static`]; with a Backend, `/ws`
    /// and `/auth/` go to it; with `wss://`, TLS ends here and port 80 redirects.
    pub fn site_template(&self) -> String {
        let mut s = format!(
            "# {slug}: the nginx site of this application, written by its install script (PowerRustCOBOL AI, Package).\n# The script rewrites this file each time it runs: keep settings of your own in another file.\n",
            slug = self.slug
        );
        if self.wss {
            s.push_str(
                "server {\n    listen 80;\n    server_name @SERVER_NAME@;\n    return 301 https://$host$request_uri;\n}\n\nserver {\n    listen 443 ssl;\n    server_name @SERVER_NAME@;\n    ssl_certificate     \"@CERT@\";\n    ssl_certificate_key \"@KEY@\";\n    ssl_protocols TLSv1.2 TLSv1.3;\n",
            );
        } else {
            s.push_str("server {\n    listen 80;\n    server_name @SERVER_NAME@;\n");
        }
        s.push_str("\n    root \"@WEB_ROOT@\";\n    index index.html;\n");
        s.push_str(&crate::csp::nginx_static(self.headers, "    "));
        if let Some(b) = &self.backend {
            let a = &b.addr;
            s.push_str(&format!(
                "\n    # The Backend: the page opens its address ([web] backend_url) and nginx hands it to the service on loopback.\n    location /ws {{\n        proxy_pass http://{a};\n        proxy_http_version 1.1;\n        proxy_set_header Upgrade $http_upgrade;\n        proxy_set_header Connection \"upgrade\";\n        proxy_set_header Host $host;\n        proxy_read_timeout 3600s;\n        proxy_send_timeout 3600s;\n    }}\n    location /auth/ {{\n        proxy_pass http://{a};\n        proxy_set_header Host $host;\n        proxy_set_header X-Forwarded-Proto $scheme;\n    }}\n"
            ));
        }
        s.push_str("}\n");
        s
    }
}

/// The messages a script carries (the rows of [`TABLE`] it prints).
pub(crate) const KEYS: &[&str] = &[
    "e_value", "e_unknown", "e_no_nginx", "e_bad_nginx", "e_root", "e_admin", "e_tool", "e_zip", "e_hash", "e_secrets", "e_cert", "e_path", "e_name", "e_file", "e_port", "e_no_http", "e_start", "e_nginx", "e_failed",
    "s_restored", "s_check", "s_extract", "s_files", "s_account", "s_service", "s_wait", "s_site", "s_test", "s_reload", "w_nginx_stopped", "w_selinux", "w_toml", "done", "done_update", "d_address", "d_service",
    "d_status", "d_log", "d_installed",
];

/// `'text'` for a POSIX shell: a quote inside becomes `'\''`.
pub(crate) fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// `'text'` for PowerShell: a quote inside is doubled.
pub(crate) fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// A message with each `{}` turned into what `slot` gives: `%s` for `printf`, `{0}`, `{1}` for `-f`.
pub(crate) fn with_slots(text: &str, slot: impl Fn(usize) -> String) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let mut n = 0;
    let mut rest = text;
    while let Some(i) = rest.find("{}") {
        out.push_str(&rest[..i]);
        out.push_str(&slot(n));
        n += 1;
        rest = &rest[i + 2..];
    }
    out.push_str(rest);
    out
}

/// The usage text of a script: the lines the operator reads on a missing or wrong parameter.
pub(crate) fn usage_lines(i: &Install, flag: impl Fn(&str) -> String, nginx_label: &str) -> Vec<(String, String)> {
    let m = |k: &str| msg(k, i.lang).to_owned();
    let secrets = i.backend.as_ref().map(|b| b.required_secrets.join(", ")).unwrap_or_default();
    let mut v = vec![(nginx_label.to_owned(), m("p_nginx"))];
    v.push((flag("install-dir"), with_slots(&m("p_install"), |_| i.default_install_dir())));
    v.push((flag("server-name"), with_slots(&m("p_server"), |_| i.server_name.clone())));
    if i.wss {
        v.push((flag("cert"), m("p_cert")));
        v.push((flag("key"), m("p_key")));
    }
    if i.backend.is_some() {
        if secrets.is_empty() {
            v.push((flag("env-file"), m("p_env")));
        } else {
            v.push((flag("env-file"), with_slots(&m("p_env_req"), |_| secrets.clone())));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers() -> Vec<(String, String)> {
        vec![("Content-Security-Policy".into(), "default-src 'self'".into()), ("X-Content-Type-Options".into(), "nosniff".into())]
    }

    pub(crate) fn sample(os: TargetOs, lang: Lang, wss: bool, backend: bool) -> Install<'static> {
        let h: &'static [(String, String)] = Box::leak(headers().into_boxed_slice());
        Install {
            project: "Power Oficina",
            stem: "PowerOficina".into(),
            slug: crate::deploy::slug("Power Oficina"),
            os,
            lang,
            zip_sha256: "0".repeat(64),
            headers: h,
            backend: backend.then(|| BackendFacts { exe: if os == TargetOs::Windows { "power-oficina-backend.exe".into() } else { "power-oficina-backend".into() }, addr: "127.0.0.1:8443".into(), port: 8443, required_secrets: vec!["CRM_TOKEN".into(), "COOKIE_KEY".into()], has_kb: true }),
            server_name: "oficina.example.com".into(),
            wss,
        }
    }

    #[test]
    fn the_site_has_tls_only_with_wss_and_the_backend_only_with_one() {
        let tls = sample(TargetOs::Linux, Lang::En, true, true).site_template();
        assert!(tls.contains("listen 443 ssl;") && tls.contains("ssl_certificate     \"@CERT@\";") && tls.contains("return 301 https://$host$request_uri;"));
        assert!(tls.contains("location /ws {") && tls.contains("proxy_pass http://127.0.0.1:8443;") && tls.contains("location /auth/ {"));
        assert!(tls.contains("types {") && tls.contains("application/wasm"), "the static rules of the fix");
        let plain = sample(TargetOs::Linux, Lang::En, false, true).site_template();
        assert!(!plain.contains("ssl_certificate") && !plain.contains("listen 443") && plain.contains("listen 80;"));
        let site_only = sample(TargetOs::Linux, Lang::En, false, false).site_template();
        assert!(!site_only.contains("proxy_pass") && !site_only.contains("/ws"));
        let mut locations = 0;
        for t in [&tls, &plain, &site_only] {
            assert!(t.contains("root \"@WEB_ROOT@\";") && t.contains("server_name @SERVER_NAME@;"));
            assert!(!t.to_ascii_lowercase().contains("cobolt"));
            // The three rules of the nginx fix hold in the site this script writes, not only in the hosting kit.
            locations += crate::csp::nginx_rules_hold(t, &headers()).expect("the nginx rules hold in the production site");
        }
        assert!(locations >= 6, "two locations with their own add_header in each of the three sites, found {locations}");
    }

    #[test]
    fn quoting_and_slots_are_right() {
        assert_eq!(sh_quote("it's"), "'it'\\''s'");
        assert_eq!(ps_quote("it's"), "'it''s'");
        assert_eq!(with_slots("a {} b {}", |n| format!("%{n}")), "a %0 b %1");
        assert_eq!(with_slots("no slots", |_| "x".into()), "no slots");
    }

    #[test]
    fn names_follow_the_system() {
        let l = sample(TargetOs::Linux, Lang::En, true, true);
        let m = sample(TargetOs::MacOs, Lang::En, true, true);
        let w = sample(TargetOs::Windows, Lang::En, true, true);
        assert_eq!((l.service(), l.account(), l.default_install_dir()), ("power-oficina".into(), "power-oficina".into(), "/opt/power-oficina".into()));
        assert_eq!((m.service(), m.account(), m.default_install_dir()), ("app.power-oficina".into(), "_power-oficina".into(), "/usr/local/power-oficina".into()));
        assert_eq!(w.default_install_dir(), "%ProgramFiles%\\Power Oficina");
        assert_eq!((l.zip_name(), l.script_name(), w.script_name()), ("PowerOficina.zip".into(), "PowerOficina-install.sh".into(), "PowerOficina-install.ps1".into()));
    }
}
