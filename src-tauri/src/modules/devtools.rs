use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tauri::async_runtime::spawn;
use futures_util::{StreamExt, SinkExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

// A simple broadcaster that relays messages to all other connected clients
// This effectively bridges Target <-> Frontend
struct SharedState {
    peers: Mutex<Vec<mpsc::UnboundedSender<Message>>>,
}

pub struct DevToolsManager {
    port: u16,
    target_js: String, 
    state: Arc<SharedState>,
}

impl DevToolsManager {
    pub fn new(port: u16) -> Self {
        let js_content = include_str!("assets/target.js");
        
        Self { 
            port,
            target_js: js_content.to_string(),
            state: Arc::new(SharedState {
                peers: Mutex::new(Vec::new()),
            })
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
            
            while let Ok((stream, addr)) = listener.accept().await {
                let manager_clone = manager.clone();
                spawn(async move {
                    if let Err(_e) = manager_clone.handle_connection(stream, addr).await {
                       // println!("[DevTools] Connection error: {}", _e);
                    }
                });
            }
        });
    }

    async fn handle_connection(&self, mut stream: TcpStream, _addr: SocketAddr) -> std::io::Result<()> {
        let mut buffer = [0; 1024]; 

        // Peek to distinguish HTTP vs WS
        let n = stream.peek(&mut buffer).await?;
        let request_str = String::from_utf8_lossy(&buffer[..n]);
        
        if request_str.starts_with("GET /target.js") {
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
        
        // WebSocket Upgrade
        match tokio_tungstenite::accept_async(stream).await {
            Ok(ws_stream) => {
                 let (mut write, mut read) = ws_stream.split();
                 
                 // Channel for sending messages TO this client
                 let (tx, mut rx) = mpsc::unbounded_channel();
                 
                 // Register peer
                 {
                     let mut peers = self.state.peers.lock().unwrap();
                     peers.push(tx.clone());
                 }
                 
                 // Spawn task to forward messages from RX channel -> WS Write
                 let _forward_task = spawn(async move {
                     while let Some(msg) = rx.recv().await {
                         if let Err(_) = write.send(msg).await {
                             break;
                         }
                     }
                 });

                 // Read loop: broadcast messages from THIS client -> ALL Peers
                 while let Some(msg) = read.next().await {
                     if let Ok(m) = msg {
                         if m.is_text() || m.is_binary() {
                             // Broadcast
                             // Note: In a real implementation we would route by ID.
                             // For now, we broadcast to everyone (which includes self, unless we filter).
                             // Ideally, we filter out self.
                             // But peers is locked. 
                             // Optimized: Broadcast to everyone. The client usually ignores its own echoes if IDs match,
                             // or we can implement filtering.
                             // Let's filter by checking sender validity? No easy way with unbounded channel equality.
                             // Let's just broadcast to all.
                             
                             let peers = self.state.peers.lock().unwrap();
                             for peer in peers.iter() {
                                 // Simple pointer check is hard with channels.
                                 // We just send to all using clone.
                                 // If echo is an issue, we can address it. 
                                 // Chii/Weinre usually handle echoes fine or use rooms.
                                 if !peer.same_channel(&tx) {
                                     let _ = peer.send(m.clone());
                                 }
                             }
                         }
                     } else {
                         break;
                     }
                 }
                 
                 // Cleanup
                 {
                     let mut peers = self.state.peers.lock().unwrap();
                     peers.retain(|peer| !peer.same_channel(&tx));
                 }
            },
            Err(_e) => {
                // Not a websocket
            }
        }
        
        Ok(())
    }

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
