$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
$fixture = Join-Path $project ('bin/verification/scripts-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture -Force | Out-Null
$passed = 0
function Assert($condition, $message) {
    if (!$condition) { throw $message }
    $script:passed++
}
try {
    foreach ($file in Get-ChildItem (Join-Path $project 'scripts/*.ps1')) {
        $tokens = $null; $errors = $null
        [System.Management.Automation.Language.Parser]::ParseFile($file.FullName, [ref]$tokens, [ref]$errors) | Out-Null
        Assert ($errors.Count -eq 0) ('Syntax errors in ' + $file.Name)
    }
    # Exercise the timestamps check with fake netsh output; never change system settings.
    $tokens = $null; $errors = $null
    $diagnosticsAst = [System.Management.Automation.Language.Parser]::ParseFile((Join-Path $project 'scripts/diagnostics.ps1'), [ref]$tokens, [ref]$errors)
    $check = $diagnosticsAst.Find({param($node) $node -is [System.Management.Automation.Language.CommandAst] -and $node.GetCommandName() -eq 'Check' -and $node.CommandElements[1].Value -eq 'TCP timestamps'}, $true)
    $body = $check.CommandElements[2].ScriptBlock.GetScriptBlock()
    foreach ($case in @(
        @('TCP timestamps : enabled', 'Passed'),
        @('TCP timestamps : disabled', 'Disabled'),
        @('TCP timestamps : allowed', 'Unknown'),
        @('Unrecognized output', 'Unknown'),
        @('TCP timestamps : default', 'Unknown')
    )) {
        $actual = & {
            function netsh { $global:LASTEXITCODE = 0; $case[0] }
            function Add($title, $state, $detail) { $state }
            & $body
        }
        Assert ($actual -eq $case[1]) ('Incorrect timestamps state for ' + $case[0])
    }
    $job = Join-Path $fixture 'job'
    New-Item -ItemType Directory -Path $job | Out-Null
    $plan = @{ exe='unused.exe'; strategies=@(@{name='test';args='--filter-tcp=443'}); targets=@(@{name='test';url='https://example.org'}) }
    $planPath = Join-Path $job 'plan.json'
    $plan | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $planPath -Encoding UTF8
    $runner = Join-Path $project 'scripts/test-runner.ps1'
    & powershell.exe -NoProfile -File $runner -JobDir $job -ValidateOnly | Out-Null
    Assert ($LASTEXITCODE -eq 0) 'Valid test plan was rejected'
    $plan.targets[0].url = 'file:///C:/example'
    $plan | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $planPath -Encoding UTF8
    $ErrorActionPreference = 'Continue'
    & powershell.exe -NoProfile -File $runner -JobDir $job -ValidateOnly *> (Join-Path $fixture 'invalid-plan.log')
    $ErrorActionPreference = 'Stop'
    Assert ($LASTEXITCODE -ne 0) 'Invalid test plan was accepted'

    # PrepareOnly exercises archive validation and list preservation without services or network.
    $old = Join-Path $fixture 'old'
    $release = Join-Path $fixture 'release'
    foreach ($folder in @($old, $release)) {
        foreach ($sub in @('bin','lists','utils')) { New-Item -ItemType Directory -Path (Join-Path $folder $sub) -Force | Out-Null }
        Set-Content -LiteralPath (Join-Path $folder 'service.bat') -Value 'set LOCAL_VERSION=1.2.3'
    }
    $userList = 'list-general-user.txt'
    [IO.File]::WriteAllText((Join-Path $old ('lists/' + $userList)), "custom.example`n")
    [IO.File]::WriteAllText((Join-Path $old 'lists/custom.txt'), "extra.example`n")
    [IO.File]::WriteAllText((Join-Path $release ('lists/' + $userList)), "upstream.example`n")
    [IO.File]::WriteAllText((Join-Path $release 'lists/list-general.txt'), "release.example`n")
    [IO.File]::WriteAllBytes((Join-Path $release 'bin/winws.exe'), [byte[]]@(0))
    $archive = Join-Path $fixture 'release.zip'
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [IO.Compression.ZipFile]::CreateFromDirectory($release, $archive)
    $config = Join-Path $fixture 'config.json'
    $configText = @{root=$old} | ConvertTo-Json
    [IO.File]::WriteAllText($config, $configText)
    & (Join-Path $project 'scripts/update-zapret.ps1') -Root $old -Config $config -PrepareOnly -Archive $archive -Version '1.2.3' | Out-Null
    $new = Join-Path $fixture 'zapret-discord-youtube-1.2.3'
    Assert ([IO.File]::ReadAllText((Join-Path $new ('lists/' + $userList))) -eq "custom.example`n") 'User list was overwritten'
    Assert ([IO.File]::ReadAllText((Join-Path $new 'lists/custom.txt')) -eq "extra.example`n") 'Custom list was lost'
    Assert ([IO.File]::ReadAllText((Join-Path $new 'lists/list-general.txt')) -eq "release.example`n") 'Upstream list was lost'
    Assert ([IO.File]::ReadAllText($config) -eq $configText) 'PrepareOnly changed configuration'
    Write-Output "PASS: $passed script checks"
} finally {
    $resolved = [IO.Path]::GetFullPath($fixture)
    $allowed = [IO.Path]::GetFullPath((Join-Path $project 'bin/verification')) + [IO.Path]::DirectorySeparatorChar
    if ($resolved.StartsWith($allowed, [StringComparison]::OrdinalIgnoreCase)) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
# The rejected-plan test intentionally leaves LASTEXITCODE=1. Report the suite result.
exit 0
