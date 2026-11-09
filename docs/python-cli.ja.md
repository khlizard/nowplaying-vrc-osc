# 旧 CLI 版（Python / Now Playing App 2 連携）

[`nowplaying-vrc-osc.py`](../nowplaying-vrc-osc.py) は、**Now Playing App 2**（NP2）から再生中の曲を受け取り、VRChat の Chatbox へ OSC で送るコマンドライン版です。

- GUI 版（DJ Now Playing for VRChat）とは曲の取得元が違います。GUI 版は rekordbox を直接読み、NP2 は使いません。
- NP2 を使う場合の参考として残しています。新しい機能は GUI 版にだけ追加します。
- [Now Playing App](https://nowplayingapp.com)（Triode）の公式ツールではありません。

## 仕組み

```
Now Playing App 2
 ├── config.json (currentTrack)  ── 監視（既定）──┐
 └── Socket.IO                   ── 予備 ────────┤
                                                 ▼
                                    nowplaying-vrc-osc.py
                                                 │  /chatbox/input
                                                 ▼
                                    VRChat (OSC / UDP)
```

NP2 が DJ ソフトから取得した曲名とアーティスト名を、`🎵 Title - Artist` の形で Chatbox に送ります。
起動時に再生中の曲は送りません。同じ曲が続く間は送り直しません（`currentTrack.id` で判定）。

## 必要なもの

- Python 3.10 以上
- [Now Playing App 2](https://nowplayingapp.com)
- VRChat（OSC を有効にしておく）

## インストール

```powershell
python -m venv .venv
.venv\Scripts\Activate.ps1
pip install -r requirements.txt
```

## 使い方

```powershell
# NP2 → VRChat
python nowplaying-vrc-osc.py

# NP2 からの受信だけを確認する（VRChat には送らない）
python nowplaying-vrc-osc.py --receive-only

# VRChat への送信だけを確認する（NP2 には接続しない）
python nowplaying-vrc-osc.py --send-only -m "接続テスト"
```

VRChat 側では、アクションメニュー → **Osc** → **Enabled** をオンにします。

## 設定

環境変数か、カレントディレクトリの `.env` で設定します。書き方は [`.env.example`](../.env.example) を参照してください。
同じ設定はコマンドラインのオプションでも指定でき、オプションが優先されます。

| 環境変数 | オプション | 既定値 | 説明 |
|---|---|---|---|
| `NP_SOURCE` | `--source` | `auto` | 取得方式。`auto`（config.json を監視し、無ければ Socket.IO）/ `config` / `socketio` |
| `NP_CONFIG_PATH` | `--np-config-path` | `~/.nowplaying/config.json` | NP2 の config.json |
| `NP_POLL_INTERVAL` | `--poll-interval` | `0.5` | config.json の確認間隔（秒） |
| `NP_URL` | `--np-url` | config.json の `localSocketPort` から決める。無ければ `http://localhost:9000` | Socket.IO の URL |
| `VRC_OSC_HOST` | `--vrc-host` | `127.0.0.1` | OSC の送信先 |
| `VRC_OSC_PORT` | `--vrc-port` | `9000` | OSC の送信先ポート |
| `VRC_CHATBOX_NOTIFICATION` | `--notification` / `--no-notification` | `false` | Chatbox の通知音 |
| `VRC_CHATBOX_INDICATOR` | `--indicator` / `--no-indicator` | `false` | 送信前後に入力中インジケーターを出す |
| `NP_MSG_HEADER` | `--msg-header` | `🎵` | 先頭のヘッダ |
| `NP_MSG_SEPARATOR` | `--msg-separator` | `-` | 項目の区切り（前後にスペースが付く） |
| `NP_MSG_FIELDS` | `--msg-fields` | `title,artist` | 表示する項目（`title,artist,album,label,comment` から選ぶ） |
| `NP_STRIP_TITLE_BRACKETS` | `--strip-title-brackets` / `--no-strip-title-brackets` | `true` | 曲名内の `[～]` を除く |
| `NP_SHOW_LABEL` | `--show-label` / `--no-show-label` | `false` | 非推奨。true のとき表示項目に label を足す |

## うまく動かないとき

| 症状 | 確認すること |
|---|---|
| 曲が更新されない | NP2 が起動しているか。DJ ソフトで曲を再生しているか |
| config.json の監視が動かない | `%USERPROFILE%\.nowplaying\config.json` があるか。`--np-config-path` を確認する |
| Socket.IO に切り替わってしまう | config.json のパスが正しいか。`--source config` で監視を強制する |
| VRChat に表示されない | OSC が有効か。`--send-only` で送信を試す |
| Socket.IO に接続できない | config.json の `localSocketPort` を確認し、`--np-url http://localhost:<ポート>` を指定する |
