namespace ZapretGui;
sealed partial class MainForm {
 void BuildFeatures(TabControl tabs,Control manage){ BuildTests(tabs); BuildSettings(tabs); BuildUpdates(tabs);
  AsyncButton(manage,"Применить стратегию к службе",()=>Backend.Install(root.Text,strategies.Text));
  AsyncButton(manage,"Удалить службу",()=>Backend.Power("$s=Get-Service zapret;Stop-Service zapret;$s.WaitForStatus('Stopped',[TimeSpan]::FromSeconds(20));sc.exe delete zapret;if($LASTEXITCODE){throw 'Ошибка удаления'}",true),confirm:true);
  var diag=Page(tabs,"Диагностика");AsyncButton(diag,"Проверить систему",CheckSystemCards);

 }
 FlowLayoutPanel Page(TabControl tabs,string title){var page=new TabPage(title);tabs.TabPages.Add(page);var p=new FlowLayoutPanel(){Dock=DockStyle.Fill,FlowDirection=FlowDirection.TopDown,AutoScroll=false};page.Controls.Add(p);return p;}
 void AsyncButton(Control parent,string title,Func<Task<string>> action,bool confirm=false){var b=new PillButton(){Text=title,AutoSize=true};parent.Controls.Add(b);b.Click+=async(_,_)=>{if(confirm&&MessageBox.Show(this,"Удалить службу Zapret?","Удаление службы",MessageBoxButtons.OKCancel,MessageBoxIcon.Warning)!=DialogResult.OK)return;b.Enabled=false;ActionOutput(b,"Выполняется: "+title+"…");try{ActionOutput(b,await action());}catch(Exception e){ActionOutput(b,e.Message);}finally{if(!b.IsDisposed)b.Enabled=true;}};}
}
