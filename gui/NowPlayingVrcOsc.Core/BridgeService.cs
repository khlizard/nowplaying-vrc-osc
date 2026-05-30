namespace NowPlayingVrcOsc.Core;

public sealed class BridgeService : IAsyncDisposable
{
    private readonly AppSettings _settings;
    private readonly TrackDeduplicator _deduplicator = new();
    private readonly Action<string> _coordinatorStatusChangedHandler;
    private CancellationTokenSource? _cts;
    private VrcChatboxSender? _sender;
    private TrackSourceCoordinator? _coordinator;

    public event Action<TrackInfo>? TrackChanged;
    public event Action<string>? MessageSent;
    /// <summary>ソース切替・接続状態の変化（UI 表示用）。バックグラウンドスレッドから呼ばれる。</summary>
    public event Action<string>? StatusChanged;

    public bool IsRunning => _cts != null && !_cts.IsCancellationRequested;

    public BridgeService(AppSettings settings)
    {
        _settings = settings;
        _coordinatorStatusChangedHandler = s => StatusChanged?.Invoke(s);
    }

    public async Task StartAsync()
    {
        if (IsRunning) return;
        await DisposeCoordinatorAsync().ConfigureAwait(false);

        _deduplicator.Reset();

        // Socket.IO は接続時に NP2 が現在曲を push しない。
        // config.json の currentTrack をベースラインとしてシードし、初回スキップを無効化する。
        // （auto かつ config.json が存在しない場合も同様）
        bool socketIoFromStart = _settings.Source == "socketio" ||
            (_settings.Source != "config" && !File.Exists(_settings.ConfigPath));
        if (socketIoFromStart)
            _deduplicator.Seed(NpConfig.ReadCurrentTrack(_settings.ConfigPath));

        _cts = new CancellationTokenSource();
        _sender = new VrcChatboxSender(
            _settings.VrcHost, _settings.VrcPort,
            _settings.ChatboxNotification, _settings.ChatboxIndicator);

        var coordinator = new TrackSourceCoordinator(_settings);
        coordinator.TrackReceived += OnTrackReceived;
        coordinator.StatusChanged += _coordinatorStatusChangedHandler;
        _coordinator = coordinator;

        await coordinator.StartAsync(_cts.Token).ConfigureAwait(false);
    }

    private void OnTrackReceived(TrackInfo track)
    {
        if (!_deduplicator.IsNewTrack(track)) return;

        TrackChanged?.Invoke(track);

        var message = TrackFormatter.FormatChatboxMessage(track, _settings.ShowLabel, _settings.StripTitleBrackets);
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
        await DisposeCoordinatorAsync().ConfigureAwait(false);
    }

    private async ValueTask DisposeCoordinatorAsync()
    {
        var coordinator = _coordinator;
        _coordinator = null;
        if (coordinator == null) return;

        coordinator.TrackReceived -= OnTrackReceived;
        coordinator.StatusChanged -= _coordinatorStatusChangedHandler;
        await coordinator.DisposeAsync().ConfigureAwait(false);
    }
}
