use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Profile {
    brand: String,
    matches: Vec<String>,
}
pub fn brand(name: &str) -> String {
    if is_luna_virtual_microphone(name) {
        return "Luna Mic".into();
    }
    let profiles: Vec<Profile> =
        serde_json::from_str(include_str!("../../assets/microphones/profiles.json"))
            .unwrap_or_default();
    let name = name.to_lowercase();
    profiles
        .into_iter()
        .find(|p| p.matches.iter().any(|s| name.contains(s)))
        .map(|p| p.brand)
        .unwrap_or_else(|| "Generic".into())
}
/// A source feeding its own driver input would create an audio feedback loop.
pub fn is_luna_virtual_microphone(name: &str) -> bool {
    name.to_ascii_lowercase()
        .contains("luna mic virtual microphone")
}
#[derive(Clone, Serialize, PartialEq)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub brand: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub format: String,
    pub is_default: bool,
}
#[derive(Clone, Default, Serialize)]
pub struct Devices {
    pub inputs: Vec<DeviceInfo>,
    pub outputs: Vec<DeviceInfo>,
}
/// The standard VB-CABLE pair is required; a render endpoint alone is insufficient.
pub fn has_vb_cable(devices: &Devices) -> bool {
    let matches = |name: &str, endpoint: &str| {
        let name = name.to_ascii_lowercase();
        name.contains(endpoint) && name.contains("vb-audio")
    };
    devices
        .outputs
        .iter()
        .any(|d| matches(&d.name, "cable input"))
        && devices
            .inputs
            .iter()
            .any(|d| matches(&d.name, "cable output"))
}
// CPAL 0.16 has no cross-platform endpoint ID. Name plus duplicate occurrence is
// deterministic within an enumeration, but identical devices can swap on reconnect.
pub fn enumerate() -> Result<Devices, String> {
    let host = cpal::default_host();
    let default_input = host.default_input_device().and_then(|d| d.name().ok());
    let default_output = host.default_output_device().and_then(|d| d.name().ok());
    let inputs = list(
        host.input_devices().map_err(|e| e.to_string())?,
        true,
        default_input,
    );
    let outputs = list(
        host.output_devices().map_err(|e| e.to_string())?,
        false,
        default_output,
    );
    Ok(Devices { inputs, outputs })
}
fn list(
    devices: impl Iterator<Item = cpal::Device>,
    input: bool,
    default: Option<String>,
) -> Vec<DeviceInfo> {
    let mut names = std::collections::HashMap::<String, usize>::new();
    devices
        .filter_map(|d| {
            let name = d.name().ok()?;
            let n = names.entry(name.clone()).or_default();
            let id = format!("{name}::{n}");
            *n += 1;
            let config = if input {
                d.default_input_config()
            } else {
                d.default_output_config()
            }
            .ok();
            Some(DeviceInfo {
                id,
                brand: brand(&name),
                is_default: default.as_ref() == Some(&name),
                name,
                sample_rate: config.as_ref().map(|c| c.sample_rate().0).unwrap_or(0),
                channels: config.as_ref().map(|c| c.channels()).unwrap_or(0),
                format: config
                    .map(|c| format!("{:?}", c.sample_format()))
                    .unwrap_or_else(|| "Unavailable".into()),
            })
        })
        .collect()
}
pub fn find(id: Option<&str>, input: bool) -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    let Some(id) = id else {
        return (if input {
            host.default_input_device()
        } else {
            host.default_output_device()
        })
        .ok_or_else(|| "No default audio device is available.".into());
    };
    let devices: Vec<_> = if input {
        host.input_devices().map_err(|e| e.to_string())?.collect()
    } else {
        host.output_devices().map_err(|e| e.to_string())?.collect()
    };
    let mut names = std::collections::HashMap::<String, usize>::new();
    for d in devices {
        if let Ok(name) = d.name() {
            let n = names.entry(name.clone()).or_default();
            let key = format!("{name}::{n}");
            *n += 1;
            if key == id {
                return Ok(d);
            }
        }
    }
    Err("Selected device is disconnected. Waiting for it to return.".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recognizes_brands() {
        assert_eq!(brand("Microphone (HyperX QuadCast S)"), "HyperX");
        assert_eq!(brand("BLUE YETI"), "Logitech / Blue");
        assert_eq!(brand("Mystery USB microphone"), "Generic");
    }
    #[test]
    fn cable_dependency_requires_both_vb_audio_endpoints() {
        let device = |name: &str| DeviceInfo {
            id: name.into(),
            name: name.into(),
            brand: "Generic".into(),
            sample_rate: 48000,
            channels: 2,
            format: "I16".into(),
            is_default: false,
        };
        let mut list = Devices::default();
        list.outputs
            .push(device("CABLE Input (VB-Audio Virtual Cable)"));
        assert!(!has_vb_cable(&list));
        list.inputs
            .push(device("CABLE Output (Unrelated hardware)"));
        assert!(!has_vb_cable(&list));
        list.inputs
            .push(device("CABLE Output (VB-Audio Virtual Cable)"));
        assert!(has_vb_cable(&list));
    }
    #[test]
    fn recognizes_virtual_capture_without_rejecting_physical_mics() {
        assert!(is_luna_virtual_microphone(
            "Microphone (Luna Mic Virtual Microphone)"
        ));
        assert!(!is_luna_virtual_microphone("Microphone (HyperX QuadCast)"));
        assert_eq!(brand("Luna Mic Virtual Microphone"), "Luna Mic");
    }
}
