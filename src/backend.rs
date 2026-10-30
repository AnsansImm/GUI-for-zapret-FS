use std::os::windows::process::CommandExt;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
const HIDDEN: u32 = 0x08000000;
pub fn command(program: &str) -> Command {
    let mut c = Command::new(program);
    c.creation_flags(HIDDEN);
    c
}
fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let o = command(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    let s = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    if o.status.success() {
        Ok(s)
    } else {
        Err(format!("{program}: {s}"))
    }
}
pub fn status() -> String {
    let mut s = String::new();
    for name in ["zapret", "WinDivert"] {
        s.push_str(&run("sc.exe", &["query", name]).unwrap_or_else(|e| e));
        s.push('\n');
    }
    s
}
pub fn strategies(root: &Path) -> Vec<String> {
    let mut v: Vec<_> = fs::read_dir(root)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| {
            let p = e.path();
            let n = p.file_name()?.to_str()?.to_owned();
            (p.is_file() && n.ends_with(".bat") && !n.starts_with("service")).then_some(n)
        })
        .collect();
    v.sort();
    v
}
pub fn strategy_args(root: &Path, name: &str) -> Result<String, String> {
    if !strategies(root).iter().any(|n| n == name) {
        return Err("Стратегия не найдена".into());
    }
    let text = fs::read_to_string(root.join(name)).map_err(|e| e.to_string())?;
    let (_, tail) = text
        .split_once("\"%BIN%winws.exe\"")
        .ok_or("Неизвестный формат стратегии")?;
    let game = fs::read_to_string(root.join("utils/game_filter.enabled")).unwrap_or_default();
    let get = |key: &str| {
        game.lines()
            .find_map(|l| l.strip_prefix(&format!("{key}=")))
            .unwrap_or("")
            .trim()
    };
    let mode = get("mode");
    let ports = |key: &str| {
        let p = get(key);
        if p.is_empty() {
            "1024-65535"
        } else {
            p
        }
    };
    let tcp = if mode == "all" || mode == "tcp" {
        ports("tcp")
    } else {
        "12"
    };
    let udp = if mode == "all" || mode == "udp" {
        ports("udp")
    } else {
        "12"
    };
    for p in [tcp, udp] {
        if !valid_ports(p) {
            return Err("Неверный диапазон портов".into());
        }
    }
    let mut a = tail
        .lines()
        .map(|l| l.trim().trim_end_matches('^').trim())
        .collect::<Vec<_>>()
        .join(" ");
    for (k, v) in [
        ("%BIN%", format!("{}\\", root.join("bin").display())),
        ("%LISTS%", format!("{}\\", root.join("lists").display())),
        ("%GameFilterTCP%", tcp.into()),
        ("%GameFilterUDP%", udp.into()),
    ] {
        a = a.replace(k, &v)
    }
    if a.contains(['%', '\n', '\r', '&', '|', '<', '>']) {
        return Err("Неподдерживаемые параметры стратегии".into());
    }
    Ok(a.trim().into())
}
pub fn valid_ports(s: &str) -> bool {
    !s.is_empty()
        && s.split(',').all(|x| {
            let v: Vec<_> = x.split('-').collect();
            if v.len() > 2 {
                return false;
            }
            let nums: Option<Vec<u16>> = v
                .iter()
                .map(|p| p.parse::<u16>().ok().filter(|n| *n > 0))
                .collect();
            nums.is_some_and(|n| n.len() == 1 || n[0] <= n[1])
        })
}
pub fn helper(action: &str, root: &Path, strategy: &str) -> Result<String, String> {
    if matches!(action, "start" | "install") {
        let state = snapshot()?;
        if state.winws && !state.service_running {
            return Err("Zapret уже запущен отдельно. Завершите отдельный winws перед запуском или перенастройкой службы.".into());
        }
    }
    match action {
        "stop_standalone" => {
            let exe = root
                .join("bin/winws.exe")
                .to_string_lossy()
                .replace('\'', "''");
            let script = format!(
                r#"$ErrorActionPreference='Stop';$expected=[IO.Path]::GetFullPath('{exe}');$services=@(Get-CimInstance Win32_Service -Filter 'ProcessId > 0' -OperationTimeoutSec 8|Select-Object -ExpandProperty ProcessId);$targets=@(Get-CimInstance Win32_Process -Filter "Name='winws.exe'" -OperationTimeoutSec 8|Where-Object{{$_.ExecutablePath -and [IO.Path]::GetFullPath($_.ExecutablePath).Equals($expected,[StringComparison]::OrdinalIgnoreCase) -and $_.ProcessId -notin $services}});foreach($p in $targets){{$current=Get-CimInstance Win32_Process -Filter ('ProcessId='+$p.ProcessId) -OperationTimeoutSec 8;if($current -and $current.CreationDate -eq $p.CreationDate){{$process=Get-Process -Id $p.ProcessId -ErrorAction Stop;$process.Kill();if(-not $process.WaitForExit(10000)){{throw 'winws did not stop'}}}}}};[Console]::OutputEncoding=New-Object Text.UTF8Encoding($false);Write-Output ('Остановлено отдельных процессов Zapret: '+$targets.Count)"#
            );
            run(
                "powershell.exe",
                &["-NoProfile", "-NonInteractive", "-Command", &script],
            )
        }
        "fix_bfe" => run("sc.exe", &["start", "BFE"]),
        "fix_timestamps" => run(
            "netsh.exe",
            &["interface", "tcp", "set", "global", "timestamps=enabled"],
        ),
        "start" => run("sc.exe", &["start", "zapret"]),
        "stop" => run("sc.exe", &["stop", "zapret"]),
        "install" => {
            let args = strategy_args(root, strategy)?;
            if !root.join("bin/winws.exe").is_file() {
                return Err("Не найден bin/winws.exe".into());
            }
            for (name, contents) in [
                (
                    "list-general-user.txt",
                    "# Never leave this file empty\ndomain.example.abc\n",
                ),
                ("list-exclude-user.txt", "domain.example.abc\n"),
                ("ipset-exclude-user.txt", "203.0.113.113/32\n"),
            ] {
                use std::io::Write;
                match fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(root.join("lists").join(name))
                {
                    Ok(mut f) => f
                        .write_all(contents.as_bytes())
                        .map_err(|e| e.to_string())?,
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                    Err(e) => return Err(e.to_string()),
                }
            }
            // Never replace a live service: stop first and wait for Windows to finish.
            let _ = run("sc.exe", &["stop", "zapret"]);
            for _ in 0..30 {
                let q = run("sc.exe", &["query", "zapret"]).unwrap_or_default();
                if !q.contains("RUNNING") && !q.contains("STOP_PENDING") {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            let exists = run("sc.exe", &["query", "zapret"]).is_ok();
            let state = run("sc.exe", &["query", "zapret"]).unwrap_or_default();
            if state.contains("RUNNING") || state.contains("STOP_PENDING") {
                return Err("Служба не остановилась. Применение отменено.".into());
            }
            let bin = format!("\"{}\" {}", root.join("bin/winws.exe").display(), args);
            let result = run(
                "sc.exe",
                &[
                    if exists { "config" } else { "create" },
                    "zapret",
                    "binPath=",
                    &bin,
                    "start=",
                    "auto",
                    "DisplayName=",
                    "zapret",
                ],
            )?;
            run(
                "reg.exe",
                &[
                    "add",
                    r"HKLM\System\CurrentControlSet\Services\zapret",
                    "/v",
                    "zapret-discord-youtube",
                    "/t",
                    "REG_SZ",
                    "/d",
                    strategy.trim_end_matches(".bat"),
                    "/f",
                ],
            )?;
            run(
                "netsh.exe",
                &["interface", "tcp", "set", "global", "timestamps=enabled"],
            )?;
            Ok(format!(
                "{result}\n{}",
                run("sc.exe", &["start", "zapret"])?
            ))
        }
        "remove" => {
            let _ = run("sc.exe", &["stop", "zapret"]);
            run("sc.exe", &["delete", "zapret"])
        }
        _ => Err("Неизвестная операция".into()),
    }
}
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
pub fn elevated(action: String, root: PathBuf, strategy: String) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let output = std::env::temp_dir().join(format!(
        "zapret-gui-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let argv = [
        "--helper".into(),
        action,
        root.to_string_lossy().into_owned(),
        strategy,
        output.to_string_lossy().into_owned(),
    ]
    .iter()
    .map(|s| format!("\"{}\"", s))
    .collect::<Vec<_>>()
    .join(" ");
    let script=format!("$ErrorActionPreference='Stop'; $p=Start-Process -FilePath {} -ArgumentList {} -Verb RunAs -Wait -PassThru; exit $p.ExitCode",quote(&exe.to_string_lossy()),quote(&argv));
    let result = run("powershell.exe", &["-NoProfile", "-Command", &script]);
    let data = fs::read_to_string(&output);
    let _ = fs::remove_file(output);
    match data {
        Ok(d) => serde_json::from_str(&d).map_err(|e| e.to_string())?,
        Err(_) => Err(result
            .err()
            .unwrap_or("Помощник не вернул результат".into())),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ports() {
        for p in ["443", "1024-65535", "80,443,1000-2000"] {
            assert!(valid_ports(p))
        }
        for p in ["0", "65536", "90-80", "1-2-3", "", "1 & whoami"] {
            assert!(!valid_ports(p))
        }
    }
    #[test]
    fn shipped_strategies() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("zapret-discord-youtube-1.10.4");
        if root.exists() {
            let all = strategies(&root);
            assert!(!all.is_empty());
            for n in all {
                assert!(strategy_args(&root, &n).is_ok(), "{n}")
            }
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Check {
    pub level: String,
    pub title: String,
    pub detail: String,
}
pub fn diagnostics(root: &Path) -> Vec<Check> {
    let mut checks = Vec::new();
    for (file, title) in [
        ("service.bat", "Менеджер сборки"),
        ("bin/winws.exe", "Исполняемый файл Zapret"),
        ("bin/WinDivert64.sys", "Драйвер WinDivert"),
        ("lists/list-general-user.txt", "Пользовательский список"),
        ("lists/list-exclude-user.txt", "Пользовательские исключения"),
    ] {
        let present = root.join(file).is_file();
        checks.push(Check {
            level: if present { "ok" } else { "warn" }.into(),
            title: title.into(),
            detail: format!(
                "{} — {}",
                file,
                if present {
                    "найден"
                } else {
                    "не найден"
                }
            ),
        });
    }
    let script = r#"
$ErrorActionPreference = 'Stop'
$rows = @()
function Add-Check($level, $title, $detail) {
    $script:rows += [pscustomobject]@{level=$level;title=$title;detail=$detail}
}
foreach ($item in @(@('zapret','Служба Zapret'),@('WinDivert','Служба WinDivert'),@('BFE','Базовая фильтрация Windows'))) {
    try {
        $s = Get-Service -Name $item[0] -ErrorAction Stop
        $level = if ($s.Status -eq 'Running') {'ok'} else {'warn'}
        Add-Check $level $item[1] ($s.Status.ToString())
    } catch {
        Add-Check 'warn' $item[1] 'Не найдена или недоступна для проверки'
    }
}
try {
    $global = @(& netsh.exe interface tcp show global 2>&1)
    $exitCode = $LASTEXITCODE
    $line = $global | Where-Object { [string]$_ -match '(?i)timestamps|RFC\s*1323|временн.*мет|мет.*времен' } | Select-Object -First 1
    if ($exitCode -eq 0 -and $line) {
        $value = ([string]$line -split ':',2)[1].Trim()
        if ($value -match '^(?i:enabled|включено|включен|включён)$') {
            Add-Check 'ok' 'TCP timestamps' 'Глобальный параметр включён. Стратегии с fooling=ts могут его использовать.'
        } elseif ($value -match '^(?i:disabled|отключено|отключен|отключён)$') {
            Add-Check 'warn' 'TCP timestamps' 'Глобальный параметр отключён. Для стратегий с fooling=ts его нужно включить.'
        } else {
            Add-Check 'info' 'TCP timestamps' ('Глобальное состояние: '+$value)
        }
    } else {
        # Read raw CIM values: the Automatic profile has null here, and the
        # Get-NetTCPSetting enum adapter throws when converting that null.
        $settings = @(Get-CimInstance -Namespace root/StandardCimv2 -ClassName MSFT_NetTCPSetting -OperationTimeoutSec 8 -ErrorAction Stop)
        $profiles = @($settings | Where-Object { $null -ne $_.Timestamps } | ForEach-Object {
            $value = switch ([int]$_.Timestamps) { 0 {'отключено'} 1 {'включено'} default {[string]$_.Timestamps} }
            $_.SettingName+': '+$value
        })
        if ($profiles.Count -eq 0) { throw 'Windows не вернула значение timestamps' }
        Add-Check 'info' 'TCP timestamps' ('Профили TCP: '+($profiles -join '; ')+'. Глобальное состояние netsh не распознано.')
    }
} catch { Add-Check 'unknown' 'TCP timestamps' ('Не удалось прочитать состояние: '+$_.Exception.Message) }
try {
    $p = @(Get-Process -Name winws -ErrorAction SilentlyContinue)
    if ($p.Count -gt 0) { Add-Check 'ok' 'Процесс winws' ('Запущен; количество: '+$p.Count) }
    else { Add-Check 'warn' 'Процесс winws' 'Не запущен' }
    if (Get-Process -Name AdguardSvc -ErrorAction SilentlyContinue) {
        Add-Check 'warn' 'AdGuard' 'Обнаружен AdguardSvc. Возможен конфликт с Discord; это не означает, что конфликт подтверждён.'
    } else { Add-Check 'ok' 'AdGuard' 'Процесс AdguardSvc не обнаружен' }
} catch { Add-Check 'unknown' 'Процессы' $_.Exception.Message }
try {
    $all = @(Get-Service -ErrorAction Stop)
    $conflicts = @($all | Where-Object { $_.Name -match 'Killer|TracSrvWrapper|EPWD|SmartByte|VPN|HappService' -or $_.DisplayName -match 'Intel.*Connectivity.*Network' })
    if ($conflicts.Count -gt 0) {
        Add-Check 'warn' 'Возможные конфликты' (($conflicts | ForEach-Object { $_.DisplayName+' ['+$_.Status+']' }) -join '; ')
    } else { Add-Check 'ok' 'Возможные конфликты' 'Известные имена конфликтующих служб не обнаружены. Проверка не гарантирует отсутствие конфликтов.' }
} catch { Add-Check 'unknown' 'Возможные конфликты' $_.Exception.Message }
try {
    $proxy = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
    if ($proxy.ProxyEnable -eq 1) { Add-Check 'warn' 'Системный прокси' ('Включён: '+$proxy.ProxyServer) }
    else { Add-Check 'ok' 'Системный прокси' 'Прокси текущего пользователя отключён' }
} catch { Add-Check 'unknown' 'Системный прокси' $_.Exception.Message }
try {
    $hosts = Join-Path $env:SystemRoot 'System32\drivers\etc\hosts'
    $found = @(Get-Content -LiteralPath $hosts | Where-Object { $_ -notmatch '^\s*#' -and $_ -match '(?i)(^|\s)(\S+\.)?(youtube\.com|youtu\.be)(\s|$)' })
    if ($found.Count -gt 0) { Add-Check 'warn' 'Файл hosts' 'Найдены активные записи YouTube. Они могут влиять на доступность сайта.' }
    else { Add-Check 'ok' 'Файл hosts' 'Активные записи YouTube не обнаружены' }
} catch { Add-Check 'unknown' 'Файл hosts' $_.Exception.Message }
try {
    $strategy = Get-ItemPropertyValue 'HKLM:\SYSTEM\CurrentControlSet\Services\zapret' -Name 'zapret-discord-youtube' -ErrorAction Stop
    Add-Check 'info' 'Установленная стратегия' $strategy
} catch { Add-Check 'info' 'Установленная стратегия' 'Имя стратегии в реестре не найдено или недоступно' }
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
ConvertTo-Json -InputObject @($rows) -Compress -Depth 4
"#;
    match run(
        "powershell.exe",
        &["-NoProfile", "-NonInteractive", "-Command", script],
    ) {
        Ok(json) => {
            match serde_json::from_str::<Vec<Check>>(json.trim_start_matches('\u{feff}').trim()) {
                Ok(mut rows) => checks.append(&mut rows),
                Err(e) => checks.push(Check {
                    level: "unknown".into(),
                    title: "Системная проверка".into(),
                    detail: format!("Не удалось прочитать отчёт: {e}"),
                }),
            }
        }
        Err(e) => checks.push(Check {
            level: "unknown".into(),
            title: "Системная проверка".into(),
            detail: e,
        }),
    }
    checks
}

#[cfg(test)]
mod diagnostic_tests {
    #[test]
    fn read_only_report_is_structured() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("zapret-discord-youtube-1.10.4");
        let report = super::diagnostics(&root);
        assert!(
            report.iter().any(|r| r.title == "Служба Zapret"),
            "{}",
            serde_json::to_string(&report).unwrap()
        );
        assert!(report.iter().any(|r| r.title == "Файл hosts"));
        assert!(report
            .iter()
            .all(|r| !r.title.is_empty() && !r.detail.is_empty()));
    }
}

#[derive(Default, serde::Deserialize)]
pub struct Snapshot {
    pub service_exists: bool,
    pub service_running: bool,
    pub winws: bool,
    pub strategy: String,
}
pub fn snapshot() -> Result<Snapshot, String> {
    let script = r#"$ErrorActionPreference='Stop';$s=Get-Service zapret -ErrorAction SilentlyContinue;$strategy='';try{$strategy=[string](Get-ItemPropertyValue 'HKLM:\SYSTEM\CurrentControlSet\Services\zapret' -Name 'zapret-discord-youtube' -ErrorAction Stop)}catch{};[Console]::OutputEncoding=New-Object Text.UTF8Encoding($false);@{service_exists=[bool]$s;service_running=[bool]($s -and $s.Status -eq 'Running');winws=[bool](Get-Process winws -ErrorAction SilentlyContinue);strategy=$strategy}|ConvertTo-Json -Compress"#;
    let data = run(
        "powershell.exe",
        &["-NoProfile", "-NonInteractive", "-Command", script],
    )?;
    serde_json::from_str(data.trim_start_matches('\u{feff}').trim()).map_err(|e| e.to_string())
}
