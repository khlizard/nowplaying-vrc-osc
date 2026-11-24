# rekordbox Now Playing for VRChat — 設計メモ（v3）

アプリの仕組みと、変更するときに守ることをまとめた開発者向けの文書です。
使い方は [`README.md`](../README.md)、ビルドと配布は [`app/README.md`](../app/README.md) を参照してください。

## 目的

rekordbox で再生した曲を、VRChat の Chatbox へ OSC で送ります。
rekordbox の `master.db` を読み取り専用で直接読むので、ほかのアプリは要りません。

## 構成

Tauri v2 のアプリです（v2 までは C# / WPF。v3 で移植し、機能と設定ファイルはそのまま引き継いでいます）。

| 場所 | 役割 |
|---|---|
| `app/ui/` | 画面（Vite と TypeScript。メイン画面 `index.html`、設定画面 `settings.html`、このソフトについて `about.html`） |
| `app/src-tauri/src/` の `app.rs`、`commands.rs`、`updater.rs`、`lib.rs`、`main.rs` | アプリ層。ウィンドウ、トレイ、ダイアログ、画面から呼ぶコマンド、自動更新 |
| `app/src-tauri/src/` のそれ以外 | 曲の取得、整形、OSC 送信。Tauri に依存しない |

- 画面は Rust のコマンドを呼び、状態の変化は `main-state`（メイン画面）と `about-state`（このソフトについて）のイベントで受け取ります。画面は受け取った状態を表示するだけで、ボタンを押せるかどうかなどは状態から決めます。
- メイン画面右上の ☰ は、Rust のネイティブのポップアップメニュー（設定...、このソフトについて...）を開きます。メニューのイベントは、トレイのメニューの分も含めて `app.rs` の `handle_menu_event` 1 か所で受けます。
- 設定画面と「このソフトについて」はメイン画面を親にした子ウィンドウです。開いている間はメイン画面を操作できないので、同時には開きません。
- 確認や警告のダイアログは Rust 側から出します。
- 多重起動はできません（`tauri-plugin-single-instance`）。2 つ目を起動すると、起動中のウィンドウが前に出ます。

主な依存クレート:

- rusqlite と、`build.rs` で `sqlite3mc-src` からビルドする SQLite3 Multiple Ciphers（暗号化された `master.db` を開く）
- windows（DPAPI で復号キーを保存する）
- ureq（HTTPS。復号キーの取得と Spotify）
- velopack（インストーラーと自動更新）

OSC の送信は自前で実装しています（`osc.rs`）。

## 処理の流れ

`bridge.rs` のスレッドが、確認の間隔ごとに `rekordbox/source.rs` の `RekordboxTrackSource::tick` を呼びます。受け取った曲は、重複を除いてから整形して送ります。

1. **DB の場所を決める**（`rekordbox/paths.rs`）
   1. 設定の `DbPathOverride`
   2. `%APPDATA%\Pioneer\rekordboxAgent\storage\options.json` の `db-path`
   3. `%APPDATA%\Pioneer\rekordbox\master.db`
2. **rekordbox が起動しているか確かめる。** `master.db-wal` が無ければ未起動とみなし、DB を開きません。読み取り専用で開いても `-wal` と `-shm` が作られて残るためです。
3. **変化があったときだけ読む。** `master.db` と `-wal` の更新時刻とサイズが前回と同じなら開きません。確認の間隔は `PollIntervalSeconds`（既定 1.0 秒、下限 0.2 秒）です。
4. **DB を開く**（`rekordbox/master_db.rs`）。読み取り専用で開き、読み終えるたびに閉じます。
5. **新しい再生を見つける**（`rekordbox/cursor.rs`）。`djmdSongHistory` の rowid をカーソルにします。
   - 起動後と再接続後の最初の読み取りは、基準位置を決めるだけで送りません。
   - 新しい行が複数あるときは、最も新しい 1 行だけを使います。
   - 履歴が削除されて rowid が戻ったときは、送らずに基準を取り直します。
6. **曲情報を作る**（`rekordbox/normalize.rs`）。`djmdContent` と、アーティスト・アルバム・レーベルの各テーブルから `TrackInfo` を作ります。`TrackInfo.Id` は `djmdContent.ID` です。
7. **Spotify の曲名を補う**（`spotify.rs`）。Spotify 由来の曲は曲名が暗号化されています。このときは Spotify の oEmbed から曲名だけを取得します。取得できなければ送らず、状態欄に表示します。
8. **重複を除く**（`dedup.rs`）。ContentID（無ければ曲名）が直前と同じなら送りません。
9. **整形して送る**（`format.rs` → `osc.rs`）。

## 復号キー

`master.db` は暗号化されています。`rekordbox/key_resolver.rs` は次の順にキーを探し、実際に DB を開けたものだけを使います。

1. `%AppData%\NowPlayingVrcOsc\key.dpapi` に保存済みのキー（DPAPI で暗号化）
2. 公開されている情報からの自動取得（GitHub への HTTPS 通信。取得元はコード内の `rekordbox/default-key-sources.json`）
3. 設定画面での手入力（確認してから 1. に保存する）

自動取得が全部失敗したときは、10 分間は再取得しません。
読み取り中に「データベースとして読めない」エラー（`SQLITE_NOTADB`）が 3 回続いたら、キーを探し直します。

## メッセージの組み立て

- 項目は title → artist → album → label → comment の順に並べ、選んだものだけを ` <区切り> ` でつなぎます。
- 先頭に `ヘッダ + 半角スペース` を付けます。ヘッダが空なら本文だけにします。
- 前後の空白は除きます。`none`、`unknown`、`n/a`、`?` は空として扱います。
- `StripTitleBrackets` が有効なら、曲名から `[～]` を取り除きます。`(～)` は残します。
- 既定の出力は `🎵 Title - Artist` です。
- 文字数の切り詰めはしません。

旧設定の `ShowLabel=true` は、読み込み時に `MsgIncludeLabel=true` へ移します。
ラベルを `[label]` の形で付ける方式はありません。

## OSC

| アドレス | 引数 | 送るとき |
|---|---|---|
| `/chatbox/input` | `text`, `true`, `notification` | 曲を送るとき。`notification` は `ChatboxNotification`（既定 false） |
| `/chatbox/typing` | `true` / `false` | `ChatboxIndicator` が true のとき、送信の前後 |

送信先は既定で `127.0.0.1:9000`（UDP）です。

## 設定

`%AppData%\NowPlayingVrcOsc\settings.json` に保存します。v2 と同じ場所・同じキー名（PascalCase）なので、v2 の設定をそのまま読めます。項目と既定値は `settings.rs` の `AppSettings` を参照してください。
読めないときや壊れているときは既定値で起動します。復号キーは `settings.json` には保存しません。

## 自動更新

- Velopack のインストーラー版でだけ動きます。ZIP 版と `npm run tauri dev` では動きません。
- packId（`NowPlayingVrcOsc`）、exe 名（`NowPlayingVrcOsc.exe`）、packTitle（`DJ Now Playing for VRChat`）で、インストール済みの版と新しい版を結び付けます。packTitle は旧名のままで、アプリの表示名（`rekordbox Now Playing for VRChat`）とは違います。
- 起動時に 1 回と、「このソフトについて」の［更新を確認］を押したときに、GitHub Releases（`khlizard/nowplaying-vrc-osc`、プレリリースを含む）を確認し、新しい版があればダウンロードします。
- 確認の状態は `updater.rs` の `UpdateStatus` です（`NotInstalled`、`Idle`、`Checking`、`UpToDate`、`Downloaded`、`Failed`）。確認を始められるのは `Idle`、`UpToDate`、`Failed` のときだけで、確認は同時に 1 つしか走りません（`try_begin_check`）。
- 適用はアプリを終了したとき、またはメイン画面の［更新して再起動］を押したときです。
- 環境変数 `NPVO_UPDATE_FEED` にフォルダを指定すると、そのフォルダを更新元として試せます。
- アンインストール時は `%AppData%\NowPlayingVrcOsc` を削除します。

## 外部との通信

| 通信先 | いつ |
|---|---|
| GitHub（raw.githubusercontent.com） | 保存済みのキーが無く、rekordbox が起動しているとき（復号キーの自動取得） |
| open.spotify.com | 曲名が取れない Spotify 由来の曲が履歴に追加されたとき。送るのはトラック ID だけ |
| GitHub Releases | インストーラー版の起動時と、［更新を確認］を押したとき（更新確認） |
| VRChat（OSC / UDP） | 曲の送信とテスト送信 |

## 変更するときに守ること

- `master.db` に書き込まない。書き込みの SQL も、`rekey`・`journal_mode`・`wal_checkpoint` などの PRAGMA も使わない。
- DB を開くのは `rekordbox/master_db.rs` だけにする。
- 復号キーをログ、画面、例外メッセージに出さない。
- 起動時と再接続時に、その時点の曲を送らない。
- テストに実際の `master.db` を使わない。テストの中で暗号化した DB を作って使う。
- テストで実際の `%AppData%\NowPlayingVrcOsc` を読み書きしない。
- packId、exe 名、packTitle を変えない（インストール済みの版が自動更新されなくなる）。
- cargo を直接実行しない。`npm run tauri` と `npm run test:rust` を使う（[`app/README.md`](../app/README.md)）。

## 制約

- rekordbox は「再生時間設定」（環境設定 > 詳細 > ブラウズ）の秒数だけ再生してから履歴に書きます。送信はその分遅れます。
- 履歴にデッキの情報はありません。最後に記録された曲を再生中とみなします。
- Spotify の曲はアーティストを送りません（oEmbed が返さないため）。
- VRChat の Chatbox は 144 文字までです。アプリ側では切り詰めません。
