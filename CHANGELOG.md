# 変更履歴

GUI 版（DJ Now Playing for VRChat）の変更点です。インストーラー版の更新の説明にも、この節を使います。

## 未リリース

- rekordbox を直接読めないとき（起動していない、復号キーを取得できない、など）は、Now Playing App 2 が取得した曲を送るようにしました。Serato や Traktor など、rekordbox 以外の DJ ソフトでも使えます。
- 設定画面に「rekordbox を読めないときは Now Playing App から取得する」（既定はオン）と、Now Playing App の `config.json` の場所を加えました。
- アプリ名を「DJ Now Playing for VRChat」に戻しました。

## 3.0.1-beta

- アプリ名を「rekordbox Now Playing for VRChat」に変えました。rekordbox 専用であることが分かる名前にしました（インストーラー版のショートカット名は、更新を引き継ぐため「DJ Now Playing for VRChat」のままです）。
- メイン画面の右上に ☰ メニューを置き、［設定...］をそこへ移しました。
- 「このソフトについて」を加えました。バージョンを確認でき、インストーラー版は［更新を確認］で手動でも更新を確認できます。
- メイン画面を小さくしました。長い曲名は 2 行までで省略します。

## 3.0.0-beta

- アプリを C# / WPF から Tauri（Rust）に作り直しました。機能、画面の項目、設定ファイル（`settings.json`、`key.dpapi`）は 2.0.0-beta と同じです。
- 多重起動を防ぐようにしました。2 つ目を起動すると、起動中のウィンドウが前に出ます。
- ウィンドウ、トレイ、設定画面のタイトルを「DJ Now Playing for VRChat」にそろえました。
- 動作には Microsoft Edge WebView2 ランタイムが必要です（Windows 11 は標準搭載。インストーラー版は、無ければ自動で入れます）。

## 2.0.0-beta

- rekordbox の `master.db` を読み取り専用で直接読むようにしました。Now Playing App は不要になりました。
- Spotify の曲は、曲名を Spotify から取得して送ります。
- Velopack のインストーラーと自動更新に対応しました。

## 1.1.0-beta

- Chatbox のメッセージ（ヘッダ、区切り文字、表示する項目）を設定できるようにしました。設定画面にプレビューを付けました。

## 1.0.0-beta.2

- 最初の配布版です（Now Playing App 2 と連携）。
