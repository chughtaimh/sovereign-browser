use tauri::{AppHandle, Manager, WebviewUrl, WebviewBuilder, PhysicalPosition, PhysicalSize, Window, WebviewWindow, Emitter};
use tauri::menu::{MenuBuilder, SubmenuBuilder, PredefinedMenuItem, MenuItemBuilder};
// Dialog plugin kept for potential future use but not currently used
use url::Url;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};

#[cfg(desktop)]
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
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

// Show settings window - creates a new window for settings
fn show_settings_window(app: &AppHandle) {
    // Check if window already exists
    if app.get_window("settings").is_some() {
        // Focus existing window
        if let Some(win) = app.get_window("settings") {
            let _ = win.set_focus();
        }
        return;
    }
    
    // Create a new settings window
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
    
    match settings_window {
        Ok(_) => println!("Settings window created"),
        Err(e) => println!("Failed to create settings window: {:?}", e),
    }
}

// Show suggestion window - creates a new window for input
fn show_suggestion_window(app: &AppHandle) {
    // Check if window already exists
    if app.get_window("suggestion").is_some() {
        // Focus existing window
        if let Some(win) = app.get_window("suggestion") {
            let _ = win.set_focus();
        }
        return;
    }
    
    // Create a new suggestion window
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
    
    match suggestion_window {
        Ok(_) => println!("Suggestion window created"),
        Err(e) => println!("Failed to create suggestion window: {:?}", e),
    }
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
    let url_string = if url.contains("://") {
        url
    } else {
        format!("https://{}", url)
    };

    if let Ok(valid_url) = Url::parse(&url_string) {
        if let Some(webview) = app.get_webview("content") {
            let js_script = format!("window.location.href = '{}'", valid_url);
            let _ = webview.eval(&js_script);
        }
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

fn main() {
    // Store reference to main webview window globally
    let toolbar_webview: Arc<Mutex<Option<WebviewWindow>>> = Arc::new(Mutex::new(None));

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let main_window: Window = app.get_window("main").unwrap();
            let handle = app.handle().clone();
            
            // CRITICAL: Get reference to toolbar webview BEFORE add_child
            if let Some(main_wv) = app.get_webview_window("main") {
                println!("Got toolbar webview reference before add_child");
                *toolbar_webview.lock().unwrap() = Some(main_wv);
            }

            // --- Build Native Menu ---
            let sovereign_menu = SubmenuBuilder::new(app, "Sovereign")
                .item(&PredefinedMenuItem::about(app, Some("About Sovereign Browser"), None)?)
                .separator()
                .item(&MenuItemBuilder::with_id("settings", "Settings").accelerator("CmdOrCtrl+,").build(app)?)
                .separator()
                .item(&PredefinedMenuItem::quit(app, Some("Quit Sovereign Browser"))?)
                .build()?;

            // File menu with Print
            let file_menu = SubmenuBuilder::new(app, "File")
                .item(&MenuItemBuilder::with_id("print", "Print...").accelerator("CmdOrCtrl+P").build(app)?)
                .build()?;

            // Edit menu with standard macOS shortcuts
            let edit_menu = SubmenuBuilder::new(app, "Edit")
                .item(&PredefinedMenuItem::undo(app, Some("Undo"))?)
                .item(&PredefinedMenuItem::redo(app, Some("Redo"))?)
                .separator()
                .item(&PredefinedMenuItem::cut(app, Some("Cut"))?)
                .item(&PredefinedMenuItem::copy(app, Some("Copy"))?)
                .item(&PredefinedMenuItem::paste(app, Some("Paste"))?)
                .item(&PredefinedMenuItem::select_all(app, Some("Select All"))?)
                .build()?;

            let feedback_menu = SubmenuBuilder::new(app, "Feedback")
                .item(&MenuItemBuilder::with_id("leave_suggestion", "Leave a Suggestion...").build(app)?)
                .build()?;

            let menu = MenuBuilder::new(app)
                .items(&[&sovereign_menu, &file_menu, &edit_menu, &feedback_menu])
                .build()?;

            app.set_menu(menu)?;

            // Handle menu events
            let handle_for_menu = handle.clone();
            app.on_menu_event(move |_app_handle, event| {
                println!("Menu event received: {:?}", event.id().0.as_str());
                match event.id().0.as_str() {
                    "settings" => {
                        show_settings_window(&handle_for_menu);
                    }
                    "leave_suggestion" => {
                        show_suggestion_window(&handle_for_menu);
                    }
                    "print" => {
                        if let Some(webview) = handle_for_menu.get_webview("content") {
                            let _ = webview.eval("window.print()");
                        }
                    }
                    _ => {}
                }
            });

            // --- Register Global Shortcuts ---
            #[cfg(desktop)]
            {
                // URL bar focus shortcuts (only fire when window is focused)
                let cmd_l = Shortcut::new(Some(Modifiers::META), Code::KeyL);
                let ctrl_l = Shortcut::new(Some(Modifiers::CONTROL), Code::KeyL);
                let cmd_k = Shortcut::new(Some(Modifiers::META), Code::KeyK);
                let ctrl_k = Shortcut::new(Some(Modifiers::CONTROL), Code::KeyK);
                
                let cmd_shift_r = Shortcut::new(Some(Modifiers::META | Modifiers::SHIFT), Code::KeyR);
                let ctrl_shift_r = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyR);
                let cmd_shift_l = Shortcut::new(Some(Modifiers::META | Modifiers::SHIFT), Code::KeyL);
                let ctrl_shift_l = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyL);
                let cmd_left_bracket = Shortcut::new(Some(Modifiers::META), Code::BracketLeft);
                let cmd_right_bracket = Shortcut::new(Some(Modifiers::META), Code::BracketRight);
                let cmd_shift_alt_r = Shortcut::new(Some(Modifiers::META | Modifiers::SHIFT | Modifiers::ALT), Code::KeyR);
                let cmd_opt_c = Shortcut::new(Some(Modifiers::META | Modifiers::ALT), Code::KeyC);

                let handle_for_shortcuts = handle.clone();
                let main_window_for_sc = main_window.clone();
                
                app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::new()
                        .with_handler(move |_app_handle, shortcut, event| {
                            if event.state() != ShortcutState::Pressed {
                                return;
                            }
                            
                            // Focus URL bar - only when our window is focused
                            if shortcut == &cmd_l || shortcut == &ctrl_l || shortcut == &cmd_k || shortcut == &ctrl_k {
                                // Check if our window is focused before stealing the shortcut
                                if !main_window_for_sc.is_focused().unwrap_or(false) {
                                    return; // Don't capture if another app is focused
                                }
                                
                                // Emit event to toolbar webview to focus URL bar
                                let _ = main_window_for_sc.emit("focus-url-bar", ());
                                return;
                            }
                            
                            // Hard reload
                            if shortcut == &cmd_shift_r || shortcut == &ctrl_shift_r {
                                if let Some(webview) = handle_for_shortcuts.get_webview("content") {
                                    if let Ok(url) = webview.url() {
                                        let js = format!("window.location.href = '{}'", url);
                                        let _ = webview.eval(&js);
                                    }
                                }
                                return;
                            }
                            
                            // Copy clean link
                            if shortcut == &cmd_shift_l || shortcut == &ctrl_shift_l {
                                if let Some(webview) = handle_for_shortcuts.get_webview("content") {
                                    if let Ok(url) = webview.url() {
                                        let _ = handle_for_shortcuts.clipboard().write_text(url.to_string());
                                    }
                                }
                                return;
                            }
                            
                            // Copy URL (Cmd+Option+C)
                            if shortcut == &cmd_opt_c {
                                if let Some(webview) = handle_for_shortcuts.get_webview("content") {
                                    if let Ok(url) = webview.url() {
                                        let _ = handle_for_shortcuts.clipboard().write_text(url.to_string());
                                    }
                                }
                                return;
                            }
                            
                            // Back
                            if shortcut == &cmd_left_bracket {
                                if let Some(webview) = handle_for_shortcuts.get_webview("content") {
                                    let _ = webview.eval("window.history.back()");
                                }
                                return;
                            }
                            
                            // Forward
                            if shortcut == &cmd_right_bracket {
                                if let Some(webview) = handle_for_shortcuts.get_webview("content") {
                                    let _ = webview.eval("window.history.forward()");
                                }
                                return;
                            }
                            
                            // Clear site data
                            if shortcut == &cmd_shift_alt_r {
                                if let Some(webview) = handle_for_shortcuts.get_webview("content") {
                                    let js = r#"
                                        localStorage.clear();
                                        sessionStorage.clear();
                                        document.cookie.split(";").forEach(c => {
                                            document.cookie = c.replace(/^ +/, "").replace(/=.*/, "=;expires=" + new Date().toUTCString() + ";path=/");
                                        });
                                        alert('🧹 Site data cleared!');
                                        window.location.reload();
                                    "#;
                                    let _ = webview.eval(js);
                                }
                                return;
                            }
                        })
                        .build(),
                )?;
                
                // Register all shortcuts
                let gs = app.global_shortcut();
                let _ = gs.register(cmd_l);
                let _ = gs.register(ctrl_l);
                let _ = gs.register(cmd_k);
                let _ = gs.register(ctrl_k);
                let _ = gs.register(cmd_shift_r);
                let _ = gs.register(ctrl_shift_r);
                let _ = gs.register(cmd_shift_l);
                let _ = gs.register(ctrl_shift_l);
                let _ = gs.register(cmd_left_bracket);
                let _ = gs.register(cmd_right_bracket);
                let _ = gs.register(cmd_shift_alt_r);
                let _ = gs.register(cmd_opt_c);
            }

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
            copy_current_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}