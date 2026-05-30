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

    /// <summary>選択フィールドを compact join し、ヘッダを付与する。</summary>
    public static string FormatChatboxMessage(TrackInfo track, MessageFormatOptions opts)
    {
        var parts = new List<string>();
        if (opts.IncludeTitle)
        {
            var title = NormalizeField(track.Title);
            if (opts.StripTitleBrackets && title.Length > 0)
                title = RemoveBrackets(title);
            if (title.Length > 0) parts.Add(title);
        }
        if (opts.IncludeArtist)
        {
            var artist = NormalizeField(track.Artist);
            if (artist.Length > 0) parts.Add(artist);
        }
        if (opts.IncludeAlbum)
        {
            var album = NormalizeField(track.Album);
            if (album.Length > 0) parts.Add(album);
        }
        if (opts.IncludeLabel)
        {
            var label = NormalizeField(track.Label);
            if (label.Length > 0) parts.Add(label);
        }
        if (opts.IncludeComment)
        {
            var comment = NormalizeField(track.Comment);
            if (comment.Length > 0) parts.Add(comment);
        }

        var body = string.Join(opts.SeparatorWithSpaces, parts);
        var header = opts.Header?.Trim() ?? "";
        if (header.Length == 0) return body;
        if (body.Length == 0) return header;
        return $"{header} {body}";
    }

    /// <summary>メイン画面プレビュー用（ヘッダなしの本文のみ）。</summary>
    public static string FormatTrackBody(TrackInfo track, MessageFormatOptions opts)
    {
        var withHeader = FormatChatboxMessage(track, opts);
        var header = opts.Header?.Trim() ?? "";
        if (header.Length == 0) return withHeader;
        if (withHeader.StartsWith(header + " ", StringComparison.Ordinal))
            return withHeader[(header.Length + 1)..];
        if (withHeader == header) return "";
        return withHeader;
    }
}
