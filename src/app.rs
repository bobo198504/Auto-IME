use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::model::{config_path, save_config, Action, AppConfig, MatchMode, Rule, SwitchMethod, WindowInfo};
use crate::win32::{self, HotkeyEvent};

pub struct AutoImeApp {
    config: Arc<Mutex<AppConfig>>,
    hotkey_rx: Receiver<HotkeyEvent>,
    selected: Option<usize>,
    status: String,
    capture_armed: bool,
    capturing_hotkey: bool,
    click_capture_rx: Option<Receiver<WindowInfo>>,
    editing: bool,
    draft: Option<AppConfig>,
    quitting: bool,
}

impl AutoImeApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        config: Arc<Mutex<AppConfig>>,
        hotkey_rx: Receiver<HotkeyEvent>,
    ) -> Self {
        install_chinese_font(&cc.egui_ctx);
        win32::set_ui_repaint(cc.egui_ctx.clone());

        let initial = config
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default();

        let selected = if initial.rules.is_empty() {
            None
        } else {
            Some(0)
        };

        Self {
            config,
            hotkey_rx,
            selected,
            status: "就绪：Ctrl+Alt+Q 进入点击捕获，然后点击目标控件。".to_string(),
            capture_armed: false,
            capturing_hotkey: false,
            click_capture_rx: None,
            editing: false,
            draft: None,
            quitting: false,
        }
    }

    fn current_config(&self) -> AppConfig {
        self.config
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    fn editing_config(&mut self) -> AppConfig {
        if self.draft.is_none() {
            self.draft = Some(self.current_config());
        }
        self.draft.clone().unwrap_or_else(|| self.current_config())
    }

    fn persist(&mut self, cfg: &AppConfig) -> Result<(), String> {
        if let Ok(mut guard) = self.config.lock() {
            *guard = cfg.clone();
        }
        save_config(cfg)
    }

    fn publish(&mut self, cfg: &AppConfig) {
        match self.persist(cfg) {
            Ok(()) => {
                let base = self.status.trim_end_matches("（已保存）").trim_end();
                self.status = format!("{base}（已保存）");
            }
            Err(err) => {
                self.status = format!("保存失败：{err}");
            }
        }
    }

    fn handle_close_request(&mut self, ctx: &egui::Context) {
        let close_requested = ctx.input(|input| input.viewport().close_requested());
        if close_requested && !self.quitting {
            self.quitting = true;
            let mut cfg = self.current_config();
            if let Some(rect) = ctx.input(|i| i.viewport().outer_rect) {
                cfg.window_pos = Some((rect.min.x, rect.min.y));
            }
            let _ = self.persist(&cfg);
            std::process::exit(0);
        }
    }

    fn arm_click_capture(&mut self, ctx: &egui::Context) {
        if !self.editing {
            self.status = "请先点“编辑”进入可编辑状态，再点击捕获。".to_string();
            return;
        }
        self.capture_armed = true;
        win32::set_capture_armed(true);
        let (tx, rx) = mpsc::channel::<WindowInfo>();
        self.click_capture_rx = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            win32::wait_for_click(tx, ctx);
        });
        self.status = "点击捕获已就绪：请点击要捕获的控件。".to_string();
    }

    fn apply_capture(&mut self, cfg: &mut AppConfig, info: WindowInfo) {
        let is_new = self.selected.is_none() || self.selected.unwrap_or(0) >= cfg.rules.len();
        if is_new {
            cfg.rules.push(Rule::default());
            self.selected = Some(cfg.rules.len() - 1);
            self.editing = true;
        }

        if let Some(index) = self.selected {
            if let Some(rule) = cfg.rules.get_mut(index) {
                let control_label = if !info.control_text.is_empty() {
                    info.control_text.trim()
                } else if !info.control_type.is_empty() {
                    info.control_type.trim()
                } else {
                    info.control_class.trim()
                };
                rule.name = format!("{} / {}", info.process_name.trim(), control_label);
                rule.process_pattern = info.process_name.trim().to_string();
                rule.window_title_pattern = info.window_title.trim().to_string();
                rule.window_class_pattern = info.window_class.trim().to_string();
                rule.control_text_pattern = smart_control_pattern(&info.control_text);
                rule.control_class_pattern = info.control_class.trim().to_string();
                rule.control_type_pattern = info.control_type.trim().to_string();
                rule.automation_id_pattern = info.automation_id.trim().to_string();
                rule.container_text_pattern = info.container_text.trim().to_string();
                rule.ancestor_class_pattern = info
                    .ancestor_classes
                    .first()
                    .cloned()
                    .unwrap_or_default();
                if is_new {
                    // 新建规则默认只勾选“进程名 + 控件文本”，其它字段往往相同或不可靠。
                    rule.use_process = true;
                    rule.use_window_title = false;
                    rule.use_window_class = false;
                    rule.use_control_text = !info.control_text.trim().is_empty();
                    rule.use_control_class = false;
                    rule.use_control_type = false;
                    rule.use_automation_id = false;
                    rule.use_container_text = !info.container_text.trim().is_empty();
                    rule.use_ancestor_class = false;
                }
                rule.enabled = true;
            }
        }

        self.status = format!(
            "已捕获：{} / {} / {}",
            info.process_name.trim(),
            info.window_title.trim(),
            info.control_class.trim()
        );
    }

    fn remove_selected(&mut self, cfg: &mut AppConfig) {
        let Some(index) = self.selected else {
            return;
        };
        if index >= cfg.rules.len() {
            return;
        }

        cfg.rules.remove(index);
        self.selected = if cfg.rules.is_empty() {
            None
        } else if index >= cfg.rules.len() {
            Some(cfg.rules.len() - 1)
        } else {
            Some(index)
        };
        self.editing = false;
    }

    fn process_events(&mut self, ctx: &egui::Context) {
        let clicked = self
            .click_capture_rx
            .as_ref()
            .and_then(|rx| rx.try_recv().ok());
        if let Some(info) = clicked {
            self.click_capture_rx = None;
            self.capture_armed = false;
            win32::set_capture_armed(false);
            let mut cfg = self.editing_config();
            self.apply_capture(&mut cfg, info);
            self.draft = Some(cfg);
        }

        while let Ok(event) = self.hotkey_rx.try_recv() {
            match event {
                HotkeyEvent::CaptureArm => self.arm_click_capture(ctx),
            }
        }
    }

}

impl eframe::App for AutoImeApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_close_request(ctx);
        self.process_events(ctx);
        if win32::should_quit() {
            let mut cfg = self.current_config();
            if let Some(rect) = ctx.input(|i| i.viewport().outer_rect) {
                cfg.window_pos = Some((rect.min.x, rect.min.y));
            }
            let _ = self.persist(&cfg);
            std::process::exit(0);
        }
        if win32::should_focus() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut cfg = if self.editing {
            self.editing_config()
        } else {
            self.current_config()
        };
        let mut changed = false;
        let mut save_clicked = false;

        if self.capturing_hotkey {
            match read_hotkey_combo(ui.ctx()) {
                Some(Some(combo)) => {
                    cfg.ime_toggle_hotkey = combo;
                    changed = true;
                    self.capturing_hotkey = false;
                }
                Some(None) => {
                    self.capturing_hotkey = false;
                }
                None => {}
            }
        }

        egui::Panel::top("top_panel").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui
                    .checkbox(&mut cfg.default_ime_chinese, "系统输入法默认中文")
                    .changed()
                {
                    changed = true;
                }

                ui.separator();
                ui.label(&self.status);
            });

            ui.horizontal(|ui| {
                ui.label("点击捕获：Ctrl+Alt+Q 进入待捕获，然后点击目标控件。");
                if ui.button("点击捕获").clicked() {
                    let ctx = ui.ctx().clone();
                    self.arm_click_capture(&ctx);
                }
            });

            ui.horizontal(|ui| {
                ui.label("输入法切换热键：");
                if self.capturing_hotkey {
                    ui.label("请按下快捷键，Esc 取消");
                } else if ui.button(&cfg.ime_toggle_hotkey).clicked() {
                    self.capturing_hotkey = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("切换方式：");
                egui::ComboBox::from_id_salt("switch_method_combo")
                    .selected_text(cfg.switch_method.label())
                    .show_ui(ui, |ui| {
                        for method in SwitchMethod::ALL {
                            if ui
                                .selectable_label(cfg.switch_method == method, method.label())
                                .clicked()
                            {
                                cfg.switch_method = method;
                                changed = true;
                            }
                        }
                    });
                ui.label(match cfg.switch_method {
                    SwitchMethod::Simulate => "使用上方热键模拟切换",
                    SwitchMethod::Ime => "直接调用 IME 底层接口，无需热键",
                });
            });

            ui.add_space(4.0);
        });

        egui::Panel::left("rules_panel")
            .resizable(true)
            .default_size(300.0)
            .show(ui, |ui| {
                ui.heading(format!("规则 ({})", cfg.rules.len()));
                ui.add_space(4.0);

                for (index, rule) in cfg.rules.iter().enumerate() {
                    let selected = self.selected == Some(index);
                    let mut text = rule.label();
                    if !rule.enabled {
                        text = format!("[停用] {text}");
                    }
                    if ui.selectable_label(selected, text).clicked() {
                        self.selected = Some(index);
                        self.editing = false;
                        self.draft = None;
                    }
                    ui.separator();
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("新建规则").clicked() {
                        cfg.rules.push(Rule::default());
                        self.selected = Some(cfg.rules.len() - 1);
                        self.editing = true;
                        changed = true;
                    }
                    if ui.button("删除规则").clicked() {
                        self.remove_selected(&mut cfg);
                        changed = true;
                    }
                });

                if let Some(index) = self.selected {
                    if index < cfg.rules.len() {
                        ui.add_space(8.0);
                        ui.label("优先级（数字越小越先匹配）");
                        if ui
                            .add(egui::DragValue::new(&mut cfg.rules[index].priority).speed(1))
                            .changed()
                        {
                            changed = true;
                        }
                    }
                }
            });

        egui::CentralPanel::default().show(ui, |ui| {
            let Some(index) = self.selected else {
                ui.centered_and_justified(|ui| {
                    ui.label("请选择左侧规则，或点击“新建规则”开始。");
                });
                return;
            };
            if index >= cfg.rules.len() {
                self.selected = if cfg.rules.is_empty() {
                    None
                } else {
                    Some(cfg.rules.len() - 1)
                };
                self.editing = false;
                return;
            }

            let rule = &mut cfg.rules[index];
            ui.heading("编辑规则");
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                if self.editing {
                    if ui.button("保存").clicked() {
                        save_clicked = true;
                    }
                } else if ui.button("编辑").clicked() {
                    self.editing = true;
                    self.draft = None;
                }
            });

            ui.add_enabled_ui(self.editing, |ui| {
            egui::Grid::new("rule_editor_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("规则名称");
                    if ui.text_edit_singleline(&mut rule.name).changed() {
                        changed = true;
                    }
                    ui.end_row();

                    ui.label("启用");
                    if ui.checkbox(&mut rule.enabled, "").changed() {
                        changed = true;
                    }
                    ui.end_row();

                    ui.label("匹配方式");
                    egui::ComboBox::from_id_salt("match_mode_combo")
                        .selected_text(rule.match_mode.label())
                        .show_ui(ui, |ui| {
                            for mode in MatchMode::ALL {
                                if ui
                                    .selectable_label(rule.match_mode == mode, mode.label())
                                    .clicked()
                                {
                                    rule.match_mode = mode;
                                    changed = true;
                                }
                            }
                        });
                    ui.end_row();

                    ui.label("动作");
                    egui::ComboBox::from_id_salt("action_combo")
                        .selected_text(rule.action.label())
                        .show_ui(ui, |ui| {
                            for action in Action::ALL {
                                if ui
                                    .selectable_label(rule.action == action, action.label())
                                    .clicked()
                                {
                                    rule.action = action;
                                    changed = true;
                                }
                            }
                        });
                    ui.end_row();

                    ui.label("进程名");
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut rule.use_process, "").changed() {
                            changed = true;
                        }
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut rule.process_pattern)
                                    .hint_text("例如 chrome.exe，留空表示任意"),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                    });
                    ui.end_row();

                    ui.label("窗口标题");
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut rule.use_window_title, "").changed() {
                            changed = true;
                        }
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut rule.window_title_pattern)
                                    .hint_text("例如 *微信*，留空表示任意"),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                    });
                    ui.end_row();

                    ui.label("窗口类名");
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut rule.use_window_class, "").changed() {
                            changed = true;
                        }
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut rule.window_class_pattern)
                                    .hint_text("例如 Chrome_WidgetWin_*，留空表示任意"),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                    });
                    ui.end_row();

                    ui.label("控件文本");
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut rule.use_control_text, "").changed() {
                            changed = true;
                        }
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut rule.control_text_pattern)
                                    .hint_text("例如 *搜索*，留空表示任意"),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                    });
                    ui.end_row();

                    ui.label("控件类名");
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut rule.use_control_class, "").changed() {
                            changed = true;
                        }
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut rule.control_class_pattern)
                                    .hint_text("例如 Edit 或 Chrome_*，留空表示任意"),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                    });
                    ui.end_row();

                    ui.label("控件类型");
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut rule.use_control_type, "").changed() {
                            changed = true;
                        }
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut rule.control_type_pattern)
                                    .hint_text("例如 Edit、Button、Document，留空表示任意"),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                    });
                    ui.end_row();

                    ui.label("自动化ID");
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut rule.use_automation_id, "").changed() {
                            changed = true;
                        }
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut rule.automation_id_pattern)
                                    .hint_text("例如 searchInput，留空表示任意"),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                    });
                    ui.end_row();

                    ui.label("父级标签");
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut rule.use_container_text, "").changed() {
                            changed = true;
                        }
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut rule.container_text_pattern)
                                    .hint_text("例如 Media Explorer，留空表示任意"),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                    });
                    ui.end_row();

                    ui.label("父级类名");
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut rule.use_ancestor_class, "").changed() {
                            changed = true;
                        }
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut rule.ancestor_class_pattern)
                                    .hint_text("例如 REAPER*Mainwnd，留空表示任意"),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                    });
                    ui.end_row();
                });
            });

        });

        if save_clicked {
            self.publish(&cfg);
            self.draft = None;
            self.editing = false;
        } else if changed {
            if self.editing {
                self.draft = Some(cfg.clone());
            } else {
                self.publish(&cfg);
            }
        }
    }
}

fn read_hotkey_combo(ctx: &egui::Context) -> Option<Option<String>> {
    let mut captured = None;
    ctx.input(|input| {
        for event in &input.events {
            if let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            {
                if *key == egui::Key::Escape {
                    captured = Some(None);
                    break;
                }
                if is_modifier_key(*key) {
                    continue;
                }
                if let Some(name) = egui_key_name(*key) {
                    let mut parts = Vec::new();
                    if modifiers.ctrl {
                        parts.push("Ctrl".to_string());
                    }
                    if modifiers.alt {
                        parts.push("Alt".to_string());
                    }
                    if modifiers.shift {
                        parts.push("Shift".to_string());
                    }
                    parts.push(name);
                    captured = Some(Some(parts.join("+")));
                    break;
                }
            }
        }
    });
    captured
}

fn is_modifier_key(key: egui::Key) -> bool {
    matches!(
        key,
        egui::Key::ShiftLeft
            | egui::Key::ShiftRight
            | egui::Key::ControlLeft
            | egui::Key::ControlRight
            | egui::Key::AltLeft
            | egui::Key::AltRight
            | egui::Key::SuperLeft
            | egui::Key::SuperRight
    )
}

fn egui_key_name(key: egui::Key) -> Option<String> {
    let name = match key {
        egui::Key::A => "A",
        egui::Key::B => "B",
        egui::Key::C => "C",
        egui::Key::D => "D",
        egui::Key::E => "E",
        egui::Key::F => "F",
        egui::Key::G => "G",
        egui::Key::H => "H",
        egui::Key::I => "I",
        egui::Key::J => "J",
        egui::Key::K => "K",
        egui::Key::L => "L",
        egui::Key::M => "M",
        egui::Key::N => "N",
        egui::Key::O => "O",
        egui::Key::P => "P",
        egui::Key::Q => "Q",
        egui::Key::R => "R",
        egui::Key::S => "S",
        egui::Key::T => "T",
        egui::Key::U => "U",
        egui::Key::V => "V",
        egui::Key::W => "W",
        egui::Key::X => "X",
        egui::Key::Y => "Y",
        egui::Key::Z => "Z",
        egui::Key::Num0 => "0",
        egui::Key::Num1 => "1",
        egui::Key::Num2 => "2",
        egui::Key::Num3 => "3",
        egui::Key::Num4 => "4",
        egui::Key::Num5 => "5",
        egui::Key::Num6 => "6",
        egui::Key::Num7 => "7",
        egui::Key::Num8 => "8",
        egui::Key::Num9 => "9",
        egui::Key::Space => "Space",
        egui::Key::Tab => "Tab",
        egui::Key::Enter => "Enter",
        egui::Key::Backspace => "Backspace",
        egui::Key::Minus => "-",
        egui::Key::Equals => "=",
        egui::Key::Comma => ",",
        egui::Key::Period => ".",
        egui::Key::Slash => "/",
        egui::Key::Backslash => "\\",
        egui::Key::Semicolon => ";",
        egui::Key::Quote => "'",
        egui::Key::OpenBracket => "[",
        egui::Key::CloseBracket => "]",
        egui::Key::Backtick => "`",
        egui::Key::F1 => "F1",
        egui::Key::F2 => "F2",
        egui::Key::F3 => "F3",
        egui::Key::F4 => "F4",
        egui::Key::F5 => "F5",
        egui::Key::F6 => "F6",
        egui::Key::F7 => "F7",
        egui::Key::F8 => "F8",
        egui::Key::F9 => "F9",
        egui::Key::F10 => "F10",
        egui::Key::F11 => "F11",
        egui::Key::F12 => "F12",
        _ => return None,
    };
    Some(name.to_string())
}

fn smart_control_pattern(text: &str) -> String {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix('在') {
        if rest.ends_with("中搜索") {
            return "在*中搜索".to_string();
        }
    }
    text.to_string()
}

fn install_chinese_font(ctx: &egui::Context) {
    let font_dir = std::path::Path::new("C:\\Windows\\Fonts");
    let candidates: &[(&str, u32)] = &[
        ("msyh.ttc", 0),
        ("msyh.ttf", 0),
        ("simhei.ttf", 0),
        ("Deng.ttf", 0),
        ("simsun.ttc", 0),
        ("simkai.ttf", 0),
    ];

    let Some((bytes, index)) = candidates.iter().find_map(|(name, index)| {
        let path = font_dir.join(name);
        std::fs::read(&path).ok().map(|bytes| (bytes, *index))
    }) else {
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    let mut data = egui::FontData::from_owned(bytes);
    data.index = index;
    fonts.font_data.insert("cjk".to_owned(), std::sync::Arc::new(data));

    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("cjk".to_owned());
    }

    ctx.set_fonts(fonts);
}

fn about_pos_path() -> std::path::PathBuf {
    config_path().with_file_name("about_pos.json")
}

pub fn load_about_pos() -> Option<(f32, f32)> {
    let text = std::fs::read_to_string(about_pos_path()).ok()?;
    let arr: [f32; 2] = serde_json::from_str(&text).ok()?;
    Some((arr[0], arr[1]))
}

fn save_about_pos(ctx: &egui::Context) {
    if let Some(rect) = ctx.input(|i| i.viewport().outer_rect) {
        if let Ok(text) = serde_json::to_string(&[rect.min.x, rect.min.y]) {
            let _ = std::fs::write(about_pos_path(), text);
        }
    }
}

pub struct AboutApp;

impl AboutApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_chinese_font(&cc.egui_ctx);
        Self
    }
}

impl eframe::App for AboutApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) {
            save_about_pos(ctx);
            std::process::exit(0);
        }
        if win32::should_quit() {
            save_about_pos(ctx);
            std::process::exit(0);
        }
        if win32::should_about_focus() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(10.0);
            ui.label("Auto IME - 输入法自动切换");
            ui.separator();
            ui.label("版本号：1.0");
            ui.label(format!("日期：{}", win32::today_string()));
            ui.label("署名：阿波");
            ui.add_space(10.0);
            ui.separator();
            ui.label("实时鼠标点击参数：");
            if let Some(info) = win32::observed_info() {
                ui.monospace(info.summary());
            } else {
                ui.label("暂无点击数据");
            }
            ui.add_space(10.0);
            if ui.button("关闭").clicked() {
                save_about_pos(ui.ctx());
                std::process::exit(0);
            }
        });
    }
}
