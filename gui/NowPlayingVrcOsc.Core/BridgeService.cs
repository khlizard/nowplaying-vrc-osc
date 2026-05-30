namespace NowPlayingVrcOsc.Core;

public sealed class BridgeService : IAsyncDisposable
{
    private readonly AppSettings _settings;
    private readonly TrackDeduplicator _deduplicator = new();
    private CancellationTokenSource? _cts;
    private VrcChatboxSender? _sender;
    private ITrackSource? _source;

    public event Action<TrackInfo>? TrackChanged;
    public event Action<string>? MessageSent;

    public bool IsRunning => _cts != null && !_cts.IsCancellationRequested;

    public BridgeService(AppSettings settings) => _settings = settings;

    public Task StartAsync()
    {
        if (IsRunning) return Task.CompletedTask;

        _deduplicator.Reset();
        _cts = new CancellationTokenSource();
        _sender = new VrcChatboxSender(
            _settings.VrcHost, _settings.VrcPort,
            _settings.ChatboxNotification, _settings.ChatboxIndicator);

        var source = new ConfigJsonTrackSource(_settings.ConfigPath, _settings.PollIntervalSeconds);
        source.TrackReceived += OnTrackReceived;
        _source = source;

        return _source.StartAsync(_cts.Token);
    }

    private void OnTrackReceived(TrackInfo track)
    {
        if (!_deduplicator.IsNewTrack(track)) return;

        TrackChanged?.Invoke(track);

        var message = TrackFormatter.FormatChatboxMessage(track, _settings.ShowLabel, _settings.StripTitleBrackets);
        _sender?.Send(message);
        MessageSent?.Invoke(message);
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
        if (_source != null)
        {
            await _source.DisposeAsync().ConfigureAwait(false);
            _source = null;
        }
    }
}
