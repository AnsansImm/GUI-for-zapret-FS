using System.Drawing.Drawing2D;
namespace ZapretGui;
sealed partial class MainForm
{
 static readonly Color Surface=Color.FromArgb(39,39,41), Background=Color.FromArgb(28,28,30), Ink=Color.FromArgb(240,238,243), Accent=Color.FromArgb(190,128,230);
 static readonly Color Danger=Color.FromArgb(180,52,63);
 void ApplyDesign(TabControl tabs,ListBox oldNav)
 {
  SuspendLayout();Controls.Remove(oldNav);oldNav.Dispose();
  Font=new Font("Segoe UI",10);BackColor=Background;ForeColor=Ink;Padding=new Padding(18);Text=AppInfo.Title;
  // Reserve enough space for every page, including both service action cards.
  void SetMinimumWindowSize(int dpi){var scale=dpi/96d;MinimumSize=SizeFromClientSize(new Size((int)Math.Ceiling(1240*scale),(int)Math.Ceiling(900*scale)));}
  SetMinimumWindowSize(DeviceDpi);Size=MinimumSize;
  DpiChanged+=(_,e)=>SetMinimumWindowSize(e.DeviceDpiNew);
  void Style(Control control)
  {
   control.BackColor=Surface;control.ForeColor=Ink;
   if(control is TabPage)control.Padding=new Padding(16);
   if(control is FlowLayoutPanel flow){if(flow.Padding.All==0)flow.Padding=new Padding(8);flow.AutoScroll=false;}
   if(control is Button button){button.FlatStyle=FlatStyle.Flat;button.FlatAppearance.BorderColor=Color.FromArgb(76,73,81);button.FlatAppearance.MouseOverBackColor=Color.FromArgb(65,53,74);button.Padding=new Padding(12,6,12,6);button.MinimumSize=new Size(0,36);button.Margin=new Padding(3,5,8,5);button.Cursor=Cursors.Hand;}
   if(control is TextBox text){text.BackColor=Background;text.BorderStyle=BorderStyle.FixedSingle;}
   if(control is ComboBox combo){combo.FlatStyle=FlatStyle.Flat;combo.BackColor=Color.FromArgb(52,51,55);}
   if(control is ListBox list){list.BorderStyle=BorderStyle.FixedSingle;list.BackColor=Background;}
   if(control is Label label)label.ForeColor=Color.FromArgb(199,194,205);
   foreach(Control child in control.Controls)Style(child);
  }
  Style(tabs);log.ReadOnly=true;log.BackColor=Background;log.ForeColor=Color.FromArgb(199,181,213);log.Font=new Font("Consolas",9);log.Height=120;log.BorderStyle=BorderStyle.None;
  var header=new Panel(){Dock=DockStyle.Top,Height=115,BackColor=Background};
  var brandImage=Icon!.ToBitmap();var brandIcon=new PictureBox(){Image=brandImage,SizeMode=PictureBoxSizeMode.Zoom,Size=new Size(32,32),Location=new Point(8,4),BackColor=Background};header.Controls.Add(brandIcon);Disposed+=(_,_)=>brandImage.Dispose();
  var brand=new Label(){Text=AppInfo.Name,AutoSize=true,Font=new Font("Segoe UI Semibold",17),ForeColor=Ink,Location=new Point(48,3)};header.Controls.Add(brand);
  var subtitle=new Label(){Text="v"+AppInfo.Version+" • Управление подключением",AutoSize=true,ForeColor=Color.FromArgb(158,153,164),Location=new Point(280,14)};header.Controls.Add(subtitle);
  var menu=new FlowLayoutPanel(){Dock=DockStyle.Bottom,Height=58,BackColor=Background,WrapContents=false,AutoScroll=false};header.Controls.Add(menu);
  var buttons=new List<PillButton>();
  foreach(TabPage page in tabs.TabPages){var index=tabs.TabPages.IndexOf(page);var button=new PillButton(){Text=page.Text,AutoSize=true,Height=40,MinimumSize=new Size(90,40),Margin=new Padding(0,4,8,4),Font=Font};button.Click+=(_,_)=>{tabs.SelectedIndex=index;foreach(var b in buttons){b.Active=b==button;b.Invalidate();}};buttons.Add(button);menu.Controls.Add(button);}
  if(buttons.Count>0)buttons[0].Active=true;
  void FitNavigation(){for(var i=0;i<buttons.Count;i++){buttons[i].AutoSize=false;buttons[i].MinimumSize=new Size(0,40);buttons[i].Width=Math.Max(86,TextRenderer.MeasureText(buttons[i].Text,buttons[i].Font).Width+28);buttons[i].Height=40;}}
  menu.ClientSizeChanged+=(_,_)=>FitNavigation();FitNavigation();
  var journalHeader=new Label(){Text="ЖУРНАЛ ДЕЙСТВИЙ",Dock=DockStyle.Bottom,Height=28,ForeColor=Color.FromArgb(158,153,164),BackColor=Background,Padding=new Padding(0,7,0,0),Font=new Font("Segoe UI",8)};
    Controls.Clear();
  var shell=new TableLayoutPanel(){Dock=DockStyle.Fill,ColumnCount=1,RowCount=4,BackColor=Background,Margin=Padding.Empty,Padding=Padding.Empty};
  shell.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,100));shell.RowStyles.Add(new RowStyle(SizeType.Absolute,115));shell.RowStyles.Add(new RowStyle(SizeType.Percent,100));shell.RowStyles.Add(new RowStyle(SizeType.Absolute,28));shell.RowStyles.Add(new RowStyle(SizeType.Absolute,70));
  header.Dock=DockStyle.Fill;journalHeader.Dock=DockStyle.Fill;log.Dock=DockStyle.Fill;tabs.Margin=Padding.Empty;header.Margin=Padding.Empty;log.Margin=Padding.Empty;
  shell.Controls.Add(header,0,0);shell.Controls.Add(tabs,0,1);shell.Controls.Add(journalHeader,0,2);shell.Controls.Add(log,0,3);Controls.Add(shell);
  ArrangeManagement(tabs); ArrangeLists(tabs); ArrangeBrowser(tabs); arrangeTestsView?.Invoke(); ArrangeSettings(tabs); ArrangeDiagnostics(tabs); ArrangeUpdates(tabs); FinishPageCards(tabs);
  tabs.SelectedIndexChanged+=(_,_)=>{for(var i=0;i<buttons.Count;i++){buttons[i].Active=i==tabs.SelectedIndex;buttons[i].Invalidate();}};
  ResumeLayout(true);
 }
 sealed class ContentTabs:TabControl
 {
  protected override void WndProc(ref Message m)
  {
   if(m.Msg==0x1328){m.Result=(IntPtr)1;return;}
   base.WndProc(ref m);
  }
 }
 sealed class PillButton:Button
 {
  [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
  public bool Active{get;set;}
  public PillButton(){SetStyle(ControlStyles.UserPaint|ControlStyles.AllPaintingInWmPaint|ControlStyles.OptimizedDoubleBuffer,true);FlatStyle=FlatStyle.Flat;FlatAppearance.BorderSize=0;Cursor=Cursors.Hand;Padding=new Padding(14,6,14,6);}
  protected override void OnPaint(PaintEventArgs e){if(Width<2||Height<2)return;e.Graphics.Clear(Parent?.BackColor??Background);e.Graphics.SmoothingMode=SmoothingMode.AntiAlias;var rect=new Rectangle(0,0,Width-1,Height-1);using var path=new GraphicsPath();var d=Math.Min(Math.Min(Height-1,Width-1),30);path.AddArc(0,0,d,d,180,90);path.AddArc(rect.Right-d,0,d,d,270,90);path.AddArc(rect.Right-d,rect.Bottom-d,d,d,0,90);path.AddArc(0,rect.Bottom-d,d,d,90,90);path.CloseFigure();var primary=BackColor==Accent;var danger=BackColor==Danger;using var brush=new SolidBrush(danger?Danger:primary?Accent:Active?Color.FromArgb(78,61,91):Surface);e.Graphics.FillPath(brush,path);using var pen=new Pen(danger?Color.FromArgb(224,92,104):Active?Accent:Color.FromArgb(69,67,73));e.Graphics.DrawPath(pen,path);var textRect=Rectangle.Inflate(rect,-10,-3);TextRenderer.DrawText(e.Graphics,Text,Font,textRect,!Enabled?Color.FromArgb(125,120,132):primary?Background:Active?Color.FromArgb(235,211,250):Ink,TextFormatFlags.HorizontalCenter|TextFormatFlags.VerticalCenter|TextFormatFlags.EndEllipsis|TextFormatFlags.SingleLine);}
 }
}
