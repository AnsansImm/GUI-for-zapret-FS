using System.Globalization;
using System.Net;
using System.Net.Sockets;
namespace ZapretGui;
static class EditorText
{
 public static string Display(string text)=>text.Replace("\r\n","\n").Replace('\r','\n').Replace("\n","\r\n");
 public static string Normalize(string input,bool ip)
 {
  var lines=new List<string>();var seen=new HashSet<string>(StringComparer.OrdinalIgnoreCase);var number=0;
  foreach(var source in input.TrimStart('\uFEFF').Replace("\r\n","\n").Replace('\r','\n').Split('\n'))
  {
   number++;var line=source.Trim();if(line.Length==0)continue;
   // Only a # at a token boundary starts a comment; URL fragments belong to the URL.
   var commentIndex=-1;for(var i=0;i<line.Length;i++)if(line[i]=='#'&&(i==0||char.IsWhiteSpace(line[i-1]))){commentIndex=i;break;}
   var comment=commentIndex<0?null:line[commentIndex..];
   var entries=(commentIndex<0?line:line[..commentIndex]).Split((char[]?)null,StringSplitOptions.RemoveEmptyEntries);
   var normalized=new List<string>();
   foreach(var entry in entries)
   {
    try
    {
     var value=ip?NormalizeIp(entry):NormalizeDomain(entry);
     if(ip||seen.Add(value))normalized.Add(value);
    }
    catch(ArgumentException){throw new IOException($"Строка {number}: неверная {(ip?"IP-запись":"домен или HTTP/HTTPS ссылка")} «{entry}». Файл не изменён.");}
   }
   // Store comments as separate lines so Zapret never treats comment text as an address.
   if(comment!=null)lines.Add(comment);
   lines.AddRange(normalized);
  }
  return string.Join("\n",lines);
 }
 static string NormalizeDomain(string value)
 {
  string host;
  if(value.Contains("://"))host=ListStore.Domain(value);
  else
  {
   if(value.IndexOfAny(['/', '\\', ':', '@', '?', '#'])>=0)throw new ArgumentException();
   host=new IdnMapping().GetAscii(value.TrimEnd('.')).ToLowerInvariant();
   if(host.StartsWith("www.",StringComparison.Ordinal))host=host[4..];
  }
  if(host.Length is 0 or >253||Uri.CheckHostName(host)!=UriHostNameType.Dns)throw new ArgumentException();
  foreach(var label in host.Split('.'))
   if(label.Length is 0 or >63||label[0]=='-'||label[^1]=='-'||label.Any(c=>!char.IsAsciiLetterOrDigit(c)&&c!='-'))throw new ArgumentException();
  return host;
 }
 static IPAddress Address(string value)
 {
  if(value.Contains('%')||!IPAddress.TryParse(value,out var address))throw new ArgumentException();
  if(address.AddressFamily==AddressFamily.InterNetwork)
  {
   var parts=value.Split('.');
   if(parts.Length!=4||parts.Any(p=>p.Length==0||p.Any(c=>!char.IsAsciiDigit(c))||p.Length>1&&p[0]=='0'||!byte.TryParse(p,out _)))throw new ArgumentException();
  }
  return address;
 }
 static string NormalizeIp(string value)
 {
  if(value.Contains('/'))
  {
   var parts=value.Split('/');if(parts.Length!=2)throw new ArgumentException();
   var address=Address(parts[0]);
   if(parts[1].Length==0||parts[1].Any(c=>!char.IsAsciiDigit(c))||!int.TryParse(parts[1],out var prefix)||prefix>(address.AddressFamily==AddressFamily.InterNetwork?32:128))throw new ArgumentException();
   return address+"/"+prefix;
  }
  if(value.Contains('-'))
  {
   var parts=value.Split('-');if(parts.Length!=2)throw new ArgumentException();
   var first=Address(parts[0]);var last=Address(parts[1]);
   if(first.AddressFamily!=last.AddressFamily||first.GetAddressBytes().AsSpan().SequenceCompareTo(last.GetAddressBytes())>0)throw new ArgumentException();
   return first+"-"+last;
  }
  return Address(value).ToString();
 }
}
