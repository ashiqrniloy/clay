use std::{env, path::PathBuf};

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("windows")
            .join("no-uac.manifest");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg-bin=update-doc-registry=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bin=update-doc-registry=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
}
