//! System tray management with i18n support

use std::sync::Arc;
use tauri::{
    AppHandle, Manager, Wry,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
};
use tokio::sync::RwLock;

/// Tray menu text translations
#[derive(Debug, Clone)]
pub struct TrayTranslations {
    pub show_window: String,
    pub hide_window: String,
    pub quit: String,
}

impl Default for TrayTranslations {
    fn default() -> Self {
        Self {
            show_window: "Show Window".to_string(),
            hide_window: "Hide Window".to_string(),
            quit: "Quit".to_string(),
        }
    }
}

impl TrayTranslations {
    /// Create translations for a given language code
    pub fn for_language(lang: &str) -> Self {
        match lang {
            "zh" => Self {
                show_window: "显示窗口".to_string(),
                hide_window: "隐藏窗口".to_string(),
                quit: "退出".to_string(),
            },
            "ja" => Self {
                show_window: "ウィンドウを表示".to_string(),
                hide_window: "ウィンドウを隠す".to_string(),
                quit: "終了".to_string(),
            },
            _ => Self::default(), // English fallback
        }
    }
}

/// Tray menu state that can be updated
pub struct TrayMenuState {
    pub tray_icon: Arc<RwLock<Option<TrayIcon>>>,
    pub translations: Arc<RwLock<TrayTranslations>>,
    pub menu_items: Arc<RwLock<Option<TrayMenuItems>>>,
}

/// Holds references to menu items for text updates
pub struct TrayMenuItems {
    pub show_item: MenuItem<Wry>,
    pub hide_item: MenuItem<Wry>,
    pub quit_item: MenuItem<Wry>,
}

impl TrayMenuState {
    pub fn new() -> Self {
        Self {
            tray_icon: Arc::new(RwLock::new(None)),
            translations: Arc::new(RwLock::new(TrayTranslations::default())),
            menu_items: Arc::new(RwLock::new(None)),
        }
    }
}

/// Initialize the system tray with the given app handle
pub fn init_tray(app: &AppHandle) -> Result<TrayIcon, Box<dyn std::error::Error>> {
    let tray_icon = app.default_window_icon().cloned().ok_or_else(|| {
        tracing::error!("Failed to get default window icon for tray");
        "Failed to get default window icon"
    })?;

    // Get or create the tray state
    let tray_state: Arc<TrayMenuState> = if let Some(state) = app.try_state::<Arc<TrayMenuState>>() {
        state.inner().clone()
    } else {
        let state = Arc::new(TrayMenuState::new());
        app.manage(state.clone());
        state
    };

    // Get current translations (or use default)
    let translations = tauri::async_runtime::block_on(async {
        tray_state.translations.read().await.clone()
    });

    // Create menu items
    let show_item = MenuItem::with_id(app, "show", &translations.show_window, true, None::<&str>)?;
    let hide_item = MenuItem::with_id(app, "hide", &translations.hide_window, true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", &translations.quit, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;

    // Store menu items for later updates
    let menu_items = TrayMenuItems {
        show_item: show_item.clone(),
        hide_item: hide_item.clone(),
        quit_item: quit_item.clone(),
    };

    // Create menu
    let menu = Menu::with_items(app, &[
        &show_item,
        &hide_item,
        &separator,
        &quit_item,
    ])?;

    // Store menu items in state
    tauri::async_runtime::block_on(async {
        *tray_state.menu_items.write().await = Some(menu_items);
    });

    // Build tray icon
    let tray = TrayIconBuilder::new()
        .icon(tray_icon)
        .tooltip("ClawX")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(move |tray, event| {
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    let app_handle = tray.app_handle();
                    if let Some(window) = app_handle.get_webview_window("main") {
                        if let Ok(true) = window.is_visible() {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
                _ => {}
            }
        })
        .on_menu_event(move |app, event| {
            match event.id().as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "hide" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                    }
                }
                "quit" => {
                    tracing::info!("Quit requested from system tray menu");
                    app.exit(0);
                }
                _ => {}
            }
        })
        .build(app)?;

    // Store tray icon reference
    tauri::async_runtime::block_on(async {
        *tray_state.tray_icon.write().await = Some(tray.clone());
    });

    tracing::info!("System tray initialized successfully");
    Ok(tray)
}

/// Update tray menu text with new translations
pub async fn update_tray_language(
    app: &AppHandle,
    language: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let tray_state = app
        .try_state::<Arc<TrayMenuState>>()
        .ok_or("Tray state not found")?;

    let translations = TrayTranslations::for_language(language);

    // Update translations in state
    {
        let mut state_translations = tray_state.translations.write().await;
        *state_translations = translations.clone();
    }

    // Update menu item texts if they exist
    if let Some(menu_items) = tray_state.menu_items.read().await.as_ref() {
        menu_items.show_item.set_text(&translations.show_window)?;
        menu_items.hide_item.set_text(&translations.hide_window)?;
        menu_items.quit_item.set_text(&translations.quit)?;
        tracing::debug!("Tray menu text updated for language: {}", language);
    }

    Ok(())
}

/// Get the current tray language from AppState settings
pub async fn get_tray_language_from_state(app: &AppHandle) -> String {
    use crate::core::AppState;
    use tauri::Manager;

    if let Some(state) = app.try_state::<std::sync::Arc<AppState>>() {
        let settings = state.settings.read().await;
        settings.get("language")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "en".to_string())
    } else {
        "en".to_string()
    }
}
