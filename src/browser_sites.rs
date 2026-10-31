use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Browser {
    #[default]
    Chrome,
    Firefox,
    Opera,
    Edge,
}
impl Browser {
    pub const ALL: [Self; 4] = [Self::Chrome, Self::Firefox, Self::Opera, Self::Edge];
    pub fn name(self) -> &'static str {
        match self {
            Self::Chrome => "Chrome",
            Self::Firefox => "Firefox",
            Self::Opera => "Opera",
            Self::Edge => "Edge",
        }
    }
}
#[derive(Clone)]
pub struct Site {
    pub domain: String,
    pub title: String,
    pub url: String,
    pub source: String,
    pub visited: i64,
}
#[derive(Default)]
pub struct Discovery {
    pub sites: Vec<Site>,
    pub notes: Vec<String>,
}
pub struct Sites {
    pub browser: Browser,
    pub port: u16,
    pub domain: String,
    pub source: String,
    pub discovery: Discovery,
    pub rx: Option<mpsc::Receiver<Discovery>>,
    pub message: String,
}
impl Default for Sites {
    fn default() -> Self {
        Self {
            browser: Browser::Chrome,
            port: 9222,
            domain: String::new(),
            source: String::new(),
            discovery: Discovery::default(),
            rx: None,
            message: String::new(),
        }
    }
}
impl Sites {
    pub fn start(&mut self, history_only: bool) {
        if self.rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.discovery = Discovery::default();
        self.message.clear();
        let (browser, port) = (self.browser, self.port);
        std::thread::spawn(move || {
            let _ = tx.send(discover(browser, port, history_only));
        });
    }
    pub fn poll(&mut self) {
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(result) => {
                    self.discovery = result;
                    self.rx = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.message = "Чтение сайтов прервано. Повторите попытку.".into();
                    self.rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
    }
}
pub fn domain(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return Err("Введите домен или HTTP/HTTPS-адрес сайта.".into());
    }
    let url = url::Url::parse(&if value.contains("://") {
        value.to_owned()
    } else {
        format!("https://{value}")
    })
    .map_err(|_| "Некорректный адрес сайта")?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Поддерживаются только HTTP и HTTPS сайты.".into());
    }
    let Some(url::Host::Domain(host)) = url.host() else {
        return Err("Нужен домен сайта, а не IP-адрес.".into());
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_owned();
    if host.len() > 253
        || !host.contains('.')
        || !host.split('.').all(|p| {
            !p.is_empty()
                && p.len() <= 63
                && !p.starts_with('-')
                && !p.ends_with('-')
                && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
    {
        return Err("Некорректный домен сайта.".into());
    }
    Ok(host)
}
fn http_json(port: u16, endpoint: &str) -> Result<serde_json::Value, String> {
    let mut stream = TcpStream::connect_timeout(
        &SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        Duration::from_millis(800),
    )
    .map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    write!(
        stream,
        "GET {endpoint} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    stream
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("Ответ CDP слишком большой".into());
    }
    let split = bytes
        .windows(4)
        .position(|p| p == b"\r\n\r\n")
        .ok_or("Некорректный ответ CDP")?;
    let header = String::from_utf8_lossy(&bytes[..split]);
    if header
        .lines()
        .next()
        .and_then(|s| s.split_whitespace().nth(1))
        != Some("200")
    {
        return Err("CDP вернул ошибку".into());
    }
    let body = &bytes[split + 4..];
    if header
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        let mut decoded = Vec::new();
        let mut rest = body;
        loop {
            let end = rest
                .windows(2)
                .position(|p| p == b"\r\n")
                .ok_or("Повреждённый ответ CDP")?;
            let size = usize::from_str_radix(
                std::str::from_utf8(&rest[..end])
                    .map_err(|e| e.to_string())?
                    .split(';')
                    .next()
                    .unwrap(),
                16,
            )
            .map_err(|e| e.to_string())?;
            rest = &rest[end + 2..];
            if size == 0 {
                break;
            }
            if size > rest.len().saturating_sub(2) || rest.get(size..size + 2) != Some(b"\r\n") {
                return Err("Неполный ответ CDP".into());
            }
            decoded.extend_from_slice(&rest[..size]);
            rest = &rest[size + 2..];
        }
        serde_json::from_slice(&decoded).map_err(|e| e.to_string())
    } else {
        serde_json::from_slice(body).map_err(|e| e.to_string())
    }
}
fn cdp(port: u16) -> Result<Vec<Site>, String> {
    let tabs = http_json(port, "/json/list")?;
    let tabs = tabs.as_array().ok_or("Неизвестный формат вкладок CDP")?;
    let mut seen = HashSet::new();
    let mut sites = Vec::new();
    for tab in tabs {
        if tab["type"].as_str() != Some("page") {
            continue;
        }
        let Some(url) = tab["url"].as_str() else {
            continue;
        };
        if let Ok(domain) = domain(url) {
            if seen.insert(domain.clone()) {
                sites.push(Site {
                    domain,
                    title: tab["title"].as_str().unwrap_or("").into(),
                    url: url.into(),
                    source: format!("Вкладка браузера · CDP :{port}"),
                    visited: 0,
                });
            }
        }
    }
    Ok(sites)
}
#[derive(Clone)]
struct Profile {
    path: PathBuf,
    label: String,
}
fn profiles(browser: Browser, local: &Path, roaming: &Path) -> Vec<Profile> {
    let mut candidates: Vec<Profile> = Vec::new();
    let mut add = |dir: PathBuf, label: String| {
        let path = dir.join(if browser == Browser::Firefox {
            "places.sqlite"
        } else {
            "History"
        });
        if path.is_file() && !candidates.iter().any(|p| p.path == path) {
            candidates.push(Profile { path, label });
        }
    };
    if browser == Browser::Firefox {
        let base = roaming.join("Mozilla/Firefox");
        if let Ok(ini) = fs::read_to_string(base.join("profiles.ini")) {
            let mut name = String::new();
            let mut path = String::new();
            let mut relative = true;
            for line in ini.lines().chain(std::iter::once("[end]")) {
                let line = line.trim();
                if line.starts_with('[') {
                    if !path.is_empty() {
                        add(
                            if relative {
                                base.join(&path)
                            } else {
                                PathBuf::from(&path)
                            },
                            name.clone(),
                        );
                    }
                    name.clear();
                    path.clear();
                    relative = true;
                } else if let Some(v) = line.strip_prefix("Name=") {
                    name = v.into();
                } else if let Some(v) = line.strip_prefix("Path=") {
                    path = v.into();
                } else if let Some(v) = line.strip_prefix("IsRelative=") {
                    relative = v != "0";
                }
            }
        }
        if let Ok(dirs) = fs::read_dir(base.join("Profiles")) {
            for entry in dirs.flatten() {
                add(
                    entry.path(),
                    entry.file_name().to_string_lossy().into_owned(),
                );
            }
        }
    } else {
        let bases = match browser {
            Browser::Chrome => vec![local.join("Google/Chrome/User Data")],
            Browser::Edge => vec![local.join("Microsoft/Edge/User Data")],
            Browser::Opera => vec![
                roaming.join("Opera Software/Opera Stable"),
                roaming.join("Opera Software/Opera GX Stable"),
            ],
            _ => unreachable!(),
        };
        for base in bases {
            let family = base
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            add(base.clone(), family.clone());
            if let Ok(dirs) = fs::read_dir(&base) {
                for entry in dirs.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if entry.path().is_dir() {
                        add(entry.path(), format!("{family} / {name}"));
                    }
                }
            }
        }
    }
    candidates.sort_by(|a, b| a.path.cmp(&b.path));
    candidates
}
fn discover(browser: Browser, port: u16, history_only: bool) -> Discovery {
    let mut result = Discovery::default();
    if !history_only && browser != Browser::Firefox {
        match cdp(port) {
            Ok(sites) if !sites.is_empty() => {
                result.sites = sites;
                result.notes.push("Открытые сайты подключённого Chromium-браузера. Выберите нужный: активная вкладка автоматически не определяется.".into());
                return result;
            }
            Ok(_) => result
                .notes
                .push("CDP доступен, но HTTP/HTTPS-вкладок нет. Показана история.".into()),
            Err(_) => result.notes.push(format!(
                "CDP на 127.0.0.1:{port} недоступен. Показана история {}.",
                browser.name()
            )),
        }
    }
    let local = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_default();
    let roaming = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_default();
    let profiles = profiles(browser, &local, &roaming);
    if profiles.is_empty() {
        result.notes.push(format!("История {} не найдена в стандартных профилях. Можно выбрать файл истории вручную или ввести домен.",browser.name()));
    }
    for profile in profiles {
        history(&mut result, &profile, browser);
    }
    result.sites.sort_by(|a, b| b.visited.cmp(&a.visited));
    let mut seen = HashSet::new();
    result.sites.retain(|s| seen.insert(s.domain.clone()));
    result.sites.truncate(50);
    result
}
fn history(result: &mut Discovery, profile: &Profile, browser: Browser) {
    match super::sqlite_history::read(&profile.path, browser == Browser::Firefox) {
        Ok(rows) => {
            for row in rows {
                if let Ok(domain) = domain(&row[0]) {
                    result.sites.push(Site {
                        domain,
                        title: row[1].clone(),
                        url: row[0].clone(),
                        source: format!("История {} · {}", browser.name(), profile.label),
                        visited: row[2].parse().unwrap_or(0),
                    });
                }
            }
        }
        Err(e) => result
            .notes
            .push(format!("{} / {}: {e}", browser.name(), profile.label)),
    }
}
pub fn custom_history(path: PathBuf, browser: Browser) -> Discovery {
    let mut result = Discovery::default();
    let label = path
        .parent()
        .and_then(Path::file_name)
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    history(&mut result, &Profile { path, label }, browser);
    let mut seen = HashSet::new();
    result.sites.retain(|s| seen.insert(s.domain.clone()));
    result.sites.truncate(50);
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn domains_from_urls() {
        assert_eq!(
            domain(" HTTPS://SteamGames.COM/path?q=1 ").unwrap(),
            "steamgames.com"
        );
        assert_eq!(domain("пример.рф").unwrap(), "xn--e1afmkfd.xn--p1ai");
        assert_eq!(domain("www.example.org.").unwrap(), "example.org");
        assert_eq!(
            domain("https://WWW.example.org/path").unwrap(),
            "example.org"
        );
        assert_eq!(domain("store.example.org").unwrap(), "store.example.org");
        for value in [
            "chrome://settings",
            "about:blank",
            "file:///test",
            "127.0.0.1",
            "https://[::1]",
            "localhost",
            "*.example.org",
            "example.org\nother.org",
        ] {
            assert!(domain(value).is_err(), "{value}");
        }
    }
    #[test]
    fn profiles_include_chromium_opera_and_firefox_ini() {
        let base = std::env::temp_dir().join(format!("zapret-profiles-{}", std::process::id()));
        for path in [
            "Google/Chrome/User Data/Default/History",
            "Microsoft/Edge/User Data/Profile 2/History",
            "Opera Software/Opera Stable/Default/History",
            "Mozilla/Firefox/Profiles/test/places.sqlite",
        ] {
            let p = base.join(path);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, []).unwrap();
        }
        fs::write(
            base.join("Mozilla/Firefox/profiles.ini"),
            "[Profile0]\nName=Test\nIsRelative=1\nPath=Profiles/test\n",
        )
        .unwrap();
        for b in Browser::ALL {
            assert_eq!(profiles(b, &base, &base).len(), 1, "{}", b.name());
        }
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn cdp_lists_page_domains_and_ignores_internal_targets() {
        let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                assert!(request.len() < 4096);
            }
            let body = r#"[{"type":"page","url":"https://example.org/a","title":"Example"},{"type":"page","url":"https://example.org/b"},{"type":"service_worker","url":"https://worker.org"},{"type":"page","url":"chrome://settings"}]"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let sites = cdp(port).unwrap();
        thread.join().unwrap();
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].domain, "example.org");
    }
}
