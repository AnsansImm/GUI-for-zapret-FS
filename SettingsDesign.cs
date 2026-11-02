namespace ZapretGui;
sealed partial class MainForm
{
 void ArrangeSettings(TabControl main)
 {
  var page=main.TabPages.Cast<TabPage>().First(p=>p.Text=="Настройки");
  var old=page.Controls.OfType<TabControl>().First();
  var inner=new ContentTabs(){Dock=DockStyle.Fill,Margin=Padding.Empty};
  foreach(var p in old.TabPages.Cast<TabPage>().ToArray())inner.TabPages.Add(p);
  var shell=new TableLayoutPanel(){Dock=DockStyle.Fill,ColumnCount=1,RowCount=2,BackColor=Background,Margin=Padding.Empty};
  shell.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,100));shell.RowStyles.Add(new RowStyle(SizeType.Absolute,62));shell.RowStyles.Add(new RowStyle(SizeType.Percent,100));
  var menu=new FlowLayoutPanel(){Dock=DockStyle.Fill,BackColor=Background,WrapContents=false,Margin=Padding.Empty};
  var nav=new List<PillButton>();
  foreach(TabPage sub in inner.TabPages){var index=inner.TabPages.IndexOf(sub);var b=new PillButton(){Text=sub.Text,Width=145,Height=38,Margin=new Padding(0,6,10,0),Active=index==0};nav.Add(b);menu.Controls.Add(b);b.Click+=(_,_)=>{inner.SelectedIndex=index;foreach(var item in nav){item.Active=item==b;item.Invalidate();}};}
  shell.Controls.Add(menu,0,0);shell.Controls.Add(inner,0,1);page.Controls.Add(shell);shell.BringToFront();old.Dispose();page.Padding=Padding.Empty;
  foreach(TabPage sub in inner.TabPages)
  {
   sub.Padding=Padding.Empty;sub.BackColor=Background;
   var flow=sub.Controls.OfType<FlowLayoutPanel>().First();
   var grid=new TableLayoutPanel(){Dock=DockStyle.Fill,ColumnCount=2,RowCount=1,BackColor=Background,Margin=Padding.Empty};
   grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,50));grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,50));grid.RowStyles.Add(new RowStyle(SizeType.Percent,100));
   sub.Controls.Add(grid);grid.BringToFront();
   if(sub.Text=="Game Filter")
   {
    var mode=flow.Controls.OfType<ListBox>().First();var fields=flow.Controls.OfType<TextBox>().ToArray();var actions=flow.Controls.OfType<Button>().ToArray();
    var content=CardContent("Game Filter","Выберите протоколы и укажите игровые порты.");content.Margin=new Padding(0,0,16,0);
    var modes=new TableLayoutPanel(){Height=44,ColumnCount=4,RowCount=1,Margin=new Padding(0,0,0,16),BackColor=Surface};
    var selections=new List<PillButton>();
    var captions=new[]{"Отключён","TCP","UDP","TCP + UDP"};
    for(var i=0;i<4;i++){var index=i;modes.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,25));var b=new PillButton(){Text=captions[i],Dock=DockStyle.Fill,Margin=new Padding(0,0,i==3?0:6,0),Active=mode.SelectedIndex==i};modes.Controls.Add(b,i,0);selections.Add(b);b.Click+=(_,_)=>mode.SelectedIndex=index;}
    var hidden=new Panel(){Visible=false};Controls.Add(hidden);hidden.Controls.Add(mode);
    mode.SelectedIndexChanged+=(_,_)=>{for(var i=0;i<selections.Count;i++){selections[i].Active=mode.SelectedIndex==i;selections[i].Invalidate();}fields[0].Enabled=mode.SelectedIndex is 1 or 3;fields[1].Enabled=mode.SelectedIndex is 2 or 3;};
    fields[0].Enabled=mode.SelectedIndex is 1 or 3;fields[1].Enabled=mode.SelectedIndex is 2 or 3;
    content.Controls.Add(Description("Режим фильтрации"));content.Controls.Add(modes);
    for(var i=0;i<2;i++){var label=Description(i==0?"Порты TCP":"Порты UDP");label.Margin=new Padding(0,0,0,8);content.Controls.Add(label);fields[i].Margin=new Padding(0,0,0,18);content.Controls.Add(fields[i]);}
    foreach(var button in actions){StyleAction(button,button==actions.Last());content.Controls.Add(button);}
    actions[0].Text="Прочитать настройки";actions[1].Text="Сохранить настройки";
    var help=CardContent("Как настроить","Game Filter добавляет обработку игровых портов к выбранной стратегии.");
    help.Controls.Add(Heading("Формат портов"));help.Controls.Add(Description("Один порт: 443\nДиапазон: 1024-65535\nНесколько значений: 443,5000-6000"));
    help.Controls.Add(Heading("Применение"));help.Controls.Add(Description("Сохраните настройки, затем заново примените стратегию к службе на странице «Управление». При отдельном запуске перезапустите батник."));
    grid.Controls.Add(content,0,0);grid.Controls.Add(help,1,0);
   }
   else if(sub.Text=="IPSet")
   {
    var actions=flow.Controls.OfType<Button>().ToArray();
    var content=CardContent("IPSet","Выберите режим обработки IP-адресов.");content.Margin=new Padding(0,0,16,0);
    foreach(var button in actions){StyleAction(button,button==actions.Last());content.Controls.Add(button);}
    var help=CardContent("Список адресов","Резервная копия сохраняется при переключении загруженного списка в другой режим.");
    help.Controls.Add(Heading("Все IP"));help.Controls.Add(Description("Использовать пустой список адресов для обработки всех IP выбранной стратегией."));
    help.Controls.Add(Heading("Восстановление"));help.Controls.Add(Description("«Восстановить список» возвращает сохранённую резервную копию. После изменения режима перезапустите Zapret."));
    grid.Controls.Add(content,0,0);grid.Controls.Add(help,1,0);
   }
   else
   {
    var columns=flow.Controls.OfType<TableLayoutPanel>().First();
    var boxes=columns.Controls.OfType<FlowLayoutPanel>().ToArray();
    for(var i=0;i<boxes.Length;i++)
    {
     var box=boxes[i];var labels=box.Controls.OfType<Label>().ToArray();var list=box.Controls.OfType<ListBox>().Single();var actions=box.Controls.OfType<Button>().ToArray();
     var card=new TableLayoutPanel(){Dock=DockStyle.Fill,ColumnCount=1,RowCount=4,BackColor=Surface,Margin=new Padding(0,0,i==0?16:0,0)};
     card.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,100));card.RowStyles.Add(new RowStyle(SizeType.Absolute,44));card.RowStyles.Add(new RowStyle(SizeType.Absolute,56));card.RowStyles.Add(new RowStyle(SizeType.Percent,100));card.RowStyles.Add(new RowStyle(SizeType.Absolute,150));
     labels[0].Font=new Font("Segoe UI Semibold",17);labels[0].ForeColor=Ink;labels[0].AutoSize=true;card.Controls.Add(labels[0],0,0);
     labels[1].Dock=DockStyle.Fill;labels[1].AutoEllipsis=true;labels[1].ForeColor=Color.FromArgb(180,174,189);card.Controls.Add(labels[1],0,1);
     list.Dock=DockStyle.Fill;list.BorderStyle=BorderStyle.None;list.BackColor=Surface;list.Margin=new Padding(0,0,0,14);card.Controls.Add(list,0,2);
     var buttons=new TableLayoutPanel(){Dock=DockStyle.Fill,ColumnCount=1,RowCount=3,BackColor=Surface,Margin=Padding.Empty};buttons.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,100));
     for(var j=0;j<actions.Length;j++){buttons.RowStyles.Add(new RowStyle(SizeType.Absolute,48));StyleAction(actions[j],j==0);actions[j].Dock=DockStyle.Top;buttons.Controls.Add(actions[j],0,j);}
     card.Controls.Add(buttons,0,3);grid.Controls.Add(card,i,0);
    }
   }
   flow.Dispose();
  }
 }
}
