#![windows_subsystem = "windows"]

mod ipc_client;

use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_kit::component::input::{InputEvent, InputState, NumberInput};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState, SliderValue};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable as _, Theme, ThemeMode, TitleBar, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use ipc_client::{ClientEvent, IpcClient};
use rtx_video_manager::ipc::{
    HostEvent, UiBusyKind, UiCommand, UiError, UiHdrState, UiPage, UiSnapshot, UiVsrState,
};
use rtx_video_manager::{
    AppError, CompatibilityInfo, HdrAdjustments, Hotkey, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN,
    NVCPL_SHA256_HEX, Settings, VsrQuality, VsrSnapshot, format_hotkey,
};
use std::fs::OpenOptions;
use std::io::Write as _;
use std::ptr::null;
use std::sync::mpsc::Receiver;
use std::time::Duration;
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError};
use windows_sys::Win32::System::Threading::CreateMutexW;

const INK: u32 = 0x0B1215;
const PANEL: u32 = 0x121C20;
const RAISED: u32 = 0x19262C;
const RULE: u32 = 0x2A4046;
const TEXT: u32 = 0xEDF5F4;
const MUTED: u32 = 0x8CA1A5;
const SIGNAL: u32 = 0x65DDE6;
const SIGNAL_HOVER: u32 = 0x84E9F0;
const WARM: u32 = 0xF0B35C;
const DANGER: u32 = 0xFF7B70;
const SUBTLE: u32 = 0x64797D;
const UI_MUTEX: &str = r"Local\RtxHdrSwitch.SettingsWindow.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HotkeyField {
    Hdr,
    Vsr,
    Console,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TuningField {
    Peak,
    MiddleGray,
    Contrast,
    Saturation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TuningSource {
    Slider,
    Input,
    Snapshot,
}

struct ConsoleApp {
    snapshot: Option<UiSnapshot>,
    draft: Settings,
    hdr_draft: Option<HdrAdjustments>,
    hdr_dirty: bool,
    vsr_choice: VsrQuality,
    tuning_saving: bool,
    page: UiPage,
    recording: Option<HotkeyField>,
    saving: bool,
    dirty: bool,
    connection_error: Option<String>,
    transient_error: Option<String>,
    saved_note: Option<String>,
    ipc: Option<IpcClient>,
    focus_handle: FocusHandle,
    peak_slider: Entity<SliderState>,
    middle_gray_slider: Entity<SliderState>,
    contrast_slider: Entity<SliderState>,
    saturation_slider: Entity<SliderState>,
    peak_input: Entity<InputState>,
    middle_gray_input: Entity<InputState>,
    contrast_input: Entity<InputState>,
    saturation_input: Entity<InputState>,
    subscriptions: Vec<Subscription>,
}

impl ConsoleApp {
    fn new(
        connection: Result<(IpcClient, Receiver<ClientEvent>), String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let peak_slider = cx.new(|_| SliderState::new().min(0.).max(1000.).step(1.));
        let middle_gray_slider = cx.new(|_| SliderState::new().min(0.).max(100.).step(1.));
        let contrast_slider = cx.new(|_| SliderState::new().min(-100.).max(100.).step(1.));
        let saturation_slider = cx.new(|_| SliderState::new().min(-100.).max(100.).step(1.));
        let peak_input = cx.new(|cx| InputState::new(window, cx).step(1.).min(0.).max(5000.));
        let middle_gray_input = cx.new(|cx| InputState::new(window, cx).step(1.).min(0.).max(100.));
        let contrast_input = cx.new(|cx| InputState::new(window, cx).step(1.).min(-100.).max(100.));
        let saturation_input =
            cx.new(|cx| InputState::new(window, cx).step(1.).min(-100.).max(100.));
        let mut app = Self {
            snapshot: None,
            draft: Settings::default(),
            hdr_draft: None,
            hdr_dirty: false,
            vsr_choice: VsrQuality::Auto,
            tuning_saving: false,
            page: UiPage::Overview,
            recording: None,
            saving: false,
            dirty: false,
            connection_error: None,
            transient_error: None,
            saved_note: None,
            ipc: None,
            focus_handle: cx.focus_handle(),
            peak_slider,
            middle_gray_slider,
            contrast_slider,
            saturation_slider,
            peak_input,
            middle_gray_input,
            contrast_input,
            saturation_input,
            subscriptions: Vec::new(),
        };
        app.bind_tuning_controls(window, cx);
        app.apply_connection(connection, window, cx);
        window.activate_window();
        app
    }

    fn bind_tuning_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (field, slider) in [
            (TuningField::Peak, self.peak_slider.clone()),
            (TuningField::MiddleGray, self.middle_gray_slider.clone()),
            (TuningField::Contrast, self.contrast_slider.clone()),
            (TuningField::Saturation, self.saturation_slider.clone()),
        ] {
            self.subscriptions.push(cx.subscribe_in(
                &slider,
                window,
                move |app, _, event: &SliderEvent, window, cx| {
                    let value = match event {
                        SliderEvent::Change(value) | SliderEvent::Release(value) => value.end(),
                    };
                    app.set_tuning_from_slider(field, value, window, cx);
                },
            ));
        }
        for (field, input) in [
            (TuningField::Peak, self.peak_input.clone()),
            (TuningField::MiddleGray, self.middle_gray_input.clone()),
            (TuningField::Contrast, self.contrast_input.clone()),
            (TuningField::Saturation, self.saturation_input.clone()),
        ] {
            self.subscriptions.push(cx.subscribe_in(
                &input,
                window,
                move |app, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        app.set_tuning_from_input(field, window, cx);
                    }
                },
            ));
        }
    }

    fn apply_connection(
        &mut self,
        connection: Result<(IpcClient, Receiver<ClientEvent>), String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match connection {
            Ok((client, events)) => {
                log_line("console connected to host");
                self.ipc = Some(client);
                self.connection_error = None;
                self.start_event_pump(events, window, cx);
            }
            Err(error) => {
                log_line(&format!("console connection failed: {error}"));
                self.ipc = None;
                self.connection_error = Some(error);
            }
        }
    }

    fn start_event_pump(
        &self,
        events: Receiver<ClientEvent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            loop {
                let mut disconnected = None;
                while let Ok(event) = events.try_recv() {
                    if matches!(&event, ClientEvent::Disconnected(_)) {
                        disconnected = Some(event);
                        break;
                    }
                    if this
                        .update_in(cx, |app, window, cx| {
                            app.handle_client_event(event, window, cx);
                            cx.notify();
                        })
                        .is_err()
                    {
                        return;
                    }
                }
                if let Some(event) = disconnected {
                    if this
                        .update_in(cx, |app, window, cx| {
                            app.handle_client_event(event, window, cx);
                            cx.notify();
                        })
                        .is_err()
                    {
                        return;
                    }
                    return;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(40))
                    .await;
            }
        })
        .detach();
    }

    fn handle_client_event(
        &mut self,
        event: ClientEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            ClientEvent::Connected => {
                log_line("console IPC handshake complete");
                self.connection_error = None;
                self.transient_error = None;
            }
            ClientEvent::Disconnected(message) => {
                log_line(&format!("console disconnected: {message}"));
                self.connection_error = Some(message);
                self.recording = None;
                self.saving = false;
                self.tuning_saving = false;
            }
            ClientEvent::Event(event) => match *event {
                HostEvent::Hello { .. } => {}
                HostEvent::Snapshot(snapshot) => {
                    self.apply_snapshot(snapshot, window, cx);
                }
                HostEvent::StateChanged(snapshot) => {
                    if self.tuning_saving && snapshot.busy != Some(UiBusyKind::HdrTuning) {
                        self.tuning_saving = false;
                        self.hdr_dirty = false;
                    }
                    if self.saving && snapshot.busy.is_none() {
                        self.dirty = false;
                        self.saving = false;
                        self.saved_note = Some("设置已保存".into());
                    }
                    self.apply_snapshot(snapshot, window, cx);
                }
                HostEvent::Error(UiError { message, critical }) => {
                    self.saving = false;
                    self.tuning_saving = false;
                    self.transient_error = Some(message.clone());
                    if critical {
                        self.connection_error = Some(message);
                    }
                }
                HostEvent::HotkeyCaptureChanged { active } => {
                    if !active {
                        self.recording = None;
                    }
                }
                HostEvent::ActivatePage { page } => {
                    self.page = page;
                    cx.activate(true);
                }
                HostEvent::ToggleConsoleVisibility => {
                    if window.is_window_active() {
                        window.remove_window();
                    } else {
                        window.activate_window();
                        cx.activate(true);
                    }
                }
                HostEvent::Activate => {
                    cx.activate(true);
                }
            },
        }
    }

    fn apply_snapshot(
        &mut self,
        snapshot: UiSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.dirty && self.recording.is_none() {
            self.draft = snapshot.settings.clone();
        }
        let hdr = match &snapshot.hdr {
            UiHdrState::Ready(hdr) => Some(hdr.clone()),
            _ => None,
        };
        if snapshot.busy != Some(UiBusyKind::VsrQuality) {
            self.vsr_choice = match &snapshot.vsr {
                UiVsrState::Ready(vsr) if vsr.enabled => vsr.quality,
                _ => snapshot.settings.last_vsr_quality,
            };
        }
        self.snapshot = Some(snapshot);
        if !self.hdr_dirty && !self.tuning_saving {
            self.hdr_draft = hdr.as_ref().map(HdrAdjustments::from_snapshot);
            self.sync_tuning_controls(window, cx);
        }
    }

    fn reconnect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.connection_error = None;
        self.apply_connection(IpcClient::connect_retry(), window, cx);
        cx.notify();
    }

    fn send(&mut self, command: UiCommand, cx: &mut Context<Self>) {
        let Some(client) = self.ipc.as_ref() else {
            self.connection_error = Some("后台未连接，请重新启动 RtxVideoManager.exe。".into());
            cx.notify();
            return;
        };
        if let Err(error) = client.send(command) {
            self.connection_error = Some(error);
            cx.notify();
        }
    }

    fn toggle_hdr(&mut self, cx: &mut Context<Self>) {
        log_line("console requested HDR toggle");
        self.transient_error = None;
        self.send(UiCommand::ToggleHdr, cx);
    }

    fn toggle_vsr(&mut self, cx: &mut Context<Self>) {
        log_line("console requested VSR toggle");
        self.transient_error = None;
        self.send(UiCommand::ToggleVsr, cx);
    }

    fn choose_vsr_quality(&mut self, quality: VsrQuality, cx: &mut Context<Self>) {
        if self.vsr_choice == quality {
            return;
        }
        self.vsr_choice = quality;
        self.transient_error = None;
        self.saved_note = None;
        self.send(UiCommand::SetVsrQuality { quality }, cx);
    }

    fn begin_recording(&mut self, field: HotkeyField, window: &mut Window, cx: &mut Context<Self>) {
        if self.recording == Some(field) {
            self.recording = None;
            self.send(UiCommand::EndHotkeyCapture, cx);
        } else if self.recording.is_some() {
            self.recording = Some(field);
            self.focus_handle.focus(window, cx);
        } else {
            self.recording = Some(field);
            self.focus_handle.focus(window, cx);
            self.send(UiCommand::BeginHotkeyCapture, cx);
        }
        cx.notify();
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.recording else {
            return;
        };
        if event.keystroke.key.eq_ignore_ascii_case("escape")
            && !event.keystroke.modifiers.modified()
        {
            self.recording = None;
            self.send(UiCommand::EndHotkeyCapture, cx);
            cx.notify();
            return;
        }
        match hotkey_from_keystroke(&event.keystroke) {
            Ok(hotkey) => {
                let (first, second) = match target {
                    HotkeyField::Hdr => (self.draft.vsr_hotkey, self.draft.show_console_hotkey),
                    HotkeyField::Vsr => (self.draft.hotkey, self.draft.show_console_hotkey),
                    HotkeyField::Console => (self.draft.hotkey, self.draft.vsr_hotkey),
                };
                if hotkey == first || hotkey == second {
                    self.transient_error = Some("三个快捷键不能相同。".into());
                    cx.notify();
                    return;
                }
                match target {
                    HotkeyField::Hdr => self.draft.hotkey = hotkey,
                    HotkeyField::Vsr => self.draft.vsr_hotkey = hotkey,
                    HotkeyField::Console => self.draft.show_console_hotkey = hotkey,
                }
                self.recording = None;
                self.dirty = true;
                self.transient_error = None;
                self.send(UiCommand::EndHotkeyCapture, cx);
            }
            Err(message) => {
                self.transient_error = Some(message);
            }
        }
        cx.notify();
    }

    fn set_tuning_from_slider(
        &mut self,
        field: TuningField,
        slider_value: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = if field == TuningField::Peak {
            let Some(hdr) = self.hdr_ready() else {
                return;
            };
            let position = slider_value.clamp(0., 1000.);
            (hdr.peak_min as f32 + (hdr.peak_max - hdr.peak_min) as f32 * position / 1000.).round()
                as i32
        } else {
            slider_value.round() as i32
        };
        self.set_tuning_value(field, value, TuningSource::Slider, window, cx);
    }

    fn set_tuning_from_input(
        &mut self,
        field: TuningField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = self.tuning_input(field);
        let Ok(value) = input.read(cx).value().trim().parse::<i32>() else {
            return;
        };
        self.set_tuning_value(field, value, TuningSource::Input, window, cx);
    }

    fn set_tuning_value(
        &mut self,
        field: TuningField,
        value: i32,
        source: TuningSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(hdr) = self.hdr_ready().cloned() else {
            return;
        };
        let value = match field {
            TuningField::Peak => value.clamp(hdr.peak_min, hdr.peak_max),
            TuningField::MiddleGray => value.clamp(0, 100),
            TuningField::Contrast | TuningField::Saturation => value.clamp(-100, 100),
        };
        let Some(draft) = self.hdr_draft.as_mut() else {
            return;
        };
        match field {
            TuningField::Peak => draft.peak_brightness = value,
            TuningField::MiddleGray => draft.middle_gray = value,
            TuningField::Contrast => draft.contrast = value,
            TuningField::Saturation => draft.saturation = value,
        }
        self.hdr_dirty = *draft != HdrAdjustments::from_snapshot(&hdr);
        self.write_tuning_control(field, value, &hdr, source, window, cx);
        self.saved_note = None;
        cx.notify();
    }

    fn write_tuning_control(
        &self,
        field: TuningField,
        value: i32,
        hdr: &rtx_video_manager::HdrSnapshot,
        source: TuningSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if source != TuningSource::Input {
            let input = self.tuning_input(field);
            input.update(cx, |state, cx| {
                state.set_value(value.to_string(), window, cx);
            });
        }
        if source != TuningSource::Slider {
            let slider = self.tuning_slider(field);
            let slider_value = if field == TuningField::Peak {
                let range = (hdr.peak_max - hdr.peak_min) as f32;
                if range <= 0. {
                    0.
                } else {
                    (value - hdr.peak_min) as f32 / range * 1000.
                }
            } else {
                value as f32
            };
            slider.update(cx, |state, cx| {
                state.set_value(SliderValue::Single(slider_value), window, cx);
            });
        }
        if field == TuningField::Peak && source == TuningSource::Snapshot {
            self.peak_input.update(cx, |state, cx| {
                state.set_min(Some(hdr.peak_min as f64), window, cx);
                state.set_max(Some(hdr.peak_max as f64), window, cx);
            });
        }
    }

    fn sync_tuning_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(hdr) = self.hdr_ready().cloned() else {
            return;
        };
        let Some(draft) = self.hdr_draft.clone() else {
            return;
        };
        for (field, value) in [
            (TuningField::Peak, draft.peak_brightness),
            (TuningField::MiddleGray, draft.middle_gray),
            (TuningField::Contrast, draft.contrast),
            (TuningField::Saturation, draft.saturation),
        ] {
            self.write_tuning_control(field, value, &hdr, TuningSource::Snapshot, window, cx);
        }
    }

    fn tuning_input(&self, field: TuningField) -> Entity<InputState> {
        match field {
            TuningField::Peak => self.peak_input.clone(),
            TuningField::MiddleGray => self.middle_gray_input.clone(),
            TuningField::Contrast => self.contrast_input.clone(),
            TuningField::Saturation => self.saturation_input.clone(),
        }
    }

    fn tuning_slider(&self, field: TuningField) -> Entity<SliderState> {
        match field {
            TuningField::Peak => self.peak_slider.clone(),
            TuningField::MiddleGray => self.middle_gray_slider.clone(),
            TuningField::Contrast => self.contrast_slider.clone(),
            TuningField::Saturation => self.saturation_slider.clone(),
        }
    }

    fn set_startup(&mut self, value: bool, cx: &mut Context<Self>) {
        if self.draft.start_at_login != value {
            self.draft.start_at_login = value;
            self.dirty = true;
            self.saved_note = None;
            cx.notify();
        }
    }

    fn set_notifications(&mut self, value: bool, cx: &mut Context<Self>) {
        if self.draft.notifications != value {
            self.draft.notifications = value;
            self.dirty = true;
            self.saved_note = None;
            cx.notify();
        }
    }

    fn reset_defaults(&mut self, cx: &mut Context<Self>) {
        self.draft = Settings::default();
        self.vsr_choice = if self.vsr_ready().is_some_and(|vsr| vsr.enabled) {
            self.vsr_ready()
                .map(|vsr| vsr.quality)
                .unwrap_or(VsrQuality::Auto)
        } else {
            VsrQuality::Auto
        };
        self.dirty = true;
        self.saved_note = None;
        self.transient_error = None;
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        if self.recording.is_some() {
            return;
        }
        self.saving = true;
        log_line("console requested settings save");
        self.saved_note = None;
        self.transient_error = None;
        let settings = self.draft.clone();
        self.send(UiCommand::ApplySettings { settings }, cx);
        cx.notify();
    }

    fn reset_tuning(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(hdr) = self.hdr_ready().cloned() {
            let defaults = HdrAdjustments::defaults_for(&hdr);
            self.hdr_dirty = defaults != HdrAdjustments::from_snapshot(&hdr);
            self.hdr_draft = Some(defaults);
            self.transient_error = None;
            self.sync_tuning_controls(window, cx);
            cx.notify();
        }
    }

    fn save_tuning(&mut self, cx: &mut Context<Self>) {
        let Some(adjustments) = self.hdr_draft.clone() else {
            return;
        };
        if !self.hdr_dirty || !self.can_tune() {
            return;
        }
        self.tuning_saving = true;
        self.transient_error = None;
        self.saved_note = None;
        log_line("console requested HDR tuning");
        self.send(UiCommand::ApplyHdrTuning { adjustments }, cx);
        cx.notify();
    }

    fn hdr_ready(&self) -> Option<&rtx_video_manager::HdrSnapshot> {
        match self.snapshot.as_ref().map(|snapshot| &snapshot.hdr) {
            Some(UiHdrState::Ready(hdr)) => Some(hdr),
            _ => None,
        }
    }

    fn vsr_ready(&self) -> Option<&VsrSnapshot> {
        match self.snapshot.as_ref().map(|snapshot| &snapshot.vsr) {
            Some(UiVsrState::Ready(vsr)) => Some(vsr),
            _ => None,
        }
    }

    fn compatibility(&self) -> Option<&CompatibilityInfo> {
        self.snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.diagnostics.compatibility.as_ref())
    }

    fn compatibility_writable(&self) -> bool {
        self.compatibility()
            .is_some_and(CompatibilityInfo::is_writable)
    }

    fn compatibility_read_only(&self) -> bool {
        self.compatibility()
            .is_some_and(|compatibility| !compatibility.is_writable())
    }

    fn busy_kind(&self) -> Option<UiBusyKind> {
        self.snapshot.as_ref().and_then(|snapshot| snapshot.busy)
    }

    fn busy(&self) -> bool {
        self.busy_kind().is_some()
    }

    fn can_toggle_hdr(&self) -> bool {
        self.connection_error.is_none()
            && !self.busy()
            && self.compatibility_writable()
            && self
                .hdr_ready()
                .is_some_and(|hdr| hdr.supported && hdr.error_code == 0)
    }

    fn can_toggle_vsr(&self) -> bool {
        self.connection_error.is_none()
            && !self.busy()
            && self.compatibility_writable()
            && self.vsr_ready().is_some_and(|vsr| vsr.supported)
    }

    fn can_choose_vsr(&self) -> bool {
        self.connection_error.is_none()
            && !self.busy()
            && self.vsr_ready().is_some_and(|vsr| vsr.supported)
            && self.compatibility_writable()
    }

    fn can_tune(&self) -> bool {
        self.connection_error.is_none()
            && !self.busy()
            && self.compatibility_writable()
            && self
                .hdr_ready()
                .is_some_and(|hdr| hdr.supported && hdr.error_code == 0)
    }

    fn diagnostic_text(&self) -> String {
        self.snapshot
            .as_ref()
            .map(format_diagnostic_text)
            .unwrap_or_else(|| "RTX Video Manager diagnostics\napp_version=unknown".into())
    }

    fn enabled(&self) -> bool {
        self.hdr_ready().is_some_and(|hdr| hdr.enabled)
    }

    fn status_text(&self) -> (String, String, String) {
        if let Some(error) = &self.connection_error {
            return ("后台连接已断开".into(), error.clone(), "○ 已断线".into());
        }
        match self.snapshot.as_ref().map(|snapshot| &snapshot.hdr) {
            None | Some(UiHdrState::Loading) => (
                "正在读取 HDR 状态".into(),
                "正在连接 NVIDIA 显示设置服务".into(),
                "● 读取中".into(),
            ),
            Some(UiHdrState::Error(error)) => (
                "RTX VIDEO HDR 不可用".into(),
                error.clone(),
                "● 不可用".into(),
            ),
            Some(UiHdrState::Ready(hdr)) if !self.compatibility_writable() => (
                "RTX VIDEO HDR · 只读兼容".into(),
                if !hdr.supported {
                    "当前 GPU 或显示器不支持 RTX 视频 HDR；当前构建仅允许读取。".into()
                } else if hdr.error_code != 0 {
                    format!("{}；当前构建仅允许读取。", error_for_code(hdr.error_code))
                } else {
                    self.compatibility()
                        .and_then(|compatibility| compatibility.reason.clone())
                        .unwrap_or_else(|| "当前 NVIDIA 构建未经验证，已禁止写入。".into())
                },
                "● 只读".into(),
            ),
            Some(UiHdrState::Ready(hdr)) if !hdr.supported => (
                "RTX VIDEO HDR 不支持".into(),
                "当前 GPU 或显示器不支持 RTX 视频 HDR".into(),
                "● 不支持".into(),
            ),
            Some(UiHdrState::Ready(hdr)) if hdr.error_code != 0 => (
                "RTX VIDEO HDR 不可用".into(),
                error_for_code(hdr.error_code).into(),
                "● 不可用".into(),
            ),
            Some(UiHdrState::Ready(_)) if self.busy_kind() == Some(UiBusyKind::HdrToggle) => (
                "正在切换 RTX VIDEO HDR".into(),
                "请稍候，正在写入并回读 NVIDIA 状态".into(),
                "● 切换中".into(),
            ),
            Some(UiHdrState::Ready(hdr)) if hdr.enabled => (
                "RTX VIDEO HDR 已开启".into(),
                if hdr.active {
                    "当前视频正在以 HDR10 输出".into()
                } else {
                    "已开启，等待支持的视频开始播放".into()
                },
                "● 已开启".into(),
            ),
            Some(UiHdrState::Ready(_)) => (
                "RTX VIDEO HDR 已关闭".into(),
                "当前视频以 SDR 输出".into(),
                "○ 已关闭".into(),
            ),
        }
    }

    fn vsr_status_text(&self) -> (String, String, String) {
        if let Some(error) = &self.connection_error {
            return (
                "VSR 后台连接已断开".into(),
                error.clone(),
                "○ 已断线".into(),
            );
        }
        match self.snapshot.as_ref().map(|snapshot| &snapshot.vsr) {
            None | Some(UiVsrState::Loading) => (
                "正在读取 VSR 状态".into(),
                "正在连接 NVIDIA 视频增强服务".into(),
                "● 读取中".into(),
            ),
            Some(UiVsrState::Error(error)) => {
                ("RTX VSR 不可用".into(), error.clone(), "● 不可用".into())
            }
            Some(UiVsrState::Ready(vsr)) if !vsr.supported => (
                "RTX VSR 不支持".into(),
                "当前 GPU 不支持 RTX 视频超分辨率".into(),
                "● 不支持".into(),
            ),
            Some(UiVsrState::Ready(_)) if !self.compatibility_writable() => (
                "RTX VSR · 只读兼容".into(),
                "当前 NVIDIA 构建仅允许读取 VSR 状态".into(),
                "● 只读".into(),
            ),
            Some(UiVsrState::Ready(_)) if self.busy_kind() == Some(UiBusyKind::VsrToggle) => (
                "正在切换 RTX VSR".into(),
                "请稍候，正在写入并回读质量档位".into(),
                "● 切换中".into(),
            ),
            Some(UiVsrState::Ready(vsr)) => {
                let quality = quality_label(vsr.quality);
                if vsr.enabled {
                    (
                        format!("RTX VSR 已开启 · {quality}"),
                        if vsr.active {
                            format!(
                                "当前视频正在应用{}档超分辨率",
                                quality_label(vsr.active_quality.unwrap_or(vsr.quality))
                            )
                        } else {
                            "已开启，等待兼容视频开始播放".into()
                        },
                        "● 已开启".into(),
                    )
                } else {
                    (
                        "RTX VSR 已关闭".into(),
                        format!("下次启用使用 {} 档", quality_label(self.vsr_choice)),
                        "○ 已关闭".into(),
                    )
                }
            }
        }
    }

    fn render_title_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        let _ = cx;
        h_flex()
            .id("title-bar")
            .h(px(34.))
            .w_full()
            .flex_shrink_0()
            .items_center()
            .border_b_1()
            .border_color(rgb(RULE))
            .bg(rgb(INK))
            .child(
                h_flex()
                    .flex_1()
                    .h_full()
                    .pl_3()
                    .gap_2()
                    .items_center()
                    .window_control_area(WindowControlArea::Drag)
                    .child(div().w(px(7.)).h(px(7.)).rounded_full().bg(rgb(SIGNAL)))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0xC6D4D5))
                            .child("RTX Video Manager"),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .child("· 后台运行中"),
                    ),
            )
            .child(
                h_flex()
                    .id("window-controls")
                    .h_full()
                    .flex_shrink_0()
                    .child(
                        div()
                            .id("minimize")
                            .w(px(35.))
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(MUTED))
                            .hover(|this| this.bg(rgb(RAISED)).text_color(rgb(TEXT)))
                            .window_control_area(WindowControlArea::Min)
                            .child("−"),
                    )
                    .child(
                        div()
                            .id("close")
                            .w(px(35.))
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(MUTED))
                            .hover(|this| this.bg(rgb(DANGER)).text_color(rgb(INK)))
                            .window_control_area(WindowControlArea::Close)
                            .child("×"),
                    ),
            )
    }

    fn render_overview_hdr_card(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (status, detail, badge) = self.status_text();
        let enabled = self.enabled();
        let can_toggle = self.can_toggle_hdr();
        let read_only = self.compatibility_read_only();
        let hdr = self.hdr_ready();
        let peak = hdr.map_or("—".into(), |hdr| hdr.peak_brightness.to_string());
        let peak_min = hdr.map_or("—".into(), |hdr| hdr.peak_min.to_string());
        let peak_max = hdr.map_or("—".into(), |hdr| hdr.peak_max.to_string());

        v_flex()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .gap_2()
            .p_5()
            .rounded(px(14.))
            .border_1()
            .border_color(rgb(RULE))
            .bg(rgb(PANEL))
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(7.)).h(px(7.)).rounded_full().bg(if enabled {
                        rgb(SIGNAL)
                    } else {
                        rgb(MUTED)
                    }))
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(MUTED))
                            .child(status),
                    )
                    .child(
                        div()
                            .ml_auto()
                            .px_2()
                            .py_1()
                            .rounded_full()
                            .border_1()
                            .border_color(if enabled {
                                rgb(SIGNAL).opacity(0.38)
                            } else {
                                rgb(RULE)
                            })
                            .bg(if enabled {
                                rgb(SIGNAL).opacity(0.13)
                            } else {
                                rgb(PANEL)
                            })
                            .text_size(px(10.))
                            .text_color(if enabled { rgb(SIGNAL) } else { rgb(MUTED) })
                            .child(badge),
                    ),
            )
            .child(
                v_flex().h(px(56.)).mt_1().justify_center().child(
                    div()
                        .font_family("Bahnschrift")
                        .text_size(px(30.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(TEXT))
                        .child("RTX VIDEO HDR"),
                ),
            )
            .child(
                div()
                    .min_h(px(38.))
                    .text_size(px(11.))
                    .text_color(if self.connection_error.is_some() {
                        rgb(DANGER)
                    } else {
                        rgb(MUTED)
                    })
                    .child(detail),
            )
            .child(
                h_flex()
                    .items_end()
                    .gap_2()
                    .child(
                        div()
                            .font_family("Cascadia Mono")
                            .text_size(px(24.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(TEXT))
                            .child(peak),
                    )
                    .child(
                        div()
                            .pb(px(3.))
                            .font_family("Cascadia Mono")
                            .text_size(px(9.))
                            .text_color(rgb(SUBTLE))
                            .child("NITS"),
                    ),
            )
            .child(spectrum_bar())
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .font_family("Cascadia Mono")
                    .text_size(px(8.))
                    .text_color(rgb(SUBTLE))
                    .child(peak_min)
                    .child(peak_max),
            )
            .child(
                h_flex()
                    .mt_auto()
                    .items_center()
                    .gap_3()
                    .pt_3()
                    .border_t_1()
                    .border_color(rgb(RULE))
                    .child(
                        Switch::new("overview-hdr-toggle")
                            .checked(enabled)
                            .disabled(!can_toggle)
                            .on_change(cx.listener(|app, _, _, cx| app.toggle_hdr(cx))),
                    )
                    .child(
                        v_flex()
                            .gap(px(1.))
                            .child(
                                div()
                                    .font_family("Cascadia Mono")
                                    .text_size(px(9.))
                                    .text_color(rgb(MUTED))
                                    .child(format_hotkey(self.draft.hotkey)),
                            )
                            .child(div().text_size(px(9.)).text_color(rgb(SUBTLE)).child(
                                if read_only {
                                    "只读兼容"
                                } else {
                                    "点击开关立即生效"
                                },
                            )),
                    ),
            )
    }

    fn render_overview_vsr_card(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (status, detail, badge) = self.vsr_status_text();
        let enabled = self.vsr_ready().is_some_and(|vsr| vsr.enabled);
        let can_toggle = self.can_toggle_vsr();
        let read_only = self.compatibility_read_only();
        let quality = self.vsr_ready().map_or(self.vsr_choice, |vsr| vsr.quality);
        let active_quality = self
            .vsr_ready()
            .and_then(|vsr| vsr.active_quality)
            .map(quality_label)
            .unwrap_or("无活动视频");

        v_flex()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .gap_2()
            .p_5()
            .rounded(px(14.))
            .border_1()
            .border_color(rgb(RULE))
            .bg(rgb(PANEL))
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(7.)).h(px(7.)).rounded_full().bg(if enabled {
                        rgb(WARM)
                    } else {
                        rgb(MUTED)
                    }))
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(MUTED))
                            .child(status),
                    )
                    .child(
                        div()
                            .ml_auto()
                            .px_2()
                            .py_1()
                            .rounded_full()
                            .border_1()
                            .border_color(if enabled {
                                rgb(WARM).opacity(0.38)
                            } else {
                                rgb(RULE)
                            })
                            .bg(if enabled {
                                rgb(WARM).opacity(0.13)
                            } else {
                                rgb(PANEL)
                            })
                            .text_size(px(10.))
                            .text_color(if enabled { rgb(WARM) } else { rgb(MUTED) })
                            .child(badge),
                    ),
            )
            .child(
                v_flex()
                    .h(px(56.))
                    .mt_1()
                    .justify_center()
                    .gap(px(0.))
                    .child(
                        div()
                            .font_family("Bahnschrift")
                            .text_size(px(22.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(TEXT))
                            .child("RTX VIDEO"),
                    )
                    .child(
                        div()
                            .font_family("Bahnschrift")
                            .text_size(px(22.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(TEXT))
                            .child("SUPER RESOLUTION"),
                    ),
            )
            .child(
                div()
                    .min_h(px(38.))
                    .text_size(px(11.))
                    .text_color(if self.connection_error.is_some() {
                        rgb(DANGER)
                    } else {
                        rgb(MUTED)
                    })
                    .child(detail),
            )
            .child(
                h_flex()
                    .items_end()
                    .gap_2()
                    .child(
                        div()
                            .font_family("Cascadia Mono")
                            .text_size(px(24.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(TEXT))
                            .child(quality_label(quality).to_string()),
                    )
                    .child(
                        div()
                            .pb(px(3.))
                            .font_family("Cascadia Mono")
                            .text_size(px(9.))
                            .text_color(rgb(SUBTLE))
                            .child("QUALITY · AUTO 可选"),
                    ),
            )
            .child(vsr_ladder())
            .child(
                h_flex()
                    .justify_between()
                    .font_family("Cascadia Mono")
                    .text_size(px(8.))
                    .text_color(rgb(SUBTLE))
                    .child("AUTO")
                    .child("1")
                    .child("2")
                    .child("3")
                    .child("4"),
            )
            .child(
                h_flex()
                    .mt_auto()
                    .items_center()
                    .gap_3()
                    .pt_3()
                    .border_t_1()
                    .border_color(rgb(RULE))
                    .child(
                        Switch::new("overview-vsr-toggle")
                            .checked(enabled)
                            .disabled(!can_toggle)
                            .on_change(cx.listener(|app, _, _, cx| app.toggle_vsr(cx))),
                    )
                    .child(
                        v_flex()
                            .gap(px(1.))
                            .child(
                                div()
                                    .font_family("Cascadia Mono")
                                    .text_size(px(9.))
                                    .text_color(rgb(MUTED))
                                    .child(format_hotkey(self.draft.vsr_hotkey)),
                            )
                            .child(div().text_size(px(9.)).text_color(rgb(SUBTLE)).child(
                                if read_only {
                                    "只读兼容".to_string()
                                } else {
                                    format!("当前生效：{active_quality}")
                                },
                            )),
                    ),
            )
    }

    fn render_overview_summary(&self) -> impl IntoElement {
        let compatibility = self
            .compatibility()
            .map(|value| {
                if value.is_writable() {
                    "已验证可写"
                } else {
                    "只读兼容"
                }
            })
            .unwrap_or("正在读取");
        let hdr_signal = self
            .hdr_ready()
            .map(|hdr| {
                if hdr.active {
                    "HDR10 输出中"
                } else if hdr.enabled {
                    "HDR 等待兼容视频"
                } else {
                    "HDR 已关闭"
                }
            })
            .unwrap_or("HDR 读取中");
        let vsr_signal = self
            .vsr_ready()
            .map(|vsr| {
                if vsr.active {
                    "VSR 正在生效"
                } else if vsr.enabled {
                    "VSR 等待兼容视频"
                } else {
                    "VSR 已关闭"
                }
            })
            .unwrap_or("VSR 读取中");
        let hotkeys = format!(
            "H {} · V {} · G {}",
            format_hotkey(self.draft.hotkey),
            format_hotkey(self.draft.vsr_hotkey),
            format_hotkey(self.draft.show_console_hotkey)
        );

        h_flex()
            .w_full()
            .h(px(74.))
            .flex_shrink_0()
            .rounded(px(14.))
            .border_1()
            .border_color(rgb(RULE))
            .bg(rgb(PANEL))
            .child(summary_cell(
                "NVIDIA BACKEND",
                format!("{compatibility} · 0x318 + 0x315"),
            ))
            .child(div().w(px(1.)).h(px(38.)).bg(rgb(RULE)))
            .child(summary_cell(
                "CURRENT SIGNAL",
                format!("{hdr_signal} · {vsr_signal}"),
            ))
            .child(div().w(px(1.)).h(px(38.)).bg(rgb(RULE)))
            .child(summary_cell("GLOBAL HOTKEYS", hotkeys))
    }

    fn render_driver_info(&self) -> impl IntoElement {
        let compatibility = self.compatibility();
        let mode = compatibility
            .map(|value| {
                if value.is_writable() {
                    "已验证可写"
                } else {
                    "只读兼容"
                }
            })
            .unwrap_or("等待读取");
        let version = compatibility
            .and_then(|value| value.dll_version.as_deref())
            .unwrap_or("未知版本");
        let probe = compatibility
            .map(|value| {
                if value.probe_passed {
                    "通过"
                } else {
                    "未通过"
                }
            })
            .unwrap_or("等待读取");

        v_flex()
            .w(px(212.))
            .h_full()
            .flex_shrink_0()
            .overflow_hidden()
            .gap_2()
            .p_4()
            .rounded(px(14.))
            .border_1()
            .border_color(rgb(RULE))
            .bg(rgb(PANEL))
            .child(
                h_flex()
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(TEXT))
                            .child("驱动信息"),
                    )
                    .child(
                        div()
                            .ml_auto()
                            .font_family("Cascadia Mono")
                            .text_size(px(9.))
                            .text_color(rgb(SUBTLE))
                            .child("0x318"),
                    ),
            )
            .child(info_value("兼容模式", mode))
            .child(info_value("NVCpl 版本", version))
            .child(info_value("读取探针", probe))
    }

    fn render_video_info(&self) -> impl IntoElement {
        let vsr = self.vsr_ready();
        let active = if vsr.is_some_and(|vsr| vsr.active) {
            "正在处理兼容视频"
        } else {
            "等待兼容视频"
        };
        let active_quality = vsr
            .and_then(|vsr| vsr.active_quality)
            .map(quality_label)
            .unwrap_or("无");
        let selected_quality = vsr.map(|vsr| quality_label(vsr.quality)).unwrap_or("—");

        v_flex()
            .w(px(212.))
            .h_full()
            .flex_shrink_0()
            .overflow_hidden()
            .gap_2()
            .p_4()
            .rounded(px(14.))
            .border_1()
            .border_color(rgb(RULE))
            .bg(rgb(PANEL))
            .child(
                h_flex()
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(TEXT))
                            .child("当前视频"),
                    )
                    .child(
                        div()
                            .ml_auto()
                            .font_family("Cascadia Mono")
                            .text_size(px(9.))
                            .text_color(rgb(SUBTLE))
                            .child("0x315"),
                    ),
            )
            .child(info_value("处理状态", active))
            .child(info_value("活动档位", active_quality))
            .child(info_value("启用档位", selected_quality))
    }

    fn render_hero(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (status, detail, badge) = self.status_text();
        let enabled = self.enabled();
        let can_toggle = self.can_toggle_hdr();
        let read_only = self.compatibility_read_only();
        let busy = self.busy_kind() == Some(UiBusyKind::HdrToggle);
        let primary_hint = if read_only {
            "当前 NVIDIA 构建仅支持读取".to_string()
        } else {
            format!("HDR {}", format_hotkey(self.draft.hotkey))
        };

        let toggle_style = ButtonCustomVariant::new(cx)
            .color(if enabled {
                rgb(RAISED).into()
            } else {
                rgb(SIGNAL).into()
            })
            .foreground(if enabled {
                rgb(TEXT).into()
            } else {
                rgb(INK).into()
            })
            .hover(if enabled {
                rgb(0x203138).into()
            } else {
                rgb(SIGNAL_HOVER).into()
            })
            .active(if enabled {
                rgb(0x17262B).into()
            } else {
                rgb(0x51CBD4).into()
            });

        h_flex()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .gap_5()
            .p_5()
            .rounded(px(14.))
            .border_1()
            .border_color(rgb(RULE))
            .bg(rgb(PANEL))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(div().w(px(7.)).h(px(7.)).rounded_full().bg(if enabled {
                                rgb(SIGNAL)
                            } else {
                                rgb(MUTED)
                            }))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(MUTED))
                                    .child(status),
                            ),
                    )
                    .child(
                        div()
                            .mt(px(4.))
                            .font_family("Bahnschrift")
                            .text_size(px(34.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(TEXT))
                            .child("RTX VIDEO HDR"),
                    )
                    .child(
                        div()
                            .min_h(px(20.))
                            .text_size(px(12.))
                            .text_color(if self.connection_error.is_some() {
                                rgb(DANGER)
                            } else {
                                rgb(MUTED)
                            })
                            .child(detail),
                    ),
            )
            .child(
                v_flex()
                    .w(px(176.))
                    .flex_shrink_0()
                    .justify_center()
                    .gap_3()
                    .child(
                        div()
                            .self_center()
                            .px_2()
                            .py_1()
                            .rounded_full()
                            .border_1()
                            .border_color(if enabled {
                                rgb(SIGNAL).opacity(0.38)
                            } else {
                                rgb(RULE)
                            })
                            .bg(if enabled {
                                rgb(SIGNAL).opacity(0.13)
                            } else {
                                rgb(PANEL)
                            })
                            .text_size(px(11.))
                            .text_color(if enabled { rgb(SIGNAL) } else { rgb(MUTED) })
                            .child(badge),
                    )
                    .child(
                        Button::new("toggle-hdr")
                            .custom(toggle_style)
                            .label(if read_only {
                                "只读兼容模式"
                            } else if enabled {
                                "关闭 RTX HDR"
                            } else {
                                "开启 RTX HDR"
                            })
                            .loading(busy)
                            .disabled(!can_toggle)
                            .w_full()
                            .h(px(48.))
                            .on_click(cx.listener(|app, _, _, cx| app.toggle_hdr(cx))),
                    )
                    .child(
                        v_flex()
                            .gap(px(1.))
                            .text_size(px(9.))
                            .text_color(rgb(SUBTLE))
                            .text_center()
                            .child(primary_hint),
                    ),
            )
    }

    fn render_vsr_hero(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (status, detail, badge) = self.vsr_status_text();
        let enabled = self.vsr_ready().is_some_and(|vsr| vsr.enabled);
        let supported = self.vsr_ready().is_some_and(|vsr| vsr.supported);
        let can_toggle = self.can_toggle_vsr();
        let read_only = self.compatibility_read_only();
        let busy = self.busy_kind() == Some(UiBusyKind::VsrToggle);

        let toggle_style = ButtonCustomVariant::new(cx)
            .color(if enabled {
                rgb(RAISED).into()
            } else {
                rgb(WARM).into()
            })
            .foreground(if enabled {
                rgb(TEXT).into()
            } else {
                rgb(INK).into()
            })
            .hover(if enabled {
                rgb(0x203138).into()
            } else {
                rgb(0xF5C879).into()
            })
            .active(if enabled {
                rgb(0x17262B).into()
            } else {
                rgb(0xD99A43).into()
            });

        h_flex()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .gap_5()
            .p_5()
            .rounded(px(14.))
            .border_1()
            .border_color(rgb(RULE))
            .bg(rgb(PANEL))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(div().w(px(7.)).h(px(7.)).rounded_full().bg(if enabled {
                                rgb(WARM)
                            } else {
                                rgb(MUTED)
                            }))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(MUTED))
                                    .child(status),
                            ),
                    )
                    .child(
                        v_flex()
                            .mt(px(4.))
                            .gap(px(0.))
                            .child(
                                div()
                                    .font_family("Bahnschrift")
                                    .text_size(px(28.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(TEXT))
                                    .child("RTX VIDEO"),
                            )
                            .child(
                                div()
                                    .font_family("Bahnschrift")
                                    .text_size(px(28.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(TEXT))
                                    .child("SUPER RESOLUTION"),
                            ),
                    )
                    .child(
                        div()
                            .min_h(px(36.))
                            .text_size(px(12.))
                            .text_color(if self.connection_error.is_some() {
                                rgb(DANGER)
                            } else {
                                rgb(MUTED)
                            })
                            .child(detail),
                    ),
            )
            .child(
                v_flex()
                    .w(px(190.))
                    .flex_shrink_0()
                    .justify_center()
                    .gap_3()
                    .child(
                        div()
                            .self_center()
                            .px_2()
                            .py_1()
                            .rounded_full()
                            .border_1()
                            .border_color(if enabled {
                                rgb(WARM).opacity(0.38)
                            } else {
                                rgb(RULE)
                            })
                            .bg(if enabled {
                                rgb(WARM).opacity(0.13)
                            } else {
                                rgb(PANEL)
                            })
                            .text_size(px(11.))
                            .text_color(if enabled { rgb(WARM) } else { rgb(MUTED) })
                            .child(badge),
                    )
                    .child(
                        Button::new("toggle-vsr")
                            .custom(toggle_style)
                            .label(if read_only {
                                "只读兼容模式"
                            } else if !supported {
                                "VSR 不可用"
                            } else if enabled {
                                "关闭 RTX VSR"
                            } else {
                                "开启 RTX VSR"
                            })
                            .loading(busy)
                            .disabled(!can_toggle)
                            .w_full()
                            .h(px(46.))
                            .on_click(cx.listener(|app, _, _, cx| app.toggle_vsr(cx))),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(SUBTLE))
                            .text_center()
                            .child(if read_only {
                                "当前 NVIDIA 构建仅支持读取".into()
                            } else {
                                format!("切换 {}", format_hotkey(self.draft.vsr_hotkey))
                            }),
                    ),
            )
    }

    fn render_vsr_quality(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let vsr = self.vsr_ready().cloned();
        let enabled = vsr.as_ref().is_some_and(|value| value.enabled);
        let active_quality = vsr
            .as_ref()
            .and_then(|value| value.active_quality)
            .map(quality_label);
        let active = active_quality.unwrap_or("无活动视频");
        let active_present = active_quality.is_some();
        let can_choose = self.can_choose_vsr();
        v_flex()
            .w_full()
            .flex_1()
            .min_h(px(0.))
            .gap_3()
            .p_4()
            .rounded(px(14.))
            .border_1()
            .border_color(if enabled {
                rgb(WARM).opacity(0.45)
            } else {
                rgb(RULE)
            })
            .bg(rgb(PANEL))
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(TEXT))
                            .child("VSR 质量"),
                    )
                    .child(
                        div()
                            .font_family("Cascadia Mono")
                            .text_size(px(9.))
                            .text_color(rgb(SUBTLE))
                            .child("NVIDIA 0x315 / 0x317"),
                    )
                    .child(
                        div()
                            .ml_auto()
                            .text_size(px(10.))
                            .text_color(rgb(MUTED))
                            .child(format!(
                                "{}：{}",
                                if enabled {
                                    "当前选择"
                                } else {
                                    "下次启用"
                                },
                                quality_label(self.vsr_choice)
                            )),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(MUTED))
                    .child(if enabled {
                        "数字越高，画质越好，同时占用更多 GPU 性能。"
                    } else {
                        "VSR 已关闭；选择的档位会保存为下次启用时使用。"
                    }),
            )
            .child(
                h_flex().w_full().gap_2().children(
                    [
                        VsrQuality::Auto,
                        VsrQuality::Level1,
                        VsrQuality::Level2,
                        VsrQuality::Level3,
                        VsrQuality::Level4,
                    ]
                    .into_iter()
                    .map(|quality| {
                        let selected = self.vsr_choice == quality;
                        Button::new(vsr_quality_id(quality))
                            .custom(
                                ButtonCustomVariant::new(cx)
                                    .color(if selected {
                                        rgb(WARM).into()
                                    } else {
                                        rgb(RAISED).into()
                                    })
                                    .foreground(if selected {
                                        rgb(INK).into()
                                    } else {
                                        rgb(TEXT).into()
                                    })
                                    .hover(rgb(0x203138).into())
                                    .active(rgb(0x17262B).into()),
                            )
                            .label(quality_label(quality))
                            .disabled(!can_choose)
                            .w(px(78.))
                            .h(px(38.))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.choose_vsr_quality(quality, cx);
                            }))
                    }),
                ),
            )
            .child(h_flex().w_full().gap_2().mt_auto().children([
                quality_guide_cell("AUTO", "驱动按内容自动选择"),
                quality_guide_cell("1–2", "性能优先 · GPU 成本更低"),
                quality_guide_cell("3–4", "画质优先 · GPU 成本更高"),
            ]))
            .child(
                h_flex()
                    .w_full()
                    .h(px(42.))
                    .items_center()
                    .gap_3()
                    .px_4()
                    .rounded(px(10.))
                    .border_1()
                    .border_color(if active_present {
                        rgb(WARM).opacity(0.42)
                    } else {
                        rgb(RULE)
                    })
                    .bg(if active_present {
                        rgb(WARM).opacity(0.08)
                    } else {
                        rgb(RAISED)
                    })
                    .child(
                        div()
                            .w(px(7.))
                            .h(px(7.))
                            .rounded_full()
                            .bg(if active_present {
                                rgb(WARM)
                            } else {
                                rgb(MUTED)
                            }),
                    )
                    .child(
                        v_flex()
                            .gap(px(1.))
                            .child(
                                div()
                                    .text_size(px(9.))
                                    .text_color(rgb(SUBTLE))
                                    .child("当前视频生效档位"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(if active_present { rgb(WARM) } else { rgb(TEXT) })
                                    .child(active),
                            ),
                    )
                    .child(
                        div()
                            .ml_auto()
                            .font_family("Cascadia Mono")
                            .text_size(px(9.))
                            .text_color(rgb(SUBTLE))
                            .child("0x315"),
                    ),
            )
            .child(
                h_flex()
                    .mt_auto()
                    .gap_4()
                    .text_size(px(10.))
                    .text_color(rgb(SUBTLE))
                    .child("关闭 VSR 时仅保存下次档位，不调用写接口")
                    .child(
                        div()
                            .ml_auto()
                            .font_family("Cascadia Mono")
                            .child("实时状态读取 0x315 · 档位写入 0x317"),
                    ),
            )
    }

    fn render_tab(
        &self,
        page: UiPage,
        label: &'static str,
        id: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.page == page;
        Button::new(id)
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(if active {
                        rgb(RAISED).into()
                    } else {
                        rgb(INK).into()
                    })
                    .foreground(if active {
                        rgb(TEXT).into()
                    } else {
                        rgb(MUTED).into()
                    })
                    .hover(rgb(0x17262B).into())
                    .active(rgb(0x203138).into()),
            )
            .compact()
            .label(label)
            .on_click(cx.listener(move |app, _, _, cx| {
                app.page = page;
                cx.notify();
            }))
    }

    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .h(px(42.))
            .flex_shrink_0()
            .gap_1()
            .children([
                self.render_tab(UiPage::Overview, "总览", "tab-overview", cx)
                    .into_any_element(),
                self.render_tab(UiPage::Hdr, "HDR", "tab-hdr", cx)
                    .into_any_element(),
                self.render_tab(UiPage::Vsr, "VSR", "tab-vsr", cx)
                    .into_any_element(),
                self.render_tab(UiPage::Settings, "设置", "tab-settings", cx)
                    .into_any_element(),
                self.render_tab(UiPage::Diagnostics, "诊断", "tab-diagnostics", cx)
                    .into_any_element(),
            ])
    }

    fn render_keycaps(&self, hotkey: Hotkey) -> impl IntoElement {
        h_flex().gap_1().children(
            format_hotkey(hotkey)
                .split('+')
                .map(keycap)
                .collect::<Vec<_>>(),
        )
    }

    fn render_hotkey_row(
        &self,
        field: HotkeyField,
        label: &'static str,
        hotkey: Hotkey,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let recording = self.recording == Some(field);
        h_flex()
            .h(px(47.))
            .flex_shrink_0()
            .items_center()
            .gap_3()
            .px_4()
            .border_b_1()
            .border_color(rgb(RULE))
            .child(
                div()
                    .w(px(74.))
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child(label),
            )
            .child(self.render_keycaps(hotkey))
            .child(
                Button::new(match field {
                    HotkeyField::Hdr => "record-hdr",
                    HotkeyField::Vsr => "record-vsr",
                    HotkeyField::Console => "record-console",
                })
                .custom(
                    ButtonCustomVariant::new(cx)
                        .color(rgb(RAISED).into())
                        .foreground(if recording {
                            rgb(WARM).into()
                        } else {
                            rgb(0xDBE7E8).into()
                        })
                        .hover(rgb(0x203138).into())
                        .active(rgb(0x17262B).into()),
                )
                .label(if recording { "请按键…" } else { "录制" })
                .compact()
                .ml_auto()
                .on_click(cx.listener(move |app, _, window, cx| {
                    app.begin_recording(field, window, cx);
                })),
            )
    }

    fn render_tuning_control(
        &self,
        field: TuningField,
        label: &'static str,
        range: String,
    ) -> impl IntoElement {
        let input = self.tuning_input(field);
        let slider = self.tuning_slider(field);
        let enabled = self.can_tune() && !self.tuning_saving;
        let unavailable_opacity = if enabled { 1.0 } else { 0.45 };
        v_flex()
            .flex_1()
            .min_w(px(0.))
            .gap_1()
            .opacity(unavailable_opacity)
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(MUTED))
                            .child(label),
                    )
                    .child(
                        NumberInput::new(&input)
                            .disabled(!enabled)
                            .accessibility_label(label)
                            .w(px(112.))
                            .h(px(28.)),
                    ),
            )
            .child(
                Slider::new(&slider)
                    .disabled(!enabled)
                    .accessibility_label(label)
                    .bg(rgb(SIGNAL))
                    .text_color(rgb(TEXT))
                    .h(px(20.))
                    .w_full(),
            )
            .child(
                h_flex()
                    .justify_between()
                    .font_family("Cascadia Mono")
                    .text_size(px(9.))
                    .text_color(rgb(SUBTLE))
                    .child(range),
            )
    }

    fn render_tuning(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let hdr = self.hdr_ready();
        let peak_range = hdr.map_or("—".into(), |hdr| {
            format!("驱动范围 {} – {} nits", hdr.peak_min, hdr.peak_max)
        });
        let status_line = hdr.map_or("等待 NVIDIA HDR 状态".into(), |hdr| {
            format!(
                "当前峰值 {} nits · 写入后逐项回读，失败自动恢复",
                hdr.peak_brightness
            )
        });
        let disabled = !self.hdr_dirty || !self.can_tune() || self.tuning_saving;

        v_flex()
            .w_full()
            .flex_1()
            .min_h(px(234.))
            .flex_shrink_0()
            .gap_3()
            .p_4()
            .rounded(px(14.))
            .border_1()
            .border_color(if self.hdr_dirty { rgb(WARM) } else { rgb(RULE) })
            .bg(rgb(PANEL))
            .child(
                h_flex()
                    .h(px(28.))
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(TEXT))
                            .child("HDR 调校"),
                    )
                    .child(
                        div()
                            .font_family("Cascadia Mono")
                            .text_size(px(9.))
                            .text_color(rgb(SUBTLE))
                            .child("NVIDIA 0x318 · 独立应用"),
                    )
                    .when(self.hdr_dirty, |this| {
                        this.child(
                            div()
                                .px_2()
                                .py(px(2.))
                                .rounded_full()
                                .bg(rgb(WARM).opacity(0.12))
                                .text_size(px(9.))
                                .text_color(rgb(WARM))
                                .child("未应用"),
                        )
                    })
                    .child(
                        h_flex()
                            .ml_auto()
                            .gap_2()
                            .child(
                                Button::new("reset-tuning")
                                    .ghost()
                                    .compact()
                                    .label("还原")
                                    .disabled(self.tuning_saving || self.hdr_ready().is_none())
                                    .on_click(cx.listener(|app, _, window, cx| {
                                        app.reset_tuning(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("apply-tuning")
                                    .custom(
                                        ButtonCustomVariant::new(cx)
                                            .color(rgb(SIGNAL).into())
                                            .foreground(rgb(INK).into())
                                            .hover(rgb(SIGNAL_HOVER).into())
                                            .active(rgb(0x51CBD4).into()),
                                    )
                                    .compact()
                                    .label(if self.tuning_saving {
                                        "写入中…"
                                    } else {
                                        "应用调校"
                                    })
                                    .loading(self.tuning_saving)
                                    .disabled(disabled)
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.save_tuning(cx);
                                    })),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .gap_5()
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_2()
                            .child(self.render_tuning_control(
                                TuningField::Peak,
                                "峰值亮度",
                                peak_range,
                            ))
                            .child(self.render_tuning_control(
                                TuningField::MiddleGray,
                                "中间灰",
                                "0 – 100".into(),
                            )),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_2()
                            .child(self.render_tuning_control(
                                TuningField::Contrast,
                                "对比度",
                                "−100 – +100".into(),
                            ))
                            .child(self.render_tuning_control(
                                TuningField::Saturation,
                                "饱和度",
                                "−100 – +100".into(),
                            )),
                    ),
            )
            .child(
                div()
                    .mt_auto()
                    .font_family("Cascadia Mono")
                    .text_size(px(9.))
                    .text_color(rgb(SUBTLE))
                    .child(status_line),
            )
    }

    fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .h(px(273.))
            .flex_shrink_0()
            .overflow_hidden()
            .rounded(px(14.))
            .border_1()
            .border_color(rgb(RULE))
            .bg(rgb(PANEL))
            .child(self.render_hotkey_row(HotkeyField::Hdr, "切换 HDR", self.draft.hotkey, cx))
            .child(self.render_hotkey_row(HotkeyField::Vsr, "切换 VSR", self.draft.vsr_hotkey, cx))
            .child(self.render_hotkey_row(
                HotkeyField::Console,
                "唤起控制台",
                self.draft.show_console_hotkey,
                cx,
            ))
            .child(behavior_row(
                "启动",
                "登录 Windows 时自动启动",
                "只启动后台托盘，不打开控制台",
                Switch::new("startup")
                    .checked(self.draft.start_at_login)
                    .on_change(cx.listener(|app, value, _, cx| {
                        app.set_startup(*value, cx);
                    })),
            ))
            .child(behavior_row(
                "反馈",
                "切换时显示系统浮层",
                "屏幕下方 500 ms 半透明提示",
                Switch::new("notifications")
                    .checked(self.draft.notifications)
                    .on_change(cx.listener(|app, value, _, cx| {
                        app.set_notifications(*value, cx);
                    })),
            ))
            .child(
                Button::new("open-diagnostics")
                    .ghost()
                    .label("系统诊断")
                    .w_full()
                    .h(px(38.))
                    .px_4()
                    .justify_start()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.page = UiPage::Diagnostics;
                        cx.notify();
                    })),
            )
    }

    fn render_diagnostics(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let hdr = self.hdr_ready();
        let vsr = self.vsr_ready();
        let diagnostics = self.snapshot.as_ref().map(|snapshot| &snapshot.diagnostics);
        let compatibility = diagnostics.and_then(|value| value.compatibility.as_ref());
        let compatibility_text = compatibility
            .map(|value| {
                if value.is_writable() {
                    "已验证可写"
                } else {
                    "只读兼容"
                }
            })
            .unwrap_or("等待读取");
        let dll_version = compatibility
            .and_then(|value| value.dll_version.as_deref())
            .unwrap_or("未知");
        let probe_text = compatibility
            .map(|value| {
                if value.probe_passed {
                    "通过"
                } else {
                    "未完成"
                }
            })
            .unwrap_or("等待");
        let actual_sha256 = compatibility
            .map(|value| value.actual_sha256.as_str())
            .unwrap_or("—");
        let error_code = hdr.map_or("—".into(), |value| value.error_code.to_string());
        let peak = hdr.map_or("—".into(), |value| value.peak_brightness.to_string());
        let peak_range = hdr.map_or("—".into(), |value| {
            format!("{} – {}", value.peak_min, value.peak_max)
        });
        let middle_gray = hdr.map_or("—".into(), |value| value.middle_gray.to_string());
        let tone = hdr.map_or("—".into(), |value| {
            format!("{} / {}", value.contrast, value.saturation)
        });
        let vsr_status = vsr.map_or("—".into(), |value| {
            if value.enabled {
                format!("质量 {}", quality_label(value.quality))
            } else {
                "关闭".into()
            }
        });
        let vsr_active = vsr
            .and_then(|value| value.active_quality)
            .map(quality_label)
            .unwrap_or("无")
            .to_string();
        let expected_sha256 = diagnostics
            .map(|value| value.expected_sha256.as_str())
            .unwrap_or(NVCPL_SHA256_HEX);
        let raw_hdr = diagnostics
            .and_then(|value| value.raw_hdr_hex.as_deref())
            .unwrap_or("—");
        let raw_vsr_info = diagnostics
            .and_then(|value| value.raw_vsr_info_hex.as_deref())
            .unwrap_or("—");
        let raw_vsr_value = diagnostics
            .and_then(|value| value.raw_vsr_value_hex.as_deref())
            .unwrap_or("—");
        v_flex()
            .w_full()
            .h_full()
            .gap_3()
            .p_4()
            .overflow_hidden()
            .rounded(px(14.))
            .border_1()
            .border_color(rgb(RULE))
            .bg(rgb(PANEL))
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(12.))
                            .text_color(rgb(TEXT))
                            .child("系统诊断"),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("copy-diagnostics")
                                    .ghost()
                                    .compact()
                                    .label("复制诊断")
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            app.diagnostic_text(),
                                        ));
                                        app.saved_note = Some("诊断信息已复制".into());
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("close-diagnostics")
                                    .ghost()
                                    .compact()
                                    .label("返回控制台")
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.page = UiPage::Overview;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(diagnostic_compact(
                        "NVCpl 路径",
                        diagnostics
                            .and_then(|value| value.nvcpl_path.as_deref())
                            .unwrap_or("未找到"),
                    ))
                    .child(diagnostic_compact("兼容模式", compatibility_text))
                    .child(diagnostic_compact("DLL 版本", dll_version))
                    .child(diagnostic_compact("读取探针", probe_text))
                    .child(diagnostic_compact("错误码", &error_code))
                    .child(diagnostic_compact("峰值", &peak))
                    .child(diagnostic_compact("峰值范围", &peak_range))
                    .child(diagnostic_compact("中间灰", &middle_gray))
                    .child(diagnostic_compact("对比度 / 饱和度", &tone))
                    .child(diagnostic_compact("VSR 状态", &vsr_status))
                    .child(diagnostic_compact("VSR 活动档位", &vsr_active)),
            )
            .child(
                v_flex()
                    .w_full()
                    .mt_auto()
                    .gap_2()
                    .child(raw_diagnostic_row("允许的 SHA-256", expected_sha256))
                    .child(raw_diagnostic_row("实际 SHA-256", actual_sha256))
                    .child(raw_diagnostic_row("原始 0x318", raw_hdr))
                    .child(raw_diagnostic_row("原始 0x315", raw_vsr_info))
                    .child(raw_diagnostic_row("原始 0x317", raw_vsr_value)),
            )
            .child(
                div().text_size(px(10.)).text_color(rgb(SUBTLE)).child(
                    diagnostics
                        .and_then(|value| value.last_error.as_deref())
                        .map(|error| format!("最近错误：{error}"))
                        .unwrap_or_else(|| "最近错误：无".into()),
                ),
            )
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let save_disabled = !self.dirty
            || self.saving
            || self.recording.is_some()
            || self.connection_error.is_some();
        let connection = if self.connection_error.is_some() {
            "IPC 未连接 · 后台未运行"
        } else if self.compatibility_read_only() {
            "HDR 0x318 + VSR 0x315 · 只读兼容模式"
        } else {
            "HDR 0x318 + VSR 0x315 · 后端已连接"
        };
        let note = self
            .transient_error
            .as_deref()
            .or(self.saved_note.as_deref())
            .unwrap_or(connection);
        h_flex()
            .w_full()
            .h(px(46.))
            .flex_shrink_0()
            .mt_auto()
            .items_center()
            .gap_2()
            .pt_2()
            .border_t_1()
            .border_color(rgb(RULE))
            .child(
                div()
                    .font_family("Cascadia Mono")
                    .text_size(px(9.))
                    .text_color(if self.transient_error.is_some() {
                        rgb(DANGER)
                    } else {
                        rgb(SUBTLE)
                    })
                    .child(note.to_string()),
            )
            .child(
                h_flex()
                    .ml_auto()
                    .gap_2()
                    .when(self.connection_error.is_some(), |this| {
                        this.child(Button::new("reconnect").ghost().label("重连后台").on_click(
                            cx.listener(|app, _, window, cx| {
                                app.reconnect(window, cx);
                            }),
                        ))
                    })
                    .child(
                        Button::new("reset")
                            .ghost()
                            .label("恢复默认")
                            .on_click(cx.listener(|app, _, _, cx| app.reset_defaults(cx))),
                    )
                    .child(
                        Button::new("save")
                            .custom(
                                ButtonCustomVariant::new(cx)
                                    .color(rgb(SIGNAL).into())
                                    .foreground(rgb(INK).into())
                                    .hover(rgb(SIGNAL_HOVER).into())
                                    .active(rgb(0x51CBD4).into()),
                            )
                            .label(if self.saving {
                                "保存中…"
                            } else {
                                "保存更改"
                            })
                            .loading(self.saving)
                            .disabled(save_disabled)
                            .on_click(cx.listener(|app, _, _, cx| app.save(cx))),
                    ),
            )
    }
}

fn format_diagnostic_text(snapshot: &UiSnapshot) -> String {
    let mut lines = vec![
        "RTX Video Manager diagnostics".to_string(),
        format!("app_version={}", snapshot.app_version),
    ];
    let diagnostics = &snapshot.diagnostics;
    lines.push(format!(
        "nvcpl_path={}",
        diagnostics.nvcpl_path.as_deref().unwrap_or("not_found")
    ));
    lines.push(format!("expected_sha256={}", diagnostics.expected_sha256));
    if let Some(compatibility) = &diagnostics.compatibility {
        lines.push(format!("compatibility={:?}", compatibility.mode));
        lines.push(format!("actual_sha256={}", compatibility.actual_sha256));
        lines.push(format!(
            "dll_version={}",
            compatibility.dll_version.as_deref().unwrap_or("unknown")
        ));
        lines.push(format!("probe_passed={}", compatibility.probe_passed));
        if let Some(reason) = &compatibility.reason {
            lines.push(format!("reason={reason}"));
        }
    }
    if let Some(raw) = &diagnostics.raw_hdr_hex {
        lines.push(format!("raw_0x318={raw}"));
    }
    if let Some(raw) = &diagnostics.raw_vsr_info_hex {
        lines.push(format!("raw_0x315={raw}"));
    }
    if let Some(raw) = &diagnostics.raw_vsr_value_hex {
        lines.push(format!("raw_0x317={raw}"));
    }
    if let Some(last_error) = &diagnostics.last_error {
        lines.push(format!("last_error={last_error}"));
    }
    if let UiHdrState::Ready(hdr) = &snapshot.hdr {
        lines.push(format!("supported={}", hdr.supported));
        lines.push(format!("enabled={}", hdr.enabled));
        lines.push(format!("active={}", hdr.active));
        lines.push(format!("error_code={}", hdr.error_code));
        lines.push(format!("peak_brightness={}", hdr.peak_brightness));
        lines.push(format!("peak_range={}..{}", hdr.peak_min, hdr.peak_max));
        lines.push(format!("middle_gray={}", hdr.middle_gray));
        lines.push(format!("contrast={}", hdr.contrast));
        lines.push(format!("saturation={}", hdr.saturation));
    }
    if let UiVsrState::Ready(vsr) = &snapshot.vsr {
        lines.push(format!("vsr_supported={}", vsr.supported));
        lines.push(format!("vsr_enabled={}", vsr.enabled));
        lines.push(format!("vsr_active={}", vsr.active));
        lines.push(format!("vsr_quality={}", vsr.quality.raw()));
        lines.push(format!("vsr_status={}", vsr.status));
    }
    lines.join("\n")
}

impl Render for ConsoleApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.page {
            UiPage::Overview => v_flex()
                .w_full()
                .h_full()
                .gap_3()
                .child(
                    h_flex()
                        .w_full()
                        .flex_1()
                        .min_h(px(0.))
                        .gap_3()
                        .child(self.render_overview_hdr_card(cx))
                        .child(self.render_overview_vsr_card(cx)),
                )
                .child(self.render_overview_summary())
                .into_any_element(),
            UiPage::Hdr => v_flex()
                .w_full()
                .h_full()
                .gap_3()
                .child(
                    h_flex()
                        .w_full()
                        .h(px(154.))
                        .flex_shrink_0()
                        .gap_3()
                        .child(self.render_hero(cx))
                        .child(self.render_driver_info()),
                )
                .child(self.render_tuning(cx))
                .into_any_element(),
            UiPage::Vsr => v_flex()
                .w_full()
                .h_full()
                .gap_3()
                .child(
                    h_flex()
                        .w_full()
                        .h(px(164.))
                        .flex_shrink_0()
                        .gap_3()
                        .child(self.render_vsr_hero(cx))
                        .child(self.render_video_info()),
                )
                .child(self.render_vsr_quality(cx))
                .into_any_element(),
            UiPage::Settings => self.render_settings(cx).into_any_element(),
            UiPage::Diagnostics => self.render_diagnostics(cx).into_any_element(),
        };
        v_flex()
            .id("console-root")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key_down))
            .size_full()
            .font_family("Microsoft YaHei UI")
            .bg(rgb(INK))
            .text_color(rgb(TEXT))
            .child(self.render_title_bar(cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .px(px(28.))
                    .pt(px(14.))
                    .pb(px(14.))
                    .gap_3()
                    .child(self.render_tabs(cx))
                    .child(v_flex().flex_1().min_h(px(0.)).child(content))
                    .child(self.render_footer(cx)),
            )
    }
}

fn spectrum_bar() -> impl IntoElement {
    h_flex()
        .w_full()
        .h(px(12.))
        .overflow_hidden()
        .rounded(px(5.))
        .border_1()
        .border_color(rgb(0x294249))
        .children(
            [0x16262B, 0x21464B, 0x3C9299, 0x9BE7E9]
                .into_iter()
                .map(|color| div().flex_1().h_full().bg(rgb(color))),
        )
}

fn vsr_ladder() -> impl IntoElement {
    h_flex()
        .w_full()
        .h(px(12.))
        .gap_1()
        .children([0.16, 0.24, 0.34, 0.46, 0.74].into_iter().map(|opacity| {
            div()
                .flex_1()
                .h_full()
                .rounded(px(4.))
                .border_1()
                .border_color(rgb(0x685A42))
                .bg(rgb(WARM).opacity(opacity))
        }))
}

fn summary_cell(label: &'static str, value: String) -> impl IntoElement {
    v_flex()
        .flex_1()
        .min_w(px(0.))
        .h_full()
        .justify_center()
        .gap_1()
        .px_4()
        .child(
            div()
                .font_family("Cascadia Mono")
                .text_size(px(8.))
                .text_color(rgb(SUBTLE))
                .child(label),
        )
        .child(
            div()
                .truncate()
                .text_size(px(10.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(0xDBE8E9))
                .child(value),
        )
}

fn quality_guide_cell(label: &'static str, detail: &'static str) -> impl IntoElement {
    v_flex()
        .flex_1()
        .h(px(44.))
        .min_w(px(0.))
        .justify_center()
        .gap(px(2.))
        .px_3()
        .rounded(px(9.))
        .border_1()
        .border_color(rgb(RULE))
        .bg(rgb(RAISED))
        .child(
            div()
                .font_family("Cascadia Mono")
                .text_size(px(9.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(0xDBE8E9))
                .child(label),
        )
        .child(
            div()
                .truncate()
                .text_size(px(9.))
                .text_color(rgb(SUBTLE))
                .child(detail),
        )
}

fn info_value(label: &'static str, value: &str) -> impl IntoElement {
    v_flex()
        .w_full()
        .min_w(px(0.))
        .gap(px(2.))
        .child(
            div()
                .font_family("Cascadia Mono")
                .text_size(px(8.))
                .text_color(rgb(SUBTLE))
                .child(label),
        )
        .child(
            div()
                .w_full()
                .min_w(px(0.))
                .overflow_hidden()
                .truncate()
                .font_family("Cascadia Mono")
                .text_size(px(9.))
                .text_color(rgb(0xC8D6D7))
                .child(value.to_string()),
        )
}

fn behavior_row(
    label: &'static str,
    title: &'static str,
    detail: &'static str,
    switch: Switch,
) -> impl IntoElement {
    h_flex()
        .h(px(47.))
        .flex_shrink_0()
        .items_center()
        .gap_3()
        .px_4()
        .border_b_1()
        .border_color(rgb(RULE))
        .child(
            div()
                .w(px(92.))
                .text_size(px(12.))
                .text_color(rgb(MUTED))
                .child(label),
        )
        .child(
            v_flex()
                .min_w_0()
                .gap(px(1.))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(0xDFE9E9))
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(rgb(SUBTLE))
                        .child(detail),
                ),
        )
        .child(div().ml_auto().child(switch))
}

fn keycap(label: &str) -> impl IntoElement {
    div()
        .min_w(px(30.))
        .h(px(25.))
        .flex()
        .items_center()
        .justify_center()
        .px_2()
        .rounded(px(6.))
        .border_1()
        .border_color(rgb(0x395158))
        .bg(rgb(0x18282D))
        .font_family("Bahnschrift")
        .text_size(px(11.))
        .text_color(rgb(0xDCEAEB))
        .child(label.to_string())
}

fn diagnostic_compact(label: &str, value: &str) -> impl IntoElement {
    v_flex()
        .w(px(244.))
        .min_w(px(0.))
        .gap(px(1.))
        .child(
            div()
                .text_size(px(9.))
                .text_color(rgb(SUBTLE))
                .child(label.to_string()),
        )
        .child(
            div()
                .w_full()
                .min_w(px(0.))
                .overflow_hidden()
                .font_family("Cascadia Mono")
                .text_size(px(9.))
                .text_color(rgb(0xC8D6D7))
                .truncate()
                .child(value.to_string()),
        )
}

fn raw_diagnostic_row(label: &str, value: &str) -> impl IntoElement {
    h_flex()
        .w_full()
        .min_w(px(0.))
        .gap_3()
        .child(
            div()
                .w(px(112.))
                .flex_shrink_0()
                .font_family("Cascadia Mono")
                .text_size(px(8.))
                .text_color(rgb(SUBTLE))
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .font_family("Cascadia Mono")
                .text_size(px(9.))
                .text_color(rgb(0xC8D6D7))
                .truncate()
                .child(value.to_string()),
        )
}

fn quality_label(quality: VsrQuality) -> &'static str {
    match quality {
        VsrQuality::Off => "关闭",
        VsrQuality::Level1 => "1",
        VsrQuality::Level2 => "2",
        VsrQuality::Level3 => "3",
        VsrQuality::Level4 => "4",
        VsrQuality::Auto => "自动",
    }
}

fn vsr_quality_id(quality: VsrQuality) -> &'static str {
    match quality {
        VsrQuality::Off => "vsr-off",
        VsrQuality::Level1 => "vsr-level-1",
        VsrQuality::Level2 => "vsr-level-2",
        VsrQuality::Level3 => "vsr-level-3",
        VsrQuality::Level4 => "vsr-level-4",
        VsrQuality::Auto => "vsr-auto",
    }
}

fn error_for_code(code: i32) -> &'static str {
    match code {
        1 => "未检测到活动的 HDR10 显示器",
        2 => "Windows HDR 未开启",
        3 => "NVIDIA Surround 已开启，与 RTX 视频 HDR 冲突",
        _ => "NVIDIA 返回了未知状态",
    }
}

fn hotkey_from_keystroke(keystroke: &Keystroke) -> Result<Hotkey, String> {
    let mut modifiers = 0;
    if keystroke.modifiers.control {
        modifiers |= MOD_CONTROL;
    }
    if keystroke.modifiers.alt {
        modifiers |= MOD_ALT;
    }
    if keystroke.modifiers.shift {
        modifiers |= MOD_SHIFT;
    }
    if keystroke.modifiers.platform {
        modifiers |= MOD_WIN;
    }
    if modifiers == 0 {
        return Err("快捷键至少需要一个修饰键（Ctrl、Alt、Shift 或 Win）。".into());
    }
    let Some(vk) = virtual_key_for(&keystroke.key) else {
        return Err(format!("暂不支持将 {} 用作全局快捷键。", keystroke.key));
    };
    let hotkey = Hotkey { modifiers, vk };
    hotkey
        .validate()
        .map_err(|error: AppError| error.user_message())?;
    Ok(hotkey)
}

fn virtual_key_for(key: &str) -> Option<u32> {
    if key.len() == 1 {
        let byte = key.as_bytes()[0];
        if byte.is_ascii_alphabetic() {
            return Some(byte.to_ascii_uppercase() as u32);
        }
        if byte.is_ascii_digit() {
            return Some(byte as u32);
        }
    }
    if let Some(number) = key
        .strip_prefix('f')
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| (1..=24).contains(value))
    {
        return Some(0x6F + number);
    }
    Some(match key.to_ascii_lowercase().as_str() {
        "backspace" => 0x08,
        "tab" => 0x09,
        "enter" => 0x0D,
        "space" => 0x20,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "end" => 0x23,
        "home" => 0x24,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        "insert" => 0x2D,
        "delete" => 0x2E,
        ";" | "semicolon" => 0xBA,
        "=" | "plus" | "equal" => 0xBB,
        "," | "comma" => 0xBC,
        "-" | "minus" => 0xBD,
        "." | "period" => 0xBE,
        "/" | "slash" => 0xBF,
        "`" | "backquote" => 0xC0,
        "[" | "leftbracket" => 0xDB,
        "\\" | "backslash" => 0xDC,
        "]" | "rightbracket" => 0xDD,
        "'" | "quote" => 0xDE,
        _ => return None,
    })
}

fn apply_visual_theme(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |theme| {
        theme.font_family = "Microsoft YaHei UI".into();
        theme.mono_font_family = "Cascadia Mono".into();
        theme.background = rgb(INK).into();
        theme.foreground = rgb(TEXT).into();
        theme.border = rgb(RULE).into();
        theme.input = rgb(RULE).into();
        theme.muted_foreground = rgb(MUTED).into();
        theme.primary = rgb(SIGNAL).into();
        theme.primary_foreground = rgb(INK).into();
        theme.primary_hover = rgb(SIGNAL_HOVER).into();
        theme.ring = rgb(SIGNAL).into();
        theme.title_bar = rgb(0x101A1E).into();
        theme.title_bar_border = rgb(RULE).into();
        theme.secondary = rgb(RAISED).into();
        theme.secondary_foreground = rgb(TEXT).into();
        theme.secondary_hover = rgb(0x203138).into();
        theme.danger = rgb(DANGER).into();
    });
}

fn main() {
    let mutex_name = wide(UI_MUTEX);
    let mutex = unsafe { CreateMutexW(null(), 0, mutex_name.as_ptr()) };
    if mutex.is_null() {
        return;
    }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        ipc_client::activate_existing();
        unsafe { CloseHandle(mutex) };
        return;
    }

    let initial_connection = IpcClient::connect_retry();
    log_line("GPUI console starting");
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            apply_visual_theme(cx);
            let window_options = WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(860.), px(620.)), cx)),
                is_resizable: false,
                is_minimizable: true,
                ..TitleBar::window_options()
            };
            gpui_kit::open_window(window_options, cx, move |window, cx| {
                window.set_window_title("RTX Video Manager");
                cx.new(|cx| ConsoleApp::new(initial_connection, window, cx))
            })
            .expect("failed to open RTX Video Manager");
        });

    unsafe { CloseHandle(mutex) };
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn log_line(text: &str) {
    let Some(base) = std::env::var_os("LOCALAPPDATA") else {
        return;
    };
    let directory = std::path::PathBuf::from(base).join("RtxHdrSwitch");
    let _ = std::fs::create_dir_all(&directory);
    let path = directory.join("rtx-video-manager-console.log");
    if std::fs::metadata(&path)
        .map(|metadata| metadata.len() > 1_048_576)
        .unwrap_or(false)
    {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let _ = writeln!(file, "[{timestamp}] {text}");
    }
}

#[cfg(test)]
mod tests {
    use super::{error_for_code, format_diagnostic_text, hotkey_from_keystroke};
    use gpui_kit::{Keystroke, Modifiers};
    use rtx_video_manager::ipc::{DiagnosticsInfo, UiHdrState, UiSnapshot, UiVsrState};
    use rtx_video_manager::{
        CompatibilityInfo, CompatibilityMode, HdrSnapshot, Hotkey, NVCPL_SHA256_HEX, Settings,
        VsrQuality, VsrSnapshot,
    };

    fn stroke(modifiers: Modifiers, key: &str) -> Keystroke {
        Keystroke {
            modifiers,
            key: key.into(),
            key_char: None,
        }
    }

    #[test]
    fn converts_letters_digits_and_named_keys() {
        assert_eq!(
            hotkey_from_keystroke(&stroke(
                Modifiers {
                    control: true,
                    alt: true,
                    ..Default::default()
                },
                "h"
            )),
            Ok(Hotkey::default())
        );
        assert_eq!(
            hotkey_from_keystroke(&stroke(Modifiers::control(), "space"))
                .unwrap()
                .vk,
            0x20
        );
        assert_eq!(
            hotkey_from_keystroke(&stroke(Modifiers::alt(), "f12"))
                .unwrap()
                .vk,
            0x7B
        );
    }

    #[test]
    fn rejects_unmodified_or_modifier_only_keystrokes() {
        assert!(hotkey_from_keystroke(&stroke(Modifiers::none(), "h")).is_err());
        assert!(hotkey_from_keystroke(&stroke(Modifiers::control(), "control")).is_err());
    }

    #[test]
    fn maps_hdr_error_codes_to_actionable_copy() {
        assert!(error_for_code(1).contains("HDR10"));
        assert!(error_for_code(2).contains("Windows HDR"));
        assert!(error_for_code(3).contains("Surround"));
    }

    #[test]
    fn formats_read_only_diagnostics_for_clipboard() {
        let snapshot = UiSnapshot {
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
                quality: VsrQuality::Level4,
                active_quality: None,
                status: 0,
            }),
            settings: Settings::default(),
            hotkey_registered: true,
            busy: None,
            diagnostics: DiagnosticsInfo {
                nvcpl_path: Some(r"C:\NVIDIA\nvcpl.dll".into()),
                expected_sha256: NVCPL_SHA256_HEX.into(),
                compatibility: Some(CompatibilityInfo {
                    mode: CompatibilityMode::ReadOnlyUnknownBuild,
                    actual_sha256: "abcd".into(),
                    expected_sha256: NVCPL_SHA256_HEX.into(),
                    dll_version: Some("8.17.15.5527".into()),
                    probe_passed: true,
                    reason: Some("hash changed".into()),
                }),
                raw_hdr_hex: Some("00010203".into()),
                raw_vsr_info_hex: Some("0100000004000000".into()),
                raw_vsr_value_hex: Some("04000000".into()),
                last_error: Some("read only".into()),
            },
        };

        let text = format_diagnostic_text(&snapshot);
        assert!(text.contains("compatibility=ReadOnlyUnknownBuild"));
        assert!(text.contains("actual_sha256=abcd"));
        assert!(text.contains("raw_0x318=00010203"));
        assert!(text.contains("raw_0x315=0100000004000000"));
        assert!(text.contains("raw_0x317=04000000"));
        assert!(text.contains("vsr_quality=4"));
        assert!(text.contains("peak_range=400..1440"));
        assert!(text.contains("last_error=read only"));
    }
}
