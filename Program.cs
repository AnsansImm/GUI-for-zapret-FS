using System.Diagnostics;
using System.Text.Json;
namespace ZapretGui;
static class Program {
 [STAThread] static void Main() { ApplicationConfiguration.Initialize(); DarkControls.Initialize(); Application.Run(new MainForm()); }
}
sealed partial class MainForm : Form {
 readonly TextBox root = new(){Width=650}; readonly ComboBox strategies = new(){Width=400,DropDownStyle=ComboBoxStyle.DropDownList};
 readonly TextBox log = new(){Multiline=true,Dock=DockStyle.Bottom,Height=160,ScrollBars=ScrollBars.Both};
 readonly ComboBox files = new(){Width=400,DropDownStyle=ComboBoxStyle.DropDownList};
 readonly TextBox editor = new(){Multiline=true,Dock=DockStyle.Fill,ScrollBars=ScrollBars.Both,AcceptsReturn=true,WordWrap=false};
 bool loadingEditor; bool choosingFile; string editorBaseline=""; readonly System.Windows.Forms.Timer saveTimer=new(){Interval=800};
 readonly TextBox domain = new(){Width=350}; string? selected; string original="";
 string Config => Path.Combine(AppContext.BaseDirectory,"config.json");
 public MainForm() {
 Text=AppInfo.Title;Icon=AppInfo.LoadIcon();ShowIcon=true;Width=1000;Height=750;
 var tabs=new ContentTabs(){Dock=DockStyle.Fill}; var manage=new TabPage("Управление"); var lists=new TabPage("Списки"); var sites=new TabPage("Добавить сайт"); tabs.TabPages.AddRange([manage,lists,sites]); tabs.Appearance=TabAppearance.FlatButtons; tabs.ItemSize=new Size(0,1); tabs.SizeMode=TabSizeMode.Fixed; var nav=new ListBox(){Dock=DockStyle.Left,Width=175}; nav.SelectedIndexChanged+=(_,_)=>{if(nav.SelectedIndex>=0)tabs.SelectedIndex=nav.SelectedIndex;}; Controls.Add(tabs); Controls.Add(nav); Controls.Add(log);
 var panel=new FlowLayoutPanel(){Dock=DockStyle.Fill,FlowDirection=FlowDirection.TopDown,WrapContents=false,AutoScroll=false}; manage.Controls.Add(panel); panel.Controls.Add(root);
 Add(panel,"Выбрать папку",()=>{using var d=new FolderBrowserDialog();if(d.ShowDialog()==DialogResult.OK){if(!TrySavePending())return;root.Text=d.SelectedPath;RefreshRoot();File.WriteAllText(Config,JsonSerializer.Serialize(new {root=root.Text}));}});
 panel.Controls.Add(strategies);
 Add(panel,"Запустить батник отдельно",()=>{var path=Path.Combine(root.Text,strategies.Text);if(!File.Exists(path))throw new Exception("Выберите стратегию");Process.Start(new ProcessStartInfo(path){UseShellExecute=true,Verb="runas",WorkingDirectory=root.Text});});
 AsyncButton(panel,"Состояние и стратегия службы",Backend.Status);
 AsyncButton(panel,"Запустить службу",()=>Backend.Service("start")); AsyncButton(panel,"Остановить службу",()=>Backend.Service("stop"));
 Add(panel,"Оригинальное управление службой",()=>{var p=Path.Combine(root.Text,"service.bat");if(!File.Exists(p))throw new Exception("service.bat не найден");Process.Start(new ProcessStartInfo(p){UseShellExecute=true,Verb="runas",WorkingDirectory=root.Text});});
 var bar=new FlowLayoutPanel(){Dock=DockStyle.Top,Height=65};bar.Controls.Add(files);Add(bar,"Сохранить",Save); lists.Controls.Add(editor);lists.Controls.Add(bar);
 files.SelectedIndexChanged+=(_,_)=>{if(choosingFile)return;var next=files.SelectedItem as string;if(!TrySavePending()){choosingFile=true;files.SelectedItem=selected;choosingFile=false;return;}try{selected=next;if(selected!=null)LoadEditor();}catch(Exception e){log.Text=e.Message;}};
 editor.Font=new Font(FontFamily.GenericMonospace,10); editor.TextChanged+=(_,_)=>{if(!loadingEditor){saveTimer.Stop();saveTimer.Start();}};saveTimer.Tick+=(_,_)=>{saveTimer.Stop();try{Save();}catch(Exception e){log.Text=e.Message;}};FormClosed+=(_,_)=>saveTimer.Dispose();
 var sp=new FlowLayoutPanel(){Dock=DockStyle.Fill,FlowDirection=FlowDirection.TopDown,WrapContents=false,AutoScroll=false}; var siteHeader=new FlowLayoutPanel(){Dock=DockStyle.Top,Height=145,FlowDirection=FlowDirection.TopDown,WrapContents=false,Padding=new Padding(6)}; siteHeader.Controls.Add(new Label(){Text="Домен или адрес сайта — введите вручную или выберите ниже",AutoSize=true});domain.Width=600;siteHeader.Controls.Add(domain);var siteActions=new FlowLayoutPanel(){Width=700,Height=55};Add(siteActions,"Через Zapret",()=>AddDomain(false));Add(siteActions,"Исключить из Zapret",()=>AddDomain(true));siteHeader.Controls.Add(siteActions);sites.Controls.Add(sp);sites.Controls.Add(siteHeader);BuildBrowser(sp);
 BuildFeatures(tabs,panel); foreach(TabPage page in tabs.TabPages)nav.Items.Add(page.Text);nav.SelectedIndex=0; ApplyDesign(tabs,nav); DarkControls.Attach(this); FormClosing+=(_,e)=>{if(!e.Cancel)e.Cancel=!TrySavePending();};
 try {if(File.Exists(Config)){using var config=JsonDocument.Parse(File.ReadAllText(Config));root.Text=config.RootElement.GetProperty("root").GetString();}RefreshRoot();}catch(Exception e){log.Text=e.Message;}
 }
 void Add(Control c,string title,Action action){var b=new PillButton(){Text=title,AutoSize=true};b.Click+=(_,_)=>{try{action();}catch(Exception e){ActionOutput(b,e.Message);}};c.Controls.Add(b);}
 void RefreshRoot()
 {
  saveTimer.Stop();loadingEditor=true;choosingFile=true;
  try
  {
   selected=null;editor.Clear();original="";editorBaseline="";strategies.Items.Clear();files.Items.Clear();
   if(!Directory.Exists(root.Text))return;
   strategies.Items.AddRange(Directory.GetFiles(root.Text,"*.bat").Select(path=>Path.GetFileName(path)!).Where(n=>!n.StartsWith("service",StringComparison.OrdinalIgnoreCase)).OrderBy(n=>n,StrategyOrder.Instance).ToArray());
   if(strategies.Items.Count>0)strategies.SelectedIndex=0;
   var p=Path.Combine(root.Text,"lists");if(Directory.Exists(p)){using(var guard=ListStore.Lock(p))ListStore.Recover(p);files.Items.AddRange(Directory.GetFiles(p,"*.txt"));}
  }
  finally{loadingEditor=false;choosingFile=false;refreshStrategyView?.Invoke();refreshFileView?.Invoke();refreshTestView?.Invoke();}
 }
 bool TrySavePending(){if(editor.Text==editorBaseline)return true;try{Save();return true;}catch(Exception e){log.Text=e.Message;return false;}}
 void LoadEditor(){loadingEditor=true;try{original=File.ReadAllText(selected!);editor.Text=EditorText.Display(original);editorBaseline=editor.Text;}finally{loadingEditor=false;}}
 void Save(){saveTimer.Stop();if(selected==null||editor.Text==editorBaseline)return;var text=EditorText.Normalize(editor.Text,Path.GetFileName(selected).StartsWith("ipset",StringComparison.OrdinalIgnoreCase));ListStore.Save(selected,text,original);original=text;var cursor=editor.SelectionStart;loadingEditor=true;try{editor.Text=EditorText.Display(text+(text.Length>0?"\n":""));editor.SelectionStart=Math.Min(cursor,editor.TextLength);editorBaseline=editor.Text;}finally{loadingEditor=false;}log.Text="Список сохранён автоматически. Для применения перезапустите Zapret.";}
 void AddDomain(bool exclude){if(!TrySavePending())return;var host=ListStore.Add(Path.Combine(root.Text,"lists"),domain.Text,exclude);if(selected!=null){LoadEditor();}log.Text="Добавлен: "+host+". Перезапустите Zapret для применения.";}
 void Elevate(string action){Process.Start(new ProcessStartInfo("sc.exe"){UseShellExecute=true,Verb="runas",Arguments=action+" zapret"});log.Text="Запрошено действие: "+action;}
 static async Task<string> Run(string exe,params string[] args){var si=new ProcessStartInfo(exe){RedirectStandardOutput=true,RedirectStandardError=true,CreateNoWindow=true};foreach(var a in args)si.ArgumentList.Add(a);using var p=Process.Start(si)!;var o=p.StandardOutput.ReadToEndAsync();var e=p.StandardError.ReadToEndAsync();await p.WaitForExitAsync();var output=await o+await e;if(p.ExitCode!=0&&!(exe.Equals("sc.exe",StringComparison.OrdinalIgnoreCase)&&p.ExitCode==1060))throw new IOException(output);return output;}
}
