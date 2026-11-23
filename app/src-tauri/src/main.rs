// release ビルドではコンソールウィンドウを出さない。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// アンインストール時に、ユーザーデータ（settings.json と key.dpapi）を消す。失敗してもアンインストールは止めない。
fn delete_user_data() {
    let dir = nowplaying_vrc_osc_lib::settings::data_dir();
    if dir.is_dir() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

fn main() {
    // インストール・更新・アンインストール時に Velopack から呼ばれたときの処理。最初に実行する必要がある。
    velopack::VelopackApp::build()
        .on_before_uninstall_fast_callback(|_| delete_user_data())
        .run();
    nowplaying_vrc_osc_lib::run();
}
