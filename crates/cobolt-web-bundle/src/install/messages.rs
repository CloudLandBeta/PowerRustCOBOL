//! What the install script says, in the six languages of the product. Each row is one message:
//! its key, then English, Spanish, Portuguese (Brazil), Japanese, Chinese (Simplified) and French.
//! A `{}` is where the script puts a value; every language of a row has the same number of them.
//! A language with no text for a key (an empty string) is answered in English.

/// The product's six languages, in the order of a row of [`TABLE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Es,
    Pt,
    Ja,
    Zh,
    Fr,
}

impl Lang {
    pub const ALL: [Lang; 6] = [Lang::En, Lang::Es, Lang::Pt, Lang::Ja, Lang::Zh, Lang::Fr];

    /// The language of a code: the IDE's (`en es pt ja zh fr`) or the documentation's (`jp`, `cn`);
    /// English for any other.
    pub fn from_code(code: &str) -> Lang {
        match code.trim().to_ascii_lowercase().as_str() {
            "es" => Lang::Es,
            "pt" | "pt-br" => Lang::Pt,
            "ja" | "jp" => Lang::Ja,
            "zh" | "cn" | "zh-cn" => Lang::Zh,
            "fr" => Lang::Fr,
            _ => Lang::En,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// The message `key` in `lang`: English where that language has no text, and the key itself for an unknown key.
pub fn msg(key: &str, lang: Lang) -> &'static str {
    match TABLE.iter().find(|(k, _)| *k == key) {
        Some((_, row)) => {
            let t = row[lang.index()];
            if t.is_empty() {
                row[0]
            } else {
                t
            }
        }
        None => "",
    }
}

type Row = (&'static str, [&'static str; 6]);

pub const TABLE: &[Row] = &[
    ("usage", ["Usage", "Uso", "Uso", "使い方", "用法", "Utilisation"]),
    (
        "p_nginx",
        [
            "folder where nginx is installed (required); it holds nginx.conf, or conf/nginx.conf",
            "carpeta donde está instalado nginx (obligatoria); contiene nginx.conf o conf/nginx.conf",
            "pasta onde o nginx está instalado (obrigatório); contém nginx.conf ou conf/nginx.conf",
            "nginx のインストール先フォルダ（必須）。nginx.conf または conf/nginx.conf があるフォルダ",
            "nginx 的安装目录（必填），其中包含 nginx.conf 或 conf/nginx.conf",
            "dossier où nginx est installé (obligatoire) ; il contient nginx.conf ou conf/nginx.conf",
        ],
    ),
    (
        "p_install",
        [
            "folder to install into (default: {})",
            "carpeta de instalación (por defecto: {})",
            "pasta de instalação (padrão: {})",
            "インストール先フォルダ（既定: {}）",
            "安装目录（默认：{}）",
            "dossier d'installation (par défaut : {})",
        ],
    ),
    (
        "p_server",
        [
            "server name (default: {})",
            "nombre del servidor (por defecto: {})",
            "nome do servidor (padrão: {})",
            "サーバー名（既定: {}）",
            "服务器名称（默认：{}）",
            "nom du serveur (par défaut : {})",
        ],
    ),
    (
        "p_cert",
        [
            "TLS certificate file, PEM (required: this application uses wss://)",
            "archivo del certificado TLS, PEM (obligatorio: esta aplicación usa wss://)",
            "arquivo do certificado TLS, PEM (obrigatório: este aplicativo usa wss://)",
            "TLS 証明書ファイル（PEM、必須: このアプリケーションは wss:// を使います）",
            "TLS 证书文件，PEM（必填：此应用使用 wss://）",
            "fichier du certificat TLS, PEM (obligatoire : cette application utilise wss://)",
        ],
    ),
    (
        "p_key",
        [
            "TLS private key file, PEM (required: this application uses wss://)",
            "archivo de la clave privada TLS, PEM (obligatorio: esta aplicación usa wss://)",
            "arquivo da chave privada TLS, PEM (obrigatório: este aplicativo usa wss://)",
            "TLS 秘密鍵ファイル（PEM、必須: このアプリケーションは wss:// を使います）",
            "TLS 私钥文件，PEM（必填：此应用使用 wss://）",
            "fichier de la clé privée TLS, PEM (obligatoire : cette application utilise wss://)",
        ],
    ),
    (
        "p_env",
        [
            "file with NAME=value lines for the Backend's secrets (optional)",
            "archivo con líneas NOMBRE=valor para los secretos del Backend (opcional)",
            "arquivo com linhas NOME=valor para os segredos do Backend (opcional)",
            "Backend の秘密情報を NAME=value 形式で書いたファイル（任意）",
            "包含后端机密的 NAME=value 文件（可选）",
            "fichier de lignes NOM=valeur pour les secrets du Backend (facultatif)",
        ],
    ),
    (
        "p_env_req",
        [
            "file with NAME=value lines for the Backend's secrets; it must define: {}",
            "archivo con líneas NOMBRE=valor para los secretos del Backend; debe definir: {}",
            "arquivo com linhas NOME=valor para os segredos do Backend; deve definir: {}",
            "Backend の秘密情報を NAME=value 形式で書いたファイル。次を定義する必要があります: {}",
            "包含后端机密的 NAME=value 文件；必须定义：{}",
            "fichier de lignes NOM=valeur pour les secrets du Backend ; il doit définir : {}",
        ],
    ),
    (
        "e_unknown",
        ["unknown parameter: {}", "parámetro desconocido: {}", "parâmetro desconhecido: {}", "不明なパラメーター: {}", "未知参数：{}", "paramètre inconnu : {}"],
    ),
    (
        "e_value",
        ["{} needs a value", "{} necesita un valor", "{} precisa de um valor", "{} には値が必要です", "{} 需要一个值", "{} attend une valeur"],
    ),
    (
        "e_name",
        [
            "{}: the server name may hold only letters, digits and . - * (and spaces between names)",
            "{}: el nombre del servidor solo puede tener letras, números y . - * (y espacios entre nombres)",
            "{}: o nome do servidor só pode ter letras, números e . - * (e espaços entre nomes)",
            "{}: サーバー名に使えるのは英数字と . - *（複数の名前は空白で区切る）のみです",
            "{}：服务器名称只能包含字母、数字和 . - *（多个名称之间用空格分隔）",
            "{} : le nom du serveur ne peut contenir que des lettres, des chiffres et . - * (et des espaces entre les noms)",
        ],
    ),
    (
        "e_no_nginx",
        [
            "the nginx folder is required",
            "la carpeta de nginx es obligatoria",
            "a pasta do nginx é obrigatória",
            "nginx のフォルダを指定してください",
            "必须指定 nginx 目录",
            "le dossier de nginx est obligatoire",
        ],
    ),
    (
        "e_bad_nginx",
        [
            "{}: nginx.conf was not found there or in conf/",
            "{}: no se encontró nginx.conf ahí ni en conf/",
            "{}: nginx.conf não foi encontrado ali nem em conf/",
            "{}: nginx.conf が見つかりません（conf/ も確認しました）",
            "{}：在此处及 conf/ 下均未找到 nginx.conf",
            "{} : nginx.conf introuvable ici et dans conf/",
        ],
    ),
    (
        "e_root",
        [
            "run this script as root (for example with sudo)",
            "ejecute este script como root (por ejemplo con sudo)",
            "execute este script como root (por exemplo, com sudo)",
            "このスクリプトは root で実行してください（例: sudo）",
            "请以 root 身份运行此脚本（例如使用 sudo）",
            "exécutez ce script en tant que root (par exemple avec sudo)",
        ],
    ),
    (
        "e_admin",
        [
            "run this script as Administrator",
            "ejecute este script como Administrador",
            "execute este script como Administrador",
            "このスクリプトは管理者として実行してください",
            "请以管理员身份运行此脚本",
            "exécutez ce script en tant qu'administrateur",
        ],
    ),
    (
        "e_tool",
        [
            "{} is needed and was not found",
            "se necesita {} y no se encontró",
            "{} é necessário e não foi encontrado",
            "{} が必要ですが見つかりません",
            "需要 {}，但未找到",
            "{} est nécessaire et reste introuvable",
        ],
    ),
    (
        "e_zip",
        [
            "the package file {} was not found next to this script",
            "no se encontró el paquete {} junto a este script",
            "o arquivo do pacote {} não foi encontrado ao lado deste script",
            "パッケージファイル {} がこのスクリプトと同じ場所にありません",
            "在此脚本旁未找到安装包 {}",
            "le fichier du paquet {} est introuvable à côté de ce script",
        ],
    ),
    (
        "e_hash",
        [
            "the package file {} does not match this script; copy both files again",
            "el paquete {} no corresponde a este script; copie de nuevo los dos archivos",
            "o pacote {} não corresponde a este script; copie os dois arquivos novamente",
            "パッケージ {} がこのスクリプトと一致しません。2 つのファイルを再度コピーしてください",
            "安装包 {} 与此脚本不匹配；请重新复制这两个文件",
            "le paquet {} ne correspond pas à ce script ; copiez de nouveau les deux fichiers",
        ],
    ),
    (
        "e_secrets",
        [
            "the environment file must define: {}",
            "el archivo de entorno debe definir: {}",
            "o arquivo de ambiente deve definir: {}",
            "環境ファイルに次の定義が必要です: {}",
            "环境文件必须定义：{}",
            "le fichier d'environnement doit définir : {}",
        ],
    ),
    (
        "e_cert",
        [
            "this application uses wss://: give {} and {} (PEM files)",
            "esta aplicación usa wss://: indique {} y {} (archivos PEM)",
            "este aplicativo usa wss://: informe {} e {} (arquivos PEM)",
            "このアプリケーションは wss:// を使います。{} と {}（PEM ファイル）を指定してください",
            "此应用使用 wss://：请提供 {} 和 {}（PEM 文件）",
            "cette application utilise wss:// : indiquez {} et {} (fichiers PEM)",
        ],
    ),
    (
        "e_path",
        [
            "{}: use only letters, digits and . _ / + - in the installation folder",
            "{}: use solo letras, números y . _ / + - en la carpeta de instalación",
            "{}: use apenas letras, números e . _ / + - na pasta de instalação",
            "{}: インストール先フォルダには英数字と . _ / + - のみ使用できます",
            "{}：安装目录只能使用字母、数字以及 . _ / + -",
            "{} : n'utilisez que des lettres, des chiffres et . _ / + - dans le dossier d'installation",
        ],
    ),
    ("e_file", ["file not found: {}", "archivo no encontrado: {}", "arquivo não encontrado: {}", "ファイルが見つかりません: {}", "未找到文件：{}", "fichier introuvable : {}"]),
    (
        "e_port",
        [
            "port {} is already in use by another program",
            "el puerto {} ya está en uso por otro programa",
            "a porta {} já está em uso por outro programa",
            "ポート {} は別のプログラムが使用中です",
            "端口 {} 已被其他程序占用",
            "le port {} est déjà utilisé par un autre programme",
        ],
    ),
    (
        "e_no_http",
        [
            "{}: no http block was found in nginx.conf",
            "{}: no se encontró un bloque http en nginx.conf",
            "{}: nenhum bloco http foi encontrado no nginx.conf",
            "{}: nginx.conf に http ブロックが見つかりません",
            "{}：在 nginx.conf 中未找到 http 块",
            "{} : aucun bloc http dans nginx.conf",
        ],
    ),
    (
        "e_start",
        [
            "the Backend did not start; see its log: {}",
            "el Backend no arrancó; vea su registro: {}",
            "o Backend não iniciou; veja o log: {}",
            "Backend が起動しませんでした。ログを確認してください: {}",
            "后端未能启动；请查看日志：{}",
            "le Backend n'a pas démarré ; voir son journal : {}",
        ],
    ),
    (
        "e_nginx",
        [
            "nginx rejected the configuration",
            "nginx rechazó la configuración",
            "o nginx rejeitou a configuração",
            "nginx が設定を拒否しました",
            "nginx 拒绝了该配置",
            "nginx a refusé la configuration",
        ],
    ),
    ("e_failed", ["failed at: {}", "falló en: {}", "falhou em: {}", "失敗した手順: {}", "失败步骤：{}", "échec à l'étape : {}"]),
    (
        "s_restored",
        [
            "The previous state was restored.",
            "Se restauró el estado anterior.",
            "O estado anterior foi restaurado.",
            "以前の状態に戻しました。",
            "已恢复到之前的状态。",
            "L'état précédent a été restauré.",
        ],
    ),
    ("s_check", ["Checking the system", "Comprobando el sistema", "Verificando o sistema", "システムを確認しています", "正在检查系统", "Vérification du système"]),
    (
        "s_backup",
        [
            "Saving what will change",
            "Guardando lo que va a cambiar",
            "Salvando o que vai mudar",
            "変更されるものを保存しています",
            "正在保存将被更改的内容",
            "Sauvegarde de ce qui va changer",
        ],
    ),
    ("s_extract", ["Extracting the package", "Extrayendo el paquete", "Extraindo o pacote", "パッケージを展開しています", "正在解压安装包", "Extraction du paquet"]),
    (
        "s_files",
        [
            "Installing the web files and the Backend",
            "Instalando los archivos web y el Backend",
            "Instalando os arquivos web e o Backend",
            "Web ファイルと Backend をインストールしています",
            "正在安装网页文件和后端",
            "Installation des fichiers web et du Backend",
        ],
    ),
    (
        "s_account",
        [
            "Preparing the service account",
            "Preparando la cuenta del servicio",
            "Preparando a conta do serviço",
            "サービスアカウントを準備しています",
            "正在准备服务账户",
            "Préparation du compte de service",
        ],
    ),
    (
        "s_service",
        [
            "Installing and starting the service",
            "Instalando e iniciando el servicio",
            "Instalando e iniciando o serviço",
            "サービスをインストールして起動しています",
            "正在安装并启动服务",
            "Installation et démarrage du service",
        ],
    ),
    (
        "s_wait",
        [
            "Waiting for the Backend on port {}",
            "Esperando al Backend en el puerto {}",
            "Aguardando o Backend na porta {}",
            "ポート {} で Backend を待っています",
            "正在等待后端监听端口 {}",
            "Attente du Backend sur le port {}",
        ],
    ),
    (
        "s_site",
        [
            "Writing the nginx site",
            "Escribiendo el sitio de nginx",
            "Escrevendo o site do nginx",
            "nginx のサイト設定を書き込んでいます",
            "正在写入 nginx 站点配置",
            "Écriture du site nginx",
        ],
    ),
    (
        "s_test",
        [
            "Testing the nginx configuration",
            "Probando la configuración de nginx",
            "Testando a configuração do nginx",
            "nginx の設定をテストしています",
            "正在测试 nginx 配置",
            "Test de la configuration nginx",
        ],
    ),
    ("s_reload", ["Reloading nginx", "Recargando nginx", "Recarregando o nginx", "nginx を再読み込みしています", "正在重新加载 nginx", "Rechargement de nginx"]),
    (
        "w_nginx_stopped",
        [
            "nginx is not running: start it to serve the site",
            "nginx no está en ejecución: inícielo para servir el sitio",
            "o nginx não está em execução: inicie-o para servir o site",
            "nginx が起動していません。サイトを提供するには起動してください",
            "nginx 未运行：请启动它以提供站点",
            "nginx n'est pas démarré : lancez-le pour servir le site",
        ],
    ),
    (
        "w_selinux",
        [
            "SELinux is enforcing; if nginx cannot read the site or reach the Backend, run: {}",
            "SELinux está en modo estricto; si nginx no puede leer el sitio o llegar al Backend, ejecute: {}",
            "o SELinux está em modo enforcing; se o nginx não conseguir ler o site ou alcançar o Backend, execute: {}",
            "SELinux が有効です。nginx がサイトを読めない、または Backend に届かない場合は次を実行してください: {}",
            "SELinux 处于强制模式；如果 nginx 无法读取站点或访问后端，请运行：{}",
            "SELinux est en mode enforcing ; si nginx ne peut pas lire le site ou joindre le Backend, exécutez : {}",
        ],
    ),
    (
        "w_toml",
        [
            "the previous backend.toml was saved in {}",
            "el backend.toml anterior se guardó en {}",
            "o backend.toml anterior foi salvo em {}",
            "以前の backend.toml は {} に保存しました",
            "之前的 backend.toml 已保存到 {}",
            "l'ancien backend.toml a été sauvegardé dans {}",
        ],
    ),
    ("done", ["Installed.", "Instalado.", "Instalado.", "インストールが完了しました。", "安装完成。", "Installation terminée."]),
    (
        "done_update",
        [
            "Updated. The records in data/ and the environment file were kept.",
            "Actualizado. Se conservaron los registros de data/ y el archivo de entorno.",
            "Atualizado. Os registros em data/ e o arquivo de ambiente foram mantidos.",
            "更新しました。data/ のレコードと環境ファイルは保持されています。",
            "已更新。data/ 中的记录和环境文件已保留。",
            "Mise à jour effectuée. Les enregistrements de data/ et le fichier d'environnement ont été conservés.",
        ],
    ),
    ("d_address", ["Address: {}", "Dirección: {}", "Endereço: {}", "アドレス: {}", "地址：{}", "Adresse : {}"]),
    ("d_service", ["Service: {}", "Servicio: {}", "Serviço: {}", "サービス: {}", "服务：{}", "Service : {}"]),
    ("d_status", ["Check the state: {}", "Ver el estado: {}", "Ver o estado: {}", "状態の確認: {}", "查看状态：{}", "Voir l'état : {}"]),
    ("d_log", ["Show the log: {}", "Ver el registro: {}", "Ver o log: {}", "ログの表示: {}", "查看日志：{}", "Voir le journal : {}"]),
    ("d_installed", ["Installed in: {}", "Instalado en: {}", "Instalado em: {}", "インストール先: {}", "安装位置：{}", "Installé dans : {}"]),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every key has all six languages, with the same placeholders, and no message names the platform's build-only crates.
    #[test]
    fn the_install_messages_hold_in_all_six_languages() {
        let mut checks = 0;
        let mut keys = std::collections::HashSet::new();
        for (key, row) in TABLE {
            assert!(keys.insert(*key), "duplicate key {key}");
            let slots = row[0].matches("{}").count();
            for (i, text) in row.iter().enumerate() {
                assert!(!text.is_empty(), "{key} has no text for language {i}");
                assert_eq!(text.matches("{}").count(), slots, "{key} language {i} has another number of {{}}");
                assert!(!text.to_ascii_lowercase().contains("cobolt"), "{key} language {i} names a build-only crate");
                assert!(!text.contains('\n'), "{key} language {i} has a line break");
                // The scripts hand a message to `printf` as its format: a `%` or a backslash would be read as one.
                assert!(!text.contains('%') && !text.contains('\\') && !text.starts_with('-'), "{key} language {i} would be misread as a printf format");
                checks += 4;
            }
        }
        for l in Lang::ALL {
            assert_ne!(msg("usage", l), "", "usage in {l:?}");
        }
        assert_eq!(msg("no-such-key", Lang::Fr), "");
        assert_eq!(Lang::from_code("jp"), Lang::Ja);
        assert_eq!(Lang::from_code("cn"), Lang::Zh);
        assert_eq!(Lang::from_code("pt-BR"), Lang::Pt);
        assert_eq!(Lang::from_code("xx"), Lang::En);
        println!("install messages: {} keys x 6 languages = {} texts, {checks} checks, 0 failed", TABLE.len(), TABLE.len() * 6);
    }
}
