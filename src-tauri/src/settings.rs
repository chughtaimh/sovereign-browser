use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SearchEngine {
    DuckDuckGo,
    Google,
    Bing,
    Brave,
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::DuckDuckGo
    }
}

impl SearchEngine {
    pub fn query_url(&self, query: &str) -> String {
        let q = urlencoding::encode(query);
        match self {
            Self::DuckDuckGo => format!("https://duckduckgo.com/?q={}", q),
            Self::Google => format!("https://google.com/search?q={}", q),
            Self::Bing => format!("https://bing.com/search?q={}", q),
            Self::Brave => format!("https://search.brave.com/search?q={}", q),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub homepage: String,
    pub search_engine: SearchEngine,
    pub block_trackers: bool,
    pub https_only: bool,
    pub clear_on_exit: bool,
    pub theme: String, // "dark", "light", "system"
    pub compact_mode: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            homepage: "https://duckduckgo.com".to_string(),
            search_engine: SearchEngine::default(),
            block_trackers: true,
            https_only: true,
            clear_on_exit: false,
            theme: "dark".to_string(),
            compact_mode: false,
        }
    }
}

impl Settings {
    pub fn get_path(app: &AppHandle) -> PathBuf {
        app.path()
            .app_data_dir()
            .expect("failed to get app data dir")
            .join("settings.json")
    }

    pub fn load(app: &AppHandle) -> Self {
        let path = Self::get_path(app);
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                    println!("[Settings] Failed to parse settings: {}, returning defaults", e);
                    Self::default()
                }),
                Err(e) => {
                    println!("[Settings] Failed to read file: {}, returning defaults", e);
                    Self::default()
                }
            }
        } else {
            Self::default()
        }
    }

    pub fn save(&self, app: &AppHandle) -> Result<(), String> {
        let path = Self::get_path(app);
        let tmp_path = path.with_extension("tmp");
        let parent = path.parent().unwrap();

        fs::create_dir_all(parent).map_err(|e| e.to_string())?;

        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        
        // Atomic Write Strategy: Write to tmp, then rename.
        // This ensures we never have a half-written file if the app crashes.
        fs::write(&tmp_path, json).map_err(|e| e.to_string())?;
        fs::rename(tmp_path, path).map_err(|e| e.to_string())?;

        Ok(())
    }
}
