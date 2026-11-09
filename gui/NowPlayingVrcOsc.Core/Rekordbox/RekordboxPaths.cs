using System.Text.Json;

namespace NowPlayingVrcOsc.Core.Rekordbox;

public static class RekordboxPaths
{
    /// <summary>
    /// master.db のパスを決める。
    /// 優先順: 明示指定 → rekordboxAgent の options.json の db-path → 既定の %APPDATA%\Pioneer\rekordbox\master.db
    /// </summary>
    public static string ResolveDbPath(string? overridePath)
    {
        if (!string.IsNullOrWhiteSpace(overridePath)) return overridePath;

        var appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
        var fromOptions = ReadDbPathFromOptions(Path.Combine(appData, "Pioneer", "rekordboxAgent", "storage", "options.json"));
        if (!string.IsNullOrWhiteSpace(fromOptions)) return fromOptions;

        return Path.Combine(appData, "Pioneer", "rekordbox", "master.db");
    }

    /// <summary>{"options":[["db-path","..."],...]} から db-path を取り出す。失敗時は null。</summary>
    internal static string? ReadDbPathFromOptions(string optionsJsonPath)
    {
        try
        {
            if (!File.Exists(optionsJsonPath)) return null;
            using var doc = JsonDocument.Parse(File.ReadAllText(optionsJsonPath));
            if (doc.RootElement.ValueKind != JsonValueKind.Object ||
                !doc.RootElement.TryGetProperty("options", out var options) ||
                options.ValueKind != JsonValueKind.Array)
                return null;

            foreach (var pair in options.EnumerateArray())
            {
                if (pair.ValueKind != JsonValueKind.Array || pair.GetArrayLength() < 2) continue;
                var k = pair[0];
                var v = pair[1];
                if (k.ValueKind == JsonValueKind.String && k.GetString() == "db-path" &&
                    v.ValueKind == JsonValueKind.String)
                {
                    var s = v.GetString();
                    if (!string.IsNullOrWhiteSpace(s)) return s;
                }
            }
        }
        catch
        {
            // 解析エラーは既定にフォールバック
        }
        return null;
    }
}
