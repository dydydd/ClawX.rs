//! Shell operations IPC command handlers

use tauri_plugin_shell::ShellExt;

/// Open an external URL in the default browser
#[tauri::command]
pub async fn open_external(app: tauri::AppHandle, url: String) -> Result<(), String> {
    app.shell()
        .open(&url, None)
        .map_err(|e| format!("Failed to open URL: {}", e))
}

/// Open a file or folder with the system default application
#[tauri::command]
pub async fn open_path(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let path = std::path::Path::new(&path);

    if !path.exists() {
        return Err(format!("Path does not exist: {}", path.display()));
    }

    // Use Tauri shell to open the path with the default application
    let path_string = path.display().to_string();
    app.shell()
        .open(&path_string, None)
        .map_err(|e| format!("Failed to open path: {}", e))
}

/// Show a file or folder in the file manager
#[tauri::command]
pub async fn show_item_in_folder(path: String) -> Result<(), String> {
    let path = std::path::Path::new(&path);

    if !path.exists() {
        return Err(format!("Path does not exist: {}", path.display()));
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .args(["/select,", &path.display().to_string()])
            .spawn()
            .map_err(|e| format!("Failed to open file manager: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &path.display().to_string()])
            .spawn()
            .map_err(|e| format!("Failed to open file manager: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        // Try various file managers
        let file_managers = ["nautilus", "dolphin", "thunar", "pcmanfm"];

        let mut success = false;
        for fm in &file_managers {
            if std::process::Command::new(fm)
                .arg(&path)
                .spawn()
                .is_ok()
            {
                success = true;
                break;
            }
        }

        if !success {
            return Err("Failed to open file manager. Please install a file manager like nautilus, dolphin, or thunar.".to_string());
        }
    }

    Ok(())
}