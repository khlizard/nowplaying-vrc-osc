//! 復号キーを Windows DPAPI（現在のユーザー）で暗号化して保存する（C# 版 `DpapiKeyStore`）。
//! 鍵は平文で残さない。v2 の key.dpapi とバイト互換（entropy は "NowPlayingVrcOsc" の UTF-8、ヘッダなし）。

use std::io;
use std::path::PathBuf;

const ENTROPY: &[u8] = b"NowPlayingVrcOsc";

/// 復号キーの保存先（テストで差し替え可能にするための抽象）。
pub trait KeyStore: Send {
    fn load(&self) -> Option<String>;
    fn save(&self, key: &str) -> io::Result<()>;
    fn delete(&self) -> io::Result<()>;
}

pub struct DpapiKeyStore {
    path: PathBuf,
}

impl DpapiKeyStore {
    pub fn new(path: PathBuf) -> Self {
        DpapiKeyStore { path }
    }

    /// `%APPDATA%\NowPlayingVrcOsc\key.dpapi`
    pub fn default_path() -> PathBuf {
        crate::settings::data_dir().join("key.dpapi")
    }

    pub fn exists(&self) -> bool {
        self.path.is_file()
    }

    /// 保存済みの鍵。無い・壊れている場合は None。
    pub fn load(&self) -> Option<String> {
        let bytes = std::fs::read(&self.path).ok()?;
        let plain = imp::unprotect(&bytes, ENTROPY).ok()?;
        let s = String::from_utf8_lossy(&plain).into_owned();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }

    pub fn save(&self, key: &str) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            if !dir.as_os_str().is_empty() {
                std::fs::create_dir_all(dir)?;
            }
        }
        let protected = imp::protect(key.as_bytes(), ENTROPY)?;
        let mut tmp = self.path.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        std::fs::write(&tmp, protected)?;
        std::fs::rename(&tmp, &self.path)
    }

    /// 削除する。無ければ成功扱い。
    pub fn delete(&self) -> io::Result<()> {
        match std::fs::remove_file(&self.path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

impl KeyStore for DpapiKeyStore {
    fn load(&self) -> Option<String> {
        DpapiKeyStore::load(self)
    }
    fn save(&self, key: &str) -> io::Result<()> {
        DpapiKeyStore::save(self, key)
    }
    fn delete(&self) -> io::Result<()> {
        DpapiKeyStore::delete(self)
    }
}

#[cfg(windows)]
mod imp {
    use std::io;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        }
    }

    /// DPAPI が確保した出力ブロブをコピーして解放する。`wipe` なら解放前に 0 で埋める。
    ///
    /// # Safety
    /// `out` は DPAPI が `LocalAlloc` で確保した有効なブロブであること。
    unsafe fn take_output(out: CRYPT_INTEGER_BLOB, wipe: bool) -> Vec<u8> {
        if out.pbData.is_null() {
            return Vec::new();
        }
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        if wipe {
            std::ptr::write_bytes(out.pbData, 0, out.cbData as usize);
        }
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut core::ffi::c_void)));
        v
    }

    pub fn protect(data: &[u8], entropy: &[u8]) -> io::Result<Vec<u8>> {
        let din = blob(data);
        let ent = blob(entropy);
        let mut out = CRYPT_INTEGER_BLOB::default();
        // SAFETY: 入力ブロブは呼び出し中だけ有効なスライスを指す。出力は take_output で解放する。
        unsafe {
            CryptProtectData(
                &din,
                PCWSTR::null(),
                Some(&ent),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
            .map_err(|e| io::Error::other(format!("DPAPI 暗号化に失敗しました: {e}")))?;
            Ok(take_output(out, false))
        }
    }

    pub fn unprotect(data: &[u8], entropy: &[u8]) -> io::Result<Vec<u8>> {
        let din = blob(data);
        let ent = blob(entropy);
        let mut out = CRYPT_INTEGER_BLOB::default();
        // SAFETY: 同上。平文は解放前に 0 で埋める。
        unsafe {
            CryptUnprotectData(
                &din,
                None,
                Some(&ent),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
            .map_err(|e| io::Error::other(format!("DPAPI 復号に失敗しました: {e}")))?;
            Ok(take_output(out, true))
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::io;

    pub fn protect(_data: &[u8], _entropy: &[u8]) -> io::Result<Vec<u8>> {
        Err(io::Error::other("DPAPI は Windows でのみ使えます"))
    }

    pub fn unprotect(_data: &[u8], _entropy: &[u8]) -> io::Result<Vec<u8>> {
        Err(io::Error::other("DPAPI は Windows でのみ使えます"))
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    fn store(dir: &tempfile::TempDir) -> DpapiKeyStore {
        DpapiKeyStore::new(dir.path().join("sub").join("key.dpapi"))
    }

    #[test]
    fn 保存して読める_平文はファイルに残らない() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        assert!(!s.exists());
        let key = "ab".repeat(32);
        s.save(&key).unwrap();
        assert!(s.exists());
        assert_eq!(s.load().as_deref(), Some(key.as_str()));

        let raw = std::fs::read(&s.path).unwrap();
        assert!(!raw.windows(key.len()).any(|w| w == key.as_bytes()));
        // 一時ファイルは残らない
        assert!(!s.path.with_extension("dpapi.tmp").exists());
    }

    #[test]
    fn 上書き保存できる() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        s.save(&"a".repeat(64)).unwrap();
        s.save(&"b".repeat(64)).unwrap();
        assert_eq!(s.load(), Some("b".repeat(64)));
    }

    #[test]
    fn 無い_壊れている場合は_none() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        assert_eq!(s.load(), None);
        std::fs::create_dir_all(s.path.parent().unwrap()).unwrap();
        std::fs::write(&s.path, b"not a dpapi blob").unwrap();
        assert_eq!(s.load(), None);
    }

    #[test]
    fn 別の_entropy_で保存されたものは読めない() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        std::fs::create_dir_all(s.path.parent().unwrap()).unwrap();
        std::fs::write(&s.path, imp::protect(b"secret", b"other").unwrap()).unwrap();
        assert_eq!(s.load(), None);
    }

    #[test]
    fn 削除できる_無くても成功() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        s.delete().unwrap();
        s.save(&"a".repeat(64)).unwrap();
        s.delete().unwrap();
        assert!(!s.exists());
        assert_eq!(s.load(), None);
    }

    #[test]
    fn 既定のパスは_data_dir_の_key_dpapi() {
        assert_eq!(
            DpapiKeyStore::default_path(),
            crate::settings::data_dir().join("key.dpapi")
        );
    }
}
