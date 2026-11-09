using System.Runtime.Versioning;
using System.Security.Cryptography;
using System.Text;

namespace NowPlayingVrcOsc.Core.Rekordbox;

/// <summary>復号キーの保存先（テストで差し替え可能にするための抽象）。</summary>
internal interface IKeyStore
{
    string? Load();
    void Save(string key);
    void Delete();
}

/// <summary>復号キーを Windows DPAPI（現在のユーザー）で暗号化して保存する。鍵は平文で残さない。</summary>
[SupportedOSPlatform("windows")]
public sealed class DpapiKeyStore : IKeyStore
{
    private static readonly byte[] Entropy = Encoding.UTF8.GetBytes("NowPlayingVrcOsc");

    private readonly string _path;

    public DpapiKeyStore(string path) => _path = path;

    public static string DefaultPath() =>
        Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "NowPlayingVrcOsc", "key.dpapi");

    /// <summary>保存済みの鍵。無い・壊れている場合は null。</summary>
    public string? Load()
    {
        try
        {
            if (!File.Exists(_path)) return null;
            var plain = ProtectedData.Unprotect(File.ReadAllBytes(_path), Entropy, DataProtectionScope.CurrentUser);
            var s = Encoding.UTF8.GetString(plain);
            return string.IsNullOrEmpty(s) ? null : s;
        }
        catch
        {
            return null;
        }
    }

    public void Save(string key)
    {
        var dir = Path.GetDirectoryName(_path);
        if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);
        var protectedBytes = ProtectedData.Protect(Encoding.UTF8.GetBytes(key), Entropy, DataProtectionScope.CurrentUser);
        var tmp = _path + ".tmp";
        File.WriteAllBytes(tmp, protectedBytes);
        File.Move(tmp, _path, overwrite: true);
    }

    public void Delete()
    {
        try
        {
            if (File.Exists(_path)) File.Delete(_path);
        }
        catch
        {
            // 削除失敗は無視（次回の検証で弾かれる）
        }
    }
}
