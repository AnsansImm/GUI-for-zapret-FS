using System.Text.Json;
namespace ZapretGui;
static class ListStore
{
    const string General = "list-general-user.txt", Exclude = "list-exclude-user.txt";
    const string Journal = ".zapret-gui-transaction.json";
    public static FileStream Lock(string directory) => new(Path.Combine(directory, ".zapret-gui.lock"), FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
    public static void Atomic(string path, string text)
    {
        var temp = path + "." + Guid.NewGuid() + ".tmp";
        try
        {
            using (var stream = new FileStream(temp, FileMode.CreateNew, FileAccess.Write, FileShare.None))
            {
                var bytes = new System.Text.UTF8Encoding(false).GetBytes(text);
                stream.Write(bytes); stream.Flush(true);
            }
            File.Move(temp, path, true);
        }
        finally { if (File.Exists(temp)) File.Delete(temp); }
    }
    public static void Recover(string directory)
    {
        var journal = Path.Combine(directory, Journal);
        if (!File.Exists(journal)) return;
        var old = JsonSerializer.Deserialize<Dictionary<string,string>>(File.ReadAllText(journal)) ?? throw new IOException("Повреждён журнал списков");
        Atomic(Path.Combine(directory, General), old["general"]);
        Atomic(Path.Combine(directory, Exclude), old["exclude"]);
        File.Delete(journal);
    }
    public static string Domain(string value)
    {
        value = value.Trim();
        if (!Uri.TryCreate(value.Contains("://") ? value : "https://" + value, UriKind.Absolute, out var uri) ||
            (uri.Scheme != "http" && uri.Scheme != "https") || Uri.CheckHostName(uri.IdnHost) != UriHostNameType.Dns)
            throw new ArgumentException("Введите домен или HTTP/HTTPS ссылку");
        var host = uri.IdnHost.TrimEnd('.').ToLowerInvariant();
        return host.StartsWith("www.") ? host[4..] : host;
    }
    public static string Add(string directory, string value, bool exclude)
    {
        var host = Domain(value);
        using var guard = Lock(directory);
        Recover(directory);
        string Read(string name) => File.Exists(Path.Combine(directory,name)) ? File.ReadAllText(Path.Combine(directory,name)) : "";
        var old = new Dictionary<string,string>{{"general",Read(General)},{"exclude",Read(Exclude)}};
        string Change(string text, bool add)
        {
            var lines = text.Split('\n').Select(x=>x.Trim()).Where(x=>!x.Equals(host,StringComparison.OrdinalIgnoreCase) && !x.Equals("www."+host,StringComparison.OrdinalIgnoreCase)).Where(x=>x.Length>0).Distinct(StringComparer.OrdinalIgnoreCase).ToList();
            if(add) lines.Add(host);
            if(!lines.Any(x=>!x.StartsWith('#'))) lines.Add("domain.example.abc");
            return string.Join("\n",lines)+"\n";
        }
        Atomic(Path.Combine(directory,Journal),JsonSerializer.Serialize(old));
        try
        {
            Atomic(Path.Combine(directory,General),Change(old["general"],!exclude));
            Atomic(Path.Combine(directory,Exclude),Change(old["exclude"],exclude));
            File.Delete(Path.Combine(directory,Journal));
        }
        catch { Recover(directory); throw; }
        return host;
    }
    public static void Save(string path,string text,string expected)
    {
        var directory=Path.GetDirectoryName(path)!;
        using var guard=Lock(directory);
        Recover(directory);
        if(File.ReadAllText(path)!=expected) throw new IOException("Список изменился на диске. Откройте его заново перед сохранением.");
        Atomic(path,text);
    }
}
