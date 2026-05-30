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
        WindowIcon.TryApply(this);
        Result = current;
        LoadFields(current);
        UpdatePreview();
    }

    private void LoadFields(AppSettings s)
    {
        SelectSource(s.Source);
        ConfigPathBox.Text = s.ConfigPath;
        PollBox.Text = s.PollIntervalSeconds.ToString(CultureInfo.InvariantCulture);
        NpUrlBox.Text = s.NpUrl;
        HostBox.Text = s.VrcHost;
        PortBox.Text = s.VrcPort.ToString(CultureInfo.InvariantCulture);
        NotificationCheck.IsChecked = s.ChatboxNotification;
        IndicatorCheck.IsChecked = s.ChatboxIndicator;
        MsgHeaderBox.Text = s.MsgHeader;
        MsgSeparatorBox.Text = s.MsgSeparator;
        IncludeTitleCheck.IsChecked = s.MsgIncludeTitle;
        IncludeArtistCheck.IsChecked = s.MsgIncludeArtist;
        IncludeAlbumCheck.IsChecked = s.MsgIncludeAlbum;
        IncludeLabelCheck.IsChecked = s.MsgIncludeLabel;
        IncludeCommentCheck.IsChecked = s.MsgIncludeComment;
        StripBracketsCheck.IsChecked = s.StripTitleBrackets;
        UpdateNpUrlHint();
    }

    private AppSettings BuildSettingsFromFields()
    {
        var poll = double.TryParse(PollBox.Text, NumberStyles.Float, CultureInfo.InvariantCulture, out var p) && p > 0
            ? p : 0.5;
        var port = int.TryParse(PortBox.Text, NumberStyles.Integer, CultureInfo.InvariantCulture, out var pt) && pt is >= 1 and <= 65535
            ? pt : 9000;
        return new AppSettings
        {
            Source = (SourceCombo.SelectedItem as ComboBoxItem)?.Content as string ?? "auto",
            ConfigPath = ConfigPathBox.Text.Trim(),
            PollIntervalSeconds = poll,
            NpUrl = NpUrlBox.Text.Trim(),
            VrcHost = HostBox.Text.Trim(),
            VrcPort = port,
            ChatboxNotification = NotificationCheck.IsChecked == true,
            ChatboxIndicator = IndicatorCheck.IsChecked == true,
            MsgHeader = MsgHeaderBox.Text,
            MsgSeparator = MsgSeparatorBox.Text.Trim(),
            MsgIncludeTitle = IncludeTitleCheck.IsChecked == true,
            MsgIncludeArtist = IncludeArtistCheck.IsChecked == true,
            MsgIncludeAlbum = IncludeAlbumCheck.IsChecked == true,
            MsgIncludeLabel = IncludeLabelCheck.IsChecked == true,
            MsgIncludeComment = IncludeCommentCheck.IsChecked == true,
            StripTitleBrackets = StripBracketsCheck.IsChecked == true,
        };
    }

    private void UpdatePreview()
    {
        if (PreviewText is null) return;
        try
        {
            var settings = BuildSettingsFromFields();
            var track = NpConfig.ReadCurrentTrack(settings.ConfigPath);
            if (track == null)
            {
                PreviewText.Text = "（config.json の currentTrack が読めません）";
                return;
            }
            PreviewText.Text = TrackFormatter.FormatChatboxMessage(track, settings.ToMessageFormat());
        }
        catch
        {
            PreviewText.Text = "（プレビュー不可）";
        }
    }

    private void UpdateNpUrlHint()
    {
        if (NpUrlHint is null) return;
        var auto = NpConfig.ResolveNpUrl(null, ConfigPathBox.Text?.Trim() ?? "");
        NpUrlHint.Text = $"空欄 = config.json の localSocketPort から自動検出（現在: {auto}）";
    }

    private void ConfigPathBox_TextChanged(object sender, TextChangedEventArgs e)
    {
        UpdateNpUrlHint();
        UpdatePreview();
    }

    private void MessageFields_Changed(object sender, RoutedEventArgs e) => UpdatePreview();

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
        SourceCombo.SelectedIndex = 0;
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
        if (string.IsNullOrWhiteSpace(MsgSeparatorBox.Text))
        {
            Invalid("区切り文字を入力してください。", MsgSeparatorBox);
            return;
        }
        if (IncludeTitleCheck.IsChecked != true &&
            IncludeArtistCheck.IsChecked != true &&
            IncludeAlbumCheck.IsChecked != true &&
            IncludeLabelCheck.IsChecked != true &&
            IncludeCommentCheck.IsChecked != true)
        {
            Invalid("表示する項目を 1 つ以上選んでください。", IncludeTitleCheck);
            return;
        }

        var settings = BuildSettingsFromFields();
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
