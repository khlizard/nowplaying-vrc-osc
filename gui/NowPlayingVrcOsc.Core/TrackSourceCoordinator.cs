namespace NowPlayingVrcOsc.Core;

/// <summary>
/// Source 設定（auto / config / socketio）に応じて ITrackSource を選択・起動する。
/// Python 版 run_track_source + run_config_watcher のフォールバック動作と同等（parity）。
/// </summary>
public sealed class TrackSourceCoordinator : ITrackSource
{
    private readonly AppSettings _settings;
    private readonly string _resolvedNpUrl;
    private ITrackSource? _activeSource;

    public event Action<TrackInfo>? TrackReceived;
    /// <summary>ソース切替・接続状態の変化を通知（UI 表示用）。</summary>
    public event Action<string>? StatusChanged;

    public TrackSourceCoordinator(AppSettings settings)
    {
        _settings = settings;
        _resolvedNpUrl = NpConfig.ResolveNpUrl(settings.NpUrl, settings.ConfigPath);
    }

    public Task StartAsync(CancellationToken cancellationToken) =>
        _settings.Source switch
        {
            "config"   => RunConfigOnlyAsync(cancellationToken),
            "socketio" => RunSocketIoAsync(cancellationToken),
            _          => RunAutoAsync(cancellationToken),     // "auto" or anything else
        };

    // --- auto: config 優先、ファイルが無ければ Socket.IO -----------------

    private async Task RunAutoAsync(CancellationToken cancellationToken)
    {
        if (File.Exists(_settings.ConfigPath))
            await RunConfigWithFallbackAsync(cancellationToken);
        else
        {
            StatusChanged?.Invoke($"config.json が見つかりません — Socket.IO を使用 ({_resolvedNpUrl})");
            await RunSocketIoAsync(cancellationToken);
        }
    }

    /// <summary>
    /// config.json ポーリング。ファイルが消えたら Socket.IO にフォールバック。
    /// Python: run_config_watcher → run_np_listener（last_key を引き継ぐ）
    /// </summary>
    private async Task RunConfigWithFallbackAsync(CancellationToken cancellationToken)
    {
        var pollInterval = TimeSpan.FromSeconds(_settings.PollIntervalSeconds);
        long? lastMtime = null;
        StatusChanged?.Invoke("監視中 (config.json)");

        while (!cancellationToken.IsCancellationRequested)
        {
            if (!File.Exists(_settings.ConfigPath))
            {
                StatusChanged?.Invoke($"config.json が消失 — Socket.IO に切替 ({_resolvedNpUrl})");
                await RunSocketIoAsync(cancellationToken);
                return;
            }

            try
            {
                var mtime = new DateTimeOffset(
                    File.GetLastWriteTimeUtc(_settings.ConfigPath)).ToUnixTimeMilliseconds();
                if (lastMtime == null || mtime != lastMtime)
                {
                    lastMtime = mtime;
                    var track = NpConfig.ReadCurrentTrack(_settings.ConfigPath);
                    if (track != null) TrackReceived?.Invoke(track);
                }
            }
            catch { }

            try { await Task.Delay(pollInterval, cancellationToken); }
            catch (OperationCanceledException) { break; }
        }
    }

    // --- config のみ -------------------------------------------------------

    private async Task RunConfigOnlyAsync(CancellationToken cancellationToken)
    {
        var source = new ConfigJsonTrackSource(_settings.ConfigPath, _settings.PollIntervalSeconds);
        source.TrackReceived += t => TrackReceived?.Invoke(t);
        _activeSource = source;
        StatusChanged?.Invoke("監視中 (config.json)");
        await source.StartAsync(cancellationToken);
    }

    // --- Socket.IO ---------------------------------------------------------

    private async Task RunSocketIoAsync(CancellationToken cancellationToken)
    {
        var source = new SocketIoTrackSource(_resolvedNpUrl);
        source.TrackReceived += t => TrackReceived?.Invoke(t);
        source.StatusChanged  += s => StatusChanged?.Invoke(s);
        _activeSource = source;
        await source.StartAsync(cancellationToken);
    }

    public async ValueTask DisposeAsync()
    {
        if (_activeSource != null)
        {
            await _activeSource.DisposeAsync();
            _activeSource = null;
        }
    }
}
