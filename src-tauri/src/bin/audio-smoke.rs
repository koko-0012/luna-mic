//! Manual hardware smoke test. Captures in memory only; never records to disk.
//! Monitoring is opt-in and requires a named headphones endpoint.
use luna_mic_core::{
    audio::engine::Engine,
    devices,
    settings::{Config, Parameters},
};
use std::{thread, time::Duration};
fn main() -> Result<(), String> {
    let devices = devices::enumerate()?;
    println!(
        "{}",
        serde_json::to_string_pretty(&devices).map_err(|e| e.to_string())?
    );
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        println!("Pass --capture [input ID] to capture for 3 seconds. Optional: --monitor <headphones ID>. No file is recorded.");
        return Ok(());
    }
    if args[0] != "--capture" {
        return Err("Expected --capture".into());
    }
    let input = args
        .get(1)
        .filter(|s| !s.starts_with("--"))
        .cloned()
        .or_else(|| {
            devices
                .inputs
                .iter()
                .find(|d| d.is_default)
                .map(|d| d.id.clone())
        });
    let monitor = args
        .iter()
        .position(|a| a == "--monitor")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let mut config = Config {
        input,
        monitor: monitor.is_some(),
        route_output: if args.iter().any(|arg| arg == "--same-output") {
            monitor.clone()
        } else {
            None
        },
        monitor_output: monitor,
        ..Config::default()
    };
    if let Some(index) = args.iter().position(|arg| arg == "--method") {
        let method = args
            .get(index + 1)
            .ok_or("Missing method: rnnoise / speex / webrtc / deepfilter")?;
        config.noise_method = serde_json::from_value(serde_json::Value::String(method.clone()))
            .map_err(|error| error.to_string())?;
    }
    config.parameters.suppression_on = args.iter().any(|arg| arg == "--noise-removal");
    let noise_requested = config.parameters.suppression_on;
    let engine = Engine::new(config.clone());
    thread::sleep(Duration::from_secs(1));
    let initial = engine
        .snapshot
        .lock()
        .map_err(|_| "Status mutex unavailable")?
        .clone();
    if !initial.diagnostics.running {
        return Err(initial.diagnostics.message);
    }
    if noise_requested && !initial.diagnostics.suppression_available {
        return Err("Noise removal unavailable; use a 48 kHz input and enabled feature.".into());
    }
    if config.monitor && initial.diagnostics.monitor_rate == 0 {
        return Err(initial.diagnostics.warning);
    }
    println!(
        "Capture: {}",
        serde_json::to_string(&initial.diagnostics).unwrap()
    );
    thread::sleep(Duration::from_secs(1));
    let second = engine
        .snapshot
        .lock()
        .map_err(|_| "Status mutex unavailable")?
        .clone();
    if second.levels.frames <= initial.levels.frames {
        return Err("Capture frame counter stalled".into());
    }
    config.parameters = Parameters {
        bypass: true,
        ..config.parameters
    };
    engine.update(config.clone())?;
    thread::sleep(Duration::from_secs(1));
    let bypass = engine
        .snapshot
        .lock()
        .map_err(|_| "Status mutex unavailable")?
        .clone();
    println!("Bypass: {}", serde_json::to_string(&bypass.levels).unwrap());
    if !bypass.diagnostics.running || bypass.levels.frames <= second.levels.frames {
        return Err("Parameter update interrupted capture".into());
    }
    if bypass.diagnostics.monitor_rate > 0 {
        println!(
            "Monitor diagnostics: {}",
            serde_json::to_string(&bypass.diagnostics).unwrap()
        );
    }
    config.input = Some("__missing_smoke_test_device__".into());
    engine.update(config.clone())?;
    thread::sleep(Duration::from_millis(400));
    let absent = engine
        .snapshot
        .lock()
        .map_err(|_| "Status mutex unavailable")?
        .clone();
    if absent.diagnostics.running {
        return Err("Missing input should stop capture".into());
    }
    println!("Missing device handled: {}", absent.diagnostics.message);
    config.input = initial
        .devices
        .inputs
        .iter()
        .find(|d| d.is_default)
        .map(|d| d.id.clone());
    engine.update(config)?;
    thread::sleep(Duration::from_secs(1));
    let restored = engine
        .snapshot
        .lock()
        .map_err(|_| "Status mutex unavailable")?
        .clone();
    if !restored.diagnostics.running {
        return Err("Capture did not restore".into());
    }
    println!("PASS: capture frames advance, live bypass works, missing input is handled, capture restores.");
    Ok(())
}
