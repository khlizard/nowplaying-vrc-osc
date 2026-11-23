/// rekordbox から取得した 1 曲分の情報。C# 版の `TrackInfo` に相当する。
/// `id` は `djmdContent.ID`（重複抑制に使う）。値が無い項目は `None`。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrackInfo {
    pub id: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub label: Option<String>,
    pub comment: Option<String>,
}
