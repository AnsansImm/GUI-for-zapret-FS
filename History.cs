using System.Runtime.InteropServices;
namespace ZapretGui;
static class History
{
 [DllImport("winsqlite3",CallingConvention=CallingConvention.Cdecl)]static extern int sqlite3_open_v2([MarshalAs(UnmanagedType.LPUTF8Str)]string path,out IntPtr db,int flags,IntPtr vfs);
 [DllImport("winsqlite3",CallingConvention=CallingConvention.Cdecl)]static extern int sqlite3_close(IntPtr db);
 [DllImport("winsqlite3",CallingConvention=CallingConvention.Cdecl)]static extern IntPtr sqlite3_errmsg(IntPtr db);
 [DllImport("winsqlite3",CallingConvention=CallingConvention.Cdecl)]static extern int sqlite3_busy_timeout(IntPtr db,int ms);
 [DllImport("winsqlite3",CallingConvention=CallingConvention.Cdecl)]static extern IntPtr sqlite3_backup_init(IntPtr dest,[MarshalAs(UnmanagedType.LPUTF8Str)]string name,IntPtr source,[MarshalAs(UnmanagedType.LPUTF8Str)]string sourceName);
 [DllImport("winsqlite3",CallingConvention=CallingConvention.Cdecl)]static extern int sqlite3_backup_step(IntPtr backup,int pages);
 [DllImport("winsqlite3",CallingConvention=CallingConvention.Cdecl)]static extern int sqlite3_backup_finish(IntPtr backup);
 [DllImport("winsqlite3",CallingConvention=CallingConvention.Cdecl)]static extern int sqlite3_prepare_v2(IntPtr db,[MarshalAs(UnmanagedType.LPUTF8Str)]string sql,int length,out IntPtr stmt,IntPtr tail);
 [DllImport("winsqlite3",CallingConvention=CallingConvention.Cdecl)]static extern int sqlite3_step(IntPtr stmt);
 [DllImport("winsqlite3",CallingConvention=CallingConvention.Cdecl)]static extern int sqlite3_finalize(IntPtr stmt);
 [DllImport("winsqlite3",CallingConvention=CallingConvention.Cdecl)]static extern IntPtr sqlite3_column_text(IntPtr stmt,int column);
 public static List<string[]> Read(string path)
 {
  IntPtr src=IntPtr.Zero,dst=IntPtr.Zero,stmt=IntPtr.Zero;
  try{
   if(sqlite3_open_v2(path,out src,1,IntPtr.Zero)!=0)throw new IOException("Не удалось открыть историю браузера");
   sqlite3_busy_timeout(src,200);if(sqlite3_open_v2(":memory:",out dst,6,IntPtr.Zero)!=0)throw new IOException("Не удалось создать снимок истории");
   var backup=sqlite3_backup_init(dst,"main",src,"main");if(backup==IntPtr.Zero)throw new IOException("Не удалось создать снимок SQLite");
   try{var deadline=DateTime.UtcNow.AddSeconds(5);int rc;do{rc=sqlite3_backup_step(backup,256);if(rc==5||rc==6)Thread.Sleep(50);if(DateTime.UtcNow>deadline)throw new IOException("История занята. Повторите попытку");}while(rc==0||rc==5||rc==6);if(rc!=101)throw new IOException("Ошибка снимка SQLite: "+rc);}finally{sqlite3_backup_finish(backup);}
   var sql=Path.GetFileName(path).Equals("places.sqlite",StringComparison.OrdinalIgnoreCase)?"SELECT url,COALESCE(title,'') FROM moz_places WHERE last_visit_date IS NOT NULL ORDER BY last_visit_date DESC LIMIT 100":"SELECT url,COALESCE(title,'') FROM urls ORDER BY last_visit_time DESC LIMIT 100";
   if(sqlite3_prepare_v2(dst,sql,-1,out stmt,IntPtr.Zero)!=0)throw new IOException(Marshal.PtrToStringUTF8(sqlite3_errmsg(dst)));
   var result=new List<string[]>();int status;while((status=sqlite3_step(stmt))==100)result.Add([Marshal.PtrToStringUTF8(sqlite3_column_text(stmt,0))??"",Marshal.PtrToStringUTF8(sqlite3_column_text(stmt,1))??""]);if(status!=101)throw new IOException("Ошибка чтения истории");return result;
  }finally{if(stmt!=IntPtr.Zero)sqlite3_finalize(stmt);if(dst!=IntPtr.Zero)sqlite3_close(dst);if(src!=IntPtr.Zero)sqlite3_close(src);}
 }
}
