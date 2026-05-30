using System.Text;
using System.Text.Json;

namespace NowPlayingVrcOsc.Core;

/// <summary>
/// NP2 config.json から Socket.IO 接続先 URL を解決するユーティリティ。
/// Python 版の np_url_from_config / resolve_np_url と同じロジック（parity）。
/// 機密保護のため currentTrack / localSocketPort 以外は参照しない。
/// </summary>
public static class NpConfig
{
    // Python 版 DEFAULT_NP_URL と一致（config に localSocketPort が無いときのフォールバック）
    public const string DefaultNpUrl = "http://localhost:9000";

    /// <summary>config.json の localSocketPort を読む（無ければ null）。</summary>
    public static int? ReadLocalSocketPort(string configPath)
    {
        try
        {
            if (!File.Exists(configPath)) return null;
            var json = File.ReadAllText(configPath, Encoding.UTF8);
            using var doc = JsonDocument.Parse(json);
            if (doc.RootElement.TryGetProperty("localSocketPort", out var el))
            {
                if (el.ValueKind == JsonValueKind.Number && el.TryGetInt32(out var p)) return p;
                if (el.ValueKind == JsonValueKind.String && int.TryParse(el.GetString(), out var ps)) return ps;
            }
        }
        catch { /* パース失敗・書き込み途中は無視 */ }
        return null;
    }

    /// <summary>
    /// config.json の currentTrack を読む（無い・読めない場合は null）。
    /// 機密保護のため currentTrack のみ参照する。
    /// </summary>
    public static TrackInfo? ReadCurrentTrack(string configPath)
    {
        try
        {
            if (!File.Exists(configPath)) return null;
            var json = File.ReadAllText(configPath, Encoding.UTF8);
            using var doc = JsonDocument.Parse(json);
            if (!doc.RootElement.TryGetProperty("currentTrack", out var el)) return null;
            if (el.ValueKind != JsonValueKind.Object) return null;
            var title = GetString(el, "title");
            if (string.IsNullOrEmpty(title)) return null;
            return new TrackInfo(
                Id: GetString(el, "id"),
                Title: title,
                Artist: GetString(el, "artist"),
                Label: GetString(el, "label")
            );
        }
        catch { return null; }
    }

    private static string? GetString(JsonElement el, string property)
    {
        if (el.TryGetProperty(property, out var val) && val.ValueKind == JsonValueKind.String)
            return val.GetString();
        return null;
    }

    /// <summary>
    /// Socket.IO 接続先 URL を解決する。優先順位: 明示指定 &gt; config の localSocketPort &gt; 既定。
    /// </summary>
    public static string ResolveNpUrl(string? explicitUrl, string configPath)
    {
        if (!string.IsNullOrWhiteSpace(explicitUrl)) return explicitUrl.Trim();
        var port = ReadLocalSocketPort(configPath);
        if (port is > 0) return $"http://localhost:{port}";
        return DefaultNpUrl;
    }
}
