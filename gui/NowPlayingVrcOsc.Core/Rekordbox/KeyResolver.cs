using System.Runtime.Versioning;

namespace NowPlayingVrcOsc.Core.Rekordbox;

internal interface IKeyResolver
{
    /// <summary>復号キーを返す。見つからなければ null。</summary>
    Task<string?> ResolveAsync(string dbPath, CancellationToken ct);

    /// <summary>キャッシュ済みの鍵を破棄する（鍵が使えなくなったとき）。</summary>
    void Invalidate();
}

/// <summary>
/// 復号キーの解決。順序: (1) DPAPI キャッシュ (2) URL 候補。
/// 各候補は MasterDb で実際に DB を開けたものだけ採用する。
/// 手動入力は App 側が VerifyKey → DpapiKeyStore.Save で行うので (1) に含まれる。
/// </summary>
public sealed class KeyResolver : IKeyResolver
{
    private static readonly TimeSpan UrlRetryInterval = TimeSpan.FromMinutes(10);

    private readonly IKeyStore _store;
    private readonly UrlKeySource? _urls;
    private readonly Func<DateTimeOffset> _clock;
    private readonly Func<string, string, KeyCheckResult> _verify;
    private DateTimeOffset _urlRetryAfter = DateTimeOffset.MinValue;

    internal KeyResolver(IKeyStore store, UrlKeySource? urls,
        Func<DateTimeOffset>? clock = null, Func<string, string, KeyCheckResult>? verify = null)
    {
        _store = store;
        _urls = urls;
        _clock = clock ?? (() => DateTimeOffset.UtcNow);
        _verify = verify ?? MasterDb.VerifyKey;
    }

    /// <summary>既定構成（Windows では DPAPI キャッシュ + 固定 URL）。</summary>
    public static KeyResolver CreateDefault()
    {
        IKeyStore store = OperatingSystem.IsWindows()
            ? CreateDpapiStore()
            : new MemoryKeyStore();
        return new KeyResolver(store, new UrlKeySource());
    }

    [SupportedOSPlatform("windows")]
    private static IKeyStore CreateDpapiStore() => new DpapiKeyStore(DpapiKeyStore.DefaultPath());

    public async Task<string?> ResolveAsync(string dbPath, CancellationToken ct)
    {
        // (1) キャッシュ
        var cached = _store.Load();
        if (cached != null)
        {
            switch (_verify(dbPath, cached))
            {
                case KeyCheckResult.Ok:
                case KeyCheckResult.Busy: // 検証できない場合は本読み取りで判定する
                    return cached;
                case KeyCheckResult.WrongKey:
                case KeyCheckResult.InvalidFormat:
                    _store.Delete();
                    break;
                default:
                    return null; // DB 未検出・その他エラーは破棄せず待つ
            }
        }

        // (2) URL 候補
        if (_urls == null || _clock() < _urlRetryAfter) return null;

        var candidates = await _urls.FetchCandidatesAsync(ct).ConfigureAwait(false);
        foreach (var candidate in candidates)
        {
            ct.ThrowIfCancellationRequested();
            var r = _verify(dbPath, candidate);
            if (r == KeyCheckResult.Ok)
            {
                _store.Save(candidate);
                return candidate;
            }
            if (r is KeyCheckResult.Busy or KeyCheckResult.DbNotFound or KeyCheckResult.Error)
                return null; // 判定不能。バックオフせず次回再試行
        }

        _urlRetryAfter = _clock() + UrlRetryInterval;
        return null;
    }

    public void Invalidate() => _store.Delete();

    private sealed class MemoryKeyStore : IKeyStore
    {
        private string? _key;
        public string? Load() => _key;
        public void Save(string key) => _key = key;
        public void Delete() => _key = null;
    }
}
