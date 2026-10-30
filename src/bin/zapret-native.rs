#[path = "../list_store.rs"]
mod list_store;
use std::os::windows::process::CommandExt;
use std::{
    fs,
    io::{self, Read, Write},
    process::{Command, Stdio},
};
fn handle(v: serde_json::Value) -> Result<serde_json::Value, String> {
    let action = v["action"].as_str().ok_or("Нет команды")?;
    let directory = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .unwrap()
        .to_owned();
    if action == "open_gui" {
        let exe = directory.join("zapret-gui.exe");
        if !exe.is_file() {
            return Err("Не найден zapret-gui.exe рядом с помощником".into());
        }
        let path = exe.to_string_lossy().replace('\'', "''");
        let script = format!(
            r#"$ErrorActionPreference='Stop';$p=Get-Process -Name zapret-gui -ErrorAction SilentlyContinue|Where-Object{{$_.Path -eq '{path}' -and $_.MainWindowHandle -ne 0}}|Select-Object -First 1;if($p){{Add-Type 'using System;using System.Runtime.InteropServices;public class ZWindow{{[DllImport("user32.dll")] public static extern bool ShowWindowAsync(IntPtr h,int n);[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);}}';[ZWindow]::ShowWindowAsync($p.MainWindowHandle,9)|Out-Null;[ZWindow]::SetForegroundWindow($p.MainWindowHandle)|Out-Null}}else{{Start-Process -FilePath '{path}' -WorkingDirectory '{directory}'}}"#,
            directory = directory.to_string_lossy().replace('\'', "''")
        );
        let o = Command::new("powershell.exe")
            .creation_flags(0x08000000)
            .args(["-NoProfile", "-Command", &script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| e.to_string())?;
        if !o.success() {
            return Err("Не удалось открыть GUI".into());
        }
        return Ok(serde_json::json!({"ok":true}));
    }
    let config: serde_json::Value = serde_json::from_slice(
        &fs::read(directory.join("config.json"))
            .map_err(|_| "Сначала откройте программу и выберите папку Zapret")?,
    )
    .map_err(|e| e.to_string())?;
    let root = config["root"]
        .as_str()
        .ok_or("В программе не выбрана папка Zapret")?;
    let domain = v["domain"]
        .as_str()
        .ok_or("Нет домена")?
        .to_ascii_lowercase();
    let state = list_store::handle(std::path::Path::new(root), &domain, action)?;
    Ok(serde_json::json!({"ok":true,"domain":domain,"state":state}))
}
fn main() {
    // Native Messaging requires an ordinary console-subsystem executable: stdout
    // carries only length-prefixed JSON. No console is created by the browser.
    let mut input = io::stdin().lock();
    let mut header = [0u8; 4];
    if input.read_exact(&mut header).is_err() {
        return;
    }
    let len = u32::from_le_bytes(header) as usize;
    if len > 65536 {
        return;
    }
    let mut bytes = vec![0; len];
    if input.read_exact(&mut bytes).is_err() {
        return;
    }
    let result = serde_json::from_slice(&bytes)
        .map_err(|e| e.to_string())
        .and_then(handle)
        .unwrap_or_else(|e| serde_json::json!({"ok":false,"error":e}));
    if let Ok(data) = serde_json::to_vec(&result) {
        let mut output = io::stdout().lock();
        let _ = output.write_all(&(data.len() as u32).to_le_bytes());
        let _ = output.write_all(&data);
        let _ = output.flush();
    }
}
