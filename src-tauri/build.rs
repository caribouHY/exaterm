use std::{env, fs, path::PathBuf};

fn main() {
    ensure_sidecar_paths_exist();
    tauri_build::build()
}

fn ensure_sidecar_paths_exist() {
    let target = env::var("TARGET").expect("Cargo TARGET is set for build scripts");
    let extension = if target.contains("windows") {
        ".exe"
    } else {
        ""
    };
    let directory = PathBuf::from("binaries");
    fs::create_dir_all(&directory).expect("create sidecar binary directory");

    let path = directory.join(format!("exaterm-mcp-{target}{extension}"));
    if !path.exists() {
        fs::write(path, []).expect("create sidecar placeholder");
    }
}
