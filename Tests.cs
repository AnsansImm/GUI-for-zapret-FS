using System.Diagnostics;
using System.Text.Json;
namespace ZapretGui;
sealed partial class MainForm
{
 string TestDir=>Path.Combine(AppContext.BaseDirectory,"runtime","test-run");
 bool testBusy;
 void BuildTests(TabControl tabs)
 {
  var panel=Page(tabs,"Тестирование");
  var choices=new CheckedListBox(){Width=650,Height=160,CheckOnClick=true};panel.Controls.Add(choices);
  Add(panel,"Обновить список стратегий",()=>{choices.Items.Clear();choices.Items.AddRange(strategies.Items.Cast<string>().ToArray());});
  var dpi=new CheckBox(){Text="DPI TCP 16–20",AutoSize=true};panel.Controls.Add(dpi);
  var timeout=new NumericUpDown(){Minimum=1,Maximum=20,Value=4};panel.Controls.Add(new Label(){Text="Таймаут, секунд",AutoSize=true});panel.Controls.Add(timeout);
  var result=new RichTextBox(){ReadOnly=true,Width=700,Height=180,WordWrap=false,BorderStyle=BorderStyle.None,Font=new Font("Consolas",10),ScrollBars=RichTextBoxScrollBars.Both};panel.Controls.Add(result);
  var timer=new System.Windows.Forms.Timer(){Interval=1000};
  var renderedRows=0;string? lastStrategy=null;string? lastReport=null;
  var progress=new Label(){Text="Проверка ещё не запускалась",Dock=DockStyle.Top,Height=36,AutoEllipsis=true,ForeColor=Ink};
  void Poll()
  {
   try
   {
    var path=Path.Combine(TestDir,"report.json");if(!File.Exists(path))return;var raw=File.ReadAllText(path);if(raw==lastReport)return;
    using var json=JsonDocument.Parse(raw);var report=json.RootElement;var rows=report.GetProperty("rows");
    progress.Text=report.GetProperty("message").GetString()+"  •  "+report.GetProperty("completed")+" / "+report.GetProperty("total");
    if(rows.GetArrayLength()<renderedRows){result.Clear();renderedRows=0;lastStrategy=null;}
    if(rows.GetArrayLength()>renderedRows)
    {
     for(var i=renderedRows;i<rows.GetArrayLength();i++)
     {
      var row=rows[i];var strategy=row.GetProperty("strategy").GetString();
      if(strategy!=lastStrategy){result.SelectionStart=result.TextLength;result.SelectionColor=Accent;result.AppendText((result.TextLength>0?Environment.NewLine:"")+"━━  "+strategy+"  ━━"+Environment.NewLine);result.SelectionColor=Color.FromArgb(170,164,181);result.AppendText("Цель".PadRight(24)+"  "+"HTTP".PadRight(14)+"  "+"TLS 1.2".PadRight(14)+"  "+"TLS 1.3".PadRight(14)+"  "+"Ping"+Environment.NewLine);lastStrategy=strategy;}
      ColorRow(result,row);
     }
     renderedRows=rows.GetArrayLength();result.SelectionStart=result.TextLength;result.SelectionLength=0;result.ScrollToCaret();
    }
    lastReport=raw;
   }catch(IOException){}catch(JsonException){}
  }  timer.Tick+=(_,_)=>Poll();timer.Start();FormClosed+=(_,_)=>timer.Dispose();Poll();
  bool Pending(){var j=Path.Combine(TestDir,"journal.json");if(!File.Exists(j))return false;using var doc=JsonDocument.Parse(File.ReadAllText(j));return !doc.RootElement.GetProperty("restored").GetBoolean();}
  async Task<string> Launch(bool recover)
  {
   if(testBusy)throw new IOException("Проверка уже выполняется");
   var script=Path.Combine(AppContext.BaseDirectory,"scripts","test-runner.ps1");
   if(!File.Exists(script))throw new IOException("Не найден test-runner.ps1");
   if(!recover){
    if(Pending())throw new IOException("Сначала восстановите состояние предыдущего теста");
    var selected=choices.CheckedItems.Cast<string>().Select(name=>new{name,args=Backend.Arguments(root.Text,name)}).ToArray();if(selected.Length==0)throw new IOException("Отметьте стратегии");
    var urls=dpi.Checked?new[]{new TestTarget("DPI suite","https://example.org")}:OriginalTargets();
    if(Directory.Exists(TestDir))Directory.Move(TestDir,TestDir+"-"+DateTime.Now.ToString("yyyyMMdd-HHmmss-fff"));Directory.CreateDirectory(TestDir);
    ListStore.Atomic(Path.Combine(TestDir,"plan.json"),JsonSerializer.Serialize(new{exe=Path.Combine(root.Text,"bin","winws.exe"),strategies=selected,targets=urls,timeout=(int)timeout.Value,mode=dpi.Checked?"dpi":"standard",range_bytes=32768,dpi_host=""}));
   }
   if(!recover){result.Clear();renderedRows=0;lastStrategy=null;lastReport=null;}
   testBusy=true;try{
    using var process=Process.Start(new ProcessStartInfo("powershell.exe"){UseShellExecute=true,Verb="runas",WindowStyle=ProcessWindowStyle.Hidden,Arguments="-NoProfile -ExecutionPolicy Bypass -File \""+script+"\" -JobDir \""+TestDir+"\""+(recover?" -Recover":"")})!;
    await process.WaitForExitAsync();Poll();if(process.ExitCode!=0)throw new IOException("Тест завершился с ошибкой. "+(File.Exists(Path.Combine(TestDir,"fatal.log"))?File.ReadAllText(Path.Combine(TestDir,"fatal.log")):result.Text));return result.Text;
   }finally{testBusy=false;}
  }
  AsyncButton(panel,"Начать проверку (временно остановит Zapret)",()=>Launch(false));
  Add(panel,"Отменить и восстановить состояние",()=>{if(Directory.Exists(TestDir))File.WriteAllText(Path.Combine(TestDir,"cancel"),"cancel");});
  AsyncButton(panel,"Восстановить после прерванной проверки",()=>Launch(true));
  arrangeTestsView=()=>ArrangeTests(panel,choices,dpi,timeout,result,progress);
  FormClosing+=(_,e)=>{if(testBusy){e.Cancel=true;log.Text="Сначала отмените проверку и дождитесь восстановления состояния.";}};
 }
}
