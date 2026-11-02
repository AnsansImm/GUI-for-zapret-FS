using System.Runtime.InteropServices;
namespace ZapretGui;
static class DarkControls
{
 [DllImport("uxtheme.dll",CharSet=CharSet.Unicode)]static extern int SetWindowTheme(IntPtr hwnd,string app,string? id);
 [DllImport("uxtheme.dll",EntryPoint="#133")]static extern bool AllowDarkModeForWindow(IntPtr hwnd,bool allow);
 [DllImport("uxtheme.dll",EntryPoint="#135")]static extern int SetPreferredAppMode(int mode);
 [DllImport("dwmapi.dll")]static extern int DwmSetWindowAttribute(IntPtr hwnd,int attribute,ref int value,int size);
 static void ApplyTitleBar(Form form)
 {
  if(!OperatingSystem.IsWindowsVersionAtLeast(10,0,17763))return;
  var enabled=1;
  if(DwmSetWindowAttribute(form.Handle,20,ref enabled,sizeof(int))<0)
   DwmSetWindowAttribute(form.Handle,19,ref enabled,sizeof(int));
 }
 public static void Initialize(){try{if(OperatingSystem.IsWindowsVersionAtLeast(10,0,18362))SetPreferredAppMode(2);}catch(EntryPointNotFoundException){}}
 public static void Attach(Control control)
 {
  void Apply(){if(!control.IsHandleCreated)return;if(control is Form form)ApplyTitleBar(form);try{if(OperatingSystem.IsWindowsVersionAtLeast(10,0,18362))AllowDarkModeForWindow(control.Handle,true);SetWindowTheme(control.Handle,"DarkMode_Explorer",null);}catch(EntryPointNotFoundException){}}
  control.HandleCreated+=(_,_)=>Apply();if(control.IsHandleCreated)Apply();
  control.ControlAdded+=(_,e)=>{if(e.Control!=null)Attach(e.Control);};
  foreach(Control child in control.Controls)Attach(child);
 }
}
