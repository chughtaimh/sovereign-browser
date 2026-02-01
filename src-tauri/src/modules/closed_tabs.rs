use crate::state::{AppState, ClosedTab, Tab};

const MAX_CLOSED_TABS: usize = 25;

/// Archives a tab to closed tabs stack
pub fn archive_tab(state: &AppState, tab: &Tab) {
    let closed_tab = ClosedTab::from(tab);
    let mut closed = state.closed_tabs.lock().unwrap();

    closed.push_back(closed_tab);

    // Limit to 25 closed tabs (FIFO)
    if closed.len() > MAX_CLOSED_TABS {
        closed.pop_front();
    }

    println!("[ClosedTabs] Archived tab '{}' at URL: {}", tab.title, tab.url);
}

/// Retrieves last closed tab (LIFO)
pub fn pop_closed_tab(state: &AppState) -> Option<ClosedTab> {
    let mut closed = state.closed_tabs.lock().unwrap();
    let tab = closed.pop_back();

    if let Some(ref t) = tab {
        println!("[ClosedTabs] Restored tab '{}' at URL: {}", t.title, t.url);
    }

    tab
}

/// Gets count of closed tabs (for UI)
pub fn closed_tab_count(state: &AppState) -> usize {
    let closed = state.closed_tabs.lock().unwrap();
    closed.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    // TODO: Add unit tests for archive/restore cycle, max size enforcement
}
