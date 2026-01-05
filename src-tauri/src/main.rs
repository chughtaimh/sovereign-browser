use tauri::{AppHandle, Manager, Listener, WebviewUrl, WebviewBuilder, LogicalPosition, LogicalSize};
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
            let _ = webview.load_url(valid_url.as_str());
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
            let main_window = app.get_window("main").unwrap();
            let handle = app.handle().clone();

            let top_bar_height = 50.0;

            let content_webview = WebviewBuilder::new(
                "content", 
                WebviewUrl::External(Url::parse("https://duckduckgo.com").unwrap())
            )
            .auto_resize()
            .build(&main_window)?;

            let size = main_window.inner_size()?;
            // Initial sizing
            content_webview.set_bounds(tauri::Rect {
                position: LogicalPosition::new(0.0, top_bar_height).into(),
                size: LogicalSize::new(size.width as f64, size.height as f64 - top_bar_height).into(),
            })?;

            // Resize handling
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