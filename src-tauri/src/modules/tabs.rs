// Tab reordering module - Pure logic + Tauri command
// Follows strict modular monolith pattern

use tauri::{AppHandle, State, Emitter};
use crate::state::{Tab, AppState};
use std::collections::HashMap;

/// Pure logic for reordering tabs.
/// Returns true if the order changed, false otherwise.
///
/// Algorithm:
/// 1. Map existing tabs by ID for O(1) lookup
/// 2. Rebuild vector based on new_order
/// 3. Append any missing tabs (safety - prevents data loss on race conditions)
fn reorder_logic(tabs: &mut Vec<Tab>, new_order: &[String]) -> bool {
    // Quick check: if lengths don't match or empty, no-op
    if tabs.is_empty() || new_order.is_empty() {
        return false;
    }

    // Check if order actually changed (before modifying)
    let old_order: Vec<String> = tabs.iter().map(|t| t.id.clone()).collect();

    // Map existing tabs by ID
    let mut tab_map: HashMap<String, Tab> = tabs.drain(..).map(|t| (t.id.clone(), t)).collect();

    // Rebuild based on new_order
    let mut reordered = Vec::new();
    for id in new_order {
        if let Some(tab) = tab_map.remove(id) {
            reordered.push(tab);
        }
    }

    // Safety: Append any tabs that weren't in new_order (prevents data loss)
    reordered.extend(tab_map.into_values());

    // Compare old order with new order
    let new_order_actual: Vec<String> = reordered.iter().map(|t| t.id.clone()).collect();
    let changed = old_order != new_order_actual;

    // Update the original vector
    *tabs = reordered;

    changed
}

/// Tauri command to reorder tabs
#[tauri::command]
pub fn reorder_tabs(
    app: AppHandle,
    state: State<AppState>,
    new_order: Vec<String>
) -> Result<(), String> {
    println!("[Tab Reorder] Received new order: {:?}", new_order);

    let changed = {
        let mut tabs = state.tabs.lock().map_err(|e| e.to_string())?;
        println!("[Tab Reorder] Current order: {:?}", tabs.iter().map(|t| &t.id).collect::<Vec<_>>());
        let result = reorder_logic(&mut tabs, &new_order);
        println!("[Tab Reorder] After reorder: {:?}", tabs.iter().map(|t| &t.id).collect::<Vec<_>>());
        println!("[Tab Reorder] Changed: {}", result);
        result
    };

    if changed {
        // Emit update event to sync UI
        let tabs = state.tabs.lock().map_err(|e| e.to_string())?;
        let active_id = state.active_tab_id.lock().map_err(|e| e.to_string())?.clone();

        println!("[Tab Reorder] Emitting update-tabs event");
        let _ = app.emit("update-tabs", serde_json::json!({
            "tabs": *tabs,
            "activeTabId": active_id
        }));
    } else {
        println!("[Tab Reorder] No change detected, skipping emit");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Tab;
    use std::time::Instant;

    fn create_test_tab(id: &str, title: &str) -> Tab {
        Tab {
            id: id.to_string(),
            webview_label: format!("webview-{}", id),
            title: title.to_string(),
            url: "https://example.com".to_string(),
            favicon: None,
            last_accessed: Some(Instant::now()),
            is_loading: false,
            can_go_back: false,
            can_go_forward: false,
            last_focus_was_content: true,
            screenshot: None,
        }
    }

    #[test]
    fn test_reorder_logic() {
        let mut tabs = vec![
            create_test_tab("tab-1", "Tab 1"),
            create_test_tab("tab-2", "Tab 2"),
            create_test_tab("tab-3", "Tab 3"),
        ];

        // Test reordering
        let new_order = vec!["tab-3".to_string(), "tab-1".to_string(), "tab-2".to_string()];
        let changed = reorder_logic(&mut tabs, &new_order);

        assert!(changed);
        assert_eq!(tabs[0].id, "tab-3");
        assert_eq!(tabs[1].id, "tab-1");
        assert_eq!(tabs[2].id, "tab-2");
    }

    #[test]
    fn test_reorder_with_missing_id() {
        let mut tabs = vec![
            create_test_tab("tab-1", "Tab 1"),
            create_test_tab("tab-2", "Tab 2"),
            create_test_tab("tab-3", "Tab 3"),
        ];

        // Only provide 2 IDs (missing tab-2)
        let new_order = vec!["tab-3".to_string(), "tab-1".to_string()];
        let changed = reorder_logic(&mut tabs, &new_order);

        assert!(changed);
        // tab-3 and tab-1 should be first, tab-2 appended
        assert_eq!(tabs[0].id, "tab-3");
        assert_eq!(tabs[1].id, "tab-1");
        assert_eq!(tabs[2].id, "tab-2"); // Appended for safety
        assert_eq!(tabs.len(), 3); // No data loss
    }

    #[test]
    fn test_no_change() {
        let mut tabs = vec![
            create_test_tab("tab-1", "Tab 1"),
            create_test_tab("tab-2", "Tab 2"),
        ];

        // Same order
        let new_order = vec!["tab-1".to_string(), "tab-2".to_string()];
        let changed = reorder_logic(&mut tabs, &new_order);

        assert!(!changed);
        assert_eq!(tabs[0].id, "tab-1");
        assert_eq!(tabs[1].id, "tab-2");
    }

    #[test]
    fn test_empty_new_order() {
        let mut tabs = vec![create_test_tab("tab-1", "Tab 1")];
        let new_order = vec![];

        let changed = reorder_logic(&mut tabs, &new_order);
        assert!(!changed);
        assert_eq!(tabs.len(), 1); // No data loss
    }
}
