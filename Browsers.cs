using System.Text.Json;
namespace ZapretGui;
sealed partial class MainForm
{
 void BuildBrowser(Control parent)
 {
  var browser=new ComboBox(){Width=180,DropDownStyle=ComboBoxStyle.DropDownList};browser.Items.AddRange(["Chrome","Edge","Opera","Firefox"]);browser.SelectedIndex=0;parent.Controls.Add(browser);
  var port=new NumericUpDown(){Minimum=1,Maximum=65535,Value=9222};parent.Controls.Add(new Label(){Text="CDP порт",AutoSize=true});parent.Controls.Add(port);
  var profiles=new ComboBox(){Width=650,DropDownStyle=ComboBoxStyle.DropDownList};parent.Controls.Add(profiles);
  var sites=new ListBox(){Width=700,Height=240,HorizontalScrollbar=true};parent.Controls.Add(sites);
  void Show(IEnumerable<string[]> entries){sites.Items.Clear();var seen=new HashSet<string>();foreach(var entry in entries){try{var host=ListStore.Domain(entry[0]);if(seen.Add(host))sites.Items.Add(new BrowserSite(host,entry[1],entry[0]));}catch(ArgumentException){}}log.Text="Найдено сайтов: "+sites.Items.Count;}
  string[] FindHistory(string selectedBrowser)
  {
   var local=Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);var roaming=Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
   var dir=selectedBrowser switch{"Chrome"=>Path.Combine(local,"Google/Chrome/User Data"),"Edge"=>Path.Combine(local,"Microsoft/Edge/User Data"),"Opera"=>Path.Combine(roaming,"Opera Software"),_=>Path.Combine(roaming,"Mozilla/Firefox/Profiles")};
   if(!Directory.Exists(dir))return [];
   var name=selectedBrowser=="Firefox"?"places.sqlite":"History";
   var directories=Directory.GetDirectories(dir).Prepend(dir).ToList();
   if(selectedBrowser=="Opera")foreach(var sub in Directory.GetDirectories(dir))directories.AddRange(Directory.GetDirectories(sub));
   return directories.Select(d=>Path.Combine(d,name)).Where(File.Exists).Distinct(StringComparer.OrdinalIgnoreCase).OrderByDescending(File.GetLastWriteTimeUtc).ToArray();
  }
  AsyncButton(parent,"Выбрать файл истории",async()=>{using var dlg=new OpenFileDialog(){Filter="История браузера|History;places.sqlite|Все файлы|*.*"};if(dlg.ShowDialog()!=DialogResult.OK)return "Выбор отменён";var entries=await Task.Run(()=>History.Read(dlg.FileName));Show(entries);return "История из выбранного файла: "+sites.Items.Count+" сайтов.";});
  AsyncButton(parent,"Прочитать историю",async()=>{
   var selectedBrowser=browser.Text;sites.Items.Clear();
   var data=await Task.Run(()=>{var paths=FindHistory(selectedBrowser);if(paths.Length==0)throw new IOException("История "+selectedBrowser+" не найдена. Можно выбрать файл вручную.");var entries=new List<string[]>();var failures=new List<string>();foreach(var path in paths){try{entries.AddRange(History.Read(path));}catch(Exception e){failures.Add(Path.GetFileName(Path.GetDirectoryName(path))+": "+e.Message);}}if(entries.Count==0&&failures.Count==paths.Length)throw new IOException(string.Join(Environment.NewLine,failures));return (entries,failures);});
   Show(data.entries);return selectedBrowser+": "+sites.Items.Count+" сайтов из истории."+(data.failures.Count>0?Environment.NewLine+"Не удалось прочитать: "+string.Join("; ",data.failures):"");
  });  AsyncButton(parent,"Найти открытые вкладки CDP",async()=>{using var handler=new System.Net.Http.HttpClientHandler(){UseProxy=false,AllowAutoRedirect=false};using var http=new System.Net.Http.HttpClient(handler){Timeout=TimeSpan.FromSeconds(4)};using var response=await http.GetAsync("http://127.0.0.1:"+port.Value+"/json/list");response.EnsureSuccessStatusCode();using var doc=JsonDocument.Parse(await response.Content.ReadAsStringAsync());Show(doc.RootElement.EnumerateArray().Where(x=>x.TryGetProperty("type",out var t)&&t.GetString()=="page").Select(x=>new[]{x.GetProperty("url").GetString()??"",x.TryGetProperty("title",out var title)?title.GetString()??"":""}));return "Открытые вкладки: "+sites.Items.Count;});
  sites.SelectedIndexChanged+=(_,_)=>{if(sites.SelectedItem is BrowserSite site)domain.Text=site.Domain;};
  parent.Controls.Add(new Label(){Text="Выберите сайт и нажмите «Через Zapret» или «Исключить». CDP требует запуска Chromium с --remote-debugging-port и отдельным --user-data-dir. Firefox: история.",Width=700,Height=45});
 }
 sealed record BrowserSite(string Domain,string Title,string Url){public override string ToString()=>Domain+" — "+Title;}
}
