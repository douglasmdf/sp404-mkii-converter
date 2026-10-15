//! User settings, persisted as JSON in the OS config folder.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SaveMode {
    Ask,
    Fixed,
    NextToSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub sanitize_filenames: bool,
    pub sanitize_folder_names: bool,
    pub max_filename_length: Option<usize>,
    pub flatten_folders: bool,
    pub downmix_multichannel: bool,
    pub save_mode: SaveMode,
    pub fixed_directory: Option<PathBuf>,
    pub workers: usize,
    pub write_json_log: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            sanitize_filenames: true,
            sanitize_folder_names: false,
            max_filename_length: None,
            flatten_folders: false,
            downmix_multichannel: true,
            save_mode: SaveMode::Ask,
            fixed_directory: None,
            workers: default_workers(),
            write_json_log: false,
        }
    }
}

pub fn default_workers() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8)
}

impl Settings {
    /// Reads settings; a missing or unreadable file yields defaults.
    pub fn load_from(path: &Path) -> Settings {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Settings::default(),
        }
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).expect("settings always serialize");
        std::fs::write(path, json)
    }

    pub fn load() -> Settings {
        Settings::load_from(&settings_path())
    }

    pub fn save(&self) -> std::io::Result<()> {
        self.save_to(&settings_path())
    }
}

/// Folder for settings and the undo manifest. `SP404_DATA_DIR` overrides it (used by tests).
pub fn app_data_dir() -> PathBuf {
    data_dir_from(std::env::var_os("SP404_DATA_DIR"))
}

pub fn data_dir_from(env_override: Option<OsString>) -> PathBuf {
    match env_override {
        Some(dir) => PathBuf::from(dir),
        None => dirs::config_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("sp404-converter"),
    }
}

pub fn settings_path() -> PathBuf {
    app_data_dir().join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let s = Settings::default();
        assert!(s.sanitize_filenames);
        assert!(!s.sanitize_folder_names);
        assert_eq!(s.max_filename_length, None);
        assert!(!s.flatten_folders);
        assert!(s.downmix_multichannel);
        assert_eq!(s.save_mode, SaveMode::Ask);
        assert_eq!(s.fixed_directory, None);
        assert!(s.workers >= 1 && s.workers <= 8);
        assert!(!s.write_json_log);
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");
        let mut s = Settings::default();
        s.flatten_folders = true;
        s.max_filename_length = Some(16);
        s.save_mode = SaveMode::NextToSource;
        s.save_to(&path).unwrap();
        assert_eq!(Settings::load_from(&path), s);
    }

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Settings::load_from(&dir.path().join("nope.json")), Settings::default());
    }

    #[test]
    fn corrupt_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(Settings::load_from(&path), Settings::default());
    }

    #[test]
    fn partial_file_fills_missing_fields_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"flattenFolders": true}"#).unwrap();
        let s = Settings::load_from(&path);
        assert!(s.flatten_folders);
        assert!(s.sanitize_filenames);
    }

    #[test]
    fn json_uses_camel_case() {
        let json = serde_json::to_string(&Settings::default()).unwrap();
        assert!(json.contains("\"sanitizeFilenames\":true"));
        assert!(json.contains("\"saveMode\":\"ask\""));
        let next = serde_json::to_string(&SaveMode::NextToSource).unwrap();
        assert_eq!(next, "\"nextToSource\"");
    }

    #[test]
    fn data_dir_env_override() {
        assert_eq!(data_dir_from(Some("/tmp/x".into())), PathBuf::from("/tmp/x"));
        assert!(data_dir_from(None).ends_with("sp404-converter"));
    }
}
