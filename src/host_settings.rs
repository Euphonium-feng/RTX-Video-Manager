use rtx_video_manager::{AppError, Settings};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recovery {
    NotNeeded,
    Restored,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplyFailure {
    pub cause: AppError,
    pub recovery: Recovery,
    pub hotkey_recovery: Recovery,
}

impl ApplyFailure {
    pub fn user_message(&self) -> String {
        let cause = self.cause.user_message();
        if self.recovery == Recovery::Failed {
            format!("{cause}\n\n部分设置未能恢复，请重新打开控制台检查快捷键和开机启动。")
        } else {
            cause
        }
    }

    pub fn hotkey_registered_after(&self, was_registered: bool) -> bool {
        match self.hotkey_recovery {
            Recovery::NotNeeded => was_registered,
            Recovery::Restored => true,
            Recovery::Failed => false,
        }
    }
}

pub trait HotkeyPort {
    fn register(&mut self, settings: &Settings) -> Result<(), AppError>;
}

pub trait StartupPort {
    fn set_enabled(&mut self, enabled: bool) -> Result<(), AppError>;
}

pub trait SettingsPort {
    fn persist(&mut self, settings: &Settings) -> Result<(), AppError>;
}

pub struct ApplyPorts<'a> {
    pub hotkeys: &'a mut dyn HotkeyPort,
    pub startup: &'a mut dyn StartupPort,
    pub settings: &'a mut dyn SettingsPort,
}

pub fn apply_transaction(
    candidate: &Settings,
    previous: &Settings,
    was_registered: bool,
    ports: &mut ApplyPorts<'_>,
) -> Result<(), ApplyFailure> {
    candidate
        .validate()
        .map_err(|cause| ApplyFailure {
            cause,
            recovery: Recovery::NotNeeded,
            hotkey_recovery: Recovery::NotNeeded,
        })?;

    let hotkeys_changed = candidate.hotkey != previous.hotkey
        || candidate.vsr_hotkey != previous.vsr_hotkey
        || candidate.show_console_hotkey != previous.show_console_hotkey
        || !was_registered;
    let startup_changed = candidate.start_at_login != previous.start_at_login;

    if hotkeys_changed {
        if let Err(cause) = ports.hotkeys.register(candidate) {
            let restored = ports.hotkeys.register(previous).is_ok();
            return Err(ApplyFailure {
                cause,
                recovery: if restored {
                    Recovery::Restored
                } else {
                    Recovery::Failed
                },
                hotkey_recovery: if restored {
                    Recovery::Restored
                } else {
                    Recovery::Failed
                },
            });
        }
    }

    if startup_changed {
        if let Err(cause) = ports.startup.set_enabled(candidate.start_at_login) {
            let mut restored = ports.startup.set_enabled(previous.start_at_login).is_ok();
            let hotkey_recovery = if hotkeys_changed {
                if ports.hotkeys.register(previous).is_ok() {
                    Recovery::Restored
                } else {
                    restored = false;
                    Recovery::Failed
                }
            } else {
                Recovery::NotNeeded
            };
            return Err(ApplyFailure {
                cause,
                recovery: if restored {
                    Recovery::Restored
                } else {
                    Recovery::Failed
                },
                hotkey_recovery,
            });
        }
    }

    if let Err(cause) = ports.settings.persist(candidate) {
        let mut restored = true;
        if startup_changed {
            restored &= ports.startup.set_enabled(previous.start_at_login).is_ok();
        }
        let hotkey_recovery = if hotkeys_changed {
            if ports.hotkeys.register(previous).is_ok() {
                Recovery::Restored
            } else {
                restored = false;
                Recovery::Failed
            }
        } else {
            Recovery::NotNeeded
        };
        return Err(ApplyFailure {
            cause,
            recovery: if restored {
                Recovery::Restored
            } else {
                Recovery::Failed
            },
            hotkey_recovery,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtx_video_manager::{Hotkey, VsrQuality};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    struct FakeHotkeys {
        calls: Rc<RefCell<Vec<Settings>>>,
        fail_on: Rc<RefCell<Vec<AppError>>>,
    }

    impl HotkeyPort for FakeHotkeys {
        fn register(&mut self, settings: &Settings) -> Result<(), AppError> {
            self.calls.borrow_mut().push(settings.clone());
            let failure = self.fail_on.borrow_mut().pop();
            match failure {
                Some(error) => Err(error),
                None => Ok(()),
            }
        }
    }

    #[derive(Default)]
    struct FakeStartup {
        calls: Rc<RefCell<Vec<bool>>>,
        fail_on: Rc<RefCell<Vec<AppError>>>,
    }

    impl StartupPort for FakeStartup {
        fn set_enabled(&mut self, enabled: bool) -> Result<(), AppError> {
            self.calls.borrow_mut().push(enabled);
            let failure = self.fail_on.borrow_mut().pop();
            match failure {
                Some(error) => Err(error),
                None => Ok(()),
            }
        }
    }

    #[derive(Default)]
    struct FakeSettings {
        calls: Rc<RefCell<Vec<Settings>>>,
        fail_on: Rc<RefCell<Vec<AppError>>>,
    }

    impl SettingsPort for FakeSettings {
        fn persist(&mut self, settings: &Settings) -> Result<(), AppError> {
            self.calls.borrow_mut().push(settings.clone());
            let failure = self.fail_on.borrow_mut().pop();
            match failure {
                Some(error) => Err(error),
                None => Ok(()),
            }
        }
    }

    fn candidate() -> Settings {
        Settings::default()
    }

    fn failure() -> AppError {
        AppError::Settings("test failure".into())
    }

    #[test]
    fn vsr_only_hotkey_change_is_registered_and_restored_on_save_failure() {
        let previous = candidate();
        let changed = Settings {
            vsr_hotkey: Hotkey {
                modifiers: rtx_video_manager::MOD_CONTROL,
                vk: b'J' as u32,
            },
            ..candidate()
        };
        let mut hotkeys = FakeHotkeys::default();
        let mut startup = FakeStartup::default();
        let mut settings = FakeSettings {
            fail_on: Rc::new(RefCell::new(vec![failure()])),
            ..FakeSettings::default()
        };
        let hotkey_calls = hotkeys.calls.clone();
        let mut ports = ApplyPorts {
            hotkeys: &mut hotkeys,
            startup: &mut startup,
            settings: &mut settings,
        };
        let failure = apply_transaction(&changed, &previous, true, &mut ports).unwrap_err();
        assert_eq!(failure.recovery, Recovery::Restored);
        assert_eq!(failure.hotkey_recovery, Recovery::Restored);
        let calls = hotkey_calls.borrow();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].vsr_hotkey, changed.vsr_hotkey);
        assert_eq!(calls[1].vsr_hotkey, previous.vsr_hotkey);
    }

    #[test]
    fn hotkey_conflict_leaves_startup_and_storage_untouched() {
        let previous = candidate();
        let changed = Settings {
            vsr_hotkey: Hotkey {
                modifiers: rtx_video_manager::MOD_CONTROL,
                vk: b'J' as u32,
            },
            start_at_login: false,
            ..candidate()
        };
        let mut hotkeys = FakeHotkeys {
            fail_on: Rc::new(RefCell::new(vec![failure()])),
            ..FakeHotkeys::default()
        };
        let mut startup = FakeStartup::default();
        let mut settings = FakeSettings::default();
        let startup_calls = startup.calls.clone();
        let settings_calls = settings.calls.clone();
        let mut ports = ApplyPorts {
            hotkeys: &mut hotkeys,
            startup: &mut startup,
            settings: &mut settings,
        };
        let error = apply_transaction(&changed, &previous, true, &mut ports).unwrap_err();
        assert_eq!(error.cause.user_message(), failure().user_message());
        assert!(startup_calls.borrow().is_empty());
        assert!(settings_calls.borrow().is_empty());
    }

    #[test]
    fn autostart_failure_restores_hotkeys() {
        let previous = candidate();
        let changed = Settings {
            hotkey: Hotkey {
                modifiers: rtx_video_manager::MOD_ALT,
                vk: b'K' as u32,
            },
            start_at_login: false,
            ..candidate()
        };
        let mut hotkeys = FakeHotkeys::default();
        let mut startup = FakeStartup {
            fail_on: Rc::new(RefCell::new(vec![failure()])),
            ..FakeStartup::default()
        };
        let mut settings = FakeSettings::default();
        let hotkey_calls = hotkeys.calls.clone();
        let settings_calls = settings.calls.clone();
        let mut ports = ApplyPorts {
            hotkeys: &mut hotkeys,
            startup: &mut startup,
            settings: &mut settings,
        };
        let failure = apply_transaction(&changed, &previous, true, &mut ports).unwrap_err();
        assert_eq!(failure.recovery, Recovery::Restored);
        assert_eq!(failure.hotkey_recovery, Recovery::Restored);
        assert_eq!(hotkey_calls.borrow().len(), 2);
        assert!(settings_calls.borrow().is_empty());
    }

    #[test]
    fn save_failure_restores_startup_and_hotkeys() {
        let previous = candidate();
        let changed = Settings {
            hotkey: Hotkey {
                modifiers: rtx_video_manager::MOD_SHIFT,
                vk: b'L' as u32,
            },
            start_at_login: false,
            ..candidate()
        };
        let mut hotkeys = FakeHotkeys::default();
        let mut startup = FakeStartup::default();
        let mut settings = FakeSettings {
            fail_on: Rc::new(RefCell::new(vec![failure()])),
            ..FakeSettings::default()
        };
        let startup_calls = startup.calls.clone();
        let mut ports = ApplyPorts {
            hotkeys: &mut hotkeys,
            startup: &mut startup,
            settings: &mut settings,
        };
        apply_transaction(&changed, &previous, true, &mut ports).unwrap_err();
        assert_eq!(*startup_calls.borrow(), vec![false, true]);
    }

    #[test]
    fn failed_rollback_is_reported_as_partial() {
        let previous = candidate();
        let changed = Settings {
            hotkey: Hotkey {
                modifiers: rtx_video_manager::MOD_ALT,
                vk: b'K' as u32,
            },
            start_at_login: false,
            ..candidate()
        };
        let mut hotkeys = FakeHotkeys {
            fail_on: Rc::new(RefCell::new(vec![failure(), failure()])),
            ..FakeHotkeys::default()
        };
        let mut startup = FakeStartup::default();
        let mut settings = FakeSettings::default();
        let mut ports = ApplyPorts {
            hotkeys: &mut hotkeys,
            startup: &mut startup,
            settings: &mut settings,
        };
        let failure = apply_transaction(&changed, &previous, true, &mut ports).unwrap_err();
        assert_eq!(failure.recovery, Recovery::Failed);
        assert!(failure.user_message().contains("部分设置未能恢复"));
    }

    #[test]
    fn successful_apply_persists_once_and_skips_unchanged_hotkeys() {
        let previous = candidate();
        let changed = Settings {
            notifications: false,
            last_vsr_quality: VsrQuality::Level2,
            ..candidate()
        };
        let mut hotkeys = FakeHotkeys::default();
        let mut startup = FakeStartup::default();
        let mut settings = FakeSettings::default();
        let hotkey_calls = hotkeys.calls.clone();
        let settings_calls = settings.calls.clone();
        let mut ports = ApplyPorts {
            hotkeys: &mut hotkeys,
            startup: &mut startup,
            settings: &mut settings,
        };
        apply_transaction(&changed, &previous, true, &mut ports).unwrap();
        assert!(hotkey_calls.borrow().is_empty());
        assert_eq!(*settings_calls.borrow(), vec![changed]);
    }

    #[test]
    fn unregistered_hotkeys_are_reapplied_even_when_values_match() {
        let previous = candidate();
        let mut hotkeys = FakeHotkeys::default();
        let mut startup = FakeStartup::default();
        let mut settings = FakeSettings::default();
        let hotkey_calls = hotkeys.calls.clone();
        let mut ports = ApplyPorts {
            hotkeys: &mut hotkeys,
            startup: &mut startup,
            settings: &mut settings,
        };
        apply_transaction(&previous, &previous, false, &mut ports).unwrap();
        assert_eq!(hotkey_calls.borrow().len(), 1);
    }
}
