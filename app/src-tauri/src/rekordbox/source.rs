//! rekordbox の master.db（履歴）を読み取り専用でポーリングし、新しく再生された曲を通知する
//! （C# 版 `RekordboxTrackSource`）。起動時点の曲は通知しない（カーソルのメモリ上基準のみ）。
//! ループ・待ち時間・スレッドは持たない。呼び出し側が `tick` を繰り返し呼ぶ。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

use super::cursor::{HistoryCursor, HistoryRow};
use super::key_resolver::{default_key_resolver, KeyResolver};
use super::master_db::{wal_path, DbError, DbErrorKind, MasterDb};
use super::normalize::{self, ContentRow};
use super::paths::resolve_db_path;
use crate::settings::AppSettings;
use crate::spotify::{SpotifyOEmbedClient, SpotifyTitleProvider};
use crate::track::TrackInfo;

const NOT_A_DB_LIMIT: u32 = 3;
pub const MIN_INTERVAL_SECONDS: f64 = 0.2;

const MSG_DB_MISSING: &str = "master.db が見つかりません";
const MSG_WAITING: &str = "rekordbox の起動を待っています";
const MSG_NO_KEY: &str = "復号キーを取得できません（設定画面で入力してください）";
const MSG_CONNECTED: &str = "rekordbox に接続しました";
const MSG_BUSY: &str = "rekordbox がデータベースを使用中です。再試行します";
const MSG_BAD_KEY: &str = "復号キーでデータベースを開けません。再試行します";
const MSG_SCHEMA: &str = "master.db の構成が想定と異なります";
const MSG_NO_TITLE_SPOTIFY: &str = "曲名を取得できませんでした（Spotify）";
const MSG_NO_TITLE: &str = "曲名を取得できませんでした";

/// `tick` が通知するもの。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceEvent {
    /// 接続状態・エラーの変化（画面表示用）。
    Status(String),
    /// 新しく再生された曲。
    Track(TrackInfo),
}

pub struct RekordboxTrackSource {
    get_db_path: Box<dyn Fn() -> PathBuf + Send>,
    keys: Box<dyn KeyResolver>,
    spotify: Box<dyn SpotifyTitleProvider>,
    cursor: HistoryCursor,
    /// 復号キー。ログや Debug に出さないため、この型は Debug を実装しない。
    key: Option<String>,
    last_signature: Option<String>,
    connected: bool,
    not_a_db_count: u32,
    last_state: Option<String>,
}

impl RekordboxTrackSource {
    pub fn new(
        get_db_path: Box<dyn Fn() -> PathBuf + Send>,
        keys: Box<dyn KeyResolver>,
        spotify: Box<dyn SpotifyTitleProvider>,
    ) -> Self {
        RekordboxTrackSource {
            get_db_path,
            keys,
            spotify,
            cursor: HistoryCursor::new(),
            key: None,
            last_signature: None,
            connected: false,
            not_a_db_count: 0,
            last_state: None,
        }
    }

    pub fn from_settings(settings: &AppSettings) -> Self {
        let override_path = settings.db_path_override.clone();
        Self::new(
            Box::new(move || resolve_db_path(&override_path)),
            default_key_resolver(),
            Box::new(SpotifyOEmbedClient::new_default()),
        )
    }

    /// 1 回分の確認。状態は Status、送る曲は Track で通知する。
    pub fn tick(&mut self, stop: &AtomicBool, emit: &mut dyn FnMut(SourceEvent)) {
        let db_path = (self.get_db_path)();
        let wal = wal_path(&db_path);

        if !db_path.is_file() {
            self.disconnect(MSG_DB_MISSING, emit);
            return;
        }
        if !wal.is_file() {
            // -wal が無いときは開かない（rekordbox 未起動。開くと -wal/-shm を作ってしまう）
            self.disconnect(MSG_WAITING, emit);
            return;
        }

        let signature = signature(&db_path, &wal);
        if self.connected && self.last_signature.as_deref() == Some(signature.as_str()) {
            return;
        }

        if self.key.is_none() {
            self.key = self.keys.resolve(&db_path, stop);
            if self.key.is_none() {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                self.connected = false;
                self.set_state(MSG_NO_KEY, emit);
                return;
            }
        }

        let key = self.key.clone().unwrap_or_default();
        let (fired, content) = match self.read_once(&db_path, &key) {
            Ok(v) => v,
            Err(DbError::Sqlite { kind, .. }) => {
                self.handle_db_error(kind, emit);
                return;
            }
            Err(DbError::Schema(_)) => {
                self.connected = false;
                self.set_state(MSG_SCHEMA, emit);
                return;
            }
            Err(DbError::InvalidKeyFormat) => {
                self.report_unexpected("InvalidKeyFormat", emit);
                return;
            }
        };

        // 読み取り成功: シグネチャを確定する
        self.last_signature = Some(signature);
        self.connected = true;
        self.not_a_db_count = 0;
        self.set_state(MSG_CONNECTED, emit);

        let Some(fired) = fired else {
            return;
        };

        let mut track = normalize::build(fired.content_id.as_deref(), content.as_ref());
        let spotify_id = normalize::try_get_spotify_track_id(
            content.as_ref().and_then(|c| c.folder_path.as_deref()),
        );
        if track.title.is_none() {
            if let Some(id) = &spotify_id {
                track.title = self.spotify.get_title(id, stop);
            }
        }

        if track.title.is_none() {
            // 停止で打ち切られたときは通知しない
            if stop.load(Ordering::Relaxed) {
                return;
            }
            let msg = if spotify_id.is_some() {
                MSG_NO_TITLE_SPOTIFY
            } else {
                MSG_NO_TITLE
            };
            self.notify(msg, emit);
            return;
        }

        emit(SourceEvent::Track(track));
    }

    /// ループ側で panic を捕まえたときに呼ぶ。接続を切った扱いにして通知する。
    pub fn report_unexpected(&mut self, kind: &str, emit: &mut dyn FnMut(SourceEvent)) {
        self.connected = false;
        self.notify(&format!("予期しないエラー ({kind})"), emit);
    }

    /// 1 回の読み取り。open → BEGIN → クエリ → COMMIT → close。
    fn read_once(
        &mut self,
        db_path: &Path,
        key: &str,
    ) -> Result<(Option<HistoryRow>, Option<ContentRow>), DbError> {
        let mut db = MasterDb::open(db_path, key, false)?;
        db.begin_read()?;

        let (max_rowid, max_id) = db.get_max()?;
        let mut id_at_cursor = None;
        let mut new_rows = Vec::new();
        if let Some(cursor_rowid) = self.cursor.rowid {
            id_at_cursor = db.get_id_at(cursor_rowid)?;
            new_rows = db.get_rows_after(cursor_rowid)?;
        }

        let fired = self
            .cursor
            .observe(
                max_rowid,
                max_id.as_deref(),
                id_at_cursor.as_deref(),
                &new_rows,
            )
            .cloned();
        let mut content = None;
        if let Some(cid) = fired.as_ref().and_then(|f| f.content_id.as_deref()) {
            content = db.get_content(cid)?;
        }

        db.commit()?;
        Ok((fired, content))
    }

    fn handle_db_error(&mut self, kind: DbErrorKind, emit: &mut dyn FnMut(SourceEvent)) {
        self.connected = false;
        match kind {
            DbErrorKind::NotADb => {
                self.not_a_db_count += 1;
                if self.not_a_db_count >= NOT_A_DB_LIMIT {
                    // 鍵が使えない。キャッシュを捨てて解決し直す
                    self.not_a_db_count = 0;
                    self.key = None;
                    self.keys.invalidate();
                }
                self.set_state(MSG_BAD_KEY, emit);
            }
            DbErrorKind::Busy => self.set_state(MSG_BUSY, emit),
            DbErrorKind::Other => {
                self.notify("データベース読み取りエラー (SqliteError)", emit);
            }
        }
    }

    /// 接続できない状態へ。カーソルも捨てる（再接続時に基準を取り直し、古い曲を送らない）。
    fn disconnect(&mut self, message: &str, emit: &mut dyn FnMut(SourceEvent)) {
        self.connected = false;
        self.last_signature = None;
        self.cursor.reset();
        self.set_state(message, emit);
    }

    /// 状態の通知。直前と同じ文言なら出さない。
    fn set_state(&mut self, message: &str, emit: &mut dyn FnMut(SourceEvent)) {
        if self.last_state.as_deref() == Some(message) {
            return;
        }
        self.last_state = Some(message.to_string());
        emit(SourceEvent::Status(message.to_string()));
    }

    /// 出来事の通知。毎回出す。
    fn notify(&mut self, message: &str, emit: &mut dyn FnMut(SourceEvent)) {
        self.last_state = None;
        emit(SourceEvent::Status(message.to_string()));
    }
}

/// db と wal の `更新時刻(ms):サイズ`。
fn signature(db_path: &Path, wal: &Path) -> String {
    format!("{}|{}", file_sig(db_path), file_sig(wal))
}

fn file_sig(path: &Path) -> String {
    let Ok(meta) = std::fs::metadata(path) else {
        return "-".to_string();
    };
    let Ok(modified) = meta.modified() else {
        return "-".to_string();
    };
    // Unix エポックからのミリ秒（エポック以前は負）
    let ms: i128 = match modified.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_millis() as i128,
        Err(e) => -(e.duration().as_millis() as i128),
    };
    format!("{ms}:{}", meta.len())
}

#[cfg(test)]
mod tests {
    use super::super::master_db::fixture::{other_key, FixtureDb};
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Arc, Mutex};

    const CONNECTED: &str = MSG_CONNECTED;

    struct FakeKeyResolver {
        key: Option<String>,
        invalidated: Arc<AtomicUsize>,
    }

    impl KeyResolver for FakeKeyResolver {
        fn resolve(&mut self, _db_path: &Path, _stop: &AtomicBool) -> Option<String> {
            self.key.clone()
        }
        fn invalidate(&mut self) {
            self.invalidated.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct FakeSpotify {
        title: Option<String>,
        requested: Arc<Mutex<Vec<String>>>,
    }

    impl SpotifyTitleProvider for FakeSpotify {
        fn get_title(&mut self, track_id: &str, _stop: &AtomicBool) -> Option<String> {
            self.requested.lock().unwrap().push(track_id.to_string());
            self.title.clone()
        }
    }

    struct Harness {
        fx: FixtureDb,
        /// source が参照する DB パス（テスト中に差し替えられる）。
        path: Arc<Mutex<PathBuf>>,
        source: RekordboxTrackSource,
        invalidated: Arc<AtomicUsize>,
        requested: Arc<Mutex<Vec<String>>>,
        tracks: Vec<TrackInfo>,
        statuses: Vec<String>,
        stop: AtomicBool,
    }

    impl Harness {
        fn new(fx: FixtureDb, key: Option<String>, spotify_title: Option<&str>) -> Harness {
            let invalidated = Arc::new(AtomicUsize::new(0));
            let requested = Arc::new(Mutex::new(Vec::new()));
            let path = Arc::new(Mutex::new(fx.path.clone()));
            let path2 = path.clone();
            let source = RekordboxTrackSource::new(
                Box::new(move || path2.lock().unwrap().clone()),
                Box::new(FakeKeyResolver {
                    key,
                    invalidated: invalidated.clone(),
                }),
                Box::new(FakeSpotify {
                    title: spotify_title.map(str::to_string),
                    requested: requested.clone(),
                }),
            );
            Harness {
                fx,
                path,
                source,
                invalidated,
                requested,
                tracks: Vec::new(),
                statuses: Vec::new(),
                stop: AtomicBool::new(false),
            }
        }

        /// 正しい鍵を使うハーネス。
        fn with_key(fx: FixtureDb) -> Harness {
            let key = fx.key.clone();
            Harness::new(fx, Some(key), None)
        }

        fn tick(&mut self) {
            let (tracks, statuses) = (&mut self.tracks, &mut self.statuses);
            self.source.tick(&self.stop, &mut |e| match e {
                SourceEvent::Track(t) => tracks.push(t),
                SourceEvent::Status(s) => statuses.push(s),
            });
        }

        fn count(&self, status: &str) -> usize {
            self.statuses.iter().filter(|s| *s == status).count()
        }
    }

    #[test]
    fn 起動時は何も送らず_新しい行で一度だけ送る() {
        let fx = FixtureDb::new();
        fx.add_content_full(
            "c0",
            Some("Old Song"),
            "C:/a.mp3",
            Some("Old Artist"),
            None,
            None,
            None,
        );
        fx.add_history("h0", "c0", false, None);
        fx.add_content_full(
            "c1",
            Some("New Song"),
            "C:/b.mp3",
            Some("New Artist"),
            Some("Alb"),
            None,
            None,
        );

        let mut h = Harness::with_key(fx);
        h.tick();
        assert_eq!(h.count(CONNECTED), 1);
        h.tick();
        assert!(h.tracks.is_empty());

        h.fx.add_history("h1", "c1", false, None);
        h.tick();
        h.tick();
        h.tick();

        assert_eq!(h.tracks.len(), 1);
        let t = &h.tracks[0];
        assert_eq!(t.id.as_deref(), Some("c1"));
        assert_eq!(t.title.as_deref(), Some("New Song"));
        assert_eq!(t.artist.as_deref(), Some("New Artist"));
        assert_eq!(t.album.as_deref(), Some("Alb"));
        // 接続の通知は SetState なので重複しない
        assert_eq!(h.count(CONNECTED), 1);
    }

    #[test]
    fn spotify_の行で曲名が暗号化されていたら_oembed_の曲名を使う() {
        let fx = FixtureDb::new();
        let sid = "0123456789abcdefghijAB";
        fx.add_content_full(
            "c1",
            Some("$A7:v1:enc"),
            &format!("spotify:track:{sid}"),
            Some("$A7:v1:enc2"),
            None,
            None,
            None,
        );
        let key = fx.key.clone();
        let mut h = Harness::new(fx, Some(key), Some("Spotify Title"));
        h.tick();
        assert_eq!(h.count(CONNECTED), 1);

        h.fx.add_history("h1", "c1", false, None);
        h.tick();

        assert_eq!(h.tracks.len(), 1);
        assert_eq!(h.tracks[0].title.as_deref(), Some("Spotify Title"));
        assert_eq!(h.tracks[0].artist, None);
        assert_eq!(*h.requested.lock().unwrap(), vec![sid.to_string()]);
    }

    #[test]
    fn oembed_が失敗したら何も送らず状態を通知する() {
        let fx = FixtureDb::new();
        fx.add_content_full(
            "c1",
            Some("$A7:v1:enc"),
            "spotify:track:0123456789abcdefghijAB",
            None,
            None,
            None,
            None,
        );
        let key = fx.key.clone();
        let mut h = Harness::new(fx, Some(key), None);
        h.tick();

        h.fx.add_history("h1", "c1", false, None);
        h.tick();
        assert_eq!(h.count("曲名を取得できませんでした（Spotify）"), 1);
        assert!(h.tracks.is_empty());
    }

    #[test]
    fn spotify_以外で曲名が無ければ専用の文言を通知する() {
        let fx = FixtureDb::new();
        fx.add_content("c1", None);
        let mut h = Harness::with_key(fx);
        h.tick();
        h.fx.add_history("h1", "c1", false, None);
        h.tick();
        assert_eq!(h.count("曲名を取得できませんでした"), 1);
        assert!(h.tracks.is_empty());
    }

    #[test]
    fn 停止中に_spotify_が打ち切られたら通知しない() {
        let fx = FixtureDb::new();
        fx.add_content_full(
            "c1",
            Some("$A7:v1:enc"),
            "spotify:track:0123456789abcdefghijAB",
            None,
            None,
            None,
            None,
        );
        let key = fx.key.clone();
        let mut h = Harness::new(fx, Some(key), None);
        h.tick();
        h.fx.add_history("h1", "c1", false, None);
        h.stop.store(true, Ordering::Relaxed);
        h.tick();
        assert_eq!(h.count("曲名を取得できませんでした（Spotify）"), 0);
        assert!(h.tracks.is_empty());
    }

    #[test]
    fn wal_が無ければ_db_を開かず_起動待ちを通知する() {
        let mut fx = FixtureDb::new();
        fx.add_content("c1", Some("x"));
        fx.close_writer();
        let before = fx.file_names();

        let mut h = Harness::with_key(fx);
        h.tick();
        h.tick();

        assert_eq!(h.count("rekordbox の起動を待っています"), 1);
        assert_eq!(before, h.fx.file_names());
        assert!(!h.fx.wal_path().exists());
        assert!(!h.fx.shm_path().exists());
    }

    #[test]
    fn db_が無ければ通知する() {
        let fx = FixtureDb::new();
        let mut h = Harness::with_key(fx);
        *h.path.lock().unwrap() = h.fx.dir.path().join("none.db");
        h.tick();
        h.tick();
        assert_eq!(h.count("master.db が見つかりません"), 1);
    }

    #[test]
    fn 鍵が無ければ状態を通知する() {
        let fx = FixtureDb::new();
        let mut h = Harness::new(fx, None, None);
        h.tick();
        h.tick();
        assert_eq!(
            h.statuses,
            vec!["復号キーを取得できません（設定画面で入力してください）".to_string()]
        );
    }

    #[test]
    fn 鍵違いが_3_回続くとリゾルバを破棄し_状態に鍵を含めない() {
        let fx = FixtureDb::new();
        let wrong = other_key(&fx.key);
        let mut h = Harness::new(fx, Some(wrong.clone()), None);
        h.tick();
        h.tick();
        assert_eq!(h.invalidated.load(Ordering::SeqCst), 0);
        h.tick();
        assert_eq!(h.invalidated.load(Ordering::SeqCst), 1);
        assert!(h.statuses.iter().all(|s| !s.contains(&wrong)));
        assert!(h.statuses.iter().any(|s| s == MSG_BAD_KEY));
    }

    #[test]
    fn 切断でカーソルを捨て_再接続時に古い曲を送らない() {
        let fx = FixtureDb::new();
        fx.add_content("c0", Some("Old"));
        fx.add_content("c1", Some("Gap"));
        fx.add_history("h0", "c0", false, None);
        let real = fx.path.clone();
        let mut h = Harness::with_key(fx);
        h.tick();
        assert_eq!(h.count(CONNECTED), 1);

        // 切断中に履歴が増える
        *h.path.lock().unwrap() = h.fx.dir.path().join("none.db");
        h.tick();
        h.fx.add_history("h1", "c1", false, None);

        // 再接続は基準を取り直すだけで、切断中の曲は送らない
        *h.path.lock().unwrap() = real;
        h.tick();
        h.tick();
        assert!(h.tracks.is_empty());
        assert_eq!(h.count(CONNECTED), 2);

        // 再接続後の新しい行は送る
        h.fx.add_content("c2", Some("After"));
        h.fx.add_history("h2", "c2", false, None);
        h.tick();
        assert_eq!(h.tracks.len(), 1);
        assert_eq!(h.tracks[0].title.as_deref(), Some("After"));
    }

    #[test]
    fn 想定外エラーの通知は毎回出る() {
        let fx = FixtureDb::new();
        let mut h = Harness::with_key(fx);
        let mut statuses = Vec::new();
        for _ in 0..2 {
            h.source
                .report_unexpected("TestPanic", &mut |e| statuses.push(e));
        }
        assert_eq!(
            statuses,
            vec![
                SourceEvent::Status("予期しないエラー (TestPanic)".to_string()),
                SourceEvent::Status("予期しないエラー (TestPanic)".to_string()),
            ]
        );
    }

    #[test]
    fn 最小間隔は_0_2_秒() {
        assert_eq!(MIN_INTERVAL_SECONDS, 0.2);
    }
}
