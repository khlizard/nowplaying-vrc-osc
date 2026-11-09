using NowPlayingVrcOsc.Core.Rekordbox;

namespace NowPlayingVrcOsc.Core;

public sealed class BridgeService : IAsyncDisposable
{
    private readonly AppSettings _settings;
    private readonly TrackDeduplicator _deduplicator = new();
    private readonly Action<string> _sourceStatusChangedHandler;
    private CancellationTokenSource? _cts;
    private VrcChatboxSender? _sender;
    private RekordboxTrackSource? _source;

    public event Action<TrackInfo>? TrackChanged;
    public event Action<string>? MessageSent;
    /// <summary>接続状態の変化（UI 表示用）。バックグラウンドスレッドから呼ばれる。</summary>
    public event Action<string>? StatusChanged;

    public bool IsRunning => _cts != null && !_cts.IsCancellationRequested;

    public BridgeService(AppSettings settings)
    {
        _settings = settings;
        _sourceStatusChangedHandler = s => StatusChanged?.Invoke(s);
    }

    public async Task StartAsync()
    {
        if (IsRunning) return;
        await DisposeSourceAsync().ConfigureAwait(false);

        _deduplicator.Reset();

        // rekordbox ソースは起動時点の曲を通知しないので、最初の新規再生をそのまま送る。
        _deduplicator.Seed(null);

        _cts = new CancellationTokenSource();
        _sender = new VrcChatboxSender(
            _settings.VrcHost, _settings.VrcPort,
            _settings.ChatboxNotification, _settings.ChatboxIndicator);

        var source = new RekordboxTrackSource(_settings);
        source.TrackReceived += OnTrackReceived;
        source.StatusChanged += _sourceStatusChangedHandler;
        _source = source;

        await source.StartAsync(_cts.Token).ConfigureAwait(false);
    }

    private void OnTrackReceived(TrackInfo track)
    {
        if (!_deduplicator.IsNewTrack(track)) return;

        TrackChanged?.Invoke(track);

        var message = TrackFormatter.FormatChatboxMessage(track, _settings.ToMessageFormat());
        try
        {
            _sender?.Send(message);
            MessageSent?.Invoke(message);
        }
        catch (Exception ex)
        {
            StatusChanged?.Invoke($"Chatbox送信エラー: {ex.Message}");
        }
    }

    public void Stop()
    {
        _cts?.Cancel();
        _sender?.Dispose();
        _sender = null;
        _cts?.Dispose();
        _cts = null;
    }

    public async ValueTask DisposeAsync()
    {
        Stop();
        await DisposeSourceAsync().ConfigureAwait(false);
    }

    private async ValueTask DisposeSourceAsync()
    {
        var source = _source;
        _source = null;
        if (source == null) return;

        source.TrackReceived -= OnTrackReceived;
        source.StatusChanged -= _sourceStatusChangedHandler;
        await source.DisposeAsync().ConfigureAwait(false);
    }
}
