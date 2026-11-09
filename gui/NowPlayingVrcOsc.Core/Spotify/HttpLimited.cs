namespace NowPlayingVrcOsc.Core;

/// <summary>外部 HTTP 取得の共通処理（サイズ上限つき本文読み取り）。</summary>
internal static class HttpLimited
{
    public const int MaxBytes = 1024 * 1024;

    /// <summary>本文を上限付きで読む。超過時は null。</summary>
    public static async Task<byte[]?> ReadBytesAsync(HttpResponseMessage response, int maxBytes, CancellationToken ct)
    {
        if (response.Content.Headers.ContentLength is long len && len > maxBytes) return null;
        await using var stream = await response.Content.ReadAsStreamAsync(ct).ConfigureAwait(false);
        using var ms = new MemoryStream();
        var buf = new byte[8192];
        while (true)
        {
            var n = await stream.ReadAsync(buf, ct).ConfigureAwait(false);
            if (n == 0) break;
            if (ms.Length + n > maxBytes) return null;
            ms.Write(buf, 0, n);
        }
        return ms.ToArray();
    }
}
