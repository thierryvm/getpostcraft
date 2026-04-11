mod ai_keys;
mod commands;
mod network_rules;
mod sidecar;
mod state;

pub use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState::new();

    // Pre-warm key cache from persisted storage so generation works immediately
    // on app restart without re-entering credentials.
    {
        let loaded = ai_keys::load_all();
        if let Ok(mut cache) = state.key_cache.lock() {
            *cache = loaded;
        }
    }

    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            // AI
            commands::ai::generate_content,
            // Settings — BYOK
            commands::settings::save_ai_key,
            commands::settings::test_ai_key,
            commands::settings::get_ai_key_status,
            commands::settings::delete_ai_key,
            // Settings — active provider
            commands::settings::set_active_provider,
            commands::settings::get_active_provider,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
