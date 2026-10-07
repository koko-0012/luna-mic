//! Explicit Windows default-microphone action, isolated from capture and DSP.
//!
//! Windows exposes public APIs to read defaults, but the desktop setter is the
//! undocumented PolicyConfig COM interface. Its IID / method order were checked
//! against EarTrumpet's IPolicyConfigWin7 declaration and AudioEndPointController.
//! We only call SetDefaultEndpoint (slot 13 including IUnknown). If Windows stops
//! exposing this interface, return an error and let the user open Sound settings.
//! No registry edits, drivers, network access, or callback-thread calls are used.

#[cfg(windows)]
mod platform {
    use std::{ffi::c_void, ptr};
    use windows::{
        core::{IUnknown, IUnknown_Vtbl, Interface, BSTR, GUID, HRESULT, PCWSTR},
        Win32::{
            Devices::Properties::DEVPKEY_Device_FriendlyName, Media::Audio::*, System::Com::*,
            UI::Shell::PropertiesSystem::PROPERTYKEY,
        },
    };

    const POLICY_CLASS: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
    const POLICY_INTERFACE: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);
    const ROLES: [ERole; 3] = [eConsole, eMultimedia, eCommunications];

    #[repr(C)]
    struct PolicyVtable {
        unknown: IUnknown_Vtbl,
        // Ten preceding methods are not called. Pointer-sized slots preserve ABI.
        unused: [usize; 10],
        set_default: unsafe extern "system" fn(*mut c_void, PCWSTR, ERole) -> HRESULT,
    }
    struct Apartment;
    impl Apartment {
        fn new() -> windows::core::Result<Self> {
            unsafe {
                CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            }
            Ok(Self)
        }
    }
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }

    unsafe fn endpoint_id(device: &IMMDevice) -> windows::core::Result<String> {
        let wide = device.GetId()?;
        let text = wide.to_string();
        CoTaskMemFree(Some(wide.0.cast()));
        Ok(text?)
    }
    fn policy() -> windows::core::Result<IUnknown> {
        unsafe {
            let object: IUnknown = CoCreateInstance(&POLICY_CLASS, None, CLSCTX_ALL)?;
            let mut raw = ptr::null_mut();
            (object.vtable().QueryInterface)(object.as_raw(), &POLICY_INTERFACE, &mut raw).ok()?;
            // QueryInterface succeeded and transferred one owning COM reference.
            Ok(IUnknown::from_raw(raw))
        }
    }
    fn set_role(policy: &IUnknown, id: &str, role: ERole) -> windows::core::Result<()> {
        let wide: Vec<u16> = id.encode_utf16().chain(Some(0)).collect();
        unsafe {
            // `policy` was obtained by this exact IID. Its first slots are IUnknown,
            // so the owning wrapper safely releases the same COM interface later.
            let table = &**(policy.as_raw() as *const *const PolicyVtable);
            (table.set_default)(policy.as_raw(), PCWSTR(wide.as_ptr()), role).ok()
        }
    }

    /// Exposed for an opt-in manual integration test; reading does not change defaults.
    pub fn default_capture_ids() -> Result<[String; 3], String> {
        let _apartment = Apartment::new().map_err(|e| e.to_string())?;
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .map_err(|e| e.to_string())?;
            let mut result = std::array::from_fn(|_| String::new());
            for (i, role) in ROLES.into_iter().enumerate() {
                result[i] = endpoint_id(
                    &enumerator
                        .GetDefaultAudioEndpoint(eCapture, role)
                        .map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
            }
            Ok(result)
        }
    }
    pub fn restore_capture_ids(ids: &[String; 3]) -> Result<(), String> {
        let _apartment = Apartment::new().map_err(|e| e.to_string())?;
        let policy = policy().map_err(|e| e.to_string())?;
        for (id, role) in ids.iter().zip(ROLES) {
            set_role(&policy, id, role).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn set_default_microphone(selected: &str) -> Result<String, String> {
        let (name, occurrence) = selected
            .rsplit_once("::")
            .ok_or("Invalid microphone selection")?;
        let occurrence = occurrence
            .parse::<usize>()
            .map_err(|_| "Invalid microphone selection")?;
        let _apartment = Apartment::new().map_err(|e| e.to_string())?;
        let (id, previous, enumerator) = unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .map_err(|e| e.to_string())?;
            let endpoints = enumerator
                .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)
                .map_err(|e| e.to_string())?;
            let key = PROPERTYKEY {
                fmtid: DEVPKEY_Device_FriendlyName.fmtid,
                pid: DEVPKEY_Device_FriendlyName.pid,
            };
            let mut matches = Vec::new();
            for i in 0..endpoints.GetCount().map_err(|e| e.to_string())? {
                let device = endpoints.Item(i).map_err(|e| e.to_string())?;
                let store = device
                    .OpenPropertyStore(STGM_READ)
                    .map_err(|e| e.to_string())?;
                let value = store.GetValue(&key).map_err(|e| e.to_string())?;
                if BSTR::try_from(&value).map_err(|e| e.to_string())? == name {
                    matches.push(endpoint_id(&device).map_err(|e| e.to_string())?);
                }
            }
            // CPAL 0.16 identities use duplicate name occurrences. Refuse ambiguous
            // hardware rather than possibly changing the wrong Windows endpoint.
            if matches.len() > 1 {
                return Err("Multiple microphones share this name. Set the default in Windows Sound settings.".into());
            }
            let id = matches
                .get(occurrence)
                .ok_or("Selected microphone disconnected. Refresh devices and try again.")?
                .clone();
            let previous = ROLES.map(|role| {
                enumerator
                    .GetDefaultAudioEndpoint(eCapture, role)
                    .and_then(|device| endpoint_id(&device))
                    .ok()
            });
            (id, previous, enumerator)
        };
        let policy = policy().map_err(|e| {
            format!(
                "Windows could not change the default microphone. Use Windows Sound settings. {e}"
            )
        })?;
        for role in ROLES {
            if let Err(error) = set_role(&policy, &id, role) {
                // Restore known previous choices if only some roles were changed.
                for (previous, role) in previous.iter().zip(ROLES) {
                    if let Some(previous) = previous {
                        let _ = set_role(&policy, previous, role);
                    }
                }
                return Err(format!("Could not set every Windows microphone role; attempted to restore previous defaults. {error}"));
            }
        }
        for role in ROLES {
            let current = unsafe {
                enumerator
                    .GetDefaultAudioEndpoint(eCapture, role)
                    .and_then(|device| endpoint_id(&device))
            }
            .map_err(|e| e.to_string())?;
            if current != id {
                return Err("Windows did not keep this microphone as the default. Check Windows Sound settings.".into());
            }
        }
        Ok(format!("{name} is now the Windows default microphone for audio and voice calls. Apps with an explicit mic choice keep their own selection."))
    }
}
#[cfg(windows)]
pub use platform::{default_capture_ids, restore_capture_ids, set_default_microphone};
#[cfg(not(windows))]
pub fn set_default_microphone(_: &str) -> Result<String, String> {
    Err("This action is only available on Windows.".into())
}
