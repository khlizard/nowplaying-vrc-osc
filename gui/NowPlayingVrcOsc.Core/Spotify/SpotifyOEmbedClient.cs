using System.Net;
using System.Text.Json;
using NowPlayingVrcOsc.Core.Rekordbox;

namespace NowPlayingVrcOsc.Core.Spotify;

internal interface ISpotifyTitleProvider
{
    Task<string?> GetTitleAsync(string trackId, CancellationToken ct);
}

/// <summary>
/// Spotify oEmbed（認証不要）から曲名を取得する。失敗しても例外を投げず null を返す。
/// 成功はメモリキャッシュ、失敗した ID は 10 分再試行しない、429 は Retry-After の間全体を停止。
/// </summary>
public sealed class SpotifyOEmbedClient : ISpotifyTitleProvider, IDisposable
{
    public const string Host = "open.spotify.com";
    private const int MaxTitleLength = 500;
    private const int DefaultRetryAfterSeconds = 60;
    private const int MaxRetryAfterSeconds = 3600;
    private static readonly TimeSpan RequestTimeout = TimeSpan.FromSeconds(5);
    private static readonly TimeSpan FailedRetryInterval = TimeSpan.FromMinutes(10);

    private readonly HttpClient _http;
    private readonly Func<DateTimeOffset> _clock;
    private readonly object _gate = new();
    private readonly Dictionary<string, string> _cache = new(StringComparer.Ordinal);
    private readonly Dictionary<string, DateTimeOffset> _failedUntil = new(StringComparer.Ordinal);
    private DateTimeOffset _pausedUntil = DateTimeOffset.MinValue;

    public SpotifyOEmbedClient() : this(null, null) { }

    public SpotifyOEmbedClient(HttpMessageHandler? handler, Func<DateTimeOffset>? clock)
    {
        _http = handler != null ? new HttpClient(handler, disposeHandler: false) : new HttpClient();
        _http.Timeout = Timeout.InfiniteTimeSpan; // CancellationToken 側で制御する
        _clock = clock ?? (() => DateTimeOffset.UtcNow);
    }

    public async Task<string?> GetTitleAsync(string trackId, CancellationToken ct)
    {
        if (!ContentNormalizer.IsSpotifyTrackId(trackId)) return null;

        lock (_gate)
        {
            if (_cache.TryGetValue(trackId, out var cached)) return cached;
            var now = _clock();
            if (now < _pausedUntil) return null;
            if (_failedUntil.TryGetValue(trackId, out var until) && now < until) return null;
        }

        var url = "https://open.spotify.com/oembed?url=" +
                  Uri.EscapeDataString("https://open.spotify.com/track/" + trackId);
        try
        {
            using var cts = CancellationTokenSource.CreateLinkedTokenSource(ct);
            cts.CancelAfter(RequestTimeout);
            using var req = new HttpRequestMessage(HttpMethod.Get, url);
            req.Headers.UserAgent.ParseAdd("NowPlayingVrcOsc");
            using var res = await _http.SendAsync(req, HttpCompletionOption.ResponseHeadersRead, cts.Token).ConfigureAwait(false);

            if (res.StatusCode == HttpStatusCode.TooManyRequests)
            {
                Pause(RetryAfterSeconds(res));
                return null;
            }
            if (!res.IsSuccessStatusCode) return Fail(trackId);

            var final = res.RequestMessage?.RequestUri;
            if (final == null || final.Scheme != Uri.UriSchemeHttps ||
                !string.Equals(final.Host, Host, StringComparison.OrdinalIgnoreCase))
                return Fail(trackId);

            var bytes = await HttpLimited.ReadBytesAsync(res, HttpLimited.MaxBytes, cts.Token).ConfigureAwait(false);
            if (bytes == null) return Fail(trackId);

            var title = ParseTitle(bytes);
            if (title == null) return Fail(trackId);

            lock (_gate) _cache[trackId] = title;
            return title;
        }
        catch (OperationCanceledException) when (ct.IsCancellationRequested)
        {
            throw;
        }
        catch
        {
            return Fail(trackId);
        }
    }

    private static string? ParseTitle(byte[] body)
    {
        try
        {
            using var doc = JsonDocument.Parse(body);
            if (doc.RootElement.ValueKind != JsonValueKind.Object ||
                !doc.RootElement.TryGetProperty("title", out var t) ||
                t.ValueKind != JsonValueKind.String)
                return null;
            var s = t.GetString()?.Trim();
            return string.IsNullOrEmpty(s) || s.Length > MaxTitleLength ? null : s;
        }
        catch (JsonException)
        {
            return null;
        }
    }

    private string? Fail(string trackId)
    {
        lock (_gate) _failedUntil[trackId] = _clock() + FailedRetryInterval;
        return null;
    }

    private void Pause(int seconds)
    {
        lock (_gate) _pausedUntil = _clock() + TimeSpan.FromSeconds(seconds);
    }

    private int RetryAfterSeconds(HttpResponseMessage res)
    {
        var ra = res.Headers.RetryAfter;
        double seconds = DefaultRetryAfterSeconds;
        if (ra?.Delta is TimeSpan d) seconds = d.TotalSeconds;
        else if (ra?.Date is DateTimeOffset date) seconds = (date - _clock()).TotalSeconds;
        if (seconds < 0) seconds = DefaultRetryAfterSeconds;
        return (int)Math.Min(Math.Ceiling(seconds), MaxRetryAfterSeconds);
    }

    public void Dispose() => _http.Dispose();
}
