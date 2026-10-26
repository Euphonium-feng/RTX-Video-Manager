use sha2::{Digest, Sha256};
use std::fmt;

pub mod ipc;

pub const NVCPL_RELATIVE_PATH: &str = r"NVIDIA Corporation\NVIDIA App\NvCpl\nvcpl.dll";
pub const NVCPL_SHA256_HEX: &str =
    "eee812ab8ff74975d3cc7fe77130f5766911b6ca2d9c42c1156376a6c13f79f0";
pub const HDR_SETTING_ID: u32 = 0x318;
pub const HDR_INFO_SIZE: usize = 0x30;
pub const MIDDLE_GRAY_MIN: i32 = 0;
pub const MIDDLE_GRAY_MAX: i32 = 100;
pub const TONE_ADJUSTMENT_MIN: i32 = -100;
pub const TONE_ADJUSTMENT_MAX: i32 = 100;

pub const MOD_ALT: u32 = 0x0001;
pub const MOD_CONTROL: u32 = 0x0002;
pub const MOD_SHIFT: u32 = 0x0004;
pub const MOD_WIN: u32 = 0x0008;
pub const HOTKEY_MODIFIER_MASK: u32 = MOD_ALT | MOD_CONTROL | MOD_SHIFT | MOD_WIN;

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct HdrSnapshot {
    pub supported: bool,
    pub enabled: bool,
    pub active: bool,
    pub error_code: i32,
    pub peak_brightness: i32,
    pub peak_min: i32,
    pub peak_max: i32,
    pub middle_gray: i32,
    pub contrast: i32,
    pub saturation: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum CompatibilityMode {
    WritableVerified,
    ReadOnlyUnknownBuild,
}

impl CompatibilityMode {
    pub const fn is_writable(self) -> bool {
        matches!(self, Self::WritableVerified)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct CompatibilityInfo {
    pub mode: CompatibilityMode,
    pub actual_sha256: String,
    pub expected_sha256: String,
    pub dll_version: Option<String>,
    pub probe_passed: bool,
    pub reason: Option<String>,
}

impl CompatibilityInfo {
    pub fn is_writable(&self) -> bool {
        self.mode.is_writable()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToggleOutcome {
    pub before: HdrSnapshot,
    pub after: HdrSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct HdrAdjustments {
    pub peak_brightness: i32,
    pub middle_gray: i32,
    pub contrast: i32,
    pub saturation: i32,
}

impl HdrAdjustments {
    pub fn from_snapshot(snapshot: &HdrSnapshot) -> Self {
        Self {
            peak_brightness: snapshot.peak_brightness,
            middle_gray: snapshot.middle_gray,
            contrast: snapshot.contrast,
            saturation: snapshot.saturation,
        }
    }

    pub fn defaults_for(snapshot: &HdrSnapshot) -> Self {
        Self {
            peak_brightness: snapshot.peak_max,
            middle_gray: 50,
            contrast: 0,
            saturation: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TuningOutcome {
    pub before: HdrSnapshot,
    pub after: HdrSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppError {
    NvidiaAppMissing,
    UnsupportedVersion,
    SignatureInvalid,
    NvCplLoadFailed(u32),
    MissingExport(&'static str),
    ComInitializationFailed(i32),
    InitFailed(i32),
    ApiFailed(&'static str, i32),
    UxdServiceUnavailable,
    InvalidData(&'static str),
    HdrUnsupported,
    NoHdrDisplay,
    WindowsHdrDisabled,
    SurroundEnabled,
    ReadOnlyCompatibility,
    VerificationFailed,
    RollbackFailed,
    Io(String),
    Settings(String),
}

impl AppError {
    pub fn user_message(&self) -> String {
        match self {
            Self::NvidiaAppMissing => "未找到受支持的 NVIDIA App。".into(),
            Self::UnsupportedVersion => {
                "NVIDIA App / nvcpl.dll 版本未经验证，已禁止修改 HDR。".into()
            }
            Self::SignatureInvalid => "nvcpl.dll 的数字签名无效。".into(),
            Self::NvCplLoadFailed(code) => format!("无法加载 nvcpl.dll（错误 {code}）。"),
            Self::MissingExport(name) => format!("nvcpl.dll 缺少导出：{name}。"),
            Self::ComInitializationFailed(code) => {
                format!("无法初始化 Windows COM 会话（HRESULT 0x{code:08X}）。")
            }
            Self::InitFailed(code) => format!("NVIDIA 控制接口初始化失败（{code}）。"),
            Self::ApiFailed(operation, code) => {
                format!("NVIDIA 接口 {operation} 调用失败（{code}）。")
            }
            Self::UxdServiceUnavailable => "NVIDIA 显示设置服务未运行，已禁止修改 HDR。".into(),
            Self::InvalidData(detail) => format!("NVIDIA 返回的数据无效：{detail}。"),
            Self::HdrUnsupported => "当前 GPU 或显示器不支持 RTX 视频 HDR。".into(),
            Self::NoHdrDisplay => "未检测到活动的 HDR10 显示器。".into(),
            Self::WindowsHdrDisabled => "Windows HDR 未开启。".into(),
            Self::SurroundEnabled => "NVIDIA Surround 已开启，与 RTX 视频 HDR 冲突。".into(),
            Self::ReadOnlyCompatibility => {
                "当前 NVIDIA 构建未经验证，已进入只读兼容模式并禁止写入。".into()
            }
            Self::VerificationFailed => "写入后的回读校验失败，已尝试恢复原状态。".into(),
            Self::RollbackFailed => "写入失败，且无法恢复原始 HDR 状态。".into(),
            Self::Io(detail) => detail.clone(),
            Self::Settings(detail) => detail.clone(),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.user_message())
    }
}

impl std::error::Error for AppError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Hotkey {
    pub modifiers: u32,
    pub vk: u32,
}

impl Default for Hotkey {
    fn default() -> Self {
        Self {
            modifiers: MOD_CONTROL | MOD_ALT,
            vk: b'H' as u32,
        }
    }
}

impl Hotkey {
    pub const fn console_default() -> Self {
        Self {
            modifiers: MOD_CONTROL | MOD_ALT,
            vk: b'G' as u32,
        }
    }

    pub fn validate(self) -> Result<(), AppError> {
        if self.modifiers & HOTKEY_MODIFIER_MASK == 0 {
            return Err(AppError::Settings(
                "快捷键至少需要一个修饰键（Ctrl、Alt、Shift 或 Win）。".into(),
            ));
        }
        if self.modifiers & !HOTKEY_MODIFIER_MASK != 0 {
            return Err(AppError::Settings("快捷键包含未知修饰键。".into()));
        }
        if self.vk == 0 || self.vk > 0xff || is_modifier_vk(self.vk) {
            return Err(AppError::Settings("快捷键主键无效。".into()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Settings {
    pub hotkey: Hotkey,
    #[serde(default = "default_show_console_hotkey")]
    pub show_console_hotkey: Hotkey,
    pub start_at_login: bool,
    pub notifications: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: Hotkey::default(),
            show_console_hotkey: default_show_console_hotkey(),
            start_at_login: true,
            notifications: true,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), AppError> {
        self.hotkey.validate()?;
        self.show_console_hotkey.validate()?;
        if self.hotkey == self.show_console_hotkey {
            return Err(AppError::Settings(
                "切换 HDR 和唤起控制台的快捷键不能相同。".into(),
            ));
        }
        Ok(())
    }
}

fn default_show_console_hotkey() -> Hotkey {
    Hotkey::console_default()
}

pub trait NvTransport {
    fn read_info(&mut self) -> Result<[u8; HDR_INFO_SIZE], AppError>;
    fn write_info(&mut self, info: &[u8; HDR_INFO_SIZE]) -> Result<(), AppError>;
}

pub fn parse_hdr_info(raw: &[u8; HDR_INFO_SIZE]) -> Result<HdrSnapshot, AppError> {
    let supported = read_bool(raw, 0x00)?;
    let enabled = read_bool(raw, 0x04)?;
    let error_code = read_i32(raw, 0x08);
    let active = read_bool(raw, 0x0c)?;
    let peak_brightness = read_i32(raw, 0x10);
    let middle_gray = read_i32(raw, 0x14);
    let contrast = read_i32(raw, 0x18) - 100;
    let saturation = read_i32(raw, 0x1c) - 100;
    let peak_max = read_i32(raw, 0x20);
    let peak_min = read_i32(raw, 0x24);

    if supported && (peak_min < 0 || peak_max < peak_min) {
        return Err(AppError::InvalidData("HDR 亮度范围无效"));
    }
    if !error_code_is_known(error_code) {
        return Err(AppError::InvalidData("未知的 HDR 错误码"));
    }

    Ok(HdrSnapshot {
        supported,
        enabled,
        active,
        error_code,
        peak_brightness,
        peak_min,
        peak_max,
        middle_gray,
        contrast,
        saturation,
    })
}

pub fn encode_hdr_write(original: &[u8; HDR_INFO_SIZE], enabled: bool) -> [u8; HDR_INFO_SIZE] {
    let mut output = *original;
    write_i32(&mut output, 0x04, if enabled { 1 } else { 0 });
    write_i32(&mut output, 0x08, 0);
    write_i32(&mut output, 0x0c, 0);
    output
}

pub fn encode_hdr_tuning(
    original: &[u8; HDR_INFO_SIZE],
    adjustments: &HdrAdjustments,
) -> [u8; HDR_INFO_SIZE] {
    let mut output = *original;
    write_i32(&mut output, 0x10, adjustments.peak_brightness);
    write_i32(&mut output, 0x14, adjustments.middle_gray);
    write_i32(&mut output, 0x18, adjustments.contrast + 100);
    write_i32(&mut output, 0x1c, adjustments.saturation + 100);
    write_i32(&mut output, 0x08, 0);
    write_i32(&mut output, 0x0c, 0);
    output
}

pub fn validate_for_write(snapshot: &HdrSnapshot) -> Result<(), AppError> {
    if !snapshot.supported {
        return Err(AppError::HdrUnsupported);
    }
    match snapshot.error_code {
        0 => Ok(()),
        1 => Err(AppError::NoHdrDisplay),
        2 => Err(AppError::WindowsHdrDisabled),
        3 => Err(AppError::SurroundEnabled),
        _ => Err(AppError::InvalidData("未知的 HDR 错误码")),
    }
}

pub fn classify_compatibility(
    hash_matches: bool,
    signature_valid: bool,
    probe_valid: bool,
) -> Result<CompatibilityMode, AppError> {
    if !signature_valid {
        return Err(AppError::SignatureInvalid);
    }
    if !probe_valid {
        return Err(AppError::InvalidData("HDR 读取探针未通过"));
    }
    Ok(if hash_matches {
        CompatibilityMode::WritableVerified
    } else {
        CompatibilityMode::ReadOnlyUnknownBuild
    })
}

pub fn validate_hdr_adjustments(
    snapshot: &HdrSnapshot,
    adjustments: &HdrAdjustments,
) -> Result<(), AppError> {
    validate_for_write(snapshot)?;
    if adjustments.peak_brightness < snapshot.peak_min
        || adjustments.peak_brightness > snapshot.peak_max
    {
        return Err(AppError::Settings(format!(
            "峰值亮度必须位于 {} 到 {} nits 之间。",
            snapshot.peak_min, snapshot.peak_max
        )));
    }
    if !(MIDDLE_GRAY_MIN..=MIDDLE_GRAY_MAX).contains(&adjustments.middle_gray) {
        return Err(AppError::Settings(format!(
            "中间灰必须位于 {MIDDLE_GRAY_MIN} 到 {MIDDLE_GRAY_MAX} 之间。"
        )));
    }
    for (name, value) in [
        ("对比度", adjustments.contrast),
        ("饱和度", adjustments.saturation),
    ] {
        if !(TONE_ADJUSTMENT_MIN..=TONE_ADJUSTMENT_MAX).contains(&value) {
            return Err(AppError::Settings(format!(
                "{name}必须位于 {TONE_ADJUSTMENT_MIN} 到 {TONE_ADJUSTMENT_MAX} 之间。"
            )));
        }
    }
    Ok(())
}

pub fn toggle_with_transport<T: NvTransport>(transport: &mut T) -> Result<ToggleOutcome, AppError> {
    let original = transport.read_info()?;
    let before = parse_hdr_info(&original)?;
    validate_for_write(&before)?;

    let desired = !before.enabled;
    let desired_raw = encode_hdr_write(&original, desired);
    if let Err(write_error) = transport.write_info(&desired_raw) {
        if matches!(&write_error, AppError::ReadOnlyCompatibility) {
            return Err(write_error);
        }
        return recover_after_error(transport, &original, &before, write_error);
    }

    let after_raw = match transport.read_info() {
        Ok(value) => value,
        Err(_) => {
            return recover_after_error(
                transport,
                &original,
                &before,
                AppError::VerificationFailed,
            );
        }
    };
    let after = match parse_hdr_info(&after_raw) {
        Ok(value) => value,
        Err(_) => {
            return recover_after_error(
                transport,
                &original,
                &before,
                AppError::VerificationFailed,
            );
        }
    };
    if after.enabled == desired && persistent_fields_equal(&before, &after) {
        return Ok(ToggleOutcome { before, after });
    }

    recover_after_error(transport, &original, &before, AppError::VerificationFailed)
}

pub fn tune_with_transport<T: NvTransport>(
    transport: &mut T,
    adjustments: &HdrAdjustments,
) -> Result<TuningOutcome, AppError> {
    let original = transport.read_info()?;
    let before = parse_hdr_info(&original)?;
    validate_hdr_adjustments(&before, adjustments)?;

    let desired_raw = encode_hdr_tuning(&original, adjustments);
    if let Err(write_error) = transport.write_info(&desired_raw) {
        if matches!(&write_error, AppError::ReadOnlyCompatibility) {
            return Err(write_error);
        }
        return recover_tuning_after_error(transport, &original, &before, write_error);
    }

    let after_raw = match transport.read_info() {
        Ok(value) => value,
        Err(_) => {
            return recover_tuning_after_error(
                transport,
                &original,
                &before,
                AppError::VerificationFailed,
            );
        }
    };
    let after = match parse_hdr_info(&after_raw) {
        Ok(value) => value,
        Err(_) => {
            return recover_tuning_after_error(
                transport,
                &original,
                &before,
                AppError::VerificationFailed,
            );
        }
    };
    if tuning_matches(&before, &after, adjustments) {
        return Ok(TuningOutcome { before, after });
    }

    recover_tuning_after_error(transport, &original, &before, AppError::VerificationFailed)
}

fn recover_after_error<T: NvTransport>(
    transport: &mut T,
    original: &[u8; HDR_INFO_SIZE],
    before: &HdrSnapshot,
    original_error: AppError,
) -> Result<ToggleOutcome, AppError> {
    if transport.write_info(original).is_err() {
        return Err(AppError::RollbackFailed);
    }
    let restored = transport
        .read_info()
        .ok()
        .and_then(|raw| parse_hdr_info(&raw).ok());
    match restored {
        Some(value)
            if value.enabled == before.enabled && persistent_fields_equal(before, &value) =>
        {
            Err(original_error)
        }
        _ => Err(AppError::RollbackFailed),
    }
}

fn recover_tuning_after_error<T: NvTransport>(
    transport: &mut T,
    original: &[u8; HDR_INFO_SIZE],
    before: &HdrSnapshot,
    original_error: AppError,
) -> Result<TuningOutcome, AppError> {
    if transport.write_info(original).is_err() {
        return Err(AppError::RollbackFailed);
    }
    let restored = transport
        .read_info()
        .ok()
        .and_then(|raw| parse_hdr_info(&raw).ok());
    match restored {
        Some(value)
            if persistent_fields_equal(before, &value) && value.enabled == before.enabled =>
        {
            Err(original_error)
        }
        _ => Err(AppError::RollbackFailed),
    }
}

fn tuning_matches(before: &HdrSnapshot, after: &HdrSnapshot, desired: &HdrAdjustments) -> bool {
    after.supported
        && after.error_code == 0
        && after.enabled == before.enabled
        && after.peak_min == before.peak_min
        && after.peak_max == before.peak_max
        && after.peak_brightness == desired.peak_brightness
        && after.middle_gray == desired.middle_gray
        && after.contrast == desired.contrast
        && after.saturation == desired.saturation
}

pub fn persistent_fields_equal(left: &HdrSnapshot, right: &HdrSnapshot) -> bool {
    left.peak_brightness == right.peak_brightness
        && left.peak_max == right.peak_max
        && left.peak_min == right.peak_min
        && left.middle_gray == right.middle_gray
        && left.contrast == right.contrast
        && left.saturation == right.saturation
}

pub fn format_hotkey(hotkey: Hotkey) -> String {
    let mut parts = Vec::with_capacity(5);
    if hotkey.modifiers & MOD_CONTROL != 0 {
        parts.push("Ctrl".to_string());
    }
    if hotkey.modifiers & MOD_ALT != 0 {
        parts.push("Alt".to_string());
    }
    if hotkey.modifiers & MOD_SHIFT != 0 {
        parts.push("Shift".to_string());
    }
    if hotkey.modifiers & MOD_WIN != 0 {
        parts.push("Win".to_string());
    }
    parts.push(key_label(hotkey.vk));
    parts.join("+")
}

pub fn key_label(vk: u32) -> String {
    match vk {
        0x08 => "Backspace".into(),
        0x09 => "Tab".into(),
        0x0d => "Enter".into(),
        0x1b => "Esc".into(),
        0x20 => "Space".into(),
        0x21 => "PageUp".into(),
        0x22 => "PageDown".into(),
        0x23 => "End".into(),
        0x24 => "Home".into(),
        0x25 => "Left".into(),
        0x26 => "Up".into(),
        0x27 => "Right".into(),
        0x28 => "Down".into(),
        0x2d => "Insert".into(),
        0x2e => "Delete".into(),
        0x30..=0x39 | 0x41..=0x5a => char::from(vk as u8).to_string(),
        0x70..=0x87 => format!("F{}", vk - 0x6f),
        0xba => ";".into(),
        0xbb => "=".into(),
        0xbc => ",".into(),
        0xbd => "-".into(),
        0xbe => ".".into(),
        0xbf => "/".into(),
        0xc0 => "`".into(),
        0xdb => "[".into(),
        0xdc => "\\".into(),
        0xdd => "]".into(),
        0xde => "'".into(),
        _ => format!("VK_{vk:02X}"),
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut result = String::with_capacity(64);
    for byte in digest {
        use fmt::Write as _;
        let _ = write!(result, "{byte:02x}");
    }
    result
}

pub fn hex_lower(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use fmt::Write as _;
        let _ = write!(result, "{byte:02x}");
    }
    result
}

pub fn is_verified_nvcpl(bytes: &[u8]) -> bool {
    sha256_hex(bytes) == NVCPL_SHA256_HEX
}

fn error_code_is_known(value: i32) -> bool {
    matches!(value, 0..=3)
}

fn is_modifier_vk(vk: u32) -> bool {
    matches!(vk, 0x10 | 0x11 | 0x12 | 0x5b | 0x5c | 0xa0..=0xa5)
}

fn read_i32(raw: &[u8; HDR_INFO_SIZE], offset: usize) -> i32 {
    i32::from_le_bytes([
        raw[offset],
        raw[offset + 1],
        raw[offset + 2],
        raw[offset + 3],
    ])
}

fn read_bool(raw: &[u8; HDR_INFO_SIZE], offset: usize) -> Result<bool, AppError> {
    match read_i32(raw, offset) {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(AppError::InvalidData("布尔字段不是 0 或 1")),
    }
}

fn write_i32(raw: &mut [u8; HDR_INFO_SIZE], offset: usize, value: i32) {
    raw[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct FakeTransport {
        reads: VecDeque<Result<[u8; HDR_INFO_SIZE], AppError>>,
        writes: Vec<[u8; HDR_INFO_SIZE]>,
        write_results: VecDeque<Result<(), AppError>>,
    }

    impl FakeTransport {
        fn new(reads: Vec<Result<[u8; HDR_INFO_SIZE], AppError>>) -> Self {
            Self {
                reads: reads.into(),
                writes: Vec::new(),
                write_results: VecDeque::new(),
            }
        }

        fn with_write_results(mut self, results: Vec<Result<(), AppError>>) -> Self {
            self.write_results = results.into();
            self
        }
    }

    impl NvTransport for FakeTransport {
        fn read_info(&mut self) -> Result<[u8; HDR_INFO_SIZE], AppError> {
            self.reads
                .pop_front()
                .unwrap_or(Err(AppError::InvalidData("unexpected read")))
        }

        fn write_info(&mut self, info: &[u8; HDR_INFO_SIZE]) -> Result<(), AppError> {
            self.writes.push(*info);
            self.write_results.pop_front().unwrap_or(Ok(()))
        }
    }

    fn sample_raw(enabled: bool) -> [u8; HDR_INFO_SIZE] {
        let mut raw = [0u8; HDR_INFO_SIZE];
        write_i32(&mut raw, 0x00, 1);
        write_i32(&mut raw, 0x04, if enabled { 1 } else { 0 });
        write_i32(&mut raw, 0x08, 0);
        write_i32(&mut raw, 0x0c, 0);
        write_i32(&mut raw, 0x10, 1440);
        write_i32(&mut raw, 0x14, 50);
        write_i32(&mut raw, 0x18, 100);
        write_i32(&mut raw, 0x1c, 100);
        write_i32(&mut raw, 0x20, 1440);
        write_i32(&mut raw, 0x24, 400);
        raw
    }

    #[test]
    fn parses_verified_snapshot() {
        let snapshot = parse_hdr_info(&sample_raw(false)).unwrap();
        assert!(snapshot.supported);
        assert!(!snapshot.enabled);
        assert!(!snapshot.active);
        assert_eq!(snapshot.error_code, 0);
        assert_eq!(snapshot.peak_brightness, 1440);
        assert_eq!(snapshot.peak_min, 400);
        assert_eq!(snapshot.peak_max, 1440);
        assert_eq!(snapshot.contrast, 0);
        assert_eq!(snapshot.saturation, 0);
    }

    #[test]
    fn classifies_verified_and_unknown_signed_builds() {
        assert_eq!(
            classify_compatibility(true, true, true),
            Ok(CompatibilityMode::WritableVerified)
        );
        assert_eq!(
            classify_compatibility(false, true, true),
            Ok(CompatibilityMode::ReadOnlyUnknownBuild)
        );
        assert_eq!(
            classify_compatibility(true, false, true),
            Err(AppError::SignatureInvalid)
        );
        assert_eq!(
            classify_compatibility(false, true, false),
            Err(AppError::InvalidData("HDR 读取探针未通过"))
        );
    }

    #[test]
    fn encodes_raw_hdr_bytes_as_lowercase_hex() {
        assert_eq!(hex_lower(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff");
    }

    #[test]
    fn derives_dynamic_hdr_defaults_from_snapshot() {
        let snapshot = parse_hdr_info(&sample_raw(false)).unwrap();
        assert_eq!(
            HdrAdjustments::defaults_for(&snapshot),
            HdrAdjustments {
                peak_brightness: 1440,
                middle_gray: 50,
                contrast: 0,
                saturation: 0,
            }
        );
    }

    #[test]
    fn encoding_changes_only_target_and_status_fields() {
        let original = sample_raw(false);
        let encoded = encode_hdr_write(&original, true);
        assert_eq!(read_i32(&encoded, 0x04), 1);
        assert_eq!(read_i32(&encoded, 0x08), 0);
        assert_eq!(read_i32(&encoded, 0x0c), 0);
        for offset in [0x00, 0x10, 0x14, 0x18, 0x1c, 0x20, 0x24] {
            assert_eq!(&encoded[offset..offset + 4], &original[offset..offset + 4]);
        }
    }

    #[test]
    fn encodes_hdr_adjustments_without_changing_enabled_or_display_limits() {
        let original = sample_raw(true);
        let adjustments = HdrAdjustments {
            peak_brightness: 1000,
            middle_gray: 42,
            contrast: 12,
            saturation: -8,
        };
        let encoded = encode_hdr_tuning(&original, &adjustments);
        assert_eq!(read_i32(&encoded, 0x04), 1);
        assert_eq!(read_i32(&encoded, 0x08), 0);
        assert_eq!(read_i32(&encoded, 0x0c), 0);
        assert_eq!(read_i32(&encoded, 0x10), 1000);
        assert_eq!(read_i32(&encoded, 0x14), 42);
        assert_eq!(read_i32(&encoded, 0x18), 112);
        assert_eq!(read_i32(&encoded, 0x1c), 92);
        assert_eq!(read_i32(&encoded, 0x20), 1440);
        assert_eq!(read_i32(&encoded, 0x24), 400);
    }

    #[test]
    fn tuning_transaction_updates_all_four_fields() {
        let adjustments = HdrAdjustments {
            peak_brightness: 1000,
            middle_gray: 42,
            contrast: 12,
            saturation: -8,
        };
        let original = sample_raw(true);
        let tuned = encode_hdr_tuning(&original, &adjustments);
        let mut transport = FakeTransport::new(vec![Ok(original), Ok(tuned)]);
        let result = tune_with_transport(&mut transport, &adjustments).unwrap();
        assert_eq!(result.after.peak_brightness, 1000);
        assert_eq!(result.after.middle_gray, 42);
        assert_eq!(result.after.contrast, 12);
        assert_eq!(result.after.saturation, -8);
        assert!(result.after.enabled);
        assert_eq!(transport.writes, vec![tuned]);
    }

    #[test]
    fn tuning_rejects_out_of_range_values_before_writing() {
        let snapshot = parse_hdr_info(&sample_raw(false)).unwrap();
        for adjustments in [
            HdrAdjustments {
                peak_brightness: 399,
                middle_gray: 50,
                contrast: 0,
                saturation: 0,
            },
            HdrAdjustments {
                peak_brightness: 1441,
                middle_gray: 50,
                contrast: 0,
                saturation: 0,
            },
            HdrAdjustments {
                peak_brightness: 1000,
                middle_gray: 101,
                contrast: 0,
                saturation: 0,
            },
            HdrAdjustments {
                peak_brightness: 1000,
                middle_gray: 50,
                contrast: -101,
                saturation: 0,
            },
            HdrAdjustments {
                peak_brightness: 1000,
                middle_gray: 50,
                contrast: 0,
                saturation: 101,
            },
        ] {
            assert!(validate_hdr_adjustments(&snapshot, &adjustments).is_err());
        }
    }

    #[test]
    fn tuning_readback_mismatch_restores_original_bytes() {
        let adjustments = HdrAdjustments {
            peak_brightness: 1000,
            middle_gray: 42,
            contrast: 12,
            saturation: -8,
        };
        let original = sample_raw(false);
        let wrong = sample_raw(false);
        let mut transport = FakeTransport::new(vec![Ok(original), Ok(wrong), Ok(original)]);
        assert_eq!(
            tune_with_transport(&mut transport, &adjustments),
            Err(AppError::VerificationFailed)
        );
        assert_eq!(transport.writes.len(), 2);
        assert_eq!(transport.writes[1], original);
    }

    #[test]
    fn toggles_off_to_on() {
        let mut transport = FakeTransport::new(vec![Ok(sample_raw(false)), Ok(sample_raw(true))]);
        let result = toggle_with_transport(&mut transport).unwrap();
        assert!(!result.before.enabled);
        assert!(result.after.enabled);
        assert_eq!(transport.writes.len(), 1);
        assert_eq!(read_i32(&transport.writes[0], 0x04), 1);
    }

    #[test]
    fn rejects_unsupported_before_writing() {
        let mut raw = sample_raw(false);
        write_i32(&mut raw, 0x00, 0);
        let mut transport = FakeTransport::new(vec![Ok(raw)]);
        assert_eq!(
            toggle_with_transport(&mut transport),
            Err(AppError::HdrUnsupported)
        );
        assert!(transport.writes.is_empty());
    }

    #[test]
    fn rejects_windows_hdr_disabled_before_writing() {
        let mut raw = sample_raw(false);
        write_i32(&mut raw, 0x08, 2);
        let mut transport = FakeTransport::new(vec![Ok(raw)]);
        assert_eq!(
            toggle_with_transport(&mut transport),
            Err(AppError::WindowsHdrDisabled)
        );
        assert!(transport.writes.is_empty());
    }

    #[test]
    fn restores_after_readback_mismatch() {
        let original = sample_raw(false);
        let mut wrong = sample_raw(false);
        write_i32(&mut wrong, 0x10, 1400);
        let mut transport = FakeTransport::new(vec![Ok(original), Ok(wrong), Ok(original)]);
        assert_eq!(
            toggle_with_transport(&mut transport),
            Err(AppError::VerificationFailed)
        );
        assert_eq!(transport.writes.len(), 2);
        assert_eq!(transport.writes[0], encode_hdr_write(&original, true));
        assert_eq!(transport.writes[1], original);
    }

    #[test]
    fn reports_rollback_failure() {
        let original = sample_raw(false);
        let wrong = sample_raw(false);
        let mut transport = FakeTransport::new(vec![Ok(original), Ok(wrong)])
            .with_write_results(vec![Ok(()), Err(AppError::ApiFailed("set", 9))]);
        assert_eq!(
            toggle_with_transport(&mut transport),
            Err(AppError::RollbackFailed)
        );
    }

    #[test]
    fn attempts_rollback_after_write_error() {
        let original = sample_raw(false);
        let mut transport = FakeTransport::new(vec![Ok(original), Ok(original)])
            .with_write_results(vec![Err(AppError::ApiFailed("set", 7)), Ok(())]);
        assert_eq!(
            toggle_with_transport(&mut transport),
            Err(AppError::ApiFailed("set", 7))
        );
        assert_eq!(transport.writes.len(), 2);
        assert_eq!(transport.writes[1], original);
    }

    #[test]
    fn read_only_write_error_does_not_attempt_rollback() {
        let adjustments = HdrAdjustments {
            peak_brightness: 1400,
            middle_gray: 45,
            contrast: 5,
            saturation: -5,
        };
        let original = sample_raw(false);
        let mut transport = FakeTransport::new(vec![Ok(original)])
            .with_write_results(vec![Err(AppError::ReadOnlyCompatibility), Ok(())]);
        assert_eq!(
            tune_with_transport(&mut transport, &adjustments),
            Err(AppError::ReadOnlyCompatibility)
        );
        assert_eq!(transport.writes.len(), 1);

        let mut transport = FakeTransport::new(vec![Ok(original)])
            .with_write_results(vec![Err(AppError::ReadOnlyCompatibility), Ok(())]);
        assert_eq!(
            toggle_with_transport(&mut transport),
            Err(AppError::ReadOnlyCompatibility)
        );
        assert_eq!(transport.writes.len(), 1);
    }

    #[test]
    fn validates_and_formats_hotkeys() {
        let hotkey = Hotkey::default();
        hotkey.validate().unwrap();
        assert_eq!(format_hotkey(hotkey), "Ctrl+Alt+H");
        assert!(Hotkey {
            modifiers: 0,
            vk: b'H' as u32
        }
        .validate()
        .is_err());
        assert!(Hotkey {
            modifiers: MOD_CONTROL,
            vk: 0x11
        }
        .validate()
        .is_err());
        assert_eq!(format_hotkey(Hotkey::console_default()), "Ctrl+Alt+G");
        let duplicate = Settings {
            show_console_hotkey: Hotkey::default(),
            ..Settings::default()
        };
        assert!(duplicate.validate().is_err());
    }

    #[test]
    fn computes_known_sha256() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
