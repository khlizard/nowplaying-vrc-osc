//! Velopack による自動更新（GitHub Releases）。インストールされていないとき（開発中や ZIP 版）は何もしない。

use std::sync::Mutex;

use velopack::sources::{FileSource, GithubSource};
use velopack::{UpdateCheck, UpdateManager, VelopackAsset};

const REPO_URL: &str = "https://github.com/khlizard/nowplaying-vrc-osc";
/// 更新の確認先をローカルのフォルダに差し替える（更新の動作確認用）。
const FEED_OVERRIDE_ENV: &str = "NPVO_UPDATE_FEED";

pub struct Updater {
    manager: Option<UpdateManager>,
    pending: Mutex<Option<VelopackAsset>>,
}

impl Updater {
    pub fn new() -> Self {
        Self {
            manager: create_manager(),
            pending: Mutex::new(None),
        }
    }

    /// 更新を確認してダウンロードする。取得できたらそのバージョンを返す。失敗は無視する。
    /// ネットワークを待つので、UI のスレッドから呼ばない。
    pub fn check_and_download(&self) -> Option<String> {
        let manager = self.manager.as_ref()?;
        let UpdateCheck::UpdateAvailable(info) = manager.check_for_updates().ok()? else {
            return None;
        };
        manager.download_updates(&info, None).ok()?;
        let asset = info.TargetFullRelease.clone();
        let version = asset.Version.clone();
        *self.pending.lock().unwrap_or_else(|p| p.into_inner()) = Some(asset);
        Some(version)
    }

    pub fn pending_version(&self) -> Option<String> {
        self.pending
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(|a| a.Version.clone())
    }

    /// 更新を適用して再起動する。成功するとプロセスはこの中で終わる。戻ってきたら失敗している。
    pub fn apply_and_restart(&self) {
        if let (Some(manager), Some(asset)) = (self.manager.as_ref(), self.pending_asset()) {
            let _ = manager.apply_updates_and_restart(&asset);
        }
    }

    /// 終了後に更新を適用するよう Velopack に頼む（再起動はしない）。この後すぐ終了すること。
    pub fn apply_on_exit(&self) {
        if let (Some(manager), Some(asset)) = (self.manager.as_ref(), self.pending_asset()) {
            let _ = manager.wait_exit_then_apply_updates(&asset, true, false, Vec::<String>::new());
        }
    }

    fn pending_asset(&self) -> Option<VelopackAsset> {
        self.pending
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}

fn create_manager() -> Option<UpdateManager> {
    let feed = std::env::var(FEED_OVERRIDE_ENV)
        .ok()
        .filter(|f| !f.trim().is_empty());
    // 未インストールのときは new が Err を返す。
    match feed {
        Some(dir) if std::path::Path::new(&dir).is_dir() => {
            UpdateManager::new(FileSource::new(dir), None, None).ok()
        }
        _ => UpdateManager::new(GithubSource::new(REPO_URL, None, true), None, None).ok(),
    }
}
