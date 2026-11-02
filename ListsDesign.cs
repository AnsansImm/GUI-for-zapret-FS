namespace ZapretGui;
sealed partial class MainForm
{
 Action? refreshFileView;
 void ArrangeLists(TabControl tabs)
 {
  var page=tabs.TabPages[1];var oldBar=page.Controls.OfType<FlowLayoutPanel>().First();var save=oldBar.Controls.OfType<Button>().First();
  var hidden=new Panel(){Visible=false};Controls.Add(hidden);hidden.Controls.Add(files);
  var layout=new TableLayoutPanel(){Dock=DockStyle.Fill,ColumnCount=2,RowCount=1,BackColor=Background};layout.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute,300));layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,100));
  var sidebar=new TableLayoutPanel(){Dock=DockStyle.Fill,ColumnCount=1,RowCount=4,BackColor=Surface,Padding=new Padding(14),Margin=new Padding(0,0,16,0)};sidebar.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,100));sidebar.RowStyles.Add(new RowStyle(SizeType.Absolute,35));sidebar.RowStyles.Add(new RowStyle(SizeType.Percent,40));sidebar.RowStyles.Add(new RowStyle(SizeType.Absolute,35));sidebar.RowStyles.Add(new RowStyle(SizeType.Percent,60));
  ListBox Box()=>new(){Dock=DockStyle.Fill,BackColor=Surface,ForeColor=Ink,BorderStyle=BorderStyle.None,IntegralHeight=false,Font=new Font("Segoe UI",10),HorizontalScrollbar=true,DisplayMember="Name"};
  var user=Box();var other=Box();sidebar.Controls.Add(new Label(){Text="ПОЛЬЗОВАТЕЛЬСКИЕ СПИСКИ",AutoSize=true,ForeColor=Accent,Padding=new Padding(0,6,0,0)},0,0);sidebar.Controls.Add(user,0,1);sidebar.Controls.Add(new Label(){Text="ОСТАЛЬНЫЕ СПИСКИ",AutoSize=true,ForeColor=Color.FromArgb(180,174,189),Padding=new Padding(0,6,0,0)},0,2);sidebar.Controls.Add(other,0,3);
  var right=new TableLayoutPanel(){Dock=DockStyle.Fill,ColumnCount=1,RowCount=2,BackColor=Surface,Padding=new Padding(12),Margin=Padding.Empty};right.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,100));right.RowStyles.Add(new RowStyle(SizeType.Absolute,64));right.RowStyles.Add(new RowStyle(SizeType.Percent,100));
  var top=new TableLayoutPanel(){Dock=DockStyle.Fill,BackColor=Surface,ColumnCount=2,RowCount=1,Margin=Padding.Empty};top.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,100));top.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute,135));var title=new Label(){Text="Выберите список слева",Dock=DockStyle.Fill,AutoEllipsis=true,ForeColor=Ink,Font=new Font("Segoe UI Semibold",14),Padding=new Padding(0,6,0,0)};StyleAction(save,true);save.Dock=DockStyle.Top;save.Width=135;top.Controls.Add(title,0,0);top.Controls.Add(save,1,0);right.Controls.Add(top,0,0);editor.BorderStyle=BorderStyle.None;editor.Dock=DockStyle.Fill;right.Controls.Add(editor,0,1);layout.Controls.Add(sidebar,0,0);layout.Controls.Add(right,1,0);page.Controls.Add(layout);layout.BringToFront();oldBar.Dispose();page.Padding=Padding.Empty;
  bool syncing=false;
  void SyncSelection(){syncing=true;try{foreach(var box in new[]{user,other}){box.ClearSelected();for(int i=0;i<box.Items.Count;i++)if(((FileInfo)box.Items[i]).FullName==selected){box.SelectedIndex=i;break;}}title.Text=selected==null?"Выберите список слева":Path.GetFileName(selected);}finally{syncing=false;}}
  foreach(var box in new[]{user,other})box.SelectedIndexChanged+=(_,_)=>{if(syncing||box.SelectedItem is not FileInfo file)return;files.SelectedItem=file.FullName;SyncSelection();};
  refreshFileView=()=>{syncing=true;try{user.Items.Clear();other.Items.Clear();foreach(var path in files.Items.Cast<string>().OrderBy(Path.GetFileName)){var file=new FileInfo(path);(file.Name.Contains("-user",StringComparison.OrdinalIgnoreCase)?user:other).Items.Add(file);}}finally{syncing=false;}SyncSelection();};
  refreshFileView();
 }
}
