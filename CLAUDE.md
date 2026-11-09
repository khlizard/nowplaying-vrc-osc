# CLAUDE.md — nowplaying-vrc-osc

本リポジトリ向けの Claude Code 作業指示です。

## プロジェクト概要

**DJ Now Playing for VRChat**: rekordbox で再生した曲を、VRChat の Chatbox へ OSC で送る Windows アプリ。
rekordbox の `master.db` を読み取り専用で直接読む。

| 構成 | パス | 状態 |
|---|---|---|
| GUI 版（C# / WPF） | `gui/` | v2.0.0-beta。主に開発する対象 |
| 旧 CLI 版（Python） | `nowplaying-vrc-osc.py` | Now Playing App 2 連携。保守のみ |
| 設計メモ | `docs/design.ja.md` | GUI 版の仕組みと、守ること |

## 仕様の正

1. 実装（`gui/`）
2. [`docs/design.ja.md`](docs/design.ja.md)

GUI 版の作業を始める前に `docs/design.ja.md` を読む。設計を変えたら同じ作業の中で design を直す。

## 作業スコープ

- **GUI 版**: 新規コードは `gui/NowPlayingVrcOsc.Core/`（UI に依存しない処理）、`gui/NowPlayingVrcOsc.App/`（画面・トレイ・更新）、`gui/NowPlayingVrcOsc.Core.Tests/` に置く。
- **旧 CLI 版**: ユーザーが明示的に依頼しない限り変更しない。設定の正は [`.env.example`](.env.example)、説明は [`docs/python-cli.ja.md`](docs/python-cli.ja.md)。
- `refs/` は外部の参照ドキュメント（VRChat OSC、Now Playing App など）のローカル置き場。Git 管理外で、再配布しない。

## 技術スタック（GUI 版）

| 項目 | 内容 |
|---|---|
| ランタイム | .NET 10。Core / Tests は `net10.0`、App は `net10.0-windows`（WPF。トレイに WinForms を使う） |
| UI | コードビハインド（MVVM は使わない） |
| DB | Microsoft.Data.Sqlite.Core + SQLite3MC.PCLRaw.bundle（読み取り専用） |
| キー保存 | System.Security.Cryptography.ProtectedData（DPAPI） |
| Spotify 曲名 | oEmbed（`Core/Spotify/`） |
| OSC | 自前実装（`OscSender`） |
| 更新・インストーラー | Velopack（vpk 1.2.161） |
| テスト | xUnit |
| バージョン | `gui/Directory.Build.props` の `<Version>` |

## 守ること

詳しくは `docs/design.ja.md` の「変更するときに守ること」。

1. `master.db` に書き込まない。書き込みの SQL や、`rekey`・`journal_mode`・`wal_checkpoint` などの PRAGMA を使わない。DB を開くのは `MasterDb` だけ。
2. 起動時と再接続時に、その時点の曲を送らない。
3. 重複抑制は ContentID（`TrackInfo.Id`）優先、無ければ曲名。
4. 既定のメッセージは `🎵 Title - Artist`（ヘッダ `🎵`、区切り `-`、項目は title と artist）。曲名内の `[～]` は既定で除く。
5. OSC は `/chatbox/input [text, true, notification]`。インジケーターは `/chatbox/typing`。
6. 復号キーをログ、画面、例外メッセージに出さない。`settings.json` に保存しない。
7. テストに実際の `master.db` を使わない。
8. 新しい外部通信を足したら、README.md、docs/design.ja.md、release/README.ja.txt の通信の説明も直す。

## コマンド

作業ディレクトリは `gui/`。

```powershell
dotnet build NowPlayingVrcOsc.slnx
dotnet test NowPlayingVrcOsc.Core.Tests
dotnet run --project NowPlayingVrcOsc.App
```

| 用途 | コマンド（リポジトリのルートから） |
|---|---|
| アイコン生成（ImageMagick が必要） | `.\gui\scripts\generate-app-icon.ps1` |
| アイコン生成 + Debug/Release ビルド | `gui\scripts\build-gui.bat` |
| 配布 ZIP（BOOTH 用） | `.\gui\scripts\publish-release.ps1` |
| インストーラー（GitHub Releases 用） | `.\gui\scripts\publish-installer.ps1`（公開は `-Upload`） |

`publish-installer.ps1 -Upload` は GitHub Releases に公開し、タグも作る。ユーザーが明示的に依頼したときだけ実行する。
リリース時の確認項目は [`release/booth-checklist.ja.md`](release/booth-checklist.ja.md)。

## リポジトリ構成

```
├── README.md                    # 利用者向け（GUI 版）
├── CLAUDE.md                    # 本ファイル
├── LICENSE                      # MIT（release/LICENCE.txt と同じ内容）
├── docs/
│   ├── design.ja.md             # GUI 版の設計メモ
│   └── python-cli.ja.md         # 旧 CLI 版の説明
├── gui/
│   ├── NowPlayingVrcOsc.slnx
│   ├── Directory.Build.props    # バージョン・製品名
│   ├── NowPlayingVrcOsc.Core/   # Rekordbox/, Spotify/, 整形, OSC
│   ├── NowPlayingVrcOsc.App/    # WPF, トレイ, AppUpdater
│   ├── NowPlayingVrcOsc.Core.Tests/
│   ├── scripts/                 # ビルド・配布スクリプト
│   └── README.md                # 開発者向け
├── release/                     # 配布物に同梱する README・ライセンス・第三者表記、リリース手順
├── resources/                   # アイコン（icon.svg が元データ、icon1024.png を生成に使う）
├── nowplaying-vrc-osc.py        # 旧 CLI 版
├── .env.example, requirements.txt  # 旧 CLI 版用
└── refs/                        # 外部ドキュメント（Git 管理外）
```

## 作業ルール

1. **最小スコープ**: 依頼の範囲外は変更しない。旧 CLI 版に不用意な差分を入れない。
2. **既存の規約に合わせる**: [.editorconfig](.editorconfig) に従う。
3. **ドキュメントの同期**: 使い方・設定・コマンドが変わったら、README.md、gui/README.md、docs/design.ja.md、release/README.ja.txt のうち該当するものを同じ作業の中で直す。
4. **コミット**: ユーザーが明示的に依頼したときだけ行う。メッセージは日本語。
5. **機密**: `.env`、`key.dpapi`、`settings.json`、実際の `master.db`、復号キーをコミットしない。設定例は `.env.example` を更新する。
6. **検証**: GUI を変更したら `dotnet build` と `dotnet test` を通す。rekordbox や VRChat が必要な動作確認は、実施できなかったら完了報告で明記する。
