# BOOTH 公開前チェックリスト

作者向け。ベータ配布後、BOOTH で ZIP 公開する前に確認してください。

## 配布物

- [x] `.\gui\scripts\publish-release.ps1` で ZIP を生成した
- [ ] .NET **未インストール**の PC で解凍・起動確認した
- [x] NP2 + VRChat で曲切替・テスト送信を確認した
- [ ] 単一 exe で問題があれば `-Format Folder` で再生成した
- [x] `release/README.ja.txt` の「連絡先」を記入した
- [x] `gui/Directory.Build.props` の Company / Copyright を BOOTH 表記と揃えた

## BOOTH 商品ページ

- [ ] 商品名: **DJ Now Playing for VRChat**（アプリ内表示名は Now Playing → VRChat）
- [ ] 概要・非公式ツールである旨の免責
- [ ] 動作環境（Windows 10/11 64bit、.NET 不要）
- [ ] 必要ソフト（NP2、VRChat OSC 有効）
- [ ] 使い方（解凍 → exe 起動 → 開始）
- [ ] SmartScreen 警告への対処方法
- [ ] スクリーンショット（メイン画面・設定・トレイ・Chatbox 表示例）
- [ ] 価格（無料 / 名義価格）
- [ ] バージョン番号の明記

## 法務・ライセンス

- [x] [LICENCE.txt](LICENCE.txt)（MIT License, Copyright tokage works）
- [ ] BOOTH 説明文にライセンス（MIT）への言及
- [x] `THIRD-PARTY-NOTICES.txt` が ZIP に同梱されている（publish スクリプト）
- [x] `LICENCE.txt` が ZIP に同梱されている（publish スクリプト）

## 任意

- [ ] コード署名（SmartScreen 警告の軽減。有料）
- [x] `git tag gui-v1.1.0-beta` でリリースポイントを固定
- [ ] 更新時: Version を上げて ZIP を差し替え

## 継続リリース手順

1. バグ修正・機能追加
2. `gui/Directory.Build.props` の `<Version>` を更新
3. `.\gui\scripts\publish-release.ps1` を実行
4. 動作確認後、BOOTH の ZIP を差し替え
