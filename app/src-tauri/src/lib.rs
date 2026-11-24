mod app;
pub mod bridge;
mod commands;
pub mod dedup;
pub mod format;
pub mod http;
pub mod nowplaying_app;
pub mod osc;
pub mod rekordbox;
pub mod settings;
pub mod spotify;
pub mod track;
pub mod track_source;
mod updater;

use tauri::{Manager, WindowEvent};

use app::{AppState, MAIN_WINDOW};
use settings::AppSettings;

pub fn run() {
    let result = tauri::Builder::default()
        // 多重起動の検出は、ほかのプラグインより先に登録する必要がある。2 つ目は起動せず、既存のウィンドウを前に出す。
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            app::show_main_window(app)
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle();
            handle.manage(AppState::new(AppSettings::load()));
            app::build_tray(handle)?;
            app::show_main_window(handle);
            app::check_update(handle);
            Ok(())
        })
        // ☰ メニューとトレイのメニューのイベントは、ここ 1 か所で受ける。
        .on_menu_event(app::handle_menu_event)
        .on_window_event(|window, event| {
            if window.label() != MAIN_WINDOW {
                return;
            }
            match event {
                // × で閉じたらアプリを終了する（取得済みの更新は終了後に適用）。
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    app::exit_app(window.app_handle());
                }
                // 最小化したらウィンドウを隠してトレイだけにする。
                WindowEvent::Resized(_) => {
                    if window.is_minimized().unwrap_or(false) {
                        let _ = window.hide();
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_main_state,
            commands::start_bridge,
            commands::stop_bridge,
            commands::test_send,
            commands::show_main_menu,
            commands::apply_update_and_restart,
            commands::get_about_state,
            commands::check_for_updates,
            commands::get_settings,
            commands::get_key_saved,
            commands::resolve_db_path,
            commands::preview_message,
            commands::browse_db_path,
            commands::resolve_np_config_path,
            commands::browse_np_config_path,
            commands::save_settings,
            commands::show_warning,
            commands::close_settings,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("起動できませんでした: {e}");
        std::process::exit(1);
    }
}
