// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Authentication control's default texts, in six languages** (WASM-009 D12).
//!
//! One table for both paint paths: the browser player reads it through the application's
//! `Locale` (ABI §12.1) and the Designer through the form's language. A developer's own property
//! value always wins (R1: "their text is translatable"); an empty property means the row here.
//!
//! Each key is a property of the control ([`crate::authentication::TEXT_PROPS`]). `{0}` in a text
//! stands for the masked destination the provider reported ("+1•••23", "a•••@example.com").
//! Portuguese is Brazilian Portuguese.

/// The languages of the table, in column order.
pub const LANGUAGES: [&str; 6] = ["en", "es", "pt", "ja", "zh", "fr"];

/// `(property, [en, es, pt, ja, zh, fr])`.
pub const TEXTS: &[(&str, [&str; 6])] = &[
    ("SignInCaption", ["Sign in", "Iniciar sesión", "Entrar", "サインイン", "登录", "Se connecter"]),
    ("SignUpCaption", ["Create account", "Crear cuenta", "Criar conta", "アカウントを作成", "创建账户", "Créer un compte"]),
    ("ConfirmCaption", ["Confirm account", "Confirmar cuenta", "Confirmar conta", "アカウントを確認", "确认账户", "Confirmer le compte"]),
    ("ForgotCaption", ["Forgot password", "Contraseña olvidada", "Esqueci a senha", "パスワードを忘れた場合", "忘记密码", "Mot de passe oublié"]),
    ("ResetCaption", ["Choose a new password", "Elija una contraseña nueva", "Escolha uma nova senha", "新しいパスワードを設定", "设置新密码", "Choisissez un nouveau mot de passe"]),
    ("MfaCaption", ["Verification code", "Código de verificación", "Código de verificação", "確認コード", "验证码", "Code de vérification"]),
    ("SignOutCaption", ["Sign out", "Cerrar sesión", "Sair", "サインアウト", "退出登录", "Se déconnecter"]),
    ("HostedCaption", ["Continue to sign in", "Continuar para iniciar sesión", "Continuar para entrar", "サインインへ進む", "继续登录", "Continuer vers la connexion"]),
    ("UserLabel", ["User name", "Nombre de usuario", "Nome de usuário", "ユーザー名", "用户名", "Nom d'utilisateur"]),
    ("EmailLabel", ["E-mail", "Correo electrónico", "E-mail", "メールアドレス", "电子邮件", "Adresse e-mail"]),
    ("PasswordLabel", ["Password", "Contraseña", "Senha", "パスワード", "密码", "Mot de passe"]),
    ("NewPasswordLabel", ["New password", "Contraseña nueva", "Nova senha", "新しいパスワード", "新密码", "Nouveau mot de passe"]),
    ("CodeLabel", ["Code", "Código", "Código", "コード", "代码", "Code"]),
    ("CreateAccountLink", ["Create an account", "Crear una cuenta", "Criar uma conta", "アカウントを作成する", "创建一个账户", "Créer un compte"]),
    ("ForgotLink", ["Forgot your password?", "¿Olvidó su contraseña?", "Esqueceu sua senha?", "パスワードをお忘れですか？", "忘记密码了？", "Mot de passe oublié ?"]),
    ("BackCaption", ["Back to sign in", "Volver a iniciar sesión", "Voltar para entrar", "サインインに戻る", "返回登录", "Retour à la connexion"]),
    ("SignedInAsText", ["Signed in as", "Sesión iniciada como", "Conectado como", "サインイン中:", "当前登录:", "Connecté en tant que"]),
    ("BusyText", ["Please wait", "Espere, por favor", "Aguarde", "お待ちください", "请稍候", "Veuillez patienter"]),
    (
        "SignInFailedText",
        [
            "The user name or the password is not right.",
            "El nombre de usuario o la contraseña no son correctos.",
            "O nome de usuário ou a senha não estão corretos.",
            "ユーザー名またはパスワードが正しくありません。",
            "用户名或密码不正确。",
            "Le nom d'utilisateur ou le mot de passe est incorrect.",
        ],
    ),
    (
        "ResetRequestedText",
        [
            "If the address is registered, a code has been sent to it.",
            "Si la dirección está registrada, se le ha enviado un código.",
            "Se o endereço estiver cadastrado, um código foi enviado para ele.",
            "このアドレスが登録されていれば、コードを送信しました。",
            "如果该地址已注册，验证码已发送至该地址。",
            "Si l'adresse est enregistrée, un code y a été envoyé.",
        ],
    ),
    (
        "ErrorText",
        [
            "That did not work. Try again in a moment.",
            "No funcionó. Vuelva a intentarlo en un momento.",
            "Não deu certo. Tente de novo daqui a pouco.",
            "うまくいきませんでした。しばらくしてからもう一度お試しください。",
            "操作未成功。请稍后再试。",
            "Cela n'a pas fonctionné. Réessayez dans un instant.",
        ],
    ),
    ("NewPasswordCaption", ["Set a new password", "Defina una contraseña nueva", "Defina uma nova senha", "新しいパスワードの設定", "设置新的密码", "Définissez un nouveau mot de passe"]),
    ("RepeatPasswordLabel", ["Repeat the new password", "Repita la contraseña nueva", "Repita a nova senha", "新しいパスワード(確認)", "再次输入新密码", "Répétez le nouveau mot de passe"]),
    (
        "MfaTotpText",
        [
            "Enter the code your authenticator app shows.",
            "Introduzca el código que muestra su aplicación de autenticación.",
            "Digite o código mostrado pelo seu aplicativo autenticador.",
            "認証アプリに表示されたコードを入力してください。",
            "请输入身份验证器应用中显示的验证码。",
            "Saisissez le code affiché par votre application d'authentification.",
        ],
    ),
    (
        "MfaSmsText",
        [
            "Enter the code sent by text message to {0}.",
            "Introduzca el código enviado por SMS a {0}.",
            "Digite o código enviado por SMS para {0}.",
            "{0} に SMS で送信されたコードを入力してください。",
            "请输入通过短信发送到 {0} 的验证码。",
            "Saisissez le code envoyé par SMS au {0}.",
        ],
    ),
    (
        "MfaEmailText",
        [
            "Enter the code sent by e-mail to {0}.",
            "Introduzca el código enviado por correo electrónico a {0}.",
            "Digite o código enviado por e-mail para {0}.",
            "{0} にメールで送信されたコードを入力してください。",
            "请输入通过电子邮件发送到 {0} 的验证码。",
            "Saisissez le code envoyé par e-mail à {0}.",
        ],
    ),
    (
        "MfaChooseCaption",
        [
            "How do you want to receive your code?",
            "¿Cómo quiere recibir su código?",
            "Como você quer receber o seu código?",
            "コードの受け取り方法を選択してください",
            "您希望如何接收验证码？",
            "Comment voulez-vous recevoir votre code ?",
        ],
    ),
    ("MfaTotpChoice", ["Authenticator app", "Aplicación de autenticación", "Aplicativo autenticador", "認証アプリ", "身份验证器应用", "Application d'authentification"]),
    ("MfaSmsChoice", ["Text message", "Mensaje de texto (SMS)", "Mensagem de texto (SMS)", "SMS", "短信", "SMS"]),
    ("MfaEmailChoice", ["E-mail", "Correo electrónico", "E-mail", "メール", "电子邮件", "E-mail"]),
    ("MfaSetupCaption", ["Set up your authenticator app", "Configure su aplicación de autenticación", "Configure seu aplicativo autenticador", "認証アプリの設定", "设置身份验证器应用", "Configurez votre application d'authentification"]),
    (
        "MfaSetupText",
        [
            "Add this account to your authenticator app with the link or the key below, then enter the code it shows.",
            "Añada esta cuenta a su aplicación de autenticación con el enlace o la clave de abajo y luego introduzca el código que muestre.",
            "Adicione esta conta ao seu aplicativo autenticador com o link ou a chave abaixo e depois digite o código que ele mostrar.",
            "下のリンクまたはキーでこのアカウントを認証アプリに追加し、表示されたコードを入力してください。",
            "请使用下方的链接或密钥将此账户添加到身份验证器应用，然后输入应用显示的验证码。",
            "Ajoutez ce compte à votre application d'authentification avec le lien ou la clé ci-dessous, puis saisissez le code affiché.",
        ],
    ),
    ("SecretLabel", ["Key", "Clave", "Chave", "キー", "密钥", "Clé"]),
    (
        "PasswordsDifferText",
        [
            "The two passwords are not the same.",
            "Las dos contraseñas no coinciden.",
            "As duas senhas não são iguais.",
            "2 つのパスワードが一致しません。",
            "两次输入的密码不一致。",
            "Les deux mots de passe ne sont pas identiques.",
        ],
    ),
];

/// The column of a locale code (`pt-BR`, `es_ES`, `ja`, `zh-Hans`, `FR`); English when unknown.
pub fn language_index(locale: &str) -> usize {
    let lang = locale.trim().split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
    LANGUAGES.iter().position(|l| *l == lang).unwrap_or(0)
}

/// The default text of `key` in `locale`, falling back to English; empty for an unknown key.
pub fn default_text(key: &str, locale: &str) -> &'static str {
    TEXTS.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, row)| row[language_index(locale)]).unwrap_or("")
}

/// What the end user reads for `key`: the developer's `value` when it is not blank, else the
/// default in `locale`.
pub fn text_or_default(value: &str, key: &str, locale: &str) -> String {
    if value.trim().is_empty() {
        default_text(key, locale).to_owned()
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_has_a_text_in_every_language_and_is_a_property_of_the_control() {
        let mut keys: Vec<&str> = TEXTS.iter().map(|(k, _)| *k).collect();
        for (k, row) in TEXTS {
            for (i, t) in row.iter().enumerate() {
                assert!(!t.trim().is_empty(), "{k} is empty in {}", LANGUAGES[i]);
            }
            let holes = |t: &str| t.matches("{0}").count();
            assert!(row.iter().all(|t| holes(t) == holes(row[0])), "{k}: every language keeps the {{0}} of the English");
        }
        let n = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), n, "a key is listed twice");
        let props: Vec<&str> = crate::authentication::TEXT_PROPS.to_vec();
        let mut sorted_props = props.clone();
        sorted_props.sort_unstable();
        assert_eq!(keys, sorted_props, "the table and the control's text properties are the same set");
        println!("auth_texts: {n} keys x {} languages = {} texts, none empty", LANGUAGES.len(), n * LANGUAGES.len());
    }

    #[test]
    fn the_lookup_follows_the_locale_and_falls_back_to_english() {
        assert_eq!(default_text("PasswordLabel", "pt-BR"), "Senha");
        assert_eq!(default_text("PasswordLabel", "es_ES"), "Contraseña");
        assert_eq!(default_text("PasswordLabel", "ja"), "パスワード");
        assert_eq!(default_text("PasswordLabel", "zh-Hans"), "密码");
        assert_eq!(default_text("PasswordLabel", "FR"), "Mot de passe");
        assert_eq!(default_text("PasswordLabel", "de"), "Password", "an unknown language is English");
        assert_eq!(default_text("PasswordLabel", ""), "Password");
        assert_eq!(default_text("NoSuchKey", "en"), "");
        assert_eq!(text_or_default("Palavra-passe", "PasswordLabel", "pt-BR"), "Palavra-passe", "the developer's value wins");
        assert_eq!(text_or_default("  ", "PasswordLabel", "pt-BR"), "Senha");
    }
}
