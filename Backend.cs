using System.Diagnostics;
namespace ZapretGui;
static class Backend
{
 public static string Quote(string s)=>"'"+s.Replace("'","''")+"'";
 public static async Task<string> Power(string script,bool elevated=false)
 {
  var dir=Path.Combine(AppContext.BaseDirectory,"runtime");Directory.CreateDirectory(dir);
  var id=Guid.NewGuid().ToString();var file=Path.Combine(dir,id+".ps1");var output=Path.Combine(dir,id+".txt");
  File.WriteAllText(file,"$ErrorActionPreference='Stop';try { & { "+script+" } | Out-String | Out-File -LiteralPath "+Quote(output)+" -Encoding utf8 } catch { $_ | Out-String | Out-File -LiteralPath "+Quote(output)+" -Encoding utf8; exit 1 }",new System.Text.UTF8Encoding(true));
  try{var si=new ProcessStartInfo("powershell.exe"){UseShellExecute=true,WindowStyle=ProcessWindowStyle.Hidden,Arguments="-NoProfile -ExecutionPolicy Bypass -File \""+file+"\""};if(elevated)si.Verb="runas";using var p=Process.Start(si)!;await p.WaitForExitAsync();var result=File.Exists(output)?File.ReadAllText(output):"Нет результата";if(p.ExitCode!=0)throw new IOException(result);return result;}finally{File.Delete(file);File.Delete(output);}
 }
 public static bool Ports(string s)=>s.Length>0&&s.Split(',').All(part=>{var a=part.Split('-');return a.Length<=2&&a.All(x=>ushort.TryParse(x,out var n)&&n>0)&&(a.Length==1||ushort.Parse(a[0])<=ushort.Parse(a[1]));});
 public static string Arguments(string root,string strategy)
 {
  if(Path.GetFileName(strategy)!=strategy||!File.Exists(Path.Combine(root,strategy)))throw new IOException("Выберите стратегию");
  var text=File.ReadAllText(Path.Combine(root,strategy));const string marker="\"%BIN%winws.exe\"";var index=text.IndexOf(marker,StringComparison.OrdinalIgnoreCase);if(index<0)throw new IOException("Неизвестный формат стратегии");
  var game=Path.Combine(root,"utils","game_filter.enabled");var settings=File.Exists(game)?File.ReadAllLines(game):[];
  string Get(string k)=>settings.FirstOrDefault(x=>x.StartsWith(k+"="))?[(k.Length+1)..].Trim()??"";
  string Port(string k){if(Get("mode")!="all"&&Get("mode")!=k)return "12";var p=Get(k);if(p=="")p="1024-65535";if(!Ports(p))throw new IOException("Неверные порты");return p;}
  var args=string.Join(" ",text[(index+marker.Length)..].Split('\n').Select(x=>x.Trim().TrimEnd('^').Trim())).Replace("%BIN%",Path.Combine(root,"bin")+"\\").Replace("%LISTS%",Path.Combine(root,"lists")+"\\").Replace("%GameFilterTCP%",Port("tcp")).Replace("%GameFilterUDP%",Port("udp"));
  if(args.IndexOfAny(['%','&','|','<','>','\r','\n'])>=0)throw new IOException("Неподдерживаемые параметры стратегии");return args.Trim();
 }
 public static async Task<string> Install(string root,string strategy)
 {
  var args=Arguments(root,strategy);var exe=Path.Combine(root,"bin","winws.exe");if(!File.Exists(exe))throw new IOException("Нет bin/winws.exe");
  var lists=Path.Combine(root,"lists");Directory.CreateDirectory(lists);
  foreach(var item in new[]{("list-general-user.txt","domain.example.abc\n"),("list-exclude-user.txt","domain.example.abc\n"),("ipset-exclude-user.txt","203.0.113.113/32\n")})
   if(!File.Exists(Path.Combine(lists,item.Item1)))ListStore.Atomic(Path.Combine(lists,item.Item1),item.Item2);
  var script = "$exe="+Quote(exe)+";$bin="+Quote("\""+exe+"\" "+args)+";$service=Get-Service zapret -ErrorAction SilentlyContinue;"+
   "$servicePids=@(Get-CimInstance Win32_Service -Filter 'ProcessId > 0' | Select-Object -ExpandProperty ProcessId);"+
   "$standalone=@(Get-CimInstance Win32_Process -Filter \"Name='winws.exe'\" | Where-Object {$_.ProcessId -notin $servicePids});if($standalone.Count){throw 'Сначала остановите отдельный winws'};"+
   "if($service){Stop-Service zapret -ErrorAction Stop;$service.WaitForStatus('Stopped',[TimeSpan]::FromSeconds(20));Set-ItemProperty -LiteralPath 'HKLM:\\SYSTEM\\CurrentControlSet\\Services\\zapret' -Name ImagePath -Value $bin;Set-Service zapret -StartupType Automatic}"+
   "else{New-Service -Name zapret -DisplayName zapret -BinaryPathName $bin -StartupType Automatic | Out-Null};"+
   "New-ItemProperty -LiteralPath 'HKLM:\\SYSTEM\\CurrentControlSet\\Services\\zapret' -Name 'zapret-discord-youtube' -Value "+Quote(Path.GetFileNameWithoutExtension(strategy))+" -PropertyType String -Force | Out-Null;"+
   "Start-Service zapret -ErrorAction Stop;$service=Get-Service zapret;$service.WaitForStatus('Running',[TimeSpan]::FromSeconds(20));'Установлена стратегия: '+"+Quote(strategy)+";Get-Service zapret | Format-Table Name,Status";
  return await Power(script,true);
 }
 public static Task<string> Status()=>Power("$s=Get-CimInstance Win32_Service -Filter \"Name='zapret'\";if(!$s){'Служба zapret не установлена'}else{$s | Format-List Name,State,PathName;$p=Get-ItemProperty -LiteralPath 'HKLM:\\SYSTEM\\CurrentControlSet\\Services\\zapret';'Стратегия: '+$p.'zapret-discord-youtube'}");
 public static Task<string> Service(string action)=>Power(action=="start"?"if(!(Get-Service zapret -ErrorAction SilentlyContinue)){throw 'Служба не установлена. Выберите стратегию и нажмите Применить стратегию к службе'};Start-Service zapret -ErrorAction Stop;Get-Service zapret | Format-Table Name,Status":"Stop-Service zapret -ErrorAction Stop;$s=Get-Service zapret;$s.WaitForStatus('Stopped',[TimeSpan]::FromSeconds(20));Get-Service zapret | Format-Table Name,Status",true);
}
