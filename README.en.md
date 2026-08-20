# Auto IME

A Windows tray tool that automatically switches the input method between Chinese and English based on the currently focused control.

## Features

- Matches the current focused control using both Win32 and UI Automation.
- Rule conditions can be combined: process name, window title, window class, control text, control class, control type, automation ID, ancestor label, and ancestor class.
- Two switching methods:
  - Simulate a hotkey (`Ctrl+Space` by default)
  - IME low-level switch (directly sets the IME open state for better compatibility)
- Restores the application's input-method baseline after focus leaves matched controls.
- Lives in the system tray: single/double click focuses the settings window, and the right-click menu provides "Open / About / Exit".
- Portable: `config.json` sits next to `auto-ime.exe`.

## Usage

1. Run `auto-ime.exe`.
2. On first run with no rules, the settings window opens automatically; afterwards it can be opened from the tray menu.
3. Create or edit rules:
   - Click "Edit" to enter editing mode;
   - Press `Ctrl+Alt+Q` or click "Capture", then click the target control;
   - Tick the conditions you need and set the action (Chinese/English), then save.
4. Closing the settings window keeps the app running in the tray; choose "Exit" from the tray menu to quit completely.

## Build

Requires the Rust toolchain and MinGW `windres` (bundled under `.tools/mingw64`).

```powershell
cargo build --release
```

## Notes

- The IME Chinese/English state is read via the `WM_IME_CONTROL` open status, which works for internally-switching IMEs such as Bingling Wubi and Microsoft Pinyin/Wubi.
- Approaches confirmed not to work are documented in `FAILED_APPROACHES.md`.

## Author

Abo

## Version

1.1
