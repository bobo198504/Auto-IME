# 更新日志

本项目所有显著变更都会记录在此文件。

## [1.2.1] - 2026-09-09

### 新增

- 键盘焦点切换机制：除鼠标点击外，现在通过 WinEvent hook 监听 `EVENT_SYSTEM_FOREGROUND` 与 `EVENT_OBJECT_FOCUS`，当焦点因 Tab、方向键、Alt+Tab 等键盘操作移动时，同样按规则自动切换输入法。
- 键盘焦点场景直接以真正拥有键盘焦点的控件为匹配对象，规则评估与鼠标点击共用同一套逻辑。

### 修复

- 修复 `cargo build --release` 在未安装完整 MinGW 的环境下链接失败的问题：`build.rs` 现在会自动定位共享工具链目录（`MINGW64_ROOT` 或 `_tools/mingw64`），并补充 rustup windows-gnu 工具链缺失的 `imm32`/`shlwapi` 导入库。

## [1.2] - 2026-08-21

- 移除默认输入法选项，作者署名 abo。

## [1.1] - 2026-08-20

- 首个版本：基于当前焦点控件的输入法自动切换，支持 Win32 + UI Automation 精确识别控件。
