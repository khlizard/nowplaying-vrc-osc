using System.Text.RegularExpressions;

namespace NowPlayingVrcOsc.Core;

public static class TrackFormatter
{
    private static readonly Regex TitleBracketsRegex =
        new(@"\s*\[[^\]]*\]", RegexOptions.Compiled);

    private static readonly HashSet<string> UnknownValues =
        new(StringComparer.OrdinalIgnoreCase) { "none", "unknown", "n/a", "?" };

    public static string NormalizeField(string? value)
    {
        if (value is null) return "";
        var text = value.Trim();
        if (text.Length == 0 || UnknownValues.Contains(text)) return "";
        return text;
    }

    public static string RemoveBrackets(string text) =>
        TitleBracketsRegex.Replace(text, "").Trim();

    public static string FormatTrack(TrackInfo track, bool showLabel = false, bool stripTitleBrackets = true)
    {
        var title = NormalizeField(track.Title);
        if (stripTitleBrackets && title.Length > 0)
            title = RemoveBrackets(title);

        var artist = NormalizeField(track.Artist);
        var label = NormalizeField(track.Label);

        string main;
        if (title.Length > 0 && artist.Length > 0)
            main = $"{title} - {artist}";
        else if (title.Length > 0)
            main = title;
        else if (artist.Length > 0)
            main = artist;
        else
            main = "";

        if (label.Length > 0 && showLabel)
            main = $"{main} [{label}]";

        return main.Trim();
    }

    // Python parity: f"🎵 {format_track(...)}"
    public static string FormatChatboxMessage(TrackInfo track, bool showLabel = false, bool stripTitleBrackets = true) =>
        $"🎵 {FormatTrack(track, showLabel, stripTitleBrackets)}";
}
