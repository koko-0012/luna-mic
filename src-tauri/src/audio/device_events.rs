//! Endpoint notifications do no enumeration or stream work on the COM callback.
//! They only mark a dirty flag that the control worker consumes.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
#[cfg(windows)]
mod platform {
    use super::*;
    use windows::{
        core::{implement, Result, PCWSTR},
        Win32::{Media::Audio::*, System::Com::*, UI::Shell::PropertiesSystem::PROPERTYKEY},
    };
    #[implement(IMMNotificationClient)]
    struct Listener {
        dirty: Arc<AtomicBool>,
    }
    impl Listener {
        fn mark(&self) -> Result<()> {
            self.dirty.store(true, Ordering::Release);
            Ok(())
        }
    }
    #[allow(non_snake_case)]
    impl IMMNotificationClient_Impl for Listener {
        fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> Result<()> {
            self.mark()
        }
        fn OnDeviceAdded(&self, _: &PCWSTR) -> Result<()> {
            self.mark()
        }
        fn OnDeviceRemoved(&self, _: &PCWSTR) -> Result<()> {
            self.mark()
        }
        fn OnDefaultDeviceChanged(&self, _: EDataFlow, _: ERole, _: &PCWSTR) -> Result<()> {
            self.mark()
        }
        fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> Result<()> {
            self.mark()
        }
    }
    pub struct DeviceEvents {
        enumerator: IMMDeviceEnumerator,
        listener: IMMNotificationClient,
        pub dirty: Arc<AtomicBool>,
    }
    impl DeviceEvents {
        pub fn new() -> Option<Self> {
            // The control worker owns this MTA registration and drops it before
            // leaving the thread. COM reference ownership is managed by windows-rs.
            unsafe {
                CoInitializeEx(None, COINIT_MULTITHREADED).ok().ok()?;
                let result = (|| -> Result<Self> {
                    let enumerator: IMMDeviceEnumerator =
                        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
                    let dirty = Arc::new(AtomicBool::new(true));
                    let listener: IMMNotificationClient = Listener {
                        dirty: dirty.clone(),
                    }
                    .into();
                    enumerator.RegisterEndpointNotificationCallback(&listener)?;
                    Ok(Self {
                        enumerator,
                        listener,
                        dirty,
                    })
                })();
                match result {
                    Ok(events) => Some(events),
                    Err(_) => {
                        CoUninitialize();
                        None
                    }
                }
            }
        }
    }
    impl Drop for DeviceEvents {
        fn drop(&mut self) {
            unsafe {
                let _ = self
                    .enumerator
                    .UnregisterEndpointNotificationCallback(&self.listener);
                CoUninitialize();
            }
        }
    }
}
#[cfg(windows)]
pub use platform::DeviceEvents;
#[cfg(not(windows))]
pub struct DeviceEvents {
    pub dirty: Arc<AtomicBool>,
}
#[cfg(not(windows))]
impl DeviceEvents {
    pub fn new() -> Option<Self> {
        None
    }
}
