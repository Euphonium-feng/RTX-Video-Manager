use crate::ipc::{
    decode, read_packet, write_packet, HostEvent, IpcError, UiCommand, UiError, PIPE_NAME,
    PROTOCOL_VERSION,
};
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::mem::size_of;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_BROKEN_PIPE, ERROR_NO_DATA, ERROR_PIPE_CONNECTED,
    ERROR_PIPE_NOT_CONNECTED, HANDLE,
};
use windows_sys::Win32::Security::{
    EqualSid, GetTokenInformation, TokenUser, PSID, TOKEN_QUERY, TOKEN_USER,
};
use windows_sys::Win32::Storage::FileSystem::{ReadFile, WriteFile, PIPE_ACCESS_DUPLEX};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeClientProcessId,
    GetNamedPipeClientSessionId, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
    PIPE_WAIT,
};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, OpenProcess, OpenProcessToken, Sleep,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW;

pub const WM_APP_IPC: u32 = windows_sys::Win32::UI::WindowsAndMessaging::WM_APP + 4;

#[derive(Clone, Debug)]
pub enum IpcEvent {
    Connected,
    Disconnected,
    Command(UiCommand),
}

#[derive(Clone, Copy)]
struct HostTarget {
    hwnd: usize,
    message: u32,
}

static HOST: OnceLock<Mutex<Option<HostTarget>>> = OnceLock::new();
static EVENTS: OnceLock<Mutex<VecDeque<IpcEvent>>> = OnceLock::new();
static CONNECTION: OnceLock<Mutex<usize>> = OnceLock::new();
static WRITE_LOCK: Mutex<()> = Mutex::new(());
static STOP: AtomicBool = AtomicBool::new(true);

pub fn start(hwnd: usize, message: u32) {
    initialize();
    STOP.store(false, Ordering::Release);
    *lock(&HOST) = Some(HostTarget { hwnd, message });
    thread::Builder::new()
        .name("rtx-hdr-ipc".into())
        .spawn(server_loop)
        .expect("failed to spawn IPC server thread");
}

pub fn shutdown() {
    STOP.store(true, Ordering::Release);
    let handle = *lock(&CONNECTION);
    if handle != 0 {
        unsafe {
            DisconnectNamedPipe(handle as HANDLE);
        }
    }
}

pub fn has_client() -> bool {
    *lock(&CONNECTION) != 0
}

pub fn send(event: &HostEvent) -> bool {
    let handle = *lock(&CONNECTION);
    if handle == 0 {
        return false;
    }
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
    let result = write_packet(&mut PipeWriter(handle as HANDLE), event);
    drop(_guard);
    if result.is_err() {
        clear_connection(handle as HANDLE);
        return false;
    }
    true
}

pub fn take_events() -> Vec<IpcEvent> {
    lock(&EVENTS).drain(..).collect()
}

fn initialize() {
    let _ = HOST.set(Mutex::new(None));
    let _ = EVENTS.set(Mutex::new(VecDeque::new()));
    let _ = CONNECTION.set(Mutex::new(0));
}

fn server_loop() {
    let name = wide(PIPE_NAME);
    while !STOP.load(Ordering::Acquire) {
        let pipe = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                4096,
                4096,
                0,
                null(),
            )
        };
        if pipe.is_null() {
            unsafe { Sleep(250) };
            continue;
        }

        let connected = unsafe { ConnectNamedPipe(pipe, null_mut()) } != 0;
        if !connected {
            let code = unsafe { GetLastError() };
            if code != ERROR_PIPE_CONNECTED {
                unsafe { CloseHandle(pipe) };
                unsafe { Sleep(250) };
                continue;
            }
        }

        if !same_user_and_session(pipe) {
            unsafe {
                DisconnectNamedPipe(pipe);
                CloseHandle(pipe);
            }
            unsafe { Sleep(250) };
            continue;
        }

        set_connection(pipe);
        if !serve_client(pipe) {
            let _ = send(&HostEvent::Error(UiError {
                message: "控制台与后台通信异常，请重新打开控制台。".into(),
                critical: false,
            }));
        }
        clear_connection(pipe);
        unsafe {
            DisconnectNamedPipe(pipe);
            CloseHandle(pipe);
        }
        post_event(IpcEvent::Disconnected);
    }
}

fn serve_client(pipe: HANDLE) -> bool {
    let mut reader = PipeReader(pipe);
    let first = read_command(&mut reader);
    let Ok(first) = first else {
        return false;
    };
    match first {
        UiCommand::Hello { protocol_version } if protocol_version == PROTOCOL_VERSION => {}
        UiCommand::Hello { protocol_version } => {
            let _ = send(&HostEvent::Error(UiError {
                message: format!(
                    "控制台协议版本 {protocol_version} 与后台 {PROTOCOL_VERSION} 不匹配，请重新解压发布包。"
                ),
                critical: true,
            }));
            return false;
        }
        _ => return false,
    }

    if !send(&HostEvent::Hello {
        protocol_version: PROTOCOL_VERSION,
        app_version: env!("CARGO_PKG_VERSION").into(),
    }) {
        return false;
    }
    post_event(IpcEvent::Connected);

    loop {
        match read_command(&mut reader) {
            Ok(UiCommand::Hello { .. }) => {}
            Ok(command) => post_event(IpcEvent::Command(command)),
            Err(_) => return true,
        }
    }
}

fn read_command(reader: &mut impl Read) -> Result<UiCommand, IpcError> {
    let payload = read_packet(reader)?;
    decode(&payload)
}

fn same_user_and_session(pipe: HANDLE) -> bool {
    unsafe {
        let mut client_pid = 0u32;
        if GetNamedPipeClientProcessId(pipe, &mut client_pid) == 0 {
            return false;
        }
        let mut client_session = 0u32;
        if GetNamedPipeClientSessionId(pipe, &mut client_session) == 0 {
            return false;
        }
        let mut current_session = 0u32;
        if ProcessIdToSessionId(GetCurrentProcessId(), &mut current_session) == 0
            || current_session != client_session
        {
            return false;
        }

        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, client_pid);
        if process.is_null() {
            return false;
        }
        let mut client_token: HANDLE = null_mut();
        let process_token_ok = OpenProcessToken(process, TOKEN_QUERY, &mut client_token) != 0;
        CloseHandle(process);
        if !process_token_ok {
            return false;
        }

        let mut current_token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut current_token) == 0 {
            CloseHandle(client_token);
            return false;
        }

        let client_sid = token_user_sid(client_token);
        let current_sid = token_user_sid(current_token);
        CloseHandle(client_token);
        CloseHandle(current_token);

        matches!((client_sid, current_sid), (Some(client), Some(current)) if EqualSid(client, current) != 0)
    }
}

unsafe fn token_user_sid(token: HANDLE) -> Option<PSID> {
    let mut required = 0u32;
    GetTokenInformation(token, TokenUser, null_mut(), 0, &mut required);
    if required == 0 {
        return None;
    }
    let words = (required as usize).div_ceil(size_of::<usize>());
    let mut buffer = vec![0usize; words];
    if GetTokenInformation(
        token,
        TokenUser,
        buffer.as_mut_ptr().cast(),
        required,
        &mut required,
    ) == 0
    {
        return None;
    }
    let token_user = &*(buffer.as_ptr() as *const TOKEN_USER);
    Some(token_user.User.Sid)
}

fn post_event(event: IpcEvent) {
    lock(&EVENTS).push_back(event);
    let target = *lock(&HOST);
    if let Some(target) = target {
        unsafe {
            PostMessageW(target.hwnd as HANDLE, target.message, 0, 0);
        }
    }
}

fn set_connection(pipe: HANDLE) {
    *lock(&CONNECTION) = pipe as usize;
}

fn clear_connection(pipe: HANDLE) {
    let mut connection = lock(&CONNECTION);
    if *connection == pipe as usize {
        *connection = 0;
    }
}

fn lock<T>(mutex: &OnceLock<Mutex<T>>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .get()
        .unwrap()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

struct PipeReader(HANDLE);

impl Read for PipeReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut count = 0u32;
        let size = buffer.len().min(u32::MAX as usize) as u32;
        let ok = unsafe { ReadFile(self.0, buffer.as_mut_ptr(), size, &mut count, null_mut()) };
        if ok == 0 {
            let error = unsafe { GetLastError() };
            if matches!(
                error,
                ERROR_BROKEN_PIPE | ERROR_NO_DATA | ERROR_PIPE_NOT_CONNECTED
            ) {
                return Ok(0);
            }
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        Ok(count as usize)
    }
}

struct PipeWriter(HANDLE);

impl Write for PipeWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let mut count = 0u32;
        let size = buffer.len().min(u32::MAX as usize) as u32;
        let ok = unsafe { WriteFile(self.0, buffer.as_ptr(), size, &mut count, null_mut()) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(count as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
