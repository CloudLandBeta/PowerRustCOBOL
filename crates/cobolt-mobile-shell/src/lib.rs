// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The mobile shell of a PowerRustCOBOL Mobile application (WASM-012).
//!
//! A Mobile application is its Web bundle — the player, the program module and
//! the forms — running in the phone's own web view, inside a Tauri 2 shell.
//! The staged application crate that `cobolt_compiler::mobile_build` writes is
//! three lines: its entry point calls [`run`] with the context Tauri generates
//! from the staged configuration. Everything the shell does lives here:
//!
//! * **The window and its navigation guard** (R12): one window on the bundled
//!   page; a navigation that stays on the application's own origin goes ahead,
//!   any other address is stopped and handed to the system (the browser, the
//!   mail, phone or messages application) when the project declared
//!   `open-address`, and is simply stopped when it did not.
//! * **The lifecycle** (R11): pause and resume of the platform are told to the
//!   page as a `pr-shell` event (`{"type":"paused"}` / `{"type":"resumed"}`);
//!   the player also listens to `visibilitychange`, the portable fallback.
//! * **Deep links** (`deep-link`, ABI A26): `<app id>://<form>?<name>=<value>`
//!   opens the form with those parameters, as the address
//!   `#/<form>?<name>=<value>` (a single-page application) or
//!   `?pr-form=<form>&pr-param-<name>=<value>` (a standalone one), so
//!   `me::GetParam` reads them in the form's first handler.
//! * **Secure storage** (`secure-storage`, R9): the commands `secure_store`,
//!   `secure_load` and `secure_delete`, which the player calls for
//!   `COBOL::SecureStore`, `SecureLoad` and `SecureDelete`. The values live in
//!   the Android Keystore (an encrypted SharedPreferences file whose key never
//!   leaves the Keystore) or the iOS Keychain, through `keyring-core`.
//!
//! The Back button of Android needs nothing here: Tauri's web view goes back in
//! the page's history when it can — which is the player's navigation pop
//! (WASM-011 R13) — and otherwise lets the system send the application to the
//! background (plan §9 V0 record, item 5).

use tauri::{Manager, Url, WebviewUrl, WebviewWindowBuilder};

/// What the staged application tells the shell about itself.
#[derive(Debug, Clone, Copy)]
pub struct ShellConfig {
    /// The application id (`com.example.orders`): the deep-link scheme and the
    /// secure store's service name.
    pub app_id: &'static str,
    /// The window's title (the application's name).
    pub title: &'static str,
    /// The project's application style is single-page (`spa`): a deep link
    /// becomes a route (`#/form?…`) instead of an address (`?pr-form=…`).
    pub single_page: bool,
}

/// The label of the one window.
pub const MAIN_WINDOW: &str = "main";

/// Start the application. Called by the staged application's entry point
/// (`#[cfg_attr(mobile, tauri::mobile_entry_point)]`) with
/// `tauri::generate_context!()`. Returns when the application ends.
pub fn run(context: tauri::Context<tauri::Wry>, config: ShellConfig) {
    let builder = tauri::Builder::default();
    #[cfg(feature = "open-address")]
    let builder = builder.plugin(tauri_plugin_opener::init());
    #[cfg(feature = "deep-link")]
    let builder = builder.plugin(tauri_plugin_deep_link::init());
    #[cfg(feature = "secure-storage")]
    let builder = builder.invoke_handler(tauri::generate_handler![secure::secure_store, secure::secure_load, secure::secure_delete]);
    let outcome = builder
        .manage(config)
        .setup(move |app| {
            let start = start_address(app, &config);
            open_main_window(app, &config, start)?;
            #[cfg(feature = "deep-link")]
            listen_for_deep_links(app, config);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let Some(kind) = lifecycle_kind(event) {
                tell_page(window.app_handle(), &format!("{{\"type\":\"{kind}\"}}"));
            }
        })
        .run(context);
    if let Err(e) = outcome {
        eprintln!("PowerRustCOBOL mobile shell: the application could not start: {e}");
    }
}

/// `paused` / `resumed` for the platform's lifecycle events, `None` for the rest.
fn lifecycle_kind(event: &tauri::WindowEvent) -> Option<&'static str> {
    // `cfg(mobile)` is set by tauri-build in the application crate only; a
    // library names the targets.
    match event {
        #[cfg(any(target_os = "android", target_os = "ios"))]
        tauri::WindowEvent::Suspended => Some("paused"),
        #[cfg(any(target_os = "android", target_os = "ios"))]
        tauri::WindowEvent::Resumed => Some("resumed"),
        _ => None,
    }
}

/// Raise `pr-shell` on the page with `detail` (a JSON object).
fn tell_page(app: &tauri::AppHandle, detail: &str) {
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        let js = format!("window.dispatchEvent(new CustomEvent('pr-shell',{{detail:{detail}}}));");
        if let Err(e) = w.eval(js) {
            eprintln!("PowerRustCOBOL mobile shell: the page could not be told {detail}: {e}");
        }
    }
}

/// The page the window opens on: `index.html`, or the form a deep link that
/// started the application names.
fn start_address(_app: &tauri::App, _config: &ShellConfig) -> String {
    #[cfg(feature = "deep-link")]
    {
        use tauri_plugin_deep_link::DeepLinkExt;
        if let Ok(Some(urls)) = _app.deep_link().get_current() {
            if let Some(page) = urls.iter().find_map(|u| deep_link_page(u, _config)) {
                return page;
            }
        }
    }
    "index.html".to_owned()
}

fn open_main_window(app: &tauri::App, config: &ShellConfig, start: String) -> tauri::Result<()> {
    let handle = app.handle().clone();
    WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::App(start.into()))
        .title(config.title)
        .on_navigation(move |url| {
            if is_own_address(url) {
                return true;
            }
            hand_to_system(&handle, url);
            false
        })
        .build()?;
    Ok(())
}

/// Is `url` the application's own page? The bundled page is served from
/// `tauri://localhost` (iOS) or `http(s)://tauri.localhost` (Android); in a
/// development run it is the development server on this computer's loopback
/// address (reached from an emulator through `adb reverse`).
pub fn is_own_address(url: &Url) -> bool {
    match url.scheme() {
        "tauri" => url.host_str() == Some("localhost"),
        "http" | "https" => matches!(url.host_str(), Some("tauri.localhost") | Some("localhost") | Some("127.0.0.1") | Some("[::1]")),
        "about" => url.as_str() == "about:blank",
        _ => false,
    }
}

/// Is `url` one the system opens (the browser, the mail, phone or messages
/// application) rather than the web view?
pub fn is_system_address(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https" | "mailto" | "tel" | "sms")
}

/// Open `url` with the system, when the project declared `open-address`.
fn hand_to_system(_app: &tauri::AppHandle, url: &Url) {
    if !is_system_address(url) {
        eprintln!("PowerRustCOBOL mobile shell: stopped a navigation to {url}: not an address the application may open");
        return;
    }
    #[cfg(feature = "open-address")]
    {
        use tauri_plugin_opener::OpenerExt;
        if let Err(e) = _app.opener().open_url(url.as_str(), None::<&str>) {
            eprintln!("PowerRustCOBOL mobile shell: the system could not open {url}: {e}");
        }
    }
    #[cfg(not(feature = "open-address"))]
    eprintln!("PowerRustCOBOL mobile shell: stopped a navigation to {url}: the project does not declare the open-address feature");
}

/// The page a deep link opens (`None` when the link is not the application's
/// own scheme): `<app id>://<form>?<name>=<value>&…` → `index.html#/<form>?…`
/// for a single-page application, `index.html?pr-form=<form>&pr-param-…` for a
/// standalone one. The form name is letters, digits and `-` `_` `.` only; any
/// other link opens the main form.
pub fn deep_link_page(url: &Url, config: &ShellConfig) -> Option<String> {
    if !url.scheme().eq_ignore_ascii_case(&deep_link_scheme(config.app_id)) {
        return None;
    }
    // `scheme://form?x=1` parses `form` as the host; `scheme:form?x=1` as the path.
    let form = url.host_str().map(str::to_owned).unwrap_or_else(|| url.path().trim_matches('/').to_owned());
    let form = form.trim_matches('/');
    let valid = |s: &str| !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if !valid(form) {
        return Some("index.html".to_owned());
    }
    let params: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| valid(k))
        .take(16)
        .map(|(k, v)| (k.into_owned(), v.chars().take(512).collect()))
        .collect();
    let enc = |s: &str| -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
                other => format!("%{other:02X}"),
            })
            .collect()
    };
    Some(if config.single_page {
        let query: Vec<String> = params.iter().map(|(k, v)| format!("{}={}", enc(k), enc(v))).collect();
        if query.is_empty() {
            format!("index.html#/{form}")
        } else {
            format!("index.html#/{form}?{}", query.join("&"))
        }
    } else {
        let mut q = vec![format!("pr-form={}", enc(form))];
        q.extend(params.iter().map(|(k, v)| format!("pr-param-{}={}", enc(k), enc(v))));
        format!("index.html?{}", q.join("&"))
    })
}

/// The deep-link scheme of an application id: the id in lower case, `_`
/// written `-` (a URL scheme has no `_`).
pub fn deep_link_scheme(app_id: &str) -> String {
    app_id.trim().to_ascii_lowercase().replace('_', "-")
}

#[cfg(feature = "deep-link")]
fn listen_for_deep_links(app: &tauri::App, config: ShellConfig) {
    use tauri_plugin_deep_link::DeepLinkExt;
    let handle = app.handle().clone();
    app.deep_link().on_open_url(move |event| {
        let Some(page) = event.urls().iter().find_map(|u| deep_link_page(u, &config)) else { return };
        let Some(w) = handle.get_webview_window(MAIN_WINDOW) else { return };
        let target = w.url().ok().and_then(|current| current.join(&format!("/{page}")).ok());
        match target {
            Some(url) => {
                if let Err(e) = w.navigate(url) {
                    eprintln!("PowerRustCOBOL mobile shell: the deep link could not open its form: {e}");
                }
            }
            None => eprintln!("PowerRustCOBOL mobile shell: the deep link could not be resolved against the page's address"),
        }
    });
}

/// The secure-storage commands (R9, ABI A26).
#[cfg(feature = "secure-storage")]
pub mod secure {
    use super::ShellConfig;
    use std::sync::OnceLock;

    /// The longest key, and what a key may hold.
    pub const MAX_KEY: usize = 128;
    /// The largest value, in bytes.
    pub const MAX_VALUE: usize = 4096;

    /// Is `key` a key the store accepts? `Err` says why.
    pub fn check_key(key: &str) -> Result<(), String> {
        if key.is_empty() || key.len() > MAX_KEY {
            return Err(format!("a secure-storage key is 1 to {MAX_KEY} characters"));
        }
        if !key.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')) {
            return Err("a secure-storage key holds only letters, digits and - _ .".to_owned());
        }
        Ok(())
    }

    /// The platform's store, made once (the Android one needs the application
    /// context, which the shell has set by the time a page can call).
    fn store() -> Result<(), String> {
        static READY: OnceLock<Result<(), String>> = OnceLock::new();
        READY
            .get_or_init(|| {
                #[cfg(target_os = "android")]
                {
                    let s = android_native_keyring_store::Store::new().map_err(|e| format!("the Android Keystore is unavailable: {e}"))?;
                    keyring_core::set_default_store(s);
                    Ok(())
                }
                #[cfg(target_os = "ios")]
                {
                    let s = apple_native_keyring_store::protected::Store::new().map_err(|e| format!("the iOS Keychain is unavailable: {e}"))?;
                    keyring_core::set_default_store(s);
                    Ok(())
                }
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                {
                    Err("this platform has no secure store for a Mobile application".to_owned())
                }
            })
            .clone()
    }

    fn entry(config: &ShellConfig, key: &str) -> Result<keyring_core::Entry, String> {
        check_key(key)?;
        store()?;
        keyring_core::Entry::new(config.app_id, key).map_err(|e| e.to_string())
    }

    /// `COBOL::SecureStore(key, value)`.
    #[tauri::command]
    pub async fn secure_store(config: tauri::State<'_, ShellConfig>, key: String, value: String) -> Result<(), String> {
        if value.len() > MAX_VALUE {
            return Err(format!("a secure-storage value is at most {MAX_VALUE} bytes"));
        }
        entry(&config, &key)?.set_password(&value).map_err(|e| e.to_string())
    }

    /// `COBOL::SecureLoad(key)`: the value, or `None` when the key holds none.
    #[tauri::command]
    pub async fn secure_load(config: tauri::State<'_, ShellConfig>, key: String) -> Result<Option<String>, String> {
        match entry(&config, &key)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    /// `COBOL::SecureDelete(key)`: `true` when there was a value to delete.
    #[tauri::command]
    pub async fn secure_delete(config: tauri::State<'_, ShellConfig>, key: String) -> Result<bool, String> {
        match entry(&config, &key)?.delete_credential() {
            Ok(()) => Ok(true),
            Err(keyring_core::Error::NoEntry) => Ok(false),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPA: ShellConfig = ShellConfig { app_id: "com.example.my_orders", title: "Orders", single_page: true };
    const STANDALONE: ShellConfig = ShellConfig { app_id: "com.example.my_orders", title: "Orders", single_page: false };

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn only_the_applications_own_page_stays_in_the_web_view() {
        for own in ["tauri://localhost/index.html", "http://tauri.localhost/index.html#/orders", "https://tauri.localhost/", "http://127.0.0.1:8750/", "about:blank"] {
            assert!(is_own_address(&url(own)), "{own}");
        }
        for other in ["https://example.com/", "http://tauri.localhost.evil.com/", "tauri://evil/", "mailto:a@b.c", "tel:+5511999999999", "file:///etc/passwd", "javascript:alert(1)", "data:text/html,x"] {
            assert!(!is_own_address(&url(other)), "{other}");
        }
        for system in ["https://example.com/", "mailto:a@b.c", "tel:+5511999999999", "sms:+5511999999999"] {
            assert!(is_system_address(&url(system)), "{system}");
        }
        for never in ["file:///etc/passwd", "javascript:alert(1)", "intent://x#Intent;end", "data:text/html,x"] {
            assert!(!is_system_address(&url(never)), "{never}");
        }
        println!("navigation guard: 5 own addresses stay, 8 others leave the web view; 4 go to the system, 4 are never opened");
    }

    #[test]
    fn a_deep_link_becomes_the_forms_route_or_address() {
        assert_eq!(deep_link_scheme("com.example.my_orders"), "com.example.my-orders");
        let link = url("com.example.my-orders://order-form?id=7&note=a%20b");
        assert_eq!(deep_link_page(&link, &SPA).as_deref(), Some("index.html#/order-form?id=7&note=a%20b"));
        assert_eq!(deep_link_page(&link, &STANDALONE).as_deref(), Some("index.html?pr-form=order-form&pr-param-id=7&pr-param-note=a%20b"));
        assert_eq!(deep_link_page(&url("com.example.my-orders://main-form"), &SPA).as_deref(), Some("index.html#/main-form"));
        assert_eq!(deep_link_page(&url("com.example.my-orders:%3Cscript%3E"), &SPA).as_deref(), Some("index.html"), "a bad form name opens the main form");
        assert_eq!(deep_link_page(&url("https://example.com/order-form"), &SPA), None, "not the application's scheme");
        assert_eq!(deep_link_page(&url("other.app://order-form"), &SPA), None);
        let bad_param = url("com.example.my-orders://f?ok=1&b%20ad=2");
        assert_eq!(deep_link_page(&bad_param, &SPA).as_deref(), Some("index.html#/f?ok=1"), "a parameter name outside the alphabet is dropped");
        println!("deep links: the scheme of an id with _, a route and an address with 2 parameters, a bare form, 4 refusals");
    }

    #[cfg(feature = "secure-storage")]
    #[test]
    fn secure_storage_keys_are_checked() {
        for good in ["session", "auth.token", "user_1-key"] {
            assert!(secure::check_key(good).is_ok(), "{good}");
        }
        for bad in ["", "a b", "x/y", "ç"] {
            assert!(secure::check_key(bad).is_err(), "{bad}");
        }
        assert!(secure::check_key(&"k".repeat(129)).is_err());
        println!("secure-storage keys: 3 accepted, 5 refused");
    }
}
