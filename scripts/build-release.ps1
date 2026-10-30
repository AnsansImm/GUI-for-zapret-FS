# Build into a separate Cargo cache; keep target/release as the portable package.
$ErrorActionPreference = 'Stop'
$taskProject = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $taskProject
try {
    cargo build --release --offline --target-dir target/cargo-cache
    if ($LASTEXITCODE -ne 0) { throw 'Build failed' }
    $taskRelease = Join-Path $taskProject 'target\release'
    $taskCache = Join-Path $taskProject 'target\cargo-cache\release'
    New-Item -ItemType Directory -Force -Path $taskRelease | Out-Null
    $taskExe = Join-Path $taskRelease 'zapret-gui.exe'
    $taskPrevious = Join-Path $taskRelease 'zapret-gui-previous.exe'
    if (Test-Path -LiteralPath $taskExe) {
        if (Test-Path -LiteralPath $taskPrevious) { Remove-Item -LiteralPath $taskPrevious }
        Move-Item -LiteralPath $taskExe -Destination $taskPrevious
    }
    Copy-Item -LiteralPath (Join-Path $taskCache 'zapret-gui.exe') -Destination $taskExe
    $taskNativeSource=Join-Path $taskCache 'zapret-native.exe'
    $taskNativeTarget=Join-Path $taskRelease 'zapret-native.exe'
    if (!(Test-Path -LiteralPath $taskNativeTarget) -or (Get-FileHash -LiteralPath $taskNativeSource).Hash -ne (Get-FileHash -LiteralPath $taskNativeTarget).Hash) {
        if (Test-Path -LiteralPath $taskNativeTarget) { Remove-Item -LiteralPath $taskNativeTarget }
        Copy-Item -LiteralPath $taskNativeSource -Destination $taskNativeTarget
    }
    Write-Output "Portable build: $taskRelease"
} finally { Pop-Location }

