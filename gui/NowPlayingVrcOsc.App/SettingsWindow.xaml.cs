using System.Globalization;
using System.IO;
using System.Windows;
using System.Windows.Controls;
using NowPlayingVrcOsc.Core;
using MessageBox = System.Windows.MessageBox;
using OpenFileDialog = Microsoft.Win32.OpenFileDialog;

namespace NowPlayingVrcOsc.App;

public partial class SettingsWindow : Window
{
    /// <summary>OK で保存された設定（DialogResult == true のとき有効）。</summary>
    public AppSettings Result { get; private set; }

    public SettingsWindow(AppSettings current)
    {
        InitializeComponent();
        Result = current;
        LoadFields(current);
    }

    private void LoadFields(AppSettings s)
    {
        SelectSource(s.Source);
        ConfigPathBox.Text = s.ConfigPath;
        PollBox.Text = s.PollIntervalSeconds.ToString(CultureInfo.InvariantCulture);
        HostBox.Text = s.VrcHost;
        PortBox.Text = s.VrcPort.ToString(CultureInfo.InvariantCulture);
        NotificationCheck.IsChecked = s.ChatboxNotification;
        IndicatorCheck.IsChecked = s.ChatboxIndicator;
        ShowLabelCheck.IsChecked = s.ShowLabel;
        StripBracketsCheck.IsChecked = s.StripTitleBrackets;
    }

    private void SelectSource(string source)
    {
        foreach (ComboBoxItem item in SourceCombo.Items)
        {
            if ((string)item.Content == source)
            {
                SourceCombo.SelectedItem = item;
                return;
            }
        }
        SourceCombo.SelectedIndex = 0; // auto
    }

    private void Browse_Click(object sender, RoutedEventArgs e)
    {
        var dlg = new OpenFileDialog
        {
            Title = "NP2 config.json を選択",
            Filter = "config.json|config.json|JSON ファイル|*.json|すべてのファイル|*.*",
            CheckFileExists = false,
        };
        if (!string.IsNullOrWhiteSpace(ConfigPathBox.Text))
        {
            try { dlg.InitialDirectory = Path.GetDirectoryName(ConfigPathBox.Text); }
            catch { /* 無効なパスは無視 */ }
        }
        if (dlg.ShowDialog(this) == true)
            ConfigPathBox.Text = dlg.FileName;
    }

    private void Ok_Click(object sender, RoutedEventArgs e)
    {
        if (!double.TryParse(PollBox.Text, NumberStyles.Float, CultureInfo.InvariantCulture, out var poll) || poll <= 0)
        {
            Invalid("ポーリング間隔には正の数値を入力してください。", PollBox);
            return;
        }
        if (!int.TryParse(PortBox.Text, NumberStyles.Integer, CultureInfo.InvariantCulture, out var port) || port is < 1 or > 65535)
        {
            Invalid("VRChat ポートには 1〜65535 の値を入力してください。", PortBox);
            return;
        }
        if (string.IsNullOrWhiteSpace(HostBox.Text))
        {
            Invalid("VRChat ホストを入力してください。", HostBox);
            return;
        }

        var settings = new AppSettings
        {
            Source = (SourceCombo.SelectedItem as ComboBoxItem)?.Content as string ?? "auto",
            ConfigPath = ConfigPathBox.Text.Trim(),
            PollIntervalSeconds = poll,
            VrcHost = HostBox.Text.Trim(),
            VrcPort = port,
            ChatboxNotification = NotificationCheck.IsChecked == true,
            ChatboxIndicator = IndicatorCheck.IsChecked == true,
            ShowLabel = ShowLabelCheck.IsChecked == true,
            StripTitleBrackets = StripBracketsCheck.IsChecked == true,
        };
        settings.Save();

        Result = settings;
        DialogResult = true;
    }

    private void Invalid(string message, System.Windows.Controls.Control focus)
    {
        MessageBox.Show(this, message, "設定", MessageBoxButton.OK, MessageBoxImage.Warning);
        focus.Focus();
    }
}
