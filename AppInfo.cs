using System.Reflection;
namespace ZapretGui;

static class AppInfo
{
    public const string Name = "gui-for-zapret-fs";
    public static string Version => typeof(AppInfo).Assembly
        .GetCustomAttribute<AssemblyInformationalVersionAttribute>()!
        .InformationalVersion.Split('+')[0];
    public static string Title => $"{Name} {Version}";
    public static Icon LoadIcon()
    {
        using var stream = typeof(AppInfo).Assembly.GetManifestResourceStream(Name + ".app.ico")
            ?? throw new InvalidOperationException("Не найдена иконка приложения");
        using var icon = new Icon(stream);
        return (Icon)icon.Clone();
    }
}
