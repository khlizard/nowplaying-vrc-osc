using System.Drawing;
using System.IO;
using WpfApplication = System.Windows.Application;

namespace NowPlayingVrcOsc.App;

internal static class AppIcon
{
    private const string PackUri = "pack://application:,,,/Assets/app.ico";
    private const string OutputRelativePath = "Assets/app.ico";

    public static Icon LoadTrayIcon()
    {
        var stream = WpfApplication.GetResourceStream(new Uri(PackUri))?.Stream;
        if (stream != null)
            return new Icon(stream);

        var path = Path.Combine(AppContext.BaseDirectory, OutputRelativePath);
        if (File.Exists(path))
            return new Icon(path);

        return SystemIcons.Application;
    }
}
