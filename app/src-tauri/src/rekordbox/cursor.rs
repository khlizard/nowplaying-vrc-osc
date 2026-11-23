//! 履歴テーブルの rowid カーソル（NP cursor.py の移植。C# 版 `HistoryCursor`）。
//! メモリ上のみで保持する。初回観測は基準位置を置くだけで発火しない（起動時の曲は送らない）。

use chrono::{DateTime, Utc};

/// djmdSongHistory の 1 行（カーソル判定に必要な分だけ）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryRow {
    pub row_id: i64,
    pub id: Option<String>,
    pub deleted: bool,
    pub created_at: Option<String>,
    pub content_id: Option<String>,
}

#[derive(Debug, Default)]
pub struct HistoryCursor {
    pub rowid: Option<i64>,
    pub entry_id: Option<String>,
}

impl HistoryCursor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_cursor(&self) -> bool {
        self.rowid.is_some()
    }

    pub fn reset(&mut self) {
        self.rowid = None;
        self.entry_id = None;
    }

    /// スナップショットを観測し、通知すべき行があれば返す。
    ///
    /// - `max_rowid`: 最大 rowid（行が無ければ 0）
    /// - `max_id`: 最大 rowid の行の ID
    /// - `id_at_cursor`: カーソル位置 rowid の行の ID（行が無ければ None）
    /// - `new_rows`: rowid > カーソル の行
    pub fn observe<'a>(
        &mut self,
        max_rowid: i64,
        max_id: Option<&str>,
        id_at_cursor: Option<&str>,
        new_rows: &'a [HistoryRow],
    ) -> Option<&'a HistoryRow> {
        let Some(cursor) = self.rowid else {
            self.set(max_rowid, max_id);
            return None;
        };

        if max_rowid < cursor || id_at_cursor != self.entry_id.as_deref() {
            self.set(max_rowid, max_id);
            return None;
        }

        if new_rows.is_empty() {
            return None;
        }

        self.set(max_rowid, max_id);

        let mut best: Option<(&HistoryRow, DateTime<Utc>)> = None;
        for row in new_rows {
            if row.deleted {
                continue;
            }
            let t = parse_created_at(row.created_at.as_deref()).unwrap_or(DateTime::<Utc>::MIN_UTC);
            let better = match best {
                None => true,
                Some((b, bt)) => t > bt || (t == bt && row.row_id > b.row_id),
            };
            if better {
                best = Some((row, t));
            }
        }
        best.map(|(r, _)| r)
    }

    fn set(&mut self, rowid: i64, id: Option<&str>) {
        self.rowid = Some(rowid);
        self.entry_id = id.map(str::to_string);
    }
}

/// `yyyy-MM-dd HH:mm:ss.fff zzz` 形式（例 `2026-01-01 12:00:10.000 +00:00`）を UTC にする。
pub fn parse_created_at(s: Option<&str>) -> Option<DateTime<Utc>> {
    let s = s?.trim();
    if s.is_empty() {
        return None;
    }
    DateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.3f %:z")
        .ok()
        .map(|t| t.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(rid: i64, id: &str, del: bool, created: Option<&str>) -> HistoryRow {
        HistoryRow {
            row_id: rid,
            id: Some(id.to_string()),
            deleted: del,
            created_at: created.map(str::to_string),
            content_id: Some(format!("c{rid}")),
        }
    }

    fn baselined() -> HistoryCursor {
        let mut c = HistoryCursor::new();
        assert!(c.observe(5, Some("e5"), None, &[]).is_none());
        c
    }

    #[test]
    fn 初回観測は基準を置くだけで発火しない() {
        let mut c = HistoryCursor::new();
        assert!(c.observe(5, Some("e5"), None, &[]).is_none());
        assert_eq!(c.rowid, Some(5));
        assert_eq!(c.entry_id.as_deref(), Some("e5"));
    }

    #[test]
    fn 新しい行は一度だけ発火する() {
        let mut c = baselined();
        let rows = [row(6, "e6", false, None)];
        let fired = c.observe(6, Some("e6"), Some("e5"), &rows);
        assert_eq!(fired.map(|r| r.row_id), Some(6));
        assert!(c.observe(6, Some("e6"), Some("e6"), &[]).is_none());
    }

    #[test]
    fn 削除済みの行だけなら発火しないが進む() {
        let mut c = baselined();
        let rows = [row(6, "e6", true, None), row(7, "e7", true, None)];
        assert!(c.observe(7, Some("e7"), Some("e5"), &rows).is_none());
        assert_eq!(c.rowid, Some(7));
    }

    #[test]
    fn 最大_rowid_が減ったらリセットして発火しない() {
        let mut c = baselined();
        assert!(c.observe(3, Some("e3"), None, &[]).is_none());
        assert_eq!(c.rowid, Some(3));
    }

    #[test]
    fn カーソル位置の_id_が変わったらリセットして発火しない() {
        let mut c = baselined();
        let rows = [row(6, "e6", false, None), row(9, "e9", false, None)];
        assert!(c.observe(9, Some("e9"), Some("other"), &rows).is_none());
        assert_eq!(c.rowid, Some(9));
        assert_eq!(c.entry_id.as_deref(), Some("e9"));
    }

    #[test]
    fn created_at_が逆転していたら最新の時刻を選ぶ() {
        let mut c = baselined();
        let rows = [
            row(6, "e6", false, Some("2026-01-01 12:00:10.000 +00:00")),
            row(7, "e7", false, Some("2026-01-01 12:00:05.000 +00:00")),
        ];
        let fired = c.observe(7, Some("e7"), Some("e5"), &rows);
        assert_eq!(fired.map(|r| r.row_id), Some(6));
    }

    #[test]
    fn 解釈できない_created_at_は最古として扱い_次に_rowid() {
        let mut c = baselined();
        let rows = [
            row(6, "e6", false, Some("2026-01-01 12:00:10.000 +00:00")),
            row(7, "e7", false, Some("garbage")),
            row(8, "e8", false, None),
        ];
        let fired = c.observe(8, Some("e8"), Some("e5"), &rows);
        assert_eq!(fired.map(|r| r.row_id), Some(6));

        let mut c2 = baselined();
        let rows2 = [row(6, "e6", false, None), row(7, "e7", false, None)];
        let fired2 = c2.observe(7, Some("e7"), Some("e5"), &rows2);
        assert_eq!(fired2.map(|r| r.row_id), Some(7));
    }

    #[test]
    fn 空テーブルの後に挿入されたら発火する() {
        let mut c = HistoryCursor::new();
        assert!(c.observe(0, None, None, &[]).is_none());
        let rows = [row(1, "e1", false, None)];
        let fired = c.observe(1, Some("e1"), None, &rows);
        assert_eq!(fired.map(|r| r.row_id), Some(1));
    }

    #[test]
    fn リセットでカーソルを捨てる() {
        let mut c = baselined();
        assert!(c.has_cursor());
        c.reset();
        assert!(!c.has_cursor());
        assert_eq!(c.entry_id, None);
    }

    #[test]
    fn created_at_の解釈はオフセットを_utc_に直す() {
        let a = parse_created_at(Some("2026-01-01 12:00:00.000 +09:00")).unwrap();
        let b = parse_created_at(Some("2026-01-01 03:00:00.000 +00:00")).unwrap();
        assert_eq!(a, b);
        assert_eq!(parse_created_at(Some("  ")), None);
        assert_eq!(parse_created_at(None), None);
    }
}
