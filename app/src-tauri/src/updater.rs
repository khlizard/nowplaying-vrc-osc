//! Velopack による自動更新（GitHub Releases）。インストールされていないとき（開発中や ZIP 版）は何もしない。

use std::sync::Mutex;

use serde::Serialize;
use velopack::sources::{FileSource, GithubSource};
use velopack::{UpdateCheck, UpdateManager, VelopackAsset};

const REPO_URL: &str = "https://github.com/khlizard/nowplaying-vrc-osc";
/// 更新の確認先をローカルのフォルダに差し替える（更新の動作確認用）。
const FEED_OVERRIDE_ENV: &str = "NPVO_UPDATE_FEED";

/// 更新の確認の状態。「このソフトについて」に表示する。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UpdateStatus {
    /// インストーラー版でない（開発中や ZIP 版）。確認できない。
    NotInstalled,
    /// まだ確認していない。
    Idle,
    Checking,
    UpToDate,
    /// 新しい版をダウンロードした。終了時に適用する。
    Downloaded {
        version: String,
    },
    Failed {
        message: String,
    },
}

impl UpdateStatus {
    /// 確認を始めてよいか。確認中、取得済み、未インストールのときは始めない。
    pub fn can_begin_check(&self) -> bool {
        matches!(self, Self::Idle | Self::UpToDate | Self::Failed { .. })
    }
}

pub struct Updater {
    manager: Option<UpdateManager>,
    pending: Mutex<Option<VelopackAsset>>,
    status: Mutex<UpdateStatus>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|p| p.into_inner())
}

impl Updater {
    pub fn new() -> Self {
        let manager = create_manager();
        let status = if manager.is_some() {
            UpdateStatus::Idle
        } else {
            UpdateStatus::NotInstalled
        };
        Self {
            manager,
            pending: Mutex::new(None),
            status: Mutex::new(status),
        }
    }

    pub fn status(&self) -> UpdateStatus {
        lock(&self.status).clone()
    }

    /// 確認を始められるなら状態を「確認中」にして true を返す。始められなければ何もせず false。
    /// 確認は同時に 1 つだけにする（起動時の確認と手動の確認が重なったときや、連打したとき）。
    pub fn try_begin_check(&self) -> bool {
        let mut status = lock(&self.status);
        if !status.can_begin_check() {
            return false;
        }
        *status = UpdateStatus::Checking;
        true
    }

    /// 更新を確認して、あればダウンロードする。先に `try_begin_check` で true を受け取っておくこと。
    /// ネットワークを待つので、UI のスレッドから呼ばない。
    pub fn check_and_download(&self) {
        let result = match self.manager.as_ref() {
            Some(manager) => self.check_with(manager),
            None => UpdateStatus::NotInstalled,
        };
        *lock(&self.status) = result;
    }

    fn check_with(&self, manager: &UpdateManager) -> UpdateStatus {
        let info = match manager.check_for_updates() {
            Ok(UpdateCheck::UpdateAvailable(info)) => info,
            Ok(_) => return UpdateStatus::UpToDate,
            Err(e) => {
                return UpdateStatus::Failed {
                    message: e.to_string(),
                }
            }
        };
        if let Err(e) = manager.download_updates(&info, None) {
            return UpdateStatus::Failed {
                message: e.to_string(),
            };
        }
        let asset = info.TargetFullRelease.clone();
        let version = asset.Version.clone();
        *lock(&self.pending) = Some(asset);
        UpdateStatus::Downloaded { version }
    }

    pub fn pending_version(&self) -> Option<String> {
        lock(&self.pending).as_ref().map(|a| a.Version.clone())
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
        lock(&self.pending).clone()
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

#[cfg(test)]
mod tests {
    use super::UpdateStatus;

    #[test]
    fn 確認を始められるのは未確認_最新_失敗のときだけ() {
        assert!(UpdateStatus::Idle.can_begin_check());
        assert!(UpdateStatus::UpToDate.can_begin_check());
        assert!(UpdateStatus::Failed {
            message: "x".into()
        }
        .can_begin_check());

        assert!(!UpdateStatus::NotInstalled.can_begin_check());
        assert!(!UpdateStatus::Checking.can_begin_check());
        assert!(!UpdateStatus::Downloaded {
            version: "1.0.0".into()
        }
        .can_begin_check());
    }

    #[test]
    fn 画面に渡す形はkindで区別する() {
        let json = serde_json::to_string(&UpdateStatus::Downloaded {
            version: "3.1.0".into(),
        })
        .unwrap();
        assert_eq!(json, r#"{"kind":"downloaded","version":"3.1.0"}"#);
        let json = serde_json::to_string(&UpdateStatus::UpToDate).unwrap();
        assert_eq!(json, r#"{"kind":"upToDate"}"#);
    }
}
