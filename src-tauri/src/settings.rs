use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoiseMethod {
    #[default]
    Rnnoise,
    Speex,
    Webrtc,
    Deepfilter,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Parameters {
    // `enabled` controls output mute, not stream lifetime. Metering continues.
    pub enabled: bool,
    // `bypass` crossfades processed destinations to the untouched input sample.
    pub bypass: bool,
    pub gain_on: bool,
    pub gain_db: f32,
    pub suppression_on: bool,
    pub suppression_strength: f32,
    pub gate_on: bool,
    pub gate_db: f32,
    pub gate_attack: f32,
    pub gate_hold: f32,
    pub gate_release: f32,
    pub eq_on: bool,
    pub eq: [f32; 8],
    pub compressor_on: bool,
    pub compressor_db: f32,
    pub ratio: f32,
    pub compressor_attack: f32,
    pub compressor_release: f32,
    pub makeup_db: f32,
    pub limiter_on: bool,
    pub ceiling_db: f32,
}
impl Default for Parameters {
    fn default() -> Self {
        Self {
            enabled: true,
            bypass: false,
            gain_on: true,
            gain_db: 0.,
            suppression_on: false,
            suppression_strength: 1.,
            gate_on: true,
            gate_db: -48.,
            gate_attack: 5.,
            gate_hold: 120.,
            gate_release: 180.,
            eq_on: true,
            eq: [0.; 8],
            compressor_on: false,
            compressor_db: -18.,
            ratio: 3.,
            compressor_attack: 10.,
            compressor_release: 120.,
            makeup_db: 0.,
            limiter_on: true,
            ceiling_db: -1.,
        }
    }
}
impl Parameters {
    pub fn validate(&self) -> Result<(), String> {
        let fields = [
            (self.suppression_strength, 0., 1.),
            (self.gain_db, -24., 18.),
            (self.gate_db, -80., -5.),
            (self.gate_attack, 1., 100.),
            (self.gate_hold, 0., 1000.),
            (self.gate_release, 10., 2000.),
            (self.compressor_db, -60., 0.),
            (self.ratio, 1., 12.),
            (self.compressor_attack, 1., 100.),
            (self.compressor_release, 10., 2000.),
            (self.makeup_db, 0., 12.),
            (self.ceiling_db, -12., -0.1),
        ];
        if fields
            .iter()
            .any(|(v, lo, hi)| !v.is_finite() || v < lo || v > hi)
            || self
                .eq
                .iter()
                .any(|v| !v.is_finite() || !(-12. ..=12.).contains(v))
        {
            return Err("Audio setting is outside its safe range.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    // Engine choice is an app preference: sound presets do not silently change it.
    pub noise_method: NoiseMethod,
    // None follows Windows' default endpoint; Some remembers an explicit device.
    pub input: Option<String>,
    pub input_channel: usize,
    pub monitor: bool,
    pub monitor_raw: bool,
    pub monitor_output: Option<String>,
    pub route_output: Option<String>,
    pub minimize_to_tray: bool,
    pub start_minimized: bool,
    pub start_with_windows: bool,
    pub remember_device: bool,
    pub remember_sound: bool,
    pub preset: String,
    pub parameters: Parameters,
    pub presets: Vec<Preset>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            noise_method: NoiseMethod::default(),
            input: None,
            input_channel: 0,
            monitor: false,
            monitor_raw: false,
            monitor_output: None,
            route_output: None,
            minimize_to_tray: true,
            start_minimized: false,
            start_with_windows: false,
            remember_device: true,
            remember_sound: true,
            preset: "Natural".into(),
            parameters: Parameters::default(),
            presets: vec![],
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub parameters: Parameters,
}

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("This settings file uses an unsupported version.".into());
        }
        self.parameters.validate()?;
        if self.input_channel > 63 || self.presets.len() > 64 || self.preset.len() > 80 {
            return Err("Settings exceed supported limits.".into());
        }
        for p in &self.presets {
            if p.name.trim().is_empty() || p.name.len() > 80 {
                return Err("Preset names must be 1–80 bytes.".into());
            }
            p.parameters.validate()?;
        }
        Ok(())
    }
}

pub fn load(path: &Path) -> (Config, Option<String>) {
    if !path.exists() {
        return (Config::default(), None);
    }
    if fs::metadata(path)
        .map(|m| m.len() > 1_048_576)
        .unwrap_or(false)
    {
        return (Config::default(),Some("Settings file is too large; using defaults. Original file is preserved until you save.".into()));
    }
    let parsed = fs::read(path)
        .map_err(|e| e.to_string())
        .and_then(|data| serde_json::from_slice::<Config>(&data).map_err(|e| e.to_string()))
        .and_then(|c| {
            c.validate()?;
            Ok(c)
        });
    match parsed {
        Ok(mut c) => {
            // Monitoring never starts unexpectedly; the user must enable it each session.
            c.monitor=false;
            if !c.remember_device { c.input=None; }
            if !c.remember_sound { c.parameters=Parameters::default(); c.preset="Natural".into(); }
            (c,None)
        },
        Err(e) => (Config::default(),Some(format!("Could not load settings; using defaults. Original file is preserved until you save. {e}")))
    }
}
/// Read settings from the previous name only when Luna Mic has no settings yet.
/// The legacy file is left intact; subsequent saves use the new destination.
pub fn load_with_legacy(path: &Path, legacy: &Path) -> (Config, Option<String>) {
    if path.exists() || !legacy.exists() {
        return load(path);
    }
    load(legacy)
}
pub fn save(path: &Path, config: &Config) -> Result<(), String> {
    config.validate()?;
    let parent = path.parent().ok_or("Invalid settings path")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(config).map_err(|e| e.to_string())?;
    use std::io::Write;
    let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
    f.write_all(&bytes)
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())?;
    drop(f);
    // Same-directory rename replaces atomically. Keep the previous file as backup.
    let backup = path.with_extension("json.bak");
    if path.exists() {
        fs::copy(path, &backup).map_err(|e| e.to_string())?;
    }
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

pub fn builtins() -> Vec<Preset> {
    ["Raw", "Natural", "Clean", "Broadcast", "Deep"]
        .into_iter()
        .map(|name| {
            let mut p = Parameters::default();
            match name {
                "Raw" => p.bypass = true,
                "Clean" => {
                    p.gate_db = -42.;
                    p.compressor_on = true;
                    p.ratio = 2.;
                }
                "Broadcast" => {
                    p.eq = [-2., 2., 1., -1., 0., 2., 2., 0.];
                    p.compressor_on = true;
                    p.compressor_db = -22.;
                    p.ratio = 4.;
                    p.makeup_db = 3.;
                }
                "Deep" => {
                    p.eq = [2., 3., 2., 0., 0., 0., -1., -1.];
                }
                _ => {}
            }
            Preset {
                name: name.into(),
                parameters: p,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounds_and_nan() {
        let mut p = Parameters {
            ratio: f32::NAN,
            ..Parameters::default()
        };
        assert!(p.validate().is_err());
        p.ratio = 13.;
        assert!(p.validate().is_err());
    }
    #[test]
    fn preset_roundtrip() {
        let list = builtins();
        let data = serde_json::to_string(&list).unwrap();
        let decoded: Vec<Preset> = serde_json::from_str(&data).unwrap();
        assert_eq!(decoded[3].parameters, list[3].parameters);
    }
    #[test]
    fn disk_and_corruption() {
        let dir = std::env::temp_dir().join(format!("miclab-test-{}", std::process::id()));
        let path = dir.join("settings.json");
        let c = Config::default();
        save(&path, &c).unwrap();
        save(&path, &c).unwrap();
        assert!(path.with_extension("json.bak").exists());
        assert_eq!(load(&path).0.parameters, c.parameters);
        fs::write(&path, "broken").unwrap();
        assert!(load(&path).1.is_some());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn rename_preserves_legacy_settings_but_prefers_new_file() {
        let dir = std::env::temp_dir().join(format!("luna-rename-test-{}", std::process::id()));
        let legacy = dir.join("old/settings.json");
        let current = dir.join("new/settings.json");
        let old = Config {
            input: Some("Saved microphone::0".into()),
            ..Config::default()
        };
        save(&legacy, &old).unwrap();
        assert_eq!(load_with_legacy(&current, &legacy).0.input, old.input);
        let new = Config {
            preset: "Clean".into(),
            ..Config::default()
        };
        save(&current, &new).unwrap();
        assert_eq!(load_with_legacy(&current, &legacy).0.preset, "Clean");
        assert!(legacy.exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
