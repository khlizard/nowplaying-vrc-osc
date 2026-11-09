using System.IO;
using Velopack;

namespace NowPlayingVrcOsc.App;

public partial class App : System.Windows.Application
{
    [STAThread]
    private static void Main(string[] args)
    {
        // Velopack のフック処理。必ず最初に呼ぶ。
        VelopackApp.Build()
            .OnBeforeUninstallFastCallback(_ => DeleteUserData())
            .Run();

        var app = new App();
        app.InitializeComponent();
        app.Run();
    }

    // アンインストール時にユーザーデータ（settings.json / key.dpapi）を削除する
    private static void DeleteUserData()
    {
        try
        {
            var dir = Path.GetDirectoryName(Core.AppSettings.SettingsPath());
            if (!string.IsNullOrEmpty(dir) && Directory.Exists(dir))
                Directory.Delete(dir, recursive: true);
        }
        catch
        {
            // アンインストールを妨げない
        }
    }
}
