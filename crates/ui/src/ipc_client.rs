use rtx_hdr_switch::ipc::{
    HostEvent, IpcError, PIPE_NAME, PROTOCOL_VERSION, UiCommand, read_packet, write_packet,
};
use std::io::{self, Read, Write};
use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, OPEN_EXISTING};

const FILE_GENERIC_READ_WRITE: u32 = 0xC000_0000;

#[derive(Debug)]
pub enum ClientEvent {
    Connected,
    Event(HostEvent),
    Disconnected(String),
}

pub struct IpcClient {
    commands: Sender<UiCommand>,
    _handle: Arc<PipeHandle>,
}

impl IpcClient {
    pub fn connect_retry() -> Result<(Self, Receiver<ClientEvent>), String> {
        let handle = connect_pipe()?;
        let handle = Arc::new(PipeHandle(handle as usize));
        let (event_tx, event_rx) = mpsc::channel();
        let (command_tx, command_rx) = mpsc::channel();

        let read_handle = handle.clone();
        let read_events = event_tx.clone();
        thread::Builder::new()
            .name("rtx-hdr-ui-reader".into())
            .spawn(move || read_loop(read_handle, read_events))
            .map_err(|error| format!("无法启动控制台通信线程：{error}"))?;

        let write_handle = handle.clone();
        thread::Builder::new()
            .name("rtx-hdr-ui-writer".into())
            .spawn(move || write_loop(write_handle, command_rx, event_tx))
            .map_err(|error| format!("无法启动控制台通信写入线程：{error}"))?;

        command_tx
            .send(UiCommand::Hello {
                protocol_version: PROTOCOL_VERSION,
            })
            .map_err(|_| "后台通信通道已关闭。".to_string())?;

        Ok((
            Self {
                commands: command_tx,
                _handle: handle,
            },
            event_rx,
        ))
    }

    pub fn send(&self, command: UiCommand) -> Result<(), String> {
        self.commands
            .send(command)
            .map_err(|_| "后台连接已断开。".to_string())
    }
}

pub fn activate_existing() -> bool {
    let Ok(handle) = connect_pipe() else {
        return false;
    };
    let handle = PipeHandle(handle as usize);
    let mut writer = PipeWriter(handle.raw());
    write_packet(
        &mut writer,
        &UiCommand::Hello {
            protocol_version: PROTOCOL_VERSION,
        },
    )
    .is_ok()
        && write_packet(&mut writer, &UiCommand::Activate).is_ok()
}

fn connect_pipe() -> Result<HANDLE, String> {
    let name = wide(PIPE_NAME);
    let mut last_error = 0;
    for _ in 0..40 {
        let handle = unsafe {
            CreateFileW(
                name.as_ptr(),
                FILE_GENERIC_READ_WRITE,
                0,
                null(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        if !handle.is_null() {
            return Ok(handle);
        }
        last_error = unsafe { GetLastError() };
        thread::sleep(Duration::from_millis(50));
    }
    Err(format!(
        "无法连接后台 RTX HDR 服务（Windows 错误 {last_error}）。请先启动 RtxHdrSwitch.exe。"
    ))
}

fn read_loop(handle: Arc<PipeHandle>, events: Sender<ClientEvent>) {
    let mut reader = PipeReader(handle.raw());
    let mut greeted = false;
    loop {
        let result = read_packet(&mut reader).and_then(|payload| {
            rtx_hdr_switch::ipc::decode::<HostEvent>(&payload)
                .map_err(|error| IpcError::InvalidFrame(error.to_string()))
        });
        match result {
            Ok(HostEvent::Hello {
                protocol_version, ..
            }) => {
                if protocol_version != PROTOCOL_VERSION {
                    let _ = events.send(ClientEvent::Disconnected(format!(
                        "控制台协议版本 {protocol_version} 与后台 {PROTOCOL_VERSION} 不匹配。"
                    )));
                    return;
                }
                greeted = true;
                let _ = events.send(ClientEvent::Connected);
            }
            Ok(event) if greeted => {
                if events.send(ClientEvent::Event(event)).is_err() {
                    return;
                }
            }
            Ok(_) => {
                let _ = events.send(ClientEvent::Disconnected(
                    "后台未返回有效的协议握手。".into(),
                ));
                return;
            }
            Err(error) => {
                let _ = events.send(ClientEvent::Disconnected(format!(
                    "后台连接已断开：{error}"
                )));
                return;
            }
        }
    }
}

fn write_loop(handle: Arc<PipeHandle>, commands: Receiver<UiCommand>, events: Sender<ClientEvent>) {
    let mut writer = PipeWriter(handle.raw());
    while let Ok(command) = commands.recv() {
        if let Err(error) = write_packet(&mut writer, &command) {
            let _ = events.send(ClientEvent::Disconnected(format!(
                "无法写入后台连接：{error}"
            )));
            return;
        }
    }
}

struct PipeHandle(usize);

impl PipeHandle {
    fn raw(&self) -> HANDLE {
        self.0 as HANDLE
    }
}

impl Drop for PipeHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.raw());
        }
    }
}

struct PipeReader(HANDLE);

impl Read for PipeReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut count = 0u32;
        let size = buffer.len().min(u32::MAX as usize) as u32;
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::ReadFile(
                self.0,
                buffer.as_mut_ptr(),
                size,
                &mut count,
                null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
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
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::WriteFile(
                self.0,
                buffer.as_ptr(),
                size,
                &mut count,
                null_mut(),
            )
        };
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
