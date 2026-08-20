#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod daemon;
mod model;
mod win32;

use std::sync::mpsc;

use app::{load_about_pos, AboutApp, AutoImeApp};
use model::{load_config, save_config};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--about") {
        run_about();
    } else if args.iter().any(|a| a == "--ui" || a == "--settings") {
        run_ui();
    } else {
        daemon::run_daemon();
    }
}

fn run_ui() {
    if win32::is_ui_already_running() {
        return;
    }

    let config = load_config();
    let initial_window_pos = config.window_pos;
    let config = std::sync::Arc::new(std::sync::Mutex::new(config));

    let (hotkey_tx, hotkey_rx) = mpsc::channel();

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([920.0, 620.0])
        .with_min_inner_size([760.0, 500.0])
        .with_title("Auto IME - 输入法自动切换");
    if let Some((x, y)) = initial_window_pos {
        viewport = viewport.with_position(eframe::egui::Pos2::new(x, y));
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let config_for_save = std::sync::Arc::clone(&config);
    let _ = eframe::run_native(
        "Auto IME",
        native_options,
        Box::new(move |cc| {
            let hotkey_tx = hotkey_tx.clone();
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                if let Err(err) = win32::run_hotkey_listener(hotkey_tx, ctx) {
                    eprintln!("全局热键监听启动失败: {err}");
                }
            });
            Ok(Box::new(AutoImeApp::new(cc, config_for_save, hotkey_rx)))
        }),
    );

    if let Ok(guard) = config.lock() {
        let _ = save_config(&guard);
    };
}

fn run_about() {
    if win32::is_about_already_running() {
        return;
    }

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([340.0, 300.0])
        .with_title("关于")
        .with_resizable(false);
    if let Some((x, y)) = load_about_pos() {
        viewport = viewport.with_position(eframe::egui::Pos2::new(x, y));
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let _ = eframe::run_native(
        "Auto IME - 关于",
        native_options,
        Box::new(|cc| Ok(Box::new(AboutApp::new(cc)))),
    );
}
