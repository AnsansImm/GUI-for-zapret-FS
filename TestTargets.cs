using System.Text.RegularExpressions;
namespace ZapretGui;
sealed partial class MainForm
{
 Action? refreshTestView;
 Action? arrangeTestsView;
 sealed record TestTarget(string name,string url);
 TestTarget[] OriginalTargets()
 {
  var targets=new Dictionary<string,TestTarget>();var path=Path.Combine(root.Text,"utils/targets.txt");
  if(File.Exists(path))foreach(var line in File.ReadLines(path)){var m=Regex.Match(line,"^\\s*(\\w+)\\s*=\\s*\"([^\"]+)\"");if(m.Success)targets[m.Groups[1].Value]=new(m.Groups[1].Value,m.Groups[2].Value);}
  if(targets.Count==0){var script=Path.Combine(root.Text,"utils/test zapret.ps1");if(!File.Exists(script))throw new IOException("Не найден набор проверок Zapret");foreach(Match m in Regex.Matches(File.ReadAllText(script),"Add-OrSet \\$rawTargets\\s+\"([^\"]+)\"\\s+\"([^\"]+)\""))targets[m.Groups[1].Value]=new(m.Groups[1].Value,m.Groups[2].Value);}
  if(targets.Count==0)throw new IOException("Набор проверок пуст");return targets.Values.ToArray();
 }
 void ColorRow(RichTextBox result, System.Text.Json.JsonElement row)
 {
  var columns=new[]{("target",24),("http",14),("tls12",14),("tls13",14),("ping",10)};
  foreach(var (key,width) in columns)
  {
   var value=row.TryGetProperty(key,out var v)?v.ToString():"";
   result.SelectionStart=result.TextLength;result.SelectionColor=value=="OK"||value.EndsWith(" ms")?Color.FromArgb(137,214,164):value.Contains("ERROR")||value=="Timeout"||value=="LIKELY_BLOCKED"?Color.FromArgb(242,132,145):value is "UNSUP" or "SSL/DNS"?Color.FromArgb(232,195,123):Ink;
   var display=value.Length>width?value[..(width-1)]+"…":value;
   result.AppendText(display.PadRight(width)+(key=="ping"?Environment.NewLine:"  "));
  }
 }
 void ArrangeTests(FlowLayoutPanel panel,CheckedListBox choices,CheckBox dpi,NumericUpDown timeout,RichTextBox result,Label progress)
 {
  var page=(TabPage)panel.Parent!;var buttons=panel.Controls.OfType<Button>().ToArray();
  var layout=new TableLayoutPanel(){Dock=DockStyle.Fill,ColumnCount=2,RowCount=2,BackColor=Background,MinimumSize=new Size(0,560)};page.AutoScroll=true;
  layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,38));layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,62));layout.RowStyles.Add(new RowStyle(SizeType.Absolute,340));layout.RowStyles.Add(new RowStyle(SizeType.Percent,100));
  var left=new TableLayoutPanel(){Dock=DockStyle.Fill,ColumnCount=1,RowCount=4,BackColor=Surface,Margin=new Padding(0,0,16,0)};
  left.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,100));left.RowStyles.Add(new RowStyle(SizeType.Absolute,42));left.RowStyles.Add(new RowStyle(SizeType.Absolute,42));left.RowStyles.Add(new RowStyle(SizeType.Absolute,50));left.RowStyles.Add(new RowStyle(SizeType.Percent,100));
  left.Controls.Add(Heading("Стратегии"),0,0);var hint=Description("Отметьте стратегии для сравнения.");hint.Dock=DockStyle.Fill;hint.AutoSize=false;left.Controls.Add(hint,0,1);
  StyleAction(buttons[0]);buttons[0].Text="Обновить стратегии";buttons[0].Dock=DockStyle.Top;left.Controls.Add(buttons[0],0,2);
  choices.Dock=DockStyle.Fill;choices.IntegralHeight=false;choices.BorderStyle=BorderStyle.None;choices.BackColor=Surface;choices.ForeColor=Ink;choices.Margin=Padding.Empty;left.Controls.Add(choices,0,3);layout.Controls.Add(left,0,0);layout.SetRowSpan(left,2);
  var actions=CardContent("Параметры проверки","На время проверки Zapret будет остановлен, затем его состояние восстановится.");actions.Margin=new Padding(0,0,0,16);
  var options=new FlowLayoutPanel(){Height=36,BackColor=Surface,WrapContents=false,Margin=new Padding(0,0,0,12)};
  dpi.Margin=new Padding(0,4,20,0);options.Controls.Add(dpi);options.Controls.Add(new Label(){Text="Таймаут, с",AutoSize=true,Margin=new Padding(0,4,8,0)});timeout.Width=70;options.Controls.Add(timeout);actions.Controls.Add(options);
  buttons[1].Text="Начать проверку";buttons[2].Text="Отменить и восстановить состояние";buttons[3].Text="Восстановить после прерывания";
  foreach(var button in buttons.Skip(1)){StyleAction(button,button==buttons[1]);actions.Controls.Add(button);}layout.Controls.Add(actions,1,0);
  var report=new TableLayoutPanel(){Dock=DockStyle.Fill,ColumnCount=1,RowCount=3,BackColor=Surface,Margin=Padding.Empty};report.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,100));report.RowStyles.Add(new RowStyle(SizeType.Absolute,42));report.RowStyles.Add(new RowStyle(SizeType.Absolute,44));report.RowStyles.Add(new RowStyle(SizeType.Percent,100));report.Controls.Add(Heading("Результаты проверки"),0,0);
  progress.Dock=DockStyle.Fill;progress.Margin=new Padding(0,0,0,10);progress.ForeColor=Color.FromArgb(180,174,189);report.Controls.Add(progress,0,1);
  result.Dock=DockStyle.Fill;result.BackColor=Surface;result.ForeColor=Ink;result.Margin=Padding.Empty;report.Controls.Add(result,0,2);layout.Controls.Add(report,1,1);
  page.Controls.Add(layout);layout.BringToFront();panel.Dispose();page.Padding=Padding.Empty;
  refreshTestView=()=>{if(testBusy)return;var marked=choices.CheckedItems.Cast<string>().ToHashSet();choices.Items.Clear();foreach(var name in strategies.Items.Cast<string>())choices.Items.Add(name,marked.Contains(name));};refreshTestView();
 }
}
