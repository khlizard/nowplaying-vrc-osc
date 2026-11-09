using System.Net;
using System.Security.Cryptography;
using System.Text;
using Microsoft.Data.Sqlite;
using NowPlayingVrcOsc.Core.Rekordbox;
using NowPlayingVrcOsc.Core.Spotify;

namespace NowPlayingVrcOsc.Core.Tests;

/// <summary>
/// テスト用の暗号化 master.db。実際の rekordbox DB は使わず、同じ SQLite3MC で最小スキーマを作る。
/// ライター接続を開いたままにするので -wal が残り、WAL のみの行も読める。
/// </summary>
internal sealed class FixtureDb : IDisposable
{
    public string Dir { get; }
    public string Path { get; }
    public string Key { get; }
    private SqliteConnection? _writer;

    public string WalPath => Path + "-wal";
    public string ShmPath => Path + "-shm";

    public FixtureDb()
    {
        SqliteInit.EnsureInitialized();
        Dir = System.IO.Path.Combine(System.IO.Path.GetTempPath(), "npvo-test-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Dir);
        Path = System.IO.Path.Combine(Dir, "master.db");
        Key = RandomNumberGenerator.GetHexString(64, lowercase: true);

        _writer = new SqliteConnection(new SqliteConnectionStringBuilder
        {
            DataSource = Path,
            Mode = SqliteOpenMode.ReadWriteCreate,
            Pooling = false,
        }.ToString());
        _writer.Open();
        Exec("PRAGMA cipher='sqlcipher'; PRAGMA legacy=4; PRAGMA key='" + Key + "';");
        Exec("PRAGMA journal_mode=WAL;");
        Exec("PRAGMA wal_autocheckpoint=0;");
        Exec(
            "CREATE TABLE djmdSongHistory(ID VARCHAR(255) PRIMARY KEY, HistoryID VARCHAR(255), ContentID VARCHAR(255)," +
            " rb_local_deleted INTEGER DEFAULT 0, created_at TEXT);" +
            "CREATE TABLE djmdContent(ID VARCHAR(255) PRIMARY KEY, FolderPath TEXT, Title TEXT, ArtistID VARCHAR(255)," +
            " AlbumID VARCHAR(255), LabelID VARCHAR(255), Commnt TEXT);" +
            "CREATE TABLE djmdArtist(ID VARCHAR(255) PRIMARY KEY, Name TEXT);" +
            "CREATE TABLE djmdAlbum(ID VARCHAR(255) PRIMARY KEY, Name TEXT);" +
            "CREATE TABLE djmdLabel(ID VARCHAR(255) PRIMARY KEY, Name TEXT);");
    }

    public void Exec(string sql)
    {
        using var cmd = _writer!.CreateCommand();
        cmd.CommandText = sql;
        cmd.ExecuteNonQuery();
    }

    public void AddContent(string id, string? title, string folderPath = "C:/music/a.mp3",
        string? artist = null, string? album = null, string? label = null, string? comment = null)
    {
        string? artistId = null, albumId = null, labelId = null;
        if (artist != null) { artistId = "ar" + id; Exec($"INSERT INTO djmdArtist VALUES('{artistId}', {Q(artist)})"); }
        if (album != null) { albumId = "al" + id; Exec($"INSERT INTO djmdAlbum VALUES('{albumId}', {Q(album)})"); }
        if (label != null) { labelId = "lb" + id; Exec($"INSERT INTO djmdLabel VALUES('{labelId}', {Q(label)})"); }
        Exec($"INSERT INTO djmdContent VALUES('{id}', {Q(folderPath)}, {Q(title)}, {Q(artistId)}, {Q(albumId)}, {Q(labelId)}, {Q(comment)})");
    }

    public void AddHistory(string id, string contentId, bool deleted = false, string? createdAt = null) =>
        Exec($"INSERT INTO djmdSongHistory(ID, HistoryID, ContentID, rb_local_deleted, created_at) " +
             $"VALUES('{id}', 'h1', '{contentId}', {(deleted ? 1 : 0)}, {Q(createdAt)})");

    /// <summary>ライターを閉じる（最後の接続なので WAL がチェックポイントされ -wal が消える）。</summary>
    public void CloseWriter()
    {
        _writer?.Dispose();
        _writer = null;
    }

    private static string Q(string? s) => s == null ? "NULL" : "'" + s.Replace("'", "''") + "'";

    public void Dispose()
    {
        CloseWriter();
        try { Directory.Delete(Dir, recursive: true); } catch { }
    }
}

internal sealed class FakeHandler : HttpMessageHandler
{
    private readonly Func<HttpRequestMessage, HttpResponseMessage> _respond;
    public List<Uri> Requests { get; } = [];
    public List<string?> UserAgents { get; } = [];

    public FakeHandler(Func<HttpRequestMessage, HttpResponseMessage> respond) => _respond = respond;

    public static HttpResponseMessage Json(HttpRequestMessage req, string json, HttpStatusCode code = HttpStatusCode.OK) =>
        new(code) { Content = new StringContent(json, Encoding.UTF8, "application/json"), RequestMessage = req };

    protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
    {
        Requests.Add(request.RequestUri!);
        UserAgents.Add(request.Headers.UserAgent.ToString());
        return Task.FromResult(_respond(request));
    }
}

internal sealed class FakeKeyResolver : IKeyResolver
{
    public string? Key { get; set; }
    public int Invalidated { get; private set; }
    public Task<string?> ResolveAsync(string dbPath, CancellationToken ct) => Task.FromResult(Key);
    public void Invalidate() => Invalidated++;
}

internal sealed class FakeSpotify : ISpotifyTitleProvider
{
    public string? Title { get; set; }
    public List<string> Requested { get; } = [];
    public Task<string?> GetTitleAsync(string trackId, CancellationToken ct)
    {
        Requested.Add(trackId);
        return Task.FromResult(Title);
    }
}

internal sealed class FakeClock
{
    public DateTimeOffset Now { get; set; } = new(2026, 1, 1, 0, 0, 0, TimeSpan.Zero);
    public DateTimeOffset Get() => Now;
}

internal static class Wait
{
    public static async Task UntilAsync(Func<bool> condition, int timeoutMs = 15000)
    {
        var end = Environment.TickCount64 + timeoutMs;
        while (!condition())
        {
            if (Environment.TickCount64 > end) throw new TimeoutException("条件が満たされませんでした。");
            await Task.Delay(20);
        }
    }
}
