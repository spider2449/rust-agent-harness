#[cfg(target_os = "windows")]
fn main() {
    tauri_build::build();
}

#[cfg(not(target_os = "windows"))]
fn main() {}
