use crate::model::{config_path, Action, AppConfig, SwitchMethod, WindowInfo};
use std::ffi::c_void;
use std::mem;
use std::ptr;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationElement};

type BOOL = i32;
type DWORD = u32;
type UINT = u32;
type LONG = i32;
type HWND = isize;
type HANDLE = isize;
type HINSTANCE = isize;
type HMODULE = isize;
type LRESULT = isize;
type LPARAM = isize;
type WPARAM = usize;
type WNDPROC = unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT;

const GA_ROOT: UINT = 2;
const CWP_SKIPINVISIBLE: UINT = 0x0001;
const CWP_SKIPDISABLED: UINT = 0x0002;
const CWP_SKIPTRANSPARENT: UINT = 0x0004;
const HWND_MESSAGE: HWND = -3;

const MOD_ALT: UINT = 0x0001;
const MOD_CONTROL: UINT = 0x0002;
const VK_Q: UINT = 0x51;
const WM_HOTKEY: UINT = 0x0312;

const PROCESS_QUERY_LIMITED_INFORMATION: DWORD = 0x1000;
const THREAD_QUERY_LIMITED_INFORMATION: DWORD = 0x0800;
const TH32CS_SNAPPROCESS: DWORD = 0x00000002;
const INVALID_HANDLE_VALUE: HANDLE = -1;
const ERROR_ALREADY_EXISTS: DWORD = 183;
const WAIT_OBJECT_0: DWORD = 0x00000000;

const KEYEVENTF_KEYUP: DWORD = 0x0002;

const VK_SHIFT: u8 = 0x10;
const VK_CONTROL: u8 = 0x11;
const VK_MENU: u8 = 0x12;
const VK_LWIN: u8 = 0x5B;
const VK_SPACE: u8 = 0x20;
const VK_LBUTTON: u8 = 0x01;

const WM_IME_CONTROL: UINT = 0x0283;
const IMC_GETCONVERSIONMODE: WPARAM = 0x0001;
const IMC_GETOPENSTATUS: WPARAM = 0x0005;
const IMC_SETOPENSTATUS: WPARAM = 0x0006;

const HOTKEY_ID_ARM: isize = 0x41_45_4d_49;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct POINT {
    x: LONG,
    y: LONG,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RECT {
    left: LONG,
    top: LONG,
    right: LONG,
    bottom: LONG,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct GUITHREADINFO {
    cbSize: DWORD,
    flags: DWORD,
    hwndActive: HWND,
    hwndFocus: HWND,
    hwndCapture: HWND,
    hwndMenuOwner: HWND,
    hwndMoveSize: HWND,
    hwndCaret: HWND,
    rcCaret: RECT,
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

#[repr(C)]
struct WNDCLASSW {
    style: UINT,
    lpfnWndProc: WNDPROC,
    cbClsExtra: i32,
    cbWndExtra: i32,
    hInstance: HINSTANCE,
    hIcon: HANDLE,
    hCursor: HANDLE,
    hbrBackground: HANDLE,
    lpszMenuName: *const u16,
    lpszClassName: *const u16,
}

#[repr(C)]
struct PROCESSENTRY32W {
    dwSize: DWORD,
    cntUsage: DWORD,
    th32ProcessID: DWORD,
    th32DefaultHeapID: usize,
    th32ModuleID: DWORD,
    cntThreads: DWORD,
    th32ParentProcessID: DWORD,
    pcPriClassBase: LONG,
    dwFlags: DWORD,
    szExeFile: [u16; 260],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SYSTEMTIME {
    wYear: u16,
    wMonth: u16,
    wDayOfWeek: u16,
    wDay: u16,
    wHour: u16,
    wMinute: u16,
    wSecond: u16,
    wMilliseconds: u16,
}

#[link(name = "user32")]
extern "system" {
    fn GetCursorPos(lpPoint: *mut POINT) -> BOOL;
    fn WindowFromPoint(pt: POINT) -> HWND;
    fn GetAncestor(hwnd: HWND, gaFlags: UINT) -> HWND;
    fn GetWindowTextW(hwnd: HWND, lpString: *mut u16, nMaxCount: i32) -> i32;
    fn GetClassNameW(hwnd: HWND, lpClassName: *mut u16, nMaxCount: i32) -> i32;
    fn GetWindowThreadProcessId(hwnd: HWND, lpdwProcessId: *mut DWORD) -> DWORD;
    fn GetWindowRect(hwnd: HWND, lpRect: *mut RECT) -> BOOL;
    fn ScreenToClient(hwnd: HWND, lpPoint: *mut POINT) -> BOOL;
    fn ChildWindowFromPointEx(hwnd: HWND, pt: POINT, flags: UINT) -> HWND;
    fn GetForegroundWindow() -> HWND;
    fn GetGUIThreadInfo(idThread: DWORD, pgui: *mut GUITHREADINFO) -> BOOL;
    fn RegisterClassW(lpWndClass: *const WNDCLASSW) -> u16;
    fn CreateWindowExW(
        dwExStyle: DWORD,
        lpClassName: *const u16,
        lpWindowName: *const u16,
        dwStyle: DWORD,
        x: i32,
        y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: HWND,
        hMenu: isize,
        hInstance: HINSTANCE,
        lpParam: *mut c_void,
    ) -> HWND;
    fn DefWindowProcW(hwnd: HWND, msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    fn GetMessageW(lpMsg: *mut MSG, hwnd: HWND, wMsgFilterMin: UINT, wMsgFilterMax: UINT) -> BOOL;
    fn TranslateMessage(lpMsg: *const MSG) -> BOOL;
    fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
    fn PostQuitMessage(nExitCode: i32);
    fn RegisterHotKey(hwnd: HWND, id: i32, fsModifiers: UINT, vk: UINT) -> BOOL;
    fn SetForegroundWindow(hwnd: HWND) -> BOOL;
    fn ShowWindow(hwnd: HWND, nCmdShow: i32) -> BOOL;
    fn IsWindowVisible(hwnd: HWND) -> BOOL;
    fn keybd_event(bVk: u8, bScan: u8, dwFlags: DWORD, dwExtraInfo: usize);
    fn GetAsyncKeyState(vKey: i32) -> i16;
    fn SendMessageW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
}

#[link(name = "imm32")]
extern "system" {
    fn ImmGetDefaultIMEWnd(hwnd: HWND) -> HWND;
}

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(dwDesiredAccess: DWORD, bInheritHandle: BOOL, dwProcessId: DWORD) -> HANDLE;
    fn CloseHandle(hObject: HANDLE) -> BOOL;
    fn QueryFullProcessImageNameW(
        hProcess: HANDLE,
        dwFlags: DWORD,
        lpExeName: *mut u16,
        lpdwSize: *mut DWORD,
    ) -> BOOL;
    fn CreateToolhelp32Snapshot(dwFlags: DWORD, th32ProcessID: DWORD) -> HANDLE;
    fn Process32FirstW(hSnapshot: HANDLE, lppe: *mut PROCESSENTRY32W) -> BOOL;
    fn Process32NextW(hSnapshot: HANDLE, lppe: *mut PROCESSENTRY32W) -> BOOL;
    fn GetModuleHandleW(lpModuleName: *const u16) -> HMODULE;
    fn CreateMutexW(
        lpMutexAttributes: *mut c_void,
        bInitialOwner: BOOL,
        lpName: *const u16,
    ) -> HANDLE;
    fn GetLastError() -> DWORD;
    fn ExitProcess(uExitCode: UINT);
    fn GetLocalTime(lpSystemTime: *mut SYSTEMTIME);
    fn OpenThread(dwDesiredAccess: DWORD, bInheritHandle: BOOL, dwThreadId: DWORD) -> HANDLE;
    fn CreateEventW(
        lpEventAttributes: *mut c_void,
        bManualReset: BOOL,
        bInitialState: BOOL,
        lpName: *const u16,
    ) -> HANDLE;
    fn SetEvent(hEvent: HANDLE) -> BOOL;
    fn ResetEvent(hEvent: HANDLE) -> BOOL;
    fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: DWORD) -> DWORD;
}

struct UiaControl {
    name: String,
    class: String,
    control_type: String,
    automation_id: String,
    container_text: String,
    ancestor_texts: Vec<String>,
    ancestor_classes: Vec<String>,
}

struct UiaAncestorInfo {
    container_text: String,
    ancestor_texts: Vec<String>,
    ancestor_classes: Vec<String>,
}

fn ensure_com_initialized() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
}

fn uia_automation() -> Option<IUIAutomation> {
    ensure_com_initialized();
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL).ok()? };
    Some(automation)
}

fn uia_control_from_element(element: &IUIAutomationElement) -> Option<UiaControl> {
    unsafe {
        let name = element
            .CurrentName()
            .ok()
            .map(|value| value.to_string())
            .unwrap_or_default();
        let class = element
            .CurrentClassName()
            .ok()
            .map(|value| value.to_string())
            .unwrap_or_default();
        let automation_id = element
            .CurrentAutomationId()
            .ok()
            .map(|value| value.to_string())
            .unwrap_or_default();
        let control_type = element
            .CurrentControlType()
            .ok()
            .map(|id| uia_control_type_name(id.0))
            .unwrap_or_default();

        Some(UiaControl {
            name,
            class,
            control_type,
            automation_id,
            container_text: String::new(),
            ancestor_texts: Vec::new(),
            ancestor_classes: Vec::new(),
        })
    }
}

fn uia_from_point(pt: POINT) -> Option<UiaControl> {
    let automation = uia_automation()?;
    let point = windows::Win32::Foundation::POINT {
        x: pt.x,
        y: pt.y,
    };
    let element = unsafe { automation.ElementFromPoint(point).ok()? };
    let mut control = uia_control_from_element(&element)?;
    let info = uia_ancestor_info(&automation, &element);
    control.container_text = if info.container_text.is_empty() {
        info.ancestor_texts.first().cloned().unwrap_or_default()
    } else {
        info.container_text
    };
    control.ancestor_texts = info.ancestor_texts;
    control.ancestor_classes = info.ancestor_classes;
    Some(control)
}

fn uia_ancestor_info(
    automation: &IUIAutomation,
    element: &IUIAutomationElement,
) -> UiaAncestorInfo {
    let Ok(walker) = (unsafe { automation.ControlViewWalker() }) else {
        return UiaAncestorInfo {
            container_text: String::new(),
            ancestor_texts: Vec::new(),
            ancestor_classes: Vec::new(),
        };
    };
    let mut info = UiaAncestorInfo {
        container_text: String::new(),
        ancestor_texts: Vec::new(),
        ancestor_classes: Vec::new(),
    };
    let mut current = match unsafe { walker.GetParentElement(element) } {
        Ok(parent) => parent,
        Err(_) => return info,
    };
    for index in 0..16 {
        let control_type = unsafe {
            current
                .CurrentControlType()
                .ok()
                .map(|id| id.0)
                .unwrap_or(0)
        };
        let name = unsafe {
            current
                .CurrentName()
                .ok()
                .map(|value| value.to_string())
                .unwrap_or_default()
        };
        let class = unsafe {
            current
                .CurrentClassName()
                .ok()
                .map(|value| value.to_string())
                .unwrap_or_default()
        };
        if capture_armed() {
            debug_log(&format!(
                "UIA_ANCESTOR[{index}] type={control_type} ({}) name={name} class={class}",
                uia_control_type_name(control_type)
            ));
        }
        // 容器/标签类控件：Tab、TabItem、ToolBar、Group、Pane、Window、Custom。
        let is_container = matches!(control_type, 50018 | 50019 | 50021 | 50025 | 50026 | 50032 | 50033);
        if is_container {
            if info.container_text.is_empty() && !name.is_empty() {
                info.container_text = name.clone();
            }
            if !name.is_empty() {
                info.ancestor_texts.push(name);
            }
            if !class.is_empty() {
                info.ancestor_classes.push(class);
            }
        }
        match unsafe { walker.GetParentElement(&current) } {
            Ok(parent) => current = parent,
            Err(_) => break,
        }
    }
    info
}

fn uia_control_type_name(id: i32) -> String {
    let name = match id {
        50000 => "Button",
        50001 => "Calendar",
        50002 => "CheckBox",
        50003 => "ComboBox",
        50004 => "Edit",
        50005 => "Hyperlink",
        50006 => "Image",
        50007 => "ListItem",
        50008 => "List",
        50009 => "Menu",
        50010 => "MenuBar",
        50011 => "MenuItem",
        50012 => "ProgressBar",
        50013 => "RadioButton",
        50014 => "ScrollBar",
        50015 => "Slider",
        50016 => "Spinner",
        50017 => "StatusBar",
        50018 => "Tab",
        50019 => "TabItem",
        50020 => "Text",
        50021 => "ToolBar",
        50022 => "ToolTip",
        50023 => "Tree",
        50024 => "TreeItem",
        50025 => "Custom",
        50026 => "Group",
        50027 => "Thumb",
        50028 => "DataGrid",
        50029 => "DataItem",
        50030 => "Document",
        50031 => "SplitButton",
        50032 => "Window",
        50033 => "Pane",
        50034 => "Header",
        50035 => "HeaderItem",
        50036 => "Table",
        50037 => "TitleBar",
        50038 => "Separator",
        50039 => "SemanticZoom",
        50040 => "AppBar",
        _ => return format!("Unknown({id})"),
    };
    name.to_string()
}

pub fn capture_from_cursor() -> Option<WindowInfo> {
    let mut pt = POINT::default();
    if unsafe { GetCursorPos(&mut pt) } == 0 {
        return None;
    }
    capture_from_point(pt)
}

pub fn is_already_running() -> bool {
    let name: Vec<u16> = "AutoIME_SingleInstance_Mutex\0".encode_utf16().collect();
    unsafe {
        let handle = CreateMutexW(ptr::null_mut(), 0, name.as_ptr());
        if handle == 0 {
            return true;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(handle);
            true
        } else {
            // 句柄由进程持有，进程退出时自动释放。
            false
        }
    }
}

pub fn is_ui_already_running() -> bool {
    let name: Vec<u16> = "AutoIME_UI_SingleInstance_Mutex\0".encode_utf16().collect();
    unsafe {
        let handle = CreateMutexW(ptr::null_mut(), 0, name.as_ptr());
        if handle == 0 {
            return true;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(handle);
            true
        } else {
            false
        }
    }
}

pub fn is_about_already_running() -> bool {
    let name: Vec<u16> = "AutoIME_About_SingleInstance_Mutex\0".encode_utf16().collect();
    unsafe {
        let handle = CreateMutexW(ptr::null_mut(), 0, name.as_ptr());
        if handle == 0 {
            return true;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(handle);
            true
        } else {
            false
        }
    }
}

pub fn wait_for_click(tx: Sender<WindowInfo>, ctx: eframe::egui::Context) {
    // 如果准备动作本身是通过鼠标点击触发的，先等按钮抬起，避免把那次点击当成捕获点击。
    while (unsafe { GetAsyncKeyState(VK_LBUTTON as i32) } as u16 & 0x8000) != 0 {
        std::thread::sleep(Duration::from_millis(10));
    }

    while (unsafe { GetAsyncKeyState(VK_LBUTTON as i32) } as u16 & 0x8000) == 0 {
        std::thread::sleep(Duration::from_millis(10));
    }

    // 和运行时点击保持一致的节奏：等 40ms 后抓当前鼠标位置下的控件。
    std::thread::sleep(Duration::from_millis(40));
    if let Some(info) = capture_from_cursor() {
        let _ = tx.send(info);
        ctx.request_repaint();
    }
}

static CAPTURE_EVENT: OnceLock<HANDLE> = OnceLock::new();

fn capture_event() -> HANDLE {
    *CAPTURE_EVENT.get_or_init(|| {
        let name: Vec<u16> = "AutoIME_CaptureArmed_Event\0".encode_utf16().collect();
        unsafe { CreateEventW(ptr::null_mut(), 1, 0, name.as_ptr()) }
    })
}

pub fn set_capture_armed(armed: bool) {
    let handle = capture_event();
    if handle == 0 {
        return;
    }
    unsafe {
        if armed {
            SetEvent(handle);
        } else {
            ResetEvent(handle);
        }
    }
}

fn capture_armed() -> bool {
    let handle = capture_event();
    if handle == 0 {
        return false;
    }
    unsafe { WaitForSingleObject(handle, 0) == WAIT_OBJECT_0 }
}

static QUIT_EVENT: OnceLock<HANDLE> = OnceLock::new();

fn quit_event() -> HANDLE {
    *QUIT_EVENT.get_or_init(|| {
        let name: Vec<u16> = "AutoIME_Quit_Event\0".encode_utf16().collect();
        unsafe { CreateEventW(ptr::null_mut(), 1, 0, name.as_ptr()) }
    })
}

pub fn create_quit_event() {
    let _ = quit_event();
}

pub fn signal_quit() {
    let handle = quit_event();
    if handle != 0 {
        unsafe {
            SetEvent(handle);
        }
    }
}

pub fn should_quit() -> bool {
    let handle = quit_event();
    if handle == 0 {
        return false;
    }
    unsafe { WaitForSingleObject(handle, 0) == WAIT_OBJECT_0 }
}

static FOCUS_EVENT: OnceLock<HANDLE> = OnceLock::new();

fn focus_event() -> HANDLE {
    *FOCUS_EVENT.get_or_init(|| {
        let name: Vec<u16> = "AutoIME_Focus_Event\0".encode_utf16().collect();
        unsafe { CreateEventW(ptr::null_mut(), 1, 0, name.as_ptr()) }
    })
}

pub fn create_focus_event() {
    let _ = focus_event();
}

pub fn signal_focus() {
    let handle = focus_event();
    if handle != 0 {
        unsafe {
            SetEvent(handle);
        }
    }
}

pub fn should_focus() -> bool {
    let handle = focus_event();
    if handle == 0 {
        return false;
    }
    unsafe {
        if WaitForSingleObject(handle, 0) == WAIT_OBJECT_0 {
            ResetEvent(handle);
            true
        } else {
            false
        }
    }
}

static ABOUT_FOCUS_EVENT: OnceLock<HANDLE> = OnceLock::new();

fn about_focus_event() -> HANDLE {
    *ABOUT_FOCUS_EVENT.get_or_init(|| {
        let name: Vec<u16> = "AutoIME_About_Focus_Event\0".encode_utf16().collect();
        unsafe { CreateEventW(ptr::null_mut(), 1, 0, name.as_ptr()) }
    })
}

pub fn create_about_focus_event() {
    let _ = about_focus_event();
}

pub fn signal_about_focus() {
    let handle = about_focus_event();
    if handle != 0 {
        unsafe {
            SetEvent(handle);
        }
    }
}

pub fn should_about_focus() -> bool {
    let handle = about_focus_event();
    if handle == 0 {
        return false;
    }
    unsafe {
        if WaitForSingleObject(handle, 0) == WAIT_OBJECT_0 {
            ResetEvent(handle);
            true
        } else {
            false
        }
    }
}

pub fn force_exit() -> ! {
    unsafe {
        ExitProcess(0);
    }
    std::process::exit(0)
}

pub fn today_string() -> String {
    let mut st: SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe {
        GetLocalTime(&mut st);
    }
    format!("{:04}-{:02}-{:02}", st.wYear, st.wMonth, st.wDay)
}

fn debug_log(message: &str) {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let path = dir.join("autoime_debug.log");
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                use std::io::Write;
                let _ = writeln!(file, "{message}");
            }
        }
    }
}

static OBSERVED_INFO: OnceLock<Mutex<Option<WindowInfo>>> = OnceLock::new();
static UI_REPAINT: OnceLock<eframe::egui::Context> = OnceLock::new();

pub fn set_ui_repaint(ctx: eframe::egui::Context) {
    let _ = UI_REPAINT.set(ctx);
}

pub fn observed_info() -> Option<WindowInfo> {
    if let Some(info) = OBSERVED_INFO
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
    {
        return Some(info);
    }
    // 跨进程：从后台守护进程写入的文件读取。
    std::fs::read_to_string(observed_file())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
}

fn set_observed_info(info: WindowInfo) {
    if let Ok(mut guard) = OBSERVED_INFO.get_or_init(|| Mutex::new(None)).lock() {
        *guard = Some(info.clone());
    }
    if let Ok(text) = serde_json::to_string(&info) {
        let _ = std::fs::write(observed_file(), text);
    }
    if let Some(ctx) = UI_REPAINT.get() {
        ctx.request_repaint();
    }
}

fn observed_file() -> std::path::PathBuf {
    config_path().with_file_name("observed.json")
}

#[derive(Clone, Copy, Default)]
struct ThreadIme {
    current: Option<bool>,
    baseline: Option<bool>,
    overridden: bool,
}

fn thread_ime_states() -> &'static Mutex<HashMap<u32, ThreadIme>> {
    static STATES: OnceLock<Mutex<HashMap<u32, ThreadIme>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_thread_ime(thread_id: u32) -> ThreadIme {
    thread_ime_states()
        .lock()
        .ok()
        .and_then(|states| states.get(&thread_id).copied())
        .unwrap_or_default()
}

fn set_baseline_ime(thread_id: u32, open: bool) {
    if let Ok(mut states) = thread_ime_states().lock() {
        states.entry(thread_id).or_default().baseline = Some(open);
    }
}

fn set_current_ime(thread_id: u32, open: bool) {
    if let Ok(mut states) = thread_ime_states().lock() {
        states.entry(thread_id).or_default().current = Some(open);
    }
}

fn set_overridden(thread_id: u32, overridden: bool) {
    if let Ok(mut states) = thread_ime_states().lock() {
        states.entry(thread_id).or_default().overridden = overridden;
    }
}

fn thread_exists(thread_id: u32) -> bool {
    let handle = unsafe { OpenThread(THREAD_QUERY_LIMITED_INFORMATION, 0, thread_id) };
    if handle == 0 {
        false
    } else {
        unsafe {
            CloseHandle(handle);
        }
        true
    }
}

fn prune_dead_threads() {
    if let Ok(mut states) = thread_ime_states().lock() {
        let dead: Vec<u32> = states
            .keys()
            .filter(|&&thread_id| !thread_exists(thread_id))
            .copied()
            .collect();
        for thread_id in dead {
            states.remove(&thread_id);
        }
    }
}

fn read_ime_chinese(hwnd: HWND) -> Option<bool> {
    unsafe {
        let ime_wnd = ImmGetDefaultIMEWnd(hwnd);
        if ime_wnd == 0 {
            debug_log(&format!("READ_IME hwnd={hwnd} ime_wnd=0"));
            return None;
        }
        let conv = SendMessageW(ime_wnd, WM_IME_CONTROL, IMC_GETCONVERSIONMODE, 0);
        let open = SendMessageW(ime_wnd, WM_IME_CONTROL, IMC_GETOPENSTATUS, 0);
        debug_log(&format!(
            "READ_IME hwnd={hwnd} ime_wnd={ime_wnd} conv={conv} open={open} chinese={}",
            open != 0
        ));
        Some(open != 0)
    }
}

fn set_ime_chinese(hwnd: HWND, chinese: bool) -> bool {
    unsafe {
        let ime_wnd = ImmGetDefaultIMEWnd(hwnd);
        if ime_wnd == 0 {
            return false;
        }
        let lparam: LPARAM = if chinese { 1 } else { 0 };
        let _ = SendMessageW(ime_wnd, WM_IME_CONTROL, IMC_SETOPENSTATUS, lparam);
        true
    }
}

pub fn run_monitor(config: Arc<Mutex<AppConfig>>) {
    let own_name = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()))
        .unwrap_or_else(|| "auto-ime.exe".to_string());

    let mut prev_lbtn_down = false;
    let mut last_prune = Instant::now();

    loop {
        std::thread::sleep(Duration::from_millis(20));

        if last_prune.elapsed() >= Duration::from_secs(5) {
            last_prune = Instant::now();
            prune_dead_threads();
        }

        let lbtn_down = (unsafe { GetAsyncKeyState(VK_LBUTTON as i32) } as u16 & 0x8000) != 0;
        let clicked = lbtn_down && !prev_lbtn_down;
        prev_lbtn_down = lbtn_down;

        if !clicked || capture_armed() {
            continue;
        }

        let cfg = match config.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => continue,
        };

        // 点击后稍等前台窗口切换，再抓取点击位置下的控件用于规则匹配。
        std::thread::sleep(Duration::from_millis(40));
        let Some(active) = capture_from_cursor() else {
            continue;
        };
        if active.process_name.to_lowercase() == own_name {
            continue;
        }
        set_observed_info(active.clone());

        let mut matched: Option<(String, Action)> = None;

        for rule in cfg.active_rules() {
            if rule.matches(&active, &active) {
                matched = Some((rule.id.clone(), rule.action));
                break;
            }
        }

        if unsafe { GetForegroundWindow() } != active.window_hwnd as HWND {
            continue;
        }

        // IME 状态以真正拥有键盘焦点的控件为准，避免下拉菜单/遮罩等临时窗口干扰。
        let focus_hwnd = focused_hwnd().unwrap_or(active.control_hwnd as HWND);

        let mut pid: DWORD = 0;
        let thread_id = unsafe { GetWindowThreadProcessId(focus_hwnd, &mut pid) };
        if thread_id == 0 {
            continue;
        }

        let read_chinese = read_ime_chinese(focus_hwnd);

        match matched {
            Some((_id, action)) => {
                let desired_chinese = matches!(action, Action::Chinese);
                let state = get_thread_ime(thread_id);
                let actual = read_chinese
                    .or(state.current)
                    .unwrap_or(cfg.default_ime_chinese);

                if !state.overridden {
                    set_baseline_ime(thread_id, actual);
                }

                let need_toggle = actual != desired_chinese;
                debug_log(&format!(
                    "SWITCH rule={_id} desired={desired_chinese} thread={thread_id} actual={actual} read={read_chinese:?} baseline={:?} need={need_toggle} proc={}",
                    state.baseline,
                    active.process_name
                ));
                if need_toggle {
                    apply_switch(&cfg, focus_hwnd, desired_chinese);
                    set_current_ime(thread_id, desired_chinese);
                } else if read_chinese.is_some() {
                    set_current_ime(thread_id, actual);
                }
                set_overridden(thread_id, true);
            }
            None => {
                let state = get_thread_ime(thread_id);
                if state.overridden {
                    let actual = read_chinese
                        .or(state.current)
                        .unwrap_or(cfg.default_ime_chinese);
                    debug_log(&format!(
                        "RESTORE thread={thread_id} actual={actual} baseline={:?} read={read_chinese:?} proc={}",
                        state.baseline, active.process_name
                    ));
                    if let Some(baseline) = state.baseline {
                        if actual != baseline {
                            apply_switch(&cfg, focus_hwnd, baseline);
                            set_current_ime(thread_id, baseline);
                        }
                    }
                    set_overridden(thread_id, false);
                } else if let Some(actual) = read_chinese {
                    // 未命中规则且未覆盖：持续把真实状态同步为基线，捕获用户手动切换。
                    set_baseline_ime(thread_id, actual);
                    set_current_ime(thread_id, actual);
                }
            }
        }
    }
}

fn apply_switch(cfg: &AppConfig, hwnd: HWND, chinese: bool) {
    match cfg.switch_method {
        SwitchMethod::Simulate => {
            let _ = simulate_combo(&cfg.ime_toggle_hotkey);
        }
        SwitchMethod::Ime => {
            let _ = set_ime_chinese(hwnd, chinese);
        }
    }
}

fn simulate_combo(combo: &str) -> Result<(), String> {
    let (modifiers, key) =
        parse_combo(combo).ok_or_else(|| format!("无法解析快捷键：{combo}"))?;

    unsafe {
        for &modifier in &modifiers {
            keybd_event(modifier, 0, 0, 0);
        }
        keybd_event(key, 0, 0, 0);
        std::thread::sleep(Duration::from_millis(30));
        keybd_event(key, 0, KEYEVENTF_KEYUP, 0);
        for &modifier in modifiers.iter().rev() {
            keybd_event(modifier, 0, KEYEVENTF_KEYUP, 0);
        }
    }

    Ok(())
}

fn parse_combo(combo: &str) -> Option<(Vec<u8>, u8)> {
    let mut modifiers = Vec::new();
    let mut key = None;

    for part in combo.split('+').map(str::trim) {
        if part.is_empty() {
            continue;
        }
        match part.to_ascii_uppercase().as_str() {
            "CTRL" | "CONTROL" => modifiers.push(VK_CONTROL),
            "ALT" => modifiers.push(VK_MENU),
            "SHIFT" => modifiers.push(VK_SHIFT),
            "WIN" | "WINDOWS" | "SUPER" => modifiers.push(VK_LWIN),
            other => {
                if key.is_some() {
                    return None;
                }
                key = Some(key_to_vk(other)?);
            }
        }
    }

    let key = key?;
    modifiers.sort_unstable();
    modifiers.dedup();
    Some((modifiers, key))
}

fn key_to_vk(token: &str) -> Option<u8> {
    let upper = token.to_ascii_uppercase();
    let vk = match upper.as_str() {
        "SPACE" => VK_SPACE,
        "TAB" => 0x09,
        "ENTER" | "RETURN" => 0x0D,
        "ESC" | "ESCAPE" => 0x1B,
        "BACKSPACE" => 0x08,
        "`" | "~" => 0xC0,
        "-" => 0xBD,
        "=" => 0xBB,
        "[" => 0xDB,
        "]" => 0xDD,
        "\\" => 0xDC,
        ";" => 0xBA,
        "'" => 0xDE,
        "," => 0xBC,
        "." => 0xBE,
        "/" => 0xBF,
        _ => {
            if upper.len() == 1 {
                let ch = upper.as_bytes()[0];
                if ch.is_ascii_alphanumeric() {
                    return Some(ch);
                }
            }
            if upper.starts_with('F') && upper.len() >= 2 && upper.len() <= 3 {
                if let Ok(number) = upper[1..].parse::<u8>() {
                    if (1..=24).contains(&number) {
                        return Some(0x6F + number);
                    }
                }
            }
            return None;
        }
    };
    Some(vk)
}

unsafe extern "system" fn hotkey_wnd_proc(
    _hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    _lparam: LPARAM,
) -> LRESULT {
    if msg == WM_HOTKEY {
        if wparam == HOTKEY_ID_ARM as WPARAM {
            let _ = HOTKEY_SENDER
                .get()
                .and_then(|tx| tx.send(HotkeyEvent::CaptureArm).ok());
        }
        if let Some(ctx) = HOTKEY_REPAINT.get() {
            ctx.request_repaint();
        }
    }
    unsafe { DefWindowProcW(_hwnd, msg, wparam, _lparam) }
}

pub enum HotkeyEvent {
    CaptureArm,
}

static HOTKEY_SENDER: OnceLock<Sender<HotkeyEvent>> = OnceLock::new();
static HOTKEY_REPAINT: OnceLock<eframe::egui::Context> = OnceLock::new();

pub fn run_hotkey_listener(tx: Sender<HotkeyEvent>, ctx: eframe::egui::Context) -> Result<(), String> {
    let _ = HOTKEY_SENDER.set(tx);
    let _ = HOTKEY_REPAINT.set(ctx);
    run_hotkey_listener_inner()
}

fn run_hotkey_listener_inner() -> Result<(), String> {
    let class_name: Vec<u16> = "AutoImeHotkeyWindow\0".encode_utf16().collect();
    let hinstance = unsafe { GetModuleHandleW(ptr::null()) };

    let wnd_class = WNDCLASSW {
        style: 0,
        lpfnWndProc: hotkey_wnd_proc,
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: 0,
        hCursor: 0,
        hbrBackground: 0,
        lpszMenuName: ptr::null(),
        lpszClassName: class_name.as_ptr(),
    };

    if unsafe { RegisterClassW(&wnd_class) } == 0 {
        return Err("注册热键窗口类失败".to_string());
    }

    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            ptr::null(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            0,
            hinstance,
            ptr::null_mut(),
        )
    };
    if hwnd == 0 {
        return Err("创建热键窗口失败".to_string());
    }

    if unsafe { RegisterHotKey(hwnd, HOTKEY_ID_ARM as i32, MOD_CONTROL | MOD_ALT, VK_Q) } == 0 {
        return Err("注册 Ctrl+Alt+Q 热键失败".to_string());
    }

    unsafe {
        let mut msg: MSG = mem::zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}

fn capture_from_point(pt: POINT) -> Option<WindowInfo> {
    let hwnd = unsafe { WindowFromPoint(pt) };
    if hwnd == 0 {
        return None;
    }
    let mut root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    if root == 0 {
        root = hwnd;
    }
    let control = deepest_child_from_point(root, pt, 0);
    let mut info = build_info(root, control);
    info.click_x = pt.x;
    info.click_y = pt.y;
    if let Some(uia) = uia_from_point(pt) {
        merge_uia_control(&mut info, &uia);
    }
    if capture_armed() {
        debug_log(&format!("CAPTURE {}", info.summary()));
    }
    Some(info)
}

fn focused_hwnd() -> Option<HWND> {
    let foreground = unsafe { GetForegroundWindow() };
    if foreground == 0 {
        return None;
    }

    let mut pid: DWORD = 0;
    let thread_id = unsafe { GetWindowThreadProcessId(foreground, &mut pid) };
    if thread_id == 0 {
        return None;
    }

    let mut gui: GUITHREADINFO = unsafe { mem::zeroed() };
    gui.cbSize = mem::size_of::<GUITHREADINFO>() as DWORD;
    if unsafe { GetGUIThreadInfo(thread_id, &mut gui) } == 0 {
        return None;
    }

    Some(if gui.hwndFocus != 0 {
        gui.hwndFocus
    } else {
        foreground
    })
}

fn merge_uia_control(info: &mut WindowInfo, uia: &UiaControl) {
    // 容器类（Pane/Window/Group 等）常因 UIA 命中桌面或外层容器，反而覆盖掉
    // Win32 层正确抓到的控件文本/类名，因此这些情况下保留 Win32 结果。
    let is_container = matches!(
        uia.control_type.as_str(),
        "Pane"
            | "Window"
            | "Group"
            | "Tab"
            | "TabItem"
            | "ToolBar"
            | "MenuBar"
            | "Menu"
            | "StatusBar"
            | "TitleBar"
            | "Header"
            | "ToolTip"
    );
    if !is_container {
        if !uia.name.is_empty() {
            info.control_text = uia.name.clone();
        }
        if !uia.class.is_empty() {
            info.control_class = uia.class.clone();
        }
    }
    info.control_type = uia.control_type.clone();
    info.automation_id = uia.automation_id.clone();
    info.container_text = uia.container_text.clone();
    info.ancestor_texts = uia.ancestor_texts.clone();
    info.ancestor_classes = uia.ancestor_classes.clone();
}

fn deepest_child_from_point(root: HWND, pt: POINT, depth: u32) -> HWND {
    if depth > 48 {
        return root;
    }
    let mut client = pt;
    if unsafe { ScreenToClient(root, &mut client) } == 0 {
        return root;
    }
    let child = unsafe {
        ChildWindowFromPointEx(
            root,
            client,
            CWP_SKIPINVISIBLE | CWP_SKIPDISABLED | CWP_SKIPTRANSPARENT,
        )
    };
    if child != 0 && child != root {
        deepest_child_from_point(child, pt, depth + 1)
    } else {
        root
    }
}

fn build_info(window_hwnd: HWND, control_hwnd: HWND) -> WindowInfo {
    let pid = window_process_id(window_hwnd);
    WindowInfo {
        process_name: process_name(pid),
        process_path: process_path(pid),
        window_title: window_text(window_hwnd),
        window_class: class_name(window_hwnd),
        control_text: window_text(control_hwnd),
        control_class: class_name(control_hwnd),
        control_type: String::new(),
        automation_id: String::new(),
        container_text: String::new(),
        ancestor_texts: Vec::new(),
        ancestor_classes: Vec::new(),
        window_hwnd: window_hwnd as usize,
        control_hwnd: control_hwnd as usize,
        click_x: 0,
        click_y: 0,
    }
}

fn window_process_id(hwnd: HWND) -> DWORD {
    let mut pid: DWORD = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
    }
    pid
}

fn window_text(hwnd: HWND) -> String {
    let mut buf = [0u16; 1024];
    let len = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    if len <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buf[..len as usize])
    }
}

fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    let len = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    if len <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buf[..len as usize])
    }
}

fn process_name(pid: DWORD) -> String {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == 0 || snapshot == INVALID_HANDLE_VALUE {
        return String::new();
    }

    let mut entry: PROCESSENTRY32W = unsafe { mem::zeroed() };
    entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as DWORD;

    let mut found = String::new();
    if unsafe { Process32FirstW(snapshot, &mut entry) } != 0 {
        loop {
            if entry.th32ProcessID == pid {
                found = String::from_utf16_lossy(&entry.szExeFile)
                    .trim_end_matches('\0')
                    .to_string();
                break;
            }
            if unsafe { Process32NextW(snapshot, &mut entry) } == 0 {
                break;
            }
        }
    }
    unsafe {
        CloseHandle(snapshot);
    }
    found
}

fn process_path(pid: DWORD) -> String {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle != 0 {
        let mut buf = [0u16; 2048];
        let mut size = buf.len() as DWORD;
        let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut size) };
        unsafe {
            CloseHandle(handle);
        }
        if ok != 0 && size > 0 {
            return String::from_utf16_lossy(&buf[..size as usize]);
        }
    }
    process_name(pid)
}
