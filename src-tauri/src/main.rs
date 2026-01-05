use tauri::{AppHandle, Manager, WebviewUrl, WebviewBuilder, LogicalPosition, LogicalSize, Window};
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

            let top_bar_height = 50.0;
            let physical_size = main_window.inner_size()?;
            let scale_factor = main_window.scale_factor()?;
            
            // Convert physical size to logical size (important for HiDPI/Retina displays)
            let logical_size = physical_size.to_logical::<f64>(scale_factor);
            let width = logical_size.width;
            let height = logical_size.height;

            // Create the content webview builder
            let webview_builder = WebviewBuilder::new(
                "content", 
                WebviewUrl::External(Url::parse("https://duckduckgo.com").unwrap())
            );
            
            // Add the child webview to the window with position and size
            let _content_webview = main_window.add_child(
                webview_builder,
                LogicalPosition::new(0.0, top_bar_height),
                LogicalSize::new(width, height - top_bar_height),
            )?;

            // Handle Window Resizing to keep the webview filled
            let main_window_clone = main_window.clone();
            main_window.on_window_event(move |event| {
                if let tauri::WindowEvent::Resized(physical_size) = event {
                    let scale_factor = main_window_clone.scale_factor().unwrap_or(1.0);
                    let logical_size = physical_size.to_logical::<f64>(scale_factor);
                    
                    if let Some(wv) = handle.get_webview("content") {
                        let _ = wv.set_bounds(tauri::Rect {
                            position: LogicalPosition::new(0.0, top_bar_height).into(),
                            size: LogicalSize::new(logical_size.width, logical_size.height - top_bar_height).into(),
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