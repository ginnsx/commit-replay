fn main() {
    println!("cargo:rerun-if-changed=../package.json");
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
    tauri_build::build()
}
