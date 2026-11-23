//! Chatbox 送信文の組み立て。C# 版の `MessageFormatOptions` / `TrackFormatter` に相当する。

use std::sync::OnceLock;

use regex::Regex;

use crate::track::TrackInfo;

/// Chatbox 送信文の組み立てオプション。
#[derive(Clone, Debug, PartialEq)]
pub struct MessageFormatOptions {
    pub header: String,
    pub separator: String,
    pub include_title: bool,
    pub include_artist: bool,
    pub include_album: bool,
    pub include_label: bool,
    pub include_comment: bool,
    pub strip_title_brackets: bool,
}

impl Default for MessageFormatOptions {
    fn default() -> Self {
        Self {
            header: "🎵".to_string(),
            separator: "-".to_string(),
            include_title: true,
            include_artist: true,
            include_album: false,
            include_label: false,
            include_comment: false,
            strip_title_brackets: true,
        }
    }
}

impl MessageFormatOptions {
    /// 区切りが空白のみのときは "-" にする。
    pub fn effective_separator(&self) -> String {
        if self.separator.trim().is_empty() {
            "-".to_string()
        } else {
            self.separator.trim().to_string()
        }
    }

    /// 項目間の join 用文字列（区切りの前後にスペースを付与）。
    pub fn separator_with_spaces(&self) -> String {
        format!(" {} ", self.effective_separator())
    }
}

fn brackets_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\s*\[[^\]]*\]").expect("正規表現が不正"))
}

/// 前後の空白を除き、空・none・unknown・n/a・? は空文字にする。
pub fn normalize_field(value: Option<&str>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    let text = value.trim();
    if text.is_empty() {
        return String::new();
    }
    const UNKNOWN: [&str; 4] = ["none", "unknown", "n/a", "?"];
    if UNKNOWN.iter().any(|u| text.eq_ignore_ascii_case(u)) {
        return String::new();
    }
    text.to_string()
}

/// `[～]` を空白ごと除く。
pub fn remove_brackets(text: &str) -> String {
    brackets_regex().replace_all(text, "").trim().to_string()
}

/// 選択項目を区切りで結合し、ヘッダを付ける。
pub fn format_chatbox_message(track: &TrackInfo, opts: &MessageFormatOptions) -> String {
    let mut parts: Vec<String> = Vec::new();
    if opts.include_title {
        let mut title = normalize_field(track.title.as_deref());
        if opts.strip_title_brackets && !title.is_empty() {
            title = remove_brackets(&title);
        }
        if !title.is_empty() {
            parts.push(title);
        }
    }
    let others = [
        (opts.include_artist, &track.artist),
        (opts.include_album, &track.album),
        (opts.include_label, &track.label),
        (opts.include_comment, &track.comment),
    ];
    for (include, value) in others {
        if include {
            let v = normalize_field(value.as_deref());
            if !v.is_empty() {
                parts.push(v);
            }
        }
    }

    let body = parts.join(&opts.separator_with_spaces());
    let header = opts.header.trim();
    if header.is_empty() {
        return body;
    }
    if body.is_empty() {
        return header.to_string();
    }
    format!("{header} {body}")
}

/// プレビュー用（ヘッダなしの本文のみ）。
pub fn format_track_body(track: &TrackInfo, opts: &MessageFormatOptions) -> String {
    let with_header = format_chatbox_message(track, opts);
    let header = opts.header.trim();
    if header.is_empty() {
        return with_header;
    }
    let prefix = format!("{header} ");
    if let Some(rest) = with_header.strip_prefix(&prefix) {
        return rest.to_string();
    }
    if with_header == header {
        return String::new();
    }
    with_header
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(title: Option<&str>, artist: Option<&str>) -> TrackInfo {
        TrackInfo {
            title: title.map(String::from),
            artist: artist.map(String::from),
            ..Default::default()
        }
    }

    #[test]
    fn 既定は_ヘッダ_曲名_アーティスト() {
        let t = track(Some("Title"), Some("Artist"));
        let o = MessageFormatOptions::default();
        assert_eq!(format_chatbox_message(&t, &o), "🎵 Title - Artist");
        assert_eq!(format_track_body(&t, &o), "Title - Artist");
    }

    #[test]
    fn アーティスト無しは曲名のみ() {
        let t = track(Some("Title"), None);
        assert_eq!(
            format_chatbox_message(&t, &MessageFormatOptions::default()),
            "🎵 Title"
        );
    }

    #[test]
    fn 空扱いの語は除かれる() {
        for w in ["none", "NONE", "Unknown", "n/a", "N/A", "?", "  ", ""] {
            assert_eq!(normalize_field(Some(w)), "", "{w}");
        }
        assert_eq!(normalize_field(None), "");
        assert_eq!(normalize_field(Some("  abc ")), "abc");
        let t = track(Some("Title"), Some("Unknown"));
        assert_eq!(
            format_chatbox_message(&t, &MessageFormatOptions::default()),
            "🎵 Title"
        );
    }

    #[test]
    fn 角括弧は除かれ丸括弧は残る() {
        let t = track(Some("Song [Remix] (Original Mix) [x]"), Some("A"));
        let o = MessageFormatOptions::default();
        assert_eq!(format_chatbox_message(&t, &o), "🎵 Song (Original Mix) - A");
        let keep = MessageFormatOptions {
            strip_title_brackets: false,
            ..Default::default()
        };
        assert_eq!(
            format_chatbox_message(&t, &keep),
            "🎵 Song [Remix] (Original Mix) [x] - A"
        );
        assert_eq!(remove_brackets("A [b]"), "A");
    }

    #[test]
    fn 区切りが空白なら_ハイフンにフォールバック() {
        let o = MessageFormatOptions {
            separator: "   ".to_string(),
            ..Default::default()
        };
        assert_eq!(o.effective_separator(), "-");
        assert_eq!(o.separator_with_spaces(), " - ");
        let o2 = MessageFormatOptions {
            separator: " / ".to_string(),
            ..Default::default()
        };
        assert_eq!(o2.effective_separator(), "/");
        let t = track(Some("T"), Some("A"));
        assert_eq!(format_chatbox_message(&t, &o2), "🎵 T / A");
    }

    #[test]
    fn ヘッダ空と本文空() {
        let t = track(Some("T"), Some("A"));
        let no_header = MessageFormatOptions {
            header: "  ".to_string(),
            ..Default::default()
        };
        assert_eq!(format_chatbox_message(&t, &no_header), "T - A");
        assert_eq!(format_track_body(&t, &no_header), "T - A");

        let empty = track(None, None);
        let o = MessageFormatOptions::default();
        assert_eq!(format_chatbox_message(&empty, &o), "🎵");
        assert_eq!(format_track_body(&empty, &o), "");
        assert_eq!(format_chatbox_message(&empty, &no_header), "");
    }

    #[test]
    fn 全項目は決まった順序() {
        let t = TrackInfo {
            id: Some("1".into()),
            title: Some("T".into()),
            artist: Some("A".into()),
            album: Some("Al".into()),
            label: Some("L".into()),
            comment: Some("C".into()),
        };
        let o = MessageFormatOptions {
            include_album: true,
            include_label: true,
            include_comment: true,
            ..Default::default()
        };
        assert_eq!(format_chatbox_message(&t, &o), "🎵 T - A - Al - L - C");
        let only_comment = MessageFormatOptions {
            include_title: false,
            include_artist: false,
            include_comment: true,
            ..Default::default()
        };
        assert_eq!(format_chatbox_message(&t, &only_comment), "🎵 C");
    }
}
