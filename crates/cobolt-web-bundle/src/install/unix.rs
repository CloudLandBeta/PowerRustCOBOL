//! The install script for Linux (systemd) and macOS (launchd): one `bash` file that checks everything
//! before it changes anything, undoes what it did when a step fails, and can be run again to update.

use super::{msg, sh_quote, usage_lines, with_slots, Install, TargetOs, KEYS};

pub(super) fn script(i: &Install) -> String {
    let mac = i.os == TargetOs::MacOs;
    let b = i.backend.as_ref();
    let mut s = String::new();

    s.push_str(&format!(
        "#!/usr/bin/env bash\n# {project}: installs the application on this computer (PowerRustCOBOL AI, Package).\n# It writes the web files, the Backend as a {kind}, and an nginx site; it needs this file's package ({zip}) beside it.\n#   sudo bash ./{script} <nginx-folder>\n# Run it again to update: the records in data/, the environment file and the logs are kept.\n",
        project = i.project.replace('\n', " "),
        kind = if mac { "launchd service" } else { "systemd service" },
        zip = i.zip_name(),
        script = i.script_name(),
    ));
    s.push_str("set -u\numask 022\n\n");

    let host = b.map(|b| b.addr.rsplit_once(':').map(|(h, _)| h.trim_matches(|c| c == '[' || c == ']').to_owned()).unwrap_or_else(|| "127.0.0.1".into())).unwrap_or_else(|| "127.0.0.1".into());
    let var = |name: &str, value: &str| format!("{name}={}\n", sh_quote(value));
    s.push_str(&var("PROJECT", &i.project.replace('\n', " ")));
    s.push_str(&var("SLUG", &i.slug));
    s.push_str(&var("SERVICE", &i.service()));
    s.push_str(&var("ACCOUNT", &i.account()));
    s.push_str(&var("ZIP", &i.zip_name()));
    s.push_str(&var("ZIP_SHA256", &i.zip_sha256));
    s.push_str(&var("EXE", b.map(|b| b.exe.as_str()).unwrap_or("")));
    s.push_str(&var("HOST", &host));
    s.push_str(&var("PORT", &b.map(|b| b.port.to_string()).unwrap_or_default()));
    s.push_str(&var("WSS", if i.wss { "1" } else { "0" }));
    s.push_str(&var("HAS_BACKEND", if b.is_some() { "1" } else { "0" }));
    s.push_str(&var("HAS_KB", if b.is_some_and(|b| b.has_kb) { "1" } else { "0" }));
    s.push_str(&var("SERVER_NAME_DEFAULT", &i.server_name));
    s.push_str(&var("INSTALL_DEFAULT", &i.default_install_dir()));
    s.push_str(&var("REQUIRED", &b.map(|b| b.required_secrets.join(" ")).unwrap_or_default()));
    s.push_str("STATE_DIR=/etc/$SLUG\nLOG_DIR=/var/log/$SLUG\n");
    s.push_str(&format!("OS={}\n\n", if mac { "macos" } else { "linux" }));

    for key in KEYS {
        s.push_str(&format!("M_{key}={}\n", sh_quote(&with_slots(msg(key, i.lang), |_| "%s".into()))));
    }
    s.push('\n');

    // The usage text, with the descriptions already in the language.
    let lines = usage_lines(
        i,
        |name| {
            let value = match name {
                "install-dir" => "DIR",
                "server-name" => "NAME",
                _ => "FILE",
            };
            format!("--{name} {value}")
        },
        "<nginx-folder>",
    );
    s.push_str("usage() {\n  cat <<'PRC_USAGE'\n");
    s.push_str(&format!("{}: sudo bash ./{} <nginx-folder> [options]\n\n", msg("usage", i.lang), i.script_name()));
    for (flag, text) in lines {
        s.push_str(&format!("  {flag:<24} {text}\n"));
    }
    s.push_str("PRC_USAGE\n}\n\n");

    s.push_str(COMMON);
    s.push_str(if mac { MACOS_ACCOUNT } else { LINUX_ACCOUNT });
    s.push_str(if mac { MACOS_SERVICE } else { LINUX_SERVICE });
    s.push_str("\n# The nginx site this installation writes; the script puts the name, the folder and the certificate where the @ marks are.\nwrite_site_template() {\n  cat >\"$WORK/site.tpl\" <<'PRC_SITE'\n");
    s.push_str(&i.site_template());
    s.push_str("PRC_SITE\n}\n");
    s.push_str(MAIN);
    s
}

const COMMON: &str = r##"err() { printf '!! %s\n' "$1" >&2; }
say() { printf '==> %s\n' "$1"; }
warn() { printf '!  %s\n' "$1" >&2; }
fmt() { local f=$1; shift; printf "$f" "$@"; }
q() { printf '%q' "$1"; }
abspath() { case $1 in /*) printf '%s' "$1" ;; *) printf '%s/%s' "$(pwd)" "$1" ;; esac; }
tcp_open() { (exec 3<>"/dev/tcp/$1/$2") 2>/dev/null; }
sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | awk '{print $1}'
  else openssl dgst -sha256 -r "$1" | awk '{print $1}'; fi
}
die_usage() { err "$1"; usage >&2; exit 2; }

# What this run changed, undone in reverse order when a step fails.
UNDO=()
STEP=''
undo() { UNDO[${#UNDO[@]}]=$1; }
rollback() {
  local n=${#UNDO[@]}
  trap - INT TERM
  [ "$n" -gt 0 ] || return 0
  while [ "$n" -gt 0 ]; do n=$((n - 1)); eval "${UNDO[$n]}" >/dev/null 2>&1; done
  say "$M_s_restored" >&2
}
fail() { err "$1"; rollback; exit 1; }
must() { "$@" || fail "$(fmt "$M_e_failed" "$*")"; }
step() { STEP=$1; say "$1"; }
WORK=''
cleanup() { [ -z "$WORK" ] || rm -rf "$WORK"; }
"##;

const LINUX_ACCOUNT: &str = r##"
make_account() {
  id -u "$ACCOUNT" >/dev/null 2>&1 && return 0
  local shell=/bin/false
  [ -x /usr/sbin/nologin ] && shell=/usr/sbin/nologin
  [ -x /sbin/nologin ] && shell=/sbin/nologin
  must useradd --system --user-group --no-create-home --home-dir /nonexistent --shell "$shell" "$ACCOUNT"
  undo "userdel $(q "$ACCOUNT")"
}
"##;

const MACOS_ACCOUNT: &str = r##"
make_account() {
  dscl . -read "/Users/$ACCOUNT" UniqueID >/dev/null 2>&1 && return 0
  local used id=399
  used=$({ dscl . -list /Users UniqueID; dscl . -list /Groups PrimaryGroupID; } | awk '{print $2}' | tr '\n' ' ')
  while [ "$id" -ge 300 ]; do
    case " $used " in *" $id "*) id=$((id - 1)) ;; *) break ;; esac
  done
  [ "$id" -ge 300 ] || fail "$(fmt "$M_e_failed" "dscl")"
  undo "dscl . -delete /Users/$(q "$ACCOUNT"); dscl . -delete /Groups/$(q "$ACCOUNT")"
  must dscl . -create "/Groups/$ACCOUNT"
  must dscl . -create "/Groups/$ACCOUNT" PrimaryGroupID "$id"
  must dscl . -create "/Groups/$ACCOUNT" RealName "$PROJECT"
  must dscl . -create "/Users/$ACCOUNT"
  must dscl . -create "/Users/$ACCOUNT" UserShell /usr/bin/false
  must dscl . -create "/Users/$ACCOUNT" RealName "$PROJECT"
  must dscl . -create "/Users/$ACCOUNT" UniqueID "$id"
  must dscl . -create "/Users/$ACCOUNT" PrimaryGroupID "$id"
  must dscl . -create "/Users/$ACCOUNT" NFSHomeDirectory /var/empty
  must dscl . -create "/Users/$ACCOUNT" IsHidden 1
}
"##;

const LINUX_SERVICE: &str = r##"
UNIT=/etc/systemd/system/$SERVICE.service
service_installed() { [ -f "$UNIT" ]; }
service_stop() {
  service_installed || return 0
  must systemctl stop "$SERVICE"
  undo "systemctl start $(q "$SERVICE")"
}
service_start() {
  if [ -f "$UNIT" ]; then cp -p "$UNIT" "$WORK/unit.prev"; undo "cp -p $(q "$WORK/unit.prev") $(q "$UNIT"); systemctl daemon-reload; systemctl restart $(q "$SERVICE")"
  else undo "systemctl disable --now $(q "$SERVICE"); rm -f $(q "$UNIT"); systemctl daemon-reload"; fi
  cat >"$WORK/unit" <<EOF
[Unit]
Description=$PROJECT (Backend)
After=network.target

[Service]
Type=notify
NotifyAccess=main
User=$ACCOUNT
Group=$ACCOUNT
WorkingDirectory=$INSTALL_DIR
ExecStart=$INSTALL_DIR/bin/$EXE --config $INSTALL_DIR/backend.toml --env-file $STATE_DIR/backend.env --log-file $LOG_DIR/backend.log
Restart=on-failure
RestartSec=3
TimeoutStartSec=60
TimeoutStopSec=30
NoNewPrivileges=true
PrivateTmp=true
ProtectHome=true
ProtectSystem=strict
ReadWritePaths=-$INSTALL_DIR/data -$INSTALL_DIR/assets -$LOG_DIR

[Install]
WantedBy=multi-user.target
EOF
  must cp "$WORK/unit" "$UNIT"
  must chmod 644 "$UNIT"
  must systemctl daemon-reload
  must systemctl enable "$SERVICE"
  systemctl restart "$SERVICE" || { tail -n 20 "$LOG_DIR/backend.log" >&2 2>/dev/null; fail "$(fmt "$M_e_start" "$LOG_DIR/backend.log")"; }
}
service_status_hint() { printf 'systemctl status %s' "$SERVICE"; }
"##;

const MACOS_SERVICE: &str = r##"
PLIST=/Library/LaunchDaemons/$SERVICE.plist
service_installed() { [ -f "$PLIST" ]; }
service_stop() {
  service_installed || return 0
  launchctl bootout "system/$SERVICE" >/dev/null 2>&1
  undo "launchctl bootstrap system $(q "$PLIST")"
}
service_start() {
  if [ -f "$PLIST" ]; then cp -p "$PLIST" "$WORK/plist.prev"; undo "launchctl bootout system/$(q "$SERVICE"); cp -p $(q "$WORK/plist.prev") $(q "$PLIST"); launchctl bootstrap system $(q "$PLIST")"
  else undo "launchctl bootout system/$(q "$SERVICE"); rm -f $(q "$PLIST")"; fi
  cat >"$WORK/plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$SERVICE</string>
  <key>ProgramArguments</key>
  <array>
    <string>$INSTALL_DIR/bin/$EXE</string>
    <string>--config</string><string>$INSTALL_DIR/backend.toml</string>
    <string>--env-file</string><string>$STATE_DIR/backend.env</string>
    <string>--log-file</string><string>$LOG_DIR/backend.log</string>
  </array>
  <key>UserName</key><string>$ACCOUNT</string>
  <key>GroupName</key><string>$ACCOUNT</string>
  <key>WorkingDirectory</key><string>$INSTALL_DIR</string>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
  <key>ThrottleInterval</key><integer>10</integer>
  <key>StandardErrorPath</key><string>$LOG_DIR/launchd.err</string>
</dict>
</plist>
EOF
  must cp "$WORK/plist" "$PLIST"
  must chown root:wheel "$PLIST"
  must chmod 644 "$PLIST"
  launchctl bootout "system/$SERVICE" >/dev/null 2>&1
  launchctl bootstrap system "$PLIST" || fail "$(fmt "$M_e_start" "$LOG_DIR/backend.log")"
}
service_status_hint() { printf 'sudo launchctl print system/%s' "$SERVICE"; }
"##;

const MAIN: &str = r##"
# ---- the parameters (before anything is touched) ----
NGINX=''; INSTALL_DIR=''; SERVER_NAME=''; CERT=''; KEY=''; ENV_SRC=''
while [ $# -gt 0 ]; do
  case $1 in
    -h|--help) usage; exit 0 ;;
    --install-dir|--server-name|--cert|--key|--env-file)
      [ $# -ge 2 ] || die_usage "$(fmt "$M_e_value" "$1")"
      case $1 in
        --install-dir) INSTALL_DIR=$2 ;;
        --server-name) SERVER_NAME=$2 ;;
        --cert) CERT=$2 ;;
        --key) KEY=$2 ;;
        --env-file) ENV_SRC=$2 ;;
      esac
      shift 2 ;;
    -*) die_usage "$(fmt "$M_e_unknown" "$1")" ;;
    *) if [ -z "$NGINX" ]; then NGINX=$1; shift; else die_usage "$(fmt "$M_e_unknown" "$1")"; fi ;;
  esac
done
[ -n "$NGINX" ] || die_usage "$M_e_no_nginx"

# What the last run chose is the default of this one; a parameter wins.
if [ -f "$STATE_DIR/install.conf" ]; then
  while IFS='=' read -r k v; do
    case $k in
      INSTALL_DIR) [ -n "$INSTALL_DIR" ] || INSTALL_DIR=$v ;;
      SERVER_NAME) [ -n "$SERVER_NAME" ] || SERVER_NAME=$v ;;
      CERT) [ -n "$CERT" ] || CERT=$v ;;
      KEY) [ -n "$KEY" ] || KEY=$v ;;
    esac
  done <"$STATE_DIR/install.conf"
fi
[ -n "$INSTALL_DIR" ] || INSTALL_DIR=$INSTALL_DEFAULT
[ -n "$SERVER_NAME" ] || SERVER_NAME=$SERVER_NAME_DEFAULT

[ -d "$NGINX" ] || die_usage "$(fmt "$M_e_bad_nginx" "$NGINX")"
NGINX=$(cd "$NGINX" && pwd)
if [ -f "$NGINX/nginx.conf" ]; then CONF=$NGINX/nginx.conf
elif [ -f "$NGINX/conf/nginx.conf" ]; then CONF=$NGINX/conf/nginx.conf
else die_usage "$(fmt "$M_e_bad_nginx" "$NGINX")"; fi
CONF_DIR=$(dirname "$CONF")

case $INSTALL_DIR in /*) ;; *) die_usage "$(fmt "$M_e_path" "$INSTALL_DIR")" ;; esac
case $INSTALL_DIR in *[!A-Za-z0-9._/+-]*) die_usage "$(fmt "$M_e_path" "$INSTALL_DIR")" ;; esac
INSTALL_DIR=${INSTALL_DIR%/}
case $SERVER_NAME in *[!A-Za-z0-9._*\ -]*|'') die_usage "$(fmt "$M_e_name" "$SERVER_NAME")" ;; esac

if [ "$WSS" = 1 ]; then
  { [ -n "$CERT" ] && [ -n "$KEY" ]; } || die_usage "$(fmt "$M_e_cert" "--cert" "--key")"
  CERT=$(abspath "$CERT"); KEY=$(abspath "$KEY")
  for f in "$CERT" "$KEY"; do
    [ -f "$f" ] || die_usage "$(fmt "$M_e_file" "$f")"
    case $f in *[\"\\]*) die_usage "$(fmt "$M_e_path" "$f")" ;; esac
  done
fi

ENV_DST=$STATE_DIR/backend.env
if [ "$HAS_BACKEND" = 1 ]; then
  if [ -n "$ENV_SRC" ]; then
    [ -f "$ENV_SRC" ] || die_usage "$(fmt "$M_e_file" "$ENV_SRC")"
    ENV_SRC=$(abspath "$ENV_SRC")
    for n in $REQUIRED; do
      grep -Eq "^[[:space:]]*$n[[:space:]]*=" "$ENV_SRC" || die_usage "$(fmt "$M_e_secrets" "$REQUIRED")"
    done
  elif [ -f "$ENV_DST" ]; then ENV_SRC=''
  elif [ -n "$REQUIRED" ]; then die_usage "$(fmt "$M_e_secrets" "$REQUIRED")"
  fi
fi

# ---- the system ----
say "$M_s_check"
[ "$(id -u)" -eq 0 ] || { err "$M_e_root"; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || { err "$(fmt "$M_e_tool" "$1")"; exit 1; }; }
need unzip; need awk; need sed; need grep
if [ "$OS" = linux ]; then need systemctl; need useradd; else need launchctl; need dscl; fi
NGINX_BIN=''
for c in "$NGINX/sbin/nginx" "$NGINX/nginx" /usr/sbin/nginx /usr/local/sbin/nginx /usr/local/bin/nginx /opt/homebrew/bin/nginx /usr/bin/nginx; do
  if [ -x "$c" ] && [ ! -d "$c" ]; then NGINX_BIN=$c; break; fi
done
[ -n "$NGINX_BIN" ] || NGINX_BIN=$(command -v nginx 2>/dev/null || true)
[ -n "$NGINX_BIN" ] || { err "$(fmt "$M_e_tool" "nginx")"; exit 1; }

HERE=$(cd "$(dirname "$0")" && pwd)
[ -f "$HERE/$ZIP" ] || { err "$(fmt "$M_e_zip" "$ZIP")"; exit 1; }
[ "$(sha256_of "$HERE/$ZIP")" = "$ZIP_SHA256" ] || { err "$(fmt "$M_e_hash" "$ZIP")"; exit 1; }

UPDATE=0
if [ "$HAS_BACKEND" = 1 ]; then
  service_installed && UPDATE=1
  if [ "$UPDATE" = 0 ] && tcp_open "$HOST" "$PORT"; then err "$(fmt "$M_e_port" "$PORT")"; exit 1; fi
elif [ -f "$STATE_DIR/install.conf" ]; then UPDATE=1; fi

WORK=$(mktemp -d "${TMPDIR:-/tmp}/$SLUG-install.XXXXXX") || exit 1
trap cleanup EXIT
trap 'fail "$(fmt "$M_e_failed" "$STEP")"' INT TERM

step "$M_s_extract"
must unzip -q "$HERE/$ZIP" -d "$WORK/pkg"
[ -f "$WORK/pkg/web/index.html" ] || fail "$(fmt "$M_e_file" "web/index.html")"
if [ "$HAS_BACKEND" = 1 ]; then
  [ -f "$WORK/pkg/bin/$EXE" ] || fail "$(fmt "$M_e_file" "bin/$EXE")"
  [ -f "$WORK/pkg/backend.toml" ] || fail "$(fmt "$M_e_file" "backend.toml")"
fi

# A folder of this package replaces the one of the last run; the old one is kept until the end.
swap_dir() {
  local new=$1 target=$2
  rm -rf "$target.prev"
  if [ -e "$target" ]; then
    must mv "$target" "$target.prev"
    undo "rm -rf $(q "$target"); mv $(q "$target.prev") $(q "$target")"
  else
    undo "rm -rf $(q "$target")"
  fi
  must mv "$new" "$target"
}
# A folder that holds the records of the application is only ever seeded: never replaced, never emptied.
seed_dir() {
  local from=$1 target=$2
  local parent
  [ -d "$target" ] && return 0
  parent=$(dirname "$target")
  if [ ! -d "$parent" ]; then must mkdir -p "$parent"; undo "rmdir $(q "$parent")"; fi
  if [ -d "$from" ]; then must cp -R "$from" "$target"; else must mkdir "$target"; fi
  undo "rm -rf $(q "$target")"
  must chown -R "$ACCOUNT" "$target"
}

# ---- the account, then the files ----
if [ "$HAS_BACKEND" = 1 ]; then
  step "$M_s_account"
  make_account
fi
step "$M_s_files"
if [ ! -d "$INSTALL_DIR" ]; then must mkdir -p "$INSTALL_DIR"; undo "rmdir $(q "$INSTALL_DIR")"; fi
must chmod 755 "$INSTALL_DIR"
if [ "$HAS_BACKEND" = 1 ]; then
  service_stop
fi
swap_dir "$WORK/pkg/web" "$INSTALL_DIR/web"
find "$INSTALL_DIR/web" -type d -exec chmod 755 {} +
find "$INSTALL_DIR/web" -type f -exec chmod 644 {} +
for f in "$WORK/pkg"/LICENSE* "$WORK/pkg"/NOTICE*; do
  [ -f "$f" ] && cp "$f" "$INSTALL_DIR/" && undo "rm -f $(q "$INSTALL_DIR/$(basename "$f")")"
done

if [ "$HAS_BACKEND" = 1 ]; then
  swap_dir "$WORK/pkg/bin" "$INSTALL_DIR/bin"
  must chmod 755 "$INSTALL_DIR/bin" "$INSTALL_DIR/bin/$EXE"

  if [ -f "$INSTALL_DIR/backend.toml" ]; then
    cp -p "$INSTALL_DIR/backend.toml" "$WORK/toml.prev"
    undo "cp -p $(q "$WORK/toml.prev") $(q "$INSTALL_DIR/backend.toml")"
    if ! cmp -s "$INSTALL_DIR/backend.toml" "$WORK/pkg/backend.toml"; then
      BAK=$INSTALL_DIR/backend.toml.$(date +%Y%m%d%H%M%S).bak
      must cp -p "$INSTALL_DIR/backend.toml" "$BAK"
      warn "$(fmt "$M_w_toml" "$BAK")"
    fi
  else
    undo "rm -f $(q "$INSTALL_DIR/backend.toml")"
  fi
  must cp "$WORK/pkg/backend.toml" "$INSTALL_DIR/backend.toml"
  must chmod 644 "$INSTALL_DIR/backend.toml"

  seed_dir "$WORK/pkg/data" "$INSTALL_DIR/data"
  if [ "$HAS_KB" = 1 ]; then seed_dir "$WORK/pkg/assets/KB" "$INSTALL_DIR/assets/KB"; fi

  if [ ! -d "$STATE_DIR" ]; then must mkdir -p "$STATE_DIR"; undo "rmdir $(q "$STATE_DIR")"; fi
  must chmod 755 "$STATE_DIR"
  if [ ! -d "$LOG_DIR" ]; then must mkdir -p "$LOG_DIR"; undo "rm -rf $(q "$LOG_DIR")"; fi
  must chown "$ACCOUNT" "$LOG_DIR"
  must chmod 750 "$LOG_DIR"

  # The secrets: a file only the service's account can read; one the operator gives replaces the last, the last is kept otherwise.
  if [ -f "$ENV_DST" ]; then cp -p "$ENV_DST" "$WORK/env.prev"; undo "cp -p $(q "$WORK/env.prev") $(q "$ENV_DST")"; else undo "rm -f $(q "$ENV_DST")"; fi
  if [ -n "$ENV_SRC" ]; then
    ( umask 077; cp "$ENV_SRC" "$ENV_DST.new" ) || fail "$(fmt "$M_e_failed" "cp")"
    must mv "$ENV_DST.new" "$ENV_DST"
  elif [ ! -f "$ENV_DST" ]; then
    ( umask 077; : >"$ENV_DST" ) || fail "$(fmt "$M_e_failed" "$ENV_DST")"
  fi
  must chown "$ACCOUNT" "$ENV_DST"
  must chmod 600 "$ENV_DST"

  step "$M_s_service"
  service_start
  step "$(fmt "$M_s_wait" "$PORT")"
  n=0
  until tcp_open "$HOST" "$PORT"; do
    n=$((n + 1))
    [ "$n" -le 60 ] || fail "$(fmt "$M_e_start" "$LOG_DIR/backend.log")"
    sleep 1
  done
fi

# ---- nginx ----
step "$M_s_site"
write_site_template
SITE_DIR=''
# The include lines of the http block that name a folder with a wildcard: the folder nginx already reads sites from.
find_site_dir() {
  local pat dir base patterns
  patterns=$(awk '
    /^[ \t]*http[ \t]*\{/ { inhttp = 1; depth = 1; next }
    inhttp {
      line = $0; sub(/#.*/, "", line)
      opens = gsub(/\{/, "{", line); closes = gsub(/\}/, "}", line)
      if (depth == 1 && line ~ /^[ \t]*include[ \t]/ && line ~ /\*/) {
        s = line; sub(/^[ \t]*include[ \t]+/, "", s); sub(/[ \t]*;.*/, "", s); gsub(/"/, "", s); print s
      }
      depth += opens - closes
      if (depth <= 0) inhttp = 0
    }' "$CONF")
  while IFS= read -r pat; do
    [ -n "$pat" ] || continue
    case $pat in /*) ;; *) pat=$CONF_DIR/$pat ;; esac
    dir=$(dirname "$pat"); base=$(basename "$pat")
    case $base in '*.conf'|'*') if [ -d "$dir" ]; then SITE_DIR=$dir; return 0; fi ;; esac
  done <<<"$patterns"
  return 1
}
SITE_FILE=''
if find_site_dir; then
  SITE_FILE=$SITE_DIR/$SLUG.conf
else
  # No directory of sites to put it in: one include line, marked, right after `http {`.
  SITE_FILE=$CONF_DIR/$SLUG.conf
  MARK="# $SLUG: added by its install script"
  if ! grep -qF "$MARK" "$CONF"; then
    grep -Eq '^[[:space:]]*http[[:space:]]*\{' "$CONF" || fail "$(fmt "$M_e_no_http" "$CONF")"
    [ -f "$CONF.$SLUG.bak" ] || cp -p "$CONF" "$CONF.$SLUG.bak"
    cp -p "$CONF" "$WORK/nginx.conf.prev"
    undo "cp -p $(q "$WORK/nginx.conf.prev") $(q "$CONF")"
    awk -v line="    include \"$SITE_FILE\"; $MARK" '{ print } !done && /^[ \t]*http[ \t]*\{/ { print line; done = 1 }' "$CONF" >"$WORK/nginx.conf.new" || fail "$(fmt "$M_e_failed" "awk")"
    must cp "$WORK/nginx.conf.new" "$CONF"
  fi
fi
esc() { printf '%s' "$1" | sed -e 's/[\\&|]/\\&/g'; }
sed -e "s|@SERVER_NAME@|$(esc "$SERVER_NAME")|g" -e "s|@WEB_ROOT@|$(esc "$INSTALL_DIR/web")|g" -e "s|@CERT@|$(esc "$CERT")|g" -e "s|@KEY@|$(esc "$KEY")|g" "$WORK/site.tpl" >"$WORK/site.conf" || fail "$(fmt "$M_e_failed" "sed")"
if [ -f "$SITE_FILE" ]; then cp -p "$SITE_FILE" "$WORK/site.prev"; undo "cp -p $(q "$WORK/site.prev") $(q "$SITE_FILE")"; else undo "rm -f $(q "$SITE_FILE")"; fi
must cp "$WORK/site.conf" "$SITE_FILE"
must chmod 644 "$SITE_FILE"

step "$M_s_test"
"$NGINX_BIN" -t -c "$CONF" >"$WORK/nginx.out" 2>&1 || { cat "$WORK/nginx.out" >&2; fail "$M_e_nginx"; }

step "$M_s_reload"
reload_nginx() {
  if [ "$OS" = linux ] && systemctl is-active --quiet nginx 2>/dev/null; then systemctl reload nginx && return 0; fi
  "$NGINX_BIN" -c "$CONF" -s reload >/dev/null 2>&1
}
reload_nginx || warn "$M_w_nginx_stopped"

# ---- what the next run starts from ----
mkdir -p "$STATE_DIR"
{
  printf 'INSTALL_DIR=%s\n' "$INSTALL_DIR"
  printf 'SERVER_NAME=%s\n' "$SERVER_NAME"
  [ -z "$CERT" ] || printf 'CERT=%s\n' "$CERT"
  [ -z "$KEY" ] || printf 'KEY=%s\n' "$KEY"
} >"$STATE_DIR/install.conf"
rm -rf "$INSTALL_DIR/web.prev" "$INSTALL_DIR/bin.prev"

if command -v getenforce >/dev/null 2>&1 && [ "$(getenforce 2>/dev/null)" = Enforcing ]; then
  warn "$(fmt "$M_w_selinux" "chcon -R -t httpd_sys_content_t $INSTALL_DIR/web; setsebool -P httpd_can_network_connect 1")"
fi

if [ "$UPDATE" = 1 ]; then say "$M_done_update"; else say "$M_done"; fi
SCHEME=http; [ "$WSS" = 1 ] && SCHEME=https
fmt "$M_d_address\n" "$SCHEME://${SERVER_NAME%% *}/"
fmt "$M_d_installed\n" "$INSTALL_DIR"
if [ "$HAS_BACKEND" = 1 ]; then
  fmt "$M_d_service\n" "$SERVICE"
  fmt "$M_d_status\n" "$(service_status_hint)"
  fmt "$M_d_log\n" "tail -f $LOG_DIR/backend.log"
fi
exit 0
"##;

#[cfg(test)]
mod tests {
    use super::super::tests::sample;
    use super::super::{Lang, TargetOs};
    // The tests that run `bash` are for the systems the script is written for. On a Windows machine `bash` is the
    // launcher of the Windows Subsystem for Linux (or nothing), the scripts' text is the same on every machine, and
    // the Linux and macOS jobs of the build workflow run these; a skip line would fail that workflow, so on Windows
    // they are not compiled at all.
    #[cfg(unix)]
    use std::io::Write;
    #[cfg(unix)]
    use std::process::{Command, Stdio};

    #[cfg(unix)]
    fn bash_n(text: &str) -> Result<(), String> {
        let mut c = Command::new("bash").arg("-n").stdin(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
        c.stdin.take().unwrap().write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        let o = c.wait_with_output().map_err(|e| e.to_string())?;
        if o.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&o.stderr).into_owned())
        }
    }

    #[cfg(unix)]
    #[test]
    fn every_script_parses_in_every_language() {
        let started = std::time::Instant::now();
        let mut n = 0;
        for os in [TargetOs::Linux, TargetOs::MacOs] {
            for lang in Lang::ALL {
                for (wss, backend) in [(true, true), (false, true), (false, false)] {
                    let i = sample(os, lang, wss, backend);
                    let text = i.script();
                    bash_n(&text).unwrap_or_else(|e| panic!("{os:?} {lang:?} wss={wss} backend={backend}: {e}"));
                    assert!(text.starts_with("#!/usr/bin/env bash\n"));
                    assert!(!text.to_ascii_lowercase().contains("cobolt"), "a build-only crate name in the script");
                    assert!(text.contains(&format!("ZIP_SHA256='{}'", "0".repeat(64))));
                    n += 1;
                }
            }
        }
        println!("install.sh: {n} scripts (2 systems x 6 languages x 3 shapes) parse with bash -n in {} ms", started.elapsed().as_millis());
    }

    #[cfg(unix)]
    #[test]
    fn a_name_with_a_quote_cannot_break_out() {
        let mut i = sample(TargetOs::Linux, Lang::En, true, true);
        i.project = "Bob's \"shop\" $(touch x)";
        let text = i.script();
        bash_n(&text).unwrap();
        assert!(text.contains("PROJECT='Bob'\\''s \"shop\" $(touch x)'"), "quoted with the single-quote rule");
    }

    #[cfg(unix)]
    struct Ran {
        code: i32,
        out: String,
        err: String,
    }

    #[cfg(unix)]
    fn run(script: &std::path::Path, cwd: &std::path::Path, args: &[&str]) -> Ran {
        let o = Command::new("bash").arg(script).args(args).current_dir(cwd).stdin(Stdio::null()).output().expect("bash runs");
        Ran { code: o.status.code().unwrap_or(-1), out: String::from_utf8_lossy(&o.stdout).into_owned(), err: String::from_utf8_lossy(&o.stderr).into_owned() }
    }

    #[cfg(unix)]
    fn is_root() -> bool {
        Command::new("id").arg("-u").output().map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0").unwrap_or(false)
    }

    /// Every file under `dir` with its bytes, in order: what "changed nothing" is measured against.
    #[cfg(unix)]
    fn snapshot(dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
        fn walk(base: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(String, Vec<u8>)>) {
            let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
            entries.sort();
            for p in entries {
                let rel = p.strip_prefix(base).unwrap().display().to_string();
                if p.is_dir() {
                    out.push((format!("{rel}/"), Vec::new()));
                    walk(base, &p, out);
                } else {
                    out.push((rel, std::fs::read(&p).unwrap()));
                }
            }
        }
        let mut v = Vec::new();
        walk(dir, dir, &mut v);
        v
    }

    /// The parameter check comes before the root check, so these run here as an ordinary user and prove that a refusal touches nothing.
    #[cfg(unix)]
    #[test]
    fn the_refusals_that_change_nothing_run_for_real() {
        let started = std::time::Instant::now();
        let mut checks = 0;
        let mut ok = |what: &str, cond: bool| {
            assert!(cond, "{what}");
            checks += 1;
        };
        let tmp = tempfile::tempdir().unwrap();
        let w = tmp.path();
        let nginx = w.join("nginx");
        std::fs::create_dir_all(nginx.join("conf.d")).unwrap();
        std::fs::write(nginx.join("nginx.conf"), "events {}\nhttp {\n    include conf.d/*.conf;\n}\n").unwrap();
        let bare = w.join("bare");
        std::fs::create_dir(&bare).unwrap();
        let cert = w.join("site.pem");
        let key = w.join("site.key");
        std::fs::write(&cert, "x").unwrap();
        std::fs::write(&key, "y").unwrap();
        let env_ok = w.join("ok.env");
        std::fs::write(&env_ok, "CRM_TOKEN=a\nCOOKIE_KEY=b\n").unwrap();
        let env_short = w.join("short.env");
        std::fs::write(&env_short, "CRM_TOKEN=a\n").unwrap();
        let before = snapshot(w);

        for lang in Lang::ALL {
            let i = sample(TargetOs::Linux, lang, true, true);
            let script = w.join(i.script_name());
            std::fs::write(&script, i.script()).unwrap();
            let before_run = snapshot(w);
            let m = |key: &str, slot: &str| super::super::with_slots(super::super::msg(key, lang), |_| slot.to_owned());
            let (nx, cs, ks) = (nginx.to_str().unwrap(), cert.to_str().unwrap(), key.to_str().unwrap());

            let r = run(&script, w, &[]);
            ok("no parameter: exit 2", r.code == 2);
            ok("no parameter: says the nginx folder is required, in the language", r.err.contains(&m("e_no_nginx", "")));
            ok("no parameter: prints the usage with the parameter", r.err.contains("<nginx-folder>") && r.err.contains("--install-dir DIR"));
            let r = run(&script, w, &["--help"]);
            ok("--help: exit 0 and the usage on standard output", r.code == 0 && r.out.contains("<nginx-folder>"));
            let r = run(&script, w, &["/no/such/folder"]);
            ok("a folder that is not there: exit 2 naming it", r.code == 2 && r.err.contains(&m("e_bad_nginx", "/no/such/folder")));
            let r = run(&script, w, &[bare.to_str().unwrap()]);
            ok("a folder without nginx.conf: exit 2", r.code == 2 && r.err.contains(&m("e_bad_nginx", &bare.display().to_string())));
            let r = run(&script, w, &[nx, "--bogus"]);
            ok("an unknown parameter: exit 2 naming it", r.code == 2 && r.err.contains(&m("e_unknown", "--bogus")));
            let r = run(&script, w, &[nx, "--install-dir"]);
            ok("a parameter without its value: exit 2", r.code == 2 && r.err.contains(&m("e_value", "--install-dir")));
            let r = run(&script, w, &[nx, "--env-file", env_ok.to_str().unwrap()]);
            ok("wss:// without a certificate: exit 2", r.code == 2 && r.err.contains(&super::super::with_slots(super::super::msg("e_cert", lang), |n| ["--cert", "--key"][n].to_owned())));
            let r = run(&script, w, &[nx, "--cert", cs, "--key", ks]);
            ok("secrets required and no environment file: exit 2 naming them", r.code == 2 && r.err.contains("CRM_TOKEN, COOKIE_KEY"));
            let r = run(&script, w, &[nx, "--cert", cs, "--key", ks, "--env-file", env_short.to_str().unwrap()]);
            ok("an environment file that lacks one of them: exit 2", r.code == 2 && r.err.contains("CRM_TOKEN, COOKIE_KEY"));
            let r = run(&script, w, &[nx, "--cert", cs, "--key", ks, "--env-file", env_ok.to_str().unwrap(), "--install-dir", "/opt/my app"]);
            ok("an installation folder with a space: exit 2", r.code == 2 && r.err.contains(&m("e_path", "/opt/my app")));
            let r = run(&script, w, &[nx, "--cert", cs, "--key", ks, "--env-file", env_ok.to_str().unwrap(), "--install-dir", "relative/dir"]);
            ok("a relative installation folder: exit 2", r.code == 2);
            let r = run(&script, w, &[nx, "--cert", "/no/cert.pem", "--key", ks, "--env-file", env_ok.to_str().unwrap()]);
            ok("a certificate that is not a file: exit 2 naming it", r.code == 2 && r.err.contains(&m("e_file", "/no/cert.pem")));
            if !is_root() {
                let r = run(&script, w, &[nx, "--cert", cs, "--key", ks, "--env-file", env_ok.to_str().unwrap()]);
                ok("everything right, not root: stops at the root check (exit 1)", r.code == 1 && r.err.contains(&m("e_root", "")));
                ok("and says that it is checking before it stops", r.out.contains(&m("s_check", "")));
            }
            ok("none of the refusals changed anything the run could reach", snapshot(w) == before_run);
        }
        ok("nginx folder, certificate, key and environment files are as they were", snapshot(w).iter().filter(|(p, _)| !p.ends_with(".sh")).cloned().collect::<Vec<_>>() == before.iter().filter(|(p, _)| !p.ends_with(".sh")).cloned().collect::<Vec<_>>());
        println!("install.sh refusals: {checks} checks, 0 failed, 6 languages, run as {} in {} ms", if is_root() { "root (root-check case skipped)" } else { "an ordinary user" }, started.elapsed().as_millis());
    }

    /// The part of the script that decides which folder nginx already reads sites from, run on the layouts of the systems.
    #[cfg(unix)]
    #[test]
    fn the_site_goes_where_nginx_already_reads_sites_from() {
        let text = sample(TargetOs::Linux, Lang::En, false, true).script();
        let start = text.find("find_site_dir() {").expect("the function is in the script");
        let end = start + text[start..].find("\n}\n").unwrap() + 3;
        let function = &text[start..end];
        let decide = |layout: &str, dirs: &[&str]| -> Option<String> {
            let tmp = tempfile::tempdir().unwrap();
            let base = tmp.path().canonicalize().unwrap();
            for d in dirs {
                std::fs::create_dir_all(base.join(d)).unwrap();
            }
            let conf = base.join("nginx.conf");
            std::fs::write(&conf, layout.replace("@BASE@", base.to_str().unwrap())).unwrap();
            let o = Command::new("bash")
                .arg("-c")
                .arg(format!("CONF={c}; CONF_DIR={d}; SITE_DIR=''; {function}\nif find_site_dir; then printf '%s' \"$SITE_DIR\"; else printf 'NONE'; fi", c = conf.display(), d = base.display()))
                .output()
                .unwrap();
            let out = String::from_utf8_lossy(&o.stdout).into_owned();
            assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
            if out == "NONE" {
                None
            } else {
                Some(out.strip_prefix(base.to_str().unwrap()).unwrap_or(&out).trim_start_matches('/').to_owned())
            }
        };
        let debian = "load_module x;\ninclude @BASE@/modules-enabled/*.conf;\nevents {}\nhttp {\n    include @BASE@/mime.types;\n    include @BASE@/conf.d/*.conf;\n    include @BASE@/sites-enabled/*;\n}\n";
        let rhel = "events {}\nhttp {\n    include /etc/nginx/mime.types;\n    include @BASE@/conf.d/*.conf;\n}\n";
        let homebrew = "events {}\nhttp {\n    include mime.types;\n    include servers/*;\n}\n";
        let windows = "events {}\nhttp {\n    include mime.types;\n    server { listen 80; }\n}\n";
        let in_server = "events {}\nhttp {\n    server {\n        include snippets/*.conf;\n    }\n}\n";
        let commented = "events {}\nhttp {\n    # include conf.d/*.conf;\n}\n";
        let started = std::time::Instant::now();
        let cases: [(&str, Option<String>, Option<&str>); 8] = [
            ("Debian: conf.d before sites-enabled, modules-enabled ignored", decide(debian, &["modules-enabled", "conf.d", "sites-enabled"]), Some("conf.d")),
            ("Debian with only sites-enabled on disk", decide(debian, &["modules-enabled", "sites-enabled"]), Some("sites-enabled")),
            ("RHEL: absolute conf.d", decide(rhel, &["conf.d"]), Some("conf.d")),
            ("Homebrew: servers/*", decide(homebrew, &["servers"]), Some("servers")),
            ("nginx for Windows: no wildcard include", decide(windows, &["conf"]), None),
            ("an include inside a server block is not a folder of sites", decide(in_server, &["snippets"]), None),
            ("a commented include does not count", decide(commented, &["conf.d"]), None),
            ("the folder the include names is not there", decide(rhel, &[]), None),
        ];
        for (what, got, want) in &cases {
            assert_eq!(got.as_deref(), *want, "{what}");
        }
        println!("nginx layouts: {} cases, 0 failed, {} ms", cases.len(), started.elapsed().as_millis());
    }

    /// Found by running the script in a systemd container (spec 113, T16): the account step sat between two
    /// "installing the files" lines, so the step was said twice.
    #[test]
    fn each_step_is_said_once_in_the_order_it_runs() {
        for os in [TargetOs::Linux, TargetOs::MacOs] {
            for backend in [true, false] {
                let text = sample(os, Lang::En, true, backend).script();
                let order: Vec<usize> = ["M_s_extract", "M_s_account", "M_s_files", "M_s_service", "M_s_site", "M_s_test", "M_s_reload"]
                    .iter()
                    .filter_map(|key| {
                        let needle = format!("step \"${key}\"");
                        assert!(text.matches(&needle).count() <= 1, "{key} is said more than once ({os:?}, backend {backend})");
                        text.find(&needle)
                    })
                    .collect();
                assert!(order.windows(2).all(|w| w[0] < w[1]), "the steps are said in the order they run ({os:?}, backend {backend}): {order:?}");
            }
        }
    }

    #[test]
    fn the_script_asks_for_the_nginx_folder_and_nothing_else_is_mandatory() {
        let t = sample(TargetOs::Linux, Lang::En, false, true).script();
        assert!(t.contains("[ -n \"$NGINX\" ] || die_usage \"$M_e_no_nginx\""));
        assert!(t.contains("<nginx-folder> [options]") && t.contains("--install-dir DIR") && t.contains("--env-file FILE"));
        assert!(!t.contains("--cert FILE"), "no certificate asked for without wss://");
        let w = sample(TargetOs::Linux, Lang::Pt, true, true).script();
        assert!(w.contains("--cert FILE") && w.contains("--key FILE") && w.contains("obrigatório"));
    }
}
