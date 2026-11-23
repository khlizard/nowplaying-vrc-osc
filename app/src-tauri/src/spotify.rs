//! Spotify oEmbed（認証不要）から曲名を取得する（C# 版 `SpotifyOEmbedClient`）。
//! 失敗しても panic せず None を返す。成功はメモリキャッシュ、失敗した ID は 10 分再試行しない、
//! 429 は Retry-After の間全体を停止する。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Duration, Utc};

use crate::http::{HttpClient, HttpResponse, UreqClient};
use crate::rekordbox::normalize::is_spotify_track_id;

pub const HOST: &str = "open.spotify.com";
const MAX_TITLE_LENGTH: usize = 500;
const DEFAULT_RETRY_AFTER_SECONDS: f64 = 60.0;
const MAX_RETRY_AFTER_SECONDS: f64 = 3600.0;
const FAILED_RETRY_MINUTES: i64 = 10;
const USER_AGENT: &str = "NowPlayingVrcOsc";

pub trait SpotifyTitleProvider: Send {
    fn get_title(&mut self, track_id: &str, stop: &AtomicBool) -> Option<String>;
}

type Clock = Box<dyn Fn() -> DateTime<Utc> + Send>;

pub struct SpotifyOEmbedClient {
    http: Box<dyn HttpClient>,
    clock: Clock,
    cache: HashMap<String, String>,
    failed_until: HashMap<String, DateTime<Utc>>,
    paused_until: DateTime<Utc>,
}

impl SpotifyOEmbedClient {
    pub fn new_default() -> Self {
        Self::new(Box::new(UreqClient::new()), Box::new(Utc::now))
    }

    /// HTTP と時計を差し替えられるコンストラクタ（テスト用）。
    pub fn new(http: Box<dyn HttpClient>, clock: Clock) -> Self {
        SpotifyOEmbedClient {
            http,
            clock,
            cache: HashMap::new(),
            failed_until: HashMap::new(),
            paused_until: DateTime::<Utc>::MIN_UTC,
        }
    }

    fn fail(&mut self, track_id: &str) -> Option<String> {
        let until = (self.clock)() + Duration::minutes(FAILED_RETRY_MINUTES);
        self.failed_until.insert(track_id.to_string(), until);
        None
    }

    fn pause(&mut self, seconds: i64) {
        self.paused_until = (self.clock)() + Duration::seconds(seconds);
    }

    fn retry_after_seconds(&self, res: &HttpResponse) -> i64 {
        let mut seconds = DEFAULT_RETRY_AFTER_SECONDS;
        if let Some(ra) = res.retry_after.as_deref().map(str::trim) {
            if let Ok(delta) = ra.parse::<u64>() {
                seconds = delta as f64;
            } else if let Ok(date) = DateTime::parse_from_rfc2822(ra) {
                let d = date.with_timezone(&Utc) - (self.clock)();
                seconds = d.num_milliseconds() as f64 / 1000.0;
            }
        }
        if seconds < 0.0 {
            seconds = DEFAULT_RETRY_AFTER_SECONDS;
        }
        seconds.ceil().min(MAX_RETRY_AFTER_SECONDS) as i64
    }
}

impl SpotifyTitleProvider for SpotifyOEmbedClient {
    fn get_title(&mut self, track_id: &str, stop: &AtomicBool) -> Option<String> {
        if !is_spotify_track_id(track_id) {
            return None;
        }

        if let Some(cached) = self.cache.get(track_id) {
            return Some(cached.clone());
        }
        let now = (self.clock)();
        if now < self.paused_until {
            return None;
        }
        if self.failed_until.get(track_id).is_some_and(|u| now < *u) {
            return None;
        }
        if stop.load(Ordering::Relaxed) {
            return None;
        }

        let url = format!(
            "https://{HOST}/oembed?url={}",
            escape_data_string(&format!("https://{HOST}/track/{track_id}"))
        );
        let Ok(res) = self.http.get(&url, Some(USER_AGENT)) else {
            return self.fail(track_id);
        };

        if res.status == 429 {
            let secs = self.retry_after_seconds(&res);
            self.pause(secs);
            return None;
        }
        if !res.is_success() {
            return self.fail(track_id);
        }

        let final_ok = url::Url::parse(&res.final_url).is_ok_and(|u| {
            u.scheme() == "https" && u.host_str().is_some_and(|h| h.eq_ignore_ascii_case(HOST))
        });
        if !final_ok {
            return self.fail(track_id);
        }

        let Some(title) = res.body.as_deref().and_then(parse_title) else {
            return self.fail(track_id);
        };
        self.cache.insert(track_id.to_string(), title.clone());
        Some(title)
    }
}

/// JSON の `title`（文字列）を取り出す。前後の空白を除き、空・500 文字超は None。
fn parse_title(body: &[u8]) -> Option<String> {
    let doc: serde_json::Value = serde_json::from_slice(body).ok()?;
    let s = doc.as_object()?.get("title")?.as_str()?.trim();
    // 長さは .NET の string.Length（UTF-16 コード単位）で数える
    if s.is_empty() || s.encode_utf16().count() > MAX_TITLE_LENGTH {
        None
    } else {
        Some(s.to_string())
    }
}

/// .NET の `Uri.EscapeDataString` と同じ（英数字と `-_.~` 以外を %XX にする）。
fn escape_data_string(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::fake::{ok, status, FakeHttp};
    use std::sync::{Arc, Mutex};

    const ID: &str = "0123456789abcdefghijAB";
    const ID2: &str = "0123456789abcdefghijCD";

    type Log = Arc<Mutex<Vec<(String, Option<String>)>>>;

    #[derive(Clone)]
    struct FakeClock(Arc<Mutex<DateTime<Utc>>>);

    impl FakeClock {
        fn new() -> Self {
            FakeClock(Arc::new(Mutex::new(
                DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
            )))
        }
        fn advance(&self, d: Duration) {
            *self.0.lock().unwrap() += d;
        }
        fn get(&self) -> Clock {
            let c = self.0.clone();
            Box::new(move || *c.lock().unwrap())
        }
    }

    fn make(http: FakeHttp, clock: &FakeClock) -> SpotifyOEmbedClient {
        SpotifyOEmbedClient::new(Box::new(http), clock.get())
    }

    fn no_stop() -> AtomicBool {
        AtomicBool::new(false)
    }

    fn json_ok(body: &'static str) -> FakeHttp {
        FakeHttp::new(move |u| ok(u, body))
    }

    fn log(h: &FakeHttp) -> Log {
        h.requests.clone()
    }

    #[test]
    fn 正確な_url_を要求し_前後の空白を除いた曲名を返し_キャッシュする() {
        let h = json_ok(r#"{"title":"  My Song  "}"#);
        let requests = log(&h);
        let mut c = make(h, &FakeClock::new());

        assert_eq!(c.get_title(ID, &no_stop()).as_deref(), Some("My Song"));
        assert_eq!(c.get_title(ID, &no_stop()).as_deref(), Some("My Song"));

        let r = requests.lock().unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(
            r[0].0,
            "https://open.spotify.com/oembed?url=https%3A%2F%2Fopen.spotify.com%2Ftrack%2F0123456789abcdefghijAB"
        );
        assert_eq!(r[0].1.as_deref(), Some("NowPlayingVrcOsc"));
    }

    #[test]
    fn 空または長すぎる曲名は失敗() {
        let mut c = make(json_ok(r#"{"title":"   "}"#), &FakeClock::new());
        assert_eq!(c.get_title(ID, &no_stop()), None);

        let long = format!("{{\"title\":\"{}\"}}", "a".repeat(501));
        let h2 = FakeHttp::new(move |u| ok(u, &long));
        let mut c2 = make(h2, &FakeClock::new());
        assert_eq!(c2.get_title(ID, &no_stop()), None);

        // 500 文字ちょうどは通る
        let exact = format!("{{\"title\":\"{}\"}}", "a".repeat(500));
        let h3 = FakeHttp::new(move |u| ok(u, &exact));
        let mut c3 = make(h3, &FakeClock::new());
        assert_eq!(c3.get_title(ID, &no_stop()).map(|s| s.len()), Some(500));
    }

    #[test]
    fn 本文が不正な_json_や_title_が文字列でないと失敗() {
        for body in ["not json", "[]", r#"{"title":1}"#, r#"{}"#] {
            let h = FakeHttp::new(move |u| ok(u, body));
            let mut c = make(h, &FakeClock::new());
            assert_eq!(c.get_title(ID, &no_stop()), None, "{body}");
        }
    }

    #[test]
    fn 上限超過で本文が無い応答は失敗() {
        let h = FakeHttp::new(|u| {
            Ok(HttpResponse {
                status: 200,
                final_url: u.to_string(),
                retry_after: None,
                body: None,
            })
        });
        let mut c = make(h, &FakeClock::new());
        assert_eq!(c.get_title(ID, &no_stop()), None);
    }

    #[test]
    fn 通信エラーは失敗() {
        let h = FakeHttp::new(|_| Err(crate::http::HttpError("x".into())));
        let mut c = make(h, &FakeClock::new());
        assert_eq!(c.get_title(ID, &no_stop()), None);
    }

    #[test]
    fn http429_は_retry_after_の間すべてのリクエストを止める() {
        let clock = FakeClock::new();
        let calls = Arc::new(Mutex::new(0));
        let c2 = calls.clone();
        let h = FakeHttp::new(move |u| {
            let mut n = c2.lock().unwrap();
            *n += 1;
            if *n == 1 {
                Ok(HttpResponse {
                    status: 429,
                    final_url: u.to_string(),
                    retry_after: Some("30".to_string()),
                    body: None,
                })
            } else {
                ok(u, r#"{"title":"OK"}"#)
            }
        });
        let mut c = make(h, &clock);

        assert_eq!(c.get_title(ID, &no_stop()), None);
        assert_eq!(*calls.lock().unwrap(), 1);

        // 停止中は別 ID でもリクエストしない
        clock.advance(Duration::seconds(29));
        assert_eq!(c.get_title(ID2, &no_stop()), None);
        assert_eq!(*calls.lock().unwrap(), 1);

        clock.advance(Duration::seconds(2));
        assert_eq!(c.get_title(ID2, &no_stop()).as_deref(), Some("OK"));
        // 429 の ID は失敗扱いではないので再試行できる
        assert_eq!(c.get_title(ID, &no_stop()).as_deref(), Some("OK"));
    }

    #[test]
    fn retry_after_の解釈() {
        let c = make(json_ok("{}"), &FakeClock::new());
        let res = |ra: Option<&str>| HttpResponse {
            status: 429,
            final_url: String::new(),
            retry_after: ra.map(str::to_string),
            body: None,
        };
        assert_eq!(c.retry_after_seconds(&res(None)), 60);
        assert_eq!(c.retry_after_seconds(&res(Some("30"))), 30);
        assert_eq!(c.retry_after_seconds(&res(Some("999999"))), 3600);
        assert_eq!(c.retry_after_seconds(&res(Some("garbage"))), 60);
        // 日付形式（基準時刻の 90 秒後）
        assert_eq!(
            c.retry_after_seconds(&res(Some("Thu, 01 Jan 2026 00:01:30 GMT"))),
            90
        );
        // 過去の日付は既定値
        assert_eq!(
            c.retry_after_seconds(&res(Some("Wed, 31 Dec 2025 23:00:00 GMT"))),
            60
        );
    }

    #[test]
    fn 別ホストへのリダイレクトは失敗() {
        let h = FakeHttp::new(|_| ok("https://evil.example.com/x", r#"{"title":"Evil"}"#));
        let mut c = make(h, &FakeClock::new());
        assert_eq!(c.get_title(ID, &no_stop()), None);
    }

    #[test]
    fn http_へのリダイレクトは失敗() {
        let h = FakeHttp::new(|_| ok("http://open.spotify.com/x", r#"{"title":"Evil"}"#));
        let mut c = make(h, &FakeClock::new());
        assert_eq!(c.get_title(ID, &no_stop()), None);
    }

    #[test]
    fn 失敗した_id_は_10_分再試行しない() {
        let clock = FakeClock::new();
        let fail = Arc::new(Mutex::new(true));
        let f2 = fail.clone();
        let h = FakeHttp::new(move |u| {
            if *f2.lock().unwrap() {
                status(u, 404)
            } else {
                ok(u, r#"{"title":"Now OK"}"#)
            }
        });
        let requests = log(&h);
        let mut c = make(h, &clock);

        assert_eq!(c.get_title(ID, &no_stop()), None);
        *fail.lock().unwrap() = false;
        clock.advance(Duration::minutes(9));
        assert_eq!(c.get_title(ID, &no_stop()), None);
        assert_eq!(requests.lock().unwrap().len(), 1);

        clock.advance(Duration::minutes(2));
        assert_eq!(c.get_title(ID, &no_stop()).as_deref(), Some("Now OK"));
    }

    #[test]
    fn 不正な_track_id_はリクエストしない() {
        let h = json_ok(r#"{"title":"x"}"#);
        let requests = log(&h);
        let mut c = make(h, &FakeClock::new());
        assert_eq!(c.get_title("short", &no_stop()), None);
        assert!(requests.lock().unwrap().is_empty());
    }

    #[test]
    fn 停止中はリクエストせず失敗扱いにもしない() {
        let h = json_ok(r#"{"title":"x"}"#);
        let requests = log(&h);
        let mut c = make(h, &FakeClock::new());
        assert_eq!(c.get_title(ID, &AtomicBool::new(true)), None);
        assert!(requests.lock().unwrap().is_empty());
        assert_eq!(c.get_title(ID, &no_stop()).as_deref(), Some("x"));
    }

    #[test]
    fn data_string_のエスケープ() {
        assert_eq!(escape_data_string("a-b_c.d~e"), "a-b_c.d~e");
        assert_eq!(
            escape_data_string("https://x/y z"),
            "https%3A%2F%2Fx%2Fy%20z"
        );
    }
}
