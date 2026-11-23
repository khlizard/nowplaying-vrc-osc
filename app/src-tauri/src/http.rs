//! 外部 HTTP 取得の共通処理。https のみ・5 秒タイムアウト・本文 1 MiB 上限。
//! C# 版の `HttpLimited` と、`HttpClient` 設定に相当する。テストではフェイクに差し替える。

use std::fmt;
use std::io::Read;
use std::time::Duration;

/// 本文の上限（1 MiB）。
pub const MAX_BYTES: usize = 1024 * 1024;
/// 1 リクエストの全体タイムアウト。
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// GET の結果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    /// リダイレクト後の最終 URL。
    pub final_url: String,
    /// `Retry-After` ヘッダの生の値。
    pub retry_after: Option<String>,
    /// 本文。成功（2xx）以外、または上限超過のときは `None`。
    pub body: Option<Vec<u8>>,
}

impl HttpResponse {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// 通信自体の失敗（接続・タイムアウト・https 以外など）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpError(pub String);

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for HttpError {}

pub trait HttpClient: Send {
    /// URL を GET する。`user_agent` が `None` なら既定。
    fn get(&self, url: &str, user_agent: Option<&str>) -> Result<HttpResponse, HttpError>;
}

/// 本文を上限付きで読む。超過時は `None`（読み取りエラーも `None`）。
pub fn read_limited(reader: &mut dyn Read, max_bytes: usize) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    // 上限ちょうどは許可し、1 バイト超えたら拒否する。
    let mut limited = reader.take(max_bytes as u64 + 1);
    limited.read_to_end(&mut buf).ok()?;
    if buf.len() > max_bytes {
        None
    } else {
        Some(buf)
    }
}

/// ureq の Agent による実装。
pub struct UreqClient {
    agent: ureq::Agent,
}

impl UreqClient {
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(REQUEST_TIMEOUT))
            .http_status_as_error(false)
            .https_only(true)
            .build();
        UreqClient {
            agent: ureq::Agent::new_with_config(config),
        }
    }
}

impl Default for UreqClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient for UreqClient {
    fn get(&self, url: &str, user_agent: Option<&str>) -> Result<HttpResponse, HttpError> {
        use ureq::ResponseExt;

        if !is_https(url) {
            return Err(HttpError("https 以外は使えません".to_string()));
        }
        let mut req = self.agent.get(url);
        if let Some(ua) = user_agent {
            req = req.header("User-Agent", ua);
        }
        let resp = req.call().map_err(|e| HttpError(e.to_string()))?;

        let status = resp.status().as_u16();
        let final_url = resp.get_uri().to_string();
        let header = |name: &str| {
            resp.headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        };
        let retry_after = header("retry-after");
        let too_large = header("content-length")
            .and_then(|v| v.trim().parse::<u64>().ok())
            .is_some_and(|len| len > MAX_BYTES as u64);

        let success = (200..300).contains(&status);
        let body = if success && !too_large {
            let mut reader = resp.into_body().into_reader();
            read_limited(&mut reader, MAX_BYTES)
        } else {
            None
        };
        Ok(HttpResponse {
            status,
            final_url,
            retry_after,
            body,
        })
    }
}

/// 絶対 URL で、スキームが https か。
pub fn is_https(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|u| u.scheme() == "https")
}

#[cfg(test)]
pub(crate) mod fake {
    use super::*;
    use std::sync::{Arc, Mutex};

    type Responder = Box<dyn Fn(&str) -> Result<HttpResponse, HttpError> + Send>;

    /// テスト用のフェイク。リクエスト（URL と User-Agent）を記録する。
    pub struct FakeHttp {
        respond: Responder,
        pub requests: Arc<Mutex<Vec<(String, Option<String>)>>>,
    }

    impl FakeHttp {
        pub fn new(
            respond: impl Fn(&str) -> Result<HttpResponse, HttpError> + Send + 'static,
        ) -> Self {
            FakeHttp {
                respond: Box::new(respond),
                requests: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl HttpClient for FakeHttp {
        fn get(&self, url: &str, user_agent: Option<&str>) -> Result<HttpResponse, HttpError> {
            self.requests
                .lock()
                .unwrap()
                .push((url.to_string(), user_agent.map(str::to_string)));
            (self.respond)(url)
        }
    }

    pub fn ok(url: &str, body: &str) -> Result<HttpResponse, HttpError> {
        Ok(HttpResponse {
            status: 200,
            final_url: url.to_string(),
            retry_after: None,
            body: Some(body.as_bytes().to_vec()),
        })
    }

    pub fn status(url: &str, code: u16) -> Result<HttpResponse, HttpError> {
        Ok(HttpResponse {
            status: code,
            final_url: url.to_string(),
            retry_after: None,
            body: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 上限ちょうどは読める() {
        let data = vec![b'a'; 10];
        assert_eq!(read_limited(&mut data.as_slice(), 10), Some(data.clone()));
    }

    #[test]
    fn 上限を超えると_none() {
        let data = vec![b'a'; 11];
        assert_eq!(read_limited(&mut data.as_slice(), 10), None);
    }

    #[test]
    fn 空の本文は読める() {
        assert_eq!(read_limited(&mut [].as_slice(), 10), Some(vec![]));
    }

    #[test]
    fn https_判定() {
        assert!(is_https("https://example.com/a"));
        assert!(!is_https("http://example.com/a"));
        assert!(!is_https("not a url"));
        assert!(!is_https("file:///C:/a"));
    }

    #[test]
    fn 実クライアントは_https_以外を通信せず拒否する() {
        let c = UreqClient::new();
        assert!(c.get("http://127.0.0.1:9/a", None).is_err());
    }
}
