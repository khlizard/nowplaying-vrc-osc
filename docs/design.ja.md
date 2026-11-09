# DJ Now Playing for VRChat — 設計メモ（GUI v2）

GUI 版の仕組みと、変更するときに守ることをまとめた開発者向けの文書です。
使い方は [`README.md`](../README.md)、ビルドと配布は [`gui/README.md`](../gui/README.md) を参照してください。

## 目的

rekordbox で再生した曲を、VRChat の Chatbox へ OSC で送ります。
rekordbox の `master.db` を読み取り専用で直接読むので、ほかのアプリは要りません。

## 構成

| プロジェクト | TFM | 役割 |
|---|---|---|
| `NowPlayingVrcOsc.Core` | `net10.0` | 曲の取得、整形、OSC 送信。UI に依存しない |
| `NowPlayingVrcOsc.App` | `net10.0-windows` | WPF の画面、トレイ常駐、自動更新。コードビハインドで書く（MVVM は使わない） |
| `NowPlayingVrcOsc.Core.Tests` | `net10.0` | Core の xUnit テスト |

ソリューションは `gui/NowPlayingVrcOsc.slnx`、バージョンは `gui/Directory.Build.props` で管理します。

主な依存パッケージ:

- Microsoft.Data.Sqlite.Core と SQLite3MC.PCLRaw.bundle（暗号化された `master.db` を開く）
- System.Security.Cryptography.ProtectedData（復号キーを DPAPI で保存する）
- Velopack（インストーラーと自動更新）

OSC の送信は自前で実装しています（`OscSender`）。

## 処理の流れ

`BridgeService` が `RekordboxTrackSource` から曲を受け取り、重複を除いてから整形して送信します。

1. **DB の場所を決める**（`RekordboxPaths`）
   1. 設定の `DbPathOverride`
   2. `%APPDATA%\Pioneer\rekordboxAgent\storage\options.json` の `db-path`
   3. `%APPDATA%\Pioneer\rekordbox\master.db`
2. **rekordbox が起動しているか確かめる。** `master.db-wal` が無ければ未起動とみなし、DB を開きません。読み取り専用で開いても `-wal` と `-shm` が作られて残るためです。
3. **変化があったときだけ読む。** `master.db` と `-wal` の更新時刻とサイズが前回と同じなら開きません。確認の間隔は `PollIntervalSeconds`（既定 1.0 秒、下限 0.2 秒）です。
4. **DB を開く**（`MasterDb`）。読み取り専用で開き、読み終えるたびに閉じます。
5. **新しい再生を見つける**（`HistoryCursor`）。`djmdSongHistory` の rowid をカーソルにします。
   - 起動後と再接続後の最初の読み取りは、基準位置を決めるだけで送りません。
   - 新しい行が複数あるときは、最も新しい 1 行だけを使います。
   - 履歴が削除されて rowid が戻ったときは、送らずに基準を取り直します。
6. **曲情報を作る。** `djmdContent` と、アーティスト・アルバム・レーベルの各テーブルから `TrackInfo` を作ります。`TrackInfo.Id` は `djmdContent.ID` です。
7. **Spotify の曲名を補う**（`SpotifyOEmbedClient`）。Spotify 由来の曲は曲名が暗号化されています。このときは Spotify の oEmbed から曲名だけを取得します。取得できなければ送らず、状態欄に表示します。
8. **重複を除く**（`TrackDeduplicator`）。ContentID（無ければ曲名）が直前と同じなら送りません。
9. **整形して送る**（`TrackFormatter` → `VrcChatboxSender`）。

## 復号キー

`master.db` は暗号化されています。`KeyResolver` は次の順にキーを探し、実際に DB を開けたものだけを使います。

1. `%AppData%\NowPlayingVrcOsc\key.dpapi` に保存済みのキー（DPAPI で暗号化）
2. 公開されている情報からの自動取得（GitHub への HTTPS 通信。取得元はコード内の `Rekordbox/default-key-sources.json`）
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

`%AppData%\NowPlayingVrcOsc\settings.json` に保存します。項目と既定値は `AppSettings` と [`gui/README.md`](../gui/README.md) の表を参照してください。
読めないときや壊れているときは既定値で起動します。復号キーは `settings.json` には保存しません。

## 自動更新

- Velopack のインストーラー版でだけ動きます。ZIP 版と `dotnet run` では動きません。
- 起動時に 1 回、GitHub Releases（`khlizard/nowplaying-vrc-osc`、プレリリースを含む）を確認し、新しい版があればダウンロードします。
- 適用はアプリを終了したとき、または［今すぐ更新して再起動］を押したときです。
- 環境変数 `NPVO_UPDATE_FEED` にフォルダを指定すると、そのフォルダを更新元として試せます。
- アンインストール時は `%AppData%\NowPlayingVrcOsc` を削除します。

## 外部との通信

| 通信先 | いつ |
|---|---|
| GitHub（raw.githubusercontent.com） | 保存済みのキーが無く、rekordbox が起動しているとき（復号キーの自動取得） |
| open.spotify.com | 曲名が取れない Spotify 由来の曲が履歴に追加されたとき。送るのはトラック ID だけ |
| GitHub Releases | インストーラー版の起動時（更新確認） |
| VRChat（OSC / UDP） | 曲の送信とテスト送信 |

## 変更するときに守ること

- `master.db` に書き込まない。書き込みの SQL も、`rekey`・`journal_mode`・`wal_checkpoint` などの PRAGMA も使わない。
- DB を開くのは `MasterDb` だけにする。
- 復号キーをログ、画面、例外メッセージに出さない。
- 起動時と再接続時に、その時点の曲を送らない。
- テストに実際の `master.db` を使わない。`NowPlayingVrcOsc.Core.Tests` は、テスト内で作る暗号化 DB を使う。

## 制約

- rekordbox は「再生時間設定」（環境設定 > 詳細 > ブラウズ）の秒数だけ再生してから履歴に書きます。送信はその分遅れます。
- 履歴にデッキの情報はありません。最後に記録された曲を再生中とみなします。
- Spotify の曲はアーティストを送りません（oEmbed が返さないため）。
- VRChat の Chatbox は 144 文字までです。アプリ側では切り詰めません。
