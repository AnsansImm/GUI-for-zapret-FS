using System.Reflection;
using System.Text.Json;

namespace ZapretGui;

static class Verification
{
    static int passed;
    static void Equal<T>(T expected, T actual)
    {
        if (!EqualityComparer<T>.Default.Equals(expected, actual))
            throw new Exception($"Expected {expected}; got {actual}");
        passed++;
    }
    static void Throws<T>(Action action) where T : Exception
    {
        try { action(); } catch (T) { passed++; return; }
        throw new Exception($"Expected {typeof(T).Name}");
    }
    static readonly BindingFlags Private = BindingFlags.Instance | BindingFlags.NonPublic;
    static T Field<T>(MainForm form, string name) => (T)typeof(MainForm).GetField(name, Private)!.GetValue(form)!;

    [STAThread]
    static int Main()
    {
        var temp = Path.Combine(AppContext.BaseDirectory, "zapret-verification-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(temp);
        try
        {
            Equal("youtube.com", ListStore.Domain("https://www.YouTube.com/watch?v=1"));
            Equal("sub.example.org", ListStore.Domain("sub.example.org"));
            Equal("xn--e1afmkfd.xn--p1ai", ListStore.Domain("пример.рф"));
            Throws<ArgumentException>(() => ListStore.Domain("file:///C:/test"));
            Throws<ArgumentException>(() => ListStore.Domain("127.0.0.1"));
            Equal("youtube.com\n# comment\nsub.example.org", EditorText.Normalize("https://www.youtube.com/\r\nwww.youtube.com\nyoutube.com\n# comment\nsub.example.org", false));
            Equal("1.2.3.4/24\n2001:db8::/32\n1.2.3.4-1.2.3.5", EditorText.Normalize("1.2.3.4/24 2001:db8::/32\r\n1.2.3.4-1.2.3.5", true));
            Equal("a\r\nb\r\nc\r\nd", EditorText.Display("a\nb\rc\r\nd"));
            Equal("example.org\nxn--e1afmkfd.xn--p1ai", EditorText.Normalize("WWW.EXAMPLE.ORG example.org пример.рф", false));
            Equal("# video\nyoutube.com", EditorText.Normalize("https://www.youtube.com/watch#part # video", false));
            Equal("# both\nexample.org\nexample.net", EditorText.Normalize("example.org example.net # both", false));
            Equal("example.org\n# duplicate note", EditorText.Normalize("example.org\nEXAMPLE.ORG # duplicate note", false));
            Equal("# IPv6\n2001:db8::1/128", EditorText.Normalize("2001:0DB8::1/128 # IPv6", true));
            Equal("0.0.0.0/0\n255.255.255.255/32\n::/0\n::1/128", EditorText.Normalize("0.0.0.0/0 255.255.255.255/32 ::/0 ::1/128", true));
            Equal("# only comment", EditorText.Normalize("  # only comment", true));
            Equal("", EditorText.Normalize("\uFEFF\r\n  ", true));
            foreach (var invalid in new[] { "bad_domain.org", "-bad.org", "bad-.org", "example..org", "example.org/path", "user@example.org", "example.org:443", "127.0.0.1", "https://bad_domain.org", "ftp://example.org" })
                Throws<IOException>(() => EditorText.Normalize(invalid, false));
            foreach (var invalid in new[] { "999.1.1.1", "127.1", "0x7f000001", "01.2.3.4", "1.2.3.4/33", "::1/129", "1.2.3.4/-1", "1.2.3.4/+1", "1.2.3.4/", "1.2.3.4oops", "1.2.3.5-1.2.3.4", "1.2.3.4-::1", "fe80::1%2" })
                Throws<IOException>(() => EditorText.Normalize(invalid, true));
            try { EditorText.Normalize("example.org\n# note\nbad_domain.org", false); throw new Exception("Invalid domain accepted"); }
            catch (IOException error) { Equal(true, error.Message.Contains("Строка 3")); }
            foreach (var ports in new[] { "443", "443,1024-65535", "1-65535" }) Equal(true, Backend.Ports(ports));
            foreach (var ports in new[] { "", "0", "65536", "443,", "80-1", "1-2-3", "abc" }) Equal(false, Backend.Ports(ports));
            Equal("'a''b'", Backend.Quote("a'b"));
            var expectedStrategies = new[] { "general (ALT).bat" }
                .Concat(Enumerable.Range(2, 12).Select(n => $"general (ALT{n}).bat"))
                .Concat(new[] { "general (EXP).bat", "general (FAKE TLS AUTO ALT).bat", "general (FAKE TLS AUTO ALT2).bat", "general (FAKE TLS AUTO ALT3).bat", "general (FAKE TLS AUTO).bat", "general (SIMPLE FAKE ALT).bat", "general (SIMPLE FAKE ALT2).bat", "general (SIMPLE FAKE).bat", "general.bat" }).ToArray();
            Equal(string.Join("\n", expectedStrategies), string.Join("\n", expectedStrategies.Reverse().OrderBy(n => n, StrategyOrder.Instance)));
            Equal(true, StrategyOrder.Instance.Compare("ALT99999999999999999999", "ALT100000000000000000000") < 0);

            var general = Path.Combine(temp, "list-general-user.txt");
            var exclude = Path.Combine(temp, "list-exclude-user.txt");
            ListStore.Add(temp, "www.youtube.com", false);
            ListStore.Add(temp, "youtube.com", false);
            Equal(1, File.ReadAllLines(general).Count(x => x == "youtube.com"));
            ListStore.Add(temp, "youtube.com", true);
            Equal(false, File.ReadAllText(general).Contains("youtube.com"));
            Equal(true, File.ReadAllText(exclude).Contains("youtube.com"));
            var before = File.ReadAllText(general);
            File.WriteAllText(general, "external.example\n");
            Throws<IOException>(() => ListStore.Save(general, "replacement.example", before));
            Equal("external.example\n", File.ReadAllText(general));
            var journal = Path.Combine(temp, ".zapret-gui-transaction.json");
            File.WriteAllText(journal, JsonSerializer.Serialize(new { general = "old.example\n", exclude = "excluded.example\n" }));
            using (ListStore.Lock(temp)) ListStore.Recover(temp);
            Equal("old.example\n", File.ReadAllText(general));
            Equal("excluded.example\n", File.ReadAllText(exclude));
            Equal(false, File.Exists(journal));
            File.WriteAllText(Path.Combine(temp, "strategy.bat"), "start \"zapret\" \"%BIN%winws.exe\" --filter-tcp=443 ^\r\n --hostlist=\"%LISTS%list-general-user.txt\" --filter-udp=%GameFilterUDP%");
            var args = Backend.Arguments(temp, "strategy.bat");
            Equal(true, args.Contains("--filter-tcp=443"));
            Equal(true, args.EndsWith("--filter-udp=12"));
            Equal(false, args.Contains('%'));
            Throws<IOException>(() => Backend.Arguments(temp, "../strategy.bat"));

            // Construct every page and exercise folder changes without touching services.
            ApplicationConfiguration.Initialize();
            using var form = new MainForm();
            var root = Field<TextBox>(form, "root");
            void Refresh() => typeof(MainForm).GetMethod("RefreshRoot", Private)!.Invoke(form, null);
            root.Text = temp;
            Refresh();
            Equal(1, Field<ComboBox>(form, "strategies").Items.Count);
            Field<Action>(form, "refreshStrategyView")();
            root.Text = Path.Combine(temp, "missing");
            Refresh();
            Equal(0, Field<ComboBox>(form, "strategies").Items.Count);
            var tabs = Descendants(form).OfType<TabControl>().First(t => t.TabPages.Count > 3);
            Equal(7, tabs.TabPages.Count);
            Equal(0, Descendants(tabs.TabPages[0]).OfType<ListBox>().Sum(b => b.Items.Count));
            Equal(0, Descendants(tabs.TabPages[3]).OfType<CheckedListBox>().Single().Items.Count);
            Equal(false, Field<System.Windows.Forms.Timer>(form, "saveTimer").Enabled);
            form.Size = new Size(600, 400);
            Equal(form.MinimumSize, form.Size);
            var display = Screen.FromControl(form).WorkingArea.Size;
            var requiredClient = new Size((int)Math.Ceiling(1240 * form.DeviceDpi / 96d), (int)Math.Ceiling(900 * form.DeviceDpi / 96d));
            // Hosted Windows runners can expose a 1024x768 desktop. WinForms limits
            // form dimensions to that desktop, so it cannot verify a larger layout.
            if (display.Width >= requiredClient.Width + 40 && display.Height >= requiredClient.Height + 60)
            {
                Equal(true, form.ClientSize.Width >= requiredClient.Width && form.ClientSize.Height >= requiredClient.Height);
                form.Opacity = 0;
                form.ShowInTaskbar = false;
                form.Show();
                form.PerformLayout();
                foreach (var control in Descendants(tabs.TabPages[0])) control.PerformLayout();
                foreach (var button in Descendants(tabs.TabPages[0]).OfType<Button>())
                {
                    if (!button.Parent!.ClientRectangle.Contains(button.Bounds))
                        throw new Exception($"Clipped button: {button.Text}; bounds={button.Bounds}; parent={button.Parent.ClientRectangle}");
                    passed++;
                }
                form.Close();
            }
            else Console.WriteLine($"SKIP: full-window geometry needs {requiredClient}; runner desktop is {display}. Form construction and minimum-size constraint were checked.");
            Console.WriteLine($"PASS: {passed} checks");
            return 0;
        }
        catch (Exception error) { Console.Error.WriteLine(error); return 1; }
        finally { Directory.Delete(temp, true); }
    }

    static IEnumerable<Control> Descendants(Control parent)
    {
        foreach (Control child in parent.Controls)
        {
            yield return child;
            foreach (var descendant in Descendants(child)) yield return descendant;
        }
    }
}
