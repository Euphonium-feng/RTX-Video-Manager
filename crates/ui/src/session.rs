use rtx_video_manager::ipc::{UiBusyKind, UiHdrState, UiSnapshot, UiVsrState};
use rtx_video_manager::{HdrAdjustments, Settings, VsrQuality};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionEvent {
    Disconnected,
    Snapshot,
    StateChanged,
    CommandRejected,
    SettingsSaved,
}

#[derive(Clone, Debug)]
pub struct SessionState {
    snapshot: Option<UiSnapshot>,
    draft: Settings,
    hdr_draft: Option<HdrAdjustments>,
    vsr_choice: VsrQuality,
    dirty: bool,
    saving: bool,
    tuning_saving: bool,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            snapshot: None,
            draft: Settings::default(),
            hdr_draft: None,
            vsr_choice: VsrQuality::Auto,
            dirty: false,
            saving: false,
            tuning_saving: false,
        }
    }
}

impl SessionState {
    pub fn apply_snapshot(&mut self, snapshot: UiSnapshot, recording: bool) -> SessionEvent {
        let event = if self.saving && snapshot.busy.is_none() {
            self.saving = false;
            self.dirty = false;
            SessionEvent::SettingsSaved
        } else if self.tuning_saving && snapshot.busy != Some(UiBusyKind::HdrTuning) {
            self.tuning_saving = false;
            SessionEvent::StateChanged
        } else {
            SessionEvent::Snapshot
        };
        if !self.dirty && !recording {
            self.draft = snapshot.settings.clone();
        }
        if snapshot.busy != Some(UiBusyKind::VsrQuality) {
            self.vsr_choice = match &snapshot.vsr {
                UiVsrState::Ready(vsr) if vsr.enabled => vsr.quality,
                _ => snapshot.settings.last_vsr_quality,
            };
        }
        self.snapshot = Some(snapshot);
        if !self.tuning_saving {
            self.hdr_draft = self.hdr_snapshot().map(HdrAdjustments::from_snapshot);
        }
        event
    }

    pub fn disconnect(&mut self) {
        self.saving = false;
        self.tuning_saving = false;
    }

    pub fn reject_command(&mut self) {
        self.saving = false;
        self.tuning_saving = false;
    }

    pub fn mark_settings_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn begin_settings_save(&mut self) {
        self.saving = true;
    }

    pub fn settings_save_completed(&mut self) {
        self.saving = false;
        self.dirty = false;
    }

    pub fn draft_mut(&mut self) -> &mut Settings {
        &mut self.draft
    }

    pub fn draft(&self) -> &Settings {
        &self.draft
    }

    pub fn reset_draft(&mut self) {
        self.draft = Settings::default();
        self.dirty = true;
        self.saving = false;
    }

    pub fn saving(&self) -> bool {
        self.saving
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub fn tuning_saving(&self) -> bool {
        self.tuning_saving
    }

    pub fn snapshot(&self) -> Option<&UiSnapshot> {
        self.snapshot.as_ref()
    }

    pub fn hdr_snapshot(&self) -> Option<&rtx_video_manager::HdrSnapshot> {
        match self.snapshot.as_ref().map(|snapshot| &snapshot.hdr) {
            Some(UiHdrState::Ready(hdr)) => Some(hdr),
            _ => None,
        }
    }

    pub fn vsr_ready(&self) -> Option<&rtx_video_manager::VsrSnapshot> {
        match self.snapshot.as_ref().map(|snapshot| &snapshot.vsr) {
            Some(UiVsrState::Ready(vsr)) => Some(vsr),
            _ => None,
        }
    }

    pub fn hdr_state(&self) -> Option<&UiHdrState> {
        self.snapshot.as_ref().map(|snapshot| &snapshot.hdr)
    }

    pub fn vsr_state(&self) -> Option<&UiVsrState> {
        self.snapshot.as_ref().map(|snapshot| &snapshot.vsr)
    }

    pub fn busy_kind(&self) -> Option<UiBusyKind> {
        self.snapshot.as_ref().and_then(|snapshot| snapshot.busy)
    }

    pub fn busy(&self) -> bool {
        self.busy_kind().is_some()
    }

    pub fn hdr_adjustments(&self) -> Option<HdrAdjustments> {
        self.hdr_draft.clone()
    }

    pub fn hdr_dirty(&self) -> bool {
        self.hdr_draft
            .as_ref()
            .zip(self.hdr_snapshot())
            .is_some_and(|(draft, hdr)| *draft != HdrAdjustments::from_snapshot(hdr))
    }

    pub fn update_hdr_draft(&mut self, adjustments: HdrAdjustments) {
        self.hdr_draft = Some(adjustments);
    }

    pub fn reset_hdr_draft(&mut self) -> Option<HdrAdjustments> {
        let defaults = self.hdr_snapshot().map(HdrAdjustments::defaults_for)?;
        self.hdr_draft = Some(defaults.clone());
        Some(defaults)
    }

    pub fn begin_tuning_save(&mut self) -> Option<HdrAdjustments> {
        let adjustments = self.hdr_draft.clone()?;
        if !self.hdr_dirty() || self.busy() {
            return None;
        }
        self.tuning_saving = true;
        Some(adjustments)
    }

    pub fn vsr_choice(&self) -> VsrQuality {
        self.vsr_choice
    }

    pub fn set_vsr_choice(&mut self, quality: VsrQuality) {
        self.vsr_choice = quality;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtx_video_manager::ipc::DiagnosticsInfo;
    use rtx_video_manager::{
        CompatibilityInfo, CompatibilityMode, HdrSnapshot, VsrQuality, VsrSnapshot,
    };

    fn snapshot(settings: Settings) -> UiSnapshot {
        UiSnapshot {
            protocol_version: 4,
            app_version: "test".into(),
            hdr: UiHdrState::Ready(HdrSnapshot {
                supported: true,
                enabled: false,
                active: false,
                error_code: 0,
                peak_brightness: 1440,
                peak_min: 400,
                peak_max: 1440,
                middle_gray: 50,
                contrast: 0,
                saturation: 0,
            }),
            vsr: UiVsrState::Ready(VsrSnapshot {
                supported: true,
                enabled: true,
                active: false,
                quality: VsrQuality::Level3,
                active_quality: None,
                status: 0,
            }),
            settings,
            hotkey_registered: true,
            busy: None,
            diagnostics: DiagnosticsInfo {
                nvcpl_path: None,
                expected_sha256: String::new(),
                compatibility: Some(CompatibilityInfo {
                    mode: CompatibilityMode::WritableVerified,
                    actual_sha256: String::new(),
                    expected_sha256: String::new(),
                    dll_version: None,
                    probe_passed: true,
                    reason: None,
                }),
                raw_hdr_hex: None,
                raw_vsr_info_hex: None,
                raw_vsr_value_hex: None,
                last_error: None,
            },
        }
    }

    #[test]
    fn snapshot_does_not_clobber_dirty_draft() {
        let mut state = SessionState::default();
        state.apply_snapshot(snapshot(Settings::default()), false);
        state.begin_settings_save();
        let mut edited = state.draft().clone();
        edited.notifications = false;
        *state.draft_mut() = edited.clone();
        state.mark_settings_dirty();
        state.apply_snapshot(
            snapshot(Settings {
                notifications: true,
                ..Settings::default()
            }),
            false,
        );
        assert_eq!(state.draft(), &edited);
    }

    #[test]
    fn clean_snapshot_replaces_draft() {
        let mut state = SessionState::default();
        let mut incoming = Settings::default();
        incoming.start_at_login = false;
        state.apply_snapshot(snapshot(incoming.clone()), false);
        assert_eq!(state.draft(), &incoming);
    }

    #[test]
    fn reconnect_preserves_unsaved_draft_explicitly() {
        let mut state = SessionState::default();
        state.apply_snapshot(snapshot(Settings::default()), false);
        let mut edited = Settings::default();
        edited.notifications = false;
        *state.draft_mut() = edited.clone();
        state.mark_settings_dirty();
        state.disconnect();
        assert_eq!(state.draft(), &edited);
        assert!(!state.saving());
    }

    #[test]
    fn busy_clear_completes_pending_settings_save() {
        let mut state = SessionState::default();
        state.apply_snapshot(snapshot(Settings::default()), false);
        state.begin_settings_save();
        let mut busy = snapshot(Settings::default());
        busy.busy = Some(UiBusyKind::HdrToggle);
        state.apply_snapshot(busy, false);
        let mut clear = snapshot(Settings::default());
        clear.busy = None;
        assert_eq!(
            state.apply_snapshot(clear, false),
            SessionEvent::SettingsSaved
        );
        assert!(!state.saving());
        assert!(!state.dirty());
    }

    #[test]
    fn vsr_quality_does_not_reset_a_pending_choice() {
        let mut state = SessionState::default();
        let mut first = snapshot(Settings::default());
        first.busy = Some(UiBusyKind::VsrQuality);
        state.apply_snapshot(first, false);
        state.set_vsr_choice(VsrQuality::Level2);
        let mut in_flight = snapshot(Settings::default());
        in_flight.busy = Some(UiBusyKind::VsrQuality);
        state.apply_snapshot(in_flight, false);
        assert_eq!(state.vsr_choice(), VsrQuality::Level2);
    }

    #[test]
    fn tuning_save_is_confirmed_only_after_busy_clears() {
        let mut state = SessionState::default();
        let mut initial = snapshot(Settings::default());
        initial.busy = None;
        state.apply_snapshot(initial, false);
        let defaults = state.reset_hdr_draft().unwrap();
        let dirty = HdrAdjustments {
            peak_brightness: defaults.peak_brightness - 100,
            ..defaults
        };
        state.update_hdr_draft(dirty.clone());
        assert!(state.hdr_dirty());
        assert_eq!(state.begin_tuning_save(), Some(dirty));
        let mut saving = snapshot(Settings::default());
        saving.busy = Some(UiBusyKind::HdrTuning);
        state.apply_snapshot(saving, false);
        assert!(state.tuning_saving());
        let mut complete = snapshot(Settings::default());
        complete.busy = None;
        state.apply_snapshot(complete, false);
        assert!(!state.tuning_saving());
        assert!(!state.hdr_dirty());
    }

    #[test]
    fn reset_uses_driver_defaults_and_marks_dirty() {
        let mut state = SessionState::default();
        state.apply_snapshot(snapshot(Settings::default()), false);
        let defaults = state.reset_hdr_draft().unwrap();
        assert_eq!(defaults.peak_brightness, 1440);
        assert_eq!(defaults.middle_gray, 50);
        assert_eq!(defaults.contrast, 0);
        assert_eq!(defaults.saturation, 0);
    }

    #[test]
    fn rejected_command_ends_pending_state_without_clearing_draft() {
        let mut state = SessionState::default();
        state.apply_snapshot(snapshot(Settings::default()), false);
        let mut edited = Settings::default();
        edited.notifications = false;
        *state.draft_mut() = edited.clone();
        state.mark_settings_dirty();
        state.begin_settings_save();
        state.reject_command();
        assert!(!state.saving());
        assert_eq!(state.draft(), &edited);
    }
}
