// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The notice of a page opened from disk** (the operator, 2026-10-25: a Web build's `index.html`,
//! opened by a double click, stayed blank).
//!
//! A page opened as `file://` has an opaque origin, and a browser then refuses every request the
//! application makes in CORS mode: its module scripts, its `config.json`, its fonts and its WebAssembly,
//! and any stylesheet or script that carries an `integrity` attribute. Nothing starts, and nothing says
//! why. A Web build is meant to be served — by the IDE's Run and Web Preview, or by any web server —
//! so the page now carries a short notice, hidden, in the six languages of the IDE, and one inline
//! script that shows it only when `location.protocol` is `file:`.
//!
//! The notice needs neither the stylesheet nor the application, because on a `file://` page neither
//! loads: it is plain HTML, and the script is one classic inline statement. The Content-Security-Policy
//! allows that script by its hash ([`crate::hash::csp_sha256`]), never by `'unsafe-inline'`, so the
//! policy stays default-deny. On a served page the notice stays hidden and costs nothing.
//!
//! The words for the IDE's buttons are the IDE's own (its `Tr` tables: `tb_run`, `pk_prev_title`), so a
//! developer finds the button the notice names.

/// The id of the notice's element, and of nothing else on the page.
pub const ID: &str = "pr-file-notice";

/// The one inline script of the page, character for character: the CSP allows exactly this text by its
/// hash, so it is never built from anything else.
pub const SCRIPT: &str = "if(location.protocol==='file:'){document.getElementById('pr-file-notice').hidden=false}";

/// A web server for the folder, as a developer can type it on a Mac or a Linux machine.
pub const COMMAND: &str = "python3 -m http.server 8080 --bind 127.0.0.1";

/// Where that server answers.
pub const URL: &str = "http://127.0.0.1:8080/";

/// What the notice says in one language. `before` ends where the command begins; `after` introduces
/// the address.
pub struct Text {
    /// A BCP 47 tag; its first subtag is the language.
    pub lang: &'static str,
    pub title: &'static str,
    pub before: &'static str,
    pub after: &'static str,
}

/// The IDE's six languages, in the order of its language menu.
pub const TEXTS: [Text; 6] = [
    Text {
        lang: "en",
        title: "This application cannot run from a file on your disk",
        before: "A browser blocks the scripts and files of a web application when its page is opened as <code>file://</code>. Serve the folder over HTTP instead: in PowerRustCOBOL AI press <b>Run</b> (or the <b>Web Preview</b> button), or start any web server in this folder, for example:",
        after: "Then open",
    },
    Text {
        lang: "pt-BR",
        title: "Este aplicativo não pode ser executado a partir de um arquivo no disco",
        before: "O navegador bloqueia os scripts e os arquivos de um aplicativo web quando a página é aberta como <code>file://</code>. Sirva a pasta por HTTP: no PowerRustCOBOL AI, use <b>Executar</b> (ou o botão <b>Visualização web</b>), ou inicie qualquer servidor web nesta pasta, por exemplo:",
        after: "Depois abra",
    },
    Text {
        lang: "es",
        title: "Esta aplicación no puede ejecutarse desde un archivo del disco",
        before: "El navegador bloquea los scripts y los archivos de una aplicación web cuando su página se abre como <code>file://</code>. Sirve la carpeta por HTTP: en PowerRustCOBOL AI usa <b>Ejecutar</b> (o el botón <b>Vista previa web</b>), o inicia cualquier servidor web en esta carpeta, por ejemplo:",
        after: "Después abre",
    },
    Text {
        lang: "fr",
        title: "Cette application ne peut pas s'exécuter depuis un fichier du disque",
        before: "Le navigateur bloque les scripts et les fichiers d'une application web lorsque sa page est ouverte en <code>file://</code>. Servez le dossier en HTTP : dans PowerRustCOBOL AI, utilisez <b>Exécuter</b> (ou le bouton <b>Aperçu Web</b>), ou lancez n'importe quel serveur web dans ce dossier, par exemple :",
        after: "Puis ouvrez",
    },
    Text {
        lang: "ja",
        title: "このアプリケーションはディスク上のファイルからは実行できません",
        before: "ページを <code>file://</code> として開くと、ブラウザーは Web アプリケーションのスクリプトとファイルをブロックします。フォルダーを HTTP で提供してください。PowerRustCOBOL AI では <b>実行</b>（または <b>Web プレビュー</b> ボタン）を使うか、このフォルダーで任意の Web サーバーを起動します。例:",
        after: "続いて次のアドレスを開きます:",
    },
    Text {
        lang: "zh-Hans",
        title: "此应用程序无法直接从磁盘上的文件运行",
        before: "当页面以 <code>file://</code> 方式打开时，浏览器会阻止 Web 应用程序的脚本和文件。请改用 HTTP 提供该文件夹：在 PowerRustCOBOL AI 中使用<b>运行</b>（或 <b>Web 预览</b> 按钮），或在此文件夹中启动任意 Web 服务器，例如：",
        after: "然后打开",
    },
];

/// The first subtag of a language tag, lower-cased: `pt-BR` and `pt_br` are `pt`.
fn primary(tag: &str) -> String {
    tag.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase()
}

/// The notice, hidden: one section per language, the page's own language first (`lang` is the project's
/// locale; a language the IDE does not have starts with English).
pub fn html(lang: &str) -> String {
    let wanted = primary(lang);
    let first = TEXTS.iter().position(|t| primary(t.lang) == wanted).unwrap_or(0);
    let mut out = format!("<div id=\"{ID}\" hidden>\n");
    for i in std::iter::once(first).chain((0..TEXTS.len()).filter(|i| *i != first)) {
        let t = &TEXTS[i];
        out.push_str(&format!(
            "<section lang=\"{lang}\">\n<h2>{title}</h2>\n<p>{before}</p>\n<pre>{COMMAND}</pre>\n<p>{after} <code>{URL}</code></p>\n</section>\n",
            lang = t.lang,
            title = t.title,
            before = t.before,
            after = t.after,
        ));
    }
    out.push_str("</div>\n");
    out
}

/// The Content-Security-Policy source that allows [`SCRIPT`] and nothing else inline.
pub fn script_source() -> String {
    crate::hash::csp_sha256(SCRIPT.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_is_a_text_in_each_of_the_six_languages_of_the_ide() {
        let langs: Vec<String> = TEXTS.iter().map(|t| primary(t.lang)).collect();
        assert_eq!(langs, ["en", "pt", "es", "fr", "ja", "zh"]);
        let mut titles: Vec<&str> = TEXTS.iter().map(|t| t.title).collect();
        titles.sort_unstable();
        titles.dedup();
        assert_eq!(titles.len(), 6, "six different titles: nothing is a copy of the English");
        for t in &TEXTS {
            assert!(!t.title.is_empty() && !t.before.is_empty() && !t.after.is_empty(), "{}", t.lang);
            assert!(t.before.contains("file://"), "{}: the text names what the browser refuses", t.lang);
            assert!(t.before.contains("PowerRustCOBOL AI"), "{}: it says where the button is", t.lang);
            assert!(t.before.matches("<b>").count() == 2, "{}: it names Run and Web Preview", t.lang);
        }
    }

    #[test]
    fn the_notice_is_hidden_until_the_script_shows_it() {
        let h = html("en");
        assert!(h.starts_with(&format!("<div id=\"{ID}\" hidden>")), "{h}");
        assert!(h.ends_with("</div>\n"));
        assert_eq!(h.matches("<section lang=").count(), 6);
        assert!(SCRIPT.contains(ID) && SCRIPT.contains("'file:'") && SCRIPT.contains("hidden=false"));
        assert!(!SCRIPT.contains('<'), "nothing in the script can end the element it sits in");
    }

    #[test]
    fn the_pages_own_language_comes_first_and_an_unknown_one_starts_with_english() {
        let first = |lang: &str| {
            let h = html(lang);
            let at = h.find("<section lang=\"").unwrap() + "<section lang=\"".len();
            h[at..].split('"').next().unwrap().to_owned()
        };
        assert_eq!(first("en"), "en");
        assert_eq!(first("ja"), "ja");
        assert_eq!(first("pt-BR"), "pt-BR");
        assert_eq!(first("PT_br"), "pt-BR");
        assert_eq!(first("zh"), "zh-Hans");
        assert_eq!(first("de"), "en");
        assert_eq!(first(""), "en");
        // The same six, whatever comes first.
        for lang in ["en", "pt", "es", "fr", "ja", "zh", "de"] {
            let h = html(lang);
            for t in &TEXTS {
                assert_eq!(h.matches(&format!("<section lang=\"{}\">", t.lang)).count(), 1, "{lang}: {}", t.lang);
            }
        }
    }

    #[test]
    fn the_command_and_the_address_agree() {
        assert!(COMMAND.contains("8080") && URL.contains(":8080/"));
        assert!(COMMAND.contains("127.0.0.1") && URL.contains("127.0.0.1"), "a loopback server, nothing else");
        let h = html("en");
        assert_eq!(h.matches(COMMAND).count(), 6);
        assert_eq!(h.matches(URL).count(), 6);
    }

    #[test]
    fn the_policy_source_is_the_hash_of_the_script() {
        let s = script_source();
        assert!(s.starts_with("'sha256-") && s.ends_with('\''), "{s}");
        assert_eq!(s, crate::hash::csp_sha256(SCRIPT.as_bytes()));
    }
}
