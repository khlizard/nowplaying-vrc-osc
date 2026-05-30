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

    /// <summary>
    /// 既知のベースライン曲で初期化する。
    /// Socket.IO ソース用：接続時に NP2 が現在曲を push しないため、
    /// config.json の currentTrack を先行シードして初回スキップを無効化する。
    /// knownCurrent が null の場合も _initialSeen=true にして初回スキップを無効化。
    /// </summary>
    public void Seed(TrackInfo? knownCurrent)
    {
        _initialSeen = true;
        _lastKey = knownCurrent != null ? TrackKey(knownCurrent) : null;
    }

    private static string TrackKey(TrackInfo track) =>
        !string.IsNullOrEmpty(track.Id) ? track.Id : (track.Title ?? "");
}
