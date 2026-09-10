using System.Globalization;
using System.IO.Ports;

namespace OssmOwner090;

public sealed class MainForm : Form
{
    private readonly ComboBox ports = new()
    {
        DropDownStyle = ComboBoxStyle.DropDownList,
        Width = 100
    };

    private readonly Button refresh = new() { Text = "Refresh" };
    private readonly Button connect = new() { Text = "Connect", Width = 95 };
    private readonly Label connection = new() { Text = "Disconnected", AutoSize = true };
    private readonly Label mode = new()
    {
        Text = "Mode: STOCK",
        AutoSize = true,
        Font = new Font(SystemFonts.DefaultFont, FontStyle.Bold)
    };

    private readonly Button estopButton = new()
    {
        Text = "E-STOP",
        Width = 120,
        Height = 42,
        BackColor = Color.Red,
        ForeColor = Color.White,
        Font = new Font(SystemFonts.DefaultFont.FontFamily, 11, FontStyle.Bold),
        FlatStyle = FlatStyle.Flat
    };

    private readonly Button resetEstopButton = new()
    {
        Text = "Reset E-stop",
        Width = 110,
        Height = 42
    };

    private readonly TrackBar maxSpeed = Slider(0, 600, 150, 50);
    private readonly TrackBar minStroke = Slider(0, 180, 0, 20);
    private readonly TrackBar maxStroke = Slider(0, 180, 100, 20);
    private readonly TrackBar minDepth = Slider(0, 180, 0, 20);
    private readonly TrackBar maxDepth = Slider(0, 180, 160, 20);

    private readonly NumericUpDown maxSpeedSliderMax = RangeBox(1, 2000, 600);
    private readonly NumericUpDown maxStrokeSliderMax = RangeBox(1, 1000, 180);
    private readonly NumericUpDown maxDepthSliderMax = RangeBox(1, 1000, 180);

    private readonly Label maxSpeedValue = ValueLabel();
    private readonly Label minStrokeValue = ValueLabel();
    private readonly Label maxStrokeValue = ValueLabel();
    private readonly Label minDepthValue = ValueLabel();
    private readonly Label maxDepthValue = ValueLabel();

    private readonly TextBox log = new()
    {
        Multiline = true,
        ReadOnly = true,
        ScrollBars = ScrollBars.Vertical,
        Dock = DockStyle.Fill
    };

    private readonly System.Windows.Forms.Timer ioTimer = new() { Interval = 250 };
    private SerialPort? serial;
    private bool ownerModeActive;
    private bool suppressLiveApply;

    private static TrackBar Slider(int min, int max, int value, int tickFrequency) => new()
    {
        Minimum = min,
        Maximum = max,
        Value = value,
        TickFrequency = tickFrequency,
        SmallChange = 1,
        LargeChange = Math.Max(1, tickFrequency),
        AutoSize = false,
        Height = 42,
        Width = 410
    };

    private static NumericUpDown RangeBox(decimal min, decimal max, decimal value) => new()
    {
        Minimum = min,
        Maximum = max,
        Value = value,
        DecimalPlaces = 0,
        Width = 72,
        TextAlign = HorizontalAlignment.Right
    };

    private static Label ValueLabel() => new()
    {
        AutoSize = false,
        Width = 70,
        Height = 28,
        TextAlign = ContentAlignment.MiddleRight,
        Font = new Font(SystemFonts.DefaultFont.FontFamily, 10, FontStyle.Bold)
    };

    public MainForm()
    {
        Text = "OSSM 0.9 Owner Limits";
        Width = 930;
        Height = 660;
        MinimumSize = new Size(820, 560);
        StartPosition = FormStartPosition.CenterScreen;

        BuildUi();
        RefreshPorts();
        ApplySliderMaximums();
        UpdateSliderLabels();

        refresh.Click += (_, _) => RefreshPorts();
        connect.Click += (_, _) =>
        {
            if (serial?.IsOpen == true) Disconnect();
            else Connect();
        };

        maxSpeedSliderMax.ValueChanged += (_, _) => ApplySliderMaximums();
        maxStrokeSliderMax.ValueChanged += (_, _) => ApplySliderMaximums();
        maxDepthSliderMax.ValueChanged += (_, _) => ApplySliderMaximums();

        estopButton.Click += (_, _) =>
        {
            ownerModeActive = false;
            Send("@OWNER:ESTOP");
            mode.Text = "Mode: E-STOP LATCHED";
        };

        resetEstopButton.Click += (_, _) =>
        {
            ownerModeActive = false;
            Send("@OWNER:ESTOP:RESET");
            mode.Text = "Mode: E-stop reset - select mode";
        };

        maxSpeed.ValueChanged += (_, _) =>
        {
            UpdateSliderLabels();
            LiveApplyIfOwner();
        };

        minStroke.ValueChanged += (_, _) =>
        {
            suppressLiveApply = true;
            if (minStroke.Value > maxStroke.Value)
                maxStroke.Value = minStroke.Value;
            suppressLiveApply = false;
            UpdateSliderLabels();
            LiveApplyIfOwner();
        };

        maxStroke.ValueChanged += (_, _) =>
        {
            suppressLiveApply = true;
            if (maxStroke.Value < minStroke.Value)
                minStroke.Value = maxStroke.Value;
            suppressLiveApply = false;
            UpdateSliderLabels();
            LiveApplyIfOwner();
        };

        minDepth.ValueChanged += (_, _) =>
        {
            suppressLiveApply = true;
            if (minDepth.Value > maxDepth.Value)
                maxDepth.Value = minDepth.Value;
            suppressLiveApply = false;
            UpdateSliderLabels();
            LiveApplyIfOwner();
        };

        maxDepth.ValueChanged += (_, _) =>
        {
            suppressLiveApply = true;
            if (maxDepth.Value < minDepth.Value)
                minDepth.Value = maxDepth.Value;
            suppressLiveApply = false;
            UpdateSliderLabels();
            LiveApplyIfOwner();
        };

        ioTimer.Tick += (_, _) => IoTick();
        FormClosing += (_, _) => Disconnect();
    }

    private void BuildUi()
    {
        var root = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            Padding = new Padding(12),
            RowCount = 4,
            ColumnCount = 1
        };

        root.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        root.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        root.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        root.RowStyles.Add(new RowStyle(SizeType.Percent, 100));

        var conn = new FlowLayoutPanel
        {
            Dock = DockStyle.Top,
            AutoSize = true,
            Padding = new Padding(0, 0, 0, 6)
        };

        conn.Controls.AddRange([
            new Label
            {
                Text = "COM port:",
                AutoSize = true,
                Padding = new Padding(0, 7, 0, 0)
            },
            ports,
            refresh,
            connect,
            connection
        ]);

        var controls = new FlowLayoutPanel
        {
            Dock = DockStyle.Top,
            AutoSize = true,
            Padding = new Padding(0, 0, 0, 8)
        };

        var masterOn = new Button { Text = "Master ON (fallback)", Width = 160 };
        var masterOff = new Button { Text = "Master OFF (stock)", Width = 150 };
        var ownerOn = new Button { Text = "Apply + owner ON", Width = 150 };
        var fallback = new Button { Text = "Fallback", Width = 100 };

        controls.Controls.AddRange([mode, masterOn, masterOff, ownerOn, fallback, estopButton, resetEstopButton]);

        masterOn.Click += (_, _) =>
        {
            ownerModeActive = false;
            Send("@OWNER:MASTER:ENABLE");
            mode.Text = "Mode: FALLBACK";
        };

        masterOff.Click += (_, _) =>
        {
            ownerModeActive = false;
            Send("@OWNER:MASTER:DISABLE");
            mode.Text = "Mode: STOCK";
        };

        ownerOn.Click += (_, _) =>
        {
            // Always re-enable MASTER first. This makes OWNER work even if
            // the previous state was STOCK (MASTER OFF).
            Send("@OWNER:MASTER:ENABLE");
            Send("@OWNER:HB");
            SendLimits();
            Send("@OWNER:ENABLE");
            ownerModeActive = true;
            mode.Text = "Mode: OWNER";
        };

        fallback.Click += (_, _) =>
        {
            ownerModeActive = false;
            Send("@OWNER:MASTER:ENABLE");
            Send("@OWNER:DISABLE");
            mode.Text = "Mode: FALLBACK";
        };

        var limitsBox = new GroupBox
        {
            Text = "Owner limits",
            Dock = DockStyle.Top,
            AutoSize = true,
            Padding = new Padding(10)
        };

        var grid = new TableLayoutPanel
        {
            Dock = DockStyle.Top,
            AutoSize = true,
            ColumnCount = 5,
            RowCount = 6
        };

        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 145));
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 70));
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 55));
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 105));

        AddSliderRow(grid, 0, "Maximum speed", maxSpeed, maxSpeedValue, "mm/s", maxSpeedSliderMax);
        AddSliderRow(grid, 1, "Minimum stroke", minStroke, minStrokeValue, "mm", null);
        AddSliderRow(grid, 2, "Maximum stroke", maxStroke, maxStrokeValue, "mm", maxStrokeSliderMax);
        AddSliderRow(grid, 3, "Minimum depth", minDepth, minDepthValue, "mm", null);
        AddSliderRow(grid, 4, "Maximum depth", maxDepth, maxDepthValue, "mm", maxDepthSliderMax);

        var apply = new Button
        {
            Text = "Apply limits",
            Width = 140,
            Height = 32,
            Margin = new Padding(3, 8, 3, 3)
        };
        apply.Click += (_, _) =>
        {
            Send("@OWNER:MASTER:ENABLE");
            SendLimits();
            Send("@OWNER:ENABLE");
            ownerModeActive = true;
            mode.Text = "Mode: OWNER";
        };

        grid.Controls.Add(apply, 1, 5);
        limitsBox.Controls.Add(grid);

        var logBox = new GroupBox
        {
            Text = "Firmware serial output",
            Dock = DockStyle.Fill
        };
        logBox.Controls.Add(log);

        root.Controls.Add(conn, 0, 0);
        root.Controls.Add(controls, 0, 1);
        root.Controls.Add(limitsBox, 0, 2);
        root.Controls.Add(logBox, 0, 3);

        Controls.Add(root);
    }

    private static void AddSliderRow(
        TableLayoutPanel grid,
        int row,
        string label,
        TrackBar slider,
        Label value,
        string unit,
        NumericUpDown? sliderMax)
    {
        grid.Controls.Add(
            new Label
            {
                Text = label,
                AutoSize = true,
                Padding = new Padding(0, 11, 8, 0)
            },
            0,
            row);

        grid.Controls.Add(slider, 1, row);
        grid.Controls.Add(value, 2, row);

        grid.Controls.Add(
            new Label
            {
                Text = unit,
                AutoSize = true,
                Padding = new Padding(4, 9, 0, 0)
            },
            3,
            row);

        if (sliderMax != null)
        {
            var rangePanel = new FlowLayoutPanel
            {
                AutoSize = true,
                FlowDirection = FlowDirection.LeftToRight,
                WrapContents = false,
                Margin = new Padding(0, 4, 0, 0)
            };
            rangePanel.Controls.Add(new Label
            {
                Text = "Max:",
                AutoSize = true,
                Padding = new Padding(0, 6, 2, 0)
            });
            rangePanel.Controls.Add(sliderMax);
            grid.Controls.Add(rangePanel, 4, row);
        }
    }

    private void ApplySliderMaximums()
    {
        suppressLiveApply = true;
        try
        {
            var speedMax = (int)maxSpeedSliderMax.Value;
            var strokeMax = (int)maxStrokeSliderMax.Value;
            var depthMax = (int)maxDepthSliderMax.Value;

            maxSpeed.Maximum = speedMax;
            if (maxSpeed.Value > speedMax)
                maxSpeed.Value = speedMax;

            minStroke.Maximum = strokeMax;
            maxStroke.Maximum = strokeMax;
            if (minStroke.Value > strokeMax)
                minStroke.Value = strokeMax;
            if (maxStroke.Value > strokeMax)
                maxStroke.Value = strokeMax;

            minDepth.Maximum = depthMax;
            maxDepth.Maximum = depthMax;
            if (minDepth.Value > depthMax)
                minDepth.Value = depthMax;
            if (maxDepth.Value > depthMax)
                maxDepth.Value = depthMax;
        }
        finally
        {
            suppressLiveApply = false;
        }

        UpdateSliderLabels();
        LiveApplyIfOwner();
    }

    private void UpdateSliderLabels()
    {
        maxSpeedValue.Text = maxSpeed.Value.ToString(CultureInfo.InvariantCulture);
        minStrokeValue.Text = minStroke.Value.ToString(CultureInfo.InvariantCulture);
        maxStrokeValue.Text = maxStroke.Value.ToString(CultureInfo.InvariantCulture);
        minDepthValue.Text = minDepth.Value.ToString(CultureInfo.InvariantCulture);
        maxDepthValue.Text = maxDepth.Value.ToString(CultureInfo.InvariantCulture);
    }

    private void RefreshPorts()
    {
        var selected = ports.SelectedItem?.ToString();
        var names = SerialPort.GetPortNames()
            .OrderBy(x => x, StringComparer.OrdinalIgnoreCase)
            .ToArray();

        ports.Items.Clear();
        ports.Items.AddRange(names);

        if (selected != null && names.Contains(selected))
            ports.SelectedItem = selected;
        else if (names.Contains("COM8"))
            ports.SelectedItem = "COM8";
        else if (ports.Items.Count > 0)
            ports.SelectedIndex = 0;
    }

    private void Connect()
    {
        var name = ports.SelectedItem?.ToString();
        if (string.IsNullOrWhiteSpace(name))
            return;

        try
        {
            serial = new SerialPort(name, 115200)
            {
                NewLine = "\n",
                DtrEnable = false,
                RtsEnable = false,
                WriteTimeout = 250
            };

            serial.Open();

            connection.Text = $"Connected: {name}";
            connect.Text = "Disconnect";
            mode.Text = "Mode: STOCK";

            ioTimer.Start();
            Send("@OWNER:HB");
        }
        catch (Exception ex)
        {
            MessageBox.Show(
                this,
                ex.Message,
                "Serial error",
                MessageBoxButtons.OK,
                MessageBoxIcon.Error);

            Disconnect();
        }
    }

    private void Disconnect()
    {
        ioTimer.Stop();

        try { serial?.Close(); } catch { }
        try { serial?.Dispose(); } catch { }

        serial = null;
        ownerModeActive = false;
        connection.Text = "Disconnected";
        connect.Text = "Connect";
        mode.Text = "Mode: STOCK / unknown";
    }

    private void IoTick()
    {
        if (serial?.IsOpen != true)
            return;

        try
        {
            var incoming = serial.ReadExisting();

            if (!string.IsNullOrEmpty(incoming))
            {
                log.AppendText(incoming);

                if (log.TextLength > 20000)
                    log.Text = log.Text[^10000..];

                log.SelectionStart = log.TextLength;
                log.ScrollToCaret();
            }

            Send("@OWNER:HB");
        }
        catch (Exception ex)
        {
            log.AppendText($"\r\nSerial error: {ex.Message}\r\n");
            Disconnect();
        }
    }

    private void LiveApplyIfOwner()
    {
        if (suppressLiveApply || !ownerModeActive || serial?.IsOpen != true)
            return;

        SendLimits();
    }

    private void SendLimits()
    {
        var command = string.Create(
            CultureInfo.InvariantCulture,
            $"@OWNER:LIMITS:{maxSpeed.Value}:{minStroke.Value}:{maxStroke.Value}:{minDepth.Value}:{maxDepth.Value}");

        Send(command);
    }

    private void Send(string line)
    {
        if (serial?.IsOpen != true)
            return;

        serial.Write(line + "\n");
    }
}
