use std::{fs, io::Write, net::IpAddr, path::Path};
const NONE: &str = "203.0.113.113/32\n";
pub fn classify(text: &str) -> &'static str {
    if text.trim_start_matches('\u{feff}').trim().is_empty() {
        "any"
    } else if text.lines().any(|l| l.trim() == "203.0.113.113/32") {
        "none"
    } else {
        "loaded"
    }
}
pub fn status(root: &Path) -> Result<&'static str, String> {
    fs::read_to_string(root.join("lists/ipset-all.txt"))
        .map(|s| classify(&s))
        .map_err(|e| e.to_string())
}
fn replace(path: &Path, content: &[u8]) -> Result<(), String> {
    let temp = path.with_extension(format!(
        "gui-{}.tmp",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(content)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result.map_err(|e: std::io::Error| e.to_string())
}
pub fn set_mode(root: &Path, mode: &str) -> Result<String, String> {
    let path = root.join("lists/ipset-all.txt");
    let backup = root.join("lists/ipset-all.txt.backup");
    let old = fs::read_to_string(&path).map_err(|e| format!("Не удалось прочитать IPSet: {e}"))?;
    if classify(&old) == mode {
        return Ok("Режим уже выбран".into());
    }
    match mode {
        "none" | "any" => {
            if classify(&old) == "loaded" {
                validate(&old)?;
                replace(&backup, old.as_bytes())?;
            }
            replace(&path, if mode == "none" { NONE.as_bytes() } else { b"" })?;
        }
        "loaded" => {
            let data = fs::read_to_string(&backup)
                .map_err(|_| "Нет резервного списка. Сначала обновите IPSet.".to_string())?;
            validate(&data)?;
            replace(&path, data.as_bytes())?;
        }
        _ => return Err("Неизвестный режим IPSet".into()),
    }
    Ok("Режим IPSet сохранён".into())
}
fn valid_entry(s: &str) -> bool {
    if let Some((ip, prefix)) = s.split_once('/') {
        match (ip.parse::<IpAddr>(), prefix.parse::<u8>()) {
            (Ok(IpAddr::V4(_)), Ok(n)) => n <= 32,
            (Ok(IpAddr::V6(_)), Ok(n)) => n <= 128,
            _ => false,
        }
    } else if let Some((a, b)) = s.split_once('-') {
        match (a.parse::<IpAddr>(), b.parse::<IpAddr>()) {
            (Ok(IpAddr::V4(a)), Ok(IpAddr::V4(b))) => a <= b,
            (Ok(IpAddr::V6(a)), Ok(IpAddr::V6(b))) => a <= b,
            _ => false,
        }
    } else {
        s.parse::<IpAddr>().is_ok()
    }
}
fn validate(text: &str) -> Result<(), String> {
    let mut count = 0;
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if !valid_entry(line) {
            return Err(format!(
                "Неверная запись IPSet: {}",
                line.chars().take(100).collect::<String>()
            ));
        }
        count += 1;
    }
    if count == 0 || classify(text) != "loaded" {
        return Err("Получен пустой или отключённый список IPSet".into());
    }
    Ok(())
}
pub fn update(root: &Path) -> Result<String, String> {
    let output=super::backend::command("curl.exe").args([
        "--fail","--location","--silent","--show-error","--connect-timeout","10","--max-time","60",
        "https://raw.githubusercontent.com/Flowseal/zapret-discord-youtube/refs/heads/main/.service/ipset-service.txt"
    ]).output().map_err(|e|e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "Не удалось загрузить IPSet: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let data = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    validate(&data)?;
    let path = root.join("lists/ipset-all.txt");
    let backup = root.join("lists/ipset-all.txt.backup");
    match fs::read_to_string(&path) {
        Ok(old) if classify(&old) == "loaded" => {
            replace(&backup, old.as_bytes())?;
            replace(&path, data.as_bytes())?;
            Ok("IPSet обновлён. Предыдущий список сохранён в резервной копии.".into())
        }
        Ok(_) => {
            replace(&backup, data.as_bytes())?;
            Ok("Список загружен в резервную копию. Текущий режим сохранён; выберите «По списку», чтобы включить загруженные адреса.".into())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            replace(&path, data.as_bytes())?;
            Ok("IPSet загружен, включён режим «По списку».".into())
        }
        Err(e) => Err(e.to_string()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modes() {
        assert_eq!(classify("\u{feff}\r\n"), "any");
        assert_eq!(classify(NONE), "none");
        assert_eq!(classify("1.1.1.1/32\n"), "loaded");
    }
    #[test]
    fn downloaded_validation() {
        assert!(validate("# comment\n1.1.1.0/24\n2001:db8::/32\n1.1.1.1-1.1.1.2").is_ok());
        for s in [
            "<html>error</html>",
            "",
            NONE,
            "1.1.1.1/33",
            "2001:db8::/129",
            "1.1.1.2-1.1.1.1",
        ] {
            assert!(validate(s).is_err(), "{s}");
        }
    }
    #[test]
    fn roundtrip_and_missing_backup() {
        let dir = std::env::temp_dir().join(format!(
            "zapret-ipset-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(dir.join("lists")).unwrap();
        let p = dir.join("lists/ipset-all.txt");
        fs::write(&p, NONE).unwrap();
        assert!(set_mode(&dir, "loaded").is_err());
        fs::write(&p, "1.1.1.0/24\n").unwrap();
        set_mode(&dir, "none").unwrap();
        assert_eq!(status(&dir).unwrap(), "none");
        set_mode(&dir, "any").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"");
        set_mode(&dir, "loaded").unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "1.1.1.0/24\n");
        for file in ["ipset-all.txt", "ipset-all.txt.backup"] {
            fs::remove_file(dir.join("lists").join(file)).unwrap();
        }
        fs::remove_dir(dir.join("lists")).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
