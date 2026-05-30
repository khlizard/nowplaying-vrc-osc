namespace NowPlayingVrcOsc.Core;

public class TrackDeduplicator
{
    private string? _lastKey;
    private bool _initialSeen;

    /// <summary>
    /// Returns true if this track should be sent (new track, not startup baseline).
    /// First call per Reset() always returns false (startup skip, matching Python's make_track_handler).
    /// </summary>
    public bool IsNewTrack(TrackInfo track)
    {
        var key = TrackKey(track);
        if (string.IsNullOrEmpty(key)) return false;

        if (!_initialSeen)
        {
            _initialSeen = true;
            _lastKey = key;
            return false;
        }

        if (key == _lastKey) return false;
        _lastKey = key;
        return true;
    }

    public void Reset()
    {
        _lastKey = null;
        _initialSeen = false;
    }

    private static string TrackKey(TrackInfo track) =>
        !string.IsNullOrEmpty(track.Id) ? track.Id : (track.Title ?? "");
}
