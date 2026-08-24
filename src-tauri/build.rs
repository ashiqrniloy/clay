fn main() {
    if let Ok(target) = std::env::var("TARGET") {
        println!("cargo:rustc-env=CLAY_HOST_TRIPLE={target}");
    }
    tauri_build::build()
}
