namespace ZapretGui;

sealed partial class MainForm
{
    static Label Heading(string text) => new()
    {
        Text = text, AutoSize = true, ForeColor = Ink,
        Font = new Font("Segoe UI Semibold", 17), Margin = new Padding(0, 0, 0, 10)
    };

    static Label Description(string text) => new()
    {
        Text = text, AutoSize = true, UseCompatibleTextRendering = true, ForeColor = Color.FromArgb(180, 174, 189),
        Margin = new Padding(0, 0, 0, 16)
    };

    static FlowLayoutPanel CardContent(string title, string description)
    {
        var flow = new FlowLayoutPanel
        {
            Dock = DockStyle.Fill, FlowDirection = FlowDirection.TopDown,
            WrapContents = false, AutoScroll = true, BackColor = Surface,
            Padding = new Padding(2), Margin = Padding.Empty
        };
        flow.Controls.Add(Heading(title));
        flow.Controls.Add(Description(description));
        flow.ClientSizeChanged += (_, _) =>
        {
            var width = Math.Max(100, flow.ClientSize.Width - flow.Padding.Horizontal - 20);
            foreach (Control child in flow.Controls)
            {
                if (child is Label label)
                {
                    label.MaximumSize = new Size(width, 0);
                    label.AutoSize = true;
                }
                else child.Width = width;
            }
        };
        return flow;
    }

    static void StyleAction(Button button, bool primary = false)
    {
        button.AutoSize = false;
        button.Height = 40;
        button.Margin = new Padding(0, 0, 0, 10);
        button.BackColor = primary ? Accent : Surface;
    }

    // Keep the existing control instances and their event handlers inside each card.
    static RoundedCard Frame(Control content, Padding? padding = null)
    {
        var parent = content.Parent!;
        var margin = content.Margin;
        var index = parent.Controls.GetChildIndex(content);
        var cell = parent is TableLayoutPanel table ? table.GetPositionFromControl(content) : default;
        var columnSpan = parent is TableLayoutPanel t1 ? t1.GetColumnSpan(content) : 1;
        var rowSpan = parent is TableLayoutPanel t2 ? t2.GetRowSpan(content) : 1;
        var card = new RoundedCard { Dock = DockStyle.Fill, Margin = margin, Padding = padding ?? new Padding(18) };
        parent.Controls.Remove(content);
        if (parent is TableLayoutPanel grid)
        {
            grid.Controls.Add(card, cell.Column, cell.Row);
            grid.SetColumnSpan(card, columnSpan);
            grid.SetRowSpan(card, rowSpan);
        }
        else parent.Controls.Add(card);
        parent.Controls.SetChildIndex(card, index);
        content.Dock = DockStyle.Fill;
        content.Margin = Padding.Empty;
        content.Padding = Padding.Empty;
        content.BackColor = Surface;
        card.Controls.Add(content);
        return card;
    }

    void FinishPageCards(TabControl tabs)
    {
        foreach (TabPage page in tabs.TabPages.Cast<TabPage>().Skip(1)) Wrap(page);
        static void Wrap(Control parent)
        {
            foreach (var child in parent.Controls.Cast<Control>().ToArray())
            {
                if (child is RoundedCard) continue;
                if ((child is TableLayoutPanel || child is FlowLayoutPanel) &&
                    child.BackColor == Surface && parent.BackColor == Background)
                    Frame(child);
                else Wrap(child);
            }
        }
    }

    void ArrangeUpdates(TabControl tabs)
    {
        var page = tabs.TabPages.Cast<TabPage>().First(p => p.Text == "Обновления");
        var old = page.Controls.OfType<FlowLayoutPanel>().Single();
        var buttons = old.Controls.OfType<Button>().ToArray();
        var grid = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 2, RowCount = 1, BackColor = Background };
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 44));
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 56));
        grid.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        var actions = CardContent("Обновления Zapret", "Проверьте доступную версию и установите обновление с сохранением ваших списков.");
        actions.Margin = new Padding(0, 0, 16, 0);
        foreach (var button in buttons) { StyleAction(button, button == buttons.Last()); actions.Controls.Add(button); }
        buttons[0].Text = "Проверить версию";
        buttons[1].Text = "Установить обновление";
        actions.Controls.Add(Description("После обновления выбранная папка переключится на новую версию Zapret."));
        var report = new TableLayoutPanel { Dock = DockStyle.Fill, BackColor = Surface, RowCount = 3, ColumnCount = 1, Margin = Padding.Empty };
        report.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        report.RowStyles.Add(new RowStyle(SizeType.Absolute, 42));
        report.RowStyles.Add(new RowStyle(SizeType.Absolute, 48));
        report.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        report.Controls.Add(Heading("Версия и результат"), 0, 0);
        var hint = Description("Нажмите «Проверить версию», чтобы сравнить установленный и последний выпуск.");
        hint.Dock = DockStyle.Fill; hint.AutoSize = false;
        report.Controls.Add(hint, 0, 1);
        var output = new TextBox { Dock = DockStyle.Fill, Multiline = true, ReadOnly = true, WordWrap = true,
            ScrollBars = ScrollBars.Vertical, BorderStyle = BorderStyle.None, BackColor = Surface, ForeColor = Ink, Margin = Padding.Empty };
        report.Controls.Add(output, 0, 2);
        actions.Tag = output;
        grid.Controls.Add(actions, 0, 0); grid.Controls.Add(report, 1, 0);
        page.Controls.Add(grid); grid.BringToFront(); old.Dispose(); page.Padding = Padding.Empty;
    }
}
