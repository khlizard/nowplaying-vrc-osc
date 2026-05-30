namespace NowPlayingVrcOsc.Core;

public interface ITrackSource : IAsyncDisposable
{
    event Action<TrackInfo>? TrackReceived;
    Task StartAsync(CancellationToken cancellationToken);
}
