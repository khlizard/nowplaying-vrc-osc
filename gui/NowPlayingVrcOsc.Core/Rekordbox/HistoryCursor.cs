using System.Globalization;

namespace NowPlayingVrcOsc.Core.Rekordbox;

/// <summary>djmdSongHistory の 1 行（カーソル判定に必要な分だけ）。</summary>
internal sealed record HistoryRow(long RowId, string? Id, bool Deleted, string? CreatedAt, string? ContentId);

/// <summary>
/// 履歴テーブルの rowid カーソル（NP cursor.py の移植）。メモリ上のみで保持する。
/// 初回観測は基準位置を置くだけで発火しない（起動時の曲は送らない）。
/// </summary>
internal sealed class HistoryCursor
{
    public long? Rowid { get; private set; }
    public string? EntryId { get; private set; }

    public bool HasCursor => Rowid.HasValue;

    public void Reset()
    {
        Rowid = null;
        EntryId = null;
    }

    /// <summary>
    /// スナップショットを観測し、通知すべき行があれば返す。
    /// </summary>
    /// <param name="maxRowid">最大 rowid（行が無ければ 0）</param>
    /// <param name="maxId">最大 rowid の行の ID</param>
    /// <param name="idAtCursor">カーソル位置 rowid の行の ID（行が無ければ null）</param>
    /// <param name="newRows">rowid &gt; カーソル の行</param>
    public HistoryRow? Observe(long maxRowid, string? maxId, string? idAtCursor, IReadOnlyList<HistoryRow> newRows)
    {
        if (Rowid is not long cursor)
        {
            Set(maxRowid, maxId);
            return null;
        }

        if (maxRowid < cursor || idAtCursor != EntryId)
        {
            Set(maxRowid, maxId);
            return null;
        }

        if (newRows.Count == 0) return null;

        Set(maxRowid, maxId);

        HistoryRow? best = null;
        DateTimeOffset bestTime = default;
        foreach (var row in newRows)
        {
            if (row.Deleted) continue;
            var t = ParseCreatedAt(row.CreatedAt) ?? DateTimeOffset.MinValue;
            if (best == null || t > bestTime || (t == bestTime && row.RowId > best.RowId))
            {
                best = row;
                bestTime = t;
            }
        }
        return best;
    }

    private void Set(long rowid, string? id)
    {
        Rowid = rowid;
        EntryId = id;
    }

    internal static DateTimeOffset? ParseCreatedAt(string? s)
    {
        if (string.IsNullOrWhiteSpace(s)) return null;
        return DateTimeOffset.TryParseExact(s.Trim(), "yyyy-MM-dd HH:mm:ss.fff zzz",
            CultureInfo.InvariantCulture, DateTimeStyles.AssumeUniversal, out var t)
            ? t.ToUniversalTime()
            : null;
    }
}
