//! djmdContent の値の正規化（C# 版 `ContentNormalizer`）。

use std::sync::LazyLock;

use regex::Regex;

use crate::track::TrackInfo;

/// djmdContent の 1 行分の取得結果（名前は各テーブルの Name を解決済み）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContentRow {
    pub title: Option<String>,
    pub folder_path: Option<String>,
    pub comment: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub label: Option<String>,
}

static ENCRYPTED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\$A[0-9]+:v[0-9]+:").unwrap());
// `$` は regex クレートでは末尾にだけ一致する（末尾の改行は許さない）。
static SPOTIFY_TRACK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^spotify:track:([A-Za-z0-9]{22})$").unwrap());

/// rekordbox が暗号化して保存している値（Spotify 由来など）か。
pub fn is_encrypted(value: Option<&str>) -> bool {
    value.is_some_and(|v| ENCRYPTED.is_match(v))
}

/// null・空・暗号化値は None にする。
pub fn normalize(value: Option<&str>) -> Option<String> {
    match value {
        Some(v) if !v.is_empty() && !is_encrypted(Some(v)) => Some(v.to_string()),
        _ => None,
    }
}

/// `spotify:track:<22 文字>` から ID を取り出す。
pub fn try_get_spotify_track_id(folder_path: Option<&str>) -> Option<String> {
    let caps = SPOTIFY_TRACK.captures(folder_path?)?;
    Some(caps[1].to_string())
}

pub fn is_spotify_track_id(id: &str) -> bool {
    try_get_spotify_track_id(Some(&format!("spotify:track:{id}"))).is_some()
}

pub fn build(content_id: Option<&str>, c: Option<&ContentRow>) -> TrackInfo {
    TrackInfo {
        id: content_id.map(str::to_string),
        title: normalize(c.and_then(|c| c.title.as_deref())),
        artist: normalize(c.and_then(|c| c.artist.as_deref())),
        album: normalize(c.and_then(|c| c.album.as_deref())),
        label: normalize(c.and_then(|c| c.label.as_deref())),
        comment: normalize(c.and_then(|c| c.comment.as_deref())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 暗号化値の判定() {
        for (s, expected) in [
            ("$A7:v1:x", true),
            ("$A12:v10:abc", true),
            ("plain title", false),
            ("$A:v1:x", false),
            ("", false),
        ] {
            assert_eq!(is_encrypted(Some(s)), expected, "{s}");
        }
        assert!(!is_encrypted(None));
    }

    #[test]
    fn normalize_は_null_空_暗号化値を_none_にする() {
        assert_eq!(normalize(None), None);
        assert_eq!(normalize(Some("")), None);
        assert_eq!(normalize(Some("$A7:v1:x")), None);
        assert_eq!(normalize(Some("Song")), Some("Song".to_string()));
    }

    #[test]
    fn spotify_id_は_22_文字を受け付ける() {
        let id = "a".repeat(22);
        assert_eq!(
            try_get_spotify_track_id(Some(&format!("spotify:track:{id}"))),
            Some(id)
        );
    }

    #[test]
    fn spotify_id_は_21_文字を拒否する() {
        let s = format!("spotify:track:{}", "a".repeat(21));
        assert_eq!(try_get_spotify_track_id(Some(&s)), None);
    }

    #[test]
    fn spotify_id_は末尾の改行を拒否する() {
        let s = format!("spotify:track:{}\n", "a".repeat(22));
        assert_eq!(try_get_spotify_track_id(Some(&s)), None);
    }

    #[test]
    fn build_は項目を対応づける() {
        let c = ContentRow {
            title: Some("T".into()),
            folder_path: Some("x".into()),
            comment: Some("memo".into()),
            artist: Some("$A1:v1:z".into()),
            album: Some("Alb".into()),
            label: None,
        };
        let t = build(Some("c1"), Some(&c));
        assert_eq!(t.id.as_deref(), Some("c1"));
        assert_eq!(t.title.as_deref(), Some("T"));
        assert_eq!(t.artist, None);
        assert_eq!(t.album.as_deref(), Some("Alb"));
        assert_eq!(t.comment.as_deref(), Some("memo"));
    }
}
