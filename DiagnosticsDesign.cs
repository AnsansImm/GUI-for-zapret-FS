namespace ZapretGui;
sealed partial class MainForm
{
 void ActionOutput(Control source,string text)
 {
  log.Text=text;
  for(Control? control=source;control!=null;control=control.Parent)if(control.Tag is TextBox output){output.Text=EditorText.Display(text);output.SelectionStart=0;output.ScrollToCaret();break;}
 }
 void ArrangeDiagnostics(TabControl tabs)
 {
  var page=tabs.TabPages.Cast<TabPage>().First(p=>p.Text=="Диагностика");var old=page.Controls.OfType<FlowLayoutPanel>().First();var buttons=old.Controls.OfType<Button>().ToArray();
  var layout=new TableLayoutPanel(){Dock=DockStyle.Fill,ColumnCount=2,RowCount=1,BackColor=Background};layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,32));layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,68));layout.RowStyles.Add(new RowStyle(SizeType.Percent,100));
  var actions=CardContent("Диагностика","Проверьте систему. Доступные действия появятся на карточках результатов.");actions.Margin=new Padding(0,0,16,0);
  foreach(var button in buttons){StyleAction(button,button==buttons[0]);button.Height=48;actions.Controls.Add(button);}
  var report=new TableLayoutPanel(){Dock=DockStyle.Fill,ColumnCount=1,RowCount=2,BackColor=Surface,Margin=Padding.Empty};report.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,100));report.RowStyles.Add(new RowStyle(SizeType.Absolute,46));report.RowStyles.Add(new RowStyle(SizeType.Percent,100));report.Controls.Add(Heading("Результаты проверки"),0,0);
  diagnosticCards=new TableLayoutPanel(){Dock=DockStyle.Fill,ColumnCount=2,RowCount=1,BackColor=Surface,Margin=Padding.Empty};diagnosticCards.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,50));diagnosticCards.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,50));diagnosticCards.RowStyles.Add(new RowStyle(SizeType.Percent,100));
  var empty=new Label(){Text="Нажмите «Проверить систему», чтобы получить результаты.",Dock=DockStyle.Fill,ForeColor=Color.FromArgb(180,174,189),TextAlign=ContentAlignment.MiddleCenter,Margin=new Padding(20)};diagnosticCards.Controls.Add(empty,0,0);diagnosticCards.SetColumnSpan(empty,2);
  report.Controls.Add(diagnosticCards,0,1);layout.Controls.Add(actions,0,0);layout.Controls.Add(report,1,0);page.Controls.Add(layout);layout.BringToFront();old.Dispose();page.Padding=Padding.Empty;
 }
}
