param([string]$OutputRoot, [string]$RuntimeVersion = '8.0.31', [switch]$Offline)
$ErrorActionPreference = 'Stop'
$taskProject = Split-Path -Parent $PSScriptRoot
$taskCsproj = Join-Path $taskProject 'gui-for-zapret-fs.csproj'
[xml]$taskXml = Get-Content -LiteralPath $taskCsproj -Raw
$taskVersion = [string]$taskXml.Project.PropertyGroup.Version
if ($taskVersion -notmatch '^\d+\.\d+\.\d+$') { throw 'Invalid project version' }
if (!$OutputRoot) { $OutputRoot = Join-Path $taskProject 'dist' }
$taskOutput = Join-Path ([IO.Path]::GetFullPath($OutputRoot)) $taskVersion
$taskStage = Join-Path $taskProject ('bin/release-build/' + [Guid]::NewGuid().ToString('N'))
$taskPackage = Join-Path $taskStage 'gui-for-zapret-fs'
New-Item -ItemType Directory -Path $taskPackage, $taskOutput -Force | Out-Null
Push-Location -LiteralPath $taskProject
try {
    $taskSource = 'https://api.nuget.org/v3/index.json'
    if ($Offline) {
        $taskCache = (dotnet nuget locals global-packages --list | Out-String).Trim() -replace '^[^:]+:\s*', ''
        if ($LASTEXITCODE -ne 0 -or !(Test-Path -LiteralPath $taskCache)) { throw 'NuGet package cache not found' }
        $taskSource = $taskCache
    }
    dotnet publish $taskCsproj -c Release -r win-x64 --self-contained true `
        -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true `
        -p:DebugType=None -p:DebugSymbols=false -p:RuntimeFrameworkVersion=$RuntimeVersion `
        -warnaserror --source $taskSource -o $taskPackage
    if ($LASTEXITCODE -ne 0) { throw 'Portable publish failed' }
    foreach ($taskFile in @('gui-for-zapret-fs.exe', 'scripts/test-runner.ps1', 'scripts/update-zapret.ps1', 'scripts/diagnostics.ps1')) {
        if (!(Test-Path -LiteralPath (Join-Path $taskPackage $taskFile) -PathType Leaf)) { throw "Missing release file: $taskFile" }
    }
    $taskInfo = (Get-Item -LiteralPath (Join-Path $taskPackage 'gui-for-zapret-fs.exe')).VersionInfo
    if ($taskInfo.ProductName -ne 'gui-for-zapret-fs' -or $taskInfo.ProductVersion -notlike "$taskVersion*") { throw 'EXE branding/version mismatch' }
    if (Test-Path -LiteralPath (Join-Path $taskPackage 'config.json')) { throw 'Personal configuration must not be packaged' }
    $taskReadme = @"
gui-for-zapret-fs $taskVersion - Windows x64 portable

Распакуйте всю папку и запустите gui-for-zapret-fs.exe.
Оставьте папку scripts рядом с EXE.
.NET $RuntimeVersion включён в EXE, отдельная установка .NET не требуется.
Выберите существующую папку Flowseal Zapret в интерфейсе.
Сам Zapret, драйвер и стратегии скачиваются отдельно.
Настройки и отчёты сохраняются рядом с EXE: папка должна быть доступна для записи.
"@
    [IO.File]::WriteAllText((Join-Path $taskPackage 'README.txt'), $taskReadme, (New-Object Text.UTF8Encoding($true)))
    $taskZip = Join-Path $taskOutput 'gui-for-zapret-fs-win-x64-portable.zip'
    Compress-Archive -LiteralPath $taskPackage -DestinationPath $taskZip -Force
    $taskHash = (Get-FileHash -LiteralPath $taskZip -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText(($taskZip + '.sha256'), ($taskHash + '  ' + [IO.Path]::GetFileName($taskZip) + "`n"), [Text.Encoding]::ASCII)
    Write-Output "Release ZIP: $taskZip"
    Write-Output "SHA256: $taskHash"
} finally { Pop-Location }
