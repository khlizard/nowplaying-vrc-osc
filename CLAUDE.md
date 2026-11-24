# CLAUDE.md — nowplaying-vrc-osc

本リポジトリ向けの Claude Code 作業指示です。

## プロジェクト概要

**DJ Now Playing for VRChat**: rekordbox で再生した曲を、VRChat の Chatbox へ OSC で送る Windows アプリ。
rekordbox の `master.db` を読み取り専用で直接読む。読めないときは Now Playing App 2 の `config.json`（`currentTrack`）から読む。

| 構成 | パス | 状態 |
|---|---|---|
| アプリ（Tauri v2 / Rust） | `app/` | v3.0.1-beta。主に開発する対象 |
| 設計メモ | `docs/design.ja.md` | アプリの仕組みと、守ること |

v2（C# / WPF、`gui/`）は v3 で Tauri に移植して削除した。C# 版は Git 履歴に残っている。設定ファイルと `key.dpapi` は C# 版と互換。
旧 CLI 版（Python、Now Playing App 2 連携）も削除した。Git 履歴（タグ `v1.0.0` など）に残っている。

## 仕様の正

1. 実装（`app/`）
2. [`docs/design.ja.md`](docs/design.ja.md)

アプリの作業を始める前に `docs/design.ja.md` を読む。設計を変えたら同じ作業の中で design を直す。

## 作業スコープ

- **アプリ**: 画面は `app/ui/`、Rust は `app/src-tauri/src/`。Tauri に依存しない処理（rekordbox、整形、OSC など）は Tauri の型を使わずに書き、アプリ層（`app.rs`、`commands.rs`、`updater.rs`、`lib.rs`、`main.rs`）から呼ぶ。
- `refs/` は外部の参照ドキュメント（VRChat OSC など）のローカル置き場。Git 管理外で、再配布しない。

## 技術スタック（アプリ）

| 項目 | 内容 |
|---|---|
| 本体 | Tauri v2（Rust）。プラグインは single-instance と dialog |
| UI | Vite + TypeScript（フレームワークなし）。`ui/index.html`（メイン）、`ui/settings.html`（設定）、`ui/about.html`（このソフトについて） |
| DB | rusqlite + SQLite3 Multiple Ciphers（`build.rs` で `sqlite3mc-src` をビルド。読み取り専用） |
| キー保存 | DPAPI（windows クレート）。v2 の `key.dpapi` と互換 |
| HTTP | ureq（https のみ、5 秒、1 MiB 上限） |
| Spotify 曲名 | oEmbed（`spotify.rs`） |
| OSC | 自前実装（`osc.rs`） |
| 更新・インストーラー | Velopack（クレート `velopack =1.2.161`、vpk 1.2.161） |
| テスト | `cargo test`（`npm run test:rust` 経由） |
| バージョン | `app/package.json` の `version`（`Cargo.toml` は手で合わせる） |

## 守ること

詳しくは `docs/design.ja.md` の「変更するときに守ること」。

1. `master.db` に書き込まない。書き込みの SQL や、`rekey`・`journal_mode`・`wal_checkpoint` などの PRAGMA を使わない。DB を開くのは `rekordbox/master_db.rs` だけ。
2. 起動時と再接続時、取得元（rekordbox / Now Playing App）を切り替えたときに、その時点の曲を送らない。
3. 重複抑制は ContentID（`TrackInfo.id`）優先、無ければ曲名。
4. 既定のメッセージは `🎵 Title - Artist`（ヘッダ `🎵`、区切り `-`、項目は title と artist）。曲名内の `[～]` は既定で除く。
5. OSC は `/chatbox/input [text, true, notification]`。インジケーターは `/chatbox/typing`。
6. 復号キーをログ、画面、エラーメッセージに出さない。`settings.json` に保存しない。Now Playing App の `config.json` は `currentTrack` 以外を読まず、中身を出さず、書き込まない（設定やトークンが入っている）。
7. テストに実際の `master.db` を使わない。テストで実際の `%APPDATA%\NowPlayingVrcOsc` と、実際の Now Playing App の `config.json` を読み書きしない。
8. 新しい外部通信を足したら、README.md、docs/design.ja.md、release/README.ja.txt の通信の説明も直す。
9. Velopack の packId `NowPlayingVrcOsc`、exe 名 `NowPlayingVrcOsc.exe`、packTitle `DJ Now Playing for VRChat` を変えない（インストール済みの版が自動更新されなくなる）。

## コマンド

作業ディレクトリは `app/`。**cargo を直接実行しない**（`scripts/with-target.mjs` が、ビルド先を Dropbox の外の `%LOCALAPPDATA%\nowplaying-vrc-osc\target` にし、exe からユーザー名を除く）。
`src-tauri/target` を Dropbox 内に作らない。

```powershell
npm run test:rust                      # Rust のテスト
npm run build                          # 画面の型チェックとビルド
npm run tauri dev                      # 開発用に起動
npm run tauri build -- --no-bundle     # release の exe
```

| 用途 | コマンド（`app/` で） |
|---|---|
| 配布 ZIP（BOOTH 用） | `npm run zip` |
| インストーラー（GitHub Releases 用） | `npm run pack`（公開は `-- --upload`） |
| アイコン生成 | `npx tauri icon ../resources/icon1024.png -o src-tauri/icons` |

`npm run pack -- --upload` は GitHub Releases に公開し、タグも作る。ユーザーが明示的に依頼したときだけ実行する。
リリース時の確認項目は [`release/booth-checklist.ja.md`](release/booth-checklist.ja.md)。

## リポジトリ構成

```
├── README.md                    # 利用者向け
├── CLAUDE.md                    # 本ファイル
├── LICENSE                      # MIT（release/LICENCE.txt と同じ内容）
├── docs/
│   ├── design.ja.md             # アプリの設計メモ
│   └── images/                  # README の画面写真
├── app/
│   ├── package.json             # バージョンの正
│   ├── scripts/                 # with-target.mjs, pack.mjs, zip.mjs
│   ├── ui/                      # 画面（Vite + TypeScript）
│   ├── src-tauri/               # Rust（Cargo.toml, build.rs, tauri.conf.json, src/）
│   └── README.md                # 開発者向け
├── release/                     # 配布物に同梱する README・ライセンス・第三者表記、リリース手順
├── resources/                   # アイコン（icon.svg が元データ、icon1024.png を生成に使う）
└── refs/                        # 外部ドキュメント（Git 管理外）
```

## 作業ルール

1. **最小スコープ**: 依頼の範囲外は変更しない。
2. **既存の規約に合わせる**: [.editorconfig](.editorconfig) に従う。
3. **ドキュメントの同期**: 使い方・設定・コマンドが変わったら、README.md、app/README.md、docs/design.ja.md、release/README.ja.txt のうち該当するものを同じ作業の中で直す。
4. **コミット**: ユーザーが明示的に依頼したときだけ行う。メッセージは日本語。
5. **機密**: `key.dpapi`、`settings.json`、実際の `master.db`、復号キーをコミットしない。
6. **検証**: アプリを変更したら `npm run test:rust` と `npm run build` を通す。rekordbox や VRChat が必要な動作確認は、実施できなかったら完了報告で明記する。
