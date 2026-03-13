//! Storage abstraction layer

mod keyring;
mod secrets;
mod settings;
mod providers;

pub use keyring::*;
pub use secrets::*;
pub use settings::*;
pub use providers::ProviderStore;

/// Get the default settings file path
pub fn get_settings_path() -> std::path::PathBuf {
    let data_dir = dirs::data_local_dir().expect("Failed to get data directory");
    data_dir.join("ClawX").join("settings.json")
}

/// Get the default providers file path
pub fn get_providers_path() -> std::path::PathBuf {
    let data_dir = dirs::data_local_dir().expect("Failed to get data directory");
    data_dir.join("ClawX").join("providers.json")
}