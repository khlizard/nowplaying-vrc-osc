using System.Net;
using System.Net.Http.Headers;
using NowPlayingVrcOsc.Core.Rekordbox;
using NowPlayingVrcOsc.Core.Spotify;

namespace NowPlayingVrcOsc.Core.Tests;

public class SpotifyOEmbedClientTests
{
    private const string Id = "0123456789abcdefghijAB";
    private const string Id2 = "0123456789abcdefghijCD";

    private static SpotifyOEmbedClient Make(FakeHandler h, FakeClock clock) => new(h, clock.Get);

    [Fact]
    public async Task RequestsExactUrl_ParsesTrimmedTitle_AndCaches()
    {
        var h = new FakeHandler(r => FakeHandler.Json(r, """{"title":"  My Song  "}"""));
        var c = Make(h, new FakeClock());

        Assert.Equal("My Song", await c.GetTitleAsync(Id, default));
        Assert.Equal("My Song", await c.GetTitleAsync(Id, default));

        Assert.Single(h.Requests);
        Assert.Equal(
            "https://open.spotify.com/oembed?url=" + Uri.EscapeDataString("https://open.spotify.com/track/" + Id),
            h.Requests[0].AbsoluteUri);
        Assert.Equal("NowPlayingVrcOsc", h.UserAgents[0]);
    }

    [Fact]
    public async Task EmptyOrTooLongTitle_Fails()
    {
        var h = new FakeHandler(r => FakeHandler.Json(r, """{"title":"   "}"""));
        Assert.Null(await Make(h, new FakeClock()).GetTitleAsync(Id, default));

        var h2 = new FakeHandler(r => FakeHandler.Json(r, "{\"title\":\"" + new string('a', 501) + "\"}"));
        Assert.Null(await Make(h2, new FakeClock()).GetTitleAsync(Id, default));
    }

    [Fact]
    public async Task TooManyRequests_PausesAllRequests_ForRetryAfter()
    {
        var clock = new FakeClock();
        var calls = 0;
        var h = new FakeHandler(r =>
        {
            calls++;
            if (calls == 1)
            {
                var res = new HttpResponseMessage(HttpStatusCode.TooManyRequests) { RequestMessage = r };
                res.Headers.RetryAfter = new RetryConditionHeaderValue(TimeSpan.FromSeconds(30));
                return res;
            }
            return FakeHandler.Json(r, """{"title":"OK"}""");
        });
        var c = Make(h, clock);

        Assert.Null(await c.GetTitleAsync(Id, default));
        Assert.Equal(1, calls);

        // 停止中は別 ID でもリクエストしない
        clock.Now += TimeSpan.FromSeconds(29);
        Assert.Null(await c.GetTitleAsync(Id2, default));
        Assert.Equal(1, calls);

        clock.Now += TimeSpan.FromSeconds(2);
        Assert.Equal("OK", await c.GetTitleAsync(Id2, default));
        // 429 の ID は失敗扱いではないので再試行できる
        Assert.Equal("OK", await c.GetTitleAsync(Id, default));
    }

    [Fact]
    public async Task RedirectToOtherHost_Fails()
    {
        var h = new FakeHandler(r =>
        {
            var res = FakeHandler.Json(r, """{"title":"Evil"}""");
            res.RequestMessage = new HttpRequestMessage(HttpMethod.Get, "https://evil.example.com/x");
            return res;
        });
        Assert.Null(await Make(h, new FakeClock()).GetTitleAsync(Id, default));
    }

    [Fact]
    public async Task FailedId_NotRetriedWithin10Minutes()
    {
        var clock = new FakeClock();
        var fail = true;
        var h = new FakeHandler(r => fail
            ? FakeHandler.Json(r, "{}", HttpStatusCode.NotFound)
            : FakeHandler.Json(r, """{"title":"Now OK"}"""));
        var c = Make(h, clock);

        Assert.Null(await c.GetTitleAsync(Id, default));
        fail = false;
        clock.Now += TimeSpan.FromMinutes(9);
        Assert.Null(await c.GetTitleAsync(Id, default));
        Assert.Single(h.Requests);

        clock.Now += TimeSpan.FromMinutes(2);
        Assert.Equal("Now OK", await c.GetTitleAsync(Id, default));
    }

    [Fact]
    public async Task InvalidTrackId_NoRequest()
    {
        var h = new FakeHandler(r => FakeHandler.Json(r, """{"title":"x"}"""));
        Assert.Null(await Make(h, new FakeClock()).GetTitleAsync("short", default));
        Assert.Empty(h.Requests);
    }
}

public class UrlKeySourceTests
{
    private static string Hex(char c, int n) => new(c, n);

    [Fact]
    public async Task Extracts64Hex_IgnoresLonger_Dedupes()
    {
        var k1 = Hex('a', 64);
        var k2 = Hex('B', 64);
        var body = $"key1 = \"{k1}\" again {k1} and '{k2}' too-long {Hex('c', 65)} short {Hex('d', 63)}";
        var h = new FakeHandler(r => new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent(body), RequestMessage = r });
        using var src = new UrlKeySource(h, ["https://example.com/a"]);

        var result = await src.FetchCandidatesAsync(default);
        Assert.Equal([k1, k2], result);
    }

    [Fact]
    public async Task MaxTwentyCandidates()
    {
        var body = string.Join(' ', Enumerable.Range(0, 30).Select(i => i.ToString("x").PadLeft(64, '0')));
        var h = new FakeHandler(r => new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent(body), RequestMessage = r });
        using var src = new UrlKeySource(h, ["https://example.com/a"]);
        Assert.Equal(20, (await src.FetchCandidatesAsync(default)).Count);
    }

    [Fact]
    public async Task NonHttps_Rejected_WithoutRequest()
    {
        var h = new FakeHandler(r => new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent(Hex('a', 64)), RequestMessage = r });
        using var src = new UrlKeySource(h, ["http://example.com/a"]);
        Assert.Empty(await src.FetchCandidatesAsync(default));
        Assert.Empty(h.Requests);
    }

    [Fact]
    public async Task RedirectToHttp_Rejected()
    {
        var h = new FakeHandler(r => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent(Hex('a', 64)),
            RequestMessage = new HttpRequestMessage(HttpMethod.Get, "http://example.com/a"),
        });
        using var src = new UrlKeySource(h, ["https://example.com/a"]);
        Assert.Empty(await src.FetchCandidatesAsync(default));
    }

    [Fact]
    public async Task OverOneMiB_Rejected()
    {
        var body = Hex('a', 64) + new string(' ', 1024 * 1024 + 10);
        var h = new FakeHandler(r => new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent(body), RequestMessage = r });
        using var src = new UrlKeySource(h, ["https://example.com/a"]);
        Assert.Empty(await src.FetchCandidatesAsync(default));
    }

    [Fact]
    public void DefaultSources_AreTwoHttpsUrls()
    {
        var urls = UrlKeySource.LoadDefaultUrls();
        Assert.Equal(2, urls.Count);
        Assert.All(urls, u => Assert.StartsWith("https://raw.githubusercontent.com/", u));
    }
}

public class KeyResolverTests
{
    private sealed class MemStore : IKeyStore
    {
        public string? Value;
        public string? Load() => Value;
        public void Save(string key) => Value = key;
        public void Delete() => Value = null;
    }

    [Fact]
    public async Task UrlCandidate_AcceptedOnlyIfVerified_SavedToCache_AndBackoffWhenNone()
    {
        var good = new string('a', 64);
        var bad = new string('b', 64);
        var fetches = 0;
        var h = new FakeHandler(r =>
        {
            fetches++;
            return new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent(bad + " " + good), RequestMessage = r };
        });
        var clock = new FakeClock();
        var store = new MemStore();
        var good2 = good;
        using var urls = new UrlKeySource(h, ["https://example.com/a"]);
        var resolver = new KeyResolver(store, urls, clock.Get,
            (_, k) => k == good2 ? KeyCheckResult.Ok : KeyCheckResult.WrongKey);

        Assert.Equal(good, await resolver.ResolveAsync("x", default));
        Assert.Equal(good, store.Value);

        // 鍵が使えなくなった: 破棄 → 候補なしになるとバックオフ
        good2 = "none";
        store.Value = null;
        Assert.Null(await resolver.ResolveAsync("x", default));
        Assert.Equal(2, fetches);
        clock.Now += TimeSpan.FromMinutes(5);
        Assert.Null(await resolver.ResolveAsync("x", default));
        Assert.Equal(2, fetches);
        clock.Now += TimeSpan.FromMinutes(6);
        Assert.Null(await resolver.ResolveAsync("x", default));
        Assert.Equal(3, fetches);
    }

    [Fact]
    public async Task CachedKey_UsedFirst_WrongCachedKeyDeleted()
    {
        var store = new MemStore { Value = new string('c', 64) };
        var resolver = new KeyResolver(store, null, null, (_, k) => KeyCheckResult.Ok);
        Assert.Equal(new string('c', 64), await resolver.ResolveAsync("x", default));

        var resolver2 = new KeyResolver(store, null, null, (_, k) => KeyCheckResult.WrongKey);
        Assert.Null(await resolver2.ResolveAsync("x", default));
        Assert.Null(store.Value);
    }
}
