import asyncio
import json
import websockets
from pythonosc import udp_client

# Now Playing App WebSocketポート（アプリ下部で確認）
NP_WS_URL = "ws://localhost:9000"

# VRChat OSC
osc = udp_client.SimpleUDPClient("127.0.0.1", 9000)

last_title = None

async def watch_and_send():
    global last_title
    async with websockets.connect(NP_WS_URL) as ws:
        print("接続完了")
        async for message in ws:
            data = json.loads(message)
            title  = data.get("title", "")
            artist = data.get("artist", "")

            # 曲変化時のみ送信（同じ曲を繰り返し送らない）
            if title and title != last_title:
                last_title = title
                text = f"🎵 Now Playing: {artist} - {title}"
                print(text)

                # VRChat Chatboxへ送信
                osc.send_message("/chatbox/input", [text, True, False])

asyncio.run(watch_and_send())
