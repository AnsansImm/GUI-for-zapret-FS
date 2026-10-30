param([Parameter(Mandatory=$true)][string]$JobDir,[switch]$Recover,[switch]$ValidateOnly)
$ErrorActionPreference='Stop'
$utf8=New-Object System.Text.UTF8Encoding($false)
function Read-Json($name){Get-Content -LiteralPath (Join-Path $JobDir $name) -Raw -Encoding UTF8 | ConvertFrom-Json}
function Write-Json($name,$value){
 $path=Join-Path $JobDir $name
 $tmp=$path+'.tmp'
 [IO.File]::WriteAllText($tmp,(ConvertTo-Json -InputObject $value -Depth 12),$utf8)
 if(Test-Path -LiteralPath $path){[IO.File]::Replace($tmp,$path,($path+'.previous'))}else{[IO.File]::Move($tmp,$path)}
}
function Cancelled {Test-Path -LiteralPath (Join-Path $JobDir 'cancel')}
function Stop-TestProcess {
 # The command is journalled before launch. If the helper crashes before recording
 # the PID, locate only the exact executable/arguments created by this test.
 if($script:journal.test_args){
  $matches=@(Get-CimInstance Win32_Process -Filter "Name='winws.exe'" -OperationTimeoutSec 8 | Where-Object {
   $_.ExecutablePath -and [IO.Path]::GetFullPath($_.ExecutablePath).Equals([IO.Path]::GetFullPath($plan.exe),[StringComparison]::OrdinalIgnoreCase) -and (
    ($script:journal.test_pid -and $_.ProcessId -eq $script:journal.test_pid -and $script:journal.test_created -and $_.CreationDate.ToUniversalTime().ToString('o') -eq $script:journal.test_created) -or
    ($_.CommandLine -and $_.CommandLine.EndsWith($script:journal.test_args,[StringComparison]::Ordinal))
   )
  })
  foreach($p in $matches){
   if($script:journal.test_pid -and $script:journal.test_created -and $p.ProcessId -eq $script:journal.test_pid -and $p.CreationDate.ToUniversalTime().ToString('o') -ne $script:journal.test_created){continue}
   Stop-Process -Id $p.ProcessId -Force -ErrorAction Stop
   $wait=Get-Process -Id $p.ProcessId -ErrorAction SilentlyContinue
   if($wait){$wait.WaitForExit(5000)|Out-Null;if(-not $wait.HasExited){throw 'Test process did not stop'}}
  }
  $script:journal.test_pid=0
  $script:journal.test_args=''
  $script:journal.test_created=''
  Write-Json 'journal.json' $script:journal
 }
}function Restore-State {
 Stop-TestProcess
 if($script:journal.service_running){
  $s=Get-Service -Name zapret -ErrorAction Stop
  if($s.Status -ne 'Running'){Start-Service -Name zapret -ErrorAction Stop;$s=Get-Service zapret;$s.WaitForStatus('Running',[TimeSpan]::FromSeconds(20))}
 }
 $script:journal.restored=$true
 Write-Json 'journal.json' $script:journal
}
$plan=Read-Json 'plan.json'
if($ValidateOnly){
 if(-not $plan.exe -or @($plan.strategies).Count -eq 0 -or @($plan.targets).Count -eq 0){throw 'Empty test plan'}
 foreach($t in $plan.targets){if($t.url -notmatch '^https?://' -and $t.url -notmatch '^PING:'){throw 'Invalid target'}}
 'Plan valid';exit 0
}
$principal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if(-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Administrator rights required'}
# A file handle prevents two helpers from changing state at the same time.
$lock=[IO.File]::Open((Join-Path $JobDir 'runner.lock'),[IO.FileMode]::OpenOrCreate,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)
try {
 if($Recover){
  $script:journal=Read-Json 'journal.json'
  Restore-State
  Write-Json 'report.json' @{phase='recovered';message='Состояние восстановлено';completed=0;total=0;rows=@()}
  exit 0
 }
 $report=@{phase='running';message='Подготовка';completed=0;total=(@($plan.strategies).Count*@($plan.targets).Count);rows=@()}
 $script:journal=$null
 try {
  if(Test-Path -LiteralPath (Join-Path $JobDir 'journal.json')){throw 'Previous journal exists; recover first'}
  if(-not(Test-Path -LiteralPath $plan.exe)){throw 'winws.exe not found'}
  Get-Command curl.exe -ErrorAction Stop|Out-Null
  # Standalone winws may belong to another bypass. Do not stop or recreate it.
  $s=Get-Service -Name zapret -ErrorAction SilentlyContinue
  if($s -and $s.Status -notin @('Running','Stopped')){throw 'Zapret service is changing state; retry later'}
  $servicePid=0
  if($s -and $s.Status -eq 'Running'){
   $svc=Get-CimInstance Win32_Service -Filter "Name='zapret'" -OperationTimeoutSec 8
   $servicePid=$svc.ProcessId
  }
  $others=@(Get-CimInstance Win32_Process -Filter "Name='winws.exe'" -OperationTimeoutSec 8 | Where-Object {$_.ProcessId -ne $servicePid})
  if($others.Count -gt 0){throw 'Обнаружен отдельный процесс winws. Закройте запущенную вручную стратегию перед тестированием.'}
  $script:journal=@{service_running=[bool]($s -and $s.Status -eq 'Running');test_pid=0;test_created='';test_args='';restored=$false}
  Write-Json 'journal.json' $script:journal
  if($script:journal.service_running){Stop-Service zapret -ErrorAction Stop;$s=Get-Service zapret;$s.WaitForStatus('Stopped',[TimeSpan]::FromSeconds(20))}
  $strategyErrors=0
  $strategyIndex=0
  foreach($strategy in $plan.strategies){
   $strategyIndex++
   $targetIndex=0
   $stdout=$null;$stderr=$null
   try {
   if(Cancelled){break}
   $report.message='Запуск: '+$strategy.name;Write-Json 'report.json' $report
   # Persist executable and command before launching; recover also scans this exact command.
   $script:journal.test_args=$strategy.args
   Write-Json 'journal.json' $script:journal
   $stdout=Join-Path $JobDir ('strategy-'+$strategyIndex+'.stdout.log')
   $stderr=Join-Path $JobDir ('strategy-'+$strategyIndex+'.stderr.log')
   $process=Start-Process -FilePath $plan.exe -ArgumentList $strategy.args -WorkingDirectory (Split-Path $plan.exe) -PassThru -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr
   $script:journal.test_pid=$process.Id
   $procInfo=Get-CimInstance Win32_Process -Filter ('ProcessId='+$process.Id) -OperationTimeoutSec 8
   if(-not $procInfo){throw ('Стратегия завершилась сразу после запуска: '+$strategy.name)}
   $script:journal.test_created=$procInfo.CreationDate.ToUniversalTime().ToString('o')
   Write-Json 'journal.json' $script:journal
   Start-Sleep -Milliseconds 700
   if($process.HasExited){throw ('Стратегия завершилась: '+$strategy.name)}
   foreach($target in $plan.targets){
    if(Cancelled){break}
    if($process.HasExited){throw ('Процесс стратегии завершился во время теста: '+$strategy.name)}
    $row=@{strategy=$strategy.name;target=$target.name;http='—';tls12='—';tls13='—';ping='—'}
    $report.message=$strategy.name+' / '+$target.name;Write-Json 'report.json' $report
    if($target.url -notlike 'PING:*'){
     $variants=@(@{key='http';args=@('--http1.1')},@{key='tls12';args=@('--tlsv1.2','--tls-max','1.2')},@{key='tls13';args=@('--tlsv1.3','--tls-max','1.3')})
     foreach($v in $variants){
      if(Cancelled){break}
      $curlArgs=@('-I','-s','-m',[string]$plan.timeout,'--connect-timeout','2','-o','NUL','-w','%{http_code}','--show-error')+$v.args+@($target.url)
      # Native stderr is text here; do not turn a failed connection into a PowerShell terminating error.
      $prior=$ErrorActionPreference;$ErrorActionPreference='Continue'
      $text=(& curl.exe @curlArgs 2>&1 | Out-String);$code=$LASTEXITCODE
      $ErrorActionPreference=$prior
      $row[$v.key]=if($code -eq 0){'OK'}elseif($text -match 'certificate|Could not resolve host'){'SSL/DNS'}elseif($code -eq 35 -or $text -match 'not supported|unsupported|schannel'){'UNSUP'}else{'ERROR'}
     }
    }
    if(-not(Cancelled)){
     $hostName=if($target.url -like 'PING:*'){$target.url.Substring(5)}else{([Uri]$target.url).DnsSafeHost}
     $ping=New-Object Net.NetworkInformation.Ping
     try{$reply=$ping.Send($hostName,1000);$row.ping=if($reply.Status -eq 'Success'){[string]$reply.RoundtripTime+' ms'}else{'Timeout'}}catch{$row.ping='Timeout'}finally{$ping.Dispose()}
    }
    $report.rows+=@($row);$report.completed++;$targetIndex++;Write-Json 'report.json' $report
   }
   } catch {
    $strategyErrors++
    $detail=$_.Exception.Message
    if($stderr -and (Test-Path -LiteralPath $stderr)){$detail+='; '+[IO.File]::ReadAllText($stderr)}
    if($stdout -and (Test-Path -LiteralPath $stdout)){$detail+='; '+[IO.File]::ReadAllText($stdout)}
    $report.rows+=@(@{strategy=$strategy.name;target=$detail;http='START ERROR';tls12='—';tls13='—';ping='—'})
    $report.completed+=(@($plan.targets).Count-$targetIndex)
    $report.message='Ошибка стратегии '+$strategy.name+'; переход к следующей'
    Write-Json 'report.json' $report
   } finally {
    Stop-TestProcess
    Start-Sleep -Milliseconds 500
   }
  }
  $report.phase=if(Cancelled){'cancelled'}elseif($strategyErrors -gt 0){'completed_with_errors'}else{'completed'}
  $report.message=if(Cancelled){'Проверка отменена'}elseif($strategyErrors -gt 0){'Проверка завершена; ошибок стратегий: '+$strategyErrors}else{'Проверка завершена'}
 }catch{
  $report.phase='failed';$report.message=$_.Exception.Message
 }finally{
  if($script:journal){
   try{Restore-State;$report.message+='; исходное состояние восстановлено'}catch{$report.phase='recovery_required';$report.message+='; ошибка восстановления: '+$_.Exception.Message}
  }
  Write-Json 'report.json' $report
 }
}finally{$lock.Dispose()}






