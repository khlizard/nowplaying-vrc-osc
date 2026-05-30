using System.Text.Json;
using SioClient = SocketIOClient.SocketIO;
using SioOptions = SocketIOClient.SocketIOOptions;
using SioResponse = SocketIOClient.SocketIOResponse;

namespace NowPlayingVrcOsc.Core;

/// <summary>
/// Socket.IO でNP2に接続し trackUpdate / animate イベントを受信する。
/// Python 版 run_np_listener と同等（起動時の現在曲は TrackDeduplicator でスキップ）。
/// </summary>
public sealed class SocketIoTrackSource : ITrackSource
{
    private const string ServiceName = "NowPlayingTrackReceivers";

    private readonly string _url;
    private SioClient? _client;

    public event Action<TrackInfo>? TrackReceived;
    /// <summary>接続状態の変化を通知（UI 表示用）。</summary>
    public event Action<string>? StatusChanged;

    public SocketIoTrackSource(string url) => _url = url;

    public async Task StartAsync(CancellationToken cancellationToken)
    {
        _client = new SioClient(_url, new SioOptions
        {
            ConnectionTimeout = TimeSpan.FromSeconds(15),
            ReconnectionAttempts = int.MaxValue,
            ReconnectionDelay = 3000,
        });

        _client.OnConnected += async (_, _) =>
        {
            StatusChanged?.Invoke($"接続済み (Socket.IO: {_url})");
            await _client.EmitAsync("id", ServiceName);
        };

        // キャンセル済み（意図的停止・設定変更）の場合はステータスを更新しない
        _client.OnDisconnected += (_, reason) =>
        {
            if (!cancellationToken.IsCancellationRequested)
                StatusChanged?.Invoke($"切断 ({reason}) — 再接続中…");
        };

        _client.OnError += (_, error) =>
        {
            if (!cancellationToken.IsCancellationRequested)
                StatusChanged?.Invoke($"Socket.IO エラー: {error}");
        };

        _client.On("trackUpdate", response =>
        {
            var track = ParseTrack(response);
            if (track != null) TrackReceived?.Invoke(track);
        });

        _client.On("animate", response =>
        {
            var track = ParseTrack(response);
            if (track != null) TrackReceived?.Invoke(track);
        });

        StatusChanged?.Invoke($"Socket.IO に接続中 ({_url})…");
        await _client.ConnectAsync();

        try
        {
            await Task.Delay(Timeout.Infinite, cancellationToken);
        }
        catch (OperationCanceledException) { }

        await _client.DisconnectAsync();
    }

    // Python: handle_track(data["args"][0])
    private static TrackInfo? ParseTrack(SioResponse response)
    {
        try
        {
            var data = response.GetValue<JsonElement>();
            if (!data.TryGetProperty("args", out var args) || args.GetArrayLength() == 0)
                return null;
            var t = args[0];
            var title = GetString(t, "title");
            if (string.IsNullOrEmpty(title)) return null;
            return new TrackInfo(
                Id: GetString(t, "id"),
                Title: title,
                Artist: GetString(t, "artist"),
                Label: GetString(t, "label")
            );
        }
        catch { return null; }
    }

    private static string? GetString(JsonElement el, string property)
    {
        if (el.TryGetProperty(property, out var val) && val.ValueKind == JsonValueKind.String)
            return val.GetString();
        return null;
    }

    public async ValueTask DisposeAsync()
    {
        if (_client == null) return;
        try { await _client.DisconnectAsync(); } catch { }
        _client.Dispose();
        _client = null;
    }
}
