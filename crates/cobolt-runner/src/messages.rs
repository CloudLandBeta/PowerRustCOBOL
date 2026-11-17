// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the runner says when it cannot run the program, in the six languages of the IDE, chosen by
//! the system locale (DESKTOP-WASM-001 R4, T15). The runner's own name is "the PowerRustCOBOL
//! runner"; a capability, an import or an ABI number is named as the program carries it.

/// A language the runner speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Es,
    Pt,
    Fr,
    Ja,
    Zh,
}

impl Lang {
    /// Every language, in the IDE's order.
    pub const ALL: [Lang; 6] = [Lang::En, Lang::Es, Lang::Pt, Lang::Fr, Lang::Ja, Lang::Zh];

    /// The language a locale name such as `pt_BR.UTF-8`, `ja-JP` or `zh_CN` stands for; `None` for
    /// an empty name. `C`, `POSIX` and a language the runner does not speak are English.
    pub fn of_locale(name: &str) -> Option<Lang> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        let tag = name.split(['_', '-', '.', '@']).next().unwrap_or("").to_ascii_lowercase();
        Some(match tag.as_str() {
            "es" => Lang::Es,
            "pt" => Lang::Pt,
            "fr" => Lang::Fr,
            "ja" => Lang::Ja,
            "zh" => Lang::Zh,
            _ => Lang::En,
        })
    }

    /// The system's language, as POSIX orders the variables: `LC_ALL`, then `LC_MESSAGES`, then
    /// `LANG`; English when none is set.
    pub fn system() -> Lang {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .find_map(|v| std::env::var(v).ok().and_then(|s| Lang::of_locale(&s)))
            .unwrap_or(Lang::En)
    }
}

/// Something the runner tells the person who started the program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    /// The executable carries no program (a bare runner, or one a packager stripped).
    NoProgram,
    /// The program inside is damaged; the detail says how.
    Damaged(String),
    /// The program was built for an ABI this runner does not run.
    Abi { found: u32, max: u32 },
    /// The program needs capabilities this runner does not provide, by name.
    Capabilities(Vec<String>),
    /// The program imports something this runner does not provide, `module.name`.
    Import(String),
    /// The engine could not load or start the module.
    CannotStart(String),
    /// A run-time error the program reported, ending the run.
    RuntimeError { code: u32, line: u32, text: String },
    /// The module stopped on a trap of its own.
    Trapped(String),
    /// A designer launch named a form the program does not carry: the name, and the forms it has.
    NoSuchForm { want: String, forms: Vec<String> },
}

/// The words of `msg` in `lang`.
pub fn text(lang: Lang, msg: &Msg) -> String {
    use Lang::*;
    match msg {
        Msg::NoProgram => match lang {
            En => "This file carries no program: the PowerRustCOBOL runner runs only a program a Build has put inside it. Build the project again.".into(),
            Es => "Este archivo no contiene ningún programa: el ejecutor de PowerRustCOBOL solo ejecuta un programa que una compilación ha puesto dentro de él. Vuelva a compilar el proyecto.".into(),
            Pt => "Este arquivo não contém nenhum programa: o executor do PowerRustCOBOL só executa um programa que um Build colocou dentro dele. Compile o projeto novamente.".into(),
            Fr => "Ce fichier ne contient aucun programme : l'exécuteur PowerRustCOBOL n'exécute qu'un programme qu'une compilation a placé à l'intérieur. Recompilez le projet.".into(),
            Ja => "このファイルにはプログラムが含まれていません。PowerRustCOBOL ランナーは、ビルドが内部に組み込んだプログラムだけを実行します。プロジェクトをもう一度ビルドしてください。".into(),
            Zh => "此文件不包含程序：PowerRustCOBOL 运行器只运行由构建放入其中的程序。请重新构建项目。".into(),
        },
        Msg::Damaged(detail) => match lang {
            En => format!("The program inside this file is damaged ({detail}). Build the project again."),
            Es => format!("El programa que contiene este archivo está dañado ({detail}). Vuelva a compilar el proyecto."),
            Pt => format!("O programa contido neste arquivo está danificado ({detail}). Compile o projeto novamente."),
            Fr => format!("Le programme contenu dans ce fichier est endommagé ({detail}). Recompilez le projet."),
            Ja => format!("このファイル内のプログラムが破損しています（{detail}）。プロジェクトをもう一度ビルドしてください。"),
            Zh => format!("此文件中的程序已损坏（{detail}）。请重新构建项目。"),
        },
        Msg::Abi { found, max } => match lang {
            En => format!("This program was built for ABI {found}, and this PowerRustCOBOL runner runs ABI 1 to {max}. Build it again with a current PowerRustCOBOL release."),
            Es => format!("Este programa se compiló para la ABI {found}, y este ejecutor de PowerRustCOBOL ejecuta las ABI 1 a {max}. Vuelva a compilarlo con una versión actual de PowerRustCOBOL."),
            Pt => format!("Este programa foi compilado para a ABI {found}, e este executor do PowerRustCOBOL executa as ABI 1 a {max}. Compile-o novamente com uma versão atual do PowerRustCOBOL."),
            Fr => format!("Ce programme a été compilé pour l'ABI {found}, et cet exécuteur PowerRustCOBOL exécute les ABI 1 à {max}. Recompilez-le avec une version actuelle de PowerRustCOBOL."),
            Ja => format!("このプログラムは ABI {found} 向けにビルドされていますが、この PowerRustCOBOL ランナーが実行できるのは ABI 1 から {max} までです。最新の PowerRustCOBOL でもう一度ビルドしてください。"),
            Zh => format!("此程序是为 ABI {found} 构建的，而此 PowerRustCOBOL 运行器运行 ABI 1 到 {max}。请使用当前版本的 PowerRustCOBOL 重新构建。"),
        },
        Msg::Capabilities(names) => {
            let names = names.join(", ");
            match lang {
                En => format!("This program needs a capability this PowerRustCOBOL runner does not provide: {names}. Build it again with a current PowerRustCOBOL release."),
                Es => format!("Este programa necesita una capacidad que este ejecutor de PowerRustCOBOL no ofrece: {names}. Vuelva a compilarlo con una versión actual de PowerRustCOBOL."),
                Pt => format!("Este programa precisa de um recurso que este executor do PowerRustCOBOL não oferece: {names}. Compile-o novamente com uma versão atual do PowerRustCOBOL."),
                Fr => format!("Ce programme a besoin d'une capacité que cet exécuteur PowerRustCOBOL ne fournit pas : {names}. Recompilez-le avec une version actuelle de PowerRustCOBOL."),
                Ja => format!("このプログラムには、この PowerRustCOBOL ランナーが提供していない機能が必要です: {names}。最新の PowerRustCOBOL でもう一度ビルドしてください。"),
                Zh => format!("此程序需要此 PowerRustCOBOL 运行器未提供的功能：{names}。请使用当前版本的 PowerRustCOBOL 重新构建。"),
            }
        }
        Msg::Import(name) => match lang {
            En => format!("This program uses {name}, which this PowerRustCOBOL runner does not provide. Build it again with a current PowerRustCOBOL release."),
            Es => format!("Este programa usa {name}, que este ejecutor de PowerRustCOBOL no ofrece. Vuelva a compilarlo con una versión actual de PowerRustCOBOL."),
            Pt => format!("Este programa usa {name}, que este executor do PowerRustCOBOL não oferece. Compile-o novamente com uma versão atual do PowerRustCOBOL."),
            Fr => format!("Ce programme utilise {name}, que cet exécuteur PowerRustCOBOL ne fournit pas. Recompilez-le avec une version actuelle de PowerRustCOBOL."),
            Ja => format!("このプログラムは {name} を使用しますが、この PowerRustCOBOL ランナーはそれを提供していません。最新の PowerRustCOBOL でもう一度ビルドしてください。"),
            Zh => format!("此程序使用了 {name}，但此 PowerRustCOBOL 运行器未提供它。请使用当前版本的 PowerRustCOBOL 重新构建。"),
        },
        Msg::CannotStart(detail) => match lang {
            En => format!("The program could not start: {detail}"),
            Es => format!("El programa no pudo iniciarse: {detail}"),
            Pt => format!("O programa não pôde ser iniciado: {detail}"),
            Fr => format!("Le programme n'a pas pu démarrer : {detail}"),
            Ja => format!("プログラムを開始できませんでした: {detail}"),
            Zh => format!("程序无法启动：{detail}"),
        },
        Msg::RuntimeError { code, line, text } => {
            let head = match lang {
                En => format!("Runtime error {code} at line {line}"),
                Es => format!("Error de ejecución {code} en la línea {line}"),
                Pt => format!("Erro de execução {code} na linha {line}"),
                Fr => format!("Erreur d'exécution {code} à la ligne {line}"),
                Ja => format!("実行時エラー {code}（{line} 行目）"),
                Zh => format!("运行时错误 {code}（第 {line} 行）"),
            };
            if text.is_empty() {
                head
            } else {
                format!("{head}: {text}")
            }
        }
        Msg::Trapped(detail) => match lang {
            En => format!("The program stopped unexpectedly: {detail}"),
            Es => format!("El programa se detuvo inesperadamente: {detail}"),
            Pt => format!("O programa parou inesperadamente: {detail}"),
            Fr => format!("Le programme s'est arrêté de façon inattendue : {detail}"),
            Ja => format!("プログラムが予期せず停止しました: {detail}"),
            Zh => format!("程序意外停止：{detail}"),
        },
        Msg::NoSuchForm { want, forms } => {
            let forms = forms.join(", ");
            match lang {
                En => format!("This application has no form named '{want}'. It carries: {forms}."),
                Es => format!("Esta aplicación no tiene ningún formulario llamado '{want}'. Contiene: {forms}."),
                Pt => format!("Este aplicativo não tem nenhum formulário chamado '{want}'. Ele contém: {forms}."),
                Fr => format!("Cette application n'a aucun formulaire nommé '{want}'. Elle contient : {forms}."),
                Ja => format!("このアプリケーションには '{want}' という名前のフォームはありません。含まれるフォーム: {forms}。"),
                Zh => format!("此应用程序没有名为 '{want}' 的窗体。它包含：{forms}。"),
            }
        }
    }
}
