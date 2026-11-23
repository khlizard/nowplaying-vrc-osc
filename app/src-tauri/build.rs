fn main() {
    build_sqlite3mc();
    tauri_build::build();
}

/// 暗号化された rekordbox の master.db を開くため、SQLite3 Multiple Ciphers を静的ライブラリ `sqlite3` としてビルドする。
/// rusqlite（libsqlite3-sys、bundled なし）は `sqlite3` をリンクしに行くので、これで SQLite3MC が使われる。
/// OpenSSL などの外部ツールは要らない。
fn build_sqlite3mc() {
    let dir = sqlite3mc_src::source_dir();
    println!("cargo:rerun-if-changed=build.rs");
    cc::Build::new()
        .file(dir.join(sqlite3mc_src::SOURCE_FILE))
        .include(dir)
        .define("SQLITE_THREADSAFE", "1")
        .define("SQLITE_DQS", "0")
        .define("SQLITE_OMIT_LOAD_EXTENSION", None)
        .warnings(false)
        .compile("sqlite3");
}
