use tauri::{AppHandle, Manager, WebviewUrl, WebviewBuilder, PhysicalPosition, PhysicalSize, Window};
use url::Url;

#[tauri::command]
fn navigate(app: AppHandle, url: String) {
    let url_string = if url.contains("://") {
        url
    } else {
        format!("https://{}", url)
    };

    if let Ok(valid_url) = Url::parse(&url_string) {
        if let Some(webview) = app.get_webview("content") {
            // Use JS evaluation for navigation to avoid API instability
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

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Get the main window (Window, not WebviewWindow)
            let main_window: Window = app.get_window("main").unwrap();
            let handle = app.handle().clone();

            // Toolbar height in CSS/logical pixels - must match ui/index.html #toolbar height
            // Adding extra offset to account for macOS title bar
            let toolbar_height_logical: f64 = 56.0 + 28.0; // 56px toolbar + ~28px for macOS title bar offset
            
            let physical_size = main_window.inner_size()?;
            let scale_factor = main_window.scale_factor()?;
            
            // Convert toolbar height to physical pixels
            let toolbar_height_physical = (toolbar_height_logical * scale_factor) as u32;
            
            let content_y = toolbar_height_physical;
            let content_height = physical_size.height.saturating_sub(toolbar_height_physical).max(100);

            // Chrome-like User-Agent to ensure compatibility with Google Workspace and other services
            let chrome_user_agent = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

            // Create the content webview builder with custom User-Agent
            let webview_builder = WebviewBuilder::new(
                "content", 
                WebviewUrl::External(Url::parse("https://duckduckgo.com").unwrap())
            )
            .user_agent(chrome_user_agent);
            
            // Add the child webview to the window with PHYSICAL position and size
            let _content_webview = main_window.add_child(
                webview_builder,
                PhysicalPosition::new(0, content_y as i32),
                PhysicalSize::new(physical_size.width, content_height),
            )?;

            // Handle Window Resizing to keep the webview filled
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
        .invoke_handler(tauri::generate_handler![navigate, go_back, go_forward])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}