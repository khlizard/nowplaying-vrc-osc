# リリース前チェックリスト

作者向け。新しいバージョンを GitHub Releases（インストーラー）と BOOTH（ZIP）に公開する前に確認する。
リリースのたびにチェックを外してから使う。

## 準備

- [ ] `app/package.json` と `app/src-tauri/Cargo.toml` の `version` を上げた
- [ ] 使い方・設定・通信が変わった場合、`README.md`、`app/README.md`、`docs/design.ja.md`、`release/README.ja.txt` を直した
- [ ] `CHANGELOG.md` に `## <version>` の節を書いた（インストーラーの更新の説明に使う）
- [ ] `cd app; npm run test:rust` と `npm run build` が通った

## 動作確認

- [ ] rekordbox + VRChat で、ローカル曲と Spotify 曲の送信、テスト送信を確認した
- [ ] アプリ起動時に再生中の曲が送られないことを確認した
- [ ] rekordbox を終了したあと、`master.db` のフォルダに `-wal` / `-shm` が残っていないことを確認した

## インストーラー（GitHub Releases）

- [ ] `npm run pack` で Setup.exe を作り、インストール・起動・アンインストールを確認した
- [ ] `NPVO_UPDATE_FEED` に `dist\velopack` を指定し、前の版から新しい版への自動更新を確認した
- [ ] `npm run pack -- --upload` で公開した（タグ `gui-v<version>` は自動で作られる）
- [ ] GitHub Releases のページで、Setup.exe と Portable.zip が添付されていることを確認した

## ZIP（BOOTH）

- [ ] `npm run zip` で ZIP を作った
- [ ] 開発環境の入っていない PC で、解凍・起動を確認した
- [ ] ZIP に `README.ja.txt`、`LICENCE.txt`、`THIRD-PARTY-NOTICES.txt` が入っている
- [ ] BOOTH の ZIP を差し替え、商品ページのバージョン表記を更新した

## ライセンス

- [ ] `LICENCE.txt` と ルートの `LICENSE` が同じ内容で、MIT の本文が崩れていない
- [ ] 依存クレートや npm パッケージを変えた場合、`THIRD-PARTY-NOTICES.txt` を作り直した（`app/README.md` の手順を参照）

## BOOTH 商品ページ（初回、または内容が変わったとき）

- [ ] 商品名: **DJ Now Playing for VRChat**
- [ ] 非公式ツールである旨の免責（rekordbox / AlphaTheta、VRChat、Spotify）
- [ ] 動作環境（Windows 10/11 64bit、WebView2 ランタイム、rekordbox 7、VRChat の OSC 有効）
- [ ] 送信が「再生時間設定」の秒数だけ遅れること、Spotify の曲は曲名だけであること
- [ ] 通信の説明（復号キーの自動取得、Spotify の曲名取得、インストーラー版の更新確認）
- [ ] 使い方（解凍 → exe 起動 → 開始）と、SmartScreen 警告への対処
- [ ] インストーラー版（GitHub Releases）への案内
- [ ] ライセンス（MIT）への言及
- [ ] スクリーンショット（メイン画面・設定・トレイ・Chatbox の表示例）
