using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace NowPlayingVrcOsc.Core;

public class AppSettings
{
    public string Source { get; set; } = "auto";
    public string ConfigPath { get; set; } = DefaultConfigPath();
    public double PollIntervalSeconds { get; set; } = 0.5;

    // Socket.IO 接続先 URL。空のときは config.json の localSocketPort から自動検出。
    public string NpUrl { get; set; } = "";
    public string VrcHost { get; set; } = "127.0.0.1";
    public int VrcPort { get; set; } = 9000;
    public bool ChatboxNotification { get; set; } = false;
    public bool ChatboxIndicator { get; set; } = false;

    // Chatbox メッセージ
    public string MsgHeader { get; set; } = "🎵";
    public string MsgSeparator { get; set; } = "-";
    public bool MsgIncludeTitle { get; set; } = true;
    public bool MsgIncludeArtist { get; set; } = true;
    public bool MsgIncludeAlbum { get; set; } = false;
    public bool MsgIncludeLabel { get; set; } = false;
    public bool MsgIncludeComment { get; set; } = false;
    public bool StripTitleBrackets { get; set; } = true;

    /// <summary>旧設定。読み込み時に MsgIncludeLabel へマイグレーションする。</summary>
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingDefault)]
    public bool ShowLabel { get; set; } = false;

    public MessageFormatOptions ToMessageFormat() => new()
    {
        Header = MsgHeader,
        Separator = MsgSeparator,
        IncludeTitle = MsgIncludeTitle,
        IncludeArtist = MsgIncludeArtist,
        IncludeAlbum = MsgIncludeAlbum,
        IncludeLabel = MsgIncludeLabel,
        IncludeComment = MsgIncludeComment,
        StripTitleBrackets = StripTitleBrackets,
    };

    public static string DefaultConfigPath() =>
        Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
            ".nowplaying", "config.json");

    public static string SettingsPath() =>
        Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "NowPlayingVrcOsc", "settings.json");

    public static AppSettings Load()
    {
        var path = SettingsPath();
        if (!File.Exists(path)) return new AppSettings();
        try
        {
            var json = File.ReadAllText(path, Encoding.UTF8);
            var settings = JsonSerializer.Deserialize<AppSettings>(json) ?? new AppSettings();
            MigrateShowLabel(settings, json);
            return settings;
        }
        catch { return new AppSettings(); }
    }

    /// <summary>旧 ShowLabel=true を MsgIncludeLabel へ移行（新キーが JSON に無い場合のみ）。</summary>
    private static void MigrateShowLabel(AppSettings settings, string json)
    {
        if (!settings.ShowLabel) return;
        if (json.Contains("MsgIncludeLabel", StringComparison.Ordinal)) return;
        settings.MsgIncludeLabel = true;
    }

    public void Save()
    {
        var path = SettingsPath();
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path,
            JsonSerializer.Serialize(this, new JsonSerializerOptions { WriteIndented = true }),
            Encoding.UTF8);
    }
}
