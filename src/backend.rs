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
    match action {
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
