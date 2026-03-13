fn main() {
    // Read OpenClaw version from package.json at build time
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let package_json_path = std::path::Path::new(&manifest_dir)
        .parent()
        .unwrap()
        .join("package.json");

    let openclaw_version = if package_json_path.exists() {
        let content = std::fs::read_to_string(&package_json_path).unwrap_or_default();
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            json["devDependencies"]["openclaw"]
                .as_str()
                .unwrap_or("unknown")
                .to_string()
        } else {
            "unknown".to_string()
        }
    } else {
        "unknown".to_string()
    };

    println!("cargo:rustc-env=OPENCLAW_VERSION={}", openclaw_version);
    println!("cargo:rerun-if-changed=../package.json");

    tauri_build::build()
}