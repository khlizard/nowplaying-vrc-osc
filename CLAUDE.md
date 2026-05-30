# CLAUDE.md — nowplaying-vrc-osc

本リポジトリ向けの Claude Code 作業指示です。

## プロジェクト概要

**Now Playing App 2（NP2）** から再生中の曲情報を取得し、**VRChat Chatbox** へ OSC で送る非公式ブリッジ。

| 構成 | パス | 状態 |
|---|---|---|
| Python CLI 版 | `nowplaying-vrc-osc.py` | v1 完成（参照実装） |
| C# Windows GUI 版 | `gui/`（未作成） | **これから開発** |
| ドキュメント | `docs/` | NP2 / VRChat OSC / GUI 計画 |

---

## 最重要: 開発計画の参照

**C# GUI 版の作業を始める前に、必ず次のファイルを読むこと:**

[`docs/nowplaying-vrc-osc-gui/plan.ja.md`](docs/nowplaying-vrc-osc-gui/plan.ja.md)

この計画書が **仕様・フェーズ・アーキテクチャの正** です。本 `CLAUDE.md` は要約と作業ルールのみ。詳細は計画書を参照し、計画書と矛盾する判断をしないこと。

計画書に含まれる主な内容:

- Python 版 v1 からの移植仕様（取得方式、フォーマット、OSC、設定）
- .NET 10 / WPF / Core+App 構成
- Phase 0〜4 の開発手順
- リポジトリ構成、`dotnet new` コマンド
- 別セッション用プロンプト例

---

## 作業スコープの切り分け

### Python CLI 版（`nowplaying-vrc-osc.py`）

- 既存の完成版。**ユーザーが明示的に依頼しない限り変更しない。**
- 挙動の疑問・parity 確認時は **参照実装** として読む。
- 設定の正: [`.env.example`](.env.example)

### C# GUI 版（`gui/`）

- 計画書 §6 のフェーズに従って進める。
- **Phase 1:** Core のみ（UI なし）— config 監視 + フォーマット + OSC
- **Phase 2 以降:** WPF + トレイ + 設定 UI
- 新規コードは `gui/NowPlayingVrcOsc.Core/`（`net10.0`）と `gui/NowPlayingVrcOsc.App/`（`net10.0-windows`）に置く。

---

## 技術スタック（C# GUI 版）

計画書 §3 参照。要点のみ:

| 項目 | 選定 |
|---|---|
| ランタイム | **.NET 10（LTS）** |
| Core TFM | `net10.0` |
| App TFM | `net10.0-windows`（WPF） |
| パターン | MVVM |
| config 監視 | mtime ポーリング（watchdog 不使用） |
| OSC | `OscCore` 等 |
| Socket.IO | Phase 4（`SocketIOClient`） |

---

## parity 確認（Python 版と一致させること）

C# 実装は [`nowplaying-vrc-osc.py`](nowplaying-vrc-osc.py) と **同じ結果** を出すこと。計画書 §2 および以下を特に守る:

1. **取得:** 既定は config.json ポーリング、`auto` で Socket.IO フォールバック
2. **起動時:** 現在再生中の曲は **表示・送信しない**
3. **重複抑制:** `currentTrack.id` 優先
4. **フォーマット:** `Title - Artist`（アーティスト不明時は曲名のみ）
5. **レーベル:** `ShowLabel=false` 既定（`true` のときのみ `[label]`）
6. **曲名内 `[～]`:** `StripTitleBrackets=true` 既定で除去
7. **Chatbox 文言:** `🎵 Now Playing: {format_track の結果}`
8. **OSC:** `/chatbox/input [text, true, notification]`、インジケーターは `/chatbox/typing`
9. **機密:** NP2 `config.json` は `currentTrack` のみ使用。全体をログ出力しない

---

## 関連ドキュメント

| パス | 用途 |
|---|---|
| [`docs/nowplaying-vrc-osc-gui/plan.ja.md`](docs/nowplaying-vrc-osc-gui/plan.ja.md) | **GUI 開発計画（主参照）** |
| [`docs/now-playing-app/external-connection.ja.md`](docs/now-playing-app/external-connection.ja.md) | NP2 接続（ファイル監視 / Socket.IO） |
| [`docs/vrc_osc_ja/osc-as-input-controller.md`](docs/vrc_osc_ja/osc-as-input-controller.md) | Chatbox OSC |
| [`README.md`](README.md) | Python CLI 版の使い方 |

---

## 作業ルール

1. **計画書ファースト** — C# GUI 作業前に `plan.ja.md` を読む。フェーズを飛ばさない。
2. **最小スコープ** — 依頼・フェーズ外の変更はしない。Python 版に不用意な diff を入れない。
3. **既存規約に合わせる** — [.editorconfig](.editorconfig)（Python 4 スペース、Markdown 2 スペース等）に従う。
4. **コミット** — ユーザーが明示的に依頼したときのみ。コミットメッセージは **日本語**。
5. **`.env` をコミットしない** — 機密含む可能性あり。`.env.example` を更新する。
6. **テスト** — Phase 1 完了時点で Python 版と同環境（NP2 起動）での手動 parity 確認を行う。

---

## フェーズ別: 最初にやること

計画書 §11 参照。

### Phase 0（未実施なら）

- Python 版をコミット・`v1.0.0` タグ（ユーザー依頼時）

### Phase 1（GUI 着手の第一歩）

```powershell
cd gui
dotnet new sln -n NowPlayingVrcOsc
dotnet new classlib -n NowPlayingVrcOsc.Core -f net10.0
dotnet new wpf -n NowPlayingVrcOsc.App -f net10.0
dotnet sln add NowPlayingVrcOsc.Core NowPlayingVrcOsc.App
dotnet add NowPlayingVrcOsc.App reference NowPlayingVrcOsc.Core
```

その後、計画書 §4.1 の Core モジュールを実装。

### Phase 2 以降

計画書 §6〜§7 に従う。Phase 1 の parity 確認後に GUI を載せる。

---

## 典型的な依頼例

ユーザーが次のように依頼した場合、**まず `plan.ja.md` を開き、該当フェーズを確認してから実装する:**

- 「Phase 1 の Core を作って」
- 「config.json ポーリングを C# で実装して」
- 「WPF トレイアプリの骨組みを作って」
- 「Python 版と同じフォーマットで OSC 送信して」

---

## リポジトリ構成（目標）

```
now-playing-osc/
├── nowplaying-vrc-osc.py          # Python CLI（触らない）
├── CLAUDE.md                      # 本ファイル
├── docs/
│   └── nowplaying-vrc-osc-gui/
│       └── plan.ja.md             # 開発計画（主参照）
└── gui/                           # C#（これから）
    ├── NowPlayingVrcOsc.sln
    ├── NowPlayingVrcOsc.Core/
    └── NowPlayingVrcOsc.App/
```

---

*詳細仕様・フェーズ・API 設計はすべて [`docs/nowplaying-vrc-osc-gui/plan.ja.md`](docs/nowplaying-vrc-osc-gui/plan.ja.md) を参照すること。*
