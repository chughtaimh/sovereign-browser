// Sovereign Browser Library Entry Point
// This file exposes all modules so they can be imported by main.rs
// and tested independently.


// use tauri::Manager;

// Core modules (existing)
pub mod adblock_manager;
pub mod history;
pub mod settings;

// Shared state (new)
pub mod state;

// Pure logic modules (new - no Tauri imports)
pub mod modules;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
