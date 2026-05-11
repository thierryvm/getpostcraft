mod adapters;
mod ai_keys;
mod commands;
mod db;
mod log_redact;
mod network_rules;
mod openrouter_pricing;
mod sidecar;
mod state;
mod token_store;

pub use state::AppState;
use tauri::Manager;

/// Write a self-explanatory `STARTUP_BLOCKED.txt` next to `app.db` when
/// the DB-ahead-of-binary pre-flight check fails. The user can't read
/// the log file from a crashed app; a plain-text file in the data dir
/// that double-clicks open in Notepad is the lowest-friction notice we
/// can deliver without bringing up a WebView.
///
/// Best-effort: I/O failures are silently swallowed (the log already
/// has the same content; the file is the cherry on top, not the only
/// signal).
fn write_startup_blocked_notice(payload: &str) {
    let Some(data_dir) = dirs::data_dir() else {
        return;
    };
    let dir = data_dir.join("getpostcraft");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("STARTUP_BLOCKED.txt");
    let now = chrono::Utc::now().to_rfc3339();
    let body = format!(
        "Getpostcraft n'a pas pu démarrer\n\
         ===============================\n\n\
         Date du blocage : {now}\n\n\
         {payload}\n\n\
         Détails techniques dans le log :\n\
         %LOCALAPPDATA%\\app.getpostcraft\\logs\\app.log (Windows)\n\
         ~/Library/Logs/app.getpostcraft/app.log (macOS)\n\
         ~/.local/share/app.getpostcraft/logs/app.log (Linux)\n\n\
         Tu peux supprimer ce fichier après avoir résolu le problème — \n\
         il sera recréé seulement si l'app re-bloque.\n"
    );
    let _ = std::fs::write(&path, body);
}

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
            // Wrap the async setup in a block that returns Result so we can
            // propagate `init_pool()` failures up through `setup` instead of
            // panicking. A panic in `block_on` inside `setup` produces an
            // unhandled process abort with no user-facing error; surfacing
            // the error gives Tauri a chance to log it before the window
            // closes, and gives the user a fighting chance at diagnosing
            // (e.g. "data dir not writable" → check permissions, OneDrive,
            // etc.).
            let setup_result: Result<(), String> = tauri::async_runtime::block_on(async move {
                // Init SQLite pool
                let pool = db::init_pool()
                    .await
                    .map_err(|e| format!("Failed to init SQLite: {e}"))?;

                // Load provider from DB (falls back to default if not set)
                let provider = db::settings_db::get(&pool, "active_provider")
                    .await
                    .unwrap_or_else(|| "openrouter".to_string());
                let model = db::settings_db::get(&pool, "active_model")
                    .await
                    .unwrap_or_else(|| "anthropic/claude-sonnet-4.6".to_string());

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
                Ok(())
            });

            if let Err(e) = setup_result {
                log::error!("Tauri setup failed: {e}");
                // Special handler for the "DB ahead of binary" pre-flight
                // failure surfaced by db::check_db_is_not_ahead_of_binary.
                // The user can't read the log file from a crashed app; we
                // write a self-explanatory text file to the data dir
                // (same dir as `app.db`, NOT the log dir which lives
                // under the bundle identifier on Windows) so double-
                // clicking that folder reveals the recovery path.
                // Pure best-effort — if the write fails too we just
                // fall through to the normal log-and-exit.
                //
                // We `find()` the marker rather than `strip_prefix()`
                // so the detection survives any future change to the
                // upstream wrapping in `setup_result` (e.g. a different
                // prefix than "Failed to init SQLite: "). The marker
                // itself is the single source of truth, defined in
                // `db::DB_AHEAD_MARKER` and prepended in `init_pool`.
                if let Some(idx) = e.find(db::DB_AHEAD_MARKER) {
                    let payload = &e[idx + db::DB_AHEAD_MARKER.len()..];
                    write_startup_blocked_notice(payload);
                }
                return Err(e.into());
            }

            // Spawn the daily auto-backup as a background task. Runs once
            // shortly after startup (so the user gets a backup the first
            // time they use the app post-install) and then sleeps a day
            // between runs. Failures are logged, never propagated — a
            // backup miss must not crash the app startup path.
            //
            // Lives outside the block_on above because we want it
            // detached: nothing should ever wait on this task to complete.
            let bg_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                use tokio::time::{sleep, Duration};

                // Small initial delay so the first backup doesn't fight
                // for the SQLite WAL with whatever the user is doing on
                // the first window paint (loading accounts, etc.).
                sleep(Duration::from_secs(60)).await;

                loop {
                    let state: tauri::State<'_, AppState> = bg_handle.state();
                    match crate::commands::auto_backup::run_if_due(&state).await {
                        Ok(Some(path)) => log::info!("auto_backup: created {path}"),
                        Ok(None) => log::debug!("auto_backup: skipped (recent backup exists)"),
                        Err(e) => log::warn!("auto_backup: failed (non-fatal): {e}"),
                    }
                    // Re-check every hour. The `run_if_due` guard ensures
                    // we only actually create a fresh backup once per
                    // ~23h, so this loop is a cheap heartbeat — not a
                    // backup-every-hour storm.
                    sleep(Duration::from_secs(3600)).await;
                }
            });

            // Fire-and-forget OpenRouter pricing refresh on startup. Falls
            // back silently to the static `pricing_map` if the user is
            // offline; subsequent app opens will pick up live rates the
            // moment connectivity returns.
            let pricing_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                use tokio::time::{sleep, Duration};
                // Small delay so the UI paints before we hit the network.
                sleep(Duration::from_secs(5)).await;
                let state: tauri::State<'_, AppState> = pricing_handle.state();
                match crate::openrouter_pricing::refresh(&state.pricing_cache).await {
                    Ok(n) => log::info!("openrouter_pricing: refreshed {n} models"),
                    Err(e) => {
                        log::info!("openrouter_pricing: skipping startup refresh ({e})")
                    }
                }
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            // AI
            commands::ai::generate_content,
            commands::ai::generate_variants,
            commands::ai::generate_and_save_group,
            commands::ai::save_draft,
            commands::ai::get_post_history,
            commands::ai::get_post_by_id,
            commands::ai::get_ai_usage_summary,
            commands::ai::refresh_openrouter_pricing,
            commands::ai::get_openrouter_pricing_snapshot,
            commands::ai::scrape_url_for_brief,
            commands::ai::warmup_sidecar,
            commands::ai::generate_carousel,
            commands::ai::synthesize_product_truth_from_url,
            commands::ai::analyze_url_visual,
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
            commands::publisher::update_draft_images,
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
            commands::oauth::update_account_display_handle,
            // Logs
            commands::logs::get_app_logs,
            commands::logs::get_log_file_path,
            // Data export — backup local SQLite as a `.gpcbak` ZIP
            commands::data_export::export_backup_zip,
            // Data export — portable JSON + media + Postgres schema (.zip)
            commands::data_export::export_portable_zip,
            // Data export — restore a `.gpcbak` over the live DB
            commands::data_export::import_backup_zip,
            // Auto-backup — first-launch restore prompt + manual list
            commands::auto_backup::get_restore_offer,
            commands::auto_backup::list_auto_backups,
            // Python deps — in-app pip install for the sidecar packages
            commands::python_deps::check_python_deps,
            commands::python_deps::install_python_deps,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
