param(
    [Parameter(Mandatory=$true)][string]$Root,
    [Parameter(Mandatory=$true)][string]$Config,
    [switch]$PrepareOnly,
    [string]$Archive,
    [string]$Version
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = New-Object Text.UTF8Encoding($false)
Add-Type -AssemblyName System.IO.Compression.FileSystem

function Get-Sha256($Path) {
    $stream = [IO.File]::OpenRead($Path)
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-', '') } finally { $sha.Dispose(); $stream.Dispose() }
}
function Expand-SafeZip($Zip, $Destination) {
    $base = [IO.Path]::GetFullPath($Destination) + [IO.Path]::DirectorySeparatorChar
    $reader = [IO.Compression.ZipFile]::OpenRead($Zip)
    try {
        foreach ($entry in $reader.Entries) {
            $path = [IO.Path]::GetFullPath((Join-Path $Destination $entry.FullName))
            if (!$path.StartsWith($base, [StringComparison]::OrdinalIgnoreCase) -or $entry.FullName.Contains(':')) { throw 'Unsafe archive path' }
        }
    } finally { $reader.Dispose() }
    [IO.Compression.ZipFile]::ExtractToDirectory($Zip, $Destination)
}
function Invoke-Sc([string[]]$Arguments) {
    $result = & sc.exe @Arguments 2>&1
    if ($LASTEXITCODE -ne 0) { throw ($result -join "`n") }
}
function Set-ServiceBinary($Binary) {
    Invoke-Sc -Arguments @('config', 'zapret', 'binPath=', $Binary)
}
function Start-Zapret {
    Start-Service zapret
    (Get-Service zapret).WaitForStatus('Running', [TimeSpan]::FromSeconds(20))
}

$Root = [IO.Path]::GetFullPath($Root).TrimEnd('\')
$Config = [IO.Path]::GetFullPath($Config)
if (!(Test-Path -LiteralPath (Join-Path $Root 'service.bat') -PathType Leaf)) { throw 'Zapret folder not found' }
$journal = Join-Path $Root '.zapret-gui-update.json'
# A previous interrupted switch must be rolled back before another update.
if (!$PrepareOnly -and (Test-Path -LiteralPath $journal)) {
    $old = Get-Content -LiteralPath $journal -Raw | ConvertFrom-Json
    if ($old.binary) {
        $svc = Get-Service zapret -ErrorAction SilentlyContinue
        if ($svc -and $svc.Status -ne 'Stopped') { Stop-Service zapret; $svc.WaitForStatus('Stopped', [TimeSpan]::FromSeconds(20)) }
        Set-ServiceBinary $old.binary
        if ($old.running) { Start-Zapret }
    }
    [IO.File]::WriteAllBytes($Config, [Convert]::FromBase64String($old.config))
    Remove-Item -LiteralPath $journal
}
$parent = Split-Path -Parent $Root
$stage = Join-Path $parent ('.zapret-update-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stage | Out-Null
$switched = $false
$committed = $false
$published = $false
try {
    if (!$PrepareOnly) {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        $release = Invoke-RestMethod -Uri 'https://api.github.com/repos/Flowseal/zapret-discord-youtube/releases/latest' -Headers @{'User-Agent'='Zapret-GUI';'Cache-Control'='no-cache'} -TimeoutSec 30
        $Version = ([string]$release.tag_name).TrimStart('v')
        if ($Version -notmatch '^\d+(\.\d+){1,3}$') { throw 'Unsupported release version' }
        $local = [regex]::Match([IO.File]::ReadAllText((Join-Path $Root 'service.bat')), 'LOCAL_VERSION=([0-9.]+)').Groups[1].Value
        if ($local -and [version]$Version -le [version]$local) { throw 'Последний релиз уже установлен либо ваша версия новее.' }
        $assets = @($release.assets | Where-Object { $_.name -eq "zapret-discord-youtube-$Version.zip" })
        if ($assets.Count -ne 1) { throw 'Release ZIP asset not found' }
        $asset = $assets[0]
        if ($asset.browser_download_url -notlike 'https://github.com/Flowseal/zapret-discord-youtube/releases/download/*') { throw 'Unexpected download URL' }
        $Archive = Join-Path $stage 'release.zip'
        Invoke-WebRequest -UseBasicParsing -Uri $asset.browser_download_url -OutFile $Archive -TimeoutSec 180
        if ($asset.digest -match '^sha256:([a-fA-F0-9]{64})$') {
            if ((Get-Sha256 $Archive) -ne $Matches[1]) { throw 'Release SHA256 mismatch' }
        }
    }
    if ($Version -notmatch '^\d+(\.\d+){1,3}$') { throw 'Unsupported release version' }
    $destination = Join-Path $parent "zapret-discord-youtube-$Version"
    if (Test-Path -LiteralPath $destination) { throw "Destination already exists: $destination" }
    $unpacked = Join-Path $stage 'unpacked'
    Expand-SafeZip $Archive $unpacked
    $roots = @(@(Get-Item -LiteralPath $unpacked) + @(Get-ChildItem -LiteralPath $unpacked -Directory) | Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName 'service.bat') -PathType Leaf })
    if ($roots.Count -ne 1) { throw 'Invalid release folder structure' }
    $new = $roots[0].FullName
    foreach ($required in @('bin/winws.exe', 'lists', 'utils')) {
        if (!(Test-Path -LiteralPath (Join-Path $new $required))) { throw "Release is missing $required" }
    }
    $releaseVersion = [regex]::Match([IO.File]::ReadAllText((Join-Path $new 'service.bat')), 'LOCAL_VERSION=([0-9.]+)').Groups[1].Value
    if ($releaseVersion -ne $Version) { throw 'Archive version does not match release' }
    # Copy every user list, including custom names and backups, byte for byte.
    $lists = Join-Path $Root 'lists'
    $lock = [IO.File]::Open((Join-Path $lists '.zapret-gui.lock'), 'OpenOrCreate', 'ReadWrite', 'None')
    try {
        if (Test-Path -LiteralPath (Join-Path $lists '.zapret-gui-transaction.json')) { throw 'List recovery required; open the list editor first' }
        $backup = Join-Path $stage 'user-lists-backup'
        New-Item -ItemType Directory -Path $backup | Out-Null
        foreach ($file in Get-ChildItem -LiteralPath $lists -File | Where-Object { $_.Name -like '*user*' -or $_.Name -like '*.backup' }) {
            Copy-Item -LiteralPath $file.FullName -Destination (Join-Path $backup $file.Name)
            Copy-Item -LiteralPath (Join-Path $backup $file.Name) -Destination (Join-Path $new ('lists/' + $file.Name)) -Force
            if ((Get-Sha256 $file.FullName) -ne (Get-Sha256 (Join-Path $new ('lists/' + $file.Name)))) { throw 'User list copy verification failed' }
        }
        # Preserve custom text lists too; upstream lists receive the release contents.
        foreach ($file in Get-ChildItem -LiteralPath $lists -File -Filter '*.txt') {
            $target = Join-Path $new ('lists/' + $file.Name)
            if (!(Test-Path -LiteralPath $target)) { Copy-Item -LiteralPath $file.FullName -Destination $target }
        }
        if ($PrepareOnly) { Move-Item -LiteralPath $new -Destination $destination; Write-Output $destination; return }
        $service = Get-CimInstance Win32_Service -Filter "Name='zapret'" -OperationTimeoutSec 8
        $servicePids = @(Get-CimInstance Win32_Service -Filter 'ProcessId > 0' -OperationTimeoutSec 8 | Select-Object -ExpandProperty ProcessId)
        if (@(Get-CimInstance Win32_Process -Filter "Name='winws.exe'" -OperationTimeoutSec 8 | Where-Object { $_.ProcessId -notin $servicePids }).Count) { throw 'Close the standalone Zapret window before updating' }
        $binary = if ($service) {
            # CIM may omit PathName; ImagePath is the persisted service command.
            [string](Get-ItemProperty -LiteralPath 'HKLM:\SYSTEM\CurrentControlSet\Services\zapret' -Name ImagePath -ErrorAction Stop).ImagePath
        } else { '' }
        $rootPattern = ((($Root -split '[\\/]') | ForEach-Object { [regex]::Escape($_) }) -join '[\\/]') + '[\\/]'
        if ($service) {
            $expanded = [Environment]::ExpandEnvironmentVariables($binary)
            $exeMatch = [regex]::Match($expanded, '^\s*"(?<exe>[^"]+)"|^\s*(?<exe>.+?\.exe)(?:\s|$)', 'IgnoreCase')
            if (!$exeMatch.Success) { throw 'Не удалось прочитать путь запуска службы Zapret.' }
            $serviceExe = [IO.Path]::GetFullPath($exeMatch.Groups['exe'].Value.Replace('/', '\'))
            $expectedExe = [IO.Path]::GetFullPath((Join-Path $Root 'bin/winws.exe'))
            if (!$serviceExe.Equals($expectedExe, [StringComparison]::OrdinalIgnoreCase)) {
                throw "Служба Zapret использует другую папку. Путь службы: $serviceExe. Выбранная папка: $Root. Выберите папку службы на вкладке Управление."
            }
        }
        $running = $service -and $service.State -eq 'Running'
        $configBytes = [IO.File]::ReadAllBytes($Config)
        $configValue = [Text.Encoding]::UTF8.GetString($configBytes).TrimStart([char]0xfeff) | ConvertFrom-Json
        @{binary=$binary;running=[bool]$running;config=[Convert]::ToBase64String($configBytes)} | ConvertTo-Json | Set-Content -LiteralPath $journal -Encoding UTF8
        $switched = $true
        if ($service -and $service.State -ne 'Stopped') { Stop-Service zapret; (Get-Service zapret).WaitForStatus('Stopped', [TimeSpan]::FromSeconds(20)) }
        Move-Item -LiteralPath $new -Destination $destination
        $published = $true
        if ($service) {
            $updatedBinary = [regex]::Replace([Environment]::ExpandEnvironmentVariables($binary), $rootPattern, [Text.RegularExpressions.MatchEvaluator]{param($m) $destination + '\'}, [Text.RegularExpressions.RegexOptions]::IgnoreCase)
            Set-ServiceBinary $updatedBinary
            if ($running) { Start-Zapret }
        }
        $configValue.root = $destination
        $configTemp = $Config + '.update.tmp'
        [IO.File]::WriteAllText($configTemp, ($configValue | ConvertTo-Json), (New-Object Text.UTF8Encoding($false)))
        [IO.File]::Replace($configTemp, $Config, (Join-Path $stage 'config-before-update.json'))
        Remove-Item -LiteralPath $journal
        $committed = $true
        Write-Output $destination
    } finally { $lock.Dispose() }
} catch {
    $failure = $_
    if ($switched -and !$committed) {
        try {
            if ($service) {
                $svc = Get-Service zapret
                if ($svc.Status -ne 'Stopped') { Stop-Service zapret; $svc.WaitForStatus('Stopped', [TimeSpan]::FromSeconds(20)) }
                Set-ServiceBinary $binary
                if ($running) { Start-Zapret }
            }
            [IO.File]::WriteAllBytes($Config, $configBytes)
            Remove-Item -LiteralPath $journal
            if ($published -and (Split-Path -Parent $destination) -eq $parent -and $destination -ne $Root) {
                Move-Item -LiteralPath $destination -Destination (Join-Path $stage 'failed-release')
            }
        } catch { throw "$failure; rollback failed: $_. Recovery journal: $journal" }
    }
    throw $failure
} finally {
    # Delete only the unique staging directory created by this invocation.
    if ((Split-Path -Parent $stage) -eq $parent -and (Split-Path -Leaf $stage) -match '^\.zapret-update-[a-f0-9]{32}$') {
        Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue
    }
}
