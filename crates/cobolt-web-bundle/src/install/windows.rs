//! The install script for Windows (PowerShell 5.1 and 7): checks before it changes anything, undoes
//! what it did when a step fails, and can be run again to update. The Backend becomes a Windows
//! service that runs as its own virtual account (`NT SERVICE\<name>`), with no password to keep.

use super::{msg, ps_quote, usage_lines, with_slots, Install, KEYS};

pub(super) fn script(i: &Install) -> String {
    let b = i.backend.as_ref();
    let mut s = String::from('\u{feff}');
    s.push_str(&format!(
        "# {project}: installs the application on this computer (PowerRustCOBOL AI, Package).\n# It writes the web files, the Backend as a Windows service, and an nginx site; it needs this file's package ({zip}) beside it.\n#   powershell -ExecutionPolicy Bypass -File .\\{script} -NginxDir <nginx-folder>      (as Administrator)\n# Run it again to update: the records in data\\, the environment file and the logs are kept.\n#Requires -Version 5.1\n",
        project = i.project.replace(['\n', '\r'], " "),
        zip = i.zip_name(),
        script = i.script_name(),
    ));
    s.push_str("[CmdletBinding()]\nparam(\n    [Parameter(Position = 0)][string]$NginxDir = '',\n    [string]$InstallDir = '',\n    [string]$ServerName = '',\n    [string]$Cert = '',\n    [string]$Key = '',\n    [string]$EnvFile = '',\n    [switch]$Help,\n    [Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest\n)\n\n");
    s.push_str("$ErrorActionPreference = 'Stop'\ntry { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch { }\n\n");

    let var = |name: &str, value: &str| format!("${name} = {}\n", ps_quote(value));
    let flag = |name: &str, on: bool| format!("${name} = ${}\n", if on { "true" } else { "false" });
    s.push_str(&var("Project", &i.project.replace(['\n', '\r'], " ")));
    s.push_str(&var("Slug", &i.slug));
    s.push_str(&var("Service", &i.service()));
    s.push_str(&var("Zip", &i.zip_name()));
    s.push_str(&var("ZipSha256", &i.zip_sha256));
    s.push_str(&var("Exe", b.map(|b| b.exe.as_str()).unwrap_or("")));
    s.push_str(&var("ListenHost", &b.map(|b| b.addr.rsplit_once(':').map(|(h, _)| h.trim_matches(|c| c == '[' || c == ']').to_owned()).unwrap_or_else(|| "127.0.0.1".into())).unwrap_or_else(|| "127.0.0.1".into())));
    s.push_str(&format!("$Port = {}\n", b.map(|b| b.port).unwrap_or(0)));
    s.push_str(&flag("Wss", i.wss));
    s.push_str(&flag("HasBackend", b.is_some()));
    s.push_str(&flag("HasKb", b.is_some_and(|b| b.has_kb)));
    s.push_str(&var("ServerNameDefault", &i.server_name));
    s.push_str(&var("FolderName", &i.folder_name()));
    s.push_str(&format!("$Required = @({})\n", b.map(|b| b.required_secrets.iter().map(|n| ps_quote(n)).collect::<Vec<_>>().join(", ")).unwrap_or_default()));
    s.push_str("$InstallDefault = Join-Path $env:ProgramFiles $FolderName\n$StateDir = Join-Path $env:ProgramData $Slug\n$LogDir = Join-Path $StateDir 'logs'\n\n");

    s.push_str("$M = @{\n");
    for key in KEYS {
        s.push_str(&format!("    {} = {}\n", ps_quote(key), ps_quote(&with_slots(msg(key, i.lang), |n| format!("{{{n}}}")))));
    }
    s.push_str("}\n\n");

    let lines = usage_lines(
        i,
        |name| {
            let (flag, value) = match name {
                "install-dir" => ("-InstallDir", "DIR"),
                "server-name" => ("-ServerName", "NAME"),
                "cert" => ("-Cert", "FILE"),
                "key" => ("-Key", "FILE"),
                _ => ("-EnvFile", "FILE"),
            };
            format!("{flag} {value}")
        },
        "-NginxDir FOLDER",
    );
    s.push_str("$Usage = @'\n");
    s.push_str(&format!("{}: powershell -ExecutionPolicy Bypass -File .\\{} -NginxDir <nginx-folder> [options]\n\n", msg("usage", i.lang), i.script_name()));
    for (flag, text) in lines {
        s.push_str(&format!("  {flag:<26} {text}\n"));
    }
    s.push_str("'@\n\n");

    s.push_str(COMMON);
    s.push_str("$SiteTemplate = @'\n");
    s.push_str(&i.site_template());
    s.push_str("'@\n\n");
    s.push_str(MAIN);
    s
}

const COMMON: &str = r##"function Msg([string]$key, [object[]]$a) { if ($a) { return ($M[$key] -f $a) } else { return $M[$key] } }
function Say([string]$t) { Write-Host "==> $t" }
function Warn([string]$t) { Write-Host "!   $t" -ForegroundColor Yellow }
function Err([string]$t) { [Console]::Error.WriteLine("!! $t") }
function Die-Usage([string]$t) { Err $t; [Console]::Error.WriteLine($Usage); exit 2 }
function Abs-Path([string]$p) { return [System.IO.Path]::GetFullPath($p) }
function Fwd([string]$p) { return $p.Replace('\', '/') }
function Test-Port([string]$h, [int]$p) {
    $c = New-Object System.Net.Sockets.TcpClient
    try { $r = $c.BeginConnect($h, $p, $null, $null); return ($r.AsyncWaitHandle.WaitOne(500) -and $c.Connected) } catch { return $false } finally { $c.Close() }
}
# A program of the system run to the end, its two output streams read, its exit code returned: PowerShell 5.1
# turns a native program's standard error into errors unless this is how it is called.
function Run-Native([string]$exe, [string[]]$argv) {
    $old = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try { $out = & $exe @argv 2>&1 | ForEach-Object { "$_" }; $code = $LASTEXITCODE } finally { $ErrorActionPreference = $old }
    return [pscustomobject]@{ Code = $code; Output = ($out -join "`n") }
}
function Must-Native([string]$exe, [string[]]$argv) {
    $r = Run-Native $exe $argv
    if ($r.Code -ne 0) { throw ("{0} {1}: {2}" -f $exe, ($argv -join ' '), $r.Output) }
}

# What this run changed, undone in reverse order when a step fails.
$Undo = New-Object System.Collections.Generic.List[scriptblock]
function Add-Undo([scriptblock]$s) { $script:Undo.Add($s) }
function Invoke-Rollback {
    if ($script:Undo.Count -eq 0) { return }
    for ($n = $script:Undo.Count - 1; $n -ge 0; $n--) { $undo = $script:Undo[$n]; try { & $undo } catch { } }
    Say (Msg 's_restored')
}
$Step = ''
function Step([string]$t) { $script:Step = $t; Say $t }
"##;

const MAIN: &str = r##"# ---- the parameters (before anything is touched) ----
if ($Help) { [Console]::Out.WriteLine($Usage); exit 0 }
if ($Rest -and $Rest.Count -gt 0) { Die-Usage (Msg 'e_unknown' @($Rest[0])) }
if (-not $NginxDir) { Die-Usage (Msg 'e_no_nginx') }

# What the last run chose is the default of this one; a parameter wins.
$StateFile = Join-Path $StateDir 'install.conf'
if (Test-Path -LiteralPath $StateFile) {
    foreach ($l in (Get-Content -LiteralPath $StateFile)) {
        $k, $v = $l -split '=', 2
        switch ($k) {
            'INSTALL_DIR' { if (-not $InstallDir) { $InstallDir = $v } }
            'SERVER_NAME' { if (-not $ServerName) { $ServerName = $v } }
            'CERT' { if (-not $Cert) { $Cert = $v } }
            'KEY' { if (-not $Key) { $Key = $v } }
        }
    }
}
if (-not $InstallDir) { $InstallDir = $InstallDefault }
if (-not $ServerName) { $ServerName = $ServerNameDefault }

if (-not (Test-Path -LiteralPath $NginxDir -PathType Container)) { Die-Usage (Msg 'e_bad_nginx' @($NginxDir)) }
$NginxDir = (Resolve-Path -LiteralPath $NginxDir).Path.TrimEnd('\')
if (Test-Path -LiteralPath (Join-Path $NginxDir 'conf\nginx.conf')) {
    $Conf = Join-Path $NginxDir 'conf\nginx.conf'
    $Prefix = $NginxDir
} elseif (Test-Path -LiteralPath (Join-Path $NginxDir 'nginx.conf')) {
    $Conf = Join-Path $NginxDir 'nginx.conf'
    $Prefix = if ((Split-Path $NginxDir -Leaf) -eq 'conf') { Split-Path $NginxDir -Parent } else { $NginxDir }
} else {
    Die-Usage (Msg 'e_bad_nginx' @($NginxDir))
}
$ConfDir = Split-Path $Conf -Parent

if (-not [System.IO.Path]::IsPathRooted($InstallDir) -or $InstallDir -match '[\x00-\x1f"*?<>|]') { Die-Usage (Msg 'e_path' @($InstallDir)) }
$InstallDir = (Abs-Path $InstallDir).TrimEnd('\')
if ($ServerName -notmatch '^[A-Za-z0-9._* -]+$') { Die-Usage (Msg 'e_name' @($ServerName)) }

if ($Wss) {
    if (-not $Cert -or -not $Key) { Die-Usage (Msg 'e_cert' @('-Cert', '-Key')) }
    foreach ($f in @($Cert, $Key)) {
        if (-not (Test-Path -LiteralPath $f -PathType Leaf)) { Die-Usage (Msg 'e_file' @($f)) }
        if ($f -match '"') { Die-Usage (Msg 'e_path' @($f)) }
    }
    $Cert = Abs-Path $Cert
    $Key = Abs-Path $Key
}

$EnvDst = Join-Path $StateDir 'backend.env'
if ($HasBackend) {
    if ($EnvFile) {
        if (-not (Test-Path -LiteralPath $EnvFile -PathType Leaf)) { Die-Usage (Msg 'e_file' @($EnvFile)) }
        $EnvFile = Abs-Path $EnvFile
        $envText = [System.IO.File]::ReadAllText($EnvFile)
        foreach ($n in $Required) {
            if ($envText -notmatch ('(?m)^\s*' + [regex]::Escape($n) + '\s*=')) { Die-Usage (Msg 'e_secrets' @(($Required -join ', '))) }
        }
    } elseif (Test-Path -LiteralPath $EnvDst) {
        $EnvFile = ''
    } elseif ($Required.Count -gt 0) {
        Die-Usage (Msg 'e_secrets' @(($Required -join ', ')))
    }
}

# ---- the system ----
Say (Msg 's_check')
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { Err (Msg 'e_admin'); exit 1 }
$NginxExe = $null
foreach ($c in @((Join-Path $Prefix 'nginx.exe'), (Join-Path $NginxDir 'nginx.exe'))) { if (Test-Path -LiteralPath $c -PathType Leaf) { $NginxExe = $c; break } }
if (-not $NginxExe) { $g = Get-Command nginx.exe -ErrorAction SilentlyContinue; if ($g) { $NginxExe = $g.Source } }
if (-not $NginxExe) { Err (Msg 'e_tool' @('nginx.exe')); exit 1 }
$Here = Split-Path -Parent $MyInvocation.MyCommand.Path
$ZipPath = Join-Path $Here $Zip
if (-not (Test-Path -LiteralPath $ZipPath -PathType Leaf)) { Err (Msg 'e_zip' @($Zip)); exit 1 }
if ((Get-FileHash -Algorithm SHA256 -LiteralPath $ZipPath).Hash.ToLower() -ne $ZipSha256) { Err (Msg 'e_hash' @($Zip)); exit 1 }

$Update = $false
if ($HasBackend) {
    $Update = [bool](Get-Service -Name $Service -ErrorAction SilentlyContinue)
    if (-not $Update -and (Test-Port $ListenHost $Port)) { Err (Msg 'e_port' @($Port)); exit 1 }
} elseif (Test-Path -LiteralPath $StateFile) { $Update = $true }

$Work = Join-Path ([System.IO.Path]::GetTempPath()) ("$Slug-install-" + [guid]::NewGuid().ToString('N'))
$Utf8 = New-Object System.Text.UTF8Encoding($false)
try {
    Step (Msg 's_extract')
    New-Item -ItemType Directory -Path $Work | Out-Null
    Expand-Archive -LiteralPath $ZipPath -DestinationPath (Join-Path $Work 'pkg') -Force
    $Pkg = Join-Path $Work 'pkg'
    if (-not (Test-Path -LiteralPath (Join-Path $Pkg 'web\index.html'))) { throw (Msg 'e_file' @('web\index.html')) }
    if ($HasBackend) {
        if (-not (Test-Path -LiteralPath (Join-Path $Pkg "bin\$Exe"))) { throw (Msg 'e_file' @("bin\$Exe")) }
        if (-not (Test-Path -LiteralPath (Join-Path $Pkg 'backend.toml'))) { throw (Msg 'e_file' @('backend.toml')) }
    }

    # A folder of this package replaces the one of the last run; the old one is kept until the end.
    function Swap-Dir([string]$new, [string]$target) {
        $prev = "$target.prev"
        if (Test-Path -LiteralPath $prev) { Remove-Item -LiteralPath $prev -Recurse -Force }
        if (Test-Path -LiteralPath $target) {
            Rename-Item -LiteralPath $target -NewName (Split-Path $prev -Leaf)
            Add-Undo ({ if (Test-Path -LiteralPath $target) { Remove-Item -LiteralPath $target -Recurse -Force }; Rename-Item -LiteralPath $prev -NewName (Split-Path $target -Leaf) }.GetNewClosure())
        } else {
            Add-Undo ({ if (Test-Path -LiteralPath $target) { Remove-Item -LiteralPath $target -Recurse -Force } }.GetNewClosure())
        }
        Copy-Item -LiteralPath $new -Destination $target -Recurse
    }
    # A folder that holds the records of the application is only ever seeded: never replaced, never emptied.
    $Fresh = New-Object System.Collections.Generic.List[string]
    function Seed-Dir([string]$from, [string]$target) {
        if (Test-Path -LiteralPath $target) { return }
        $parent = Split-Path $target -Parent
        if (-not (Test-Path -LiteralPath $parent)) {
            New-Item -ItemType Directory -Path $parent | Out-Null
            Add-Undo ({ Remove-Item -LiteralPath $parent -Force -ErrorAction SilentlyContinue }.GetNewClosure())
        }
        if (Test-Path -LiteralPath $from) { Copy-Item -LiteralPath $from -Destination $target -Recurse } else { New-Item -ItemType Directory -Path $target | Out-Null }
        Add-Undo ({ Remove-Item -LiteralPath $target -Recurse -Force -ErrorAction SilentlyContinue }.GetNewClosure())
        $script:Fresh.Add($target)
    }

    # ---- the files ----
    Step (Msg 's_files')
    if (-not (Test-Path -LiteralPath $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        Add-Undo ({ Remove-Item -LiteralPath $InstallDir -Force -ErrorAction SilentlyContinue }.GetNewClosure())
    }
    $svc = $null
    if ($HasBackend) {
        $svc = Get-Service -Name $Service -ErrorAction SilentlyContinue
        if ($svc -and $svc.Status -ne 'Stopped') {
            Stop-Service -Name $Service -Force
            $svc.WaitForStatus('Stopped', [TimeSpan]::FromSeconds(60))
            Add-Undo ({ Start-Service -Name $Service -ErrorAction SilentlyContinue }.GetNewClosure())
        }
    }
    Swap-Dir (Join-Path $Pkg 'web') (Join-Path $InstallDir 'web')
    foreach ($f in @(Get-ChildItem -LiteralPath $Pkg -File | Where-Object { $_.Name -like 'LICENSE*' -or $_.Name -like 'NOTICE*' })) {
        $dst = Join-Path $InstallDir $f.Name
        Copy-Item -LiteralPath $f.FullName -Destination $dst -Force
        Add-Undo ({ Remove-Item -LiteralPath $dst -Force -ErrorAction SilentlyContinue }.GetNewClosure())
    }

    if ($HasBackend) {
        Swap-Dir (Join-Path $Pkg 'bin') (Join-Path $InstallDir 'bin')

        $toml = Join-Path $InstallDir 'backend.toml'
        $newToml = Join-Path $Pkg 'backend.toml'
        if (Test-Path -LiteralPath $toml) {
            $tomlPrev = Join-Path $Work 'toml.prev'
            Copy-Item -LiteralPath $toml -Destination $tomlPrev -Force
            Add-Undo ({ Copy-Item -LiteralPath $tomlPrev -Destination $toml -Force }.GetNewClosure())
            if ((Get-FileHash -LiteralPath $toml).Hash -ne (Get-FileHash -LiteralPath $newToml).Hash) {
                $bak = "$toml." + (Get-Date -Format 'yyyyMMddHHmmss') + '.bak'
                Copy-Item -LiteralPath $toml -Destination $bak
                Warn (Msg 'w_toml' @($bak))
            }
        } else {
            Add-Undo ({ Remove-Item -LiteralPath $toml -Force -ErrorAction SilentlyContinue }.GetNewClosure())
        }
        Copy-Item -LiteralPath $newToml -Destination $toml -Force

        Seed-Dir (Join-Path $Pkg 'data') (Join-Path $InstallDir 'data')
        if ($HasKb) { Seed-Dir (Join-Path $Pkg 'assets\KB') (Join-Path $InstallDir 'assets\KB') }

        foreach ($d in @($StateDir, $LogDir)) {
            if (-not (Test-Path -LiteralPath $d)) {
                New-Item -ItemType Directory -Path $d -Force | Out-Null
                Add-Undo ({ Remove-Item -LiteralPath $d -Recurse -Force -ErrorAction SilentlyContinue }.GetNewClosure())
                # What is made in the state folder is born readable by SYSTEM and the administrators only: the secrets are never open, not even for a moment.
                if ($d -eq $StateDir) { Must-Native 'icacls.exe' @($d, '/inheritance:r', '/grant:r', '*S-1-5-18:(OI)(CI)(F)', '*S-1-5-32-544:(OI)(CI)(F)') }
            }
        }
        # The secrets: a file only the service's account, SYSTEM and the administrators can read; one the operator gives replaces the last.
        if (Test-Path -LiteralPath $EnvDst) {
            $envPrev = Join-Path $Work 'env.prev'
            Copy-Item -LiteralPath $EnvDst -Destination $envPrev -Force
            Add-Undo ({ Copy-Item -LiteralPath $envPrev -Destination $EnvDst -Force }.GetNewClosure())
        } else {
            Add-Undo ({ Remove-Item -LiteralPath $EnvDst -Force -ErrorAction SilentlyContinue }.GetNewClosure())
        }
        if ($EnvFile) { Copy-Item -LiteralPath $EnvFile -Destination $EnvDst -Force }
        elseif (-not (Test-Path -LiteralPath $EnvDst)) { [System.IO.File]::WriteAllText($EnvDst, '', $Utf8) }

        # ---- the service ----
        Step (Msg 's_service')
        $exePath = Join-Path $InstallDir "bin\$Exe"
        $binPath = '"{0}" --service --service-name {1} --config "{2}" --env-file "{3}" --log-file "{4}"' -f $exePath, $Service, (Join-Path $InstallDir 'backend.toml'), $EnvDst, (Join-Path $LogDir 'backend.log')
        if (-not $svc) {
            New-Service -Name $Service -BinaryPathName $binPath -DisplayName $Project -StartupType Automatic | Out-Null
            Add-Undo ({ Stop-Service -Name $Service -Force -ErrorAction SilentlyContinue; Run-Native 'sc.exe' @('delete', $Service) | Out-Null }.GetNewClosure())
        } else {
            $cim = Get-CimInstance -ClassName Win32_Service -Filter ("Name='{0}'" -f $Service)
            $oldPath = $cim.PathName
            Add-Undo ({ Get-CimInstance -ClassName Win32_Service -Filter ("Name='{0}'" -f $Service) | Invoke-CimMethod -MethodName Change -Arguments @{ PathName = $oldPath } | Out-Null }.GetNewClosure())
            $cim | Invoke-CimMethod -MethodName Change -Arguments @{ PathName = $binPath } | Out-Null
        }
        $account = "NT SERVICE\$Service"
        Must-Native 'sc.exe' @('config', $Service, 'obj=', $account)
        Must-Native 'sc.exe' @('sidtype', $Service, 'unrestricted')
        Must-Native 'sc.exe' @('failure', $Service, 'reset=', '86400', 'actions=', 'restart/5000/restart/10000/restart/30000')
        Must-Native 'sc.exe' @('description', $Service, ($Project -replace '"', ''))
        # Only the service's account may write where the application keeps its records, and only it, SYSTEM and the administrators read the secrets.
        foreach ($d in @((Join-Path $InstallDir 'data'), (Join-Path $InstallDir 'assets\KB'), $LogDir)) {
            if (Test-Path -LiteralPath $d) {
                $a = @($d, '/grant', ('{0}:(OI)(CI)M' -f $account))
                if ($Fresh -contains $d) { $a += '/T' }
                Must-Native 'icacls.exe' $a
            }
        }
        Must-Native 'icacls.exe' @($StateDir, '/grant', ('{0}:(R)' -f $account))
        Must-Native 'icacls.exe' @($EnvDst, '/inheritance:r', '/grant:r', '*S-1-5-18:(F)', '*S-1-5-32-544:(F)', ('{0}:(R)' -f $account))

        try { Start-Service -Name $Service } catch { throw (Msg 'e_start' @((Join-Path $LogDir 'backend.log'))) }
        Step (Msg 's_wait' @($Port))
        $waited = 0
        while (-not (Test-Port $ListenHost $Port)) {
            $waited++
            if ($waited -gt 60) { throw (Msg 'e_start' @((Join-Path $LogDir 'backend.log'))) }
            Start-Sleep -Seconds 1
        }
    }

    # ---- nginx ----
    Step (Msg 's_site')
    # The include lines of the http block that name a folder with a wildcard: the folder nginx already reads sites from.
    function Find-SiteDir {
        $inHttp = $false
        $depth = 0
        foreach ($raw in (Get-Content -LiteralPath $Conf)) {
            $line = $raw -replace '#.*$', ''
            if (-not $inHttp) {
                if ($line -match '^\s*http\s*\{') { $inHttp = $true; $depth = 1 }
                continue
            }
            if ($depth -eq 1 -and $line -match '^\s*include\s+([^;]+?)\s*;') {
                $pat = $Matches[1].Trim('"').Replace('/', '\')
                if ($pat -match '\*') {
                    if (-not [System.IO.Path]::IsPathRooted($pat)) { $pat = Join-Path $ConfDir $pat }
                    $leaf = Split-Path $pat -Leaf
                    $dir = Split-Path $pat -Parent
                    if (($leaf -eq '*.conf' -or $leaf -eq '*') -and (Test-Path -LiteralPath $dir -PathType Container)) { return $dir }
                }
            }
            $depth += ([regex]::Matches($line, '\{')).Count - ([regex]::Matches($line, '\}')).Count
            if ($depth -le 0) { $inHttp = $false }
        }
        return $null
    }
    $SiteDir = Find-SiteDir
    if ($SiteDir) {
        $SiteFile = Join-Path $SiteDir "$Slug.conf"
    } else {
        # No folder of sites to put it in: one include line, marked, right after `http {`.
        $SiteFile = Join-Path $ConfDir "$Slug.conf"
        $mark = "# ${Slug}: added by its install script"
        $confText = [System.IO.File]::ReadAllText($Conf)
        if (-not $confText.Contains($mark)) {
            if ($confText -notmatch '(?m)^[ \t]*http[ \t]*\{') { throw (Msg 'e_no_http' @($Conf)) }
            $confBak = "$Conf.$Slug.bak"
            if (-not (Test-Path -LiteralPath $confBak)) { Copy-Item -LiteralPath $Conf -Destination $confBak }
            $confPrev = Join-Path $Work 'nginx.conf.prev'
            Copy-Item -LiteralPath $Conf -Destination $confPrev -Force
            Add-Undo ({ Copy-Item -LiteralPath $confPrev -Destination $Conf -Force }.GetNewClosure())
            $inc = ('    include "' + (Fwd $SiteFile) + '"; ' + $mark).Replace('$', '$$')
            $re = New-Object System.Text.RegularExpressions.Regex('(?m)^([ \t]*http[ \t]*\{[^\r\n]*)(\r?\n)')
            $confNew = $re.Replace($confText, ('${1}${2}' + $inc + '${2}'), 1)
            [System.IO.File]::WriteAllText($Conf, $confNew, $Utf8)
        }
    }
    $site = $SiteTemplate.Replace('@SERVER_NAME@', $ServerName).Replace('@WEB_ROOT@', (Fwd (Join-Path $InstallDir 'web'))).Replace('@CERT@', (Fwd $Cert)).Replace('@KEY@', (Fwd $Key))
    if (Test-Path -LiteralPath $SiteFile) {
        $sitePrev = Join-Path $Work 'site.prev'
        Copy-Item -LiteralPath $SiteFile -Destination $sitePrev -Force
        Add-Undo ({ Copy-Item -LiteralPath $sitePrev -Destination $SiteFile -Force }.GetNewClosure())
    } else {
        Add-Undo ({ Remove-Item -LiteralPath $SiteFile -Force -ErrorAction SilentlyContinue }.GetNewClosure())
    }
    [System.IO.File]::WriteAllText($SiteFile, $site, $Utf8)

    Step (Msg 's_test')
    $nginxArgs = @('-p', (Fwd $Prefix), '-c', (Fwd $Conf))
    $t = Run-Native $NginxExe (@('-t') + $nginxArgs)
    if ($t.Code -ne 0) { Err $t.Output; throw (Msg 'e_nginx') }

    Step (Msg 's_reload')
    $r = Run-Native $NginxExe ($nginxArgs + @('-s', 'reload'))
    if ($r.Code -ne 0) { Warn (Msg 'w_nginx_stopped') }

    # ---- what the next run starts from ----
    if (-not (Test-Path -LiteralPath $StateDir)) { New-Item -ItemType Directory -Path $StateDir -Force | Out-Null }
    $state = @("INSTALL_DIR=$InstallDir", "SERVER_NAME=$ServerName")
    if ($Cert) { $state += "CERT=$Cert" }
    if ($Key) { $state += "KEY=$Key" }
    [System.IO.File]::WriteAllText($StateFile, (($state -join "`r`n") + "`r`n"), $Utf8)
    foreach ($p in @((Join-Path $InstallDir 'web.prev'), (Join-Path $InstallDir 'bin.prev'))) { if (Test-Path -LiteralPath $p) { Remove-Item -LiteralPath $p -Recurse -Force -ErrorAction SilentlyContinue } }

    if ($Update) { Say (Msg 'done_update') } else { Say (Msg 'done') }
    $scheme = if ($Wss) { 'https' } else { 'http' }
    Write-Host (Msg 'd_address' @(($scheme + '://' + ($ServerName -split ' ')[0] + '/')))
    Write-Host (Msg 'd_installed' @($InstallDir))
    if ($HasBackend) {
        Write-Host (Msg 'd_service' @($Service))
        Write-Host (Msg 'd_status' @("Get-Service $Service"))
        Write-Host (Msg 'd_log' @("Get-Content -Wait '" + (Join-Path $LogDir 'backend.log') + "'"))
    }
} catch {
    Err (Msg 'e_failed' @($Step))
    Err $_.Exception.Message
    Invoke-Rollback
    exit 1
} finally {
    if (Test-Path -LiteralPath $Work) { Remove-Item -LiteralPath $Work -Recurse -Force -ErrorAction SilentlyContinue }
}
exit 0
"##;

#[cfg(test)]
mod tests {
    use super::super::tests::sample;
    use super::super::{Lang, TargetOs};

    /// The script without its messages, its usage text and its header comments (the project's name is there): what is left is ASCII.
    fn non_ascii_only_in_words(text: &str) -> bool {
        let mut skip_until: Option<&str> = None;
        for line in text.trim_start_matches('\u{feff}').lines() {
            if let Some(end) = skip_until {
                if line == end {
                    skip_until = None;
                }
                continue;
            }
            match line {
                "$M = @{" => skip_until = Some("}"),
                "$Usage = @'" => skip_until = Some("'@"),
                l if l.starts_with('#') && !l.starts_with("#Requires") => {}
                l => {
                    if !l.is_ascii() {
                        return false;
                    }
                }
            }
        }
        true
    }

    #[test]
    fn the_windows_script_has_what_the_system_needs() {
        let started = std::time::Instant::now();
        let mut n = 0;
        for lang in Lang::ALL {
            for (wss, backend) in [(true, true), (false, true), (false, false)] {
                let i = sample(TargetOs::Windows, lang, wss, backend);
                let t = i.script();
                assert!(t.starts_with('\u{feff}'), "a byte-order mark, so PowerShell 5.1 reads the messages as UTF-8");
                assert!(t.contains("[CmdletBinding()]") && t.contains("#Requires -Version 5.1"));
                assert!(t.contains("[Parameter(Position = 0)][string]$NginxDir = ''"), "the nginx folder is the first parameter");
                assert!(t.contains("if (-not $NginxDir) { Die-Usage (Msg 'e_no_nginx') }"), "and the only one that is required");
                assert!(!t.to_ascii_lowercase().contains("cobolt"), "a build-only crate name in the script");
                assert!(t.contains(&format!("$ZipSha256 = '{}'", "0".repeat(64))));
                assert!(non_ascii_only_in_words(&t), "{lang:?}: a non-ASCII character outside the messages");
                assert_eq!(t.matches("$SiteTemplate = @'\n").count(), 1);
                assert!(t.contains("@SERVER_NAME@") && t.contains("@WEB_ROOT@"));
                // Every `@'` here-string closes at the start of a line.
                assert_eq!(t.matches("@'\n").count(), t.matches("\n'@\n").count(), "unbalanced here-strings");
                if backend {
                    for needle in ["New-Service -Name $Service", "'sidtype', $Service, 'unrestricted'", "'actions=', 'restart/5000/restart/10000/restart/30000'", "'*S-1-5-18:(F)', '*S-1-5-32-544:(F)'", "NT SERVICE\\$Service", "--service --service-name"] {
                        assert!(t.contains(needle), "{needle}");
                    }
                }
                n += 1;
            }
        }
        println!("install.ps1: {n} scripts (6 languages x 3 shapes) checked as text in {} ms; not parsed by PowerShell here (no pwsh on this machine)", started.elapsed().as_millis());
    }

    #[test]
    fn a_name_with_quotes_stays_inside_its_string() {
        let mut i = sample(TargetOs::Windows, Lang::En, true, true);
        i.project = "Bob's \"shop\" $(calc)";
        let t = i.script();
        assert!(t.contains("$Project = 'Bob''s \"shop\" $(calc)'"), "a single quote is doubled and nothing is interpolated");
        assert_eq!(i.folder_name(), "Bob's -shop- $(calc)");
        assert!(t.contains("$FolderName = 'Bob''s -shop- $(calc)'"));
    }

    #[test]
    fn the_folder_name_is_one_a_path_can_hold() {
        let mut i = sample(TargetOs::Windows, Lang::En, true, true);
        for (project, want) in [("Power Oficina", "Power Oficina"), ("a/b:c", "a-b-c"), ("  trailing. ", "trailing"), ("***", "power-oficina")] {
            i.project = project;
            assert_eq!(i.folder_name(), want, "{project}");
        }
    }
}
