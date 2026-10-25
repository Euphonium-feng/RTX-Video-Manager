#![windows_subsystem = "windows"]

use rtx_hdr_switch::{
    format_hotkey, is_verified_nvcpl, parse_hdr_info, toggle_with_transport, AppError, HdrSnapshot,
    Hotkey, NvTransport, Settings, ToggleOutcome, HDR_INFO_SIZE, HDR_SETTING_ID, MOD_ALT,
    MOD_CONTROL, MOD_SHIFT, MOD_WIN, NVCPL_RELATIVE_PATH, NVCPL_SHA256_HEX,
};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::mem::{size_of, zeroed};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use windows_sys::core::{GUID, PCWSTR};
use windows_sys::Win32::Foundation::{
    CloseHandle, FreeLibrary, GetLastError, SetLastError, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM,
};
use windows_sys::Win32::Graphics::Gdi as gdi;
use windows_sys::Win32::Security::WinTrust::{
    WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_FILE_INFO,
    WTD_CACHE_ONLY_URL_RETRIEVAL, WTD_CHOICE_FILE, WTD_REVOCATION_CHECK_NONE, WTD_REVOKE_NONE,
    WTD_SAFER_FLAG, WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY, WTD_UICONTEXT_EXECUTE,
    WTD_UI_NONE,
};
use windows_sys::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryExW};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ,
};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Controls::BST_CHECKED;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetAsyncKeyState, RegisterHotKey, SetFocus, UnregisterHotKey,
};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_ERROR, NIIF_INFO, NIM_ADD,
    NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging as wm;
use windows_sys::Win32::UI::WindowsAndMessaging::{HICON, HMENU};

const APP_NAME: &str = "RTX HDR 开关";
const MAIN_CLASS: &str = "RtxHdrSwitch.MainWindow.v1";
const SETTINGS_CLASS: &str = "RtxHdrSwitch.SettingsWindow.v1";
const MUTEX_NAME: &str = r"Local\RtxHdrSwitch.SingleInstance.v1";
const OPEN_SETTINGS_MESSAGE: &str = "RtxHdrSwitch.OpenSettings.v1";
const TASKBAR_CREATED_MESSAGE: &str = "TaskbarCreated";
const SETTINGS_KEY: &str = r"Software\RTXHDRSwitch";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "RtxHdrSwitch";

const WM_APP_TRAY: u32 = wm::WM_APP + 1;
const WM_APP_WORKER: u32 = wm::WM_APP + 2;
const WM_APP_SETTINGS_CLOSED: u32 = wm::WM_APP + 3;
const WM_NCDESTROY: u32 = 0x0082;
const WM_ACTIVATE: u32 = 0x0006;

const HOTKEY_ID: i32 = 1;
const REFRESH_TIMER_ID: usize = 1;
const RECORD_TIMER_ID: usize = 2;
const TRAY_ICON_ID: u32 = 1;

const CMD_TOGGLE: u32 = 1001;
const CMD_SETTINGS: u32 = 1002;
const CMD_STARTUP: u32 = 1003;
const CMD_NOTIFICATIONS: u32 = 1004;
const CMD_DIAGNOSTICS: u32 = 1005;
const CMD_EXIT: u32 = 1006;

const ID_RECORD: u32 = 2001;
const ID_STARTUP: u32 = 2002;
const ID_NOTIFICATIONS: u32 = 2003;
const ID_SAVE: u32 = 2100;
const ID_CANCEL: u32 = 2101;

const ICON_ON: usize = 101;
const ICON_OFF: usize = 102;
const ICON_ERROR: usize = 103;
const MOD_NOREPEAT: u32 = 0x4000;
const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x0000_0008;

static APP_STATE: OnceLock<Mutex<AppState>> = OnceLock::new();
static PENDING_EVENTS: OnceLock<Mutex<VecDeque<WorkerEvent>>> = OnceLock::new();

#[derive(Clone, Debug)]
enum HdrUi {
    Loading,
    Ready(HdrSnapshot),
    Error(String),
}

#[derive(Clone, Copy)]
struct Icons {
    on: usize,
    off: usize,
    error: usize,
}

impl Icons {
    fn load() -> Self {
        unsafe {
            let instance = GetModuleHandleW(null());
            let fallback = wm::LoadIconW(instance, wm::IDI_APPLICATION);
            Self {
                on: load_icon(instance, ICON_ON, fallback) as usize,
                off: load_icon(instance, ICON_OFF, fallback) as usize,
                error: load_icon(instance, ICON_ERROR, fallback) as usize,
            }
        }
    }

    fn selected(&self, state: &HdrUi) -> HICON {
        let handle = match state {
            HdrUi::Ready(snapshot) if snapshot.supported && snapshot.enabled => self.on,
            HdrUi::Ready(snapshot) if snapshot.supported => self.off,
            HdrUi::Ready(_) => self.off,
            HdrUi::Error(_) => self.error,
            HdrUi::Loading => self.off,
        };
        handle as HICON
    }
}

struct AppState {
    settings: Settings,
    hdr: HdrUi,
    busy: bool,
    hotkey_registered: bool,
    last_toggle: Option<Instant>,
    settings_window: usize,
    worker_tx: Option<Sender<WorkerCommand>>,
    icons: Icons,
    tray_added: bool,
    last_error: Option<String>,
    taskbar_created_message: u32,
    open_settings_message: u32,
}

enum WorkerCommand {
    Read,
    Toggle,
    Shutdown,
}

enum WorkerEvent {
    Read(Result<HdrSnapshot, AppError>),
    Toggle(Result<ToggleOutcome, AppError>),
}

type InitFn = unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32;
type CloseFn = unsafe extern "system" fn() -> i32;
type IsUxdServiceRunningFn = unsafe extern "system" fn(*mut u8) -> i32;
type GetSettingFn = unsafe extern "system" fn(u32, *const u64, u32, *mut c_void, usize) -> i32;
type SetSettingFn = unsafe extern "system" fn(u32, *const u64, u32, *const c_void, usize) -> i32;
type ExecuteFn = unsafe extern "system" fn(u32, u32, i32) -> i32;

struct ComApartment;

impl ComApartment {
    unsafe fn initialize() -> Result<Self, AppError> {
        let result = CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32);
        if result < 0 {
            return Err(AppError::ComInitializationFailed(result));
        }
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

struct NvBackend {
    module: *mut c_void,
    _com: ComApartment,
    close: CloseFn,
    get_setting: GetSettingFn,
    set_setting: SetSettingFn,
    execute: ExecuteFn,
}

impl NvBackend {
    unsafe fn open() -> Result<Self, AppError> {
        let path = nvcpl_path().ok_or(AppError::NvidiaAppMissing)?;
        let bytes = fs::read(&path)
            .map_err(|error| AppError::Io(format!("无法读取 {}：{error}", path.display())))?;
        if !is_verified_nvcpl(&bytes) {
            return Err(AppError::UnsupportedVersion);
        }
        verify_nvidia_signature(&path)?;

        // The private NvCpl API talks to the UXD service through COM on this thread.
        let com = ComApartment::initialize()?;
        let path_wide = wide(&path.to_string_lossy());
        let module = LoadLibraryExW(
            path_wide.as_ptr(),
            null_mut(),
            LOAD_WITH_ALTERED_SEARCH_PATH,
        );
        if module.is_null() {
            return Err(AppError::NvCplLoadFailed(GetLastError()));
        }

        let p_init = GetProcAddress(module, c"NvCplApiInit".as_ptr().cast());
        let p_close = GetProcAddress(module, c"NvCplApiClose".as_ptr().cast());
        let p_is_service_running =
            GetProcAddress(module, c"NvCplApiIsUxdServiceRunning".as_ptr().cast());
        let p_get = GetProcAddress(module, c"NvCplApiGetSetting".as_ptr().cast());
        let p_set = GetProcAddress(module, c"NvCplApiSetSetting".as_ptr().cast());
        let p_execute = GetProcAddress(module, c"NvCplApiExecute".as_ptr().cast());
        if p_init.is_none() {
            FreeLibrary(module);
            return Err(AppError::MissingExport("NvCplApiInit"));
        }
        if p_close.is_none() {
            FreeLibrary(module);
            return Err(AppError::MissingExport("NvCplApiClose"));
        }
        if p_is_service_running.is_none() {
            FreeLibrary(module);
            return Err(AppError::MissingExport("NvCplApiIsUxdServiceRunning"));
        }
        if p_get.is_none() {
            FreeLibrary(module);
            return Err(AppError::MissingExport("NvCplApiGetSetting"));
        }
        if p_set.is_none() {
            FreeLibrary(module);
            return Err(AppError::MissingExport("NvCplApiSetSetting"));
        }
        if p_execute.is_none() {
            FreeLibrary(module);
            return Err(AppError::MissingExport("NvCplApiExecute"));
        }

        let init: InitFn = std::mem::transmute(p_init.unwrap());
        let close: CloseFn = std::mem::transmute(p_close.unwrap());
        let is_service_running: IsUxdServiceRunningFn =
            std::mem::transmute(p_is_service_running.unwrap());
        let get_setting: GetSettingFn = std::mem::transmute(p_get.unwrap());
        let set_setting: SetSettingFn = std::mem::transmute(p_set.unwrap());
        let execute: ExecuteFn = std::mem::transmute(p_execute.unwrap());

        let mut service_running = 0u8;
        let service_code = is_service_running(&mut service_running);
        if service_code != 0 {
            FreeLibrary(module);
            return Err(AppError::ApiFailed(
                "NvCplApiIsUxdServiceRunning",
                service_code,
            ));
        }
        if service_running == 0 {
            FreeLibrary(module);
            return Err(AppError::UxdServiceUnavailable);
        }

        let init_code = init(null_mut(), null_mut());
        if init_code != 0 {
            FreeLibrary(module);
            return Err(AppError::InitFailed(init_code));
        }

        let backend = Self {
            module,
            _com: com,
            close,
            get_setting,
            set_setting,
            execute,
        };
        let raw = backend.read_hdr_bytes()?;
        parse_hdr_info(&raw)?;
        Ok(backend)
    }

    unsafe fn read_hdr_bytes(&self) -> Result<[u8; HDR_INFO_SIZE], AppError> {
        let mut raw = [0u8; HDR_INFO_SIZE];
        let context = 0u64;
        let code = (self.get_setting)(
            1,
            &context,
            HDR_SETTING_ID,
            raw.as_mut_ptr() as *mut c_void,
            raw.len(),
        );
        if code != 0 {
            return Err(AppError::ApiFailed("NvCplApiGetSetting", code));
        }
        Ok(raw)
    }

    unsafe fn write_hdr_bytes(&self, raw: &[u8; HDR_INFO_SIZE]) -> Result<(), AppError> {
        let context = 0u64;
        let code = (self.set_setting)(
            1,
            &context,
            HDR_SETTING_ID,
            raw.as_ptr() as *const c_void,
            raw.len(),
        );
        if code != 0 {
            return Err(AppError::ApiFailed("NvCplApiSetSetting", code));
        }
        // NVIDIA App stages the write and then commits the state transaction.
        let commit_code = (self.execute)(0x10000, 0, -1);
        if commit_code != 0 {
            return Err(AppError::ApiFailed("NvCplApiExecute", commit_code));
        }
        Ok(())
    }

    unsafe fn read_snapshot(&self) -> Result<HdrSnapshot, AppError> {
        let raw = self.read_hdr_bytes()?;
        parse_hdr_info(&raw)
    }
}

impl Drop for NvBackend {
    fn drop(&mut self) {
        unsafe {
            let _ = (self.close)();
            let _ = FreeLibrary(self.module);
        }
    }
}

impl NvTransport for NvBackend {
    fn read_info(&mut self) -> Result<[u8; HDR_INFO_SIZE], AppError> {
        unsafe { self.read_hdr_bytes() }
    }

    fn write_info(&mut self, info: &[u8; HDR_INFO_SIZE]) -> Result<(), AppError> {
        unsafe { self.write_hdr_bytes(info) }
    }
}

struct SettingsWindowState {
    owner: HWND,
    settings: Settings,
    recording: bool,
    last_down: [bool; 256],
    capture_keys: Vec<u32>,
    hotkey_label: HWND,
    record_button: HWND,
    startup_check: HWND,
    notification_check: HWND,
    save_button: HWND,
}

fn main() {
    let exit_code = unsafe { run() };
    std::process::exit(exit_code);
}

unsafe fn run() -> i32 {
    let instance = GetModuleHandleW(null());
    let taskbar_created_message =
        wm::RegisterWindowMessageW(wide(TASKBAR_CREATED_MESSAGE).as_ptr());
    let open_settings_message = wm::RegisterWindowMessageW(wide(OPEN_SETTINGS_MESSAGE).as_ptr());

    let mutex_name = wide(MUTEX_NAME);
    SetLastError(0);
    let mutex = CreateMutexW(null(), 0, mutex_name.as_ptr());
    if mutex.is_null() {
        log_line(&format!("CreateMutexW failed: {}", GetLastError()));
        return 1;
    }
    if GetLastError() == windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS {
        let class_name = wide(MAIN_CLASS);
        let first = wm::FindWindowW(class_name.as_ptr(), null());
        if !first.is_null() {
            wm::PostMessageW(first, open_settings_message, 0, 0);
        }
        CloseHandle(mutex);
        return 0;
    }

    let (settings, first_run) = load_settings();
    let icons = Icons::load();
    let icon_on = icons.on as HICON;
    let cursor = wm::LoadCursorW(null_mut(), wm::IDC_ARROW);
    let class_name = wide(MAIN_CLASS);
    let settings_class_name = wide(SETTINGS_CLASS);

    if !register_window_class(
        &class_name,
        Some(wndproc),
        icon_on,
        cursor,
        null_mut(),
        0,
        instance,
    ) {
        log_line(&format!(
            "RegisterClassExW(main) failed: {}",
            GetLastError()
        ));
        CloseHandle(mutex);
        return 1;
    }

    let settings_background = (gdi::COLOR_WINDOW + 1) as usize as gdi::HBRUSH;
    if !register_window_class(
        &settings_class_name,
        Some(settings_wndproc),
        icon_on,
        cursor,
        settings_background,
        wm::CS_HREDRAW | wm::CS_VREDRAW,
        instance,
    ) {
        log_line(&format!(
            "RegisterClassExW(settings) failed: {}",
            GetLastError()
        ));
        CloseHandle(mutex);
        return 1;
    }

    let _ = PENDING_EVENTS.set(Mutex::new(VecDeque::new()));
    let _ = APP_STATE.set(Mutex::new(AppState {
        settings: settings.clone(),
        hdr: HdrUi::Loading,
        busy: false,
        hotkey_registered: false,
        last_toggle: None,
        settings_window: 0,
        worker_tx: None,
        icons,
        tray_added: false,
        last_error: None,
        taskbar_created_message,
        open_settings_message,
    }));

    let title = wide(APP_NAME);
    let hwnd = wm::CreateWindowExW(
        0,
        class_name.as_ptr(),
        title.as_ptr(),
        wm::WS_OVERLAPPED,
        0,
        0,
        0,
        0,
        null_mut(),
        null_mut(),
        instance,
        null(),
    );
    if hwnd.is_null() {
        log_line(&format!("CreateWindowExW(main) failed: {}", GetLastError()));
        CloseHandle(mutex);
        return 1;
    }

    let mut startup_warning = None;
    if first_run {
        if let Err(error) = save_settings(&settings) {
            startup_warning = Some(format!("无法保存默认设置：{}", error.user_message()));
        }
    }
    if let Err(error) = set_autostart(settings.start_at_login) {
        startup_warning = Some(format!("无法更新开机启动：{}", error.user_message()));
    }

    let registered = register_configured_hotkey(hwnd, settings.hotkey);
    {
        let state = APP_STATE.get().unwrap();
        let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
        state.hotkey_registered = registered.is_ok();
    }

    add_or_update_tray(hwnd, true);
    let (tx, rx) = mpsc::channel();
    {
        let state = APP_STATE.get().unwrap();
        let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
        state.worker_tx = Some(tx.clone());
    }
    let hwnd_value = hwnd as usize;
    thread::spawn(move || nv_worker(rx, hwnd_value));
    let _ = tx.send(WorkerCommand::Read);
    wm::SetTimer(hwnd, REFRESH_TIMER_ID, 30_000, None);

    if let Err(error) = &registered {
        show_message(
            hwnd,
            &format!(
                "无法注册快捷键 {}。\n\n{}\n请从托盘菜单打开设置并选择其他组合。",
                format_hotkey(settings.hotkey),
                error.user_message()
            ),
            wm::MB_OK | wm::MB_ICONWARNING,
        );
    }
    if let Some(warning) = startup_warning {
        log_line(&warning);
        show_message(hwnd, &warning, wm::MB_OK | wm::MB_ICONWARNING);
    }

    let mut message: wm::MSG = zeroed();
    loop {
        let result = wm::GetMessageW(&mut message, null_mut(), 0, 0);
        if result <= 0 {
            break;
        }
        wm::TranslateMessage(&message);
        wm::DispatchMessageW(&message);
    }

    CloseHandle(mutex);
    0
}

unsafe fn register_window_class(
    name: &[u16],
    proc: wm::WNDPROC,
    icon: HICON,
    cursor: wm::HCURSOR,
    background: gdi::HBRUSH,
    style: u32,
    instance: HINSTANCE,
) -> bool {
    let mut class: wm::WNDCLASSEXW = zeroed();
    class.cbSize = size_of::<wm::WNDCLASSEXW>() as u32;
    class.style = style;
    class.lpfnWndProc = proc;
    class.hInstance = instance;
    class.hIcon = icon;
    class.hCursor = cursor;
    class.hbrBackground = background;
    class.lpszClassName = name.as_ptr();
    class.hIconSm = icon;
    wm::RegisterClassExW(&class) != 0
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_APP_WORKER {
        handle_worker_events(hwnd);
        return 0;
    }
    if message == WM_APP_SETTINGS_CLOSED {
        if let Some(state) = APP_STATE.get() {
            let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
            state.settings_window = 0;
        }
        return 0;
    }

    let registered_open_message = APP_STATE
        .get()
        .map(|state| {
            state
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .open_settings_message
        })
        .unwrap_or(0);
    if registered_open_message != 0 && message == registered_open_message {
        open_settings(hwnd);
        return 0;
    }

    let taskbar_message = APP_STATE
        .get()
        .map(|state| {
            state
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .taskbar_created_message
        })
        .unwrap_or(0);
    if taskbar_message != 0 && message == taskbar_message {
        if let Some(state) = APP_STATE.get() {
            let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
            state.tray_added = false;
        }
        add_or_update_tray(hwnd, true);
        return 0;
    }

    match message {
        wm::WM_HOTKEY if wparam as i32 == HOTKEY_ID => {
            request_toggle(hwnd, true);
            0
        }
        WM_APP_TRAY => {
            match lparam as u32 {
                wm::WM_LBUTTONUP => open_settings(hwnd),
                wm::WM_RBUTTONUP | wm::WM_CONTEXTMENU => show_tray_menu(hwnd),
                _ => {}
            }
            0
        }
        wm::WM_TIMER if wparam == REFRESH_TIMER_ID => {
            request_read(hwnd);
            0
        }
        wm::WM_COMMAND => {
            handle_main_command(hwnd, wparam as u32 & 0xffff);
            0
        }
        wm::WM_CLOSE => {
            wm::DestroyWindow(hwnd);
            0
        }
        wm::WM_QUERYENDSESSION => 1,
        wm::WM_ENDSESSION if wparam != 0 => {
            wm::DestroyWindow(hwnd);
            0
        }
        wm::WM_DESTROY => {
            cleanup(hwnd);
            wm::PostQuitMessage(0);
            0
        }
        _ => wm::DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe fn cleanup(hwnd: HWND) {
    let (worker, settings_window, tray_added) = if let Some(state) = APP_STATE.get() {
        let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
        let worker = state.worker_tx.take();
        let settings_window = state.settings_window;
        let tray_added = state.tray_added;
        state.settings_window = 0;
        state.tray_added = false;
        (worker, settings_window, tray_added)
    } else {
        (None, 0, false)
    };

    wm::KillTimer(hwnd, REFRESH_TIMER_ID);
    let _ = UnregisterHotKey(hwnd, HOTKEY_ID);
    if let Some(worker) = worker {
        let _ = worker.send(WorkerCommand::Shutdown);
    }
    if settings_window != 0 {
        let settings_hwnd = settings_window as HWND;
        if wm::IsWindow(settings_hwnd) != 0 {
            wm::DestroyWindow(settings_hwnd);
        }
    }
    if tray_added {
        let mut data: NOTIFYICONDATAW = zeroed();
        data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = hwnd;
        data.uID = TRAY_ICON_ID;
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

unsafe fn request_read(hwnd: HWND) {
    let sender = if let Some(state) = APP_STATE.get() {
        let state = state.lock().unwrap_or_else(|p| p.into_inner());
        if state.busy {
            return;
        }
        state.worker_tx.clone()
    } else {
        None
    };
    if let Some(sender) = sender {
        if sender.send(WorkerCommand::Read).is_err() {
            log_line("NVIDIA worker channel closed");
        }
    }
    add_or_update_tray(hwnd, false);
}

unsafe fn request_toggle(hwnd: HWND, from_hotkey: bool) {
    let sender = if let Some(state) = APP_STATE.get() {
        let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
        if from_hotkey && state.settings_window != 0 {
            return;
        }
        if state.busy {
            return;
        }
        if state
            .last_toggle
            .is_some_and(|last| last.elapsed().as_millis() < 300)
        {
            return;
        }
        if state.worker_tx.is_none() {
            return;
        }
        state.busy = true;
        state.last_toggle = Some(Instant::now());
        state.worker_tx.clone()
    } else {
        None
    };
    if let Some(sender) = sender {
        if sender.send(WorkerCommand::Toggle).is_err() {
            if let Some(state) = APP_STATE.get() {
                let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
                state.busy = false;
            }
            log_line("failed to send toggle command to NVIDIA worker");
        }
        add_or_update_tray(hwnd, false);
    }
}

fn nv_worker(receiver: mpsc::Receiver<WorkerCommand>, hwnd_value: usize) {
    let hwnd = hwnd_value as HWND;
    let mut backend: Option<NvBackend> = None;
    while let Ok(command) = receiver.recv() {
        match command {
            WorkerCommand::Read => {
                if backend.is_none() {
                    match unsafe { NvBackend::open() } {
                        Ok(value) => backend = Some(value),
                        Err(error) => {
                            log_line(&format!("NVIDIA backend open failed: {error:?}"));
                            post_worker_event(hwnd, WorkerEvent::Read(Err(error)));
                            continue;
                        }
                    }
                }
                let result = unsafe { backend.as_ref().unwrap().read_snapshot() };
                if let Err(error) = &result {
                    log_line(&format!("NVIDIA state read failed: {error:?}"));
                }
                post_worker_event(hwnd, WorkerEvent::Read(result));
            }
            WorkerCommand::Toggle => {
                if backend.is_none() {
                    match unsafe { NvBackend::open() } {
                        Ok(value) => backend = Some(value),
                        Err(error) => {
                            post_worker_event(hwnd, WorkerEvent::Toggle(Err(error)));
                            continue;
                        }
                    }
                }
                let result = toggle_with_transport(backend.as_mut().unwrap());
                if result.is_err() {
                    log_line(&format!("NVIDIA HDR toggle failed: {result:?}"));
                }
                let rollback_failed = matches!(&result, Err(AppError::RollbackFailed));
                if rollback_failed {
                    backend = None;
                }
                post_worker_event(hwnd, WorkerEvent::Toggle(result));
            }
            WorkerCommand::Shutdown => break,
        }
    }
    drop(backend);
}

fn post_worker_event(hwnd: HWND, event: WorkerEvent) {
    if let Some(queue) = PENDING_EVENTS.get() {
        queue
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push_back(event);
        unsafe {
            wm::PostMessageW(hwnd, WM_APP_WORKER, 0, 0);
        }
    }
}

unsafe fn handle_worker_events(hwnd: HWND) {
    let events = PENDING_EVENTS
        .get()
        .map(|queue| {
            let mut queue = queue.lock().unwrap_or_else(|p| p.into_inner());
            queue.drain(..).collect::<Vec<_>>()
        })
        .unwrap_or_default();

    for event in events {
        match event {
            WorkerEvent::Read(result) => {
                let (notification, critical) = if let Some(state) = APP_STATE.get() {
                    let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
                    match result {
                        Ok(snapshot) => {
                            state.hdr = HdrUi::Ready(snapshot);
                            state.last_error = None;
                            (None, false)
                        }
                        Err(error) => {
                            let text = error.user_message();
                            let changed = state.last_error.as_deref() != Some(&text);
                            state.hdr = HdrUi::Error(text.clone());
                            state.last_error = Some(text.clone());
                            let enabled = state.settings.notifications;
                            (
                                if changed && enabled {
                                    Some((text, true))
                                } else {
                                    None
                                },
                                false,
                            )
                        }
                    }
                } else {
                    (None, false)
                };
                add_or_update_tray(hwnd, false);
                if let Some((text, is_error)) = notification {
                    show_balloon(hwnd, "RTX 视频 HDR", &text, is_error);
                }
                if critical {
                    show_message(hwnd, "RTX 视频 HDR", wm::MB_OK | wm::MB_ICONERROR);
                }
            }
            WorkerEvent::Toggle(result) => {
                let (notification, critical) = if let Some(state) = APP_STATE.get() {
                    let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
                    state.busy = false;
                    match result {
                        Ok(outcome) => {
                            let enabled = outcome.after.enabled;
                            state.hdr = HdrUi::Ready(outcome.after);
                            state.last_error = None;
                            let text = if enabled {
                                "RTX 视频 HDR 已开启".to_string()
                            } else {
                                "RTX 视频 HDR 已关闭".to_string()
                            };
                            let enabled = state.settings.notifications;
                            (if enabled { Some((text, false)) } else { None }, false)
                        }
                        Err(error) => {
                            let text = error.user_message();
                            let critical = matches!(error, AppError::RollbackFailed);
                            state.hdr = HdrUi::Error(text.clone());
                            state.last_error = Some(text.clone());
                            (Some((text, true)), critical)
                        }
                    }
                } else {
                    (None, false)
                };
                add_or_update_tray(hwnd, false);
                if let Some((text, is_error)) = notification {
                    show_balloon(hwnd, "RTX 视频 HDR", &text, is_error);
                }
                if critical {
                    let detail = APP_STATE
                        .get()
                        .and_then(|state| {
                            state
                                .lock()
                                .unwrap_or_else(|p| p.into_inner())
                                .last_error
                                .clone()
                        })
                        .unwrap_or_else(|| "写入失败，且无法恢复原始状态。".into());
                    show_message(
                        hwnd,
                        &format!("{detail}\n\n请立即打开 NVIDIA App 检查 RTX 视频 HDR 状态。"),
                        wm::MB_OK | wm::MB_ICONERROR,
                    );
                }
            }
        }
    }
}

unsafe fn add_or_update_tray(hwnd: HWND, force_add: bool) {
    let (icon, tooltip, already_added) = if let Some(state) = APP_STATE.get() {
        let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
        let icon = state.icons.selected(&state.hdr);
        let tooltip = tray_tooltip(&state);
        let already_added = state.tray_added;
        if force_add {
            state.tray_added = false;
        }
        (icon, tooltip, already_added && !force_add)
    } else {
        return;
    };

    let mut data: NOTIFYICONDATAW = zeroed();
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = TRAY_ICON_ID;
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = WM_APP_TRAY;
    data.hIcon = icon;
    copy_wide(&mut data.szTip, &tooltip);

    let mut success = Shell_NotifyIconW(if already_added { NIM_MODIFY } else { NIM_ADD }, &data);
    if success == 0 && already_added {
        success = Shell_NotifyIconW(NIM_ADD, &data);
    }
    if success != 0 {
        if let Some(state) = APP_STATE.get() {
            let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
            state.tray_added = true;
        }
    }
}

unsafe fn show_balloon(hwnd: HWND, title: &str, text: &str, is_error: bool) {
    add_or_update_tray(hwnd, false);
    let icon = APP_STATE
        .get()
        .map(|state| {
            let state = state.lock().unwrap_or_else(|p| p.into_inner());
            state.icons.selected(&state.hdr)
        })
        .unwrap_or(null_mut());
    let mut data: NOTIFYICONDATAW = zeroed();
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = TRAY_ICON_ID;
    data.uFlags = NIF_INFO | NIF_ICON | NIF_TIP;
    data.hIcon = icon;
    copy_wide(&mut data.szInfoTitle, title);
    copy_wide(&mut data.szInfo, text);
    data.dwInfoFlags = if is_error { NIIF_ERROR } else { NIIF_INFO };
    let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
}

fn tray_tooltip(state: &AppState) -> String {
    let state_text = match &state.hdr {
        HdrUi::Loading => "正在读取",
        HdrUi::Ready(snapshot) if !snapshot.supported => "不支持",
        HdrUi::Ready(snapshot) if snapshot.enabled => "开",
        HdrUi::Ready(_) => "关",
        HdrUi::Error(_) => "不可用",
    };
    format!(
        "RTX 视频 HDR：{} | {}",
        state_text,
        format_hotkey(state.settings.hotkey)
    )
}

unsafe fn show_tray_menu(hwnd: HWND) {
    let (status, toggle_enabled, startup, notifications) = if let Some(state) = APP_STATE.get() {
        let state = state.lock().unwrap_or_else(|p| p.into_inner());
        let status = match &state.hdr {
            HdrUi::Loading => "状态：正在读取".to_string(),
            HdrUi::Ready(snapshot) if snapshot.enabled => "状态：开".to_string(),
            HdrUi::Ready(snapshot) if snapshot.supported => "状态：关".to_string(),
            HdrUi::Ready(_) => "状态：不支持".to_string(),
            HdrUi::Error(_) => "状态：不可用".to_string(),
        };
        (
            status,
            !state.busy,
            state.settings.start_at_login,
            state.settings.notifications,
        )
    } else {
        return;
    };

    let menu = wm::CreatePopupMenu();
    if menu.is_null() {
        return;
    }
    let status_wide = wide(&status);
    wm::AppendMenuW(menu, wm::MF_STRING | wm::MF_GRAYED, 0, status_wide.as_ptr());
    wm::AppendMenuW(menu, wm::MF_SEPARATOR, 0, null());
    let toggle = wide("切换 RTX 视频 HDR");
    wm::AppendMenuW(
        menu,
        wm::MF_STRING | if toggle_enabled { 0 } else { wm::MF_GRAYED },
        CMD_TOGGLE as usize,
        toggle.as_ptr(),
    );
    let settings = wide("设置...");
    wm::AppendMenuW(
        menu,
        wm::MF_STRING,
        CMD_SETTINGS as usize,
        settings.as_ptr(),
    );
    wm::AppendMenuW(menu, wm::MF_SEPARATOR, 0, null());
    let startup_text = wide("开机启动");
    wm::AppendMenuW(
        menu,
        wm::MF_STRING
            | if startup {
                wm::MF_CHECKED
            } else {
                wm::MF_UNCHECKED
            },
        CMD_STARTUP as usize,
        startup_text.as_ptr(),
    );
    let notifications_text = wide("显示通知");
    wm::AppendMenuW(
        menu,
        wm::MF_STRING
            | if notifications {
                wm::MF_CHECKED
            } else {
                wm::MF_UNCHECKED
            },
        CMD_NOTIFICATIONS as usize,
        notifications_text.as_ptr(),
    );
    wm::AppendMenuW(menu, wm::MF_SEPARATOR, 0, null());
    let diagnostics = wide("诊断信息...");
    wm::AppendMenuW(
        menu,
        wm::MF_STRING,
        CMD_DIAGNOSTICS as usize,
        diagnostics.as_ptr(),
    );
    let exit = wide("退出");
    wm::AppendMenuW(menu, wm::MF_STRING, CMD_EXIT as usize, exit.as_ptr());

    let mut point: windows_sys::Win32::Foundation::POINT = zeroed();
    let _ = wm::GetCursorPos(&mut point);
    let _ = wm::SetForegroundWindow(hwnd);
    let command = wm::TrackPopupMenu(
        menu,
        wm::TPM_RETURNCMD | wm::TPM_RIGHTBUTTON | wm::TPM_NONOTIFY,
        point.x,
        point.y,
        0,
        hwnd,
        null(),
    ) as u32;
    wm::DestroyMenu(menu);
    let _ = wm::PostMessageW(hwnd, wm::WM_NULL, 0, 0);
    if command != 0 {
        handle_main_command(hwnd, command);
    }
}

unsafe fn handle_main_command(hwnd: HWND, command: u32) {
    match command {
        CMD_TOGGLE => request_toggle(hwnd, false),
        CMD_SETTINGS => open_settings(hwnd),
        CMD_STARTUP => {
            let mut candidate = current_settings();
            candidate.start_at_login = !candidate.start_at_login;
            if let Err(error) = apply_settings(hwnd, candidate) {
                show_message(hwnd, &error.user_message(), wm::MB_OK | wm::MB_ICONERROR);
            }
        }
        CMD_NOTIFICATIONS => {
            let mut candidate = current_settings();
            candidate.notifications = !candidate.notifications;
            if let Err(error) = apply_settings(hwnd, candidate) {
                show_message(hwnd, &error.user_message(), wm::MB_OK | wm::MB_ICONERROR);
            }
        }
        CMD_DIAGNOSTICS => show_diagnostics(hwnd),
        CMD_EXIT => {
            wm::DestroyWindow(hwnd);
        }
        _ => {}
    }
}

fn current_settings() -> Settings {
    APP_STATE
        .get()
        .map(|state| {
            state
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .settings
                .clone()
        })
        .unwrap_or_default()
}

unsafe fn apply_settings(owner: HWND, candidate: Settings) -> Result<(), AppError> {
    candidate.hotkey.validate()?;
    let previous = current_settings();
    let was_registered = APP_STATE
        .get()
        .map(|state| {
            state
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .hotkey_registered
        })
        .unwrap_or(false);
    let changed = candidate.hotkey != previous.hotkey || !was_registered;
    if changed {
        register_configured_hotkey(owner, candidate.hotkey)?;
    }

    if let Err(error) = set_autostart(candidate.start_at_login) {
        if changed {
            let _ = register_configured_hotkey(owner, previous.hotkey);
        }
        return Err(error);
    }
    if let Err(error) = save_settings(&candidate) {
        let _ = set_autostart(previous.start_at_login);
        if changed {
            let _ = register_configured_hotkey(owner, previous.hotkey);
        }
        return Err(error);
    }

    if let Some(state) = APP_STATE.get() {
        let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
        state.settings = candidate;
        state.hotkey_registered = true;
    }
    add_or_update_tray(owner, false);
    Ok(())
}

unsafe fn register_configured_hotkey(hwnd: HWND, hotkey: Hotkey) -> Result<(), AppError> {
    hotkey.validate()?;
    let result = RegisterHotKey(hwnd, HOTKEY_ID, hotkey.modifiers | MOD_NOREPEAT, hotkey.vk);
    if result == 0 {
        let code = GetLastError();
        return Err(AppError::Settings(format!(
            "无法注册快捷键 {}（Windows 错误 {code}）。",
            format_hotkey(hotkey)
        )));
    }
    Ok(())
}

unsafe fn open_settings(owner: HWND) {
    let existing = APP_STATE
        .get()
        .map(|state| {
            state
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .settings_window
        })
        .unwrap_or(0);
    if existing != 0 {
        let hwnd = existing as HWND;
        if wm::IsWindow(hwnd) != 0 {
            let _ = wm::ShowWindow(hwnd, wm::SW_SHOWNORMAL);
            let _ = wm::SetForegroundWindow(hwnd);
            return;
        }
    }

    let settings = current_settings();
    let boxed = Box::new(SettingsWindowState {
        owner,
        settings,
        recording: false,
        last_down: [false; 256],
        capture_keys: capture_keys(),
        hotkey_label: null_mut(),
        record_button: null_mut(),
        startup_check: null_mut(),
        notification_check: null_mut(),
        save_button: null_mut(),
    });
    let state_ptr = Box::into_raw(boxed);
    let class_name = wide(SETTINGS_CLASS);
    let title = wide("RTX HDR 开关 - 设置");
    let instance = GetModuleHandleW(null());
    let owner_dpi = GetDpiForWindow(owner).max(96) as i32;
    let width = scale_for_dpi(480, owner_dpi);
    let height = scale_for_dpi(310, owner_dpi);
    let hwnd = wm::CreateWindowExW(
        wm::WS_EX_DLGMODALFRAME,
        class_name.as_ptr(),
        title.as_ptr(),
        wm::WS_CAPTION | wm::WS_SYSMENU | wm::WS_CLIPCHILDREN,
        wm::CW_USEDEFAULT,
        wm::CW_USEDEFAULT,
        width,
        height,
        owner,
        null_mut(),
        instance,
        state_ptr as *const c_void,
    );
    if hwnd.is_null() {
        drop(Box::from_raw(state_ptr));
        show_message(
            owner,
            &format!("无法创建设置窗口（Windows 错误 {}）。", GetLastError()),
            wm::MB_OK | wm::MB_ICONERROR,
        );
        return;
    }
    wm::SetWindowLongPtrW(hwnd, wm::GWLP_USERDATA, state_ptr as isize);
    create_settings_controls(hwnd, &mut *state_ptr);
    if let Some(state) = APP_STATE.get() {
        let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
        state.settings_window = hwnd as usize;
    }
    let _ = wm::ShowWindow(hwnd, wm::SW_SHOWNORMAL);
    let _ = gdi::UpdateWindow(hwnd);
    let _ = wm::SetForegroundWindow(hwnd);
}

unsafe fn create_settings_controls(hwnd: HWND, state: &mut SettingsWindowState) {
    let dpi = GetDpiForWindow(hwnd).max(96) as i32;
    let font = gdi::GetStockObject(gdi::DEFAULT_GUI_FONT);
    let title = create_control(
        hwnd,
        "STATIC",
        "RTX 视频 HDR",
        wm::WS_CHILD | wm::WS_VISIBLE,
        scale_for_dpi(20, dpi),
        scale_for_dpi(18, dpi),
        scale_for_dpi(420, dpi),
        scale_for_dpi(28, dpi),
        0,
    );
    let label = create_control(
        hwnd,
        "STATIC",
        "快捷键",
        wm::WS_CHILD | wm::WS_VISIBLE,
        scale_for_dpi(20, dpi),
        scale_for_dpi(58, dpi),
        scale_for_dpi(80, dpi),
        scale_for_dpi(24, dpi),
        0,
    );
    state.hotkey_label = create_control(
        hwnd,
        "STATIC",
        "",
        wm::WS_CHILD | wm::WS_VISIBLE,
        scale_for_dpi(105, dpi),
        scale_for_dpi(58, dpi),
        scale_for_dpi(165, dpi),
        scale_for_dpi(24, dpi),
        0,
    );
    state.record_button = create_control(
        hwnd,
        "BUTTON",
        "录制",
        wm::WS_CHILD | wm::WS_VISIBLE | wm::WS_TABSTOP | wm::BS_PUSHBUTTON as u32,
        scale_for_dpi(278, dpi),
        scale_for_dpi(52, dpi),
        scale_for_dpi(150, dpi),
        scale_for_dpi(32, dpi),
        ID_RECORD,
    );
    let hint = create_control(
        hwnd,
        "STATIC",
        "点击“录制”后，按下包含 Ctrl、Alt、Shift 或 Win 的组合键。",
        wm::WS_CHILD | wm::WS_VISIBLE,
        scale_for_dpi(20, dpi),
        scale_for_dpi(94, dpi),
        scale_for_dpi(420, dpi),
        scale_for_dpi(42, dpi),
        0,
    );
    state.startup_check = create_control(
        hwnd,
        "BUTTON",
        "登录 Windows 时自动启动",
        wm::WS_CHILD | wm::WS_VISIBLE | wm::BS_AUTOCHECKBOX as u32,
        scale_for_dpi(20, dpi),
        scale_for_dpi(150, dpi),
        scale_for_dpi(300, dpi),
        scale_for_dpi(26, dpi),
        ID_STARTUP,
    );
    state.notification_check = create_control(
        hwnd,
        "BUTTON",
        "切换时显示通知",
        wm::WS_CHILD | wm::WS_VISIBLE | wm::BS_AUTOCHECKBOX as u32,
        scale_for_dpi(20, dpi),
        scale_for_dpi(182, dpi),
        scale_for_dpi(260, dpi),
        scale_for_dpi(26, dpi),
        ID_NOTIFICATIONS,
    );
    state.save_button = create_control(
        hwnd,
        "BUTTON",
        "保存",
        wm::WS_CHILD | wm::WS_VISIBLE | wm::WS_TABSTOP | wm::BS_PUSHBUTTON as u32,
        scale_for_dpi(278, dpi),
        scale_for_dpi(226, dpi),
        scale_for_dpi(72, dpi),
        scale_for_dpi(32, dpi),
        ID_SAVE,
    );
    let cancel = create_control(
        hwnd,
        "BUTTON",
        "取消",
        wm::WS_CHILD | wm::WS_VISIBLE | wm::WS_TABSTOP | wm::BS_PUSHBUTTON as u32,
        scale_for_dpi(358, dpi),
        scale_for_dpi(226, dpi),
        scale_for_dpi(72, dpi),
        scale_for_dpi(32, dpi),
        ID_CANCEL,
    );

    for control in [
        title,
        label,
        state.hotkey_label,
        state.record_button,
        hint,
        state.startup_check,
        state.notification_check,
        state.save_button,
        cancel,
    ] {
        wm::SendMessageW(control, wm::WM_SETFONT, font as usize, 1);
    }
    wm::SendMessageW(
        state.startup_check,
        wm::BM_SETCHECK,
        if state.settings.start_at_login { 1 } else { 0 },
        0,
    );
    wm::SendMessageW(
        state.notification_check,
        wm::BM_SETCHECK,
        if state.settings.notifications { 1 } else { 0 },
        0,
    );
    update_hotkey_text(state);
    let _ = SetFocus(state.save_button);
}

#[allow(clippy::too_many_arguments)]
unsafe fn create_control(
    parent: HWND,
    class: &str,
    text: &str,
    style: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    id: u32,
) -> HWND {
    let class = wide(class);
    let text = wide(text);
    let instance = GetModuleHandleW(null());
    wm::CreateWindowExW(
        0,
        class.as_ptr(),
        text.as_ptr(),
        style,
        x,
        y,
        width,
        height,
        parent,
        id as usize as HMENU,
        instance,
        null(),
    )
}

unsafe extern "system" fn settings_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state_ptr = wm::GetWindowLongPtrW(hwnd, wm::GWLP_USERDATA) as *mut SettingsWindowState;
    match message {
        wm::WM_COMMAND if !state_ptr.is_null() => {
            let command = wparam as u32 & 0xffff;
            match command {
                ID_RECORD => start_recording(hwnd, &mut *state_ptr),
                ID_SAVE => save_settings_from_window(hwnd, &mut *state_ptr),
                ID_CANCEL => {
                    wm::DestroyWindow(hwnd);
                }
                _ => {}
            }
            0
        }
        wm::WM_TIMER if wparam == RECORD_TIMER_ID && !state_ptr.is_null() => {
            scan_recorded_hotkey(hwnd, &mut *state_ptr);
            0
        }
        WM_ACTIVATE if !state_ptr.is_null() => {
            if (wparam as u32 & 0xffff) == wm::WA_INACTIVE && (*state_ptr).recording {
                stop_recording(hwnd, &mut *state_ptr);
            }
            0
        }
        wm::WM_KEYDOWN if !state_ptr.is_null() && !(*state_ptr).recording => {
            if wparam == 0x1b {
                wm::DestroyWindow(hwnd);
                return 0;
            }
            if wparam == 0x0d {
                save_settings_from_window(hwnd, &mut *state_ptr);
                return 0;
            }
            wm::DefWindowProcW(hwnd, message, wparam, lparam)
        }
        wm::WM_CLOSE => {
            wm::DestroyWindow(hwnd);
            0
        }
        WM_NCDESTROY => {
            if !state_ptr.is_null() {
                wm::KillTimer(hwnd, RECORD_TIMER_ID);
                wm::SetWindowLongPtrW(hwnd, wm::GWLP_USERDATA, 0);
                let owner = (*state_ptr).owner;
                drop(Box::from_raw(state_ptr));
                if !owner.is_null() {
                    let _ = wm::PostMessageW(owner, WM_APP_SETTINGS_CLOSED, 0, 0);
                }
            }
            wm::DefWindowProcW(hwnd, message, wparam, lparam)
        }
        _ => wm::DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe fn start_recording(hwnd: HWND, state: &mut SettingsWindowState) {
    if state.recording {
        return;
    }
    state.recording = true;
    for (key, down) in state.last_down.iter_mut().enumerate() {
        *down = key_down(key as u32);
    }
    let text = wide("请按快捷键...");
    let _ = wm::SetWindowTextW(state.record_button, text.as_ptr());
    let _ = EnableWindow(state.save_button, 0);
    let _ = SetFocus(hwnd);
    wm::SetTimer(hwnd, RECORD_TIMER_ID, 30, None);
}

unsafe fn stop_recording(hwnd: HWND, state: &mut SettingsWindowState) {
    if !state.recording {
        return;
    }
    state.recording = false;
    wm::KillTimer(hwnd, RECORD_TIMER_ID);
    let text = wide("重新录制");
    let _ = wm::SetWindowTextW(state.record_button, text.as_ptr());
    let _ = EnableWindow(state.save_button, 1);
    update_hotkey_text(state);
}

unsafe fn scan_recorded_hotkey(hwnd: HWND, state: &mut SettingsWindowState) {
    if wm::GetForegroundWindow() != hwnd {
        return;
    }
    let modifiers = current_modifiers();
    let escape_down = key_down(0x1b);
    let escape_pressed = escape_down && !state.last_down[0x1b];
    if escape_pressed && modifiers == 0 {
        stop_recording(hwnd, state);
        return;
    }

    if modifiers != 0 {
        for key in &state.capture_keys {
            let down = key_down(*key);
            if down && !state.last_down[*key as usize] {
                state.settings.hotkey = Hotkey {
                    modifiers,
                    vk: *key,
                };
                stop_recording(hwnd, state);
                return;
            }
        }
    }

    for key in &state.capture_keys {
        state.last_down[*key as usize] = key_down(*key);
    }
    state.last_down[0x1b] = escape_down;
}

unsafe fn save_settings_from_window(hwnd: HWND, state: &mut SettingsWindowState) {
    if state.recording {
        return;
    }
    let mut candidate = state.settings.clone();
    candidate.start_at_login =
        wm::SendMessageW(state.startup_check, wm::BM_GETCHECK, 0, 0) as u32 == BST_CHECKED;
    candidate.notifications =
        wm::SendMessageW(state.notification_check, wm::BM_GETCHECK, 0, 0) as u32 == BST_CHECKED;
    match apply_settings(state.owner, candidate) {
        Ok(()) => {
            wm::DestroyWindow(hwnd);
        }
        Err(error) => {
            show_message(hwnd, &error.user_message(), wm::MB_OK | wm::MB_ICONERROR);
        }
    }
}

unsafe fn update_hotkey_text(state: &SettingsWindowState) {
    let text = wide(&format_hotkey(state.settings.hotkey));
    let _ = wm::SetWindowTextW(state.hotkey_label, text.as_ptr());
}

fn capture_keys() -> Vec<u32> {
    let mut keys = Vec::with_capacity(96);
    keys.extend(0x30..=0x39);
    keys.extend(0x41..=0x5a);
    keys.extend(0x70..=0x87);
    keys.extend([
        0x08, 0x09, 0x0d, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x2d, 0x2e, 0xba,
        0xbb, 0xbc, 0xbd, 0xbe, 0xbf, 0xc0, 0xdb, 0xdc, 0xdd, 0xde,
    ]);
    keys
}

fn key_down(key: u32) -> bool {
    unsafe { (GetAsyncKeyState(key as i32) as u16 & 0x8000) != 0 }
}

fn current_modifiers() -> u32 {
    let mut modifiers = 0;
    if key_down(0xa2) || key_down(0xa3) || key_down(0x11) {
        modifiers |= MOD_CONTROL;
    }
    if key_down(0xa4) || key_down(0xa5) || key_down(0x12) {
        modifiers |= MOD_ALT;
    }
    if key_down(0xa0) || key_down(0xa1) || key_down(0x10) {
        modifiers |= MOD_SHIFT;
    }
    if key_down(0x5b) || key_down(0x5c) {
        modifiers |= MOD_WIN;
    }
    modifiers
}

unsafe fn show_diagnostics(hwnd: HWND) {
    let text = if let Some(state) = APP_STATE.get() {
        let state = state.lock().unwrap_or_else(|p| p.into_inner());
        let hdr = match &state.hdr {
            HdrUi::Loading => "正在读取".to_string(),
            HdrUi::Error(error) => format!("不可用：{error}"),
            HdrUi::Ready(snapshot) => {
                let toggle = if snapshot.enabled { "开" } else { "关" };
                let active = if snapshot.active { "是" } else { "否" };
                format!(
                    "{toggle}\n当前处理中：{active}\n错误码：{}\n峰值：{}（范围 {}–{}）\n中间灰：{}\n对比度：{}\n饱和度：{}",
                    snapshot.error_code,
                    snapshot.peak_brightness,
                    snapshot.peak_min,
                    snapshot.peak_max,
                    snapshot.middle_gray,
                    snapshot.contrast,
                    snapshot.saturation
                )
            }
        };
        format!(
            "版本：{}\nRTX 视频 HDR：{hdr}\n快捷键：{}（{}）\n开机启动：{}\n通知：{}\n\nNvCpl：{}\n允许的 SHA-256：{}\n\n最近错误：{}",
            env!("CARGO_PKG_VERSION"),
            format_hotkey(state.settings.hotkey),
            if state.hotkey_registered {
                "已注册"
            } else {
                "未注册"
            },
            yes_no(state.settings.start_at_login),
            yes_no(state.settings.notifications),
            nvcpl_path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "未找到".into()),
            NVCPL_SHA256_HEX,
            state.last_error.as_deref().unwrap_or("无")
        )
    } else {
        "应用状态尚未初始化。".into()
    };
    show_message(hwnd, &text, wm::MB_OK | wm::MB_ICONINFORMATION);
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "是"
    } else {
        "否"
    }
}

fn nvcpl_path() -> Option<PathBuf> {
    let program_files =
        std::env::var_os("ProgramW6432").or_else(|| std::env::var_os("ProgramFiles"))?;
    let path = PathBuf::from(program_files).join(NVCPL_RELATIVE_PATH);
    path.is_file().then_some(path)
}

unsafe fn verify_nvidia_signature(path: &Path) -> Result<(), AppError> {
    let path_wide = wide(&path.to_string_lossy());
    let mut file: WINTRUST_FILE_INFO = zeroed();
    file.cbStruct = size_of::<WINTRUST_FILE_INFO>() as u32;
    file.pcwszFilePath = path_wide.as_ptr();

    let mut data: WINTRUST_DATA = zeroed();
    data.cbStruct = size_of::<WINTRUST_DATA>() as u32;
    data.dwUIChoice = WTD_UI_NONE;
    data.fdwRevocationChecks = WTD_REVOKE_NONE;
    data.dwUnionChoice = WTD_CHOICE_FILE;
    data.Anonymous.pFile = &mut file;
    data.dwStateAction = WTD_STATEACTION_VERIFY;
    data.dwProvFlags = WTD_REVOCATION_CHECK_NONE | WTD_SAFER_FLAG | WTD_CACHE_ONLY_URL_RETRIEVAL;
    data.dwUIContext = WTD_UICONTEXT_EXECUTE;

    let mut action: GUID = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    let status = WinVerifyTrust(
        null_mut(),
        &mut action,
        &mut data as *mut WINTRUST_DATA as *mut c_void,
    );
    data.dwStateAction = WTD_STATEACTION_CLOSE;
    let _ = WinVerifyTrust(
        null_mut(),
        &mut action,
        &mut data as *mut WINTRUST_DATA as *mut c_void,
    );

    if status == 0 {
        // The exact SHA-256 pin above identifies NVIDIA's signed binary; the
        // WinVerifyTrust result also rejects tampered or unsigned content.
        Ok(())
    } else {
        Err(AppError::SignatureInvalid)
    }
}

fn load_settings() -> (Settings, bool) {
    let start = query_registry_dword("StartAtLogin");
    let notifications = query_registry_dword("Notifications");
    let modifiers = query_registry_dword("HotkeyModifiers");
    let virtual_key = query_registry_dword("HotkeyVirtualKey");
    let first_run = start.is_none();

    let mut settings = Settings {
        start_at_login: start.map(|value| value != 0).unwrap_or(true),
        notifications: notifications.map(|value| value != 0).unwrap_or(true),
        ..Settings::default()
    };
    if let (Some(modifiers), Some(virtual_key)) = (modifiers, virtual_key) {
        let hotkey = Hotkey {
            modifiers,
            vk: virtual_key,
        };
        if hotkey.validate().is_ok() {
            settings.hotkey = hotkey;
        }
    }
    (settings, first_run)
}

fn save_settings(settings: &Settings) -> Result<(), AppError> {
    unsafe {
        let mut key: HKEY = null_mut();
        let path = wide(SETTINGS_KEY);
        let code = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            path.as_ptr(),
            0,
            null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            null(),
            &mut key,
            null_mut(),
        );
        if code != 0 {
            return Err(registry_error("创建设置键", code));
        }
        let result = (|| {
            set_registry_dword(key, "HotkeyModifiers", settings.hotkey.modifiers)?;
            set_registry_dword(key, "HotkeyVirtualKey", settings.hotkey.vk)?;
            set_registry_dword(
                key,
                "StartAtLogin",
                if settings.start_at_login { 1 } else { 0 },
            )?;
            set_registry_dword(
                key,
                "Notifications",
                if settings.notifications { 1 } else { 0 },
            )
        })();
        RegCloseKey(key);
        result
    }
}

fn query_registry_dword(name: &str) -> Option<u32> {
    unsafe {
        let mut key: HKEY = null_mut();
        let path = wide(SETTINGS_KEY);
        if RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, KEY_READ, &mut key) != 0 {
            return None;
        }
        let mut value_type = 0u32;
        let mut value = 0u32;
        let mut size = size_of::<u32>() as u32;
        let name = wide(name);
        let code = RegQueryValueExW(
            key,
            name.as_ptr(),
            null(),
            &mut value_type,
            &mut value as *mut u32 as *mut u8,
            &mut size,
        );
        RegCloseKey(key);
        if code == 0 && value_type == REG_DWORD && size == size_of::<u32>() as u32 {
            Some(value)
        } else {
            None
        }
    }
}

unsafe fn set_registry_dword(key: HKEY, name: &str, value: u32) -> Result<(), AppError> {
    let name = wide(name);
    let code = RegSetValueExW(
        key,
        name.as_ptr(),
        0,
        REG_DWORD,
        &value as *const u32 as *const u8,
        size_of::<u32>() as u32,
    );
    if code == 0 {
        Ok(())
    } else {
        Err(registry_error("写入设置", code))
    }
}

fn set_autostart(enabled: bool) -> Result<(), AppError> {
    unsafe {
        let mut key: HKEY = null_mut();
        let path = wide(RUN_KEY);
        let code = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            path.as_ptr(),
            0,
            null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            null(),
            &mut key,
            null_mut(),
        );
        if code != 0 {
            return Err(registry_error("打开开机启动键", code));
        }
        let value_name = wide(RUN_VALUE);
        let result = if enabled {
            let executable = std::env::current_exe()
                .map_err(|error| AppError::Settings(format!("无法获取程序路径：{error}")))?;
            let command = format!("\"{}\" --background", executable.display());
            let command = wide(&command);
            let data = std::slice::from_raw_parts(
                command.as_ptr() as *const u8,
                command.len() * size_of::<u16>(),
            );
            let code = RegSetValueExW(
                key,
                value_name.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr(),
                data.len() as u32,
            );
            if code == 0 {
                Ok(())
            } else {
                Err(registry_error("写开机启动项", code))
            }
        } else {
            let code = RegDeleteValueW(key, value_name.as_ptr());
            if code == 0 || code == 2 {
                Ok(())
            } else {
                Err(registry_error("删除开机启动项", code))
            }
        };
        RegCloseKey(key);
        result
    }
}

fn registry_error(operation: &str, code: u32) -> AppError {
    AppError::Settings(format!("{operation}失败（Windows 错误 {code}）。"))
}

fn load_icon(instance: HINSTANCE, id: usize, fallback: HICON) -> HICON {
    let icon = unsafe { wm::LoadIconW(instance, id as PCWSTR) };
    if icon.is_null() {
        fallback
    } else {
        icon
    }
}

fn scale_for_dpi(value: i32, dpi: i32) -> i32 {
    (value * dpi + 48) / 96
}

fn copy_wide(target: &mut [u16], text: &str) {
    target.fill(0);
    for (slot, value) in target
        .iter_mut()
        .zip(text.encode_utf16().chain(std::iter::once(0)))
    {
        *slot = value;
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn show_message(hwnd: HWND, text: &str, style: u32) {
    let text = wide(text);
    let title = wide(APP_NAME);
    wm::MessageBoxW(hwnd, text.as_ptr(), title.as_ptr(), style);
}

fn log_line(text: &str) {
    let Some(base) = std::env::var_os("LOCALAPPDATA") else {
        return;
    };
    let directory = PathBuf::from(base).join("RtxHdrSwitch");
    let _ = fs::create_dir_all(&directory);
    let path = directory.join("rtx-hdr-switch.log");
    if fs::metadata(&path)
        .map(|metadata| metadata.len() > 1_048_576)
        .unwrap_or(false)
    {
        let _ = fs::remove_file(&path);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let _ = writeln!(file, "[{timestamp}] {text}");
    }
}
