namespace NowPlayingVrcOsc.Core;

/// <summary>Chatbox 送信文の組み立てオプション（Python NP_MSG_* と同等）。</summary>
public sealed class MessageFormatOptions
{
    public string Header { get; init; } = "🎵";
    public string Separator { get; init; } = "-";
    public bool IncludeTitle { get; init; } = true;
    public bool IncludeArtist { get; init; } = true;
    public bool IncludeAlbum { get; init; } = false;
    public bool IncludeLabel { get; init; } = false;
    public bool IncludeComment { get; init; } = false;
    public bool StripTitleBrackets { get; init; } = true;

    public string EffectiveSeparator =>
        string.IsNullOrWhiteSpace(Separator) ? "-" : Separator.Trim();

    /// <summary>項目間の join 用文字列（区切りの前後にスペースを付与）。</summary>
    public string SeparatorWithSpaces => $" {EffectiveSeparator} ";
}
