use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchTarget {
    ActiveWindow,
    ActiveControl,
    MouseWindow,
    MouseControl,
}

impl MatchTarget {
    pub const ALL: [MatchTarget; 1] = [MatchTarget::ActiveControl];

    pub fn label(self) -> &'static str {
        match self {
            MatchTarget::ActiveWindow => "当前激活窗口",
            MatchTarget::ActiveControl => "当前焦点控件",
            MatchTarget::MouseWindow => "当前激活窗口",
            MatchTarget::MouseControl => "当前焦点控件",
        }
    }

    pub fn to_active(self) -> Self {
        match self {
            MatchTarget::ActiveWindow
            | MatchTarget::MouseWindow
            | MatchTarget::MouseControl => MatchTarget::ActiveControl,
            other => other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Chinese,
    English,
}

impl Action {
    pub const ALL: [Action; 2] = [Action::Chinese, Action::English];

    pub fn label(self) -> &'static str {
        match self {
            Action::Chinese => "中文",
            Action::English => "英文",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwitchMethod {
    Simulate,
    Ime,
}

impl SwitchMethod {
    pub const ALL: [SwitchMethod; 2] = [SwitchMethod::Simulate, SwitchMethod::Ime];

    pub fn label(self) -> &'static str {
        match self {
            SwitchMethod::Simulate => "模拟按键（Ctrl+空格）",
            SwitchMethod::Ime => "IME底层切换",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchMode {
    Wildcard,
    Regex,
}

impl MatchMode {
    pub const ALL: [MatchMode; 2] = [MatchMode::Wildcard, MatchMode::Regex];

    pub fn label(self) -> &'static str {
        match self {
            MatchMode::Wildcard => "通配符",
            MatchMode::Regex => "正则表达式",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WindowInfo {
    pub process_name: String,
    pub process_path: String,
    pub window_title: String,
    pub window_class: String,
    pub control_text: String,
    pub control_class: String,
    pub control_type: String,
    pub automation_id: String,
    pub container_text: String,
    #[serde(default)]
    pub ancestor_texts: Vec<String>,
    #[serde(default)]
    pub ancestor_classes: Vec<String>,
    pub window_hwnd: usize,
    pub control_hwnd: usize,
    pub click_x: i32,
    pub click_y: i32,
}

impl WindowInfo {
    pub fn summary(&self) -> String {
        let ancestors = if self.ancestor_texts.is_empty() {
            String::new()
        } else {
            format!("\n父层链: {}", self.ancestor_texts.join(" / "))
        };
        format!(
            "进程: {}\n窗口标题: {}\n窗口类: {}\n控件文本: {}\n控件类: {}\n控件类型: {}\n自动化ID: {}\n父级标签: {}{}\nEXE: {}",
            self.process_name,
            self.window_title,
            self.window_class,
            self.control_text,
            self.control_class,
            self.control_type,
            self.automation_id,
            self.container_text,
            ancestors,
            self.process_path
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: u32,
    pub match_target: MatchTarget,
    pub match_mode: MatchMode,
    #[serde(default = "default_true")]
    pub use_process: bool,
    #[serde(default = "default_true")]
    pub use_window_title: bool,
    #[serde(default = "default_true")]
    pub use_window_class: bool,
    #[serde(default = "default_true")]
    pub use_control_text: bool,
    #[serde(default = "default_true")]
    pub use_control_class: bool,
    #[serde(default = "default_true")]
    pub use_control_type: bool,
    #[serde(default = "default_true")]
    pub use_automation_id: bool,
    #[serde(default)]
    pub use_container_text: bool,
    #[serde(default)]
    pub use_ancestor_class: bool,
    pub process_pattern: String,
    pub window_title_pattern: String,
    pub window_class_pattern: String,
    pub control_text_pattern: String,
    pub control_class_pattern: String,
    #[serde(default)]
    pub control_type_pattern: String,
    #[serde(default)]
    pub automation_id_pattern: String,
    #[serde(default)]
    pub container_text_pattern: String,
    #[serde(default)]
    pub ancestor_class_pattern: String,
    pub action: Action,
}

fn default_true() -> bool {
    true
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            id: new_id(),
            name: "新规则".to_string(),
            enabled: true,
            priority: 100,
            match_target: MatchTarget::ActiveControl,
            match_mode: MatchMode::Wildcard,
            use_process: true,
            use_window_title: false,
            use_window_class: false,
            use_control_text: false,
            use_control_class: false,
            use_control_type: false,
            use_automation_id: false,
            use_container_text: false,
            use_ancestor_class: false,
            process_pattern: String::new(),
            window_title_pattern: String::new(),
            window_class_pattern: String::new(),
            control_text_pattern: String::new(),
            control_class_pattern: String::new(),
            control_type_pattern: String::new(),
            automation_id_pattern: String::new(),
            container_text_pattern: String::new(),
            ancestor_class_pattern: String::new(),
            action: Action::English,
        }
    }
}

impl Rule {
    pub fn is_process_only(&self) -> bool {
        self.use_process
            && !self.use_window_title
            && !self.use_window_class
            && !self.use_control_text
            && !self.use_control_class
            && !self.use_control_type
            && !self.use_automation_id
            && !self.use_container_text
    }

    pub fn matches_process(&self, process: &str) -> bool {
        self.field_matches(&self.process_pattern, process)
    }

    pub fn matches(&self, window: &WindowInfo, control: &WindowInfo) -> bool {
        let (win, ctrl) = match self.match_target {
            MatchTarget::ActiveWindow | MatchTarget::MouseWindow => (window, None),
            MatchTarget::ActiveControl | MatchTarget::MouseControl => (window, Some(control)),
        };

        let mut conditions: Vec<bool> = Vec::new();
        if self.use_process {
            self.push_condition(&mut conditions, &self.process_pattern, &win.process_name);
        }
        if self.use_window_title {
            self.push_condition(&mut conditions, &self.window_title_pattern, &win.window_title);
        }
        if self.use_window_class {
            self.push_condition(&mut conditions, &self.window_class_pattern, &win.window_class);
        }

        if let Some(ctrl) = ctrl {
            if self.use_control_text {
                self.push_condition(&mut conditions, &self.control_text_pattern, &ctrl.control_text);
            }
            if self.use_control_class {
                self.push_condition(&mut conditions, &self.control_class_pattern, &ctrl.control_class);
            }
            if self.use_control_type {
                self.push_condition(&mut conditions, &self.control_type_pattern, &ctrl.control_type);
            }
            if self.use_automation_id {
                self.push_condition(&mut conditions, &self.automation_id_pattern, &ctrl.automation_id);
            }
            if self.use_container_text {
                let pattern = self.container_text_pattern.trim();
                if !pattern.is_empty() {
                    let matched = ctrl
                        .ancestor_texts
                        .iter()
                        .any(|text| self.field_matches(pattern, text));
                    conditions.push(matched);
                }
            }
            if self.use_ancestor_class {
                let pattern = self.ancestor_class_pattern.trim();
                if !pattern.is_empty() {
                    let matched = ctrl
                        .ancestor_classes
                        .iter()
                        .any(|class| self.field_matches(pattern, class));
                    conditions.push(matched);
                }
            }
        }

        conditions.iter().all(|matched| *matched)
    }

    fn push_condition(&self, conditions: &mut Vec<bool>, pattern: &str, value: &str) {
        if pattern.trim().is_empty() {
            return;
        }
        conditions.push(self.field_matches(pattern, value));
    }

    fn field_matches(&self, pattern: &str, value: &str) -> bool {
        let pattern = pattern.trim();
        let value = value.trim();
        if pattern.is_empty() {
            return true;
        }

        match self.match_mode {
            MatchMode::Wildcard => wildcard_match(pattern, value),
            MatchMode::Regex => match Regex::new(&format!("(?i:{pattern})")) {
                Ok(re) => re.is_match(value),
                Err(_) => value.to_lowercase() == pattern.to_lowercase(),
            },
        }
    }

    pub fn label(&self) -> String {
        format!(
            "[{}] {} => {}",
            self.match_target.label(),
            self.name,
            self.action.label()
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_ime_toggle_hotkey")]
    pub ime_toggle_hotkey: String,
    #[serde(default)]
    pub window_pos: Option<(f32, f32)>,
    #[serde(default = "default_switch_method")]
    pub switch_method: SwitchMethod,
    pub rules: Vec<Rule>,
}

fn default_ime_toggle_hotkey() -> String {
    "Ctrl+Space".to_string()
}

fn default_switch_method() -> SwitchMethod {
    SwitchMethod::Simulate
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ime_toggle_hotkey: default_ime_toggle_hotkey(),
            window_pos: None,
            switch_method: default_switch_method(),
            rules: Vec::new(),
        }
    }
}

impl AppConfig {
    pub fn migrate(&mut self) {
        for rule in &mut self.rules {
            rule.match_target = rule.match_target.to_active();
        }
    }

    pub fn sorted_rules(&self) -> Vec<Rule> {
        let mut rules = self.rules.clone();
        rules.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| a.name.cmp(&b.name))
        });
        rules
    }

    pub fn active_rules(&self) -> Vec<Rule> {
        self.sorted_rules()
            .into_iter()
            .filter(|rule| rule.enabled)
            .collect()
    }

}

pub fn config_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.to_path_buf()))
        .unwrap_or_else(std::env::temp_dir)
        .join("config.json")
}

pub fn load_config() -> AppConfig {
    let path = config_path();
    match fs::read_to_string(&path) {
        Ok(text) => {
            let mut config =
                serde_json::from_str::<AppConfig>(&text).unwrap_or_default();
            config.migrate();
            config
        }
        Err(_) => AppConfig::default(),
    }
}

pub fn save_config(config: &AppConfig) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| e.to_string())
}

pub fn new_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}")
}

pub fn wildcard_match(pattern: &str, value: &str) -> bool {
    let pattern = pattern.to_lowercase();
    let value = value.to_lowercase();
    let p: Vec<char> = pattern.chars().collect();
    let v: Vec<char> = value.chars().collect();
    let (mut i, mut j) = (0usize, 0usize);
    let (mut star, mut mark) = (usize::MAX, 0usize);

    while j < v.len() {
        if i < p.len() && (p[i] == '?' || p[i] == v[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            star = i;
            mark = j;
            i += 1;
        } else if star != usize::MAX {
            i = star + 1;
            mark += 1;
            j = mark;
        } else {
            return false;
        }
    }

    while i < p.len() && p[i] == '*' {
        i += 1;
    }
    i == p.len()
}
