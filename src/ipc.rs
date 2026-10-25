use crate::{HdrSnapshot, Settings};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

pub const PROTOCOL_VERSION: u32 = 1;
pub const PIPE_NAME: &str = r"\\.\pipe\RtxHdrSwitch.Settings.v1";
pub const MAX_FRAME_SIZE: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum UiHdrState {
    Loading,
    Ready(HdrSnapshot),
    Error(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum UiPage {
    #[default]
    Console,
    Diagnostics,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct DiagnosticsInfo {
    pub nvcpl_path: Option<String>,
    pub expected_sha256: String,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct UiSnapshot {
    pub protocol_version: u32,
    pub app_version: String,
    pub hdr: UiHdrState,
    pub settings: Settings,
    pub hotkey_registered: bool,
    pub busy: bool,
    pub diagnostics: DiagnosticsInfo,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct UiError {
    pub message: String,
    pub critical: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum UiCommand {
    Hello { protocol_version: u32 },
    ToggleHdr,
    ApplySettings { settings: Settings },
    BeginHotkeyCapture,
    EndHotkeyCapture,
    Refresh,
    ActivatePage { page: UiPage },
    Activate,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum HostEvent {
    Hello {
        protocol_version: u32,
        app_version: String,
    },
    Snapshot(UiSnapshot),
    HdrChanged(UiSnapshot),
    SettingsApplied(UiSnapshot),
    Error(UiError),
    HotkeyCaptureChanged {
        active: bool,
    },
    ActivatePage {
        page: UiPage,
    },
    Activate,
}

#[derive(Debug)]
pub enum IpcError {
    Io(io::Error),
    InvalidFrame(String),
    Json(serde_json::Error),
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::InvalidFrame(detail) => formatter.write_str(detail),
            Self::Json(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for IpcError {}

impl From<io::Error> for IpcError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for IpcError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

pub fn write_packet(writer: &mut impl Write, value: &impl Serialize) -> Result<(), IpcError> {
    let payload = serde_json::to_vec(value)?;
    if payload.is_empty() {
        return Err(IpcError::InvalidFrame("IPC payload is empty".into()));
    }
    if payload.len() > MAX_FRAME_SIZE {
        return Err(IpcError::InvalidFrame(format!(
            "IPC payload is {} bytes; maximum is {MAX_FRAME_SIZE}",
            payload.len()
        )));
    }
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(&payload);
    writer.write_all(&frame)?;
    writer.flush()?;
    Ok(())
}

pub fn read_packet(reader: &mut impl Read) -> Result<Vec<u8>, IpcError> {
    let mut header = [0u8; 4];
    reader.read_exact(&mut header)?;
    let size = u32::from_le_bytes(header) as usize;
    if size == 0 {
        return Err(IpcError::InvalidFrame("IPC payload is empty".into()));
    }
    if size > MAX_FRAME_SIZE {
        return Err(IpcError::InvalidFrame(format!(
            "IPC payload is {size} bytes; maximum is {MAX_FRAME_SIZE}"
        )));
    }
    let mut payload = vec![0u8; size];
    reader.read_exact(&mut payload)?;
    Ok(payload)
}

pub fn decode<T: for<'de> Deserialize<'de>>(payload: &[u8]) -> Result<T, IpcError> {
    Ok(serde_json::from_slice(payload)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Hotkey;
    use std::io::Cursor;

    #[test]
    fn packet_round_trip_preserves_command() {
        let command = UiCommand::ApplySettings {
            settings: Settings {
                hotkey: Hotkey::default(),
                start_at_login: false,
                notifications: true,
            },
        };
        let mut buffer = Vec::new();
        write_packet(&mut buffer, &command).unwrap();
        let mut cursor = Cursor::new(buffer);
        let payload = read_packet(&mut cursor).unwrap();
        assert_eq!(decode::<UiCommand>(&payload).unwrap(), command);
    }

    #[test]
    fn rejects_oversized_declared_frame() {
        let mut bytes = Cursor::new(((MAX_FRAME_SIZE + 1) as u32).to_le_bytes());
        let error = read_packet(&mut bytes).unwrap_err();
        assert!(error.to_string().contains("maximum"));
    }

    #[test]
    fn rejects_zero_length_frame() {
        let mut bytes = Cursor::new(0u32.to_le_bytes());
        let error = read_packet(&mut bytes).unwrap_err();
        assert!(error.to_string().contains("empty"));
    }

    #[test]
    fn rejects_oversized_payload_on_write() {
        let payload = "x".repeat(MAX_FRAME_SIZE);
        let error = write_packet(&mut Vec::new(), &payload).unwrap_err();
        assert!(matches!(error, IpcError::InvalidFrame(_)));
    }
}
