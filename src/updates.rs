use std::{fs, path::Path, sync::mpsc};
pub const DOWNLOAD: &str = "https://github.com/Flowseal/zapret-discord-youtube/releases/latest";
pub fn parse_version(value: &str) -> Option<Vec<u32>> {
    let v = value
        .trim()
        .trim_start_matches('\u{feff}')
        .trim()
        .trim_start_matches('v');
    let parts = v
        .split('.')
        .map(|p| {
            if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) {
                p.parse().ok()
            } else {
                None
            }
        })
        .collect::<Option<Vec<u32>>>()?;
    (parts.len() >= 2 && parts.len() <= 4).then_some(parts)
}
pub fn local(root: &Path) -> Result<String, String> {
    let s = fs::read_to_string(root.join("service.bat")).map_err(|e| e.to_string())?;
    s.lines()
        .find_map(|l| {
            let l = l.trim();
            l.strip_prefix("set \"LOCAL_VERSION=")
                .and_then(|s| s.strip_suffix('"'))
                .filter(|s| parse_version(s).is_some())
                .map(str::to_owned)
        })
        .ok_or("Не удалось прочитать LOCAL_VERSION из service.bat".into())
}
pub fn compare(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let mut a = parse_version(a)?;
    let mut b = parse_version(b)?;
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    Some(a.cmp(&b))
}
pub fn fetch() -> Result<String, String> {
    let o=super::backend::command("curl.exe").args(["--fail","--location","--silent","--show-error","--connect-timeout","10","--max-time","20","-H","Cache-Control: no-cache","https://raw.githubusercontent.com/Flowseal/zapret-discord-youtube/main/.service/version.txt"]).output().map_err(|e|e.to_string())?;
    if !o.status.success() {
        return Err(format!(
            "Не удалось проверить обновления: {}",
            String::from_utf8_lossy(&o.stderr)
        ));
    }
    let text = String::from_utf8(o.stdout).map_err(|e| e.to_string())?;
    let version = text.trim().trim_start_matches('\u{feff}').trim();
    if parse_version(version).is_none() {
        return Err("Источник вернул неизвестный формат версии".into());
    }
    Ok(version.into())
}
#[derive(Default)]
pub struct Updates {
    pub latest: Option<String>,
    pub error: String,
    pub checked: Option<String>,
    pub rx: Option<mpsc::Receiver<Result<String, String>>>,
}
impl Updates {
    pub fn poll(&mut self) {
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(result) => {
                    self.rx = None;
                    self.checked = Some("Проверка выполнена в этом сеансе".into());
                    match result {
                        Ok(v) => {
                            self.latest = Some(v);
                            self.error.clear()
                        }
                        Err(e) => {
                            self.latest = None;
                            self.error = e;
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.rx = None;
                    self.error = "Проверка прервана".into();
                }
                _ => {}
            }
        }
    }
    pub fn start(&mut self) {
        if self.rx.is_some() {
            return;
        }
        self.error.clear();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(fetch());
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions() {
        assert_eq!(
            compare("1.10.4", "1.9.9"),
            Some(std::cmp::Ordering::Greater)
        );
        assert_eq!(
            compare("v1.10.4", "1.10.4.0"),
            Some(std::cmp::Ordering::Equal)
        );
        for v in ["<html>", "1..2", "1.2-beta", "1", "1.-2"] {
            assert!(parse_version(v).is_none());
        }
    }
    #[test]
    fn shipped_version() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("zapret-discord-youtube-1.10.4");
        if root.exists() {
            assert_eq!(local(&root).unwrap(), "1.10.4");
        }
    }
}
