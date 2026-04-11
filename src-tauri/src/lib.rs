mod ai_keys;
mod commands;
mod db;
mod network_rules;
mod sidecar;
mod state;

use tauri::Manager;
pub use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                // Init SQLite pool
                let pool = db::init_pool().await.expect("Failed to init SQLite");

                // Load provider from DB (falls back to default if not set)
                let provider = db::settings_db::get(&pool, "active_provider")
                    .await
                    .unwrap_or_else(|| "openrouter".to_string());
                let model = db::settings_db::get(&pool, "active_model")
                    .await
                    .unwrap_or_else(|| "anthropic/claude-3-5-haiku".to_string());

                let state = AppState::new(pool);

                // Override in-memory default with persisted values
                if let Ok(mut active) = state.active_provider.lock() {
                    active.provider = provider;
                    active.model = model;
                }

                // Pre-warm API key cache
                let loaded = ai_keys::load_all();
                if let Ok(mut cache) = state.key_cache.lock() {
                    *cache = loaded;
                }

                handle.manage(state);
            });
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            // AI
            commands::ai::generate_content,
            commands::ai::generate_variants,
            commands::ai::save_draft,
            commands::ai::get_post_history,
            commands::ai::scrape_url_for_brief,
            commands::ai::warmup_sidecar,
            commands::ai::generate_carousel,
            // Media
            commands::media::render_post_image,
            commands::media::render_code_image,
            commands::media::render_terminal_image,
            commands::media::render_carousel_slides,
            commands::media::export_carousel_zip,
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
