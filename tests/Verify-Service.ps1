$ErrorActionPreference='Stop'
. (Join-Path (Split-Path -Parent $PSScriptRoot) 'scripts/service-control.ps1') -FunctionsOnly
$passed=0
function Assert($condition,$message){if(!$condition){throw $message};$script:passed++}
function Fails([scriptblock]$body,[string]$message){try{& $body | Out-Null}catch{Assert ($_.Exception.Message.Contains($message)) ('Unexpected failure: '+$_.Exception.Message);return};throw 'Expected failure'}
function Reset-Case([string]$status='Stopped'){
 $script:service=[pscustomobject]@{Status=$status}
 $script:service | Add-Member ScriptMethod WaitForStatus {param($wanted,$timeout) if($script:timeout){throw 'Timeout waiting for service'};$this.Status=$wanted}
 $script:service | Add-Member ScriptMethod Refresh {if($script:crash){$this.Status='Stopped'}}
 $script:exe='C:\Zapret Test\bin\winws.exe';$script:expected='C:\Zapret Test\bin\WinDivert64.sys'
 $script:command='"'+$script:exe+'" --filter-tcp=443'
 $script:files=@($script:exe,$script:expected)
 $script:driverPresent=$true;$script:driverPath='\??\C:\missing\WinDivert64.sys';$script:driverState='Stopped'
 $script:starts=0;$script:stops=0;$script:configs=0;$script:timeout=$false;$script:crash=$false;$script:startFail=$false;$script:configCode=0;$script:changeCode=0;$script:events=@();$script:changedBinary=''
}
# All system calls are replaced in this script scope. No real service or registry is changed.
function Get-Service {param($Name,$ErrorAction) return $script:service}
function Start-Service {param($Name,$ErrorAction) $script:starts++;if($script:startFail){throw 'Start rejected'}}
function Stop-Service {param($Name,$ErrorAction) $script:stops++}
function Start-Sleep {param($Milliseconds)}
function Test-Path {param($LiteralPath,$PathType) return $LiteralPath -in $script:files}
function Get-ItemProperty {
 param($LiteralPath,$Name,$ErrorAction)
 if($LiteralPath.EndsWith('\zapret')){return [pscustomobject]@{ImagePath=$script:command}}
 if($script:driverPresent){return [pscustomobject]@{ImagePath=$script:driverPath}}
}
function Get-CimInstance {param($ClassName,$Filter,$OperationTimeoutSec,$ErrorAction) if($ClassName -eq 'Win32_SystemDriver'){return [pscustomobject]@{State=$script:driverState}};return [pscustomobject]@{Name='zapret'}}
function Invoke-CimMethod {param($InputObject,$MethodName,$Arguments,$ErrorAction) $script:changedBinary=$Arguments.PathName;Assert ($Arguments.StartMode -eq 'Automatic') 'Wrong startup mode';return [pscustomobject]@{ReturnValue=$script:changeCode}}
function Get-WinEvent {param($FilterHashtable,$MaxEvents,$ErrorAction) return $script:events}
function sc.exe {param($operation,$name,$option,$value) $script:configs++;$global:LASTEXITCODE=$script:configCode;if(!$script:configCode){$script:driverPath=$value};return 'mock sc config'}

Reset-Case
$result=Start-Zapret
Assert ($script:starts -eq 1) 'Start was not requested'
Assert ($script:configs -eq 1 -and $script:driverPath -eq ('\??\'+$script:expected)) 'Invalid driver path not repaired'
Assert ($script:service.Status -eq 'Running' -and $result.Contains('запущена')) 'Running state not verified'
Reset-Case
$script:driverPath='\??\C:\Other\WinDivert64.sys';$script:files+= 'C:\Other\WinDivert64.sys'
Start-Zapret | Out-Null
Assert ($script:configs -eq 0 -and $script:starts -eq 1) 'Valid driver path should be preserved'
Reset-Case
$script:driverPath='C:\Other\WinDivert64.sys';$script:files+='C:\Other\WinDivert64.sys'
Start-Zapret | Out-Null
Assert ($script:driverPath -eq '\??\C:\Other\WinDivert64.sys') 'Missing NT path prefix was not repaired'
Reset-Case
$script:driverPresent=$false
Start-Zapret | Out-Null
Assert ($script:configs -eq 0 -and $script:starts -eq 1) 'Missing driver registration must be left to winws'
Reset-Case
$script:driverState='Running'
Fails {Start-Zapret} 'драйвер не остановлен'
Assert ($script:configs -eq 0 -and $script:starts -eq 0) 'Active driver was changed'
Reset-Case
$script:files=@($script:exe)
Fails {Start-Zapret} 'Не найден'
Assert ($script:starts -eq 0) 'Started without a valid driver'
Reset-Case
$script:files=@()
Fails {Start-Zapret} 'Не найден файл службы'
Reset-Case
$script:service=$null
Fails {Start-Zapret} 'Служба не установлена'
Reset-Case 'Running'
Assert ((Start-Zapret).Contains('уже работает') -and $script:starts -eq 0) 'Running service was restarted'
Reset-Case 'StartPending'
Start-Zapret | Out-Null
Assert ($script:starts -eq 0 -and $script:service.Status -eq 'Running') 'Pending start was not awaited'
Reset-Case 'StopPending'
Start-Zapret | Out-Null
Assert ($script:starts -eq 1) 'Pending stop was not awaited before starting'
Reset-Case
$script:timeout=$true
Fails {Start-Zapret} 'Timeout'
Reset-Case
$script:crash=$true
Fails {Start-Zapret} 'завершилась сразу'
Reset-Case
$script:startFail=$true;$script:events=@([pscustomobject]@{Message='WinDivert: driver error 123'})
Fails {Start-Zapret} 'WinDivert: driver error 123'
Reset-Case
$script:configCode=5
Fails {Start-Zapret} 'Не удалось исправить путь'
Assert ($script:starts -eq 0) 'Started after driver repair failed'
Reset-Case 'Running'
Stop-Zapret | Out-Null
Assert ($script:stops -eq 1 -and $script:service.Status -eq 'Stopped') 'Stop was not awaited'
Reset-Case
Stop-Zapret | Out-Null
Assert ($script:stops -eq 0) 'Stopped service was stopped again'
Reset-Case
$script:command='"C:\Zapret New\bin\winws.exe" --hostlist="C:\Zapret New\lists\user.txt"'
Set-ZapretConfiguration $script:command
Assert ($script:changedBinary -ceq $script:command) 'Quoted command changed during service configuration'
$script:changeCode=2
Fails {Set-ZapretConfiguration $script:command} 'Код: 2'
Write-Output "PASS: $passed service checks (mocked; no real services changed)"
exit 0
