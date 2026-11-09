# DJ Now Playing for VRChat (GUI 版)

**正式名:** DJ Now Playing for VRChat（BOOTH 商品名・配布物表記）  
**アプリ表示名:** Now Playing → VRChat（ウィンドウ・トレイ・設定画面のタイトル）

rekordbox で再生した曲を、VRChat Chatbox へ OSC で送る Windows 常駐 GUI アプリです。
rekordbox の `master.db` を読み取り専用で直接読みます（v2 から。Now Playing App 2 は不要）。
このページは開発者向けです。仕組みの詳細は [`docs/design.ja.md`](../docs/design.ja.md) を参照してください。

## 必要なもの

- Windows 10 / 11（64bit）
- rekordbox 7
- VRChat（OSC 有効）
- ビルドする場合: .NET 10 SDK
- アイコンを生成する場合: [ImageMagick](https://imagemagick.org/)（`magick` に PATH が通っていること）

## ビルド・実行

アイコン (`Assets/app.ico`) は `resources/icon1024.png` から生成します。
`icon1024.png` の元データは `resources/icon.svg` です。PNG を更新したら、次を実行します:

```powershell
.\gui\scripts\generate-app-icon.ps1
```

`build-gui.bat`、`publish-release.ps1`、`publish-installer.ps1` は、ビルドの前にアイコンを自動で生成します。

```powershell
cd gui
dotnet build NowPlayingVrcOsc.slnx
dotnet test NowPlayingVrcOsc.Core.Tests
dotnet run --project NowPlayingVrcOsc.App
```

`gui\scripts\build-gui.bat` を使うと、アイコン生成、実行中のアプリの終了、Debug と Release のビルドをまとめて行います。

テストは Core だけが対象です。実際の `master.db` は使わず、テスト内で作る暗号化 DB と、HTTP の偽装ハンドラーを使います。

## 使い方

1. rekordbox を起動する。
2. 本アプリを起動し、**［開始］** を押す。
3. rekordbox が履歴に曲を記録したタイミングで、VRChat Chatbox に送信される。
   - 起動時に再生中の曲は送信しない。
   - 同じ曲が続けて記録されたときは再送信しない。
4. **［テスト送信］** で VRChat への疎通を確認できる。
5. **［設定...］** で master.db の場所・OSC 送信先・表示項目を変更できる。

- **最小化** → トレイに格納（監視は継続）
- **× ボタン** → アプリ終了
- トレイアイコンの **ダブルクリック** または右クリック →「ウィンドウを表示」で再表示、右クリック →「終了」で終了

### 送信のタイミング

rekordbox は、曲を「再生時間設定」（環境設定 > 詳細 > ブラウズ）の秒数だけ再生してから履歴に書き込みます。
送信はその分遅れます。rekordbox の履歴にはデッキの情報がないため、最後に記録された曲を「再生中」として扱います。

### Spotify の曲

rekordbox 上の Spotify 曲は曲名が暗号化されています。
曲名だけを Spotify の公開 API（oEmbed）から取得して送信します。アーティストは送信しません。
曲名を取得できなかったときは送信せず、状態欄に表示します。

### 復号キー

`master.db` は暗号化されています。復号キーは、公開されている情報から自動取得し（GitHub への通信あり）、
検証してから `%AppData%\NowPlayingVrcOsc\key.dpapi` に暗号化して保存します（Windows の DPAPI）。
自動取得できないときは、設定画面の「復号キー」に 64 桁の 16 進数を入力してください。

## 設定

GUI の **［設定...］** から変更でき、内容は `%AppData%\NowPlayingVrcOsc\settings.json` に保存されます。

| 設定 | 既定 | 説明 |
|---|---|---|
| DbPathOverride | （空） | `master.db` のパス。空欄なら自動検出（rekordboxAgent の `options.json` → `%APPDATA%\Pioneer\rekordbox\master.db`） |
| PollIntervalSeconds | `1.0` | `master.db` の更新確認間隔（秒） |
| VrcHost | `127.0.0.1` | OSC 送信先ホスト |
| VrcPort | `9000` | OSC 送信先ポート |
| ChatboxNotification | `false` | `/chatbox/input` の通知音 |
| ChatboxIndicator | `false` | `/chatbox/typing` インジケーター |
| MsgHeader | `🎵` | Chatbox 先頭のヘッダ（空可） |
| MsgSeparator | `-` | 表示項目の区切り文字（前後にスペースが付く。例: `Title - Artist`） |
| MsgIncludeTitle / Artist / Album / Label / Comment | title と artist のみ ON | 表示する項目 |
| StripTitleBrackets | `true` | 曲名内の `[～]` を除去 |

復号キーは `settings.json` には保存しません。

## インストーラーと自動更新（Velopack）

インストーラー版は `%LocalAppData%\NowPlayingVrcOsc` に入り、管理者権限は不要です。
デスクトップとスタートメニューにショートカットを作ります。

- 起動時に GitHub Releases（`khlizard/nowplaying-vrc-osc`、プレリリースを含む）を確認し、新しい版を裏でダウンロードします。
- 取得するとメイン画面に通知が出ます。アプリを終了すると更新されます。［今すぐ更新して再起動］ですぐ適用することもできます。
- アンインストール時は、設定と復号キー（`%AppData%\NowPlayingVrcOsc`）も削除します。
- ZIP 版と `dotnet run` では更新を確認しません。

作成には vpk（Velopack CLI）が必要です:

```powershell
dotnet tool install -g vpk --version 1.2.161
.\gui\scripts\publish-installer.ps1
```

出力: `dist/velopack/`（`NowPlayingVrcOsc-win-Setup.exe`、`NowPlayingVrcOsc-win-Portable.zip`、`*.nupkg`、`releases.win.json`）

| オプション | 説明 |
|---|---|
| `-Version 2.0.1-beta` | バージョンの上書き（既定は `Directory.Build.props`） |
| `-SkipDownload` | 前回リリースの取得を省略する（差分パッケージを作らない） |
| `-Upload` | GitHub Releases に公開する。トークンは `GITHUB_TOKEN` または `gh auth token` |

`release/notes/<Version>.md` があれば、リリースノートとして使います（任意。無くても作成できます）。
`-Upload` を付けると、タグ `gui-v<Version>` も自動で作られます。

更新の動作は、環境変数 `NPVO_UPDATE_FEED` に `dist/velopack` のようなフォルダを指定すると、GitHub の代わりにそのフォルダで試せます。

## 配布用ビルド（ZIP）

.NET 未インストール環境向けに、ランタイム同梱の ZIP を生成します。

```powershell
.\gui\scripts\publish-release.ps1
```

PowerShell の実行ポリシーで止まる場合は、`gui\scripts\publish-release.bat` から実行できます。

出力: `dist/nowplaying-vrc-osc-gui-v<Version>-win-x64/` と同名の `.zip`

| オプション | 説明 |
|---|---|
| `-Format Single` | 単一 exe（既定） |
| `-Format Folder` | exe + DLL フォルダ形式 |
| `-Version 2.0.0-beta.2` | パッケージ名のバージョン上書き |
| `-SkipZip` | ZIP 作成を省略 |

バージョンは [`Directory.Build.props`](Directory.Build.props) の `<Version>` で管理します。
同梱 README・ライセンス・第三者表記のテンプレートは [`../release/`](../release/) にあります。
BOOTH 公開前の確認項目は [`../release/booth-checklist.ja.md`](../release/booth-checklist.ja.md) を参照してください。
