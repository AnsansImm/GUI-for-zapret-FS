use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
};
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
    let o = super::backend::command("curl.exe")
        .args([
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--connect-timeout",
            "10",
            "--max-time",
            "30",
            "-H",
            "User-Agent: Zapret-GUI",
            "-H",
            "Cache-Control: no-cache",
            "https://api.github.com/repos/Flowseal/zapret-discord-youtube/releases/latest",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(format!(
            "Не удалось проверить обновления: {}",
            String::from_utf8_lossy(&o.stderr)
        ));
    }
    let text = String::from_utf8(o.stdout).map_err(|e| e.to_string())?;
    let release: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let version = release["tag_name"]
        .as_str()
        .ok_or("В релизе нет версии")?
        .trim_start_matches('v');
    if parse_version(version).is_none() {
        return Err("Источник вернул неизвестный формат версии".into());
    }
    Ok(version.into())
}
pub fn install(root: &Path, config: &Path) -> Result<String, String> {
    let script = std::env::temp_dir().join(format!(
        "zapret-update-{}-{}.ps1",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut bytes = vec![0xef, 0xbb, 0xbf];
    bytes.extend_from_slice(include_bytes!("../scripts/update-zapret.ps1"));
    fs::write(&script, bytes).map_err(|e| e.to_string())?;
    let result = super::backend::command("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(&script)
        .arg("-Root")
        .arg(root)
        .arg("-Config")
        .arg(config)
        .output()
        .map_err(|e| e.to_string());
    let _ = fs::remove_file(script);
    let output = result?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let path = text
        .lines()
        .last()
        .ok_or("Обновление не вернуло новую папку")?
        .trim();
    if !Path::new(path).join("service.bat").is_file() {
        return Err("Новая папка не найдена".into());
    }
    Ok(path.into())
}
#[derive(Default)]
pub struct Updates {
    pub latest: Option<String>,
    pub error: String,
    pub checked: Option<String>,
    pub rx: Option<mpsc::Receiver<Result<String, String>>>,
    pub install_rx: Option<mpsc::Receiver<Result<String, String>>>,
}
impl Updates {
    pub fn start_install(&mut self, root: PathBuf, config: PathBuf) {
        if self.install_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.install_rx = Some(rx);
        self.error.clear();
        std::thread::spawn(move || {
            let _ = tx.send(super::backend::elevated(
                "update_release".into(),
                root,
                config.to_string_lossy().into_owned(),
            ));
        });
    }
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
    #[test]
    fn release_archive_preserves_user_lists_and_rejects_unsafe_paths() {
        let fixture =
            std::env::temp_dir().join(format!("zapret-update-test-{}", std::process::id()));
        fs::create_dir_all(&fixture).unwrap();
        let script = r#"
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$old=Join-Path $Fixture 'old'
$release=Join-Path $Fixture 'release'
New-Item -ItemType Directory -Force -Path "$old/lists","$release/bin","$release/lists","$release/utils" | Out-Null
[IO.File]::WriteAllText("$old/service.bat",'set "LOCAL_VERSION=1.0.0"')
[IO.File]::WriteAllText("$release/service.bat",'set "LOCAL_VERSION=2.0.0"')
[IO.File]::WriteAllText("$release/bin/winws.exe",'fixture')
[IO.File]::WriteAllText("$release/lists/list-general-user.txt",'upstream')
[IO.File]::WriteAllText("$release/lists/list-general.txt",'new upstream')
foreach($name in @('list-general-user.txt','list-exclude-user.txt','ipset-exclude-user.txt','custom-user.txt','custom.txt','ipset-all.txt.backup')) {
    [IO.File]::WriteAllBytes("$old/lists/$name",[byte[]](239,187,191,97,13,10))
}
[IO.File]::WriteAllText("$old/lists/list-general.txt",'old upstream')
$zip=Join-Path $Fixture 'release.zip'
[IO.Compression.ZipFile]::CreateFromDirectory($release,$zip)
& $Updater -Root $old -Config "$Fixture/config.json" -PrepareOnly -Archive $zip -Version '2.0.0' | Out-Null
if (!$?) { throw 'Preparation failed' }
$new=Join-Path $Fixture 'zapret-discord-youtube-2.0.0'
foreach($file in Get-ChildItem "$old/lists" -File | Where-Object { $_.Name -ne 'list-general.txt' -and $_.Name -ne '.zapret-gui.lock' }) {
    if ([Convert]::ToBase64String([IO.File]::ReadAllBytes($file.FullName)) -ne [Convert]::ToBase64String([IO.File]::ReadAllBytes("$new/lists/$($file.Name)"))) { throw 'User list changed' }
}
if ([IO.File]::ReadAllText("$new/lists/list-general.txt") -ne 'new upstream') { throw 'Upstream list was not updated' }
$rejected=$false
try { & $Updater -Root $old -Config "$Fixture/config.json" -PrepareOnly -Archive $zip -Version '2.0.0' | Out-Null } catch { $rejected=$true }
if (!$rejected) { throw 'Existing destination overwritten' }
$bad=Join-Path $Fixture 'unsafe.zip'
$z=[IO.Compression.ZipFile]::Open($bad,'Create')
$null=$z.CreateEntry('../escaped.txt');$z.Dispose()
$rejected=$false
try { & $Updater -Root $old -Config "$Fixture/config.json" -PrepareOnly -Archive $bad -Version '3.0.0' | Out-Null } catch { $rejected=$true }
if (!$rejected -or (Test-Path "$Fixture/escaped.txt")) { throw 'Unsafe archive accepted' }
if (@(Get-ChildItem $Fixture -Directory -Filter '.zapret-update-*').Count) { throw 'Staging directories leaked' }
# Exercise the real switch and rollback with fake network/service functions.
Move-Item -LiteralPath $new -Destination "$Fixture/prepared"
$global:TestBinary='"' + $old + '\bin\winws.exe" --hostlist="' + $old + '\lists\list-general-user.txt"'
$global:OriginalBinary=$global:TestBinary
$global:TestRunning=$true
$global:FailNextStart=$false
function global:Get-CimInstance {
    param($ClassName,$Filter,$OperationTimeoutSec)
    if ($ClassName -eq 'Win32_Service' -and $Filter -eq "Name='zapret'") { return [pscustomobject]@{PathName='';State=$(if($global:TestRunning){'Running'}else{'Stopped'});ProcessId=123} }
    if ($ClassName -eq 'Win32_Service') { return [pscustomobject]@{ProcessId=123} }
}
function global:Get-Service {
    param($Name)
    $obj=[pscustomobject]@{Status=$(if($global:TestRunning){'Running'}else{'Stopped'})}
    $obj|Add-Member ScriptMethod WaitForStatus {param($State,$Timeout)}
    return $obj
}
function global:Stop-Service {param($Name) $global:TestRunning=$false}
function global:Start-Service {param($Name) if($global:FailNextStart){$global:FailNextStart=$false;throw 'Simulated start failure'};$global:TestRunning=$true}
function global:sc.exe { $global:TestBinary=$args[3];$global:LASTEXITCODE=0 }
function global:Get-ItemProperty { param($LiteralPath,$Name,$ErrorAction) return [pscustomobject]@{ImagePath=$global:TestBinary} }
function global:Invoke-RestMethod {
    param($Uri,$Headers,$TimeoutSec)
    return [pscustomobject]@{tag_name='2.0.0';assets=@([pscustomobject]@{name='zapret-discord-youtube-2.0.0.zip';browser_download_url='https://github.com/Flowseal/zapret-discord-youtube/releases/download/2.0.0/test.zip';digest=''})}
}
function global:Invoke-WebRequest {param([switch]$UseBasicParsing,$Uri,$OutFile,$TimeoutSec) [IO.File]::Copy((Join-Path $Fixture 'release.zip'),$OutFile)}
$config=Join-Path $Fixture 'config.json'
@{root=$old}|ConvertTo-Json|Set-Content -LiteralPath $config -Encoding UTF8
$global:FailNextStart=$true
$failed=$false
try { & $Updater -Root $old -Config $config | Out-Null } catch { if ($_ -notlike '*Simulated start failure*') { throw }; $failed=$true }
if (!$failed -or !$global:TestRunning -or $global:TestBinary -ne $global:OriginalBinary) { throw 'Service rollback failed' }
if ((Get-Content $config -Raw|ConvertFrom-Json).root -ne $old) { throw 'Config rollback failed' }
if (Test-Path -LiteralPath $new) { throw 'Failed release blocks retry' }
& $Updater -Root $old -Config $config | Out-Null
if (!$?) { throw 'Switch failed' }
if (!$global:TestRunning -or !$global:TestBinary.Contains($new+'\')) { throw 'Service was not switched' }
if ((Get-Content $config -Raw|ConvertFrom-Json).root -ne $new) { throw 'Config was not switched' }
if (Test-Path "$old/.zapret-gui-update.json") { throw 'Journal not removed' }
"#;
        let script_path = fixture.join("test.ps1");
        let prefix = format!(
            "$Fixture='{}'\n$Updater='{}'\n",
            fixture.to_string_lossy().replace('\'', "''"),
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("scripts/update-zapret.ps1")
                .to_string_lossy()
                .replace('\'', "''")
        );
        fs::write(&script_path, format!("{prefix}{script}")).unwrap();
        let result = super::super::backend::command("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(&script_path)
            .output()
            .unwrap();
        let _ = fs::remove_dir_all(&fixture);
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
