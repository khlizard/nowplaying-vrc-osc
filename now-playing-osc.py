import socketio
import uuid
from pythonosc import udp_client

# Now Playing App 2（Socket.IO）— ポートはアプリ UI または GET /config で確認
NP_URL = "http://localhost:9000"
SERVICE = "NowPlayingTrackReceivers"

# VRChat OSC
osc = udp_client.SimpleUDPClient("127.0.0.1", 9000)

last_title = None
sio = socketio.Client()


def send_to_vrc(track: dict) -> None:
    global last_title
    title = track.get("title", "")
    artist = track.get("artist", "")
    if not title or title == last_title:
        return
    last_title = title
    text = f"🎵 Now Playing: {artist} - {title}"
    print(text)
    osc.send_message("/chatbox/input", [text, True, False])


@sio.on("connect")
def on_connect() -> None:
    print("NP2 に接続しました")
    sio.emit("id", SERVICE)

    response_id = str(uuid.uuid4())

    def on_current_track(response: dict) -> None:
        track = response.get("result")
        if track:
            send_to_vrc(track)

    sio.on(response_id)(on_current_track)

    sio.emit(
        "service-call",
        {
            "from": SERVICE,
            "to": "NowPlayingHistory",
            "fn": "getCurrentTrack",
            "args": [],
            "responseId": response_id,
        },
    )


@sio.on("trackUpdate")
def on_track_update(data: dict) -> None:
    send_to_vrc(data["args"][0])


@sio.on("animate")
def on_animate(data: dict) -> None:
    send_to_vrc(data["args"][0])


sio.connect(NP_URL)
sio.wait()
