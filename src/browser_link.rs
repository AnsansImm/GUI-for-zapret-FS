use std::{fs, path::Path};
const HOST: &str = "com.zapret_gui.lists";
pub fn register(directory: &Path, browser: &str, id: &str) -> Result<String, String> {
    let native = directory.join("zapret-native.exe");
    if !native.is_file() {
        return Err(
            "Рядом с программой не найден zapret-native.exe. Скопируйте оба exe в одну папку."
                .into(),
        );
    }
    let firefox = browser == "Firefox";
    if !firefox && (id.len() != 32 || !id.bytes().all(|c| (b'a'..=b'p').contains(&c))) {
        return Err("Введите ID расширения: 32 буквы a–p со страницы расширений браузера".into());
    }
    let dir = directory.join("browser-hosts");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(if firefox {
        "firefox.json"
    } else {
        "chromium.json"
    });
    let mut value = serde_json::json!({"name":HOST,"description":"Zapret GUI user lists","path":native,"type":"stdio"});
    if firefox {
        value["allowed_extensions"] = serde_json::json!(["zapret-gui@local"]);
    } else {
        let mut origins = std::collections::BTreeSet::new();
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(old) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if let Some(items) = old["allowed_origins"].as_array() {
                    for item in items {
                        if let Some(s) = item.as_str() {
                            origins.insert(s.to_owned());
                        }
                    }
                }
            }
        }
        origins.insert(format!("chrome-extension://{id}/"));
        value["allowed_origins"] = serde_json::json!(origins);
    }
    fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).map_err(|e| e.to_string())?;
    let keys: Vec<&str> = match browser {
        "Firefox" => vec![r"HKCU\Software\Mozilla\NativeMessagingHosts"],
        "Edge" => vec![r"HKCU\Software\Microsoft\Edge\NativeMessagingHosts"],
        "Chrome / Opera" => vec![
            r"HKCU\Software\Google\Chrome\NativeMessagingHosts",
            r"HKCU\Software\Chromium\NativeMessagingHosts",
        ],
        _ => return Err("Неизвестный браузер".into()),
    };
    for key in keys {
        let o = super::backend::command("reg.exe")
            .args([
                "add",
                &format!("{key}\\{HOST}"),
                "/ve",
                "/t",
                "REG_SZ",
                "/d",
                &path.to_string_lossy(),
                "/f",
            ])
            .output()
            .map_err(|e| e.to_string())?;
        if !o.status.success() {
            return Err(format!(
                "Не удалось зарегистрировать помощник: {}",
                String::from_utf8_lossy(&o.stderr)
            ));
        }
    }
    Ok(format!("Подключение для {browser} зарегистрировано. Откройте расширение и нажмите «Обновить статус»."))
}
