using System.Text;
using System.Text.Json;

namespace NowPlayingVrcOsc.Core;

public sealed class ConfigJsonTrackSource : ITrackSource
{
    private readonly string _configPath;
    private readonly TimeSpan _pollInterval;

    public event Action<TrackInfo>? TrackReceived;

    public ConfigJsonTrackSource(string configPath, double pollIntervalSeconds = 0.5)
    {
        _configPath = configPath;
        _pollInterval = TimeSpan.FromSeconds(pollIntervalSeconds);
    }

    public async Task StartAsync(CancellationToken cancellationToken)
    {
        long? lastMtime = null;

        while (!cancellationToken.IsCancellationRequested)
        {
            try
            {
                if (File.Exists(_configPath))
                {
                    var mtime = new DateTimeOffset(File.GetLastWriteTimeUtc(_configPath)).ToUnixTimeMilliseconds();
                    if (lastMtime == null || mtime != lastMtime)
                    {
                        lastMtime = mtime;
                        var track = ReadCurrentTrack();
                        if (track != null)
                            TrackReceived?.Invoke(track);
                    }
                }
            }
            catch { }

            try
            {
                await Task.Delay(_pollInterval, cancellationToken).ConfigureAwait(false);
            }
            catch (OperationCanceledException) { break; }
        }
    }

    private TrackInfo? ReadCurrentTrack()
    {
        try
        {
            var json = File.ReadAllText(_configPath, Encoding.UTF8);
            using var doc = JsonDocument.Parse(json);
            if (!doc.RootElement.TryGetProperty("currentTrack", out var trackEl)) return null;
            return NpConfig.ParseTrack(trackEl);
        }
        catch { return null; }
    }

    public ValueTask DisposeAsync() => ValueTask.CompletedTask;
}
