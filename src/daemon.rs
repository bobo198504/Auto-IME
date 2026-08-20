use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::model::{config_path, load_config};
use crate::win32;

type BOOL = i32;
type UINT = u32;
type LONG = i32;
type HWND = isize;
type LPARAM = isize;
type WPARAM = usize;
type LRESULT = isize;
type DWORD = u32;

#[repr(C)]
#[derive(Clone, Copy)]
struct POINT {
    x: LONG,
    y: LONG,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MSG {
    hwnd: HWND,
    message: UINT,
    wParam: WPARAM,
    lParam: LPARAM,
    time: DWORD,
    pt: POINT,
}

#[link(name = "user32")]
extern "system" {
    fn PeekMessageW(
        lpMsg: *mut MSG,
        hwnd: HWND,
        wMsgFilterMin: UINT,
        wMsgFilterMax: UINT,
        wRemoveMsg: UINT,
    ) -> BOOL;
    fn TranslateMessage(lpMsg: *const MSG) -> BOOL;
    fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
}

const PM_REMOVE: UINT = 0x0001;

pub fn run_daemon() {
    if win32::is_already_running() {
        return;
    }
    win32::create_quit_event();
    win32::create_focus_event();
    win32::create_about_focus_event();

    let exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(_) => return,
    };

    let initial_config = load_config();
    if initial_config.rules.is_empty() {
        spawn_ui(&exe);
    }

    let config = Arc::new(Mutex::new(initial_config));
    let monitor_config = Arc::clone(&config);
    std::thread::spawn(move || {
        win32::run_monitor(monitor_config);
    });

    let (_tray, open_id, about_id, quit_id) = build_tray();
    let mut last_mtime = config_mtime();

    loop {
        // 驱动托盘隐藏窗口的消息，保证托盘/菜单事件能分发。
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        while unsafe { PeekMessageW(&mut msg, 0, 0, 0, PM_REMOVE) } != 0 {
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        // 配置文件被主界面写入后，热重载规则。
        let mtime = config_mtime();
        if mtime.is_some() && mtime != last_mtime {
            last_mtime = mtime;
            if let Ok(mut guard) = config.lock() {
                *guard = load_config();
            }
        }

        if let Ok(event) = TrayIconEvent::receiver().try_recv() {
            let is_left_click = match &event {
                TrayIconEvent::DoubleClick { button, .. } => *button == MouseButton::Left,
                TrayIconEvent::Click {
                    button,
                    button_state,
                    ..
                } => *button == MouseButton::Left && *button_state == MouseButtonState::Up,
                _ => false,
            };
            if is_left_click {
                win32::signal_focus();
                spawn_ui(&exe);
            }
        }

        if let Ok(event) = MenuEvent::receiver().try_recv() {
            let id = event.id;
            if open_id.as_ref() == Some(&id) {
                win32::signal_focus();
                spawn_ui(&exe);
            } else if about_id.as_ref() == Some(&id) {
                win32::signal_about_focus();
                spawn_about(&exe);
            } else if quit_id.as_ref() == Some(&id) {
                win32::signal_quit();
                std::process::exit(0);
            }
        }

        std::thread::sleep(Duration::from_millis(20));
    }
}

fn spawn_ui(exe: &PathBuf) {
    let mut command = std::process::Command::new(exe);
    command.arg("--ui");
    let _ = command.spawn();
}

fn spawn_about(exe: &PathBuf) {
    let mut command = std::process::Command::new(exe);
    command.arg("--about");
    let _ = command.spawn();
}

fn config_mtime() -> Option<std::time::SystemTime> {
    std::fs::metadata(config_path())
        .and_then(|meta| meta.modified())
        .ok()
}

fn build_tray() -> (
    Option<TrayIcon>,
    Option<MenuId>,
    Option<MenuId>,
    Option<MenuId>,
) {
    let menu = Menu::new();
    let open = MenuItem::new("打开主窗口", true, None);
    let separator = PredefinedMenuItem::separator();
    let about = MenuItem::new("关于", true, None);
    let quit = MenuItem::new("退出", true, None);

    let open_id = open.id().clone();
    let about_id = about.id().clone();
    let quit_id = quit.id().clone();

    let _ = menu.append(&open);
    let _ = menu.append(&separator);
    let _ = menu.append(&about);
    let _ = menu.append(&quit);

    let icon = make_tray_icon();
    let tray = TrayIconBuilder::new()
        .with_tooltip("Auto IME - 输入法自动切换")
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .with_menu_on_right_click(true)
        .with_icon(icon)
        .build()
        .ok();

    (tray, Some(open_id), Some(about_id), Some(quit_id))
}

fn make_tray_icon() -> Icon {
    let size = 64u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let center = (size as f32 - 1.0) / 2.0;
    let teeth = 8u32;
    let r_teeth = center;
    let r_body = center * 0.72;
    let r_hole = center * 0.30;

    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist <= r_hole || dist > r_teeth {
                continue;
            }

            let mut inside = dist <= r_body;
            if !inside {
                let angle = dy.atan2(dx);
                let sector = std::f32::consts::TAU / teeth as f32;
                let mut a = angle % sector;
                if a < 0.0 {
                    a += sector;
                }
                inside = a < sector * 0.5;
            }

            if inside {
                rgba[idx] = 224;
                rgba[idx + 1] = 224;
                rgba[idx + 2] = 224;
                rgba[idx + 3] = 255;
            }
        }
    }

    Icon::from_rgba(rgba, size, size)
        .unwrap_or_else(|_| Icon::from_rgba(vec![0; 64], 4, 4).expect("invalid fallback icon"))
}
