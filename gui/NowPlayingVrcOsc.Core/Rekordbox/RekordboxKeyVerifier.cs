namespace NowPlayingVrcOsc.Core.Rekordbox;

/// <summary>
/// 設定画面で手入力された復号キーを検証するための公開窓口。
/// DB を開く処理そのものは MasterDb に閉じ込めたまま公開する。
/// </summary>
public static class RekordboxKeyVerifier
{
    /// <summary>
    /// キーで master.db を開けるか 1 回だけ確認する。
    /// rekordbox が起動していない（-wal が無い）ときもファイルを作らずに確認できる。
    /// </summary>
    public static KeyCheckResult Verify(string dbPath, string key) => MasterDb.VerifyKey(dbPath, key);
}
