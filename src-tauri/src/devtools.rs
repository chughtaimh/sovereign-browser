use std::thread;
use tiny_http::{Server, Response, Header};
use rust_embed::RustEmbed;

// Embeds the 'src-tauri/assets/devtools' folder into the binary
#[derive(RustEmbed)]
#[folder = "assets/devtools"]
struct Asset;

pub struct DevToolsServer;

impl DevToolsServer {
    pub fn start() -> u16 {
        // 1. Pick a random free port to avoid conflicts
        let port = portpicker::pick_unused_port().unwrap_or(9222);
        
        // 2. Start the server in a separate thread
        thread::spawn(move || {
            let server = match Server::http(format!("127.0.0.1:{}", port)) {
                Ok(s) => s,
                Err(e) => {
                    println!("Failed to start DevTools server: {}", e);
                    return;
                }
            };
            
            println!("DevTools Server running at http://127.0.0.1:{}", port);
            
            for request in server.incoming_requests() {
                let url = request.url().to_string();
                
                // ROUTER
                // Simple stripping of query params if any
                let path = url.split('?').next().unwrap_or("/");
                
                if path == "/" || path == "/index.html" {
                    Self::serve_asset(request, "index.html", "text/html");
                } else if path == "/target.js" {
                    Self::serve_asset(request, "target.js", "application/javascript");
                } else if path == "/icon.png" {
                    Self::serve_asset(request, "icon.png", "image/png");
                } else {
                    // Try to serve other assets (like style.css if chii has it)
                    // Remove leading slash
                    let relative_path = if path.starts_with('/') { &path[1..] } else { path };
                    
                    if let Some(file) = Asset::get(relative_path) {
                         let mime = mime_guess::from_path(relative_path).first_or_text_plain();
                         Self::serve_asset(request, relative_path, mime.as_ref());
                    } else {
                         let _ = request.respond(Response::from_string("404").with_status_code(404));
                    }
                }
            }
        });

        port
    }

    fn serve_asset(request: tiny_http::Request, path: &str, content_type: &str) {
        if let Some(file) = Asset::get(path) {
            let header = Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes()).unwrap();
            let response = Response::from_data(file.data.as_ref())
                .with_header(header);
            let _ = request.respond(response);
        } else {
             let _ = request.respond(Response::from_string("Asset not found").with_status_code(404));
        }
    }
}
