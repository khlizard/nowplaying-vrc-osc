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
    /// 既知のベースライン曲で初期化し、初回スキップを無効化する。
    /// rekordbox ソースは起動時点の曲を通知しないため、BridgeService は Seed(null) を呼び、
    /// 最初に通知された新規再生をそのまま送信できるようにする。
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
