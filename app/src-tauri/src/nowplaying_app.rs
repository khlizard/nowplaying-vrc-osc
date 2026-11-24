//! Now Playing App 2 の `config.json` から、再生中の曲（`currentTrack`）を読む。
//! rekordbox を直接読めないときの予備として使う。Tauri には依存しない。
//!
//! config.json には Now Playing App の設定やトークンも入っている。
//! `currentTrack` 以外は読まず、中身をログ・画面・エラー文に出さない。

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::Value;

use crate::dedup::TrackDeduplicator;
use crate::rekordbox::source::SourceEvent;
use crate::track::TrackInfo;

pub const MSG_READING: &str = "Now Playing App から取得しています";

/// `%USERPROFILE%\.nowplaying\config.json`。USERPROFILE が無ければカレントを基準にする。
pub fn default_config_path() -> PathBuf {
    let base = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join(".nowplaying").join("config.json")
}

/// 明示指定があればそれ、無ければ既定の場所。
pub fn resolve_config_path(override_path: &str) -> PathBuf {
    let trimmed = override_path.trim();
    if trimmed.is_empty() {
        default_config_path()
    } else {
        PathBuf::from(trimmed)
    }
}

/// config.json を読めなかった（無い・書き込み途中で壊れている）。理由は出さない。
#[derive(Debug, PartialEq, Eq)]
pub struct ReadError;

/// `currentTrack` を読む。曲が無い・曲名が空なら `Ok(None)`。
pub fn read_current_track(path: &Path) -> Result<Option<TrackInfo>, ReadError> {
    let bytes = std::fs::read(path).map_err(|_| ReadError)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| ReadError)?;
    let json = text.strip_prefix('\u{feff}').unwrap_or(text);
    let doc: Value = serde_json::from_str(json).map_err(|_| ReadError)?;
    Ok(doc.get("currentTrack").and_then(parse_track))
}

fn parse_track(track: &Value) -> Option<TrackInfo> {
    let track = track.as_object()?;
    let text = |name: &str| -> Option<String> {
        track
            .get(name)
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|s| !s.trim().is_empty())
    };
    let title = text("title")?;
    let id = match track.get("id") {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    };
    Some(TrackInfo {
        id,
        title: Some(title),
        artist: text("artist"),
        album: text("album"),
        label: text("label"),
        comment: text("comment"),
    })
}

/// config.json を確認の間隔ごとに見て、曲が変わったら知らせる。
/// 使い始め（`reset` の後）に読んだ曲は基準にするだけで知らせない。
pub struct NowPlayingAppSource {
    path: PathBuf,
    last_signature: Option<String>,
    dedup: TrackDeduplicator,
    last_state: Option<String>,
}

impl NowPlayingAppSource {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            last_signature: None,
            dedup: TrackDeduplicator::new(),
            last_state: None,
        }
    }

    /// 使い始めの状態に戻す。次に読んだ曲は送らない。
    pub fn reset(&mut self) {
        self.last_signature = None;
        self.dedup.reset();
        self.last_state = None;
    }

    /// config.json があるか。
    pub fn is_available(&self) -> bool {
        self.path.is_file()
    }

    pub fn tick(&mut self, emit: &mut dyn FnMut(SourceEvent)) {
        let signature = file_signature(&self.path);
        if signature.is_some() && signature == self.last_signature {
            return;
        }
        // 読めなかったときは更新時刻を覚えず、次の確認で読み直す（書き込み途中のことがある）。
        let Ok(track) = read_current_track(&self.path) else {
            return;
        };
        self.last_signature = signature;
        if self.last_state.is_none() {
            self.last_state = Some(MSG_READING.to_string());
            emit(SourceEvent::Status(MSG_READING.to_string()));
        }
        if let Some(track) = track {
            if self.dedup.is_new_track(&track) {
                emit(SourceEvent::Track(track));
            }
        }
    }
}

/// `更新時刻(ms):サイズ`。読めなければ None。
fn file_signature(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let ms = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    Some(format!("{ms}:{}", meta.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, json: &str) {
        std::fs::write(path, json).unwrap();
    }

    fn track_json(id: &str, title: &str) -> String {
        format!(
            r#"{{"localSocketPort":29000,"token":"secret","currentTrack":{{"id":"{id}","title":"{title}","artist":"A","album":"","bpm":128}}}}"#
        )
    }

    fn run(source: &mut NowPlayingAppSource) -> Vec<SourceEvent> {
        let mut events = Vec::new();
        source.tick(&mut |e| events.push(e));
        events
    }

    fn tracks(events: &[SourceEvent]) -> Vec<String> {
        events
            .iter()
            .filter_map(|e| match e {
                SourceEvent::Track(t) => t.title.clone(),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn currenttrack_を読む() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        write(&p, &track_json("abc", "Song"));
        let t = read_current_track(&p).unwrap().unwrap();
        assert_eq!(t.id.as_deref(), Some("abc"));
        assert_eq!(t.title.as_deref(), Some("Song"));
        assert_eq!(t.artist.as_deref(), Some("A"));
        // 空文字は無いものとして扱う
        assert_eq!(t.album, None);
        assert_eq!(t.label, None);
    }

    #[test]
    fn 数値の_id_も受け取る() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        write(&p, r#"{"currentTrack":{"id":42,"title":"Song"}}"#);
        let t = read_current_track(&p).unwrap().unwrap();
        assert_eq!(t.id.as_deref(), Some("42"));
    }

    #[test]
    fn 曲名が無い_曲が無いときは_none() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        write(&p, r#"{"currentTrack":{"id":"x","title":"  "}}"#);
        assert_eq!(read_current_track(&p), Ok(None));
        write(&p, r#"{"currentTrack":null}"#);
        assert_eq!(read_current_track(&p), Ok(None));
        write(&p, r#"{"other":1}"#);
        assert_eq!(read_current_track(&p), Ok(None));
    }

    #[test]
    fn 壊れた_json_と無いファイルは_err() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        assert_eq!(read_current_track(&p), Err(ReadError));
        write(&p, r#"{"currentTrack":{"title":"#);
        assert_eq!(read_current_track(&p), Err(ReadError));
    }

    #[test]
    fn 最初の曲は送らず_変わったら送る() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        write(&p, &track_json("1", "First"));
        let mut source = NowPlayingAppSource::new(p.clone());

        let events = run(&mut source);
        assert_eq!(events, vec![SourceEvent::Status(MSG_READING.into())]);

        write(&p, &track_json("2", "Second"));
        assert_eq!(tracks(&run(&mut source)), vec!["Second"]);

        // ファイルが変わらなければ何もしない
        assert!(run(&mut source).is_empty());
    }

    #[test]
    fn 曲が同じなら送らない() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        write(&p, &track_json("1", "First"));
        let mut source = NowPlayingAppSource::new(p.clone());
        run(&mut source);

        // ほかの設定だけが変わった（サイズが変わる）
        write(
            &p,
            r#"{"localSocketPort":29001,"currentTrack":{"id":"1","title":"First"}}"#,
        );
        assert!(tracks(&run(&mut source)).is_empty());
    }

    #[test]
    fn reset_の後は今の曲を送らない() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        write(&p, &track_json("1", "First"));
        let mut source = NowPlayingAppSource::new(p.clone());
        run(&mut source);
        write(&p, &track_json("2", "Second"));
        assert_eq!(tracks(&run(&mut source)), vec!["Second"]);

        source.reset();
        write(&p, &track_json("3", "Third!"));
        let events = run(&mut source);
        assert!(tracks(&events).is_empty());
        assert_eq!(events, vec![SourceEvent::Status(MSG_READING.into())]);
    }

    #[test]
    fn 壊れていたら次の確認で読み直す() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        write(&p, &track_json("1", "First"));
        let mut source = NowPlayingAppSource::new(p.clone());
        run(&mut source);

        write(&p, r#"{"currentTrack":"#);
        assert!(run(&mut source).is_empty());
        write(&p, &track_json("2", "Second"));
        assert_eq!(tracks(&run(&mut source)), vec!["Second"]);
    }

    #[test]
    fn 場所の指定() {
        assert_eq!(
            resolve_config_path(r" X:\np\config.json "),
            PathBuf::from(r"X:\np\config.json")
        );
        assert!(resolve_config_path("  ").ends_with(r".nowplaying\config.json"));
    }
}
