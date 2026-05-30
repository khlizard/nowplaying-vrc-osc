using System.Text;
using System.Text.Json;

namespace NowPlayingVrcOsc.Core;

public class AppSettings
{
    public string Source { get; set; } = "auto";
    public string ConfigPath { get; set; } = DefaultConfigPath();
    public double PollIntervalSeconds { get; set; } = 0.5;
    public string VrcHost { get; set; } = "127.0.0.1";
    public int VrcPort { get; set; } = 9000;
    public bool ChatboxNotification { get; set; } = false;
    public bool ChatboxIndicator { get; set; } = false;
    public bool ShowLabel { get; set; } = false;
    public bool StripTitleBrackets { get; set; } = true;

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
            return JsonSerializer.Deserialize<AppSettings>(json) ?? new AppSettings();
        }
        catch { return new AppSettings(); }
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
