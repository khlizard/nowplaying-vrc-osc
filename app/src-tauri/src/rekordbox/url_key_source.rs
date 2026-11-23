//! 固定 URL のテキストから 64 桁 hex の候補鍵を抽出する（C# 版 `UrlKeySource`）。
//! https のみ・5 秒タイムアウト・1 MiB 上限・最終 URL も https 必須。

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;

use regex::Regex;

use crate::http::{is_https, HttpClient, UreqClient};

pub const MAX_CANDIDATES: usize = 20;

const DEFAULT_SOURCES_JSON: &str = include_str!("default-key-sources.json");

// 16 進の連続部分のうち、ちょうど 64 桁のものだけを候補にする（前後に 16 進が続くものは除く）。
static HEX_RUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[0-9a-fA-F]+").unwrap());

pub struct UrlKeySource {
    http: Box<dyn HttpClient>,
    urls: Vec<String>,
}

impl UrlKeySource {
    pub fn new_default() -> Self {
        Self::new(Box::new(UreqClient::new()), load_default_urls())
    }

    pub fn new(http: Box<dyn HttpClient>, urls: Vec<String>) -> Self {
        UrlKeySource { http, urls }
    }

    /// 全 URL から候補鍵を集める（重複除去・最大 20 件）。個々の失敗は無視する。
    /// `stop` が true なら、各リクエストの前に打ち切って、そこまでの結果を返す。
    pub fn fetch_candidates(&self, stop: &AtomicBool) -> Vec<String> {
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        for url in &self.urls {
            if stop.load(Ordering::Relaxed) {
                return result;
            }
            let Some(text) = self.fetch_text(url) else {
                continue;
            };
            for m in HEX_RUN.find_iter(&text) {
                if m.as_str().len() != 64 {
                    continue;
                }
                if seen.insert(m.as_str().to_string()) {
                    result.push(m.as_str().to_string());
                }
                if result.len() >= MAX_CANDIDATES {
                    return result;
                }
            }
        }
        result
    }

    fn fetch_text(&self, url: &str) -> Option<String> {
        if !is_https(url) {
            return None;
        }
        let res = self.http.get(url, None).ok()?;
        if !res.is_success() || !is_https(&res.final_url) {
            return None;
        }
        let bytes = res.body?;
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }
}

pub fn load_default_urls() -> Vec<String> {
    serde_json::from_str(DEFAULT_SOURCES_JSON).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::fake::{ok, FakeHttp};
    use crate::http::HttpResponse;

    fn hex(c: char, n: usize) -> String {
        c.to_string().repeat(n)
    }

    fn no_stop() -> AtomicBool {
        AtomicBool::new(false)
    }

    fn source(body: String, urls: &[&str]) -> (UrlKeySource, FakeHttpLog) {
        let http = FakeHttp::new(move |u| ok(u, &body));
        let log = http.requests.clone();
        (
            UrlKeySource::new(Box::new(http), urls.iter().map(|s| s.to_string()).collect()),
            log,
        )
    }

    type FakeHttpLog = std::sync::Arc<std::sync::Mutex<Vec<(String, Option<String>)>>>;

    #[test]
    fn 六十四桁の_hex_だけ抽出し_重複を除く() {
        let k1 = hex('a', 64);
        let k2 = hex('B', 64);
        let body = format!(
            "key1 = \"{k1}\" again {k1} and '{k2}' too-long {} short {}",
            hex('c', 65),
            hex('d', 63)
        );
        let (src, _) = source(body, &["https://example.com/a"]);
        assert_eq!(src.fetch_candidates(&no_stop()), vec![k1, k2]);
    }

    #[test]
    fn 候補は最大_20_件() {
        let body = (0..30)
            .map(|i| format!("{i:064x}"))
            .collect::<Vec<_>>()
            .join(" ");
        let (src, _) = source(body, &["https://example.com/a"]);
        assert_eq!(src.fetch_candidates(&no_stop()).len(), 20);
    }

    #[test]
    fn https_以外はリクエストせず拒否する() {
        let (src, log) = source(hex('a', 64), &["http://example.com/a"]);
        assert!(src.fetch_candidates(&no_stop()).is_empty());
        assert!(log.lock().unwrap().is_empty());
    }

    #[test]
    fn http_へのリダイレクトは拒否する() {
        let http = FakeHttp::new(|_| {
            Ok(HttpResponse {
                status: 200,
                final_url: "http://example.com/a".to_string(),
                retry_after: None,
                body: Some(hex('a', 64).into_bytes()),
            })
        });
        let src = UrlKeySource::new(Box::new(http), vec!["https://example.com/a".to_string()]);
        assert!(src.fetch_candidates(&no_stop()).is_empty());
    }

    #[test]
    fn 上限超過で本文が無い応答は拒否する() {
        // 実際の上限判定は http::read_limited（1 MiB 超は None）で行う。
        let http = FakeHttp::new(|u| {
            Ok(HttpResponse {
                status: 200,
                final_url: u.to_string(),
                retry_after: None,
                body: None,
            })
        });
        let src = UrlKeySource::new(Box::new(http), vec!["https://example.com/a".to_string()]);
        assert!(src.fetch_candidates(&no_stop()).is_empty());
    }

    #[test]
    fn 失敗した_url_は無視して次へ進む() {
        let k = hex('a', 64);
        let k2 = k.clone();
        let http = FakeHttp::new(move |u| {
            if u.ends_with("/bad") {
                crate::http::fake::status(u, 404)
            } else {
                ok(u, &k2)
            }
        });
        let src = UrlKeySource::new(
            Box::new(http),
            vec![
                "https://example.com/bad".to_string(),
                "https://example.com/good".to_string(),
            ],
        );
        assert_eq!(src.fetch_candidates(&no_stop()), vec![k]);
    }

    #[test]
    fn 停止中はリクエストしない() {
        let (src, log) = source(hex('a', 64), &["https://example.com/a"]);
        assert!(src.fetch_candidates(&AtomicBool::new(true)).is_empty());
        assert!(log.lock().unwrap().is_empty());
    }

    #[test]
    fn 既定のソースは_https_の_url_が二つ() {
        let urls = load_default_urls();
        assert_eq!(urls.len(), 2);
        assert!(urls
            .iter()
            .all(|u| u.starts_with("https://raw.githubusercontent.com/")));
    }
}
