use rtx_video_manager::ipc::UiBusyKind;
use rtx_video_manager::{
    AppError, CompatibilityInfo, HdrAdjustments, HdrSnapshot, ToggleOutcome, TuningOutcome,
    VsrOutcome, VsrQuality, VsrSnapshot, HDR_INFO_SIZE, VSR_INFO_SIZE, VSR_VALUE_SIZE,
};
use std::time::{Duration, Instant};

const TOGGLE_DEBOUNCE: Duration = Duration::from_millis(300);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Intent {
    ToggleHdr { from_hotkey: bool },
    TuneHdr(HdrAdjustments),
    ToggleVsr { from_hotkey: bool },
    SetVsrQuality(VsrQuality),
}

#[derive(Clone, Debug)]
pub struct IntentGate {
    pub read_only: bool,
    pub settings_window_open: bool,
    pub worker_available: bool,
    pub vsr_supported: bool,
    pub vsr_enabled: bool,
    pub remembered_vsr_quality: VsrQuality,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Admission {
    Ignored,
    Notify(AppError),
    UiError(AppError),
    StoreVsrQuality(VsrQuality),
    Dispatch(WorkerCommand),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkerCommand {
    Read,
    ToggleHdr,
    ToggleVsr { remembered: VsrQuality },
    SetVsrQuality(VsrQuality),
    ApplyHdrTuning(HdrAdjustments),
    Shutdown,
}

#[derive(Debug)]
pub enum WorkerEvent {
    Read(Result<ReadOutcome, AppError>),
    HdrToggle(Result<ToggleOutcome, AppError>),
    Vsr(Result<VsrOutcome, AppError>),
    HdrTuning(Result<TuningOutcome, AppError>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadOutcome {
    pub hdr: HdrSnapshot,
    pub hdr_raw: [u8; HDR_INFO_SIZE],
    pub vsr: VsrSnapshot,
    pub vsr_info_raw: [u8; VSR_INFO_SIZE],
    pub vsr_value_raw: [u8; VSR_VALUE_SIZE],
    pub compatibility: CompatibilityInfo,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Completion {
    pub notification: Option<Notice>,
    pub overlay: Option<Overlay>,
    pub dialog: Option<Dialog>,
    pub ui_error: Option<AppError>,
    pub remember_vsr_quality: Option<VsrQuality>,
    pub refresh_tray: bool,
    pub refresh_settings_button: bool,
    pub broadcast_snapshot: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub text: String,
    pub is_error: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Overlay {
    pub text: String,
    pub positive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dialog {
    pub text: String,
}

#[derive(Clone, Debug, Default)]
pub struct IntentState {
    busy: Option<UiBusyKind>,
    last_hdr_toggle: Option<Instant>,
    last_vsr_toggle: Option<Instant>,
}

impl IntentState {
    pub fn busy(&self) -> Option<UiBusyKind> {
        self.busy
    }

    pub fn is_busy(&self) -> bool {
        self.busy.is_some()
    }

    pub fn admit(&mut self, intent: Intent, gate: &IntentGate, now: Instant) -> Admission {
        match intent {
            Intent::ToggleHdr { from_hotkey } => {
                if gate.read_only {
                    return Admission::Notify(AppError::ReadOnlyCompatibility);
                }
                if from_hotkey && gate.settings_window_open {
                    return Admission::Ignored;
                }
                if self.busy.is_some()
                    || self
                        .last_hdr_toggle
                        .is_some_and(|last| now.duration_since(last) < TOGGLE_DEBOUNCE)
                    || !gate.worker_available
                {
                    return Admission::Ignored;
                }
                self.busy = Some(UiBusyKind::HdrToggle);
                self.last_hdr_toggle = Some(now);
                Admission::Dispatch(WorkerCommand::ToggleHdr)
            }
            Intent::TuneHdr(adjustments) => {
                if gate.read_only {
                    return Admission::UiError(AppError::ReadOnlyCompatibility);
                }
                if self.busy.is_some() || !gate.worker_available {
                    return Admission::Ignored;
                }
                self.busy = Some(UiBusyKind::HdrTuning);
                Admission::Dispatch(WorkerCommand::ApplyHdrTuning(adjustments))
            }
            Intent::ToggleVsr { from_hotkey } => {
                if gate.read_only {
                    return Admission::Notify(AppError::ReadOnlyCompatibility);
                }
                if !gate.vsr_supported {
                    return Admission::Notify(AppError::VsrUnsupported);
                }
                if from_hotkey && gate.settings_window_open {
                    return Admission::Ignored;
                }
                if self.busy.is_some()
                    || self
                        .last_vsr_toggle
                        .is_some_and(|last| now.duration_since(last) < TOGGLE_DEBOUNCE)
                    || !gate.worker_available
                {
                    return Admission::Ignored;
                }
                self.busy = Some(UiBusyKind::VsrToggle);
                self.last_vsr_toggle = Some(now);
                Admission::Dispatch(WorkerCommand::ToggleVsr {
                    remembered: gate.remembered_vsr_quality,
                })
            }
            Intent::SetVsrQuality(quality) => {
                if !quality.is_enabled() {
                    return Admission::UiError(AppError::Settings(
                        "请选择 Auto 或 1–4 质量档位。".into(),
                    ));
                }
                if gate.read_only {
                    return Admission::UiError(AppError::ReadOnlyCompatibility);
                }
                if !gate.vsr_enabled {
                    return Admission::StoreVsrQuality(quality);
                }
                if self.busy.is_some() || !gate.worker_available {
                    return Admission::Ignored;
                }
                self.busy = Some(UiBusyKind::VsrQuality);
                Admission::Dispatch(WorkerCommand::SetVsrQuality(quality))
            }
        }
    }

    pub fn dispatch_failed(&mut self) -> bool {
        self.busy.take().is_some()
    }

    pub fn complete(
        &mut self,
        event: &WorkerEvent,
        notifications_enabled: bool,
        previous_error: Option<&str>,
    ) -> Completion {
        match event {
            WorkerEvent::Read(_) => {}
            WorkerEvent::HdrToggle(_) => {
                self.release(UiBusyKind::HdrToggle);
            }
            WorkerEvent::Vsr(_) => {
                self.release(UiBusyKind::VsrToggle);
                self.release(UiBusyKind::VsrQuality);
            }
            WorkerEvent::HdrTuning(_) => {
                self.release(UiBusyKind::HdrTuning);
            }
        }
        match event {
            WorkerEvent::Read(result) => {
                let notification = match result {
                    Ok(_) => None,
                    Err(error) => {
                        let text = error.user_message();
                        (notifications_enabled && previous_error != Some(text.as_str()))
                            .then_some(Notice {
                                text,
                                is_error: true,
                            })
                    }
                };
                Completion {

                    notification,
                    remember_vsr_quality: result
                        .as_ref()
                        .ok()
                        .and_then(|outcome| outcome.vsr.enabled.then_some(outcome.vsr.quality)),
                    refresh_tray: true,
                    refresh_settings_button: true,
                    broadcast_snapshot: true,
                    ..Completion::default()
                }
            }
            WorkerEvent::HdrToggle(result) => match result {
                Ok(outcome) => {
                    let text = if outcome.after.enabled {
                        "RTX HDR 已开启".to_string()
                    } else {
                        "RTX HDR 已关闭".to_string()
                    };
                    Completion {

                        notification: notifications_enabled.then(|| Notice {
                            text: text.clone(),
                            is_error: false,
                        }),
                        overlay: Some(Overlay {
                            text,
                            positive: outcome.after.enabled,
                        }),
                        refresh_tray: true,
                        refresh_settings_button: true,
                        broadcast_snapshot: true,
                        ..Completion::default()
                    }
                }
                Err(error) => {
                    let text = error.user_message();
                    Completion {

                        notification: Some(Notice {
                            text: text.clone(),
                            is_error: true,
                        }),
                        dialog: matches!(error, AppError::RollbackFailed).then(|| Dialog {
                            text: format!(
                                "{text}\n\n请立即打开 NVIDIA App 检查 RTX 视频 HDR 状态。"
                            ),
                        }),
                        refresh_tray: true,
                        refresh_settings_button: true,
                        broadcast_snapshot: true,
                        ..Completion::default()
                    }
                }
            },
            WorkerEvent::Vsr(result) => match result {
                Ok(outcome) => {
                    let text = if outcome.after.enabled {
                        format!(
                            "RTX VSR 已开启 · 质量 {}",
                            vsr_quality_short(outcome.after.quality)
                        )
                    } else {
                        "RTX VSR 已关闭".to_string()
                    };
                    Completion {

                        notification: notifications_enabled.then(|| Notice {
                            text: text.clone(),
                            is_error: false,
                        }),
                        overlay: Some(Overlay {
                            text,
                            positive: outcome.after.enabled,
                        }),
                        remember_vsr_quality: outcome
                            .after
                            .enabled
                            .then_some(outcome.after.quality),
                        refresh_tray: true,
                        broadcast_snapshot: true,
                        ..Completion::default()
                    }
                }
                Err(error) => {
                    let text = error.user_message();
                    Completion {

                        notification: Some(Notice {
                            text: text.clone(),
                            is_error: true,
                        }),
                        dialog: matches!(error, AppError::RollbackFailed).then(|| Dialog {
                            text: format!("{text}\n\n请立即打开 NVIDIA App 检查 VSR 状态。"),
                        }),
                        refresh_tray: true,
                        broadcast_snapshot: true,
                        ..Completion::default()
                    }
                }
            },
            WorkerEvent::HdrTuning(result) => match result {
                Ok(_) => Completion {

                    refresh_tray: true,
                    refresh_settings_button: true,
                    broadcast_snapshot: true,
                    ..Completion::default()
                },
                Err(error) => {
                    let text = error.user_message();
                    Completion {

                        notification: Some(Notice {
                            text: text.clone(),
                            is_error: true,
                        }),
                        dialog: matches!(error, AppError::RollbackFailed).then(|| Dialog {
                            text: format!(
                                "{text}\n\n请立即打开 NVIDIA App 检查 RTX 视频 HDR 状态。"
                            ),
                        }),
                        ui_error: Some(error.clone()),
                        refresh_tray: true,
                        refresh_settings_button: true,
                        broadcast_snapshot: true,
                    }
                }
            },
        }
    }

    fn release(&mut self, kind: UiBusyKind) -> bool {
        if self.busy == Some(kind) {
            self.busy = None;
            true
        } else {
            false
        }
    }
}

pub fn vsr_quality_short(quality: VsrQuality) -> &'static str {
    match quality {
        VsrQuality::Off => "关闭",
        VsrQuality::Level1 => "1",
        VsrQuality::Level2 => "2",
        VsrQuality::Level3 => "3",
        VsrQuality::Level4 => "4",
        VsrQuality::Auto => "自动",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtx_video_manager::{
        CompatibilityMode, HdrAdjustments, ToggleOutcome, TuningOutcome, VsrOutcome,
    };

    fn gate() -> IntentGate {
        IntentGate {
            read_only: false,
            settings_window_open: false,
            worker_available: true,
            vsr_supported: true,
            vsr_enabled: true,
            remembered_vsr_quality: VsrQuality::Auto,
        }
    }

    fn hdr(peak: i32) -> HdrSnapshot {
        HdrSnapshot {
            supported: true,
            enabled: false,
            active: false,
            error_code: 0,
            peak_brightness: peak,
            peak_min: 400,
            peak_max: 1440,
            middle_gray: 50,
            contrast: 0,
            saturation: 0,
        }
    }

    fn vsr(quality: VsrQuality) -> VsrSnapshot {
        VsrSnapshot {
            supported: true,
            enabled: quality.is_enabled(),
            active: false,
            quality,
            active_quality: None,
            status: 0,
        }
    }

    fn raw_hdr() -> [u8; HDR_INFO_SIZE] {
        [0; HDR_INFO_SIZE]
    }

    fn read_event(error: Option<AppError>) -> WorkerEvent {
        match error {
            Some(error) => WorkerEvent::Read(Err(error)),
            None => WorkerEvent::Read(Ok(ReadOutcome {
                hdr: hdr(1440),
                hdr_raw: raw_hdr(),
                vsr: vsr(VsrQuality::Level4),
                vsr_info_raw: [0; VSR_INFO_SIZE],
                vsr_value_raw: [0; VSR_VALUE_SIZE],
                compatibility: CompatibilityInfo {
                    mode: CompatibilityMode::WritableVerified,
                    actual_sha256: "actual".into(),
                    expected_sha256: "expected".into(),
                    dll_version: None,
                    probe_passed: true,
                    reason: None,
                },
            })),
        }
    }

    #[test]
    fn hdr_toggle_admits_once_and_debounces() {
        let mut state = IntentState::default();
        let now = Instant::now();
        assert_eq!(
            state.admit(Intent::ToggleHdr { from_hotkey: false }, &gate(), now),
            Admission::Dispatch(WorkerCommand::ToggleHdr)
        );
        assert_eq!(state.busy(), Some(UiBusyKind::HdrToggle));
        state.complete(
            &WorkerEvent::HdrToggle(Ok(ToggleOutcome {
                before: hdr(1440),
                after: hdr(1440),
                raw: raw_hdr(),
            })),
            true,
            None,
        );
        assert_eq!(
            state.admit(
                Intent::ToggleHdr { from_hotkey: false },
                &gate(),
                now + Duration::from_millis(100)
            ),
            Admission::Ignored
        );
        assert_eq!(
            state.admit(
                Intent::ToggleHdr { from_hotkey: false },
                &gate(),
                now + TOGGLE_DEBOUNCE
            ),
            Admission::Dispatch(WorkerCommand::ToggleHdr)
        );
    }

    #[test]
    fn read_only_intents_never_dispatch() {
        let mut gate = gate();
        gate.read_only = true;
        let mut state = IntentState::default();
        assert_eq!(
            state.admit(
                Intent::ToggleHdr { from_hotkey: false },
                &gate,
                Instant::now()
            ),
            Admission::Notify(AppError::ReadOnlyCompatibility)
        );
        assert_eq!(
            state.admit(
                Intent::TuneHdr(HdrAdjustments {
                    peak_brightness: 1000,
                    middle_gray: 50,
                    contrast: 0,
                    saturation: 0,
                }),
                &gate,
                Instant::now()
            ),
            Admission::UiError(AppError::ReadOnlyCompatibility)
        );
        assert!(!state.is_busy());
    }

    #[test]
    fn vsr_quality_is_stored_without_busy_when_disabled() {
        let mut gate = gate();
        gate.vsr_enabled = false;
        let mut state = IntentState::default();
        assert_eq!(
            state.admit(
                Intent::SetVsrQuality(VsrQuality::Level3),
                &gate,
                Instant::now()
            ),
            Admission::StoreVsrQuality(VsrQuality::Level3)
        );
        assert!(!state.is_busy());
    }

    #[test]
    fn completion_releases_only_the_matching_busy_kind() {
        let mut state = IntentState::default();
        assert_eq!(
            state.admit(
                Intent::TuneHdr(HdrAdjustments {
                    peak_brightness: 1000,
                    middle_gray: 50,
                    contrast: 0,
                    saturation: 0,
                }),
                &gate(),
                Instant::now()
            ),
            Admission::Dispatch(WorkerCommand::ApplyHdrTuning(HdrAdjustments {
                peak_brightness: 1000,
                middle_gray: 50,
                contrast: 0,
                saturation: 0,
            }))
        );
        let completion = state.complete(
            &WorkerEvent::HdrTuning(Ok(TuningOutcome {
                before: hdr(1440),
                after: hdr(1000),
                raw: raw_hdr(),
            })),
            true,
            None,
        );
        assert!(completion.broadcast_snapshot);
        assert!(!state.is_busy());
        let duplicate = state.complete(
            &WorkerEvent::HdrTuning(Ok(TuningOutcome {
                before: hdr(1440),
                after: hdr(1000),
                raw: raw_hdr(),
            })),
            true,
            None,
        );
        assert!(duplicate.broadcast_snapshot);
    }

    #[test]
    fn dispatch_failure_releases_busy() {
        let mut state = IntentState::default();
        assert_eq!(
            state.admit(
                Intent::ToggleVsr { from_hotkey: false },
                &gate(),
                Instant::now()
            ),
            Admission::Dispatch(WorkerCommand::ToggleVsr {
                remembered: VsrQuality::Auto
            })
        );
        assert!(state.dispatch_failed());
        assert!(!state.is_busy());
    }

    #[test]
    fn read_failure_notification_respects_notifications_and_deduplicates() {
        let mut state = IntentState::default();
        let first = state.complete(&read_event(Some(AppError::ApiFailed("get", 1))), true, None);
        assert!(first.notification.is_some());
        let same = state.complete(
            &read_event(Some(AppError::ApiFailed("get", 1))),
            true,
            Some(first.notification.as_ref().unwrap().text.as_str()),
        );
        assert!(same.notification.is_none());
        let muted = state.complete(&read_event(Some(AppError::ApiFailed("get", 2))), false, None);
        assert!(muted.notification.is_none());
    }

    #[test]
    fn critical_vsr_failure_requests_dialog_and_overlay_is_success_only() {
        let mut state = IntentState::default();
        let completion = state.complete(
            &WorkerEvent::Vsr(Err(AppError::RollbackFailed)),
            false,
            None,
        );
        assert!(completion.notification.is_some());
        assert!(completion.dialog.is_some());
        assert!(completion.overlay.is_none());
        assert!(!completion.refresh_settings_button);
    }

    #[test]
    fn successful_vsr_remembers_enabled_quality() {
        let mut state = IntentState::default();
        let completion = state.complete(
            &WorkerEvent::Vsr(Ok(VsrOutcome {
                before: vsr(VsrQuality::Auto),
                after: vsr(VsrQuality::Level4),
                raw_info: [0; VSR_INFO_SIZE],
                raw_value: [0; VSR_VALUE_SIZE],
            })),
            true,
            None,
        );
        assert_eq!(completion.remember_vsr_quality, Some(VsrQuality::Level4));
        assert_eq!(
            completion
                .overlay
                .as_ref()
                .map(|overlay| overlay.positive),
            Some(true)
        );
    }
}


