using NowPlayingVrcOsc.Core.Rekordbox;

namespace NowPlayingVrcOsc.Core.Tests;

public class MasterDbTests
{
    private static string OtherKey(string key) => key[0] == 'a' ? "b" + key[1..] : "a" + key[1..];

    [Fact]
    public void WalOnlyRows_AreReadable_WhileWriterOpen()
    {
        using var fx = new FixtureDb();
        fx.AddContent("c1", "Song One", artist: "Artist One", album: "Album One", label: "Label One", comment: "memo");
        fx.AddHistory("h1", "c1", createdAt: "2026-01-01 00:00:00.000 +00:00");
        Assert.True(File.Exists(fx.WalPath));

        using var db = MasterDb.Open(fx.Path, fx.Key);
        db.BeginRead();
        var (rid, id) = db.GetMax();
        Assert.Equal(1, rid);
        Assert.Equal("h1", id);
        Assert.Equal("h1", db.GetIdAt(1));
        var rows = db.GetRowsAfter(0);
        Assert.Single(rows);
        Assert.Equal("c1", rows[0].ContentId);
        Assert.False(rows[0].Deleted);

        var c = db.GetContent("c1");
        Assert.NotNull(c);
        Assert.Equal("Song One", c!.Title);
        Assert.Equal("Artist One", c.Artist);
        Assert.Equal("Album One", c.Album);
        Assert.Equal("Label One", c.Label);
        Assert.Equal("memo", c.Comment);
        Assert.Null(db.GetContent("nope"));
        db.Commit();
    }

    [Fact]
    public void WrongKey_ThrowsNotADb_AndVerifyKeyReportsWrongKey()
    {
        using var fx = new FixtureDb();
        var ex = Assert.ThrowsAny<Microsoft.Data.Sqlite.SqliteException>(() => MasterDb.Open(fx.Path, OtherKey(fx.Key)));
        Assert.Equal(DbErrorKind.NotADb, MasterDb.Classify(ex));
        Assert.Equal(KeyCheckResult.WrongKey, MasterDb.VerifyKey(fx.Path, OtherKey(fx.Key)));
        Assert.DoesNotContain(fx.Key, ex.Message);
    }

    [Fact]
    public void VerifyKey_Results()
    {
        using var fx = new FixtureDb();
        Assert.Equal(KeyCheckResult.Ok, MasterDb.VerifyKey(fx.Path, fx.Key));
        Assert.Equal(KeyCheckResult.InvalidFormat, MasterDb.VerifyKey(fx.Path, "xyz"));
        Assert.Equal(KeyCheckResult.DbNotFound, MasterDb.VerifyKey(Path.Combine(fx.Dir, "none.db"), fx.Key));
    }

    [Fact]
    public void VerifyKey_WithoutWal_WorksAndCreatesNoFiles()
    {
        using var fx = new FixtureDb();
        fx.AddContent("c1", "x");
        fx.CloseWriter();
        Assert.False(File.Exists(fx.WalPath));
        Assert.False(File.Exists(fx.ShmPath));
        var before = Directory.GetFiles(fx.Dir).Select(Path.GetFileName).Order().ToArray();

        Assert.Equal(KeyCheckResult.Ok, MasterDb.VerifyKey(fx.Path, fx.Key));
        Assert.Equal(KeyCheckResult.WrongKey, MasterDb.VerifyKey(fx.Path, OtherKey(fx.Key)));

        Assert.False(File.Exists(fx.WalPath));
        Assert.False(File.Exists(fx.ShmPath));
        Assert.Equal(before, Directory.GetFiles(fx.Dir).Select(Path.GetFileName).Order().ToArray());
    }

    [Fact]
    public void InvalidUtf8Text_DoesNotThrow()
    {
        using var fx = new FixtureDb();
        fx.AddContent("c1", "x");
        fx.Exec("UPDATE djmdContent SET Title = CAST(x'41e3ff42' AS TEXT) WHERE ID='c1'");
        fx.AddHistory("h1", "c1");

        using var db = MasterDb.Open(fx.Path, fx.Key);
        db.BeginRead();
        var c = db.GetContent("c1");
        Assert.NotNull(c?.Title);
        Assert.StartsWith("A", c!.Title);
        Assert.EndsWith("B", c.Title);
        Assert.NotNull(db.QueryScalarText("SELECT Title FROM djmdContent"));
        db.Commit();
    }

    [Fact]
    public void WithoutRowIdHistory_IsSchemaError()
    {
        using var fx = new FixtureDb();
        fx.Exec("DROP TABLE djmdSongHistory; CREATE TABLE djmdSongHistory(ID TEXT PRIMARY KEY, ContentID TEXT) WITHOUT ROWID;");
        using var db = MasterDb.Open(fx.Path, fx.Key);
        Assert.Throws<MasterDbSchemaException>(() => db.BeginRead());
    }

    [Fact]
    public void MissingOptionalColumns_TreatedAsNull()
    {
        using var fx = new FixtureDb();
        fx.Exec("DROP TABLE djmdSongHistory; CREATE TABLE djmdSongHistory(ID TEXT, ContentID TEXT);" +
                "DROP TABLE djmdContent; CREATE TABLE djmdContent(ID TEXT, Title TEXT);" +
                "INSERT INTO djmdSongHistory VALUES('h1','c1'); INSERT INTO djmdContent VALUES('c1','T');");
        using var db = MasterDb.Open(fx.Path, fx.Key);
        db.BeginRead();
        var rows = db.GetRowsAfter(0);
        Assert.Single(rows);
        Assert.False(rows[0].Deleted);
        Assert.Null(rows[0].CreatedAt);
        var c = db.GetContent("c1");
        Assert.Equal("T", c?.Title);
        Assert.Null(c?.Artist);
        db.Commit();
    }
}

public class RekordboxTrackSourceTests
{
    private sealed class Harness : IAsyncDisposable
    {
        public FixtureDb Fx { get; }
        public FakeKeyResolver Keys { get; }
        public FakeSpotify Spotify { get; } = new();
        public List<TrackInfo> Tracks { get; } = [];
        public List<string> Statuses { get; } = [];
        public RekordboxTrackSource Source { get; }

        public Harness(FixtureDb fx, string? key = "")
        {
            Fx = fx;
            Keys = new FakeKeyResolver { Key = key == "" ? fx.Key : key };
            Source = new RekordboxTrackSource(
                () => fx.Path, () => 0.2, Keys, Spotify, (_, ct) => Task.Delay(10, ct));
            Source.TrackReceived += t => { lock (Tracks) Tracks.Add(t); };
            Source.StatusChanged += s => { lock (Statuses) Statuses.Add(s); };
        }

        private Task? _run;

        /// <summary>監視を裏で開始する（StartAsync はキャンセルまで戻らないため待たない）。</summary>
        public Task StartAsync()
        {
            _run = Source.StartAsync(CancellationToken.None);
            return Task.CompletedTask;
        }

        public bool HasStatus(string s) { lock (Statuses) return Statuses.Contains(s); }
        public int TrackCount { get { lock (Tracks) return Tracks.Count; } }

        public async ValueTask DisposeAsync()
        {
            await Source.DisposeAsync();
            if (_run != null) await _run;
        }
    }

    [Fact]
    public async Task StartAsync_RunsUntilCancelled()
    {
        // App は StartAsync の完了を「停止」とみなす。開始直後に完了してはいけない。
        using var fx = new FixtureDb();
        await using var h = new Harness(fx);
        using var cts = new CancellationTokenSource();
        var run = h.Source.StartAsync(cts.Token);

        await Wait.UntilAsync(() => h.HasStatus(Connected));
        await Task.Delay(200);
        Assert.False(run.IsCompleted);

        cts.Cancel();
        var finished = await Task.WhenAny(run, Task.Delay(5000));
        Assert.Same(run, finished);
        Assert.True(run.IsCompletedSuccessfully);
    }

    private const string Connected = "rekordbox に接続しました";

    [Fact]
    public async Task Startup_EmitsNothing_ThenNewRowEmitsExactlyOne()
    {
        using var fx = new FixtureDb();
        fx.AddContent("c0", "Old Song", artist: "Old Artist");
        fx.AddHistory("h0", "c0");
        fx.AddContent("c1", "New Song", artist: "New Artist", album: "Alb");

        await using var h = new Harness(fx);
        await h.StartAsync();
        await Wait.UntilAsync(() => h.HasStatus(Connected));
        await Task.Delay(150);
        Assert.Equal(0, h.TrackCount);

        fx.AddHistory("h1", "c1");
        await Wait.UntilAsync(() => h.TrackCount >= 1);
        await Task.Delay(300);

        Assert.Equal(1, h.TrackCount);
        var t = h.Tracks[0];
        Assert.Equal("c1", t.Id);
        Assert.Equal("New Song", t.Title);
        Assert.Equal("New Artist", t.Artist);
        Assert.Equal("Alb", t.Album);
        Assert.Equal(1, h.Statuses.Count(s => s == Connected));
    }

    [Fact]
    public async Task SpotifyRow_WithEncryptedTitle_UsesOEmbedTitle()
    {
        using var fx = new FixtureDb();
        var sid = "0123456789abcdefghijAB";
        fx.AddContent("c1", "$A7:v1:enc", folderPath: "spotify:track:" + sid, artist: "$A7:v1:enc2");

        await using var h = new Harness(fx);
        h.Spotify.Title = "Spotify Title";
        await h.StartAsync();
        await Wait.UntilAsync(() => h.HasStatus(Connected));

        fx.AddHistory("h1", "c1");
        await Wait.UntilAsync(() => h.TrackCount >= 1);

        Assert.Equal("Spotify Title", h.Tracks[0].Title);
        Assert.Null(h.Tracks[0].Artist);
        Assert.Equal([sid], h.Spotify.Requested);
    }

    [Fact]
    public async Task OEmbedFailure_EmitsNothing_AndRaisesStatus()
    {
        using var fx = new FixtureDb();
        fx.AddContent("c1", "$A7:v1:enc", folderPath: "spotify:track:0123456789abcdefghijAB");

        await using var h = new Harness(fx);
        h.Spotify.Title = null;
        await h.StartAsync();
        await Wait.UntilAsync(() => h.HasStatus(Connected));

        fx.AddHistory("h1", "c1");
        await Wait.UntilAsync(() => h.HasStatus("曲名を取得できませんでした（Spotify）"));
        await Task.Delay(200);
        Assert.Equal(0, h.TrackCount);
    }

    [Fact]
    public async Task WalMissing_DoesNotOpenDb_AndReportsWaiting()
    {
        using var fx = new FixtureDb();
        fx.AddContent("c1", "x");
        fx.CloseWriter();
        var before = Directory.GetFiles(fx.Dir).Select(Path.GetFileName).Order().ToArray();

        await using var h = new Harness(fx);
        await h.StartAsync();
        await Wait.UntilAsync(() => h.HasStatus("rekordbox の起動を待っています"));
        await Task.Delay(150);

        Assert.Equal(before, Directory.GetFiles(fx.Dir).Select(Path.GetFileName).Order().ToArray());
        Assert.False(File.Exists(fx.WalPath));
        Assert.False(File.Exists(fx.ShmPath));
    }

    [Fact]
    public async Task NoKey_ReportsStatus()
    {
        using var fx = new FixtureDb();
        await using var h = new Harness(fx, key: null);
        await h.StartAsync();
        await Wait.UntilAsync(() => h.HasStatus("復号キーを取得できません（設定画面で入力してください）"));
    }

    [Fact]
    public async Task WrongKey_ThreeTimes_InvalidatesResolver_AndStatusHasNoKey()
    {
        using var fx = new FixtureDb();
        var wrong = fx.Key[0] == 'a' ? "b" + fx.Key[1..] : "a" + fx.Key[1..];
        await using var h = new Harness(fx, key: wrong);
        await h.StartAsync();
        await Wait.UntilAsync(() => h.Keys.Invalidated >= 1);
        lock (h.Statuses) Assert.DoesNotContain(h.Statuses, s => s.Contains(wrong));
    }
}
