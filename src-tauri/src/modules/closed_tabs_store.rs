use crate::state::ClosedTab;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Deserialize)]
pub struct ClosedTabsStore {
    pub tabs: VecDeque<ClosedTab>,
}

impl ClosedTabsStore {
    fn get_path(app: &AppHandle) -> PathBuf {
        app.path().app_data_dir()
            .expect("Failed to get app data dir")
            .join("closed_tabs.json")
    }

    pub fn load(app: &AppHandle) -> Self {
        let path = Self::get_path(app);

        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(json) => {
                    match serde_json::from_str(&json) {
                        Ok(store) => return store,
                        Err(e) => eprintln!("Failed to parse closed_tabs.json: {}", e),
                    }
                }
                Err(e) => eprintln!("Failed to read closed_tabs.json: {}", e),
            }
        }

        // Default
        ClosedTabsStore {
            tabs: VecDeque::new(),
        }
    }

    pub fn save(&self, app: &AppHandle) -> Result<(), String> {
        let path = Self::get_path(app);
        let tmp_path = path.with_extension("tmp");
        let parent = path.parent().unwrap();

        fs::create_dir_all(parent).map_err(|e| e.to_string())?;

        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;

        // Atomic write: tmp + rename (pattern from settings.rs:84-96)
        fs::write(&tmp_path, json).map_err(|e| e.to_string())?;
        fs::rename(tmp_path, path).map_err(|e| e.to_string())?;

        Ok(())
    }
}
