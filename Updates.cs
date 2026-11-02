using System.Diagnostics;
using System.Text.Json;
namespace ZapretGui;
sealed partial class MainForm
{
 void BuildUpdates(TabControl tabs)
 {
  var panel=Page(tabs,"Обновления");
  AsyncButton(panel,"Проверить версию Zapret",async()=>{using var http=new System.Net.Http.HttpClient(){Timeout=TimeSpan.FromSeconds(30)};http.DefaultRequestHeaders.UserAgent.ParseAdd(AppInfo.Name+"/"+AppInfo.Version);using var doc=JsonDocument.Parse(await http.GetStringAsync("https://api.github.com/repos/Flowseal/zapret-discord-youtube/releases/latest"));var version=doc.RootElement.GetProperty("tag_name").GetString();var local=File.ReadLines(Path.Combine(root.Text,"service.bat")).FirstOrDefault(x=>x.Contains("LOCAL_VERSION="))??"Локальная версия неизвестна";return local+Environment.NewLine+"Последний выпуск: "+version;});
  AsyncButton(panel,"Установить обновление с сохранением списков",async()=>{if(!TrySavePending())return "Отменено";var script=Path.Combine(AppContext.BaseDirectory,"scripts/update-zapret.ps1");var result=await Backend.Power("& "+Backend.Quote(script)+" -Root "+Backend.Quote(root.Text)+" -Config "+Backend.Quote(Config),true);using var config=JsonDocument.Parse(File.ReadAllText(Config));root.Text=config.RootElement.GetProperty("root").GetString();RefreshRoot();return result;});
 }
}
