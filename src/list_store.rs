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
