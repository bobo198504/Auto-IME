use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let assets = manifest.join("assets");
    let obj = out_dir.join("app-res.o");

    let windres = manifest
        .join(".tools")
        .join("mingw64")
        .join("bin")
        .join("windres.exe");
    let windres = if windres.exists() {
        windres
    } else {
        PathBuf::from("windres")
    };

    let status = Command::new(&windres)
        .current_dir(&assets)
        .args([
            "--preprocessor=preprocess.cmd",
            "-i",
            "app.rc",
            "-O",
            "coff",
            "-o",
            obj.to_str().unwrap(),
        ])
        .status()
        .expect("failed to run windres");
    assert!(status.success(), "windres failed to compile app.rc");

    println!("cargo:rustc-link-arg={}", obj.display());
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/autoime.ico");
    println!("cargo:rerun-if-changed=assets/preprocess.cmd");
}
