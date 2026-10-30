use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
};
#[derive(Clone, Serialize, Deserialize)]
pub struct Target {
    pub name: String,
    pub url: String,
}
#[derive(Serialize, Deserialize)]
struct Strategy {
    name: String,
    args: String,
}
#[derive(Serialize, Deserialize)]
struct Plan {
    exe: String,
    strategies: Vec<Strategy>,
    targets: Vec<Target>,
    timeout: u32,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    range_bytes: u32,
    #[serde(default)]
    dpi_host: String,
}
#[derive(Default, Clone, Deserialize)]
pub struct Row {
    pub strategy: String,
    pub target: String,
    pub http: String,
    pub tls12: String,
    pub tls13: String,
    pub ping: String,
    #[serde(default)]
    pub detail: String,
}
#[derive(Default, Deserialize)]
pub struct Report {
    pub phase: String,
    pub message: String,
    pub completed: usize,
    pub total: usize,
    #[serde(default)]
    pub rows: Vec<Row>,
}
pub struct Tests {
    pub selected: std::collections::BTreeSet<String>,
    pub report: Report,
    pub timeout: u32,
    pub dpi: bool,
    pub range_bytes: u32,
    pub dpi_host: String,
    pub dir: PathBuf,
    pub rx: Option<mpsc::Receiver<Result<(), String>>>,
    pub confirm: bool,
    pub error: String,
}
impl Tests {
    pub fn new() -> Self {
        let dir = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join("runtime")
            .join("test-run");
        let mut s = Self {
            selected: Default::default(),
            report: Default::default(),
            timeout: 4,
            dpi: false,
            range_bytes: 32768,
            dpi_host: String::new(),
            dir,
            rx: None,
            confirm: false,
            error: String::new(),
        };
        s.poll();
        s
    }
    pub fn busy(&self) -> bool {
        self.rx.is_some() || self.external_running()
    }
    pub fn external_running(&self) -> bool {
        use std::os::windows::fs::OpenOptionsExt;
        let p = self.dir.join("runner.lock");
        p.exists()
            && fs::OpenOptions::new()
                .read(true)
                .write(true)
                .share_mode(0)
                .open(p)
                .is_err()
    }
    pub fn needs_recovery(&self) -> bool {
        fs::read(self.dir.join("journal.json"))
            .ok()
            .and_then(|s| serde_json::from_slice::<serde_json::Value>(&s).ok())
            .is_some_and(|v| v["restored"] != true)
    }
    pub fn poll(&mut self) {
        if let Ok(data) = fs::read(self.dir.join("report.json")) {
            if let Ok(report) = serde_json::from_slice(&data) {
                self.report = report;
            }
        }
        if let Some(rx) = &self.rx {
            if let Ok(result) = rx.try_recv() {
                self.rx = None;
                if let Err(e) = result {
                    self.error = e;
                }
            }
        }
    }
    pub fn cancel(&mut self) {
        if let Err(e) = fs::write(self.dir.join("cancel"), b"cancel") {
            self.error = e.to_string();
        }
    }
    pub fn start(&mut self, root: &Path) -> Result<(), String> {
        if self.busy() || self.needs_recovery() {
            return Err("Завершите предыдущий тест или восстановите состояние".into());
        }
        let strategies = self
            .selected
            .iter()
            .map(|name| {
                super::backend::strategy_args(root, name).map(|args| Strategy {
                    name: name.clone(),
                    args,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if strategies.is_empty() {
            return Err("Выберите стратегии".into());
        }
        let targets = if self.dpi {
            vec![Target {
                name: "DPI suite".into(),
                url: "https://example.org".into(),
            }]
        } else {
            targets(root)?
        };
        let plan = Plan {
            exe: root.join("bin/winws.exe").to_string_lossy().into_owned(),
            strategies,
            targets,
            timeout: self.timeout.clamp(1, 20),
            mode: if self.dpi { "dpi" } else { "standard" }.into(),
            range_bytes: self.range_bytes.clamp(16384, 1048576),
            dpi_host: self.dpi_host.trim().into(),
        };
        if let Some(parent) = self.dir.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        if self.dir.exists() {
            let archive = self.dir.with_file_name(format!(
                "test-report-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::rename(&self.dir, archive).map_err(|e| e.to_string())?;
        }
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        fs::write(
            self.dir.join("plan.json"),
            serde_json::to_vec_pretty(&plan).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        fs::write(
            self.dir.join("runner.ps1"),
            include_bytes!("../scripts/test-runner.ps1"),
        )
        .map_err(|e| e.to_string())?;
        self.report = Report::default();
        self.error.clear();
        self.spawn(false);
        Ok(())
    }
    pub fn recover(&mut self) {
        if !self.busy() {
            self.error.clear();
            if let Err(e) = fs::write(
                self.dir.join("runner.ps1"),
                include_bytes!("../scripts/test-runner.ps1"),
            ) {
                self.error = format!("Не удалось обновить помощник восстановления: {e}");
                return;
            }
            self.spawn(true);
        }
    }
    fn spawn(&mut self, recover: bool) {
        let dir = self.dir.clone();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let _ = fs::remove_file(dir.join("fatal.log"));
            let quote = |s: &str| format!("'{}'", s.replace('\'', "''"));
            let args = format!(
                "-NoProfile -ExecutionPolicy Bypass -File \"{}\" -JobDir \"{}\" {}",
                dir.join("runner.ps1").display(),
                dir.display(),
                if recover { "-Recover" } else { "" }
            );
            let script=format!("$ErrorActionPreference='Stop';$p=Start-Process powershell.exe -ArgumentList {} -WindowStyle Hidden -Verb RunAs -PassThru;$p.WaitForExit();$p.Refresh();exit $p.ExitCode",quote(&args));
            let result = super::backend::command("powershell.exe")
                .args(["-NoProfile", "-Command", &script])
                .output()
                .map_err(|e| e.to_string())
                .and_then(|o| {
                    if o.status.success() {
                        Ok(())
                    } else {
                        Err(format!(
                            "Помощник тестирования завершился с кодом {:?}: {}\n{}",
                            o.status.code(),
                            String::from_utf8_lossy(&o.stderr),
                            fs::read_to_string(dir.join("fatal.log")).unwrap_or_default()
                        ))
                    }
                });
            let _ = tx.send(result);
        });
    }
}
pub fn targets(root: &Path) -> Result<Vec<Target>, String> {
    let text = fs::read_to_string(root.join("utils/targets.txt"))
        .map_err(|e| format!("Не удалось прочитать utils/targets.txt: {e}"))?;
    let mut result = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let (name, value) = line.split_once('=').ok_or("Неверная строка targets.txt")?;
        let name = name.trim();
        let value = value.trim();
        if name.is_empty()
            || !name.chars().all(|c| c.is_alphanumeric() || c == '_')
            || !value.starts_with('"')
            || !value.ends_with('"')
            || value.len() < 3
        {
            return Err("Неверный формат targets.txt".into());
        }
        let url = &value[1..value.len() - 1];
        if !(url.starts_with("https://") || url.starts_with("http://") || url.starts_with("PING:"))
        {
            return Err("Допустимы HTTP/HTTPS и PING".into());
        }
        if let Some(t) = result.iter_mut().find(|t: &&mut Target| t.name == name) {
            t.url = url.into();
        } else {
            result.push(Target {
                name: name.into(),
                url: url.into(),
            });
        }
    }
    if result.is_empty() {
        return Err("В targets.txt нет целей".into());
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shipped_targets_and_runner_syntax() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("zapret-discord-youtube-1.10.4");
        if root.exists() {
            let t = targets(&root).unwrap();
            assert!(t.len() > 10);
        }
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/test-runner.ps1");
        let script=format!("$t=$null;$e=$null;[System.Management.Automation.Language.Parser]::ParseFile('{}',[ref]$t,[ref]$e)|Out-Null;if($e.Count){{$e|Out-String|Write-Output;exit 1}}",path.display());
        let o = super::super::backend::command("powershell.exe")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    }
}

#[cfg(test)]
mod plan_tests {
    #[test]
    fn runner_validates_without_touching_service() {
        use super::*;
        let dir = std::env::temp_dir().join(format!(
            "zapret-plan-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let plan = Plan {
            exe: "C:\\not-executed\\winws.exe".into(),
            strategies: vec![Strategy {
                name: "test.bat".into(),
                args: "--test".into(),
            }],
            targets: vec![Target {
                name: "Example".into(),
                url: "https://example.org".into(),
            }],
            timeout: 4,
            mode: String::new(),
            range_bytes: 32768,
            dpi_host: String::new(),
        };
        fs::write(dir.join("plan.json"), serde_json::to_vec(&plan).unwrap()).unwrap();
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/test-runner.ps1");
        let out = super::super::backend::command("powershell.exe")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(script)
            .arg("-JobDir")
            .arg(&dir)
            .arg("-ValidateOnly")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(!dir.join("journal.json").exists());
        assert!(!dir.join("report.json").exists());
        fs::remove_file(dir.join("plan.json")).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}

#[cfg(test)]
mod journal_tests {
    #[test]
    fn repeated_journal_writes_in_windows_powershell() {
        use std::{fs, path::Path};
        let dir = std::env::temp_dir().join(format!(
            "zapret-journal-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let runner = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/test-runner.ps1");
        let script = format!(
            r#"$ErrorActionPreference='Stop';$JobDir='{}';$utf8=New-Object System.Text.UTF8Encoding($false);$t=$null;$e=$null;$ast=[System.Management.Automation.Language.Parser]::ParseFile('{}',[ref]$t,[ref]$e);$f=$ast.Find({{param($n)$n -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq 'Write-Json'}},$true);Invoke-Expression $f.Extent.Text;1..5|ForEach-Object{{Write-Json 'journal.json' @{{restored=($_ -eq 5);iteration=$_}}}}"#,
            dir.display(),
            runner.display()
        );
        let out = super::super::backend::command("powershell.exe")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let data: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("journal.json")).unwrap()).unwrap();
        assert_eq!(data["restored"], true);
        assert_eq!(data["iteration"], 5);
        for e in fs::read_dir(&dir).unwrap() {
            fs::remove_file(e.unwrap().path()).unwrap();
        }
        fs::remove_dir(dir).unwrap();
    }
}

#[cfg(test)]
mod queue_tests {
    #[test]
    fn failed_strategy_does_not_abort_queue() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/test-runner.ps1");
        let script = format!(
            r#"
$ErrorActionPreference='Stop'
$t=$null;$e=$null
$ast=[System.Management.Automation.Language.Parser]::ParseFile('{}',[ref]$t,[ref]$e)
$loop=$ast.Find({{param($n)$n -is [System.Management.Automation.Language.ForEachStatementAst] -and $n.Variable.VariablePath.UserPath -eq 'strategy'}},$true)
function Cancelled{{$false}}
function Write-Json($name,$value){{}}
function Stop-TestProcess{{}}
function Start-Sleep{{}}
function Start-Process{{throw 'Simulated startup failure'}}
$JobDir=$env:TEMP
$plan=@{{strategies=@(@{{name='ALT11'}},@{{name='ALT13'}},@{{name='ALT3'}});targets=@(@{{name='target'}})}}
$report=@{{rows=@();completed=0;message=''}}
$script:journal=@{{test_args=''}}
$strategyErrors=0;$strategyIndex=0
Invoke-Expression $loop.Extent.Text
if($report.rows.Count -ne 3 -or $report.completed -ne 3 -or $strategyErrors -ne 3){{throw 'Queue stopped early'}}
"#,
            path.display()
        );
        let o = super::super::backend::command("powershell.exe")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    }
}

#[cfg(test)]
mod dpi_tests {
    #[test]
    fn dpi_status_and_ipset_restore() {
        let dir = std::env::temp_dir().join(format!(
            "zapret-dpi-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let runner =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/test-runner.ps1");
        let script = format!(
            r#"
$ErrorActionPreference='Stop';$t=$null;$e=$null
$ast=[System.Management.Automation.Language.Parser]::ParseFile('{}',[ref]$t,[ref]$e)
foreach($name in @('Get-DpiStatus','Restore-State','Convert-DpiSuite')){{$f=$ast.Find({{param($n)$n -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq $name}},$true);Invoke-Expression $f.Extent.Text}}
$entries=@([pscustomobject]@{{host='one.example';provider='One';id='1'}},[pscustomobject]@{{host='two.example';provider='Two';id='2'}})
$flat=@(Convert-DpiSuite $entries);$nested=@(Convert-DpiSuite (, $entries))
if($flat.Count -ne 2 -or $nested.Count -ne 2 -or $nested[1].url -ne 'https://two.example'){{throw 'DPI suite array flattening'}}
if((Get-DpiStatus '000 32768 0 5.000' 28 5) -ne 'LIKELY_BLOCKED'){{throw 'freeze classification'}}
if((Get-DpiStatus '200 32768 100 0.200' 0 5) -ne 'OK'){{throw 'success classification'}}
if((Get-DpiStatus 'unsupported' 35 5) -ne 'UNSUP'){{throw 'unsupported classification'}}
if((Get-DpiStatus '000 0 0 5.000' 28 5) -ne 'ERROR'){{throw 'connection timeout classification'}}
function Stop-TestProcess{{}}
function Write-Json($name,$value){{}}
$path=Join-Path '{}' 'ipset.txt';[IO.File]::WriteAllBytes($path,(New-Object byte[] 0))
$script:journal=@{{ipset_path=$path;ipset_data=[Convert]::ToBase64String([byte[]](1,2,3,255));service_running=$false;standalone=@();restored=$false}}
Restore-State
if([Convert]::ToBase64String([IO.File]::ReadAllBytes($path)) -ne $script:journal.ipset_data -or -not $script:journal.restored){{throw 'ipset restore'}}
"#,
            runner.display(),
            dir.display()
        );
        let out = super::super::backend::command("powershell.exe")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        std::fs::remove_file(dir.join("ipset.txt")).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
