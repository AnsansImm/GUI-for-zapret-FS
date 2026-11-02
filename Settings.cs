using System.Net;
namespace ZapretGui;
sealed partial class MainForm
{
 void BuildSettings(TabControl tabs)
 {
  var settingsPage=new TabPage("Настройки");tabs.TabPages.Add(settingsPage);var inner=new TabControl(){Dock=DockStyle.Fill};settingsPage.Controls.Add(inner);var panel=Page(inner,"Game Filter");panel.WrapContents=false;
  var mode=new ListBox(){Width=220,Height=90,IntegralHeight=false,BorderStyle=BorderStyle.None};mode.Items.AddRange(["none","tcp","udp","all"]);mode.SelectedIndex=0;
  var tcp=new TextBox(){Width=400,Text="1024-65535"};var udp=new TextBox(){Width=400,Text="1024-65535"};
  panel.Controls.Add(new Label(){Text="Game Filter: режим, порты TCP, порты UDP",AutoSize=true});panel.Controls.Add(mode);panel.Controls.Add(tcp);panel.Controls.Add(udp);
  Add(panel,"Прочитать настройки Game Filter",()=>{var p=Path.Combine(root.Text,"utils/game_filter.enabled");var lines=File.Exists(p)?File.ReadAllLines(p):[];string Get(string k,string fallback)=>lines.FirstOrDefault(x=>x.StartsWith(k+"="))?[(k.Length+1)..].Trim()??fallback;mode.SelectedItem=Get("mode","none");tcp.Text=Get("tcp","1024-65535");udp.Text=Get("udp","1024-65535");log.Text="Game Filter: "+mode.Text;});
  Add(panel,"Сохранить Game Filter",()=>{if(mode.SelectedIndex<0||!Backend.Ports(tcp.Text)||!Backend.Ports(udp.Text))throw new IOException("Проверьте режим и порты (например 443,1024-65535)");var p=Path.Combine(root.Text,"utils/game_filter.enabled");if(File.Exists(p))ListStore.Atomic(p+".backup",File.ReadAllText(p));ListStore.Atomic(p,$"mode={mode.Text}\ntcp={tcp.Text}\nudp={udp.Text}\n");log.Text="Game Filter сохранён. Перезапустите Zapret.";});
  panel=Page(inner,"IPSet");panel.WrapContents=false;
  Add(panel,"Состояние IPSet",()=>{log.Text="IPSet: "+IpMode(File.ReadAllText(IpPath()));});
  Add(panel,"IPSet: все IP",()=>SetIp(""));Add(panel,"IPSet: отключить",()=>SetIp("203.0.113.113/32\n"));
  Add(panel,"IPSet: восстановить список",()=>{var text=File.ReadAllText(IpPath()+".backup");ValidateIp(text);ListStore.Atomic(IpPath(),text);log.Text="Список IPSet восстановлен. Перезапустите Zapret.";});
  AsyncButton(panel,"Обновить список IPSet",async()=>{var path=IpPath();using var http=new System.Net.Http.HttpClient(){Timeout=TimeSpan.FromSeconds(45)};var text=await http.GetStringAsync("https://raw.githubusercontent.com/Flowseal/zapret-discord-youtube/refs/heads/main/.service/ipset-service.txt");ValidateIp(text);var old=File.ReadAllText(path);if(IpMode(old)=="загружен")ListStore.Atomic(path+".backup",old);ListStore.Atomic(path,text);return "IPSet обновлён. Перезапустите Zapret.";});
  panel=Page(inner,"Fake-файлы");panel.WrapContents=false;
  var columns=new TableLayoutPanel(){Width=780,Height=260,ColumnCount=2,RowCount=1};columns.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,50));columns.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,50));panel.Controls.Add(columns);
  foreach(var entry in new[]{(File:"ACTIVE_DISCORD_UDP.bin",Title:"Discord UDP"),(File:"ACTIVE_GAME_UDP.bin",Title:"Game Filter UDP")})
  {
   var box=new FlowLayoutPanel(){Dock=DockStyle.Fill,FlowDirection=FlowDirection.TopDown,WrapContents=false};columns.Controls.Add(box);
   box.Controls.Add(new Label(){Text=entry.Title,AutoSize=true});
   var current=new Label(){Text="Текущий: не прочитан",Width=370,Height=40};box.Controls.Add(current);
   var source=new ListBox(){Width=370,Height=170,IntegralHeight=false,BorderStyle=BorderStyle.None,HorizontalScrollbar=true,DisplayMember="Name"};box.Controls.Add(source);
   void RefreshFake()
   {
    source.Items.Clear();var dir=Path.Combine(root.Text,"bin");if(!Directory.Exists(dir)){current.Text="Выберите папку Zapret";return;}
    var available=Directory.GetFiles(dir,"*.bin").Where(x=>!Path.GetFileName(x).StartsWith("ACTIVE_",StringComparison.OrdinalIgnoreCase)).Order().Select(x=>new FileInfo(x)).ToArray();source.Items.AddRange(available);
    var active=Path.Combine(dir,entry.File);string? match=null;if(File.Exists(active)){var bytes=File.ReadAllBytes(active);match=available.FirstOrDefault(x=>File.ReadAllBytes(x.FullName).SequenceEqual(bytes))?.Name;current.Text="Текущий: "+(match??"неизвестный файл");}else current.Text="Текущий: отсутствует";
    if(match!=null)source.SelectedItem=available.First(x=>x.Name==match);else if(source.Items.Count>0)source.SelectedIndex=0;
   }
   Add(box,"Применить",()=>{var src=source.SelectedItem as FileInfo??throw new IOException("Выберите fake-файл");var bytes=File.ReadAllBytes(src.FullName);if(bytes.Length==0)throw new IOException("Файл пуст");var dest=Path.Combine(root.Text,"bin",entry.File);if(File.Exists(dest)){var old=File.ReadAllBytes(dest);if(old.SequenceEqual(bytes)){log.Text="Этот fake-файл уже используется";return;}WriteBytes(dest+".backup",old);}WriteBytes(dest,bytes);RefreshFake();log.Text=entry.Title+": файл заменён. Перезапустите Zapret.";});
   Add(box,"Восстановить предыдущий",()=>{var dest=Path.Combine(root.Text,"bin",entry.File);var bytes=File.ReadAllBytes(dest+".backup");if(bytes.Length==0)throw new IOException("Резервная копия пуста");WriteBytes(dest,bytes);RefreshFake();log.Text=entry.Title+": файл восстановлен. Перезапустите Zapret.";});
   Add(box,"Обновить",RefreshFake);
   tabs.SelectedIndexChanged+=(_,_)=>{if(tabs.SelectedTab?.Text=="Настройки"){try{RefreshFake();}catch(Exception e){current.Text=e.Message;}}};
   root.TextChanged+=(_,_)=>{try{RefreshFake();}catch(Exception e){current.Text=e.Message;}};
  }
 }
 string IpPath()=>Path.Combine(root.Text,"lists/ipset-all.txt");
 static string IpMode(string s)=>string.IsNullOrWhiteSpace(s.TrimStart('\uFEFF'))?"все IP":s.Split('\n').Any(x=>x.Trim()=="203.0.113.113/32")?"отключён":"загружен";
 void SetIp(string text){var p=IpPath();var old=File.ReadAllText(p);if(IpMode(old)=="загружен"){ValidateIp(old);ListStore.Atomic(p+".backup",old);}ListStore.Atomic(p,text);log.Text="IPSet: "+IpMode(text)+". Перезапустите Zapret.";}
 static void ValidateIp(string text)
 {
  var count=0;foreach(var line in text.TrimStart('\uFEFF').Split('\n')){var value=line.Split('#')[0].Trim();if(value.Length==0)continue;bool valid;
   if(value.Contains('/')){var a=value.Split('/');valid=a.Length==2&&IPAddress.TryParse(a[0],out var ip)&&int.TryParse(a[1],out var n)&&n>=0&&n<=(ip.AddressFamily==System.Net.Sockets.AddressFamily.InterNetwork?32:128);}
   else if(value.Contains('-')){var a=value.Split('-');valid=a.Length==2&&IPAddress.TryParse(a[0],out var first)&&IPAddress.TryParse(a[1],out var last)&&first.AddressFamily==last.AddressFamily&&CompareAddress(first,last)<=0;}
   else valid=IPAddress.TryParse(value,out _);
   if(!valid)throw new IOException("Неверная запись IPSet: "+value);count++;
  }if(count==0||IpMode(text)!="загружен")throw new IOException("Получен пустой или отключённый IPSet");
 }
 static int CompareAddress(IPAddress a,IPAddress b){var x=a.GetAddressBytes();var y=b.GetAddressBytes();for(int i=0;i<x.Length;i++){var c=x[i].CompareTo(y[i]);if(c!=0)return c;}return 0;}
 static void WriteBytes(string path,byte[] data){var temp=path+"."+Guid.NewGuid()+".tmp";try{using(var stream=new FileStream(temp,FileMode.CreateNew,FileAccess.Write,FileShare.None)){stream.Write(data);stream.Flush(true);}File.Move(temp,path,true);}finally{if(File.Exists(temp))File.Delete(temp);}}
}
