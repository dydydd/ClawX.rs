//! File operations IPC command handlers

use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};

/// Staged file info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedFile {
    pub id: String,
    pub file_name: String,
    pub mime_type: String,
    pub file_size: u64,
    pub staged_path: String,
    pub preview: Option<String>,
}

/// Read a file's contents
#[tauri::command]
pub async fn read_file(path: String) -> Result<String, String> {
    let path = PathBuf::from(&path);

    // Security: Ensure we're only reading from allowed directories
    // TODO: Implement proper sandboxing

    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))
}

/// Write content to a file
#[tauri::command]
pub async fn write_file(path: String, content: String) -> Result<(), String> {
    let path = PathBuf::from(&path);

    // Security: Ensure we're only writing to allowed directories
    // TODO: Implement proper sandboxing

    tokio::fs::write(&path, content)
        .await
        .map_err(|e| format!("Failed to write file: {}", e))
}

/// Stage file paths for attachment
/// Copies files to a staging directory and returns file info with previews
#[tauri::command]
pub async fn stage_file_paths(file_paths: Vec<String>) -> Result<Vec<StagedFile>, String> {
    use uuid::Uuid;
    use std::fs;

    // Create staging directory
    let staging_dir = dirs::cache_dir()
        .ok_or("Could not determine cache directory")?
        .join("ClawX")
        .join("staged-files");

    fs::create_dir_all(&staging_dir)
        .map_err(|e| format!("Failed to create staging directory: {}", e))?;

    let mut results = Vec::new();

    for path_str in file_paths {
        let source_path = PathBuf::from(&path_str);

        // Get file metadata
        let metadata = fs::metadata(&source_path)
            .map_err(|e| format!("Failed to read file metadata for {}: {}", path_str, e))?;

        let file_name = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let file_size = metadata.len();

        // Detect MIME type
        let mime_type = mime_guess::from_path(&source_path)
            .first_or_octet_stream()
            .to_string();

        // Generate unique ID and staged path
        let id = Uuid::new_v4().to_string();
        let staged_path = staging_dir.join(&id);

        // Copy file to staging directory
        fs::copy(&source_path, &staged_path)
            .map_err(|e| format!("Failed to copy file {}: {}", path_str, e))?;

        // Generate preview for images
        let preview = if mime_type.starts_with("image/") {
            generate_image_preview(&staged_path).ok()
        } else {
            None
        };

        results.push(StagedFile {
            id,
            file_name,
            mime_type,
            file_size,
            staged_path: staged_path.to_string_lossy().to_string(),
            preview,
        });
    }

    Ok(results)
}

/// Stage file buffer (base64) for attachment
#[tauri::command]
pub async fn stage_file_buffer(
    base64: String,
    file_name: String,
    mime_type: String,
) -> Result<StagedFile, String> {
    use uuid::Uuid;
    use std::fs;

    // Create staging directory
    let staging_dir = dirs::cache_dir()
        .ok_or("Could not determine cache directory")?
        .join("ClawX")
        .join("staged-files");

    fs::create_dir_all(&staging_dir)
        .map_err(|e| format!("Failed to create staging directory: {}", e))?;

    // Decode base64
    let buffer = BASE64_STANDARD.decode(&base64)
        .map_err(|e| format!("Failed to decode base64: {}", e))?;

    let file_size = buffer.len() as u64;

    // Generate unique ID and staged path
    let id = Uuid::new_v4().to_string();
    let staged_path = staging_dir.join(&id);

    // Write buffer to staging directory
    fs::write(&staged_path, &buffer)
        .map_err(|e| format!("Failed to write staged file: {}", e))?;

    // Generate preview for images
    let preview = if mime_type.starts_with("image/") {
        generate_image_preview(&staged_path).ok()
    } else {
        None
    };

    Ok(StagedFile {
        id,
        file_name,
        mime_type,
        file_size,
        staged_path: staged_path.to_string_lossy().to_string(),
        preview,
    })
}

/// Get thumbnails/preview for file paths
#[tauri::command]
pub async fn get_file_thumbnails(
    paths: Vec<String>,
) -> Result<std::collections::HashMap<String, ThumbnailInfo>, String> {
    use std::fs;

    let mut results = std::collections::HashMap::new();

    for path_str in paths {
        let path = PathBuf::from(&path_str);

        if !path.exists() {
            continue;
        }

        let metadata = fs::metadata(&path).ok();
        let file_size = metadata.map(|m| m.len()).unwrap_or(0);

        // Detect MIME type
        let mime_type = mime_guess::from_path(&path)
            .first_or_octet_stream()
            .to_string();

        // Generate preview for images
        let preview = if mime_type.starts_with("image/") {
            generate_image_preview(&path).ok()
        } else {
            None
        };

        results.insert(path_str, ThumbnailInfo {
            preview,
            file_size,
        });
    }

    Ok(results)
}

/// Thumbnail info response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailInfo {
    pub preview: Option<String>,
    pub file_size: u64,
}

/// Generate a base64 preview for an image file
fn generate_image_preview(path: &PathBuf) -> Result<String, String> {
    use std::fs;
    use std::io::Cursor;

    // Read the image file
    let buffer = fs::read(path)
        .map_err(|e| format!("Failed to read image: {}", e))?;

    // For now, just return base64 of the original image
    // TODO: Add image resizing for large images
    let mime_type = mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string();

    Ok(format!("data:{};base64,{}", mime_type, BASE64_STANDARD.encode(&buffer)))
}