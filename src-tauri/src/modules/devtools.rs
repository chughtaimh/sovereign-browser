use std::net::SocketAddr;
use std::sync::Arc;
use tauri::async_runtime::spawn;
use futures_util::{StreamExt, SinkExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct DevToolsManager {
    port: u16,
    target_js: String, // We load this into memory on init
}

impl DevToolsManager {
    pub fn new(port: u16) -> Self {
        // Load target.js from the bundled assets at compile time using include_str!
        // This fails if the file doesn't exist, which ensures we don't ship broken builds.
        // We'll trust that the previous step downloaded it.
        // Note: For now, we will read it dynamically or use include_str!. 
        // Using include_str! requires the file to be present at compile time.
        // Since we downloaded it to `src/modules/assets/target.js`, the path is relative to *this file*?
        // Actually, relative to the crate root usually for include_str if using absolute? 
        // Let's use `include_str!("./assets/target.js")` assuming this file is in `src/modules/devtools.rs` 
        // and assets is `src/modules/assets/`.
        let js_content = include_str!("assets/target.js");
        
        Self { 
            port,
            target_js: js_content.to_string() 
        }
    }

    pub fn start(self: Arc<Self>) {
        let port = self.port;
        let manager = self.clone();

        spawn(async move {
            let addr = format!("127.0.0.1:{}", port);
            let listener = match TcpListener::bind(&addr).await {
                Ok(l) => l,
                Err(e) => {
                    println!("[DevTools] Failed to bind to port {}: {}", port, e);
                    return;
                }
            };

            println!("[DevTools] Bridge listening on http://{}", addr);
            
            while let Ok((stream, _)) = listener.accept().await {
                let manager_clone = manager.clone();
                spawn(async move {
                    if let Err(e) = manager_clone.handle_connection(stream).await {
                       // println!("[DevTools] Connection error: {}", e);
                    }
                });
            }
        });
    }

    async fn handle_connection(&self, mut stream: TcpStream) -> std::io::Result<()> {
        let mut buffer = [0; 1024]; // Peek buffer
        
        // We need to peek without consuming to check for "GET /target.js"
        // But TcpStream doesn't have a peek that is easy to use with `accept_async` afterwards easily 
        // unless we read into a buffer and then re-construct.
        // Simpler approach: Read the first line. 
        // If it starts with "GET /target.js", we serve HTTP.
        // If it starts with "GET / " and has "Upgrade: websocket", we assume WS? 
        // Actually, `tokio-tungstenite` expects a raw stream. If we read bytes, we can't easily pass it back.
        // 
        // Correct approach for mixed proto:
        // Use a "Peekable" approach or just read the headers myself.
        // Since this is a local dev tool, we can be a bit hacky.
        // Let's try to just read the first few bytes.
        
        // A better way often used is to assume HTTP, parse headers. If Upgrade header is present, upgrade.
        // But `tokio-tungstenite` takes a stream.
        
        // Let's implement a minimal HTTP request parser.
        // If it's a target.js request, valid HTTP response.
        // If it's Upgrade, we need to hand it to tungstenite. 
        // *BUT* tungstenite `accept_async` performs the handshake. It expects to read the handshake request.
        // If we read it, tungstenite will hang waiting for it.
        
        // Solution: We can peek (if supported) or just stick to WS on this port and serve target.js 
        // via a custom Tauri URI `sovereign://target.js`? 
        // The user specifically asked for `http://127.0.0.1:{}/target.js` in the bootstrapper.
        // So we MUST implement HTTP.
        
        // Since `target.js` is the ONLY file we serve, and everything else is WS:
        // Let's just implement a minimal loop that reads the request.
        // NOTE: This complex mix is why frameworks like Axum/Actix are used. 
        // For a single file with no extra deps, we can maybe cheat:
        // Check if the user really insists on this architecture. Yes they did.
        
        // Let's try to read the buffer.
        let n = stream.peek(&mut buffer).await?;
        let request_str = String::from_utf8_lossy(&buffer[..n]);
        
        if request_str.starts_with("GET /target.js") {
            // Serve File
            // Consume the request (drain buffer) to be polite? 
            // Actually just write response.
             // We should read until \r\n\r\n to clear the request from the socket buffer?
             // Not strictly necessary if we just write and close, but good practice.
             let mut devnull = [0; 1024];
             let _ = stream.read(&mut devnull).await?; // Consume some bytes
             
             let response = format!(
                 "HTTP/1.1 200 OK\r\nContent-Type: application/javascript\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}",
                 self.target_js.len(),
                 self.target_js
             );
             stream.write_all(response.as_bytes()).await?;
             stream.flush().await?;
             return Ok(());
        } 
        
        // Otherwise, try WebSocket Upgrade
        // We pass the stream to tungstenite. 
        // IMPORTANT: If we peeked, the data is still there. 
        // So `accept_async` should see the headers.
        
        // Add a small delay/yield to ensure peek is done? No need.
        match tokio_tungstenite::accept_async(stream).await {
            Ok(ws_stream) => {
                 // println!("[DevTools] WebSocket connected!");
                 // Handle WS
                 let (mut write, mut read) = ws_stream.split();
                 
                 // Echo loop for now (Placeholder for the real implementation)
                 while let Some(msg) = read.next().await {
                     if let Ok(m) = msg {
                         if m.is_text() || m.is_binary() {
                             let _ = write.send(m).await;
                         }
                     }
                 }
            },
            Err(_e) => {
                // Not a websocket, and not target.js
            }
        }
        
        Ok(())
    }

    /// Returns a tiny, non-blocking script to prepare the tab for debugging.
    pub fn get_bootstrapper(&self) -> String {
        format!(
            r#"
            (function() {{
                if (window.__SOVEREIGN_DEVTOOLS_READY__) return;
                window.__SOVEREIGN_DEVTOOLS_READY__ = true;
                
                window.__SOVEREIGN_LOAD_DEVTOOLS__ = function() {{
                    if (document.getElementById('sovereign-devtools-script')) return;
                    console.log('🔌 Sovereign: Connecting to DevTools Bridge...');
                    var script = document.createElement('script');
                    script.id = 'sovereign-devtools-script';
                    script.src = 'http://127.0.0.1:{}/target.js'; 
                    document.head.appendChild(script);
                }};
            }})();
            "#,
            self.port
        )
    }
}

// Unit Tests
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_manager_serve_js() {
        // NOTE: We rely on the file existing. If this test runs in CI without assets, it fails.
        // We assume valid environment.
        let manager = Arc::new(DevToolsManager {
            port: 9876,
            target_js: "console.log('test');".to_string(), // Mock content
        });
        
        manager.clone().start();
        tokio::time::sleep(Duration::from_millis(100)).await;

        let resp = reqwest::get("http://127.0.0.1:9876/target.js").await.unwrap();
        assert_eq!(resp.status(), 200);
        let text = resp.text().await.unwrap();
        assert_eq!(text, "console.log('test');");
    }
}
