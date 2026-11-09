using System.Globalization;
using System.Text;
using System.Text.RegularExpressions;
using Microsoft.Data.Sqlite;

namespace NowPlayingVrcOsc.Core.Rekordbox;

public enum KeyCheckResult { Ok, WrongKey, InvalidFormat, DbNotFound, Busy, Error }

internal enum DbErrorKind { NotADb, Busy, Other }

/// <summary>master.db のスキーマが想定と異なる（必須列が無い、WITHOUT ROWID 等）。</summary>
internal sealed class MasterDbSchemaException : Exception
{
    public MasterDbSchemaException(string message) : base(message) { }
}

/// <summary>
/// 暗号化された rekordbox の master.db を読み取り専用で開く唯一の場所。
/// 書き込み系の SQL・PRAGMA（rekey / journal_mode / wal_checkpoint 等）は一切発行しない。
/// 鍵は例外メッセージやログに出さない。
/// </summary>
internal sealed partial class MasterDb : IDisposable
{
    [GeneratedRegex(@"^[0-9a-fA-F]{64}\z")]
    private static partial Regex KeyFormatRegex();

    private readonly SqliteConnection _conn;
    private bool _inTransaction;
    private Dictionary<string, HashSet<string>>? _schema;
    private bool _hasDeleted, _hasCreatedAt;

    private MasterDb(SqliteConnection conn) => _conn = conn;

    public static bool IsValidKeyFormat(string? key) => key != null && KeyFormatRegex().IsMatch(key);

    /// <summary>
    /// 開く。鍵が違うと SqliteException（SQLITE_NOTADB）。
    /// プーリングは無効（プールされた接続はファイルを掴み続け -wal/-shm を残すため）。
    /// </summary>
    public static MasterDb Open(string path, string key, bool immutable = false)
    {
        if (!IsValidKeyFormat(key)) throw new ArgumentException("鍵の形式が不正です。", nameof(key));
        SqliteInit.EnsureInitialized();

        var full = Path.GetFullPath(path).Replace('\\', '/');
        var uri = "file:///" + Uri.EscapeDataString(full).Replace("%2F", "/").Replace("%3A", ":")
                  + "?mode=ro" + (immutable ? "&immutable=1" : "");
        var conn = new SqliteConnection(new SqliteConnectionStringBuilder
        {
            DataSource = uri,
            Mode = SqliteOpenMode.ReadOnly,
            Pooling = false,
            DefaultTimeout = 5,
        }.ToString());
        try
        {
            conn.Open();
            using var cmd = conn.CreateCommand();
            cmd.CommandText = "PRAGMA busy_timeout=5000;";
            cmd.ExecuteNonQuery();
            cmd.CommandText = "PRAGMA cipher='sqlcipher'; PRAGMA legacy=4; PRAGMA key='" + key.Replace("'", "''") + "';";
            cmd.ExecuteNonQuery();
            // 鍵が違うとここで SQLITE_NOTADB
            cmd.CommandText = "SELECT count(*) FROM sqlite_master";
            cmd.ExecuteScalar();
            return new MasterDb(conn);
        }
        catch
        {
            conn.Dispose();
            throw;
        }
    }

    public static DbErrorKind Classify(SqliteException ex) => (ex.SqliteErrorCode & 0xFF) switch
    {
        26 => DbErrorKind.NotADb,
        5 or 6 => DbErrorKind.Busy,
        _ => DbErrorKind.Other,
    };

    /// <summary>
    /// 鍵で実 DB を開けるか 1 回だけ確認する。-wal が無いときは immutable=1 で開く（ファイルを作らない）。
    /// </summary>
    public static KeyCheckResult VerifyKey(string dbPath, string key)
    {
        if (!IsValidKeyFormat(key)) return KeyCheckResult.InvalidFormat;
        if (!File.Exists(dbPath)) return KeyCheckResult.DbNotFound;
        try
        {
            var immutable = !File.Exists(dbPath + "-wal");
            using var db = Open(dbPath, key, immutable);
            return KeyCheckResult.Ok;
        }
        catch (SqliteException ex)
        {
            return Classify(ex) switch
            {
                DbErrorKind.NotADb => KeyCheckResult.WrongKey,
                DbErrorKind.Busy => KeyCheckResult.Busy,
                _ => KeyCheckResult.Error,
            };
        }
        catch
        {
            return KeyCheckResult.Error;
        }
    }

    // ---- 読み取りトランザクション ----

    /// <summary>BEGIN してスキーマを確認する。全読み取りを同じ WAL 状態で行うため。</summary>
    public void BeginRead()
    {
        Exec("BEGIN");
        _inTransaction = true;
        _schema = LoadSchema();
        CheckSchema();
    }

    public void Commit()
    {
        if (!_inTransaction) return;
        _inTransaction = false;
        Exec("COMMIT");
    }

    private void Exec(string sql)
    {
        using var cmd = _conn.CreateCommand();
        cmd.CommandText = sql;
        cmd.ExecuteNonQuery();
    }

    private Dictionary<string, HashSet<string>> LoadSchema()
    {
        var schema = new Dictionary<string, HashSet<string>>(StringComparer.Ordinal);
        foreach (var table in new[] { "djmdSongHistory", "djmdContent", "djmdArtist", "djmdAlbum", "djmdLabel" })
        {
            var cols = new HashSet<string>(StringComparer.Ordinal);
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = $"PRAGMA table_info({table})";
            using var r = cmd.ExecuteReader();
            while (r.Read()) cols.Add(r.GetString(1));
            schema[table] = cols;
        }
        return schema;
    }

    private void CheckSchema()
    {
        var s = _schema!;
        if (!s["djmdSongHistory"].Contains("ID") || !s["djmdSongHistory"].Contains("ContentID") ||
            !s["djmdContent"].Contains("ID"))
            throw new MasterDbSchemaException("必須の列がありません。");

        using (var cmd = _conn.CreateCommand())
        {
            cmd.CommandText = "SELECT sql FROM sqlite_master WHERE type='table' AND name='djmdSongHistory'";
            var sql = cmd.ExecuteScalar() as string;
            if (sql != null && sql.Contains("WITHOUT ROWID", StringComparison.OrdinalIgnoreCase))
                throw new MasterDbSchemaException("djmdSongHistory が WITHOUT ROWID です。");
        }

        _hasDeleted = s["djmdSongHistory"].Contains("rb_local_deleted");
        _hasCreatedAt = s["djmdSongHistory"].Contains("created_at");
    }

    /// <summary>最大 rowid とその行の ID。行が無ければ (0, null)。</summary>
    public (long RowId, string? Id) GetMax()
    {
        using var cmd = _conn.CreateCommand();
        cmd.CommandText = "SELECT rowid, ID FROM djmdSongHistory ORDER BY rowid DESC LIMIT 1";
        using var r = cmd.ExecuteReader();
        return r.Read() ? (r.GetInt64(0), Text(r, 1)) : (0, null);
    }

    /// <summary>指定 rowid の行の ID。行が無ければ null。</summary>
    public string? GetIdAt(long rowid)
    {
        using var cmd = _conn.CreateCommand();
        cmd.CommandText = "SELECT ID FROM djmdSongHistory WHERE rowid = $r";
        cmd.Parameters.AddWithValue("$r", rowid);
        using var r = cmd.ExecuteReader();
        return r.Read() ? Text(r, 0) : null;
    }

    /// <summary>rowid &gt; cursor の行（rowid 昇順）。</summary>
    public List<HistoryRow> GetRowsAfter(long cursor)
    {
        var del = _hasDeleted ? "COALESCE(rb_local_deleted, 0) = 1" : "0";
        var created = _hasCreatedAt ? "created_at" : "NULL";
        using var cmd = _conn.CreateCommand();
        cmd.CommandText =
            $"SELECT rowid, ID, {del}, {created}, ContentID FROM djmdSongHistory WHERE rowid > $c ORDER BY rowid ASC";
        cmd.Parameters.AddWithValue("$c", cursor);
        var list = new List<HistoryRow>();
        using var r = cmd.ExecuteReader();
        while (r.Read())
            list.Add(new HistoryRow(r.GetInt64(0), Text(r, 1), r.GetInt64(2) == 1, Text(r, 3), Text(r, 4)));
        return list;
    }

    /// <summary>ContentID から曲情報を引く。見つからなければ null。</summary>
    public ContentRow? GetContent(string contentId)
    {
        var cols = _schema!["djmdContent"];
        string Col(string name) => cols.Contains(name) ? name : "NULL";

        string? title, folder, comment;
        object? artistKey, albumKey, labelKey;

        using (var cmd = _conn.CreateCommand())
        {
            cmd.CommandText =
                $"SELECT {Col("Title")}, {Col("FolderPath")}, {Col("Commnt")}, {Col("ArtistID")}, {Col("AlbumID")}, {Col("LabelID")} " +
                "FROM djmdContent WHERE ID = $id LIMIT 1";
            cmd.Parameters.AddWithValue("$id", contentId);
            using var r = cmd.ExecuteReader();
            if (!r.Read()) return null;
            title = Text(r, 0);
            folder = Text(r, 1);
            comment = Text(r, 2);
            artistKey = r.IsDBNull(3) ? null : r.GetValue(3);
            albumKey = r.IsDBNull(4) ? null : r.GetValue(4);
            labelKey = r.IsDBNull(5) ? null : r.GetValue(5);
        }

        return new ContentRow(title, folder, comment,
            LookupName("djmdArtist", artistKey),
            LookupName("djmdAlbum", albumKey),
            LookupName("djmdLabel", labelKey));
    }

    private string? LookupName(string table, object? id)
    {
        if (id == null) return null;
        var cols = _schema![table];
        if (!cols.Contains("ID") || !cols.Contains("Name")) return null;
        using var cmd = _conn.CreateCommand();
        cmd.CommandText = $"SELECT Name FROM {table} WHERE ID = $id LIMIT 1";
        cmd.Parameters.AddWithValue("$id", id);
        using var r = cmd.ExecuteReader();
        return r.Read() ? Text(r, 0) : null;
    }

    /// <summary>
    /// 値を文字列で読む。NULL は null、BLOB は UTF-8 置換デコード、数値は文字列化。
    /// 不正な UTF-8 でも例外にしない。
    /// </summary>
    private static string? Text(SqliteDataReader r, int i)
    {
        if (r.IsDBNull(i)) return null;
        return r.GetValue(i) switch
        {
            string s => s,
            byte[] b => Encoding.UTF8.GetString(b),
            var v => Convert.ToString(v, CultureInfo.InvariantCulture),
        };
    }

    /// <summary>テスト用: 単一値の SELECT を実行して文字列で返す。</summary>
    internal string? QueryScalarText(string selectSql)
    {
        using var cmd = _conn.CreateCommand();
        cmd.CommandText = selectSql;
        using var r = cmd.ExecuteReader();
        return r.Read() ? Text(r, 0) : null;
    }

    public void Dispose()
    {
        try
        {
            if (_inTransaction)
            {
                _inTransaction = false;
                Exec("ROLLBACK");
            }
        }
        catch
        {
            // 閉じる処理中の失敗は無視
        }
        _conn.Dispose();
    }
}

internal static class SqliteInit
{
    private static readonly object Gate = new();
    private static bool _done;

    public static void EnsureInitialized()
    {
        lock (Gate)
        {
            if (_done) return;
            SQLitePCL.Batteries_V2.Init();
            _done = true;
        }
    }
}
