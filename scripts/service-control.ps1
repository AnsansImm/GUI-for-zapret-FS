param([ValidateSet('start','stop')][string]$Action='start',[switch]$FunctionsOnly)
$ErrorActionPreference='Stop'

function Get-ZapretExecutable {
    $binary=[Environment]::ExpandEnvironmentVariables([string](Get-ItemProperty -LiteralPath 'HKLM:\SYSTEM\CurrentControlSet\Services\zapret' -Name ImagePath -ErrorAction Stop).ImagePath)
    $match=[regex]::Match($binary, '^\s*"(?<exe>[^"]+)"|^\s*(?<exe>.+?\.exe)(?:\s|$)', 'IgnoreCase')
    if(!$match.Success){throw 'Не удалось прочитать путь запуска службы Zapret.'}
    $exe=[IO.Path]::GetFullPath($match.Groups['exe'].Value)
    if(!(Test-Path -LiteralPath $exe -PathType Leaf)){throw "Не найден файл службы: $exe. Выберите актуальную папку Zapret и примените стратегию заново."}
    return $exe
}
function Repair-ZapretDriverPath([string]$Exe) {
    $key='HKLM:\SYSTEM\CurrentControlSet\Services\WinDivert'
    $driver=Get-ItemProperty -LiteralPath $key -Name ImagePath -ErrorAction SilentlyContinue
    if(!$driver){return ''} # winws installs the driver on first launch.
    $actual=[Environment]::ExpandEnvironmentVariables([string]$driver.ImagePath)
    $kernelPath=$false
    if($actual.StartsWith('\??\')){$actual=$actual.Substring(4);$kernelPath=$true}
    if($actual.StartsWith('\SystemRoot\',[StringComparison]::OrdinalIgnoreCase)){$actual=Join-Path $env:SystemRoot $actual.Substring(12);$kernelPath=$true}
    $exists=Test-Path -LiteralPath $actual -PathType Leaf -ErrorAction SilentlyContinue
    # A valid driver from another directory is not replaced automatically.
    if($exists -and $kernelPath){return ''}
    $expected=if($exists){[IO.Path]::GetFullPath($actual)}else{Join-Path (Split-Path -Parent $Exe) 'WinDivert64.sys'}
    if(!(Test-Path -LiteralPath $expected -PathType Leaf)){throw "Неверный путь WinDivert: $($driver.ImagePath). Не найден $expected. Восстановите файлы Zapret."}
    $state=Get-CimInstance Win32_SystemDriver -Filter "Name='WinDivert'" -OperationTimeoutSec 8 -ErrorAction Stop
    if(!$state -or $state.State -ne 'Stopped'){throw "WinDivert использует недействительный путь, но драйвер не остановлен. Остановите использующие его обходы и повторите запуск. Путь: $($driver.ImagePath)"}
    $target='\??\'+$expected
    $output=(& sc.exe config WinDivert binPath= $target 2>&1 | Out-String)
    if($LASTEXITCODE -ne 0){throw ('Не удалось исправить путь WinDivert: '+$output.Trim())}
    $saved=[string](Get-ItemProperty -LiteralPath $key -Name ImagePath -ErrorAction Stop).ImagePath
    if($saved -ne $target){throw 'Windows не сохранила исправленный путь WinDivert.'}
    return ('Исправлен недействительный путь WinDivert: '+$expected)
}
function Set-ZapretConfiguration([string]$Binary) {
    $service=Get-CimInstance Win32_Service -Filter "Name='zapret'" -OperationTimeoutSec 8 -ErrorAction Stop
    if(!$service){throw 'Служба Zapret не установлена.'}
    $result=Invoke-CimMethod -InputObject $service -MethodName Change -Arguments @{PathName=$Binary;StartMode='Automatic'} -ErrorAction Stop
    if($result.ReturnValue -ne 0){throw ('Windows не применила стратегию к службе. Код: '+$result.ReturnValue)}
}
function Get-ZapretFailure([datetime]$Since) {
    $messages=@(Get-WinEvent -FilterHashtable @{LogName='System';ProviderName='Service Control Manager';StartTime=$Since;Id=@(7000,7009,7011,7023,7024,7031,7034)} -MaxEvents 20 -ErrorAction SilentlyContinue |
        Where-Object {$_.Message -match '(?i)zapret|WinDivert'} | Select-Object -First 3 -ExpandProperty Message)
    return ($messages -join "`n").Trim()
}
function Start-Zapret {
    $since=[DateTime]::Now.AddSeconds(-2)
    try {
        $service=Get-Service -Name zapret -ErrorAction SilentlyContinue
        if(!$service){throw 'Служба не установлена. Выберите стратегию и нажмите «Применить стратегию к службе».'}
        if($service.Status -eq 'Running'){return 'Служба Zapret уже работает. Для смены стратегии нажмите «Применить стратегию к службе».'}
        if($service.Status -eq 'StopPending'){$service.WaitForStatus('Stopped',[TimeSpan]::FromSeconds(20));$service.Refresh()}
        $repair=''
        if($service.Status -eq 'Stopped'){
            $exe=Get-ZapretExecutable
            $repair=Repair-ZapretDriverPath $exe
            Start-Service -Name zapret -ErrorAction Stop
        }elseif($service.Status -ne 'StartPending'){throw ('Нельзя запустить службу в состоянии '+$service.Status+'.')}
        $service.WaitForStatus('Running',[TimeSpan]::FromSeconds(20))
        Start-Sleep -Milliseconds 500
        $service.Refresh()
        if($service.Status -ne 'Running'){throw ('Служба завершилась сразу после запуска. Состояние: '+$service.Status)}
        return (($repair+"`nСлужба Zapret запущена.").Trim())
    }catch{
        $reason=$_.Exception.Message
        $details=Get-ZapretFailure $since
        throw ("Не удалось запустить службу Zapret: "+$reason+$(if($details){"`n"+$details}else{''}))
    }
}
function Stop-Zapret {
    $service=Get-Service -Name zapret -ErrorAction SilentlyContinue
    if(!$service){throw 'Служба Zapret не установлена.'}
    if($service.Status -eq 'Stopped'){return 'Служба Zapret уже остановлена.'}
    Stop-Service -Name zapret -ErrorAction Stop
    $service.WaitForStatus('Stopped',[TimeSpan]::FromSeconds(20))
    $service.Refresh()
    if($service.Status -ne 'Stopped'){throw 'Не удалось дождаться остановки службы Zapret.'}
    return 'Служба Zapret остановлена.'
}
if(!$FunctionsOnly){if($Action -eq 'start'){Start-Zapret}else{Stop-Zapret}}
