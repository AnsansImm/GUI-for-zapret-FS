using System.Text.Json;
namespace ZapretGui;
sealed partial class MainForm
{
 TableLayoutPanel? diagnosticCards;
 async Task<string> CheckSystemCards()
 {
  var json=await Backend.Power("& "+Backend.Quote(Path.Combine(AppContext.BaseDirectory,"scripts/diagnostics.ps1"))+" -Root "+Backend.Quote(root.Text));
  using var doc=JsonDocument.Parse(json);
  ShowDiagnosticCards(doc.RootElement);
  return "Проверка системы завершена. Результаты показаны на карточках.";
 }
 void ShowDiagnosticCards(JsonElement results)
 {
  if(diagnosticCards==null)return;
  foreach(Control old in diagnosticCards.Controls.Cast<Control>().ToArray()){diagnosticCards.Controls.Remove(old);old.Dispose();}diagnosticCards.RowStyles.Clear();diagnosticCards.RowCount=(results.GetArrayLength()+1)/2;diagnosticCards.AutoScroll=true;
  var row=0;foreach(var item in results.EnumerateArray())
  {
   var title=item.GetProperty("title").GetString()??"";var state=item.GetProperty("state").GetString()??"Unknown";var detail=item.GetProperty("detail").GetString()??"";var good=state is "Running" or "Enabled" or "Passed";var unknown=state=="Unknown";var color=good?Color.FromArgb(137,214,164):state=="Failed"?Color.FromArgb(242,132,145):unknown||state=="Warning"?Color.FromArgb(232,195,123):Color.FromArgb(190,183,199);
   var card=new RoundedCard(){BackColor=Surface,Dock=DockStyle.Fill,Margin=new Padding(0,0,10,10),Padding=new Padding(14,10,14,8)};var body=new TableLayoutPanel(){Dock=DockStyle.Fill,ColumnCount=1,RowCount=2,BackColor=Surface};body.RowStyles.Add(new RowStyle(SizeType.Absolute,26));body.RowStyles.Add(new RowStyle(SizeType.Percent,100));var translated=state switch{"Running"=>"работает","Stopped"=>"остановлено","Enabled"=>"включено","Disabled"=>"отключено","Absent"=>"не установлено","Passed"=>"в порядке","Failed"=>"ошибка","Warning"=>"требует внимания",_=>"не удалось проверить"};body.Controls.Add(new Label(){Text=title+"   •   "+translated,AutoSize=true,ForeColor=color,Font=new Font("Segoe UI Semibold",11)},0,0);body.Controls.Add(new Label(){Text=string.IsNullOrWhiteSpace(detail)?(good?"Проверка пройдена":"Компонент сейчас не активен."):detail,Dock=DockStyle.Fill,AutoEllipsis=true,ForeColor=Color.FromArgb(180,174,189)},0,1);if(title=="TCP timestamps"&&state=="Disabled")
   {
    body.RowCount=3;body.RowStyles.Add(new RowStyle(SizeType.Absolute,44));
    AsyncButton(body,"Включить TCP timestamps",async()=>
    {
     var output=await Backend.Power("netsh interface tcp set global timestamps=enabled;if($LASTEXITCODE){throw 'Ошибка netsh'}",true);
     ActionOutput(body,output);
     return await CheckSystemCards();
    });
    var button=body.Controls.OfType<Button>().Single();StyleAction(button);button.Dock=DockStyle.Fill;button.Margin=Padding.Empty;
    body.SetCellPosition(button,new TableLayoutPanelCellPosition(0,2));
   }
   card.Controls.Add(body);if(row%2==0)diagnosticCards.RowStyles.Add(new RowStyle(SizeType.Absolute,115));if(body.RowCount==3)diagnosticCards.RowStyles[row/2].Height=160;diagnosticCards.Controls.Add(card,row%2,row/2);row++;
  }

 }
}
