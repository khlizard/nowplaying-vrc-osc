using System.ComponentModel;
using System.Diagnostics;
using System.Windows;
using NowPlayingVrcOsc.Core;
using WinFormsApp = System.Windows.Forms;
using Application = System.Windows.Application;
using MessageBox = System.Windows.MessageBox;

namespace NowPlayingVrcOsc.App;

public partial class MainWindow : Window
{
    // --send-only 既定（Python 版 DEFAULT_TEST_MESSAGE と一致）
    private const string TestMessage = "nowplaying-vrc-osc テスト";

    private AppSettings _settings = AppSettings.Load();
    private BridgeService? _bridge;
    private WinFormsApp.NotifyIcon? _trayIcon;
    private bool _exiting;

    public MainWindow()
    {
        InitializeComponent();
        WindowIcon.TryApply(this);
        InitTray();
    }

    // --- トレイ常駐 -------------------------------------------------------

    private void InitTray()
    {
        var menu = new WinFormsApp.ContextMenuStrip();
        menu.Items.Add("ウィンドウを表示", null, (_, _) => ShowWindow());
        menu.Items.Add(new WinFormsApp.ToolStripSeparator());
        menu.Items.Add("終了", null, (_, _) => ExitApp());

        _trayIcon = new WinFormsApp.NotifyIcon
        {
            Icon = AppIcon.LoadTrayIcon(),
            Text = "Now Playing → VRChat",
            Visible = true,
            ContextMenuStrip = menu,
        };
        _trayIcon.DoubleClick += (_, _) => ShowWindow();
    }

    private void ShowWindow()
    {
        ShowInTaskbar = true;
        Show();
        WindowState = WindowState.Normal;
        Activate();
    }

    private async void ExitApp()
    {
        if (_exiting) return;
        _exiting = true;

        try
        {
            await StopAndDisposeBridgeAsync();
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[NowPlayingVrcOsc] Exit cleanup failed: {ex}");
        }

        if (_trayIcon != null)
        {
            _trayIcon.Visible = false;
            _trayIcon.Dispose();
            _trayIcon = null;
        }

        // Close() は Window_Closing から再入するとハング/例外になるため呼ばない
        Application.Current.Shutdown();
    }

    // --- Bridge 開始 / 停止 ----------------------------------------------

    private void StartButton_Click(object sender, RoutedEventArgs e) => StartBridge();

    private void StartBridge()
    {
        var bridge = new BridgeService(_settings);
        _bridge = bridge;

        _bridge.StatusChanged += s =>
            Dispatcher.Invoke(() => StatusText.Text = s);

        _bridge.TrackChanged += track =>
            Dispatcher.Invoke(() =>
                CurrentTrackText.Text = TrackFormatter.FormatTrack(
                    track, _settings.ShowLabel, _settings.StripTitleBrackets));

        _bridge.MessageSent += msg =>
            Dispatcher.Invoke(() =>
                LastSentText.Text = $"{DateTime.Now:HH:mm:ss}  {msg}");

        StartButton.IsEnabled = false;
        StopButton.IsEnabled = true;
        StatusText.Text = "接続中…";

        _ = bridge.StartAsync().ContinueWith(_ =>
            Dispatcher.Invoke(() =>
            {
                // 別の bridge に差し替え済み（再起動 / 手動停止）なら UI を触らない
                if (_exiting || !ReferenceEquals(_bridge, bridge)) return;
                _bridge = null;
                StatusText.Text = "停止中";
                StartButton.IsEnabled = true;
                StopButton.IsEnabled = false;
            }));
    }

    private async void StopButton_Click(object sender, RoutedEventArgs e)
    {
        await StopAndDisposeBridgeAsync();
        StatusText.Text = "停止中";
        StartButton.IsEnabled = true;
        StopButton.IsEnabled = false;
    }

    // --- テスト送信 -------------------------------------------------------

    private void TestSendButton_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            using var test = new VrcChatboxSender(
                _settings.VrcHost, _settings.VrcPort,
                _settings.ChatboxNotification, _settings.ChatboxIndicator);
            test.Send(TestMessage);
            LastSentText.Text = $"{DateTime.Now:HH:mm:ss}  (テスト) {TestMessage}";
        }
        catch (Exception ex)
        {
            MessageBox.Show(this, $"テスト送信に失敗しました:\n{ex.Message}",
                "Now Playing → VRChat", MessageBoxButton.OK, MessageBoxImage.Warning);
        }
    }

    // --- 設定 -------------------------------------------------------------

    private async void SettingsButton_Click(object sender, RoutedEventArgs e)
    {
        var dlg = new SettingsWindow(_settings) { Owner = this };
        if (dlg.ShowDialog() != true) return;

        _settings = dlg.Result;

        // 監視中（IsRunning）なら設定を即反映するために再接続
        if (_bridge?.IsRunning == true)
        {
            await StopAndDisposeBridgeAsync();
            StartBridge();
        }
    }

    private async Task StopAndDisposeBridgeAsync()
    {
        var bridge = _bridge;
        _bridge = null;
        if (bridge == null) return;

        bridge.Stop();
        try { await bridge.DisposeAsync(); }
        catch (Exception ex)
        {
            Debug.WriteLine($"[NowPlayingVrcOsc] Bridge dispose failed: {ex}");
        }
    }

    // --- 最小化 → トレイ / × → 終了 ------------------------------------

    private void Window_StateChanged(object sender, EventArgs e)
    {
        if (_exiting || WindowState != WindowState.Minimized) return;
        Hide();
        ShowInTaskbar = false;
    }

    private void Window_Closing(object sender, CancelEventArgs e)
    {
        if (_exiting) return;
        e.Cancel = true;
        ExitApp();
    }
}
