//! 設定（settings.json）。C# 版の `AppSettings` と同じキー名・移行条件で読み書きする。

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::format::MessageFormatOptions;

fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct AppSettings {
    /// rekordbox master.db のパス。空のときは自動検出。
    pub db_path_override: String,
    pub poll_interval_seconds: f64,
    pub vrc_host: String,
    pub vrc_port: i32,
    pub chatbox_notification: bool,
    pub chatbox_indicator: bool,
    pub msg_header: String,
    pub msg_separator: String,
    pub msg_include_title: bool,
    pub msg_include_artist: bool,
    pub msg_include_album: bool,
    pub msg_include_label: bool,
    pub msg_include_comment: bool,
    pub strip_title_brackets: bool,
    /// 旧設定。読み込み時に msg_include_label へ移行する。false のときは書き出さない。
    #[serde(skip_serializing_if = "is_false")]
    pub show_label: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            db_path_override: String::new(),
            poll_interval_seconds: 1.0,
            vrc_host: "127.0.0.1".to_string(),
            vrc_port: 9000,
            chatbox_notification: false,
            chatbox_indicator: false,
            msg_header: "🎵".to_string(),
            msg_separator: "-".to_string(),
            msg_include_title: true,
            msg_include_artist: true,
            msg_include_album: false,
            msg_include_label: false,
            msg_include_comment: false,
            strip_title_brackets: true,
            show_label: false,
        }
    }
}

impl AppSettings {
    pub fn to_message_format(&self) -> MessageFormatOptions {
        MessageFormatOptions {
            header: self.msg_header.clone(),
            separator: self.msg_separator.clone(),
            include_title: self.msg_include_title,
            include_artist: self.msg_include_artist,
            include_album: self.msg_include_album,
            include_label: self.msg_include_label,
            include_comment: self.msg_include_comment,
            strip_title_brackets: self.strip_title_brackets,
        }
    }

    /// 無い・読めない・壊れている場合は既定値を返す。
    pub fn load_from(path: &Path) -> AppSettings {
        let Ok(bytes) = std::fs::read(path) else {
            return AppSettings::default();
        };
        let Ok(text) = String::from_utf8(bytes) else {
            return AppSettings::default();
        };
        // UTF-8 BOM（C# 版が付けて書く）を除く
        let json = text.strip_prefix("\u{feff}").unwrap_or(&text);
        match serde_json::from_str::<AppSettings>(json) {
            Ok(mut s) => {
                // 旧 ShowLabel=true は、新キーが JSON に無い場合のみ移行（C# と同じ文字列判定）
                if s.show_label && !json.contains("MsgIncludeLabel") {
                    s.msg_include_label = true;
                }
                s
            }
            Err(_) => AppSettings::default(),
        }
    }

    /// フォルダが無ければ作り、`<path>.tmp` に書いてから置き換える。
    pub fn save_to(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            if !dir.as_os_str().is_empty() {
                std::fs::create_dir_all(dir)?;
            }
        }
        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        let mut tmp_name: OsString = path.as_os_str().to_owned();
        tmp_name.push(".tmp");
        let tmp = PathBuf::from(tmp_name);
        if let Err(e) = std::fs::write(&tmp, json.as_bytes()) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        if let Err(e) = std::fs::rename(&tmp, path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        Ok(())
    }

    pub fn load() -> AppSettings {
        Self::load_from(&settings_path())
    }

    pub fn save(&self) -> io::Result<()> {
        self.save_to(&settings_path())
    }
}

/// `%APPDATA%\NowPlayingVrcOsc`。APPDATA が無ければカレントを基準にする。
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("NowPlayingVrcOsc")
}

pub fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &tempfile::TempDir, json: &str) -> PathBuf {
        let p = dir.path().join("settings.json");
        std::fs::write(&p, json).unwrap();
        p
    }

    #[test]
    fn 既定値() {
        let s = AppSettings::default();
        assert_eq!(s.db_path_override, "");
        assert_eq!(s.poll_interval_seconds, 1.0);
        assert_eq!(s.vrc_host, "127.0.0.1");
        assert_eq!(s.vrc_port, 9000);
        assert!(!s.chatbox_notification && !s.chatbox_indicator);
        assert_eq!(s.msg_header, "🎵");
        assert_eq!(s.msg_separator, "-");
        assert!(s.msg_include_title && s.msg_include_artist);
        assert!(!s.msg_include_album && !s.msg_include_label && !s.msg_include_comment);
        assert!(s.strip_title_brackets);
        assert!(!s.show_label);
        assert_eq!(s.to_message_format(), MessageFormatOptions::default());
    }

    #[test]
    fn v2_形式の_json_を読める() {
        let dir = tempfile::tempdir().unwrap();
        let p = write(
            &dir,
            r#"{
  "DbPathOverride": "C:/x/master.db",
  "PollIntervalSeconds": 2.5,
  "VrcHost": "localhost",
  "VrcPort": 9100,
  "ChatboxNotification": true,
  "ChatboxIndicator": true,
  "MsgHeader": "NP",
  "MsgSeparator": "/",
  "MsgIncludeTitle": true,
  "MsgIncludeArtist": false,
  "MsgIncludeAlbum": true,
  "MsgIncludeLabel": false,
  "MsgIncludeComment": true,
  "StripTitleBrackets": false,
  "Unknown": 1
}"#,
        );
        let s = AppSettings::load_from(&p);
        assert_eq!(s.db_path_override, "C:/x/master.db");
        assert_eq!(s.poll_interval_seconds, 2.5);
        assert_eq!(s.vrc_host, "localhost");
        assert_eq!(s.vrc_port, 9100);
        assert!(s.chatbox_notification && s.chatbox_indicator);
        assert_eq!(s.msg_header, "NP");
        assert_eq!(s.msg_separator, "/");
        assert!(!s.msg_include_artist && s.msg_include_album && s.msg_include_comment);
        assert!(!s.strip_title_brackets);
    }

    #[test]
    fn 欠けたキーは既定値_bom_付きも読める() {
        let dir = tempfile::tempdir().unwrap();
        let p = write(&dir, "\u{feff}{\"VrcPort\": 9001}");
        let s = AppSettings::load_from(&p);
        assert_eq!(s.vrc_port, 9001);
        assert_eq!(s.vrc_host, "127.0.0.1");
    }

    #[test]
    fn 無い_壊れている場合は既定値() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            AppSettings::load_from(&dir.path().join("none.json")),
            AppSettings::default()
        );
        let p = write(&dir, "{ これは壊れている");
        assert_eq!(AppSettings::load_from(&p), AppSettings::default());
        let p = write(&dir, r#"{"VrcPort": "abc"}"#);
        assert_eq!(AppSettings::load_from(&p), AppSettings::default());
    }

    #[test]
    fn show_label_は新キーが無いときだけ移行() {
        let dir = tempfile::tempdir().unwrap();
        let p = write(&dir, r#"{"ShowLabel": true}"#);
        assert!(AppSettings::load_from(&p).msg_include_label);

        let p = write(&dir, r#"{"ShowLabel": true, "MsgIncludeLabel": false}"#);
        assert!(!AppSettings::load_from(&p).msg_include_label);

        let p = write(&dir, r#"{"ShowLabel": false}"#);
        assert!(!AppSettings::load_from(&p).msg_include_label);
    }

    #[test]
    fn 保存して読み直せる() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub").join("settings.json");
        let s = AppSettings {
            vrc_host: "example.local".into(),
            vrc_port: 9100,
            poll_interval_seconds: 0.5,
            msg_header: "♪".into(),
            msg_include_label: true,
            ..Default::default()
        };
        s.save_to(&p).unwrap();
        assert_eq!(AppSettings::load_from(&p), s);
        // 上書き保存もできる
        let s2 = AppSettings::default();
        s2.save_to(&p).unwrap();
        assert_eq!(AppSettings::load_from(&p), s2);
    }

    #[test]
    fn show_label_が_false_なら書き出さない() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        AppSettings::default().save_to(&p).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(!text.contains("ShowLabel"));
        assert!(text.contains("\"VrcPort\": 9000"));
        assert!(text.contains("\n"));

        let s = AppSettings {
            show_label: true,
            ..Default::default()
        };
        s.save_to(&p).unwrap();
        assert!(std::fs::read_to_string(&p)
            .unwrap()
            .contains("\"ShowLabel\": true"));
    }

    #[test]
    fn tmp_が残らない() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        AppSettings::default().save_to(&p).unwrap();
        assert!(!dir.path().join("settings.json.tmp").exists());
        let n = std::fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(n, 1);
    }

    #[test]
    fn 設定パスは_settings_json() {
        assert!(settings_path().ends_with("NowPlayingVrcOsc/settings.json"));
    }
}
