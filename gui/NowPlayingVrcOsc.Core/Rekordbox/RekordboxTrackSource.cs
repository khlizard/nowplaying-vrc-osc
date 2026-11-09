using Microsoft.Data.Sqlite;
using NowPlayingVrcOsc.Core.Spotify;

namespace NowPlayingVrcOsc.Core.Rekordbox;

/// <summary>
/// rekordbox の master.db（履歴）を読み取り専用でポーリングし、新しく再生された曲を通知する。
/// 起動時点の曲は通知しない（カーソルのメモリ上基準のみ）。
/// </summary>
public sealed class RekordboxTrackSource : ITrackSource
{
    private const int NotADbLimit = 3;
    private const double MinIntervalSeconds = 0.2;

    private const string MsgDbMissing = "master.db が見つかりません";
    private const string MsgWaiting = "rekordbox の起動を待っています";
    private const string MsgNoKey = "復号キーを取得できません（設定画面で入力してください）";
    private const string MsgConnected = "rekordbox に接続しました";
    private const string MsgBusy = "rekordbox がデータベースを使用中です。再試行します";
    private const string MsgBadKey = "復号キーでデータベースを開けません。再試行します";
    private const string MsgSchema = "master.db の構成が想定と異なります";
    private const string MsgNoTitleSpotify = "曲名を取得できませんでした（Spotify）";
    private const string MsgNoTitle = "曲名を取得できませんでした";

    private readonly Func<string> _getDbPath;
    private readonly Func<double> _getIntervalSeconds;
    private readonly IKeyResolver _keys;
    private readonly ISpotifyTitleProvider _spotify;
    private readonly Func<TimeSpan, CancellationToken, Task> _delay;
    private readonly HistoryCursor _cursor = new();

    private CancellationTokenSource? _cts;
    private Task? _loop;
    private string? _key;
    private string? _lastSignature;
    private bool _connected;
    private int _notADbCount;
    private string? _lastState;

    public event Action<TrackInfo>? TrackReceived;
    /// <summary>接続状態・エラーの変化（UI 表示用）。バックグラウンドスレッドから呼ばれる。</summary>
    public event Action<string>? StatusChanged;

    public RekordboxTrackSource(AppSettings settings)
        : this(
            () => RekordboxPaths.ResolveDbPath(settings.DbPathOverride),
            () => settings.PollIntervalSeconds,
            KeyResolver.CreateDefault(),
            new SpotifyOEmbedClient(),
            Task.Delay)
    {
    }

    internal RekordboxTrackSource(
        Func<string> getDbPath,
        Func<double> getIntervalSeconds,
        IKeyResolver keys,
        ISpotifyTitleProvider spotify,
        Func<TimeSpan, CancellationToken, Task> delay)
    {
        _getDbPath = getDbPath;
        _getIntervalSeconds = getIntervalSeconds;
        _keys = keys;
        _spotify = spotify;
        _delay = delay;
    }

    /// <summary>
    /// 監視を開始し、キャンセルされる（または DisposeAsync される）まで戻らない。
    /// App はこの Task の完了を「停止した」とみなすため、ループの終了まで待つこと。
    /// </summary>
    public async Task StartAsync(CancellationToken cancellationToken)
    {
        if (_loop != null) return;
        _cts = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        var ct = _cts.Token;
        var loop = Task.Run(() => RunAsync(ct), CancellationToken.None);
        _loop = loop;
        await loop.ConfigureAwait(false);
    }

    private async Task RunAsync(CancellationToken ct)
    {
        while (!ct.IsCancellationRequested)
        {
            try
            {
                await TickAsync(ct).ConfigureAwait(false);
            }
            catch (OperationCanceledException) when (ct.IsCancellationRequested)
            {
                break;
            }
            catch (Exception ex)
            {
                // 想定外の例外でもループを止めない。型名のみ通知する（本文は出さない）。
                _connected = false;
                Notify($"予期しないエラー ({ex.GetType().Name})");
            }

            try
            {
                var sec = Math.Max(MinIntervalSeconds, _getIntervalSeconds());
                await _delay(TimeSpan.FromSeconds(sec), ct).ConfigureAwait(false);
            }
            catch (OperationCanceledException)
            {
                break;
            }
        }
    }

    private async Task TickAsync(CancellationToken ct)
    {
        var dbPath = _getDbPath();
        var walPath = dbPath + "-wal";

        if (!File.Exists(dbPath))
        {
            Disconnect(MsgDbMissing);
            return;
        }
        if (!File.Exists(walPath))
        {
            // -wal が無いときは開かない（rekordbox 未起動。開くと -wal/-shm を作ってしまう）
            Disconnect(MsgWaiting);
            return;
        }

        var signature = Signature(dbPath, walPath);
        if (_connected && signature == _lastSignature) return;

        if (_key == null)
        {
            _key = await _keys.ResolveAsync(dbPath, ct).ConfigureAwait(false);
            if (_key == null)
            {
                _connected = false;
                SetState(MsgNoKey);
                return;
            }
        }

        HistoryRow? fired;
        ContentRow? content;
        try
        {
            (fired, content) = ReadOnce(dbPath, _key);
        }
        catch (SqliteException ex)
        {
            HandleDbError(MasterDb.Classify(ex), ex);
            return;
        }
        catch (MasterDbSchemaException)
        {
            _connected = false;
            SetState(MsgSchema);
            return;
        }
        catch (IOException ex)
        {
            _connected = false;
            Notify($"データベース読み取りエラー ({ex.GetType().Name})");
            return;
        }

        // 読み取り成功: シグネチャを確定する
        _lastSignature = signature;
        _connected = true;
        _notADbCount = 0;
        SetState(MsgConnected);

        if (fired == null) return;

        var track = ContentNormalizer.Build(fired.ContentId, content);
        var hasSpotify = ContentNormalizer.TryGetSpotifyTrackId(content?.FolderPath, out var spotifyId);
        if (track.Title == null && hasSpotify)
        {
            var title = await _spotify.GetTitleAsync(spotifyId, ct).ConfigureAwait(false);
            track = track with { Title = title };
        }

        if (track.Title == null)
        {
            Notify(hasSpotify ? MsgNoTitleSpotify : MsgNoTitle);
            return;
        }

        TrackReceived?.Invoke(track);
    }

    /// <summary>1 回の読み取り。open → BEGIN → クエリ → COMMIT → close。</summary>
    private (HistoryRow? Fired, ContentRow? Content) ReadOnce(string dbPath, string key)
    {
        using var db = MasterDb.Open(dbPath, key);
        db.BeginRead();

        var (maxRowid, maxId) = db.GetMax();
        string? idAtCursor = null;
        List<HistoryRow> newRows = [];
        if (_cursor.Rowid is long cursorRowid)
        {
            idAtCursor = db.GetIdAt(cursorRowid);
            newRows = db.GetRowsAfter(cursorRowid);
        }

        var fired = _cursor.Observe(maxRowid, maxId, idAtCursor, newRows);
        ContentRow? content = null;
        if (fired?.ContentId != null) content = db.GetContent(fired.ContentId);

        db.Commit();
        return (fired, content);
    }

    private void HandleDbError(DbErrorKind kind, SqliteException ex)
    {
        _connected = false;
        switch (kind)
        {
            case DbErrorKind.NotADb:
                if (++_notADbCount >= NotADbLimit)
                {
                    // 鍵が使えない。キャッシュを捨てて解決し直す
                    _notADbCount = 0;
                    _key = null;
                    _keys.Invalidate();
                }
                SetState(MsgBadKey);
                break;
            case DbErrorKind.Busy:
                SetState(MsgBusy);
                break;
            default:
                Notify($"データベース読み取りエラー ({ex.GetType().Name})");
                break;
        }
    }

    private static string Signature(string dbPath, string walPath) => $"{FileSig(dbPath)}|{FileSig(walPath)}";

    private static string FileSig(string path)
    {
        try
        {
            var fi = new FileInfo(path);
            if (!fi.Exists) return "-";
            return $"{new DateTimeOffset(fi.LastWriteTimeUtc).ToUnixTimeMilliseconds()}:{fi.Length}";
        }
        catch
        {
            return "-";
        }
    }

    /// <summary>接続できない状態へ。カーソルも捨てる（再接続時に基準を取り直し、古い曲を送らない）。</summary>
    private void Disconnect(string message)
    {
        _connected = false;
        _lastSignature = null;
        _cursor.Reset();
        SetState(message);
    }

    /// <summary>状態の通知。直前と同じ文言なら出さない。</summary>
    private void SetState(string message)
    {
        if (message == _lastState) return;
        _lastState = message;
        StatusChanged?.Invoke(message);
    }

    /// <summary>出来事の通知。毎回出す。</summary>
    private void Notify(string message)
    {
        _lastState = null;
        StatusChanged?.Invoke(message);
    }

    public async ValueTask DisposeAsync()
    {
        var cts = _cts;
        var loop = _loop;
        _cts = null;
        _loop = null;
        if (cts != null)
        {
            try { cts.Cancel(); } catch (ObjectDisposedException) { }
        }
        if (loop != null)
        {
            try { await loop.ConfigureAwait(false); } catch { }
        }
        cts?.Dispose();
    }
}
