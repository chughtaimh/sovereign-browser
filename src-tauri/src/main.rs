use tauri::{AppHandle, Manager, WebviewUrl, WebviewBuilder, PhysicalPosition, PhysicalSize, Window, Emitter};
use tauri::menu::{MenuBuilder, SubmenuBuilder, PredefinedMenuItem, MenuItemBuilder};
use url::Url;
use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

use tauri_plugin_clipboard_manager::ClipboardExt;

#[derive(Serialize, Deserialize, Clone)]
struct Suggestion {
    timestamp: String,
    text: String,
}

fn get_suggestions_path(app: &AppHandle) -> PathBuf {
    let app_data_dir = app.path().app_data_dir().expect("Failed to get app data dir");
    fs::create_dir_all(&app_data_dir).ok();
    app_data_dir.join("suggestions.json")
}

fn save_suggestion_to_file(app: &AppHandle, text: String) -> Result<(), String> {
    let path = get_suggestions_path(app);
    
    let mut suggestions: Vec<Suggestion> = if path.exists() {
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    };
    
    let new_suggestion = Suggestion {
        timestamp: chrono::Utc::now().to_rfc3339(),
        text,
    };
    suggestions.push(new_suggestion);
    
    let json = serde_json::to_string_pretty(&suggestions).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    
    Ok(())
}

// Show settings window
fn show_settings_window(app: &AppHandle) {
    if let Some(win) = app.get_window("settings") {
        let _ = win.set_focus();
        return;
    }
    
    let settings_window = tauri::WebviewWindowBuilder::new(
        app,
        "settings",
        tauri::WebviewUrl::App("settings.html".into())
    )
    .title("Settings")
    .inner_size(450.0, 520.0)
    .resizable(true)
    .minimizable(false)
    .maximizable(false)
    .center()
    .focused(true)
    .build();
    
    if let Err(e) = settings_window {
        println!("Failed to create settings window: {:?}", e);
    }
}

// Show suggestion window
fn show_suggestion_window(app: &AppHandle) {
    if let Some(win) = app.get_window("suggestion") {
        let _ = win.set_focus();
        return;
    }
    
    let suggestion_window = tauri::WebviewWindowBuilder::new(
        app,
        "suggestion",
        tauri::WebviewUrl::App("suggestion.html".into())
    )
    .title("Leave a Suggestion")
    .inner_size(400.0, 320.0)
    .resizable(false)
    .minimizable(false)
    .maximizable(false)
    .center()
    .focused(true)
    .build();
    
    if let Err(e) = suggestion_window {
        println!("Failed to create suggestion window: {:?}", e);
    }
}

// Logic for parsing input into a navigable URL
//
// PRIVACY NOTICE:
// This function performs purely local string manipulation and heuristics.
// 1. It does NOT perform any DNS resolution or network reachability checks.
// 2. It does NOT prefetch any content.
// 3. It does NOT send any data to autocomplete servers.
// 4. The only external request happens when the user explicitly commits navigation (Enter/Go),
//    at which point the Webview initiates a standard navigation.
fn smart_parse_url(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "about:blank".to_string();
    }

    // 1. Force HTTP for implicit localhost/IP (if no scheme present)
    let has_scheme_separator = trimmed.contains("://");
    let is_localhost = trimmed.starts_with("localhost") || trimmed.starts_with("127.0.0.1");
    let is_ip = trimmed.parse::<std::net::IpAddr>().is_ok();
    
    if (is_localhost || is_ip) && !has_scheme_separator {
        let candidate = format!("http://{}", trimmed);
        if let Ok(u) = Url::parse(&candidate) {
            return u.to_string();
        }
    }

    // 2. Try parsing as-is (valid scheme)
    if let Ok(u) = Url::parse(trimmed) {
        let s = u.scheme();
        // Only accept if it's a known standard web/file scheme
        // This prevents "google.com" being parsed as scheme "google"
        if s == "http" || s == "https" || s == "file" || s == "about" || s == "data" {
            return u.to_string();
        }
    }

    // 3. Heuristic: Dot implies domain? -> Try HTTPS
    // (Exclude spaces which imply search)
    if !trimmed.contains(' ') && trimmed.contains('.') && !trimmed.ends_with('.') {
        let candidate = format!("https://{}", trimmed);
        if let Ok(u) = Url::parse(&candidate) {
            if u.host().is_some() {
                return u.to_string();
            }
        }
    }

    // 4. Fallback to Search
    let q = urlencoding::encode(trimmed);
    format!("https://duckduckgo.com/?q={}", q)
}

#[tauri::command]
fn save_suggestion(app: AppHandle, text: String) -> Result<(), String> {
    save_suggestion_to_file(&app, text)
}

#[tauri::command]
fn get_suggestions(app: AppHandle) -> Result<Vec<Suggestion>, String> {
    let path = get_suggestions_path(&app);
    if path.exists() {
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let suggestions: Vec<Suggestion> = serde_json::from_str(&content).unwrap_or_default();
        Ok(suggestions)
    } else {
        Ok(Vec::new())
    }
}

#[tauri::command]
fn get_current_url(app: AppHandle) -> Option<String> {
    if let Some(webview) = app.get_webview("content") {
        webview.url().ok().map(|u| u.to_string())
    } else {
        None
    }
}

#[tauri::command]
fn hard_reload(app: AppHandle) {
    if let Some(webview) = app.get_webview("content") {
        if let Ok(url) = webview.url() {
            let js_script = format!("window.location.href = '{}'", url);
            let _ = webview.eval(&js_script);
        }
    }
}

#[tauri::command]
fn clear_site_data(app: AppHandle) -> Result<(), String> {
    if let Some(webview) = app.get_webview("content") {
        let js_script = r#"
            localStorage.clear();
            sessionStorage.clear();
            document.cookie.split(";").forEach(function(c) { 
                document.cookie = c.replace(/^ +/, "").replace(/=.*/, "=;expires=" + new Date().toUTCString() + ";path=/"); 
            });
            window.location.reload();
        "#;
        webview.eval(js_script).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn navigate(app: AppHandle, url: String) {
    let final_url = smart_parse_url(&url);

    if let Some(webview) = app.get_webview("content") {
        let js_script = format!("window.location.href = '{}'", final_url);
        let _ = webview.eval(&js_script);
    }
}

#[tauri::command]
fn go_back(app: AppHandle) {
    if let Some(webview) = app.get_webview("content") {
        let _ = webview.eval("window.history.back()");
    }
}

#[tauri::command]
fn go_forward(app: AppHandle) {
    if let Some(webview) = app.get_webview("content") {
        let _ = webview.eval("window.history.forward()");
    }
}

#[tauri::command]
fn copy_current_url(app: AppHandle) -> Result<(), String> {
    if let Some(webview) = app.get_webview("content") {
        if let Ok(url) = webview.url() {
            app.clipboard().write_text(url.to_string()).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
fn focus_toolbar(app: AppHandle) -> Result<(), String> {
    // Invariant: Main window must be focused first
    if let Some(main_win) = app.get_window("main") {
        main_win.set_focus().map_err(|e| e.to_string())?;
    }
    
    // Invariant: Explicitly focus the toolbar webview (which has label "main" in this setup)
    if let Some(webview) = app.get_webview("main") {
        webview.set_focus().map_err(|e| e.to_string())?;
    }

    // Signal frontend to focus the specific DOM element
    app.emit("focus-url-bar", ()).map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
fn focus_content(app: AppHandle) -> Result<(), String> {
    // Invariant: Main window must be focused first
    if let Some(main_win) = app.get_window("main") {
        main_win.set_focus().map_err(|e| e.to_string())?;
    }

    // Invariant: Webview must be explicitly focused
    if let Some(wv) = app.get_webview("content") {
        wv.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let main_window: Window = app.get_window("main").unwrap();
            let handle = app.handle().clone();
            
            // --- Build Native Menu ---
            let sovereign_menu = SubmenuBuilder::new(app, "Sovereign")
                .item(&PredefinedMenuItem::about(app, Some("About Sovereign Browser"), None)?)
                .separator()
                .item(&MenuItemBuilder::with_id("settings", "Settings").accelerator("CmdOrCtrl+,").build(app)?)
                .separator()
                .item(&PredefinedMenuItem::quit(app, Some("Quit Sovereign Browser"))?)
                .build()?;

            let file_menu = SubmenuBuilder::new(app, "File")
                .item(&MenuItemBuilder::with_id("print", "Print...").accelerator("CmdOrCtrl+P").build(app)?)
                .build()?;

            let edit_menu = SubmenuBuilder::new(app, "Edit")
                .item(&PredefinedMenuItem::undo(app, Some("Undo"))?)
                .item(&PredefinedMenuItem::redo(app, Some("Redo"))?)
                .separator()
                .item(&PredefinedMenuItem::cut(app, Some("Cut"))?)
                .item(&PredefinedMenuItem::copy(app, Some("Copy"))?)
                .item(&PredefinedMenuItem::paste(app, Some("Paste"))?)
                .item(&PredefinedMenuItem::select_all(app, Some("Select All"))?)
                .build()?;

            let view_menu = SubmenuBuilder::new(app, "View")
                .item(&MenuItemBuilder::with_id("focus_location", "Open Location").accelerator("CmdOrCtrl+L").build(app)?)
                .item(&MenuItemBuilder::with_id("focus_location_alt", "Open Location (Alt)").accelerator("CmdOrCtrl+K").build(app)?)
                .item(&MenuItemBuilder::with_id("reload", "Reload Page").accelerator("CmdOrCtrl+R").build(app)?)
                .item(&MenuItemBuilder::with_id("hard_reload", "Hard Reload").accelerator("CmdOrCtrl+Shift+R").build(app)?)
                .build()?;

            let history_menu = SubmenuBuilder::new(app, "History")
                .item(&MenuItemBuilder::with_id("go_back", "Back").accelerator("CmdOrCtrl+[").build(app)?)
                .item(&MenuItemBuilder::with_id("go_forward", "Forward").accelerator("CmdOrCtrl+]").build(app)?)
                .build()?;

            let feedback_menu = SubmenuBuilder::new(app, "Feedback")
                .item(&MenuItemBuilder::with_id("leave_suggestion", "Leave a Suggestion...").build(app)?)
                .build()?;

            let menu = MenuBuilder::new(app)
                .items(&[&sovereign_menu, &file_menu, &edit_menu, &view_menu, &history_menu, &feedback_menu])
                .build()?;

            app.set_menu(menu)?;

            // Handle menu events
            let handle_for_menu = handle.clone();
            
            app.on_menu_event(move |_app_handle, event| {
                let id = event.id().0.as_str();
                match id {
                    "settings" => show_settings_window(&handle_for_menu),
                    "leave_suggestion" => show_suggestion_window(&handle_for_menu),
                    
                    // Focus Actions - Emit to Main Window
                    "focus_location" | "focus_location_alt" => {
                        if let Some(main_win) = handle_for_menu.get_window("main") {
                             // Force window focus first
                             let _ = main_win.set_focus();
                             // Then emit event
                             let _ = main_win.emit("focus-url-bar", ());
                        }
                    },
                    
                    // Navigation Actions
                    "reload" => {
                        if let Some(webview) = handle_for_menu.get_webview("content") {
                            let _ = webview.eval("window.location.reload()");
                        }
                    },
                    "hard_reload" => {
                         if let Some(webview) = handle_for_menu.get_webview("content") {
                            if let Ok(url) = webview.url() {
                                let js = format!("window.location.href = '{}'", url);
                                let _ = webview.eval(&js);
                            }
                        }
                    },
                    "go_back" => {
                        if let Some(webview) = handle_for_menu.get_webview("content") {
                            let _ = webview.eval("window.history.back()");
                        }
                    },
                    "go_forward" => {
                        if let Some(webview) = handle_for_menu.get_webview("content") {
                            let _ = webview.eval("window.history.forward()");
                        }
                    },
                    
                    "print" => {
                        if let Some(webview) = handle_for_menu.get_webview("content") {
                            let _ = webview.eval("window.print()");
                        }
                    },
                    _ => {}
                }
            });

            // --- Setup Content Webview ---
            let toolbar_height_logical: f64 = 56.0 + 28.0;
            
            let physical_size = main_window.inner_size()?;
            let scale_factor = main_window.scale_factor()?;
            let toolbar_height_physical = (toolbar_height_logical * scale_factor) as u32;
            
            let content_y = toolbar_height_physical;
            let content_height = physical_size.height.saturating_sub(toolbar_height_physical).max(100);

            let chrome_user_agent = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

            let webview_builder = WebviewBuilder::new(
                "content", 
                WebviewUrl::External(Url::parse("https://duckduckgo.com").unwrap())
            )
            .user_agent(chrome_user_agent);
            
            let _content_webview = main_window.add_child(
                webview_builder,
                PhysicalPosition::new(0, content_y as i32),
                PhysicalSize::new(physical_size.width, content_height),
            )?;

            // Handle Window Resizing
            let main_window_clone = main_window.clone();
            main_window.on_window_event(move |event| {
                if let tauri::WindowEvent::Resized(new_physical_size) = event {
                    let scale = main_window_clone.scale_factor().unwrap_or(1.0);
                    let toolbar_physical = (toolbar_height_logical * scale) as u32;
                    let content_h = new_physical_size.height.saturating_sub(toolbar_physical).max(100);
                    
                    if let Some(wv) = handle.get_webview("content") {
                        let _ = wv.set_bounds(tauri::Rect {
                            position: tauri::Position::Physical(PhysicalPosition::new(0, toolbar_physical as i32)),
                            size: tauri::Size::Physical(PhysicalSize::new(new_physical_size.width, content_h)),
                        });
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            navigate, 
            go_back, 
            go_forward,
            save_suggestion,
            get_suggestions,
            get_current_url,
            hard_reload,
            clear_site_data,
            copy_current_url,
            focus_toolbar,
            focus_content
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smart_parse_url() {
        // Standard URLs - Expect normalization (trailing slash)
        assert_eq!(smart_parse_url("https://example.com"), "https://example.com/");
        assert_eq!(smart_parse_url("http://example.com"), "http://example.com/");
        assert_eq!(smart_parse_url("about:blank"), "about:blank");
        
        // Localhost / IPs
        assert_eq!(smart_parse_url("localhost"), "http://localhost/");
        assert_eq!(smart_parse_url("localhost:3000"), "http://localhost:3000/");
        assert_eq!(smart_parse_url("127.0.0.1"), "http://127.0.0.1/");
        assert_eq!(smart_parse_url("192.168.1.10"), "http://192.168.1.10/");
        
        // Domains without scheme
        assert_eq!(smart_parse_url("example.com"), "https://example.com/");
        assert_eq!(smart_parse_url("sub.domain.co.uk"), "https://sub.domain.co.uk/");
        // Params/Fragment preserved
        assert_eq!(smart_parse_url("google.com/test?x=1#frag"), "https://google.com/test?x=1#frag");
        
        // Search Queries
        let hello_encoded = urlencoding::encode("hello world");
        assert_eq!(smart_parse_url("hello world"), format!("https://duckduckgo.com/?q={}", hello_encoded));
        
        // Encoding Tests
        let cpp_encoded = urlencoding::encode("c++");
        assert_eq!(smart_parse_url("c++"), format!("https://duckduckgo.com/?q={}", cpp_encoded));
        
        let ampersand_encoded = urlencoding::encode("hello & world");
        assert_eq!(smart_parse_url("hello & world"), format!("https://duckduckgo.com/?q={}", ampersand_encoded));
        
        let unicode_encoded = urlencoding::encode("café");
        assert_eq!(smart_parse_url("café"), format!("https://duckduckgo.com/?q={}", unicode_encoded));
    }
}