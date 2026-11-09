using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace NowPlayingVrcOsc.Core.Rekordbox;

/// <summary>
/// 固定 URL のテキストから 64 桁 hex の候補鍵を抽出する。
/// https のみ・5 秒タイムアウト・1 MiB 上限・最終 URL も https 必須。
/// </summary>
public sealed partial class UrlKeySource : IDisposable
{
    public const int MaxCandidates = 20;
    private static readonly TimeSpan RequestTimeout = TimeSpan.FromSeconds(5);

    [GeneratedRegex(@"(?<![0-9a-fA-F])[0-9a-fA-F]{64}(?![0-9a-fA-F])")]
    private static partial Regex HexRunRegex();

    private readonly HttpClient _http;
    private readonly IReadOnlyList<string> _urls;

    public UrlKeySource() : this(null, null) { }

    public UrlKeySource(HttpMessageHandler? handler, IReadOnlyList<string>? urls)
    {
        _http = handler != null ? new HttpClient(handler, disposeHandler: false) : new HttpClient();
        _http.Timeout = Timeout.InfiniteTimeSpan; // CancellationToken 側で制御する
        _urls = urls ?? LoadDefaultUrls();
    }

    internal static IReadOnlyList<string> LoadDefaultUrls()
    {
        using var s = typeof(UrlKeySource).Assembly.GetManifestResourceStream("default-key-sources.json")
                      ?? throw new InvalidOperationException("default-key-sources.json が見つかりません。");
        return JsonSerializer.Deserialize<List<string>>(s) ?? [];
    }

    /// <summary>全 URL から候補鍵を集める（重複除去・最大 20 件）。個々の失敗は無視する。</summary>
    public async Task<IReadOnlyList<string>> FetchCandidatesAsync(CancellationToken ct)
    {
        var result = new List<string>();
        var seen = new HashSet<string>(StringComparer.Ordinal);
        foreach (var url in _urls)
        {
            ct.ThrowIfCancellationRequested();
            var text = await FetchTextAsync(url, ct).ConfigureAwait(false);
            if (text == null) continue;
            foreach (Match m in HexRunRegex().Matches(text))
            {
                if (seen.Add(m.Value)) result.Add(m.Value);
                if (result.Count >= MaxCandidates) return result;
            }
        }
        return result;
    }

    private async Task<string?> FetchTextAsync(string url, CancellationToken ct)
    {
        if (!Uri.TryCreate(url, UriKind.Absolute, out var uri) || uri.Scheme != Uri.UriSchemeHttps) return null;
        try
        {
            using var cts = CancellationTokenSource.CreateLinkedTokenSource(ct);
            cts.CancelAfter(RequestTimeout);
            using var res = await _http.GetAsync(uri, HttpCompletionOption.ResponseHeadersRead, cts.Token).ConfigureAwait(false);
            if (!res.IsSuccessStatusCode) return null;
            var final = res.RequestMessage?.RequestUri ?? uri;
            if (final.Scheme != Uri.UriSchemeHttps) return null;
            var bytes = await HttpLimited.ReadBytesAsync(res, HttpLimited.MaxBytes, cts.Token).ConfigureAwait(false);
            return bytes == null ? null : Encoding.UTF8.GetString(bytes);
        }
        catch (OperationCanceledException) when (ct.IsCancellationRequested)
        {
            throw;
        }
        catch
        {
            return null;
        }
    }

    public void Dispose() => _http.Dispose();
}
