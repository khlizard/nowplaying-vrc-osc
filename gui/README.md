# DJ Now Playing for VRChat (GUI 版)

**正式名:** DJ Now Playing for VRChat（BOOTH 商品名・配布物表記）  
**アプリ表示名:** Now Playing → VRChat（ウィンドウ・トレイ・設定画面のタイトル）

[Now Playing App 2 (NP2)](../README.md) の再生中の曲を取得し、VRChat Chatbox へ
OSC で送る Windows 常駐 GUI アプリです。Python CLI 版
[`nowplaying-vrc-osc.py`](../nowplaying-vrc-osc.py) を移植したもので、同じ取得・
フォーマット・送信ルールに従います（詳細は
[`docs/nowplaying-vrc-osc-gui/plan.ja.md`](../docs/nowplaying-vrc-osc-gui/plan.ja.md)）。

## 構成

| プロジェクト | TFM | 役割 |
|---|---|---|
| `NowPlayingVrcOsc.Core` | `net10.0` | config.json 監視 / フォーマット / 重複抑制 / OSC 送信 |
| `NowPlayingVrcOsc.App` | `net10.0-windows` (WPF) | トレイ常駐 + ウィンドウ + 設定 UI |

## ビルド・実行

アイコン (`Assets/app.ico`) は `docs/icon1024.png` から生成します。初回または
PNG を更新したとき:

```powershell
.\gui\scripts\generate-app-icon.ps1
```

```powershell
cd gui
dotnet build NowPlayingVrcOsc.slnx
dotnet run --project NowPlayingVrcOsc.App
```

.NET 10 SDK が必要です。

## 使い方

1. NP2 を起動して曲を再生する。
2. 本アプリを起動し、**［開始］** を押すと `config.json` の監視が始まる。
3. 曲が **切り替わった** タイミングで VRChat Chatbox に送信される。
   （起動時に再生中の曲は送信しない＝ Python 版と同じ挙動）
4. **［テスト送信］** で VRChat への疎通を確認できる。
5. **［設定...］** で取得方式・OSC 送信先・表示オプションを変更できる。

- **最小化** → トレイに格納（監視は継続）
- **× ボタン** → アプリ終了

トレイからウィンドウを再表示: トレイアイコン **ダブルクリック** または右クリック →「ウィンドウを表示」。  
トレイから終了: 右クリック →「終了」。

## 設定

GUI の **［設定...］** から変更でき、内容は次に保存されます:

```
%AppData%\NowPlayingVrcOsc\settings.json
```

| 設定 | 既定 | 説明 |
|---|---|---|
| Source | `auto` | `auto`（config 優先→Socket.IO フォールバック） / `config` / `socketio` |
| ConfigPath | `%USERPROFILE%\.nowplaying\config.json` | NP2 の config.json |
| PollIntervalSeconds | `0.5` | 監視ポーリング間隔（秒） |
| NpUrl | （空） | Socket.IO 接続先 URL。空欄なら config.json の `localSocketPort` から自動検出（例: `http://localhost:29000`）。未検出時は `http://localhost:9000` |
| VrcHost | `127.0.0.1` | OSC 送信先ホスト |
| VrcPort | `9000` | OSC 送信先ポート |
| ChatboxNotification | `false` | `/chatbox/input` の通知音 |
| ChatboxIndicator | `false` | `/chatbox/typing` インジケーター |
| MsgHeader | `🎵` | Chatbox 先頭のヘッダ（空可） |
| MsgSeparator | `-` | 表示項目の区切り文字（結合時は前後にスペースが付く。例: `-` → `Title - Artist`） |
| MsgIncludeTitle / Artist / Album / Label / Comment | title+artist のみ ON | 表示する NP2 フィールド |
| StripTitleBrackets | `true` | 曲名 (title) 内の `[～]` を除去 |

## Python 版との挙動一致（parity）

- 取得: `config.json` の `currentTrack` を mtime ポーリング
- 起動時の現在曲は送信しない（初回は baseline 記録のみ）
- 重複抑制: `currentTrack.id` 優先（無ければ `title`）
- Chatbox 文言: ヘッダ + 選択項目を区切りで結合（空項目は省略）
- 既定: `🎵` + `title` + `-` + `artist`（Python の `NP_MSG_*` と同等）
- OSC: `/chatbox/input [text, true, notification]`、`/chatbox/typing [bool]`

## 配布用ビルド（ベータ / BOOTH）

.NET 未インストール環境向けに、ランタイム同梱の ZIP を生成します。

```powershell
.\gui\scripts\publish-release.ps1
# または
.\gui\scripts\publish-release.bat
```

出力:

```
dist/nowplaying-vrc-osc-gui-v<Version>-win-x64/
dist/nowplaying-vrc-osc-gui-v<Version>-win-x64.zip
```

| オプション | 説明 |
|---|---|
| `-Format Single` | 単一 exe（既定） |
| `-Format Folder` | exe + DLL フォルダ形式（WPF で単一 exe に問題がある場合） |
| `-Version 1.0.0-beta.2` | パッケージ名のバージョン上書き |
| `-SkipZip` | ZIP 作成を省略 |

バージョンは [`Directory.Build.props`](Directory.Build.props) の `<Version>` で管理します。
同梱 README・ライセンス（`LICENCE.txt`）・第三者表記のテンプレートは [`../release/`](../release/) にあります。
BOOTH 公開前の確認項目は [`../release/booth-checklist.ja.md`](../release/booth-checklist.ja.md) を参照。

配布前の確認:

1. .NET **未インストール**の PC で ZIP を解凍して `NowPlayingVrcOsc.exe` を起動
2. NP2 + VRChat で曲切替・テスト送信

## 今後の予定

- 送信ログウィンドウ
- 起動時トレイ最小化・Windows スタートアップ登録
