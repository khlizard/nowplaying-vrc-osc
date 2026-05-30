using System.IO;
using System.Windows;
using System.Windows.Media.Imaging;
using WpfApplication = System.Windows.Application;

namespace NowPlayingVrcOsc.App;

internal static class WindowIcon
{
    private const string PackUri = "pack://application:,,,/Assets/app.ico";

    public static void TryApply(Window window)
    {
        try
        {
            var stream = WpfApplication.GetResourceStream(new Uri(PackUri))?.Stream;
            if (stream != null)
            {
                window.Icon = BitmapFrame.Create(stream);
                return;
            }

            var path = Path.Combine(AppContext.BaseDirectory, "Assets", "app.ico");
            if (File.Exists(path))
                window.Icon = BitmapFrame.Create(new Uri(path, UriKind.Absolute));
        }
        catch
        {
            // アイコン未設定でも起動は継続する
        }
    }
}
