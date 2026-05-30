import argparse
import json
import os
import re
import sys
import time
from pathlib import Path

import socketio
from pythonosc import udp_client

DEFAULT_NP_URL = "http://localhost:9000"
DEFAULT_VRC_HOST = "127.0.0.1"
DEFAULT_VRC_PORT = 9000
DEFAULT_SERVICE = "NowPlayingTrackReceivers"
DEFAULT_TEST_MESSAGE = "nowplaying-vrc-osc テスト"
DEFAULT_SOURCE = "auto"
DEFAULT_POLL_INTERVAL = 0.5
DEFAULT_CHATBOX_NOTIFICATION = False
DEFAULT_CHATBOX_INDICATOR = False
DEFAULT_SHOW_LABEL = False
DEFAULT_STRIP_TITLE_BRACKETS = True

TITLE_BRACKETS_RE = re.compile(r"\s*\[[^\]]*\]")


def load_env_file() -> None:
    try:
        from dotenv import load_dotenv
    except ImportError:
        return
    load_dotenv()


def env_bool(name: str, default: bool) -> bool:
    value = os.getenv(name)
    if value is None:
        return default
    return value.strip().lower() in ("1", "true", "yes", "on")


def default_config_path() -> Path:
    return Path.home() / ".nowplaying" / "config.json"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="nowplaying-vrc-osc",
        description="Now Playing App 2 から曲情報を取得し VRChat Chatbox へ OSC 送信するブリッジ",
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--receive-only",
        action="store_true",
        help="NP2 から曲情報を受信して表示するだけ（VRChat へは送信しない）",
    )
    mode.add_argument(
        "--send-only",
        action="store_true",
        help="VRChat へテストメッセージを 1 回送信するだけ（NP2 には接続しない）",
    )
    parser.add_argument(
        "--message",
        "-m",
        default=DEFAULT_TEST_MESSAGE,
        help="--send-only 時に送るテキスト（省略時: nowplaying-vrc-osc テスト）",
    )
    parser.add_argument(
        "--source",
        choices=("auto", "config", "socketio"),
        default=os.getenv("NP_SOURCE", DEFAULT_SOURCE),
        help="曲情報の取得方式（既定: auto = config.json 監視、失敗時 Socket.IO）",
    )
    parser.add_argument(
        "--np-config-path",
        default=os.getenv("NP_CONFIG_PATH"),
        help="NP2 config.json のパス（省略時: ~/.nowplaying/config.json）",
    )
    parser.add_argument(
        "--poll-interval",
        type=float,
        default=float(os.getenv("NP_POLL_INTERVAL", DEFAULT_POLL_INTERVAL)),
        help=f"config.json 監視のポーリング間隔秒（既定: {DEFAULT_POLL_INTERVAL}）",
    )
    parser.add_argument(
        "--np-url",
        default=os.getenv("NP_URL"),
        help=f"NP2 の Socket.IO URL（省略時: config.json の localSocketPort から自動、なければ {DEFAULT_NP_URL}）",
    )
    parser.add_argument(
        "--vrc-host",
        default=os.getenv("VRC_OSC_HOST", DEFAULT_VRC_HOST),
        help=f"VRChat OSC 送信先ホスト（既定: {DEFAULT_VRC_HOST}）",
    )
    parser.add_argument(
        "--vrc-port",
        type=int,
        default=int(os.getenv("VRC_OSC_PORT", DEFAULT_VRC_PORT)),
        help=f"VRChat OSC 送信先ポート（既定: {DEFAULT_VRC_PORT}）",
    )
    notification = parser.add_mutually_exclusive_group()
    notification.add_argument(
        "--notification",
        dest="notification",
        action="store_true",
        help="Chatbox 送信時に通知音を鳴らす",
    )
    notification.add_argument(
        "--no-notification",
        dest="notification",
        action="store_false",
        help="Chatbox 送信時に通知音を鳴らさない",
    )
    indicator = parser.add_mutually_exclusive_group()
    indicator.add_argument(
        "--indicator",
        dest="indicator",
        action="store_true",
        help="Chatbox 送信前後で入力中インジケーターを表示する",
    )
    indicator.add_argument(
        "--no-indicator",
        dest="indicator",
        action="store_false",
        help="入力中インジケーターを表示しない",
    )
    label = parser.add_mutually_exclusive_group()
    label.add_argument(
        "--show-label",
        dest="show_label",
        action="store_true",
        help="label フィールドの [～] を表示する",
    )
    label.add_argument(
        "--no-show-label",
        dest="show_label",
        action="store_false",
        help="label フィールドの [～] を表示しない（既定）",
    )
    title_brackets = parser.add_mutually_exclusive_group()
    title_brackets.add_argument(
        "--strip-title-brackets",
        dest="strip_title_brackets",
        action="store_true",
        help="曲名（title）内の [～] を除去する",
    )
    title_brackets.add_argument(
        "--no-strip-title-brackets",
        dest="strip_title_brackets",
        action="store_false",
        help="曲名（title）内の [～] を残す",
    )
    parser.set_defaults(
        notification=env_bool("VRC_CHATBOX_NOTIFICATION", DEFAULT_CHATBOX_NOTIFICATION),
        indicator=env_bool("VRC_CHATBOX_INDICATOR", DEFAULT_CHATBOX_INDICATOR),
        show_label=env_bool("NP_SHOW_LABEL", DEFAULT_SHOW_LABEL),
        strip_title_brackets=env_bool(
            "NP_STRIP_TITLE_BRACKETS", DEFAULT_STRIP_TITLE_BRACKETS
        ),
    )
    return parser.parse_args()


def resolve_config_path(explicit: str | None) -> Path:
    if explicit:
        return Path(explicit)
    return default_config_path()


def read_config(config_path: Path) -> dict | None:
    if not config_path.is_file():
        return None
    try:
        return json.loads(config_path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return None


def read_current_track(config_path: Path) -> dict | None:
    data = read_config(config_path)
    if not data:
        return None
    track = data.get("currentTrack")
    if not isinstance(track, dict):
        return None
    if not track.get("title"):
        return None
    return track


def np_url_from_config(config_path: Path) -> str | None:
    data = read_config(config_path)
    if not data:
        return None
    port = data.get("localSocketPort")
    if port:
        return f"http://localhost:{port}"
    return None


def resolve_np_url(explicit: str | None, config_path: Path) -> str:
    if explicit:
        return explicit
    from_config = np_url_from_config(config_path)
    if from_config:
        return from_config
    return DEFAULT_NP_URL


def track_key(track: dict) -> str:
    track_id = track.get("id")
    if track_id:
        return str(track_id)
    return track.get("title", "")


def remove_brackets(text: str) -> str:
    return TITLE_BRACKETS_RE.sub("", text).strip()


def normalize_field(value: object) -> str:
    if value is None:
        return ""
    text = str(value).strip()
    if not text or text.lower() in ("none", "unknown", "n/a", "?"):
        return ""
    return text


def format_track(
    track: dict,
    show_label: bool = DEFAULT_SHOW_LABEL,
    strip_title_brackets: bool = DEFAULT_STRIP_TITLE_BRACKETS,
) -> str:
    title = normalize_field(track.get("title"))
    if strip_title_brackets and title:
        title = remove_brackets(title)
    artist = normalize_field(track.get("artist"))
    label = normalize_field(track.get("label"))

    if title and artist:
        parts = [f"{title} - {artist}"]
    elif title:
        parts = [title]
    elif artist:
        parts = [artist]
    else:
        parts = [""]

    if label and show_label:
        parts.append(f"[{label}]")
    return " ".join(parts).strip()


def make_track_handler(on_track, last_key: dict | None = None, skip_initial: bool = True):
    if last_key is None:
        last_key = {"value": None}
    initial_seen = {"value": not skip_initial}

    def handle_track(track: dict) -> None:
        key = track_key(track)
        if not key:
            return
        if not initial_seen["value"]:
            initial_seen["value"] = True
            last_key["value"] = key
            return
        if key == last_key["value"]:
            return
        last_key["value"] = key
        on_track(track)

    return handle_track


def send_chatbox(
    client: udp_client.SimpleUDPClient,
    message: str,
    notification: bool,
    indicator: bool,
) -> None:
    if indicator:
        client.send_message("/chatbox/typing", [True])
    client.send_message("/chatbox/input", [message, True, notification])
    if indicator:
        client.send_message("/chatbox/typing", [False])


def run_send_only(
    host: str,
    port: int,
    message: str,
    notification: bool,
    indicator: bool,
) -> None:
    client = udp_client.SimpleUDPClient(host, port)
    send_chatbox(client, message, notification, indicator)
    print(f"VRChat へ送信しました ({host}:{port})")
    print(f"  /chatbox/input → {message!r}")
    print(f"  通知音: {'ON' if notification else 'OFF'} / インジケーター: {'ON' if indicator else 'OFF'}")


def run_config_watcher(
    config_path: Path,
    poll_interval: float,
    on_track,
    np_url: str,
) -> None:
    last_key = {"value": None}
    handle_track = make_track_handler(on_track, last_key)
    last_mtime: float | None = None

    print(f"config.json を監視しています ({config_path})")
    print("曲情報の更新を待っています… (Ctrl+C で終了)")

    while True:
        if not config_path.is_file():
            print(f"警告: {config_path} が見つかりません。Socket.IO に切り替えます。")
            run_np_listener(np_url, on_track, last_key)
            return

        try:
            mtime = config_path.stat().st_mtime
        except OSError:
            time.sleep(poll_interval)
            continue

        if last_mtime is None or mtime != last_mtime:
            last_mtime = mtime
            track = read_current_track(config_path)
            if track:
                handle_track(track)

        time.sleep(poll_interval)


def run_np_listener(np_url: str, on_track, last_key: dict | None = None) -> None:
    sio = socketio.Client()
    skip_initial = last_key is None or last_key.get("value") is None
    handle_track = make_track_handler(on_track, last_key, skip_initial=skip_initial)

    @sio.on("connect")
    def on_connect() -> None:
        print(f"NP2 に接続しました ({np_url})")
        sio.emit("id", DEFAULT_SERVICE)

    @sio.on("trackUpdate")
    def on_track_update(data: dict) -> None:
        handle_track(data["args"][0])

    @sio.on("animate")
    def on_animate(data: dict) -> None:
        handle_track(data["args"][0])

    sio.connect(np_url)
    print("曲情報の更新を待っています… (Ctrl+C で終了)")
    sio.wait()


def run_track_source(
    source: str,
    config_path: Path,
    poll_interval: float,
    np_url: str,
    on_track,
) -> None:
    if source == "config":
        if not config_path.is_file():
            print(f"エラー: config.json が見つかりません ({config_path})", file=sys.stderr)
            sys.exit(1)
        run_config_watcher(config_path, poll_interval, on_track, np_url)
        return

    if source == "socketio":
        run_np_listener(np_url, on_track)
        return

    # auto: config 監視優先、なければ Socket.IO
    if config_path.is_file():
        run_config_watcher(config_path, poll_interval, on_track, np_url)
    else:
        print(f"警告: {config_path} が見つかりません。Socket.IO を使用します ({np_url})")
        run_np_listener(np_url, on_track)


def run_receive_only(
    source: str,
    config_path: Path,
    poll_interval: float,
    np_url: str,
    show_label: bool,
    strip_title_brackets: bool,
) -> None:
    def on_track(track: dict) -> None:
        print(format_track(track, show_label, strip_title_brackets))

    run_track_source(source, config_path, poll_interval, np_url, on_track)


def run_bridge(
    source: str,
    config_path: Path,
    poll_interval: float,
    np_url: str,
    vrc_host: str,
    vrc_port: int,
    notification: bool,
    indicator: bool,
    show_label: bool,
    strip_title_brackets: bool,
) -> None:
    osc = udp_client.SimpleUDPClient(vrc_host, vrc_port)

    def on_track(track: dict) -> None:
        text = f"🎵                         {format_track(track, show_label, strip_title_brackets)}"
        print(text)
        send_chatbox(osc, text, notification, indicator)

    run_track_source(source, config_path, poll_interval, np_url, on_track)


def main() -> None:
    load_env_file()
    args = parse_args()
    config_path = resolve_config_path(args.np_config_path)
    np_url = resolve_np_url(args.np_url, config_path)

    if args.send_only:
        run_send_only(
            args.vrc_host,
            args.vrc_port,
            args.message,
            args.notification,
            args.indicator,
        )
        return

    if args.receive_only:
        run_receive_only(
            args.source,
            config_path,
            args.poll_interval,
            np_url,
            args.show_label,
            args.strip_title_brackets,
        )
        return

    run_bridge(
        args.source,
        config_path,
        args.poll_interval,
        np_url,
        args.vrc_host,
        args.vrc_port,
        args.notification,
        args.indicator,
        args.show_label,
        args.strip_title_brackets,
    )


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\n終了しました")
        sys.exit(0)
