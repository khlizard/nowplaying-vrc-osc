//! 重複抑制。C# 版の `TrackDeduplicator` に相当する。

use crate::track::TrackInfo;

#[derive(Default)]
pub struct TrackDeduplicator {
    last_key: Option<String>,
    initial_seen: bool,
}

/// キーは Id（空でなければ）、無ければ曲名。
fn track_key(track: &TrackInfo) -> String {
    match track.id.as_deref() {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => track.title.clone().unwrap_or_default(),
    }
}

impl TrackDeduplicator {
    pub fn new() -> Self {
        Self::default()
    }

    /// 送るべき新しい曲なら true。Reset 後の最初の 1 曲は基準として記録するだけで false。
    pub fn is_new_track(&mut self, track: &TrackInfo) -> bool {
        let key = track_key(track);
        if key.is_empty() {
            return false;
        }
        if !self.initial_seen {
            self.initial_seen = true;
            self.last_key = Some(key);
            return false;
        }
        if self.last_key.as_deref() == Some(key.as_str()) {
            return false;
        }
        self.last_key = Some(key);
        true
    }

    pub fn reset(&mut self) {
        self.last_key = None;
        self.initial_seen = false;
    }

    /// 既知の基準曲で初期化し、初回スキップを無効にする。None でも無効になる。
    pub fn seed(&mut self, known_current: Option<&TrackInfo>) {
        self.initial_seen = true;
        self.last_key = known_current.map(track_key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(id: Option<&str>, title: &str) -> TrackInfo {
        TrackInfo {
            id: id.map(String::from),
            title: Some(title.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn 初回は基準として扱い_false() {
        let mut d = TrackDeduplicator::new();
        assert!(!d.is_new_track(&t(Some("1"), "A")));
        assert!(d.is_new_track(&t(Some("2"), "B")));
    }

    #[test]
    fn 同じ_id_は_false() {
        let mut d = TrackDeduplicator::new();
        d.is_new_track(&t(Some("1"), "A"));
        assert!(!d.is_new_track(&t(Some("1"), "別タイトル")));
        assert!(d.is_new_track(&t(Some("2"), "A")));
    }

    #[test]
    fn id_無しは曲名で判定() {
        let mut d = TrackDeduplicator::new();
        d.is_new_track(&t(None, "A"));
        assert!(!d.is_new_track(&t(None, "A")));
        assert!(d.is_new_track(&t(None, "B")));
        assert!(!d.is_new_track(&t(Some(""), "B")));
    }

    #[test]
    fn キーが空なら_false() {
        let mut d = TrackDeduplicator::new();
        assert!(!d.is_new_track(&TrackInfo::default()));
        d.seed(None);
        assert!(!d.is_new_track(&TrackInfo::default()));
    }

    #[test]
    fn reset_で初回扱いに戻る() {
        let mut d = TrackDeduplicator::new();
        d.is_new_track(&t(Some("1"), "A"));
        d.reset();
        assert!(!d.is_new_track(&t(Some("2"), "B")));
        assert!(d.is_new_track(&t(Some("1"), "A")));
    }

    #[test]
    fn seed_none_は最初の曲から送る() {
        let mut d = TrackDeduplicator::new();
        d.seed(None);
        assert!(d.is_new_track(&t(Some("1"), "A")));
    }

    #[test]
    fn seed_既知曲は同じ曲を送らない() {
        let mut d = TrackDeduplicator::new();
        let cur = t(Some("1"), "A");
        d.seed(Some(&cur));
        assert!(!d.is_new_track(&cur));
        assert!(d.is_new_track(&t(Some("2"), "B")));
    }
}
