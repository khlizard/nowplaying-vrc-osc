//! アプリ全体の状態と、監視・ウィンドウ・トレイの操作（C# の MainWindow のコードビハインドに相当）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::bridge::{Bridge, BridgeEvent};
use crate::settings::AppSettings;
use crate::updater::Updater;

pub const PRODUCT_NAME: &str = "rekordbox Now Playing for VRChat";
pub const MAIN_WINDOW: &str = "main";
pub const SETTINGS_WINDOW: &str = "settings";
const TRAY_ID: &str = "main";

const STATUS_STOPPED: &str = "停止中";
const STATUS_CONNECTING: &str = "接続中…";

pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// メイン画面に出す内容。変わるたびに全体を `main-state` で送る。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainState {
    pub version: String,
    pub running: bool,
    pub status: String,
    pub track: String,
    pub last_sent: String,
    pub update_version: Option<String>,
}

struct View {
    status: String,
    track: String,
    last_sent: String,
    bridge: Option<Bridge>,
    /// 監視を始めるたび・止めるたびに増やす。古い監視からの知らせを捨てるのに使う。
    generation: u64,
}

pub struct AppState {
    pub settings: Mutex<AppSettings>,
    view: Mutex<View>,
    pub updater: Arc<Updater>,
    exiting: AtomicBool,
}

impl AppState {
    pub fn new(settings: AppSettings) -> Self {
        Self {
            settings: Mutex::new(settings),
            view: Mutex::new(View {
                status: STATUS_STOPPED.into(),
                track: String::new(),
                last_sent: String::new(),
                bridge: None,
                generation: 0,
            }),
            updater: Arc::new(Updater::new()),
            exiting: AtomicBool::new(false),
        }
    }

    pub fn settings_snapshot(&self) -> AppSettings {
        lock(&self.settings).clone()
    }

    pub fn is_running(&self) -> bool {
        lock(&self.view).bridge.is_some()
    }
}

pub fn main_state(app: &AppHandle) -> MainState {
    let state = app.state::<AppState>();
    let view = lock(&state.view);
    MainState {
        version: format!("v{}", app.package_info().version),
        running: view.bridge.is_some(),
        status: view.status.clone(),
        track: view.track.clone(),
        last_sent: view.last_sent.clone(),
        update_version: state.updater.pending_version(),
    }
}

pub fn refresh_ui(app: &AppHandle) {
    let _ = app.emit_to(MAIN_WINDOW, "main-state", main_state(app));
}

fn now_hms() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

/// 監視を始める。動いていれば何もしない。
pub fn start_bridge(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let settings = state.settings_snapshot();
    let generation = {
        let mut view = lock(&state.view);
        if view.bridge.is_some() {
            return Ok(());
        }
        view.generation += 1;
        view.status = STATUS_CONNECTING.into();
        view.generation
    };
    refresh_ui(app);

    let handle = app.clone();
    let result = Bridge::start(settings, move |event| {
        on_bridge_event(&handle, generation, event)
    });
    {
        let mut view = lock(&state.view);
        match result {
            Ok(bridge) if view.generation == generation => view.bridge = Some(bridge),
            // 始める途中で止められた。
            Ok(bridge) => bridge.stop(),
            Err(e) => {
                view.status = STATUS_STOPPED.into();
                drop(view);
                refresh_ui(app);
                return Err(format!("監視を開始できませんでした: {e}"));
            }
        }
    }
    refresh_ui(app);
    Ok(())
}

pub fn stop_bridge(app: &AppHandle) {
    let state = app.state::<AppState>();
    {
        let mut view = lock(&state.view);
        view.generation += 1;
        if let Some(bridge) = view.bridge.take() {
            bridge.stop();
        }
        view.status = STATUS_STOPPED.into();
    }
    refresh_ui(app);
}

/// 設定を変えたとき、監視中なら新しい設定でやり直す。
pub fn restart_bridge_if_running(app: &AppHandle) -> Result<(), String> {
    if !app.state::<AppState>().is_running() {
        return Ok(());
    }
    stop_bridge(app);
    start_bridge(app)
}

fn on_bridge_event(app: &AppHandle, generation: u64, event: BridgeEvent) {
    let state = app.state::<AppState>();
    {
        let mut view = lock(&state.view);
        // 止めた後や、やり直した後に古い監視から届いたものは捨てる。
        if view.generation != generation {
            return;
        }
        match event {
            BridgeEvent::Status(s) => view.status = s,
            BridgeEvent::Track(t) => view.track = t,
            BridgeEvent::Sent(m) => view.last_sent = format!("{}  {m}", now_hms()),
        }
    }
    refresh_ui(app);
}

/// テスト送信。成功したら最終送信欄の文字列を返す。
pub fn test_send(app: &AppHandle, message: &str) -> Result<String, String> {
    let state = app.state::<AppState>();
    let s = state.settings_snapshot();
    let sender = crate::osc::VrcChatboxSender::new(
        &s.vrc_host,
        s.vrc_port,
        s.chatbox_notification,
        s.chatbox_indicator,
    )
    .map_err(|e| e.to_string())?;
    sender.send(message).map_err(|e| e.to_string())?;
    let line = format!("{}  (テスト) {message}", now_hms());
    lock(&state.view).last_sent = line.clone();
    refresh_ui(app);
    Ok(line)
}

/// 起動後に一度、更新を確認する。取得できたら画面に知らせる。
pub fn check_update_in_background(app: &AppHandle) {
    let handle = app.clone();
    let updater = app.state::<AppState>().updater.clone();
    std::thread::spawn(move || {
        if updater.check_and_download().is_some()
            && !handle.state::<AppState>().exiting.load(Ordering::SeqCst)
        {
            refresh_ui(&handle);
        }
    });
}

/// 監視を止めて終了する。取得済みの更新があれば終了後に適用する（再起動はしない）。
pub fn exit_app(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.exiting.swap(true, Ordering::SeqCst) {
        return;
    }
    stop_bridge(app);
    let _ = app.remove_tray_by_id(TRAY_ID);
    state.updater.apply_on_exit();
    app.exit(0);
}

/// 監視を止めて更新を適用し、再起動する。
pub fn apply_update_and_restart(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.exiting.swap(true, Ordering::SeqCst) {
        return;
    }
    stop_bridge(app);
    let _ = app.remove_tray_by_id(TRAY_ID);
    state.updater.apply_and_restart();
    // 成功するとプロセスは上で終わる。戻ってきたら適用に失敗しているので、
    // 監視もトレイも止まったまま残さないよう、通常どおり終了する（更新は次回の起動時に適用される）。
    app.exit(0);
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    // 設定画面が開いていれば、そちらを前に出す。
    if let Some(settings) = app.get_webview_window(SETTINGS_WINDOW) {
        let _ = settings.set_focus();
    }
}

/// 設定画面を開く。開いている間はメイン画面を操作できないようにする。
pub fn open_settings(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW) {
        let _ = window.set_focus();
        return Ok(());
    }
    let main = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or("メイン画面がありません。")?;
    let window = WebviewWindowBuilder::new(
        app,
        SETTINGS_WINDOW,
        WebviewUrl::App("settings.html".into()),
    )
    .title(format!("設定 — {PRODUCT_NAME}"))
    .inner_size(580.0, 720.0)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .parent(&main)
    .map_err(|e| format!("設定画面を開けません: {e}"))?
    .center()
    .build()
    .map_err(|e| format!("設定画面を開けません: {e}"))?;
    let _ = main.set_enabled(false);
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            if let Some(main) = handle.get_webview_window(MAIN_WINDOW) {
                let _ = main.set_enabled(true);
                let _ = main.set_focus();
            }
        }
    });
    Ok(())
}

pub fn close_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW) {
        let _ = window.close();
    }
}

pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "ウィンドウを表示", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &separator, &quit])?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(PRODUCT_NAME)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "quit" => exit_app(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
