//! アプリ全体の状態と、監視・ウィンドウ・トレイの操作（C# の MainWindow のコードビハインドに相当）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, LogicalPosition, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt as _, MessageDialogKind};

use crate::bridge::{Bridge, BridgeEvent};
use crate::settings::AppSettings;
use crate::updater::{UpdateStatus, Updater};

pub const PRODUCT_NAME: &str = "rekordbox Now Playing for VRChat";
pub const MAIN_WINDOW: &str = "main";
pub const SETTINGS_WINDOW: &str = "settings";
pub const ABOUT_WINDOW: &str = "about";
/// メイン画面を親にして開く子ウィンドウ。開いている間はメイン画面を操作できない。
const CHILD_WINDOWS: [&str; 2] = [SETTINGS_WINDOW, ABOUT_WINDOW];
const TRAY_ID: &str = "main";

// メニューの項目の ID。メニューのイベントは、トレイの分も含めて `handle_menu_event` にすべて届く。
const MENU_SHOW: &str = "show";
const MENU_QUIT: &str = "quit";
const MENU_SETTINGS: &str = "menu-settings";
const MENU_ABOUT: &str = "menu-about";

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
    pub running: bool,
    pub status: String,
    pub track: String,
    pub last_sent: String,
    pub update_version: Option<String>,
}

/// 「このソフトについて」に出す内容。変わるたびに全体を `about-state` で送る。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutState {
    pub product_name: String,
    pub version: String,
    pub update: UpdateStatus,
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
        running: view.bridge.is_some(),
        status: view.status.clone(),
        track: view.track.clone(),
        last_sent: view.last_sent.clone(),
        update_version: state.updater.pending_version(),
    }
}

pub fn about_state(app: &AppHandle) -> AboutState {
    AboutState {
        product_name: PRODUCT_NAME.into(),
        version: format!("v{}", app.package_info().version),
        update: app.state::<AppState>().updater.status(),
    }
}

pub fn refresh_ui(app: &AppHandle) {
    let _ = app.emit_to(MAIN_WINDOW, "main-state", main_state(app));
    if app.get_webview_window(ABOUT_WINDOW).is_some() {
        let _ = app.emit_to(ABOUT_WINDOW, "about-state", about_state(app));
    }
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

/// 更新を確認する（起動時と、「このソフトについて」の［更新を確認］）。
/// 確認中・取得済み・未インストールのときは何もしない。結果は画面に知らせる。
pub fn check_update(app: &AppHandle) {
    let updater = app.state::<AppState>().updater.clone();
    if !updater.try_begin_check() {
        return;
    }
    refresh_ui(app);
    let handle = app.clone();
    std::thread::spawn(move || {
        updater.check_and_download();
        if !handle.state::<AppState>().exiting.load(Ordering::SeqCst) {
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
    // 設定画面などが開いていれば、そちらを前に出す。
    for label in CHILD_WINDOWS {
        if let Some(child) = app.get_webview_window(label) {
            let _ = child.set_focus();
        }
    }
}

/// 設定画面を開く。開いている間はメイン画面を操作できないようにする。
pub fn open_settings(app: &AppHandle) -> Result<(), String> {
    open_child_window(
        app,
        SETTINGS_WINDOW,
        "settings.html",
        format!("設定 — {PRODUCT_NAME}"),
        (580.0, 720.0),
    )
    .map_err(|e| format!("設定画面を開けません: {e}"))
}

/// 「このソフトについて」を開く。開いている間はメイン画面を操作できないようにする。
pub fn open_about(app: &AppHandle) -> Result<(), String> {
    open_child_window(
        app,
        ABOUT_WINDOW,
        "about.html",
        format!("このソフトについて — {PRODUCT_NAME}"),
        (380.0, 160.0),
    )
    .map_err(|e| format!("「このソフトについて」を開けません: {e}"))
}

/// メイン画面を親にした子ウィンドウを開く。既に開いていれば前に出す。
/// 開いている間はメイン画面を無効にし、閉じたら戻す。
fn open_child_window(
    app: &AppHandle,
    label: &str,
    url: &str,
    title: String,
    (width, height): (f64, f64),
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.set_focus();
        return Ok(());
    }
    let main = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or("メイン画面がありません。")?;
    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .title(title)
        .inner_size(width, height)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .parent(&main)
        .map_err(|e| e.to_string())?
        .center()
        .build()
        .map_err(|e| e.to_string())?;
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

/// メイン画面の ☰ メニューを、ウィンドウの左上からの位置（論理座標）に開く。
pub fn show_main_menu(app: &AppHandle, x: f64, y: f64) -> Result<(), String> {
    let main = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or("メイン画面がありません。")?;
    let build = || -> tauri::Result<Menu<tauri::Wry>> {
        let settings = MenuItem::with_id(app, MENU_SETTINGS, "設定...", true, None::<&str>)?;
        let about =
            MenuItem::with_id(app, MENU_ABOUT, "このソフトについて...", true, None::<&str>)?;
        Menu::with_items(app, &[&settings, &about])
    };
    let menu = build().map_err(|e| e.to_string())?;
    main.popup_menu_at(&menu, LogicalPosition::new(x, y))
        .map_err(|e| e.to_string())
}

/// メニュー（☰ とトレイ）で選ばれた項目を処理する。アプリに 1 つだけ登録する。
pub fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        MENU_SHOW => show_main_window(app),
        MENU_QUIT => exit_app(app),
        // ウィンドウはメインスレッドのハンドラの中で作ると固まることがあるので、別のタスクで開く。
        MENU_SETTINGS => spawn_open(app, open_settings),
        MENU_ABOUT => spawn_open(app, open_about),
        _ => {}
    }
}

fn spawn_open(app: &AppHandle, open: fn(&AppHandle) -> Result<(), String>) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = open(&handle) {
            let mut dialog = handle
                .dialog()
                .message(e)
                .title(PRODUCT_NAME)
                .kind(MessageDialogKind::Warning);
            if let Some(main) = handle.get_webview_window(MAIN_WINDOW) {
                dialog = dialog.parent(&main);
            }
            dialog.show(|_| {});
        }
    });
}

pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, MENU_SHOW, "ウィンドウを表示", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "終了", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &separator, &quit])?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(PRODUCT_NAME)
        .menu(&menu)
        .show_menu_on_left_click(false)
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
