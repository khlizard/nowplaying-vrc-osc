# nowplaying-vrc-osc

**Now Playing App 2** から再生中の曲情報を取得し、**VRChat** の Chatbox へ OSC で表示する非公式ブリッジです。

> 本プロジェクトは [Now Playing App](https://nowplayingapp.com)（Triode）の公式ツールではありません。

## 概要

```
Now Playing App 2
 ├── config.json (currentTrack)  ── poll（既定）──┐
 └── Socket.IO :<ポート>  ── fallback ──────────┤
                                                 ▼
                                    nowplaying-vrc-osc.py
                                                 │
                                                 ▼  /chatbox/input
                                    VRChat (OSC UDP)
```

DJ ソフト（RekordBox、Serato、Traktor、Pro DJ Link 等）から NP2 が取得した曲名・アーティスト名を、VRC のチャットボックスに `🎵 Now Playing: Title - Artist` 形式で送ります（アーティスト不明時は曲名のみ）。同じ曲が続く間は再送信しません（`currentTrack.id` で判定）。

## 必要環境

- Python 3.10 以上（推奨）
- [Now Playing App 2](https://nowplayingapp.com)
- VRChat（OSC 有効化済み）

## セットアップ

```powershell
python -m venv .venv
.venv\Scripts\Activate.ps1
pip install -r requirements.txt
```

接続先は `.env.example` を参考に環境変数で指定できます（任意）。

| 変数 | 既定値 | 説明 |
|---|---|---|
| `NP_SOURCE` | `auto` | 取得方式: `auto` / `config` / `socketio` |
| `NP_CONFIG_PATH` | `~/.nowplaying/config.json` | NP2 の config.json パス |
| `NP_POLL_INTERVAL` | `0.5` | config.json 監視間隔（秒） |
| `NP_URL` | config から自動 → `http://localhost:9000` | Socket.IO URL（フォールバック時） |
| `VRC_OSC_HOST` | `127.0.0.1` | VRChat OSC 送信先 |
| `VRC_OSC_PORT` | `9000` | VRChat OSC 受信ポート |
| `NP_SHOW_LABEL` | `false` | `label` フィールドの `[～]` を表示（`true` のときのみ） |
| `NP_STRIP_TITLE_BRACKETS` | `true` | 曲名（`title`）内の `[～]` を除去 |

NP2 の Socket.IO ポートは `config.json` の `localSocketPort`、またはアプリ UI / `GET /config` で確認できます。

## VRChat 側の設定

1. アクションメニュー → **Osc** → **Enabled** をオン
2. 外部プログラムからの送信先は既定で **UDP 9000**

詳細は [`docs/vrc_osc_ja/osc-overview.md`](docs/vrc_osc_ja/osc-overview.md) を参照。

## 使い方

### 通常モード（NP2 → VRChat）

```powershell
python nowplaying-vrc-osc.py
```

NP2 を起動し、曲が変わるたびに Chatbox へ送信します。既定では `%USERPROFILE%\.nowplaying\config.json` を監視します。

### テスト: NP2 からの受信のみ

```powershell
python nowplaying-vrc-osc.py --receive-only
```

曲情報をコンソールに表示します。VRChat には送信しません。

### テスト: VRChat への送信のみ

```powershell
python nowplaying-vrc-osc.py --send-only
python nowplaying-vrc-osc.py --send-only -m "接続テスト"
```

NP2 に接続せず、テストメッセージを 1 回送信します。OSC 設定の確認に使えます。

### オプション一覧

```
python nowplaying-vrc-osc.py --help
```

| オプション | 説明 |
|---|---|
| `--receive-only` | NP2 受信テスト |
| `--send-only` | VRChat 送信テスト |
| `-m`, `--message` | `--send-only` 時の送信テキスト |
| `--source` | `auto`（既定）/ `config` / `socketio` |
| `--np-config-path` | config.json のパス |
| `--poll-interval` | config 監視間隔（秒） |
| `--np-url` | Socket.IO URL |
| `--vrc-host` | VRChat OSC ホスト |
| `--vrc-port` | VRChat OSC ポート |
| `--show-label` / `--no-show-label` | `label` フィールドの `[～]` 表示 / 非表示（既定: 非表示） |
| `--strip-title-brackets` / `--no-strip-title-brackets` | 曲名内の `[～]` 除去 / 残す |

## ドキュメント

| パス | 内容 |
|---|---|
| [`docs/now-playing-app/external-connection.ja.md`](docs/now-playing-app/external-connection.ja.md) | NP2 への外部接続（ファイル監視 / Socket.IO API） |
| [`docs/vrc_osc_ja/`](docs/vrc_osc_ja/) | VRChat OSC ドキュメント（日本語） |
| [`docs/vrc_osc/`](docs/vrc_osc/) | VRChat OSC ドキュメント（英語原文） |

## トラブルシューティング

| 症状 | 確認すること |
|---|---|
| 曲が更新されない | NP2 が起動しているか。DJ ソフト側で曲がロード／再生されているか |
| config 監視が動かない | `%USERPROFILE%\.nowplaying\config.json` が存在するか。`--np-config-path` を確認 |
| Socket.IO にフォールバックされる | config.json のパスが正しいか。`--source config` で強制監視を試す |
| VRChat に表示されない | OSC が有効か。`--send-only` で送信テスト |
| ポート不一致 | `config.json` の `localSocketPort` を確認。`--np-url http://localhost:<ポート>` を指定 |

## ライセンス

未設定。利用・再配布する場合は必要に応じてライセンスを追加してください。
