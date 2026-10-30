use std::{fs, io::Write, path::Path};
pub const TARGETS: [(&str, &str); 2] = [
    ("ACTIVE_DISCORD_UDP.bin", "Discord UDP"),
    ("ACTIVE_GAME_UDP.bin", "Game Filter UDP"),
];
pub fn available(root: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root.join("bin")).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let p = entry.path();
        if p.is_file() && p.extension().is_some_and(|s| s.eq_ignore_ascii_case("bin")) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.to_ascii_uppercase().starts_with("ACTIVE_") {
                files.push(name);
            }
        }
    }
    files.sort_by_key(|s| s.to_lowercase());
    Ok(files)
}
pub fn current(root: &Path, target: &str, files: &[String]) -> String {
    match fs::read(root.join("bin").join(target)) {
        Ok(active) => files
            .iter()
            .filter(|n| fs::read(root.join("bin").join(n)).is_ok_and(|data| data == active))
            .cloned()
            .collect::<Vec<_>>()
            .join(", "),
        Err(e) => format!("Недоступен: {e}"),
    }
}
fn write_atomic(path: &Path, data: &[u8]) -> Result<(), String> {
    let temp = path.with_extension(format!(
        "{}.tmp",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let result = (|| {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
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
pub fn replace(root: &Path, target: &str, source: &str) -> Result<String, String> {
    if !TARGETS.iter().any(|(t, _)| *t == target) {
        return Err("Неизвестный тип fake-файла".into());
    }
    if !available(root)?.iter().any(|n| n == source) {
        return Err("Файл не найден среди доступных fake-файлов".into());
    }
    let bin = root.join("bin");
    let path = bin.join(target);
    let data = fs::read(bin.join(source)).map_err(|e| e.to_string())?;
    if data.is_empty() {
        return Err("Нельзя использовать пустой fake-файл".into());
    }
    match fs::read(&path) {
        Ok(old) if old == data => return Ok("Этот fake-файл уже выбран".into()),
        Ok(old) => write_atomic(&bin.join(format!("{target}.backup")), &old)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.to_string()),
    }
    write_atomic(&path, &data)?;
    Ok(format!("{target} заменён на {source}. Для применения перезапустите Zapret кнопками «Остановить» и «Запустить»."))
}
pub fn restore(root: &Path, target: &str) -> Result<String, String> {
    if !TARGETS.iter().any(|(t, _)| *t == target) {
        return Err("Неизвестный тип fake-файла".into());
    }
    let path = root.join("bin").join(target);
    let backup = root.join("bin").join(format!("{target}.backup"));
    let data = fs::read(&backup).map_err(|e| format!("Резервная копия недоступна: {e}"))?;
    if data.is_empty() {
        return Err("Резервная копия пуста".into());
    }
    write_atomic(&path, &data)?;
    Ok(format!(
        "{target} восстановлен. Перезапустите Zapret для применения."
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replace_backup_restore_and_validation() {
        let dir = std::env::temp_dir().join(format!(
            "zapret-fakes-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let bin = dir.join("bin");
        fs::create_dir_all(&bin).unwrap();
        for (n, data) in [
            ("a.bin", b"new".as_slice()),
            ("empty.bin", b"".as_slice()),
            ("ACTIVE_DISCORD_UDP.bin", b"old".as_slice()),
        ] {
            fs::write(bin.join(n), data).unwrap();
        }
        let files = available(&dir).unwrap();
        assert_eq!(files, vec!["a.bin", "empty.bin"]);
        assert!(replace(&dir, TARGETS[0].0, "../a.bin").is_err());
        assert!(replace(&dir, "other.bin", "a.bin").is_err());
        assert!(replace(&dir, TARGETS[0].0, "empty.bin").is_err());
        replace(&dir, TARGETS[0].0, "a.bin").unwrap();
        assert_eq!(current(&dir, TARGETS[0].0, &files), "a.bin");
        assert_eq!(
            fs::read(bin.join("ACTIVE_DISCORD_UDP.bin.backup")).unwrap(),
            b"old"
        );
        replace(&dir, TARGETS[0].0, "a.bin").unwrap();
        restore(&dir, TARGETS[0].0).unwrap();
        assert_eq!(fs::read(bin.join(TARGETS[0].0)).unwrap(), b"old");
        for e in fs::read_dir(&bin).unwrap() {
            fs::remove_file(e.unwrap().path()).unwrap();
        }
        fs::remove_dir(bin).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
