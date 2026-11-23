//! master.db のパス解決（C# 版 `RekordboxPaths`）。

use std::path::{Path, PathBuf};

/// master.db のパスを決める。
/// 優先順: 明示指定 → rekordboxAgent の options.json の db-path → 既定の %APPDATA%\Pioneer\rekordbox\master.db
pub fn resolve_db_path(override_path: &str) -> PathBuf {
    if !override_path.trim().is_empty() {
        return PathBuf::from(override_path);
    }

    let app_data = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let options = app_data
        .join("Pioneer")
        .join("rekordboxAgent")
        .join("storage")
        .join("options.json");
    if let Some(p) = read_db_path_from_options(&options) {
        return PathBuf::from(p);
    }

    app_data.join("Pioneer").join("rekordbox").join("master.db")
}

/// `{"options":[["db-path","..."],...]}` から db-path を取り出す。失敗時は None。
pub fn read_db_path_from_options(options_json_path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(options_json_path).ok()?;
    let doc: serde_json::Value = serde_json::from_str(&text).ok()?;
    let options = doc.as_object()?.get("options")?.as_array()?;
    for pair in options {
        let Some(pair) = pair.as_array() else {
            continue;
        };
        if pair.len() < 2 {
            continue;
        }
        if let (Some("db-path"), Some(v)) = (pair[0].as_str(), pair[1].as_str()) {
            if !v.trim().is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 明示指定が優先される() {
        assert_eq!(
            resolve_db_path(r"X:\a\master.db"),
            PathBuf::from(r"X:\a\master.db")
        );
    }

    #[test]
    fn options_json_から_db_path_を読む() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("options.json");
        std::fs::write(
            &f,
            r#"{"options":[["other","x"],["db-path","D:\\rb\\master.db"]]}"#,
        )
        .unwrap();
        assert_eq!(
            read_db_path_from_options(&f).as_deref(),
            Some(r"D:\rb\master.db")
        );
        std::fs::write(&f, "not json").unwrap();
        assert_eq!(read_db_path_from_options(&f), None);
    }

    #[test]
    fn options_json_が無い_空の値_型違いは_none() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("options.json");
        assert_eq!(read_db_path_from_options(&f), None);
        std::fs::write(
            &f,
            r#"{"options":[["db-path","  "],["db-path",1],"x",["db-path"]]}"#,
        )
        .unwrap();
        assert_eq!(read_db_path_from_options(&f), None);
        std::fs::write(&f, r#"[]"#).unwrap();
        assert_eq!(read_db_path_from_options(&f), None);
    }
}
