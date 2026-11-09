using NowPlayingVrcOsc.Core.Rekordbox;

namespace NowPlayingVrcOsc.Core.Tests;

public class HistoryCursorTests
{
    private static HistoryRow Row(long rid, string id, bool del = false, string? created = null) =>
        new(rid, id, del, created, "c" + rid);

    private static HistoryCursor Baselined(long rowid = 5, string id = "e5")
    {
        var c = new HistoryCursor();
        Assert.Null(c.Observe(rowid, id, null, []));
        return c;
    }

    [Fact]
    public void FirstObservation_SetsBaseline_NoFire()
    {
        var c = new HistoryCursor();
        Assert.Null(c.Observe(5, "e5", null, []));
        Assert.Equal(5, c.Rowid);
        Assert.Equal("e5", c.EntryId);
    }

    [Fact]
    public void NewRow_FiresOnce()
    {
        var c = Baselined();
        var fired = c.Observe(6, "e6", "e5", [Row(6, "e6")]);
        Assert.Equal(6, fired?.RowId);
        Assert.Null(c.Observe(6, "e6", "e6", []));
    }

    [Fact]
    public void OnlyDeletedNewRows_NoFire_ButAdvances()
    {
        var c = Baselined();
        Assert.Null(c.Observe(7, "e7", "e5", [Row(6, "e6", del: true), Row(7, "e7", del: true)]));
        Assert.Equal(7, c.Rowid);
    }

    [Fact]
    public void MaxRowidDecreased_ResetsWithoutFire()
    {
        var c = Baselined();
        Assert.Null(c.Observe(3, "e3", null, []));
        Assert.Equal(3, c.Rowid);
    }

    [Fact]
    public void IdAtCursorChanged_ResetsWithoutFire()
    {
        var c = Baselined();
        Assert.Null(c.Observe(9, "e9", "other", [Row(6, "e6"), Row(9, "e9")]));
        Assert.Equal(9, c.Rowid);
        Assert.Equal("e9", c.EntryId);
    }

    [Fact]
    public void CreatedAtInversion_PicksLatestCreatedAt()
    {
        var c = Baselined();
        var fired = c.Observe(7, "e7", "e5", [
            Row(6, "e6", created: "2026-01-01 12:00:10.000 +00:00"),
            Row(7, "e7", created: "2026-01-01 12:00:05.000 +00:00"),
        ]);
        Assert.Equal(6, fired?.RowId);
    }

    [Fact]
    public void UnparsableCreatedAt_IsOlder_ThenRowid()
    {
        var c = Baselined();
        var fired = c.Observe(8, "e8", "e5", [
            Row(6, "e6", created: "2026-01-01 12:00:10.000 +00:00"),
            Row(7, "e7", created: "garbage"),
            Row(8, "e8", created: null),
        ]);
        Assert.Equal(6, fired?.RowId);

        var c2 = Baselined();
        var fired2 = c2.Observe(7, "e7", "e5", [Row(6, "e6", created: null), Row(7, "e7", created: null)]);
        Assert.Equal(7, fired2?.RowId);
    }

    [Fact]
    public void EmptyTable_ThenInsert_Fires()
    {
        var c = new HistoryCursor();
        Assert.Null(c.Observe(0, null, null, []));
        var fired = c.Observe(1, "e1", null, [Row(1, "e1")]);
        Assert.Equal(1, fired?.RowId);
    }
}

public class ContentNormalizerTests
{
    [Theory]
    [InlineData("$A7:v1:x", true)]
    [InlineData("$A12:v10:abc", true)]
    [InlineData("plain title", false)]
    [InlineData("$A:v1:x", false)]
    [InlineData("", false)]
    public void IsEncrypted(string s, bool expected) => Assert.Equal(expected, ContentNormalizer.IsEncrypted(s));

    [Fact]
    public void Normalize_NullEmptyEncrypted_ToNull()
    {
        Assert.Null(ContentNormalizer.Normalize(null));
        Assert.Null(ContentNormalizer.Normalize(""));
        Assert.Null(ContentNormalizer.Normalize("$A7:v1:x"));
        Assert.Equal("Song", ContentNormalizer.Normalize("Song"));
    }

    [Fact]
    public void SpotifyId_Accepts22Chars()
    {
        Assert.True(ContentNormalizer.TryGetSpotifyTrackId("spotify:track:" + new string('a', 22), out var id));
        Assert.Equal(new string('a', 22), id);
    }

    [Fact]
    public void SpotifyId_Rejects21Chars() =>
        Assert.False(ContentNormalizer.TryGetSpotifyTrackId("spotify:track:" + new string('a', 21), out _));

    [Fact]
    public void SpotifyId_RejectsTrailingNewline() =>
        Assert.False(ContentNormalizer.TryGetSpotifyTrackId("spotify:track:" + new string('a', 22) + "\n", out _));

    [Fact]
    public void Build_MapsFields()
    {
        var t = ContentNormalizer.Build("c1", new ContentRow("T", "x", "memo", "$A1:v1:z", "Alb", null));
        Assert.Equal("c1", t.Id);
        Assert.Equal("T", t.Title);
        Assert.Null(t.Artist);
        Assert.Equal("Alb", t.Album);
        Assert.Equal("memo", t.Comment);
    }
}

public class RekordboxPathsTests
{
    [Fact]
    public void Override_Wins() => Assert.Equal(@"X:\a\master.db", RekordboxPaths.ResolveDbPath(@"X:\a\master.db"));

    [Fact]
    public void Options_ParsesDbPath()
    {
        var f = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString("N") + ".json");
        try
        {
            File.WriteAllText(f, """{"options":[["other","x"],["db-path","D:\\rb\\master.db"]]}""");
            Assert.Equal(@"D:\rb\master.db", RekordboxPaths.ReadDbPathFromOptions(f));
            File.WriteAllText(f, "not json");
            Assert.Null(RekordboxPaths.ReadDbPathFromOptions(f));
        }
        finally { File.Delete(f); }
    }
}
