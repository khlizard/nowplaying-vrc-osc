# rekordbox Now Playing for VRChat（開発者向け）

アプリ本体のソースです。Tauri v2（Rust）で作っています。画面は `ui/`（Vite と TypeScript）、処理は `src-tauri/src/` にあります。
使い方は [ルートの README](../README.md)、仕組みは [`docs/design.ja.md`](../docs/design.ja.md) を参照してください。

## 必要なもの

- Windows 10 / 11（64bit）と WebView2 ランタイム
- Rust（stable、`x86_64-pc-windows-msvc`）と Visual Studio Build Tools（C++）
- Node.js と npm
- インストーラーを作る場合: vpk 1.2.161（`dotnet tool install -g vpk --version 1.2.161`）

## 準備

```powershell
cd app
npm install
```

Dropbox などの同期フォルダの中で開発するときは、`node_modules` を、中身を入れる前に同期から外します（Dropbox の例）。

```powershell
New-Item -ItemType Directory node_modules
Set-Content -Path node_modules -Stream com.dropbox.ignored -Value 1
```

## ビルドとテスト

cargo は直接実行しません。必ず次の npm スクリプトを使います。
`scripts/with-target.mjs` が、ビルドの出力先をリポジトリの外（同期フォルダの外）にして、exe からユーザー名を含むパスを取り除きます。

```powershell
npm run tauri dev                      # 開発用に起動する
npm run test:rust                      # Rust のテスト
npm run build                          # 画面の型チェックとビルド
npm run tauri build -- --no-bundle     # release の exe を作る
```

ビルドの出力先は `%LOCALAPPDATA%\nowplaying-vrc-osc\target` です（環境変数 `CARGO_TARGET_DIR` があればそちら）。
release の exe は `target\release\NowPlayingVrcOsc.exe` にできます。単体で動きます。

GitHub Actions（`.github/workflows/ci.yml`）でも、main への push と pull request のたびに、画面のビルド、整形の確認、Rust のテストを流します。

テストでは実際の `master.db` を使いません。テストの中で暗号化した DB を作り、HTTP は偽物に差し替えます。

## 配布物を作る

| コマンド | 出力 |
|---|---|
| `npm run zip` | BOOTH 用の ZIP（`dist/nowplaying-vrc-osc-gui-v<version>-win-x64.zip`） |
| `npm run pack` | Velopack のインストーラーと更新パッケージ（`dist/velopack/`） |

どちらも release ビルドから行います。作業は `%LOCALAPPDATA%\nowplaying-vrc-osc` で行い、できたものだけをリポジトリの `dist/` にコピーします。

`npm run zip -- --skip-build` は、ビルド済みの exe を使います。

`npm run pack` のオプション:

| オプション | 説明 |
|---|---|
| `--skip-download` | GitHub Releases から前の版を取得しない（差分パッケージを作らない） |
| `--upload` | GitHub Releases に公開し、タグ `gui-v<version>` を作る。トークンは `GITHUB_TOKEN` か `gh auth token` |

リリースノートには、ルートの `CHANGELOG.md` の `## <version>` の節を使います。節が無いと止まります。

ZIP には `release/` の README.ja.txt（`{{VERSION}}` を置き換える）、LICENCE.txt、THIRD-PARTY-NOTICES.txt を同梱します。
リリース前の確認項目は [`release/booth-checklist.ja.md`](../release/booth-checklist.ja.md) にあります。

## バージョン

`package.json` の `version` が正です。`src-tauri/tauri.conf.json` はこれを参照します。
`src-tauri/Cargo.toml` の `version` は手で合わせます。

## 自動更新を試す

インストーラー版は、起動時に GitHub Releases（プレリリースを含む）を確認します。
環境変数 `NPVO_UPDATE_FEED` に `dist\velopack` のようなフォルダを指定すると、GitHub の代わりにそのフォルダを確認します。

packId（`NowPlayingVrcOsc`）、exe 名（`NowPlayingVrcOsc.exe`）、packTitle は変えないでください。変えると、インストール済みの版が自動更新されなくなります。

## 第三者表記

依存クレートを変えたら、`release/THIRD-PARTY-NOTICES.txt` を作り直します。cargo-about が必要です（`cargo install cargo-about`）。

```powershell
npm run notices
```

Rust のクレートは自動で集めます。それ以外（SQLite3 Multiple Ciphers、`@tauri-apps/api` など）は `scripts/notices/about.hbs` に手で書いてあります。

## アイコン

元データは `resources/icon.svg`、変換に使う画像は `resources/icon1024.png` です。PNG を更新したら、次を実行して `src-tauri/icons/` を作り直します。
Windows 用以外のフォルダ（`android`、`ios` など）と、使わないサイズの PNG は消してかまいません。

```powershell
npx tauri icon ../resources/icon1024.png -o src-tauri/icons
```
