using System.Diagnostics;
using System.IO;
using Velopack;
using Velopack.Sources;

namespace NowPlayingVrcOsc.App;

/// <summary>Velopack による自動更新（GitHub Releases）。未インストール時は何もしない。</summary>
public sealed class AppUpdater
{
    private const string RepoUrl = "https://github.com/khlizard/nowplaying-vrc-osc";
    private const string FeedOverrideEnv = "NPVO_UPDATE_FEED";

    private readonly UpdateManager? _manager;
    private VelopackAsset? _pending;

    public AppUpdater()
    {
        try
        {
            IUpdateSource source;
            var feed = Environment.GetEnvironmentVariable(FeedOverrideEnv);
            if (!string.IsNullOrWhiteSpace(feed) && Directory.Exists(feed))
                source = new SimpleFileSource(new DirectoryInfo(feed));
            else
                source = new GithubSource(RepoUrl, null, prerelease: true);

            var manager = new UpdateManager(source);
            if (manager.IsInstalled) _manager = manager;
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[NowPlayingVrcOsc] Updater init failed: {ex}");
        }
    }

    /// <summary>更新を確認してダウンロードする。更新があれば取得したアセットを返す。例外は握りつぶす。</summary>
    public async Task<VelopackAsset?> CheckAndDownloadAsync()
    {
        if (_manager == null) return null;
        try
        {
            var info = await _manager.CheckForUpdatesAsync().ConfigureAwait(false);
            if (info == null) return null;
            await _manager.DownloadUpdatesAsync(info).ConfigureAwait(false);
            _pending = info.TargetFullRelease;
            return _pending;
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[NowPlayingVrcOsc] Update check failed: {ex.Message}");
            return null;
        }
    }

    /// <summary>更新を適用してアプリを再起動する（呼び出し元で後始末済みであること）。</summary>
    public void ApplyAndRestart()
    {
        if (_manager == null || _pending == null) return;
        try { _manager.ApplyUpdatesAndRestart(_pending); }
        catch (Exception ex) { Debug.WriteLine($"[NowPlayingVrcOsc] Apply failed: {ex.Message}"); }
    }

    /// <summary>終了後に更新を適用する（再起動しない）。</summary>
    public void ApplyOnExit()
    {
        if (_manager == null || _pending == null) return;
        try { _manager.WaitExitThenApplyUpdates(_pending, silent: true, restart: false); }
        catch (Exception ex) { Debug.WriteLine($"[NowPlayingVrcOsc] Apply on exit failed: {ex.Message}"); }
    }
}
