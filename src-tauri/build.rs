fn main() {
    println!("cargo:rerun-if-changed=../package.json");
    println!("cargo:rerun-if-env-changed=TAURI_SIGNING_PUBLIC_KEY");
    let manifest_dir = std::path::PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"),
    );
    let package_path = manifest_dir.join("../package.json");
    let package_json = std::fs::read_to_string(&package_path)
        .expect("failed to read app version from ../package.json");
    let package: serde_json::Value =
        serde_json::from_str(&package_json).expect("failed to parse ../package.json");
    let version = package
        .get("version")
        .and_then(serde_json::Value::as_str)
        .expect("missing version in ../package.json");
    println!("cargo:rustc-env=RELAY_APP_VERSION={version}");
    if let Ok(public_key) = std::env::var("TAURI_SIGNING_PUBLIC_KEY") {
        let public_key = public_key.trim();
        if !public_key.is_empty() {
            // 发布工作流注入公钥；私钥只保留在 GitHub 的保密配置中。
            println!("cargo:rustc-env=RELAY_UPDATER_PUBLIC_KEY={public_key}");
        }
    }
    tauri_build::build()
}
