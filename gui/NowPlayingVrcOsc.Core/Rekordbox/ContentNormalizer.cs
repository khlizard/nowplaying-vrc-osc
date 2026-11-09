using System.Text.RegularExpressions;

namespace NowPlayingVrcOsc.Core.Rekordbox;

/// <summary>djmdContent の 1 行分の取得結果（名前は各テーブルの Name を解決済み）。</summary>
internal sealed record ContentRow(
    string? Title, string? FolderPath, string? Comment,
    string? Artist, string? Album, string? Label);

internal static partial class ContentNormalizer
{
    [GeneratedRegex(@"^\$A[0-9]+:v[0-9]+:")]
    private static partial Regex EncryptedRegex();

    [GeneratedRegex(@"^spotify:track:([A-Za-z0-9]{22})\z")]
    private static partial Regex SpotifyTrackRegex();

    /// <summary>rekordbox が暗号化して保存している値（Spotify 由来など）か。</summary>
    public static bool IsEncrypted(string? value) =>
        value != null && EncryptedRegex().IsMatch(value);

    /// <summary>null・空・暗号化値は null にする。</summary>
    public static string? Normalize(string? value) =>
        string.IsNullOrEmpty(value) || IsEncrypted(value) ? null : value;

    public static bool TryGetSpotifyTrackId(string? folderPath, out string trackId)
    {
        trackId = "";
        if (folderPath == null) return false;
        var m = SpotifyTrackRegex().Match(folderPath);
        if (!m.Success) return false;
        trackId = m.Groups[1].Value;
        return true;
    }

    public static bool IsSpotifyTrackId(string? id) =>
        id != null && TryGetSpotifyTrackId("spotify:track:" + id, out _);

    public static TrackInfo Build(string? contentId, ContentRow? c) => new(
        Id: contentId,
        Title: Normalize(c?.Title),
        Artist: Normalize(c?.Artist),
        Album: Normalize(c?.Album),
        Label: Normalize(c?.Label),
        Comment: Normalize(c?.Comment));
}
