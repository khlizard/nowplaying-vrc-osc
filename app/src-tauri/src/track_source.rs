//! 曲の取得元を選ぶ。rekordbox を直接読めるときは rekordbox、読めないときは Now Playing App。
//! Tauri には依存しない。ループや待ち時間は持たず、呼び出し側が `tick` を繰り返し呼ぶ。

use std::sync::atomic::AtomicBool;

use crate::nowplaying_app::{resolve_config_path, NowPlayingAppSource};
use crate::rekordbox::source::{RekordboxTrackSource, SourceEvent};
use crate::settings::AppSettings;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Rekordbox,
    NowPlayingApp,
}

pub struct TrackSource {
    rekordbox: RekordboxTrackSource,
    /// 設定でオフのときは None。
    now_playing_app: Option<NowPlayingAppSource>,
    mode: Mode,
}

impl TrackSource {
    pub fn new(
        rekordbox: RekordboxTrackSource,
        now_playing_app: Option<NowPlayingAppSource>,
    ) -> Self {
        Self {
            rekordbox,
            now_playing_app,
            mode: Mode::Rekordbox,
        }
    }

    pub fn from_settings(settings: &AppSettings) -> Self {
        let now_playing_app = settings.now_playing_app_enabled.then(|| {
            NowPlayingAppSource::new(resolve_config_path(&settings.now_playing_app_config_path))
        });
        Self::new(
            RekordboxTrackSource::from_settings(settings),
            now_playing_app,
        )
    }

    /// 1 回分の確認。rekordbox は毎回確かめ、読めなければ Now Playing App から取る。
    pub fn tick(&mut self, stop: &AtomicBool, emit: &mut dyn FnMut(SourceEvent)) {
        let mut rekordbox_events = Vec::new();
        self.rekordbox.tick(stop, &mut |e| rekordbox_events.push(e));

        let use_now_playing_app = !self.rekordbox.is_readable()
            && self
                .now_playing_app
                .as_ref()
                .is_some_and(NowPlayingAppSource::is_available);
        let mode = if use_now_playing_app {
            Mode::NowPlayingApp
        } else {
            Mode::Rekordbox
        };

        if mode != self.mode {
            self.mode = mode;
            match mode {
                // 切り替えた時点の曲は送らない。
                Mode::NowPlayingApp => {
                    if let Some(npa) = self.now_playing_app.as_mut() {
                        npa.reset();
                    }
                }
                // rekordbox の状態は変わったときしか届かないので、今の状態を出し直す。
                Mode::Rekordbox => {
                    if let Some(state) = self.rekordbox.state() {
                        emit(SourceEvent::Status(state.to_string()));
                    }
                }
            }
        }

        match self.mode {
            Mode::Rekordbox => rekordbox_events.into_iter().for_each(|e| emit(e)),
            Mode::NowPlayingApp => {
                if let Some(npa) = self.now_playing_app.as_mut() {
                    npa.tick(emit);
                }
            }
        }
    }

    /// ループ側で panic を捕まえたときに呼ぶ。
    pub fn report_unexpected(&mut self, kind: &str, emit: &mut dyn FnMut(SourceEvent)) {
        match self.mode {
            Mode::Rekordbox => self.rekordbox.report_unexpected(kind, emit),
            Mode::NowPlayingApp => {
                if let Some(npa) = self.now_playing_app.as_mut() {
                    npa.reset();
                }
                emit(SourceEvent::Status(format!("予期しないエラー ({kind})")));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nowplaying_app::MSG_READING;
    use crate::rekordbox::key_resolver::KeyResolver;
    use crate::rekordbox::master_db::fixture::FixtureDb;
    use crate::spotify::SpotifyTitleProvider;
    use std::path::{Path, PathBuf};

    const DB_MISSING: &str = "master.db が見つかりません";
    const CONNECTED: &str = "rekordbox に接続しました";

    struct FixedKey(Option<String>);
    impl KeyResolver for FixedKey {
        fn resolve(&mut self, _db_path: &Path, _stop: &AtomicBool) -> Option<String> {
            self.0.clone()
        }
        fn invalidate(&mut self) {}
    }

    struct NoSpotify;
    impl SpotifyTitleProvider for NoSpotify {
        fn get_title(&mut self, _track_id: &str, _stop: &AtomicBool) -> Option<String> {
            None
        }
    }

    fn rekordbox(db_path: PathBuf, key: Option<String>) -> RekordboxTrackSource {
        RekordboxTrackSource::new(
            Box::new(move || db_path.clone()),
            Box::new(FixedKey(key)),
            Box::new(NoSpotify),
        )
    }

    fn npa_json(id: &str, title: &str) -> String {
        format!(r#"{{"currentTrack":{{"id":"{id}","title":"{title}","artist":"A"}}}}"#)
    }

    fn run(source: &mut TrackSource) -> Vec<SourceEvent> {
        let stop = AtomicBool::new(false);
        let mut events = Vec::new();
        source.tick(&stop, &mut |e| events.push(e));
        events
    }

    fn titles(events: &[SourceEvent]) -> Vec<String> {
        events
            .iter()
            .filter_map(|e| match e {
                SourceEvent::Track(t) => t.title.clone(),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn rekordbox_を読めなければ_now_playing_app_から取る() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.json");
        std::fs::write(&config, npa_json("1", "First")).unwrap();
        let mut source = TrackSource::new(
            rekordbox(dir.path().join("none").join("master.db"), None),
            Some(NowPlayingAppSource::new(config.clone())),
        );

        // 切り替えた時点の曲は送らない
        let events = run(&mut source);
        assert_eq!(events, vec![SourceEvent::Status(MSG_READING.into())]);

        std::fs::write(&config, npa_json("2", "Second")).unwrap();
        assert_eq!(titles(&run(&mut source)), vec!["Second"]);
    }

    #[test]
    fn config_json_が無くなったら_rekordbox_の状態に戻す() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.json");
        std::fs::write(&config, npa_json("1", "First")).unwrap();
        let mut source = TrackSource::new(
            rekordbox(dir.path().join("none").join("master.db"), None),
            Some(NowPlayingAppSource::new(config.clone())),
        );
        run(&mut source);

        std::fs::remove_file(&config).unwrap();
        assert_eq!(
            run(&mut source),
            vec![SourceEvent::Status(DB_MISSING.into())]
        );
    }

    #[test]
    fn オフなら_rekordbox_の状態のまま() {
        let dir = tempfile::tempdir().unwrap();
        let mut source = TrackSource::new(
            rekordbox(dir.path().join("none").join("master.db"), None),
            None,
        );
        assert_eq!(
            run(&mut source),
            vec![SourceEvent::Status(DB_MISSING.into())]
        );
    }

    #[test]
    fn config_json_が無ければ_rekordbox_の状態のまま() {
        let dir = tempfile::tempdir().unwrap();
        let mut source = TrackSource::new(
            rekordbox(dir.path().join("none").join("master.db"), None),
            Some(NowPlayingAppSource::new(dir.path().join("config.json"))),
        );
        assert_eq!(
            run(&mut source),
            vec![SourceEvent::Status(DB_MISSING.into())]
        );
    }

    #[test]
    fn rekordbox_を読めるときは_now_playing_app_を使わない() {
        let fx = FixtureDb::new();
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.json");
        std::fs::write(&config, npa_json("1", "First")).unwrap();
        let mut source = TrackSource::new(
            rekordbox(fx.path.clone(), Some(fx.key.clone())),
            Some(NowPlayingAppSource::new(config.clone())),
        );

        assert_eq!(
            run(&mut source),
            vec![SourceEvent::Status(CONNECTED.into())]
        );
        std::fs::write(&config, npa_json("2", "Second")).unwrap();
        assert!(titles(&run(&mut source)).is_empty());
    }
}
