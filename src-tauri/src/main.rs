//! ClawX Desktop Application - Tauri Main Entry Point
//!
//! This is the main entry point for the ClawX desktop application built with Tauri.
//! It initializes the application, registers IPC command handlers, and manages
//! the application lifecycle.

mod commands;
mod core;
mod services;

use std::sync::Arc;
use tauri::Manager;
use crate::core::AppState;

fn main() {
    run();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = app.get_webview_window("main").and_then(|win| {
                win.show().ok()?;
                win.set_focus().ok()
            });
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--flag1", "--flag2"]),
        ))
        .invoke_handler(tauri::generate_handler![
            // Gateway commands
            commands::gateway::gateway_get_status,
            commands::gateway::gateway_start,
            commands::gateway::gateway_stop,
            commands::gateway::gateway_rpc,
            // Settings commands
            commands::settings::get_setting,
            commands::settings::set_setting,
            commands::settings::set_many_settings,
            commands::settings::get_all_settings,
            commands::settings::reset_settings,
            commands::settings::export_settings,
            commands::settings::import_settings,
            // Provider commands
            commands::providers::list_provider_vendors,
            commands::providers::list_provider_accounts,
            commands::providers::create_provider_account,
            commands::providers::update_provider_account,
            commands::providers::delete_provider_account,
            commands::providers::get_provider_account,
            commands::providers::get_default_provider_account,
            commands::providers::set_default_provider_account,
            commands::providers::set_provider_api_key,
            commands::providers::get_provider_api_key,
            commands::providers::has_provider_api_key,
            commands::providers::delete_provider_api_key,
            commands::providers::get_provider_api_key_masked,
            // Channel commands
            commands::channels::list_channels,
            commands::channels::get_channel,
            commands::channels::save_channel,
            commands::channels::delete_channel,
            commands::channels::delete_channel_account,
            commands::channels::set_channel_enabled_cmd,
            commands::channels::list_all_channels,
            commands::channels::get_channel_by_id,
            commands::channels::enable_channel,
            commands::channels::disable_channel,
            commands::channels::create_channel,
            commands::channels::remove_channel,
            commands::channels::update_channel_config,
            commands::channels::update_channel_status_cmd,
            commands::channels::validate_channel_credentials,
            commands::channels::start_whatsapp_login,
            commands::channels::stop_whatsapp_login,
            commands::channels::get_whatsapp_login_status,
            commands::channels::has_whatsapp_credentials,
            commands::channels::logout_whatsapp,
            commands::channels::list_whatsapp_accounts,
            // Skill commands
            commands::skills::list_skills,
            commands::skills::install_skill,
            commands::skills::uninstall_skill,
            commands::skills::search_skills,
            commands::skills::get_skill_config,
            commands::skills::update_skill_config,
            commands::skills::get_all_skill_configs,
            commands::skills::clawhub_list_installed,
            commands::skills::open_skill_readme,
            // Cron commands
            commands::cron::list_cron_jobs,
            commands::cron::create_cron_job,
            commands::cron::delete_cron_job,
            // App commands
            commands::app::get_app_info,
            commands::app::get_platform,
            // Window commands
            commands::window::minimize_window,
            commands::window::maximize_window,
            commands::window::close_window,
            // Shell commands
            commands::shell::open_external,
            commands::shell::show_item_in_folder,
            // File commands
            commands::files::read_file,
            commands::files::write_file,
            // OpenClaw commands
            commands::openclaw::openclaw_status,
            commands::openclaw::openclaw_get_skills_dir,
            commands::openclaw::openclaw_get_cli_command,
            // Log commands
            commands::logs::get_log_dir,
            commands::logs::read_log_file,
            commands::logs::list_log_files,
            commands::logs::get_recent_logs,
            // Host API commands
            commands::hostapi::hostapi_fetch,
            // Node.js commands
            commands::nodejs::check_nodejs,
            commands::nodejs::check_nodejs_version,
            // Token usage commands
            commands::usage::get_recent_token_usage,
            // OAuth commands
            commands::oauth::oauth_start,
            commands::oauth::oauth_cancel,
            commands::oauth::oauth_submit_code,
            commands::oauth::oauth_get_status,
        ])
        .setup(|app| {
            // Initialize logging
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
                )
                .init();

            tracing::info!("=== ClawX Application Starting (Tauri) ===");
            tracing::debug!(
                "Runtime: platform={}, arch={}",
                std::env::consts::OS,
                std::env::consts::ARCH
            );

            // Initialize application state (blocking, since setup is not async)
            let state = tauri::async_runtime::block_on(async {
                AppState::new().await.expect("Failed to initialize app state")
            });

            // Make state available to commands
            let logger = state.logger.clone();
            let gateway = state.gateway.clone();
            let channels = state.channels.clone();
            let whatsapp = state.whatsapp.clone();
            let device_oauth = state.device_oauth.clone();
            let browser_oauth = state.browser_oauth.clone();

            app.manage(Arc::new(state));
            app.manage(logger);
            app.manage(channels);
            app.manage(whatsapp);
            app.manage(device_oauth);
            app.manage(browser_oauth);

            // Set app handle on gateway manager for event emission
            tauri::async_runtime::block_on(async {
                gateway.set_app_handle(app.handle().clone()).await;
            });

            tracing::info!("Application state initialized");

            // TODO: Initialize gateway manager state
            // TODO: Load and apply proxy settings
            // TODO: Sync launch-at-startup setting
            // TODO: Create system tray
            // TODO: Create application menu
            // TODO: Auto-start gateway if enabled

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // On macOS and Windows, minimize to tray instead of closing
                #[cfg(not(target_os = "linux"))]
                {
                    window.hide().unwrap();
                    api.prevent_close();
                }

                // On Linux, actually close the window
                #[cfg(target_os = "linux")]
                {
                    // Allow the window to close
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}