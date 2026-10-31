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
pub fn add_domain(root: &Path, value: &str, exclude: bool) -> Result<String, String> {
    let domain = super::browser_sites::domain(value)?;
    let lists = root.join("lists");
    let _guard = lock(&lists)?;
    recover(&lists)?;
    let read = |name| match fs::read_to_string(lists.join(name)) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.to_string()),
    };
    let old = Journal {
        general: read(GENERAL)?,
        exclude: read(EXCLUDE)?,
    };
    let change = |text: &str, add: bool| {
        let text = super::editor::normalize(text, false);
        let mut lines: Vec<_> = text
            .lines()
            .filter(|line| {
                !line.eq_ignore_ascii_case(&domain)
                    && !line.eq_ignore_ascii_case(&format!("www.{domain}"))
            })
            .map(str::to_owned)
            .collect();
        if add {
            lines.push(domain.clone());
        }
        if lines.iter().all(|line| line.starts_with('#')) {
            lines.push("domain.example.abc".into());
        }
        format!("{}\n", lines.join("\n"))
    };
    let general = change(&old.general, !exclude);
    let excluded = change(&old.exclude, exclude);
    let journal = lists.join(".zapret-gui-transaction.json");
    atomic(
        &journal,
        &serde_json::to_vec(&old).map_err(|e| e.to_string())?,
    )?;
    let result = atomic(&lists.join(GENERAL), general.as_bytes())
        .and_then(|_| atomic(&lists.join(EXCLUDE), excluded.as_bytes()))
        .and_then(|_| fs::remove_file(&journal).map_err(|e| e.to_string()));
    if let Err(e) = result {
        return match recover(&lists) {
            Ok(()) => Err(format!(
                "Не удалось сохранить домен: {e}. Исходные списки восстановлены."
            )),
            Err(rollback) => Err(format!(
                "Ошибка сохранения: {e}. Ошибка восстановления: {rollback}"
            )),
        };
    }
    Ok(format!(
        "{domain} добавлен в {}. Для применения перезапустите работающий Zapret.",
        if exclude {
            "исключения"
        } else {
            "список обхода"
        }
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn move_domain_between_lists_without_duplicates() {
        let root = std::env::temp_dir().join(format!("zapret-add-site-{}", std::process::id()));
        fs::create_dir_all(root.join("lists")).unwrap();
        fs::write(
            root.join("lists").join(GENERAL),
            "# keep\nExample.org\nexample.org \nwww.example.org\nsub.example.org\n",
        )
        .unwrap();
        fs::write(
            root.join("lists").join(EXCLUDE),
            "# exclude\nexample.org\nWWW.example.org\n",
        )
        .unwrap();
        add_domain(&root, "https://WWW.EXAMPLE.ORG/path", false).unwrap();
        let general = fs::read_to_string(root.join("lists").join(GENERAL)).unwrap();
        assert_eq!(general.lines().filter(|s| *s == "example.org").count(), 1);
        assert!(!general.contains("www.example.org"));
        assert!(general.contains("# keep\n") && general.contains("sub.example.org\n"));
        assert!(!fs::read_to_string(root.join("lists").join(EXCLUDE))
            .unwrap()
            .lines()
            .any(|s| s.eq_ignore_ascii_case("example.org")
                || s.eq_ignore_ascii_case("www.example.org")));
        add_domain(&root, "example.org", true).unwrap();
        assert!(!fs::read_to_string(root.join("lists").join(GENERAL))
            .unwrap()
            .lines()
            .any(|s| s.eq_ignore_ascii_case("example.org")
                || s.eq_ignore_ascii_case("www.example.org")));
        assert!(fs::read_to_string(root.join("lists").join(EXCLUDE))
            .unwrap()
            .contains("example.org\n"));
        assert!(add_domain(&root, "chrome://settings", false).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
