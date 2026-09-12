use std::path::{Path, PathBuf};
use std::process::Command;

/// 定位 Rust windows-gnu 构建所需的 MinGW 工具链，按优先级：
/// 1. 环境变量 MINGW64_ROOT 显式指定（可指向任意含 bin/windres.exe 的工具链，如 w64devkit）；
/// 2. 共享工具链目录：与 _tools 同处一个父目录（D:\Projects\Code\_tools\mingw64）；
/// 3. 项目内 .tools\mingw64（旧布局，clone 到无共享工具链的目录时仍可用）。
fn valid_root(root: &Path) -> bool {
    root.join("bin").join("windres.exe").exists()
}

fn find_toolchain(manifest: &Path) -> Option<PathBuf> {
    if let Ok(root) = std::env::var("MINGW64_ROOT") {
        let candidate = PathBuf::from(root);
        if valid_root(&candidate) {
            return Some(candidate);
        }
    }
    if let Some(parent) = manifest.parent() {
        let shared = parent.join("_tools").join("mingw64");
        if valid_root(&shared) {
            return Some(shared);
        }
    }
    let local = manifest.join(".tools").join("mingw64");
    if valid_root(&local) {
        return Some(local);
    }
    None
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let assets = manifest.join("assets");
    let obj = out_dir.join("app-res.o");

    let toolchain = find_toolchain(&manifest);

    // 优先使用工具链自带的 windres，找不到时退回 PATH 中的 windres。
    let windres = toolchain
        .as_ref()
        .map(|root| root.join("bin").join("windres.exe"))
        .filter(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from("windres"));

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

    // rustup 的 windows-gnu 工具链自带的链接器缺少 imm32/shlwapi 导入库。
    // 整个 lib 目录加入搜索路径会与 rustup 自带运行库冲突，因此只把这两个库
    // 复制到构建输出目录后再加入搜索路径，不在工具链或项目中留下专属文件。
    if let Some(root) = toolchain {
        let link_dir = out_dir.join("link-libs");
        if std::fs::create_dir_all(&link_dir).is_ok() {
            let mut copied = 0;
            for name in ["libimm32.a", "libshlwapi.a"] {
                let src = root.join("lib").join(name);
                if src.exists() && std::fs::copy(&src, link_dir.join(name)).is_ok() {
                    copied += 1;
                }
            }
            if copied > 0 {
                println!("cargo:rustc-link-search=native={}", link_dir.display());
            }
        }
    }
}
