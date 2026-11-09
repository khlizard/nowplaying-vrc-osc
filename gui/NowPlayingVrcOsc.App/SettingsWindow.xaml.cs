using System.Globalization;
using System.IO;
using System.Windows;
using System.Windows.Controls;
using NowPlayingVrcOsc.Core;
using NowPlayingVrcOsc.Core.Rekordbox;
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
        DbPathBox.Text = s.DbPathOverride;
        PollBox.Text = s.PollIntervalSeconds.ToString(CultureInfo.InvariantCulture);
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
        UpdateDbPathHint();
        UpdateKeyStatus();
    }

    private AppSettings BuildSettingsFromFields()
    {
        var poll = double.TryParse(PollBox.Text, NumberStyles.Float, CultureInfo.InvariantCulture, out var p) && p > 0
            ? p : 0.5;
        var port = int.TryParse(PortBox.Text, NumberStyles.Integer, CultureInfo.InvariantCulture, out var pt) && pt is >= 1 and <= 65535
            ? pt : 9000;
        return new AppSettings
        {
            DbPathOverride = DbPathBox.Text.Trim(),
            PollIntervalSeconds = poll,
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
            var track = new TrackInfo("sample", "Sample Title [Extended Mix]", "Sample Artist",
                "Sample Album", "Sample Label", "Sample Comment");
            PreviewText.Text = TrackFormatter.FormatChatboxMessage(track, settings.ToMessageFormat());
        }
        catch
        {
            PreviewText.Text = "（プレビュー不可）";
        }
    }

    private void UpdateDbPathHint()
    {
        if (DbPathHint is null) return;
        var auto = RekordboxPaths.ResolveDbPath(DbPathBox.Text?.Trim());
        DbPathHint.Text = $"空欄なら自動検出: {auto}";
    }

    private void UpdateKeyStatus()
    {
        if (KeyStatusText is null) return;
        var saved = false;
        try { saved = new DpapiKeyStore(DpapiKeyStore.DefaultPath()).Load() != null; }
        catch { /* 読み取り失敗は未保存扱い */ }
        KeyStatusText.Text = saved ? "保存済みのキーがあります" : "未保存（通常は自動取得されます）";
    }

    private void DbPathBox_TextChanged(object sender, TextChangedEventArgs e)
    {
        UpdateDbPathHint();
    }

    private void MessageFields_Changed(object sender, RoutedEventArgs e) => UpdatePreview();

    private void Browse_Click(object sender, RoutedEventArgs e)
    {
        var dlg = new OpenFileDialog
        {
            Title = "rekordbox の master.db を選択",
            Filter = "master.db|master.db|SQLite DB (*.db)|*.db|すべて (*.*)|*.*",
            CheckFileExists = false,
        };
        if (!string.IsNullOrWhiteSpace(DbPathBox.Text))
        {
            try { dlg.InitialDirectory = Path.GetDirectoryName(DbPathBox.Text); }
            catch { /* 無効なパスは無視 */ }
        }
        if (dlg.ShowDialog(this) == true)
            DbPathBox.Text = dlg.FileName;
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

        var key = KeyBox.Password.Trim();
        if (key.Length > 0)
        {
            var dbPath = RekordboxPaths.ResolveDbPath(DbPathBox.Text.Trim());
            var check = RekordboxKeyVerifier.Verify(dbPath, key);
            switch (check)
            {
                case KeyCheckResult.Ok:
                    break;
                case KeyCheckResult.InvalidFormat:
                    Invalid("復号キーは 64 桁の 16 進数で入力してください。", KeyBox);
                    return;
                case KeyCheckResult.WrongKey:
                    Invalid("このキーでは master.db を開けませんでした。", KeyBox);
                    return;
                default:
                    var reason = check switch
                    {
                        KeyCheckResult.DbNotFound => "master.db が見つかりません",
                        KeyCheckResult.Busy => "rekordbox がデータベースを使用中です",
                        _ => "読み取りエラー",
                    };
                    var answer = MessageBox.Show(this,
                        $"master.db でキーを確認できませんでした（{reason}）。このまま保存しますか？",
                        "設定", MessageBoxButton.YesNo, MessageBoxImage.Question);
                    if (answer != MessageBoxResult.Yes) return;
                    break;
            }
            try
            {
                new DpapiKeyStore(DpapiKeyStore.DefaultPath()).Save(key);
            }
            catch (Exception ex)
            {
                MessageBox.Show(this, $"復号キーを保存できませんでした: {ex.Message}", "設定",
                    MessageBoxButton.OK, MessageBoxImage.Warning);
                return;
            }
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
