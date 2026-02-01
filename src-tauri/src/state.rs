// Shared state structs to avoid circular dependencies.
// These are used by main.rs and can be tested independently.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Instant, SystemTime};
use serde::{Deserialize, Serialize};

use crate::history::HistoryStore;
use crate::settings::Settings;
use crate::adblock_manager::AdBlockManager;
use crate::modules::devtools::DevToolsManager;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Tab {
    pub id: String,
    pub webview_label: String,
    pub title: String,
    pub url: String,
    pub favicon: Option<String>,
    #[serde(skip)]
    pub last_accessed: Option<Instant>,
    pub is_loading: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub last_focus_was_content: bool,
    pub screenshot: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosedTab {
    pub id: String,           // Original tab ID (for reference)
    pub title: String,        // Page title
    pub url: String,          // Current URL when closed
    pub favicon: Option<String>,  // Favicon data URL
    pub closed_at: SystemTime,    // When tab was closed (for sorting/expiry)
}

impl From<&Tab> for ClosedTab {
    fn from(tab: &Tab) -> Self {
        ClosedTab {
            id: tab.id.clone(),
            title: tab.title.clone(),
            url: tab.url.clone(),
            favicon: tab.favicon.clone(),
            closed_at: SystemTime::now(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DropdownPayload {
    pub query: String,
    pub results: Vec<serde_json::Value>,
    pub selected_index: i32,
}

pub struct AppState {
    pub history: Arc<HistoryStore>,
    pub settings: Arc<RwLock<Settings>>,
    pub dropdown_ready: Arc<Mutex<bool>>,
    pub pending_payload: Arc<Mutex<Option<DropdownPayload>>>,
    pub tabs: Arc<Mutex<Vec<Tab>>>,
    pub active_tab_id: Arc<Mutex<Option<String>>>,
    pub last_tab_update_emit: Arc<Mutex<Instant>>,
    pub pending_launch_url: Arc<Mutex<Option<String>>>,
    pub adblock: Arc<AdBlockManager>,
    pub devtools: Arc<DevToolsManager>,
    pub closed_tabs: Arc<Mutex<VecDeque<ClosedTab>>>,  // LIFO queue, max 25 tabs
}
