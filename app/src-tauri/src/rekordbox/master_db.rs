//! 暗号化された rekordbox の master.db を読み取り専用で開く唯一の場所（C# 版 `MasterDb`）。
//! 書き込み系の SQL・PRAGMA（rekey / journal_mode / wal_checkpoint 等）は一切発行しない。
//! 鍵はエラーやログに出さない（エラーには種別とコードだけを持たせる）。

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::types::{Value, ValueRef};
use rusqlite::{params, Connection, OpenFlags};

use super::cursor::HistoryRow;
use super::normalize::ContentRow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyCheckResult {
    Ok,
    WrongKey,
    InvalidFormat,
    DbNotFound,
    Busy,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DbErrorKind {
    NotADb,
    Busy,
    Other,
}

/// master.db 読み取りの失敗。SQL 文や鍵を含めない。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DbError {
    /// SQLite のエラー。`code` は拡張結果コード（ない場合は 0）。
    Sqlite { kind: DbErrorKind, code: i32 },
    /// スキーマが想定と異なる（必須列が無い、WITHOUT ROWID 等）。
    Schema(&'static str),
    /// 鍵の形式が不正。
    InvalidKeyFormat,
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DbError::Sqlite { kind, code } => write!(f, "SQLite エラー ({kind:?}, code={code})"),
            DbError::Schema(m) => write!(f, "master.db の構成が想定と異なります: {m}"),
            DbError::InvalidKeyFormat => f.write_str("鍵の形式が不正です"),
        }
    }
}

impl std::error::Error for DbError {}

impl DbError {
    fn from_rusqlite(e: &rusqlite::Error) -> DbError {
        let code = match e {
            rusqlite::Error::SqliteFailure(f, _) => f.extended_code,
            rusqlite::Error::SqlInputError { error, .. } => error.extended_code,
            _ => 0,
        };
        let kind = if code == 0 {
            DbErrorKind::Other
        } else {
            classify_code(code)
        };
        DbError::Sqlite { kind, code }
    }

    /// SQLite エラーの種別。それ以外は None。
    pub fn kind(&self) -> Option<DbErrorKind> {
        match self {
            DbError::Sqlite { kind, .. } => Some(*kind),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        DbError::from_rusqlite(&e)
    }
}

/// 26 = NotADb（鍵違い）、5/6 = Busy、それ以外 = Other。
fn classify_code(extended_code: i32) -> DbErrorKind {
    match extended_code & 0xFF {
        26 => DbErrorKind::NotADb,
        5 | 6 => DbErrorKind::Busy,
        _ => DbErrorKind::Other,
    }
}

/// 64 桁の 16 進か。
pub fn is_valid_key_format(key: &str) -> bool {
    key.len() == 64 && key.bytes().all(|b| b.is_ascii_hexdigit())
}

/// `<db>-wal` のパス。
pub fn wal_path(db_path: &Path) -> PathBuf {
    let mut s = db_path.as_os_str().to_owned();
    s.push("-wal");
    PathBuf::from(s)
}

/// 鍵で実 DB を開けるか 1 回だけ確認する。-wal が無いときは immutable=1 で開く（ファイルを作らない）。
pub fn verify_key(db_path: &Path, key: &str) -> KeyCheckResult {
    if !is_valid_key_format(key) {
        return KeyCheckResult::InvalidFormat;
    }
    if !db_path.is_file() {
        return KeyCheckResult::DbNotFound;
    }
    let immutable = !wal_path(db_path).is_file();
    match MasterDb::open(db_path, key, immutable) {
        Ok(_) => KeyCheckResult::Ok,
        Err(e) => match e.kind() {
            Some(DbErrorKind::NotADb) => KeyCheckResult::WrongKey,
            Some(DbErrorKind::Busy) => KeyCheckResult::Busy,
            _ => KeyCheckResult::Error,
        },
    }
}

pub struct MasterDb {
    conn: Connection,
    in_transaction: bool,
    schema: HashMap<&'static str, HashSet<String>>,
    has_deleted: bool,
    has_created_at: bool,
}

const TABLES: [&str; 5] = [
    "djmdSongHistory",
    "djmdContent",
    "djmdArtist",
    "djmdAlbum",
    "djmdLabel",
];

impl MasterDb {
    /// 開く。鍵が違うと NotADb（最初のクエリで判明する）。
    pub fn open(path: &Path, key: &str, immutable: bool) -> Result<MasterDb, DbError> {
        if !is_valid_key_format(key) {
            return Err(DbError::InvalidKeyFormat);
        }
        let abs = std::path::absolute(path).map_err(|_| DbError::Sqlite {
            kind: DbErrorKind::Other,
            code: 0,
        })?;
        let mut uri = url::Url::from_file_path(&abs).map_err(|_| DbError::Sqlite {
            kind: DbErrorKind::Other,
            code: 0,
        })?;
        uri.set_query(Some(if immutable {
            "mode=ro&immutable=1"
        } else {
            "mode=ro"
        }));

        let conn = Connection::open_with_flags(
            uri.as_str(),
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.busy_timeout(Duration::from_millis(5000))?;
        conn.execute_batch(&format!(
            "PRAGMA cipher='sqlcipher'; PRAGMA legacy=4; PRAGMA key='{key}';"
        ))?;
        // 鍵が違うとここで NotADb
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
            r.get::<_, i64>(0)
        })?;
        Ok(MasterDb {
            conn,
            in_transaction: false,
            schema: HashMap::new(),
            has_deleted: false,
            has_created_at: false,
        })
    }

    // ---- 読み取りトランザクション ----

    /// BEGIN してスキーマを確認する。全読み取りを同じ WAL 状態で行うため。
    pub fn begin_read(&mut self) -> Result<(), DbError> {
        self.conn.execute_batch("BEGIN")?;
        self.in_transaction = true;
        self.schema = self.load_schema()?;
        self.check_schema()
    }

    pub fn commit(&mut self) -> Result<(), DbError> {
        if !self.in_transaction {
            return Ok(());
        }
        self.in_transaction = false;
        self.conn.execute_batch("COMMIT")?;
        Ok(())
    }

    fn load_schema(&self) -> Result<HashMap<&'static str, HashSet<String>>, DbError> {
        let mut schema = HashMap::new();
        for table in TABLES {
            let mut cols = HashSet::new();
            let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
            let mut rows = stmt.query([])?;
            while let Some(r) = rows.next()? {
                cols.insert(r.get::<_, String>(1)?);
            }
            schema.insert(table, cols);
        }
        Ok(schema)
    }

    fn check_schema(&mut self) -> Result<(), DbError> {
        let history = &self.schema["djmdSongHistory"];
        if !history.contains("ID")
            || !history.contains("ContentID")
            || !self.schema["djmdContent"].contains("ID")
        {
            return Err(DbError::Schema("必須の列がありません"));
        }

        let sql: Option<String> = self
            .conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='djmdSongHistory'",
                [],
                |r| r.get(0),
            )
            .ok();
        if sql.is_some_and(|s| s.to_ascii_uppercase().contains("WITHOUT ROWID")) {
            return Err(DbError::Schema("djmdSongHistory が WITHOUT ROWID です"));
        }

        let history = &self.schema["djmdSongHistory"];
        self.has_deleted = history.contains("rb_local_deleted");
        self.has_created_at = history.contains("created_at");
        Ok(())
    }

    /// 最大 rowid とその行の ID。行が無ければ (0, None)。
    pub fn get_max(&self) -> Result<(i64, Option<String>), DbError> {
        let mut stmt = self
            .conn
            .prepare("SELECT rowid, ID FROM djmdSongHistory ORDER BY rowid DESC LIMIT 1")?;
        let mut rows = stmt.query([])?;
        match rows.next()? {
            Some(r) => Ok((r.get::<_, i64>(0)?, text(r.get_ref(1)?))),
            None => Ok((0, None)),
        }
    }

    /// 指定 rowid の行の ID。行が無ければ None。
    pub fn get_id_at(&self, rowid: i64) -> Result<Option<String>, DbError> {
        let mut stmt = self
            .conn
            .prepare("SELECT ID FROM djmdSongHistory WHERE rowid = ?1")?;
        let mut rows = stmt.query(params![rowid])?;
        match rows.next()? {
            Some(r) => Ok(text(r.get_ref(0)?)),
            None => Ok(None),
        }
    }

    /// rowid > cursor の行（rowid 昇順）。
    pub fn get_rows_after(&self, cursor: i64) -> Result<Vec<HistoryRow>, DbError> {
        let del = if self.has_deleted {
            "COALESCE(rb_local_deleted, 0) = 1"
        } else {
            "0"
        };
        let created = if self.has_created_at {
            "created_at"
        } else {
            "NULL"
        };
        let mut stmt = self.conn.prepare(&format!(
            "SELECT rowid, ID, {del}, {created}, ContentID FROM djmdSongHistory WHERE rowid > ?1 ORDER BY rowid ASC"
        ))?;
        let mut rows = stmt.query(params![cursor])?;
        let mut list = Vec::new();
        while let Some(r) = rows.next()? {
            list.push(HistoryRow {
                row_id: r.get::<_, i64>(0)?,
                id: text(r.get_ref(1)?),
                deleted: r.get::<_, i64>(2)? == 1,
                created_at: text(r.get_ref(3)?),
                content_id: text(r.get_ref(4)?),
            });
        }
        Ok(list)
    }

    /// ContentID から曲情報を引く。見つからなければ None。
    pub fn get_content(&self, content_id: &str) -> Result<Option<ContentRow>, DbError> {
        let cols = &self.schema["djmdContent"];
        let col = |name: &'static str| if cols.contains(name) { name } else { "NULL" };

        let (title, folder, comment, artist_key, album_key, label_key);
        {
            let mut stmt = self.conn.prepare(&format!(
                "SELECT {}, {}, {}, {}, {}, {} FROM djmdContent WHERE ID = ?1 LIMIT 1",
                col("Title"),
                col("FolderPath"),
                col("Commnt"),
                col("ArtistID"),
                col("AlbumID"),
                col("LabelID"),
            ))?;
            let mut rows = stmt.query(params![content_id])?;
            let Some(r) = rows.next()? else {
                return Ok(None);
            };
            title = text(r.get_ref(0)?);
            folder = text(r.get_ref(1)?);
            comment = text(r.get_ref(2)?);
            artist_key = r.get::<_, Value>(3)?;
            album_key = r.get::<_, Value>(4)?;
            label_key = r.get::<_, Value>(5)?;
        }

        Ok(Some(ContentRow {
            title,
            folder_path: folder,
            comment,
            artist: self.lookup_name("djmdArtist", &artist_key)?,
            album: self.lookup_name("djmdAlbum", &album_key)?,
            label: self.lookup_name("djmdLabel", &label_key)?,
        }))
    }

    fn lookup_name(&self, table: &'static str, id: &Value) -> Result<Option<String>, DbError> {
        if matches!(id, Value::Null) {
            return Ok(None);
        }
        let cols = &self.schema[table];
        if !cols.contains("ID") || !cols.contains("Name") {
            return Ok(None);
        }
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT Name FROM {table} WHERE ID = ?1 LIMIT 1"))?;
        let mut rows = stmt.query(params![id])?;
        match rows.next()? {
            Some(r) => Ok(text(r.get_ref(0)?)),
            None => Ok(None),
        }
    }

    /// テスト用: 単一値の SELECT を実行して文字列で返す。
    #[cfg(test)]
    pub(crate) fn query_scalar_text(&self, select_sql: &str) -> Result<Option<String>, DbError> {
        let mut stmt = self.conn.prepare(select_sql)?;
        let mut rows = stmt.query([])?;
        match rows.next()? {
            Some(r) => Ok(text(r.get_ref(0)?)),
            None => Ok(None),
        }
    }
}

/// 値を文字列で読む。NULL は None、BLOB・TEXT は UTF-8 置換デコード、数値は文字列化。
/// 不正な UTF-8 でも失敗にしない。
fn text(v: ValueRef<'_>) -> Option<String> {
    match v {
        ValueRef::Null => None,
        ValueRef::Integer(i) => Some(i.to_string()),
        ValueRef::Real(f) => Some(f.to_string()),
        ValueRef::Text(b) | ValueRef::Blob(b) => Some(String::from_utf8_lossy(b).into_owned()),
    }
}

impl Drop for MasterDb {
    fn drop(&mut self) {
        if self.in_transaction {
            self.in_transaction = false;
            // 閉じる処理中の失敗は無視
            let _ = self.conn.execute_batch("ROLLBACK");
        }
    }
}

/// テスト用の暗号化 master.db。実際の rekordbox DB は使わず、同じ SQLite3MC で最小スキーマを作る。
/// ライター接続を開いたままにするので -wal が残り、WAL のみの行も読める。
#[cfg(test)]
pub(crate) mod fixture {
    use super::*;
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    pub struct FixtureDb {
        pub dir: tempfile::TempDir,
        pub path: PathBuf,
        pub key: String,
        writer: Option<Connection>,
    }

    fn random_hex_key() -> String {
        let state = RandomState::new();
        let mut key = String::new();
        for i in 0..4u64 {
            let mut h = state.build_hasher();
            h.write_u64(i);
            key.push_str(&format!("{:016x}", h.finish()));
        }
        key
    }

    /// 鍵を 1 文字変えた別の鍵（形式は有効）。
    pub fn other_key(key: &str) -> String {
        if key.starts_with('a') {
            format!("b{}", &key[1..])
        } else {
            format!("a{}", &key[1..])
        }
    }

    pub fn q(s: Option<&str>) -> String {
        match s {
            None => "NULL".to_string(),
            Some(s) => format!("'{}'", s.replace('\'', "''")),
        }
    }

    impl FixtureDb {
        pub fn new() -> FixtureDb {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("master.db");
            let key = random_hex_key();
            let writer = Connection::open(&path).unwrap();
            let fx = FixtureDb {
                dir,
                path,
                key,
                writer: Some(writer),
            };
            fx.exec(&format!(
                "PRAGMA cipher='sqlcipher'; PRAGMA legacy=4; PRAGMA key='{}';",
                fx.key
            ));
            fx.exec("PRAGMA journal_mode=WAL;");
            fx.exec("PRAGMA wal_autocheckpoint=0;");
            fx.exec(
                "CREATE TABLE djmdSongHistory(ID VARCHAR(255) PRIMARY KEY, HistoryID VARCHAR(255), ContentID VARCHAR(255), \
                 rb_local_deleted INTEGER DEFAULT 0, created_at TEXT);\
                 CREATE TABLE djmdContent(ID VARCHAR(255) PRIMARY KEY, FolderPath TEXT, Title TEXT, ArtistID VARCHAR(255), \
                 AlbumID VARCHAR(255), LabelID VARCHAR(255), Commnt TEXT);\
                 CREATE TABLE djmdArtist(ID VARCHAR(255) PRIMARY KEY, Name TEXT);\
                 CREATE TABLE djmdAlbum(ID VARCHAR(255) PRIMARY KEY, Name TEXT);\
                 CREATE TABLE djmdLabel(ID VARCHAR(255) PRIMARY KEY, Name TEXT);",
            );
            fx
        }

        pub fn wal_path(&self) -> PathBuf {
            wal_path(&self.path)
        }

        pub fn shm_path(&self) -> PathBuf {
            let mut s = self.path.as_os_str().to_owned();
            s.push("-shm");
            PathBuf::from(s)
        }

        pub fn exec(&self, sql: &str) {
            self.writer
                .as_ref()
                .expect("ライターは閉じています")
                .execute_batch(sql)
                .unwrap();
        }

        pub fn add_content(&self, id: &str, title: Option<&str>) {
            self.add_content_full(id, title, "C:/music/a.mp3", None, None, None, None);
        }

        #[allow(clippy::too_many_arguments)]
        pub fn add_content_full(
            &self,
            id: &str,
            title: Option<&str>,
            folder_path: &str,
            artist: Option<&str>,
            album: Option<&str>,
            label: Option<&str>,
            comment: Option<&str>,
        ) {
            let mut ids: [Option<String>; 3] = [None, None, None];
            for (i, (prefix, table, name)) in [
                ("ar", "djmdArtist", artist),
                ("al", "djmdAlbum", album),
                ("lb", "djmdLabel", label),
            ]
            .into_iter()
            .enumerate()
            {
                if let Some(n) = name {
                    let rid = format!("{prefix}{id}");
                    self.exec(&format!(
                        "INSERT INTO {table} VALUES('{rid}', {})",
                        q(Some(n))
                    ));
                    ids[i] = Some(rid);
                }
            }
            self.exec(&format!(
                "INSERT INTO djmdContent VALUES('{id}', {}, {}, {}, {}, {}, {})",
                q(Some(folder_path)),
                q(title),
                q(ids[0].as_deref()),
                q(ids[1].as_deref()),
                q(ids[2].as_deref()),
                q(comment),
            ));
        }

        pub fn add_history(
            &self,
            id: &str,
            content_id: &str,
            deleted: bool,
            created_at: Option<&str>,
        ) {
            self.exec(&format!(
                "INSERT INTO djmdSongHistory(ID, HistoryID, ContentID, rb_local_deleted, created_at) \
                 VALUES('{id}', 'h1', '{content_id}', {}, {})",
                i32::from(deleted),
                q(created_at)
            ));
        }

        /// ライターを閉じる（最後の接続なので WAL がチェックポイントされ -wal が消える）。
        pub fn close_writer(&mut self) {
            self.writer = None;
        }

        /// ディレクトリ内のファイル名一覧（ソート済み）。
        pub fn file_names(&self) -> Vec<String> {
            let mut v: Vec<String> = std::fs::read_dir(self.dir.path())
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            v.sort();
            v
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::*;
    use super::*;

    #[test]
    fn wal_のみの行をライター接続中でも読める() {
        let fx = FixtureDb::new();
        fx.add_content_full(
            "c1",
            Some("Song One"),
            "C:/music/a.mp3",
            Some("Artist One"),
            Some("Album One"),
            Some("Label One"),
            Some("memo"),
        );
        fx.add_history("h1", "c1", false, Some("2026-01-01 00:00:00.000 +00:00"));
        assert!(fx.wal_path().is_file());

        let mut db = MasterDb::open(&fx.path, &fx.key, false).unwrap();
        db.begin_read().unwrap();
        let (rid, id) = db.get_max().unwrap();
        assert_eq!(rid, 1);
        assert_eq!(id.as_deref(), Some("h1"));
        assert_eq!(db.get_id_at(1).unwrap().as_deref(), Some("h1"));
        let rows = db.get_rows_after(0).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].content_id.as_deref(), Some("c1"));
        assert!(!rows[0].deleted);

        let c = db.get_content("c1").unwrap().unwrap();
        assert_eq!(c.title.as_deref(), Some("Song One"));
        assert_eq!(c.artist.as_deref(), Some("Artist One"));
        assert_eq!(c.album.as_deref(), Some("Album One"));
        assert_eq!(c.label.as_deref(), Some("Label One"));
        assert_eq!(c.comment.as_deref(), Some("memo"));
        assert!(db.get_content("nope").unwrap().is_none());
        db.commit().unwrap();
    }

    #[test]
    fn 鍵違いは_notadb_で_verify_key_は_wrongkey() {
        let fx = FixtureDb::new();
        let wrong = other_key(&fx.key);
        let err = MasterDb::open(&fx.path, &wrong, false).err().unwrap();
        assert_eq!(err.kind(), Some(DbErrorKind::NotADb));
        assert_eq!(verify_key(&fx.path, &wrong), KeyCheckResult::WrongKey);
        // エラーの表示に鍵を含めない
        for s in [format!("{err}"), format!("{err:?}")] {
            assert!(!s.contains(&fx.key) && !s.contains(&wrong));
        }
    }

    #[test]
    fn verify_key_の結果() {
        let fx = FixtureDb::new();
        assert_eq!(verify_key(&fx.path, &fx.key), KeyCheckResult::Ok);
        assert_eq!(verify_key(&fx.path, "xyz"), KeyCheckResult::InvalidFormat);
        assert_eq!(
            verify_key(&fx.dir.path().join("none.db"), &fx.key),
            KeyCheckResult::DbNotFound
        );
    }

    #[test]
    fn 鍵の形式判定() {
        assert!(!is_valid_key_format(&"a".repeat(63)));
        assert!(is_valid_key_format(&"aB".repeat(32)));
        assert!(!is_valid_key_format(&"g".repeat(64)));
        assert!(!is_valid_key_format(&"a".repeat(65)));
        assert!(!is_valid_key_format(&format!("{}\n", "a".repeat(64))));
    }

    #[test]
    fn wal_が無くても_verify_key_できファイルを作らない() {
        let mut fx = FixtureDb::new();
        fx.add_content("c1", Some("x"));
        fx.close_writer();
        assert!(!fx.wal_path().exists());
        assert!(!fx.shm_path().exists());
        let before = fx.file_names();

        assert_eq!(verify_key(&fx.path, &fx.key), KeyCheckResult::Ok);
        assert_eq!(
            verify_key(&fx.path, &other_key(&fx.key)),
            KeyCheckResult::WrongKey
        );

        assert!(!fx.wal_path().exists());
        assert!(!fx.shm_path().exists());
        assert_eq!(before, fx.file_names());
    }

    #[test]
    fn 不正な_utf8_でも失敗しない() {
        let fx = FixtureDb::new();
        fx.add_content("c1", Some("x"));
        fx.exec("UPDATE djmdContent SET Title = CAST(x'41e3ff42' AS TEXT) WHERE ID='c1'");
        fx.add_history("h1", "c1", false, None);

        let mut db = MasterDb::open(&fx.path, &fx.key, false).unwrap();
        db.begin_read().unwrap();
        let c = db.get_content("c1").unwrap().unwrap();
        let title = c.title.unwrap();
        assert!(title.starts_with('A'));
        assert!(title.ends_with('B'));
        assert!(db
            .query_scalar_text("SELECT Title FROM djmdContent")
            .unwrap()
            .is_some());
        db.commit().unwrap();
    }

    #[test]
    fn without_rowid_の履歴はスキーマエラー() {
        let fx = FixtureDb::new();
        fx.exec(
            "DROP TABLE djmdSongHistory; CREATE TABLE djmdSongHistory(ID TEXT PRIMARY KEY, ContentID TEXT) WITHOUT ROWID;",
        );
        let mut db = MasterDb::open(&fx.path, &fx.key, false).unwrap();
        assert!(matches!(db.begin_read(), Err(DbError::Schema(_))));
    }

    #[test]
    fn 任意の列が無ければ_null_として扱う() {
        let fx = FixtureDb::new();
        fx.exec(
            "DROP TABLE djmdSongHistory; CREATE TABLE djmdSongHistory(ID TEXT, ContentID TEXT);\
             DROP TABLE djmdContent; CREATE TABLE djmdContent(ID TEXT, Title TEXT);\
             INSERT INTO djmdSongHistory VALUES('h1','c1'); INSERT INTO djmdContent VALUES('c1','T');",
        );
        let mut db = MasterDb::open(&fx.path, &fx.key, false).unwrap();
        db.begin_read().unwrap();
        let rows = db.get_rows_after(0).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].deleted);
        assert_eq!(rows[0].created_at, None);
        let c = db.get_content("c1").unwrap().unwrap();
        assert_eq!(c.title.as_deref(), Some("T"));
        assert_eq!(c.artist, None);
        db.commit().unwrap();
    }

    #[test]
    fn 鍵の形式が不正なら開かない() {
        let fx = FixtureDb::new();
        assert_eq!(
            MasterDb::open(&fx.path, "xyz", false).err(),
            Some(DbError::InvalidKeyFormat)
        );
    }

    #[test]
    fn エラー分類() {
        assert_eq!(classify_code(26), DbErrorKind::NotADb);
        assert_eq!(classify_code(5), DbErrorKind::Busy);
        assert_eq!(classify_code(6 | (1 << 8)), DbErrorKind::Busy);
        assert_eq!(classify_code(1), DbErrorKind::Other);
    }
}
