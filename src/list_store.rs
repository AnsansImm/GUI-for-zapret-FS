use std::os::windows::fs::OpenOptionsExt;
use std::{fs, io::Write, path::Path};
pub const GENERAL: &str = "list-general-user.txt";
pub const EXCLUDE: &str = "list-exclude-user.txt";
#[derive(serde::Serialize, serde::Deserialize)]
struct Journal {
    general: String,
    exclude: String,
}
pub fn lock(lists: &Path) -> Result<fs::File, String> {
    fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .share_mode(0)
        .open(lists.join(".zapret-gui.lock"))
        .map_err(|_| "Списки заняты другим действием. Повторите попытку.".into())
}
pub fn atomic(path: &Path, data: &[u8]) -> Result<(), String> {
    let temp = path.with_extension(format!(
        "gui-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let result = (|| {
        let mut f = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;
        f.write_all(data)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result.map_err(|e: std::io::Error| e.to_string())
}
pub fn recover(lists: &Path) -> Result<(), String> {
    let p = lists.join(".zapret-gui-transaction.json");
    match fs::read(&p) {
        Ok(data) => {
            let old: Journal = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
            atomic(&lists.join(GENERAL), old.general.as_bytes())?;
            atomic(&lists.join(EXCLUDE), old.exclude.as_bytes())?;
            fs::remove_file(p).map_err(|e| e.to_string())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
fn read(lists: &Path, name: &str) -> Result<String, String> {
    match fs::read_to_string(lists.join(name)) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.to_string()),
    }
}
pub fn valid_domain(s: &str) -> bool {
    s.len() <= 253
        && s.contains('.')
        && s.parse::<std::net::IpAddr>().is_err()
        && s.split('.').all(|p| {
            !p.is_empty()
                && p.len() <= 63
                && !p.starts_with('-')
                && !p.ends_with('-')
                && p.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
}
fn contains(text: &str, domain: &str) -> bool {
    text.trim_start_matches('\u{feff}')
        .lines()
        .any(|l| l.trim().eq_ignore_ascii_case(domain))
}
fn change(text: &str, domain: &str, add: bool) -> String {
    let mut lines: Vec<String> = text
        .trim_start_matches('\u{feff}')
        .lines()
        .filter(|l| !l.trim().eq_ignore_ascii_case(domain))
        .map(str::to_owned)
        .collect();
    if add {
        lines.push(domain.into());
    }
    if lines
        .iter()
        .all(|l| l.trim().is_empty() || l.trim_start().starts_with('#'))
    {
        lines.push("domain.example.abc".into());
    }
    let mut s = lines.join("\n");
    s.push('\n');
    s
}
pub fn handle(root: &Path, domain: &str, action: &str) -> Result<String, String> {
    if !valid_domain(domain) {
        return Err(
            "Нужен домен HTTP/HTTPS-сайта; IP-адреса и служебные страницы не поддерживаются."
                .into(),
        );
    }
    if !["status", "general", "exclude"].contains(&action) {
        return Err("Неизвестная команда".into());
    }
    if !root.join("service.bat").is_file() {
        return Err("Папка Zapret недоступна. Выберите её в программе.".into());
    }
    let lists = root.join("lists");
    let _guard = lock(&lists)?;
    recover(&lists)?;
    let old = Journal {
        general: read(&lists, GENERAL)?,
        exclude: read(&lists, EXCLUDE)?,
    };
    let (general, exclude) = if action == "status" {
        (old.general.clone(), old.exclude.clone())
    } else {
        let general = change(&old.general, domain, action == "general");
        let exclude = change(&old.exclude, domain, action == "exclude");
        let journal = lists.join(".zapret-gui-transaction.json");
        atomic(
            &journal,
            &serde_json::to_vec(&old).map_err(|e| e.to_string())?,
        )?;
        let result = atomic(&lists.join(GENERAL), general.as_bytes())
            .and_then(|_| atomic(&lists.join(EXCLUDE), exclude.as_bytes()))
            .and_then(|_| fs::remove_file(&journal).map_err(|e| e.to_string()));
        if let Err(e) = result {
            let rollback = recover(&lists);
            return Err(format!(
                "Не удалось изменить списки: {e}. Восстановление: {rollback:?}"
            ));
        }
        (general, exclude)
    };
    Ok(
        match (contains(&general, domain), contains(&exclude, domain)) {
            (true, true) => "both",
            (true, false) => "general",
            (false, true) => "exclude",
            _ => "none",
        }
        .into(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn domains() {
        for d in ["example.com", "xn--e1afmkfd.xn--p1ai", "a-b.example"] {
            assert!(valid_domain(d));
        }
        for d in [
            "127.0.0.1",
            "localhost",
            "https://a.com",
            "x.com\nother.com",
            "*.example.com",
            "-x.com",
        ] {
            assert!(!valid_domain(d));
        }
    }
    #[test]
    fn exact_records_comments_and_conflicts() {
        let text = "# comment\n*.example.com\nEXAMPLE.COM\nsub.example.com\n";
        assert!(contains(text, "example.com"));
        let changed = change(text, "example.com", false);
        assert!(!contains(&changed, "example.com"));
        assert!(changed.contains("*.example.com"));
        assert!(changed.contains("sub.example.com"));
        assert!(changed.contains("# comment"));
        assert_eq!(
            change("example.com\nexample.com", "example.com", true)
                .matches("example.com")
                .count(),
            1
        );
    }
    #[test]
    fn transaction_and_lock() {
        let root = std::env::temp_dir().join(format!(
            "zapret-lists-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let lists = root.join("lists");
        fs::create_dir_all(&lists).unwrap();
        fs::write(root.join("service.bat"), b"test").unwrap();
        assert_eq!(handle(&root, "example.com", "general").unwrap(), "general");
        assert_eq!(handle(&root, "example.com", "exclude").unwrap(), "exclude");
        assert_eq!(handle(&root, "example.com", "status").unwrap(), "exclude");
        let guard = lock(&lists).unwrap();
        assert!(handle(&root, "example.com", "general").is_err());
        drop(guard);
        let old = Journal {
            general: "old.example\n".into(),
            exclude: "# comment\n".into(),
        };
        atomic(
            &lists.join(".zapret-gui-transaction.json"),
            &serde_json::to_vec(&old).unwrap(),
        )
        .unwrap();
        handle(&root, "old.example", "status").unwrap();
        assert_eq!(
            fs::read_to_string(lists.join(GENERAL)).unwrap(),
            old.general
        );
        for e in fs::read_dir(&lists).unwrap() {
            fs::remove_file(e.unwrap().path()).unwrap();
        }
        fs::remove_dir(lists).unwrap();
        fs::remove_file(root.join("service.bat")).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
