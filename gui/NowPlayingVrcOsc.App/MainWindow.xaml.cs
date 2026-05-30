using System.ComponentModel;
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
        Show();
        WindowState = WindowState.Normal;
        Activate();
    }

    private void ExitApp()
    {
        _exiting = true;
        _bridge?.Stop();
        if (_trayIcon != null)
        {
            _trayIcon.Visible = false;
            _trayIcon.Dispose();
            _trayIcon = null;
        }
        Application.Current.Shutdown();
    }

    // --- Bridge 開始 / 停止 ----------------------------------------------

    private void StartButton_Click(object sender, RoutedEventArgs e) => StartBridge();

    private void StartBridge()
    {
        var bridge = new BridgeService(_settings);
        _bridge = bridge;

        _bridge.TrackChanged += track =>
            Dispatcher.Invoke(() =>
                CurrentTrackText.Text = TrackFormatter.FormatTrack(
                    track, _settings.ShowLabel, _settings.StripTitleBrackets));

        _bridge.MessageSent += msg =>
            Dispatcher.Invoke(() =>
                LastSentText.Text = $"{DateTime.Now:HH:mm:ss}  {msg}");

        StartButton.IsEnabled = false;
        StopButton.IsEnabled = true;
        StatusText.Text = $"監視中 (config.json: {_settings.ConfigPath})";

        _ = bridge.StartAsync().ContinueWith(_ =>
            Dispatcher.Invoke(() =>
            {
                // 別の bridge に差し替え済み（再起動 / 手動停止）なら UI を触らない
                if (_exiting || !ReferenceEquals(_bridge, bridge)) return;
                StatusText.Text = "停止中";
                StartButton.IsEnabled = true;
                StopButton.IsEnabled = false;
            }));
    }

    private void StopButton_Click(object sender, RoutedEventArgs e)
    {
        _bridge?.Stop();
        _bridge = null;
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

    private void SettingsButton_Click(object sender, RoutedEventArgs e)
    {
        var dlg = new SettingsWindow(_settings) { Owner = this };
        if (dlg.ShowDialog() != true) return;

        _settings = dlg.Result;

        // 監視中なら新しい設定で再起動して即反映
        if (_bridge != null)
        {
            _bridge.Stop();
            _bridge = null;
            StartBridge();
        }
    }

    // --- 閉じる → トレイ格納 ---------------------------------------------

    private void Window_Closing(object sender, CancelEventArgs e)
    {
        if (_exiting) return;
        e.Cancel = true;
        Hide();
    }
}
