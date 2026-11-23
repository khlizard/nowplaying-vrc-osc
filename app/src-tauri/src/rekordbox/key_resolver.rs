//! 復号キーの解決（C# 版 `KeyResolver`）。順序: (1) DPAPI キャッシュ (2) URL 候補。
//! 各候補は DB を実際に開けたものだけ採用する。
//! 手動入力は App 側が verify_key → DpapiKeyStore::save で行うので (1) に含まれる。

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Duration, Utc};

use super::dpapi::{DpapiKeyStore, KeyStore};
use super::master_db::{verify_key, KeyCheckResult};
use super::url_key_source::UrlKeySource;

/// URL 候補で見つからなかったときに、次に取りに行くまで空ける時間（10 分）。
const URL_RETRY_INTERVAL_MINUTES: i64 = 10;

pub trait KeyResolver: Send {
    /// 復号キーを返す。見つからなければ None。
    fn resolve(&mut self, db_path: &Path, stop: &AtomicBool) -> Option<String>;

    /// キャッシュ済みの鍵を破棄する（鍵が使えなくなったとき）。
    fn invalidate(&mut self);
}

/// 既定構成（DPAPI キャッシュ + 埋め込みの固定 URL）。
pub fn default_key_resolver() -> Box<dyn KeyResolver> {
    Box::new(DefaultKeyResolver::new(
        Box::new(DpapiKeyStore::new(DpapiKeyStore::default_path())),
        Some(UrlKeySource::new_default()),
        Box::new(Utc::now),
        Box::new(verify_key),
    ))
}

type Clock = Box<dyn Fn() -> DateTime<Utc> + Send>;
type Verify = Box<dyn Fn(&Path, &str) -> KeyCheckResult + Send>;

pub struct DefaultKeyResolver {
    store: Box<dyn KeyStore>,
    urls: Option<UrlKeySource>,
    clock: Clock,
    verify: Verify,
    url_retry_after: DateTime<Utc>,
}

impl DefaultKeyResolver {
    pub fn new(
        store: Box<dyn KeyStore>,
        urls: Option<UrlKeySource>,
        clock: Clock,
        verify: Verify,
    ) -> Self {
        DefaultKeyResolver {
            store,
            urls,
            clock,
            verify,
            url_retry_after: DateTime::<Utc>::MIN_UTC,
        }
    }
}

impl KeyResolver for DefaultKeyResolver {
    fn resolve(&mut self, db_path: &Path, stop: &AtomicBool) -> Option<String> {
        // (1) キャッシュ
        if let Some(cached) = self.store.load() {
            match (self.verify)(db_path, &cached) {
                // 検証できない場合（Busy）は本読み取りで判定する
                KeyCheckResult::Ok | KeyCheckResult::Busy => return Some(cached),
                KeyCheckResult::WrongKey | KeyCheckResult::InvalidFormat => {
                    let _ = self.store.delete();
                }
                // DB 未検出・その他エラーは破棄せず待つ
                _ => return None,
            }
        }

        // (2) URL 候補
        let urls = self.urls.as_ref()?;
        if (self.clock)() < self.url_retry_after {
            return None;
        }

        let candidates = urls.fetch_candidates(stop);
        for candidate in candidates {
            if stop.load(Ordering::Relaxed) {
                return None;
            }
            match (self.verify)(db_path, &candidate) {
                KeyCheckResult::Ok => {
                    // 保存に失敗しても、この鍵は使える（次回また取得する）
                    let _ = self.store.save(&candidate);
                    return Some(candidate);
                }
                // 判定不能。バックオフせず次回再試行
                KeyCheckResult::Busy | KeyCheckResult::DbNotFound | KeyCheckResult::Error => {
                    return None
                }
                _ => {}
            }
        }

        // 停止で打ち切られたときはバックオフしない
        if stop.load(Ordering::Relaxed) {
            return None;
        }
        self.url_retry_after = (self.clock)() + Duration::minutes(URL_RETRY_INTERVAL_MINUTES);
        None
    }

    fn invalidate(&mut self) {
        let _ = self.store.delete();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::fake::{ok, FakeHttp};
    use std::io;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct MemStore(Arc<Mutex<Option<String>>>);

    impl KeyStore for MemStore {
        fn load(&self) -> Option<String> {
            self.0.lock().unwrap().clone()
        }
        fn save(&self, key: &str) -> io::Result<()> {
            *self.0.lock().unwrap() = Some(key.to_string());
            Ok(())
        }
        fn delete(&self) -> io::Result<()> {
            *self.0.lock().unwrap() = None;
            Ok(())
        }
    }

    fn no_stop() -> AtomicBool {
        AtomicBool::new(false)
    }

    #[test]
    fn url_候補は検証できたものだけ採用し_キャッシュに保存し_なければバックオフする() {
        let good = "a".repeat(64);
        let bad = "b".repeat(64);
        let fetches = Arc::new(Mutex::new(0));
        let f2 = fetches.clone();
        let body = format!("{bad} {good}");
        let http = FakeHttp::new(move |u| {
            *f2.lock().unwrap() += 1;
            ok(u, &body)
        });
        let urls = UrlKeySource::new(Box::new(http), vec!["https://example.com/a".to_string()]);

        let now = Arc::new(Mutex::new(
            DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        ));
        let n2 = now.clone();
        let accept = Arc::new(Mutex::new(good.clone()));
        let a2 = accept.clone();
        let store = MemStore::default();
        let mut resolver = DefaultKeyResolver::new(
            Box::new(store.clone()),
            Some(urls),
            Box::new(move || *n2.lock().unwrap()),
            Box::new(move |_, k| {
                if k == *a2.lock().unwrap() {
                    KeyCheckResult::Ok
                } else {
                    KeyCheckResult::WrongKey
                }
            }),
        );
        let db = Path::new("x");

        assert_eq!(resolver.resolve(db, &no_stop()), Some(good.clone()));
        assert_eq!(store.load(), Some(good));

        // 鍵が使えなくなった: 破棄 → 候補なしになるとバックオフ
        *accept.lock().unwrap() = "none".to_string();
        store.delete().unwrap();
        assert_eq!(resolver.resolve(db, &no_stop()), None);
        assert_eq!(*fetches.lock().unwrap(), 2);
        *now.lock().unwrap() += Duration::minutes(5);
        assert_eq!(resolver.resolve(db, &no_stop()), None);
        assert_eq!(*fetches.lock().unwrap(), 2);
        *now.lock().unwrap() += Duration::minutes(6);
        assert_eq!(resolver.resolve(db, &no_stop()), None);
        assert_eq!(*fetches.lock().unwrap(), 3);
    }

    #[test]
    fn キャッシュの鍵を先に使い_違う鍵なら削除する() {
        let key = "c".repeat(64);
        let store = MemStore(Arc::new(Mutex::new(Some(key.clone()))));
        let mut r1 = DefaultKeyResolver::new(
            Box::new(store.clone()),
            None,
            Box::new(Utc::now),
            Box::new(|_, _| KeyCheckResult::Ok),
        );
        assert_eq!(r1.resolve(Path::new("x"), &no_stop()), Some(key));

        let mut r2 = DefaultKeyResolver::new(
            Box::new(store.clone()),
            None,
            Box::new(Utc::now),
            Box::new(|_, _| KeyCheckResult::WrongKey),
        );
        assert_eq!(r2.resolve(Path::new("x"), &no_stop()), None);
        assert_eq!(store.load(), None);
    }

    #[test]
    fn キャッシュが_busy_なら破棄せず使う_db_未検出なら待つ() {
        let key = "c".repeat(64);
        let store = MemStore(Arc::new(Mutex::new(Some(key.clone()))));
        let mut busy = DefaultKeyResolver::new(
            Box::new(store.clone()),
            None,
            Box::new(Utc::now),
            Box::new(|_, _| KeyCheckResult::Busy),
        );
        assert_eq!(busy.resolve(Path::new("x"), &no_stop()), Some(key.clone()));

        let mut missing = DefaultKeyResolver::new(
            Box::new(store.clone()),
            None,
            Box::new(Utc::now),
            Box::new(|_, _| KeyCheckResult::DbNotFound),
        );
        assert_eq!(missing.resolve(Path::new("x"), &no_stop()), None);
        assert_eq!(store.load(), Some(key));
    }

    #[test]
    fn invalidate_はキャッシュを消す() {
        let store = MemStore(Arc::new(Mutex::new(Some("c".repeat(64)))));
        let mut r = DefaultKeyResolver::new(
            Box::new(store.clone()),
            None,
            Box::new(Utc::now),
            Box::new(|_, _| KeyCheckResult::Ok),
        );
        r.invalidate();
        assert_eq!(store.load(), None);
    }

    #[test]
    fn 停止中は取得せずバックオフもしない() {
        let fetches = Arc::new(Mutex::new(0));
        let f2 = fetches.clone();
        let http = FakeHttp::new(move |u| {
            *f2.lock().unwrap() += 1;
            ok(u, &"a".repeat(64))
        });
        let urls = UrlKeySource::new(Box::new(http), vec!["https://example.com/a".to_string()]);
        let mut r = DefaultKeyResolver::new(
            Box::new(MemStore::default()),
            Some(urls),
            Box::new(Utc::now),
            Box::new(|_, _| KeyCheckResult::WrongKey),
        );
        assert_eq!(r.resolve(Path::new("x"), &AtomicBool::new(true)), None);
        assert_eq!(*fetches.lock().unwrap(), 0);
        // バックオフされていないので、停止が解除されれば取得する
        assert_eq!(r.resolve(Path::new("x"), &no_stop()), None);
        assert_eq!(*fetches.lock().unwrap(), 1);
    }
}
