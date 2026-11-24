//! 画面から呼ぶコマンド。ダイアログを待つものは async にする（メインスレッドで待つと固まるため）。

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt as _, MessageDialogButtons, MessageDialogKind};

use crate::app::{
    self, AboutState, AppState, MainState, MAIN_WINDOW, PRODUCT_NAME, SETTINGS_WINDOW,
};
use crate::format::format_chatbox_message;
use crate::nowplaying_app::resolve_config_path as resolve_np_config_path_core;
use crate::rekordbox::dpapi::DpapiKeyStore;
use crate::rekordbox::master_db::{verify_key, KeyCheckResult};
use crate::rekordbox::paths::resolve_db_path as resolve_db_path_core;
use crate::settings::AppSettings;
use crate::track::TrackInfo;

/// テスト送信の文言（旧 Python 版の既定と同じ）。
const TEST_MESSAGE: &str = "nowplaying-vrc-osc テスト";
const SETTINGS_TITLE: &str = "設定";

// --- メイン画面 ---

#[tauri::command]
pub fn get_main_state(app: AppHandle) -> MainState {
    app::main_state(&app)
}

#[tauri::command]
pub fn start_bridge(app: AppHandle) -> Result<(), String> {
    app::start_bridge(&app)
}

#[tauri::command]
pub fn stop_bridge(app: AppHandle) {
    app::stop_bridge(&app);
}

/// 成功したら最終送信欄の文字列。失敗したら警告を出して None。
#[tauri::command]
pub async fn test_send(app: AppHandle) -> Option<String> {
    match app::test_send(&app, TEST_MESSAGE) {
        Ok(line) => Some(line),
        Err(e) => {
            warn(
                &app,
                MAIN_WINDOW,
                PRODUCT_NAME,
                &format!("テスト送信に失敗しました:\n{e}"),
            );
            None
        }
    }
}

/// ☰ メニューを開く。位置はメイン画面の左上からの論理座標。
#[tauri::command]
pub async fn show_main_menu(app: AppHandle, x: f64, y: f64) -> Result<(), String> {
    app::show_main_menu(&app, x, y)
}

#[tauri::command]
pub async fn apply_update_and_restart(app: AppHandle) {
    if ask(
        &app,
        MAIN_WINDOW,
        PRODUCT_NAME,
        "監視を停止して更新し、再起動します。よろしいですか？",
    ) {
        app::apply_update_and_restart(&app);
    }
}

// --- このソフトについて ---

#[tauri::command]
pub fn get_about_state(app: AppHandle) -> AboutState {
    app::about_state(&app)
}

/// 更新の確認を始める（結果は `about-state` で届く）。確認中などで始められなければ何もしない。
#[tauri::command]
pub fn check_for_updates(app: AppHandle) {
    app::check_update(&app);
}

// --- 設定画面 ---

#[tauri::command]
pub fn get_settings(app: AppHandle) -> AppSettings {
    app.state::<AppState>().settings_snapshot()
}

#[tauri::command]
pub fn get_key_saved() -> bool {
    DpapiKeyStore::new(DpapiKeyStore::default_path()).exists()
}

#[tauri::command]
pub fn resolve_db_path(override_path: String) -> String {
    resolve_db_path_core(override_path.trim())
        .display()
        .to_string()
}

/// サンプルの曲で、送られるメッセージを組み立てる。
#[tauri::command]
pub fn preview_message(settings: AppSettings) -> String {
    let sample = TrackInfo {
        id: Some("sample".into()),
        title: Some("Sample Title [Extended Mix]".into()),
        artist: Some("Sample Artist".into()),
        album: Some("Sample Album".into()),
        label: Some("Sample Label".into()),
        comment: Some("Sample Comment".into()),
    };
    format_chatbox_message(&sample, &settings.to_message_format())
}

#[tauri::command]
pub async fn browse_db_path(app: AppHandle, current: String) -> Option<String> {
    let mut dialog = app
        .dialog()
        .file()
        .set_title("rekordbox の master.db を選択")
        .add_filter("rekordbox データベース (*.db)", &["db"])
        .add_filter("すべてのファイル", &["*"]);
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW) {
        dialog = dialog.set_parent(&window);
    }
    // 今の値（空なら自動検出のパス）のフォルダから開く。
    let path = resolve_db_path_core(current.trim());
    if let Some(dir) = path.parent().filter(|d| d.is_dir()) {
        dialog = dialog.set_directory(dir);
    }
    if let Some(name) = path.file_name() {
        dialog = dialog.set_file_name(name.to_string_lossy());
    }
    let picked = dialog.blocking_pick_file()?;
    picked.into_path().ok().map(|p| p.display().to_string())
}

#[tauri::command]
pub fn resolve_np_config_path(override_path: String) -> String {
    resolve_np_config_path_core(&override_path)
        .display()
        .to_string()
}

/// Now Playing App の config.json を選ぶ。
#[tauri::command]
pub async fn browse_np_config_path(app: AppHandle, current: String) -> Option<String> {
    let mut dialog = app
        .dialog()
        .file()
        .set_title("Now Playing App の config.json を選択")
        .add_filter("Now Playing App の設定 (*.json)", &["json"])
        .add_filter("すべてのファイル", &["*"]);
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW) {
        dialog = dialog.set_parent(&window);
    }
    // 今の値（空なら既定の場所）のフォルダから開く。
    let path = resolve_np_config_path_core(&current);
    if let Some(dir) = path.parent().filter(|d| d.is_dir()) {
        dialog = dialog.set_directory(dir);
    }
    if let Some(name) = path.file_name() {
        dialog = dialog.set_file_name(name.to_string_lossy());
    }
    let picked = dialog.blocking_pick_file()?;
    picked.into_path().ok().map(|p| p.display().to_string())
}

#[derive(Serialize)]
pub struct SaveResult {
    saved: bool,
    error: Option<String>,
    focus: Option<&'static str>,
}

impl SaveResult {
    fn error(message: impl Into<String>, focus: Option<&'static str>) -> Self {
        Self {
            saved: false,
            error: Some(message.into()),
            focus,
        }
    }
}

/// キーを確かめて保存し、設定を保存する。保存できたら設定画面を閉じ、監視中なら新しい設定でやり直す。
/// 入力欄の検証は画面側で済ませてある。
#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: AppSettings, key: String) -> SaveResult {
    let key = key.trim();
    if !key.is_empty() {
        let db_path = resolve_db_path_core(settings.db_path_override.trim());
        let reason = match verify_key(&db_path, key) {
            KeyCheckResult::Ok => None,
            KeyCheckResult::InvalidFormat => {
                return SaveResult::error(
                    "復号キーは 64 桁の 16 進数で入力してください。",
                    Some("key"),
                );
            }
            KeyCheckResult::WrongKey => {
                return SaveResult::error(
                    "このキーでは master.db を開けませんでした。",
                    Some("key"),
                )
            }
            KeyCheckResult::DbNotFound => Some("master.db が見つかりません"),
            KeyCheckResult::Busy => Some("rekordbox がデータベースを使用中です"),
            KeyCheckResult::Error => Some("読み取りエラー"),
        };
        if let Some(reason) = reason {
            let message = format!(
                "master.db でキーを確認できませんでした（{reason}）。このまま保存しますか？"
            );
            if !ask(&app, SETTINGS_WINDOW, SETTINGS_TITLE, &message) {
                return SaveResult {
                    saved: false,
                    error: None,
                    focus: None,
                };
            }
        }
        if let Err(e) = DpapiKeyStore::new(DpapiKeyStore::default_path()).save(key) {
            return SaveResult::error(format!("復号キーを保存できませんでした: {e}"), None);
        }
    }

    if let Err(e) = settings.save() {
        return SaveResult::error(format!("設定を保存できませんでした: {e}"), None);
    }
    *app::lock(&app.state::<AppState>().settings) = settings;
    app::close_settings(&app);
    if let Err(e) = app::restart_bridge_if_running(&app) {
        warn(&app, MAIN_WINDOW, PRODUCT_NAME, &e);
    }
    SaveResult {
        saved: true,
        error: None,
        focus: None,
    }
}

#[tauri::command]
pub async fn show_warning(app: AppHandle, message: String) {
    warn(&app, SETTINGS_WINDOW, SETTINGS_TITLE, &message);
}

#[tauri::command]
pub fn close_settings(app: AppHandle) {
    app::close_settings(&app);
}

// --- ダイアログ ---

/// 指定したウィンドウを親にした警告（親の前に出て、閉じるまで親を操作できない）。
fn warn(app: &AppHandle, label: &str, title: &str, message: &str) {
    message_dialog(
        app,
        label,
        title,
        message,
        MessageDialogKind::Warning,
        MessageDialogButtons::Ok,
    );
}

/// はい／いいえの確認。「はい」なら true。
fn ask(app: &AppHandle, label: &str, title: &str, message: &str) -> bool {
    message_dialog(
        app,
        label,
        title,
        message,
        MessageDialogKind::Info,
        MessageDialogButtons::YesNo,
    )
}

fn message_dialog(
    app: &AppHandle,
    label: &str,
    title: &str,
    message: &str,
    kind: MessageDialogKind,
    buttons: MessageDialogButtons,
) -> bool {
    let mut builder = app
        .dialog()
        .message(message)
        .title(title)
        .kind(kind)
        .buttons(buttons);
    if let Some(window) = app.get_webview_window(label) {
        builder = builder.parent(&window);
    }
    builder.blocking_show()
}
