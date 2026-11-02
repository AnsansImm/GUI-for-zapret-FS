using System.Drawing.Drawing2D;
namespace ZapretGui;
sealed partial class MainForm
{
 Action? refreshStrategyView;
 void ArrangeManagement(TabControl tabs)
 {
  var page=tabs.TabPages[0];var old=page.Controls.OfType<FlowLayoutPanel>().First();var controls=old.Controls.Cast<Control>().ToArray();
  var grid=new TableLayoutPanel(){Dock=DockStyle.Fill,ColumnCount=2,RowCount=2,BackColor=Background,Padding=new Padding(0,4,0,0)};grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,54));grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,46));grid.RowStyles.Add(new RowStyle(SizeType.Absolute,330));grid.RowStyles.Add(new RowStyle(SizeType.Percent,100));page.Controls.Add(grid);grid.BringToFront();
  FlowLayoutPanel Card(string title,string subtitle,int column,int row,int span=1)
  {
   var card=new RoundedCard(){Dock=DockStyle.Fill,Margin=new Padding(0,0,column==0?18:0,row==0&&span==1?16:0),Padding=new Padding(18)};grid.Controls.Add(card,column,row);grid.SetRowSpan(card,span);
   var content=new FlowLayoutPanel(){Dock=DockStyle.Fill,FlowDirection=FlowDirection.TopDown,WrapContents=false,AutoScroll=false,BackColor=Surface};card.Controls.Add(content);
   content.Controls.Add(new Label(){Text=title,AutoSize=true,Font=new Font("Segoe UI Semibold",17),ForeColor=Ink,Margin=new Padding(0,0,0,8)});content.Controls.Add(new Label(){Text=subtitle,Height=30,ForeColor=Color.FromArgb(165,160,174),Margin=new Padding(0,0,0,12)});return content;
  }
  var strategy=Card("Стратегии","Выберите стратегию из списка и примените к службе.",0,0,2);var service=Card("Служба Windows","Фоновая работа Zapret.",1,0);var tools=Card("Дополнительные действия","Отдельный запуск и обслуживание.",1,1);
  var choose=controls.OfType<Button>().First(x=>x.Text=="Выбрать папку");
  var pathLabel=new Label(){AutoEllipsis=true,Text=root.Text.Length>0?root.Text:"Папка Zapret не выбрана",ForeColor=Color.FromArgb(190,183,199),Width=410,Height=24,Location=new Point(0,3)};
  var folderPanel=new Panel(){Width=600,Height=44,Anchor=AnchorStyles.Top|AnchorStyles.Right,BackColor=Background};folderPanel.Controls.Add(pathLabel);folderPanel.Controls.Add(choose);choose.AutoSize=false;choose.SetBounds(420,0,165,38);choose.Text="Выбрать папку";
  var header=tabs.FindForm()!.Controls.OfType<TableLayoutPanel>().First().GetControlFromPosition(0,0)!;header.Controls.Add(folderPanel);void FitFolder(){folderPanel.Location=new Point(Math.Max(400,header.Width-600),0);}header.SizeChanged+=(_,_)=>FitFolder();FitFolder();root.TextChanged+=(_,_)=>pathLabel.Text=root.Text.Length>0?root.Text:"Папка Zapret не выбрана";
  // Keep existing strategy bindings while presenting a vertical list.
  var hidden=new Panel(){Visible=false};Controls.Add(hidden);hidden.Controls.Add(root);hidden.Controls.Add(strategies);
  var list=new ListBox(){BorderStyle=BorderStyle.None,BackColor=Surface,ForeColor=Ink,Font=new Font("Segoe UI",11),ItemHeight=32,IntegralHeight=false,Height=300};strategy.Controls.Add(list);
  refreshStrategyView=()=>{var selected=strategies.SelectedItem;list.Items.Clear();list.Items.AddRange(strategies.Items.Cast<object>().ToArray());if(selected!=null)list.SelectedItem=selected;};list.SelectedIndexChanged+=(_,_)=>{if(list.SelectedItem!=null)strategies.SelectedItem=list.SelectedItem;};refreshStrategyView();
  foreach(var c in controls.OfType<Button>().Where(x=>x!=choose)){var dest=c.Text=="Применить стратегию к службе"?strategy:c.Text is "Запустить службу" or "Остановить службу" or "Состояние и стратегия службы"?service:tools;dest.Controls.Add(c);c.AutoSize=false;c.Height=38;c.Margin=new Padding(0,0,0,10);if(dest==strategy){c.BackColor=Accent;c.ForeColor=Background;}else if(c.Text=="Удалить службу"){c.BackColor=Danger;c.ForeColor=Ink;}}
  void FitCards(){foreach(var content in new[]{strategy,service,tools})foreach(Control c in content.Controls)if(c is not Label label||!label.AutoSize)c.Width=Math.Max(180,content.ClientSize.Width-22);list.Height=Math.Max(130,strategy.ClientSize.Height-145);}
  grid.SizeChanged+=(_,_)=>FitCards();strategy.SizeChanged+=(_,_)=>FitCards();service.SizeChanged+=(_,_)=>FitCards();tools.SizeChanged+=(_,_)=>FitCards();FitCards();  var status=new Label(){Text="●  Проверка состояния…",AutoSize=true,ForeColor=Accent,Margin=new Padding(0,0,0,16)};service.Controls.Add(status);service.Controls.SetChildIndex(status,2);
  bool polling=false;async Task RefreshStatus(){if(polling)return;polling=true;try{var output=await Run("sc.exe","query","zapret");status.Text=output.Contains("RUNNING")?"●  Служба работает":output.Contains("STOPPED")?"○  Служба остановлена":output.Contains("START_PENDING")?"●  Служба запускается":output.Contains("STOP_PENDING")?"●  Служба останавливается":"○  Служба не установлена";}catch{status.Text="○  Состояние недоступно";}finally{polling=false;}}
  var statusTimer=new System.Windows.Forms.Timer(){Interval=4000};statusTimer.Tick+=async(_,_)=>await RefreshStatus();Shown+=async(_,_)=>{statusTimer.Start();await RefreshStatus();};FormClosed+=(_,_)=>statusTimer.Dispose();
  old.Dispose();page.Padding=Padding.Empty;
 }
 sealed class RoundedCard:Panel
 {
  public RoundedCard(){DoubleBuffered=true;BackColor=Background;}
  protected override void OnPaint(PaintEventArgs e){base.OnPaint(e);if(Width<2||Height<2)return;e.Graphics.SmoothingMode=SmoothingMode.AntiAlias;var w=Width-1;var h=Height-1;var d=Math.Min(36,Math.Min(w,h));using var path=new GraphicsPath();path.AddArc(0,0,d,d,180,90);path.AddArc(w-d,0,d,d,270,90);path.AddArc(w-d,h-d,d,d,0,90);path.AddArc(0,h-d,d,d,90,90);path.CloseFigure();using var brush=new SolidBrush(Surface);e.Graphics.FillPath(brush,path);using var pen=new Pen(Color.FromArgb(70,68,74));e.Graphics.DrawPath(pen,path);}
 }
}
