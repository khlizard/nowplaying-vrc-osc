# nowplaying-vrc-osc

**Now Playing App 2** から再生中の曲情報を取得し、**VRChat** の Chatbox へ OSC で表示する非公式ブリッジです。

> 本プロジェクトは [Now Playing App](https://nowplayingapp.com)（Triode）の公式ツールではありません。

## 概要

```
Now Playing App 2 (Socket.IO)
        │
        ▼  trackUpdate
  nowplaying-vrc-osc.py
        │
        ▼  /chatbox/input
  VRChat (OSC UDP)
```

DJ ソフト（RekordBox、Serato、Traktor、Pro DJ Link 等）から NP2 が取得した曲名・アーティスト名を、VRC のチャットボックスに `🎵 Now Playing: Artist - Title` 形式で送ります。同じ曲が続く間は再送信しません。

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
| `NP_URL` | `http://localhost:9000` | NP2 の Socket.IO URL |
| `VRC_OSC_HOST` | `127.0.0.1` | VRChat OSC 送信先 |
| `VRC_OSC_PORT` | `9000` | VRChat OSC 受信ポート |

NP2 のポートはアプリ UI または `GET /config` で確認してください。

## VRChat 側の設定

1. アクションメニュー → **Osc** → **Enabled** をオン
2. 外部プログラムからの送信先は既定で **UDP 9000**

詳細は [`docs/vrc_osc_ja/osc-overview.md`](docs/vrc_osc_ja/osc-overview.md) を参照。

## 使い方

### 通常モード（NP2 → VRChat）

```powershell
python nowplaying-vrc-osc.py
```

NP2 を起動し、曲が変わるたびに Chatbox へ送信します。

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
| `--np-url` | NP2 の URL |
| `--vrc-host` | VRChat OSC ホスト |
| `--vrc-port` | VRChat OSC ポート |

## ドキュメント

| パス | 内容 |
|---|---|
| [`docs/now-playing-app/external-connection.ja.md`](docs/now-playing-app/external-connection.ja.md) | NP2 への外部接続（Socket.IO API） |
| [`docs/vrc_osc_ja/`](docs/vrc_osc_ja/) | VRChat OSC ドキュメント（日本語） |
| [`docs/vrc_osc/`](docs/vrc_osc/) | VRChat OSC ドキュメント（英語原文） |

## トラブルシューティング

| 症状 | 確認すること |
|---|---|
| NP2 に接続できない | NP2 が起動しているか。`--np-url` のポートが正しいか |
| VRChat に表示されない | OSC が有効か。`--send-only` で送信テスト |
| 曲が更新されない | DJ ソフト側で曲がロード／再生されているか |

## ライセンス

未設定。利用・再配布する場合は必要に応じてライセンスを追加してください。
