import argparse
import os
import sys
import uuid

import socketio
from pythonosc import udp_client

DEFAULT_NP_URL = "http://localhost:9000"
DEFAULT_VRC_HOST = "127.0.0.1"
DEFAULT_VRC_PORT = 9000
DEFAULT_SERVICE = "NowPlayingTrackReceivers"
DEFAULT_TEST_MESSAGE = "🎵 Now Playing OSC テスト"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
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
        help="--send-only 時に送るテキスト（省略時: Now Playing OSC テスト）",
    )
    parser.add_argument(
        "--np-url",
        default=os.getenv("NP_URL", DEFAULT_NP_URL),
        help=f"NP2 の Socket.IO URL（既定: {DEFAULT_NP_URL}）",
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
    return parser.parse_args()


def format_track(track: dict) -> str:
    title = track.get("title", "")
    artist = track.get("artist", "")
    label = track.get("label", "")
    parts = [f"{artist} - {title}".strip(" -")]
    if label:
        parts.append(f"[{label}]")
    return " ".join(parts)


def run_send_only(host: str, port: int, message: str) -> None:
    client = udp_client.SimpleUDPClient(host, port)
    client.send_message("/chatbox/input", [message, True, False])
    print(f"VRChat へ送信しました ({host}:{port})")
    print(f"  /chatbox/input → {message!r}")


def run_np_listener(np_url: str, on_track) -> None:
    sio = socketio.Client()
    last_title = {"value": None}

    def handle_track(track: dict) -> None:
        title = track.get("title", "")
        if not title or title == last_title["value"]:
            return
        last_title["value"] = title
        on_track(track)

    @sio.on("connect")
    def on_connect() -> None:
        print(f"NP2 に接続しました ({np_url})")
        sio.emit("id", DEFAULT_SERVICE)

        response_id = str(uuid.uuid4())

        def on_current_track(response: dict) -> None:
            track = response.get("result")
            if track:
                handle_track(track)

        sio.on(response_id)(on_current_track)
        sio.emit(
            "service-call",
            {
                "from": DEFAULT_SERVICE,
                "to": "NowPlayingHistory",
                "fn": "getCurrentTrack",
                "args": [],
                "responseId": response_id,
            },
        )

    @sio.on("trackUpdate")
    def on_track_update(data: dict) -> None:
        handle_track(data["args"][0])

    @sio.on("animate")
    def on_animate(data: dict) -> None:
        handle_track(data["args"][0])

    sio.connect(np_url)
    print("曲情報の更新を待っています… (Ctrl+C で終了)")
    sio.wait()


def run_receive_only(np_url: str) -> None:
    def on_track(track: dict) -> None:
        print(format_track(track))

    run_np_listener(np_url, on_track)


def run_bridge(np_url: str, vrc_host: str, vrc_port: int) -> None:
    osc = udp_client.SimpleUDPClient(vrc_host, vrc_port)

    def on_track(track: dict) -> None:
        text = f"🎵 Now Playing: {format_track(track)}"
        print(text)
        osc.send_message("/chatbox/input", [text, True, False])

    run_np_listener(np_url, on_track)


def main() -> None:
    args = parse_args()

    if args.send_only:
        run_send_only(args.vrc_host, args.vrc_port, args.message)
        return

    if args.receive_only:
        run_receive_only(args.np_url)
        return

    run_bridge(args.np_url, args.vrc_host, args.vrc_port)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\n終了しました")
        sys.exit(0)
