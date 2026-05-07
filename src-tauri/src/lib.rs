mod adapters;
mod ai_keys;
mod commands;
mod db;
mod network_rules;
mod sidecar;
mod state;
mod token_store;

pub use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Install rustls CryptoProvider once at process level — required before any
    // TLS operation (OAuth callback servers). install_default() returns Err if
    // already installed, which is safe to ignore.
    let _ = rustls::crypto::ring::default_provider().install_default();

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("app".to_string()),
                    }),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview),
                ])
                .max_file_size(5_000_000) // 5 MB par fichier
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(3))
                .build(),
        )
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
            // Calendar
            commands::calendar::get_calendar_posts,
            commands::calendar::schedule_post,
            commands::calendar::unschedule_post,
            commands::calendar::delete_post,
            commands::calendar::update_post_draft,
            // Publisher — Instagram
            commands::publisher::publish_post,
            commands::publisher::save_imgbb_key,
            commands::publisher::get_imgbb_key_status,
            commands::publisher::save_image_host,
            commands::publisher::get_image_host,
            commands::publisher::update_draft_image,
            // Publisher — LinkedIn
            commands::publisher::publish_linkedin_post,
            // OAuth / Accounts — Instagram
            commands::oauth::start_oauth_flow,
            commands::oauth::list_accounts,
            commands::oauth::disconnect_account,
            commands::oauth::save_instagram_app_id,
            commands::oauth::get_instagram_app_id,
            commands::oauth::save_instagram_client_secret,
            commands::oauth::get_instagram_client_secret_status,
            // OAuth / Accounts — LinkedIn
            commands::oauth::start_linkedin_oauth_flow,
            commands::oauth::save_linkedin_client_id,
            commands::oauth::get_linkedin_client_id,
            commands::oauth::save_linkedin_client_secret,
            commands::oauth::get_linkedin_client_secret_status,
            commands::oauth::update_account_product_truth,
            commands::oauth::update_account_branding,
            // Logs
            commands::logs::get_app_logs,
            commands::logs::get_log_file_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
