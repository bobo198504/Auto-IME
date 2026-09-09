use std::path::{Path, PathBuf};
use std::process::Command;

/// 定位本项目使用的 MinGW-w64 工具链，按优先级：
/// 1. 环境变量 MINGW64_ROOT 显式指定（可移植到任意位置）；
/// 2. 共享工具链目录：与 _tools 同处一个父目录（D:\Projects\Code\_tools\mingw64）；
/// 3. 项目内 .tools\mingw64（旧布局，clone 到未共享工具链的目录时仍可用）。
///
/// 该工具链提供 windres 与 rustup windows-gnu 工具链缺少的
/// imm32/shlwapi 导入库（见下方 lib-extra 说明）。
fn find_toolchain(manifest: &Path) -> Option<PathBuf> {
    if let Ok(root) = std::env::var("MINGW64_ROOT") {
        let candidate = PathBuf::from(root);
        if candidate.join("bin").join("windres.exe").exists() {
            return Some(candidate);
        }
    }
    let shared = manifest.parent()?.join("_tools").join("mingw64");
    if shared.join("bin").join("windres.exe").exists() {
        return Some(shared);
    }
    let local = manifest.join(".tools").join("mingw64");
    if local.join("bin").join("windres.exe").exists() {
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

    // rustup 的 windows-gnu 工具链自带的链接器缺少 imm32/shlwapi 导入库，
    // 这两个库随共享工具链放在 lib-extra 中；不能把整个 lib 加入搜索路径，
    // 否则会与 rustup 自带运行库冲突。
    if let Some(root) = toolchain {
        let lib_extra = root.join("lib-extra");
        if lib_extra.exists() {
            println!("cargo:rustc-link-search=native={}", lib_extra.display());
        }
    }
}
