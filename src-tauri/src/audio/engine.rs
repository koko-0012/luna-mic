use super::{
    device_events::DeviceEvents,
    dsp::{Chain, Levels, Meter},
    output::{AudioSink, OutputCounters, RenderOutput},
};
use crate::{
    devices::{self, Devices},
    settings::{Config, Parameters},
};
use cpal::{
    traits::{DeviceTrait, StreamTrait},
    FromSample, Sample, SizedSample,
};
use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use triple_buffer::{triple_buffer, Input, Output};

#[derive(Clone, Copy)]
struct Controls {
    parameters: Parameters,
    monitor_raw: bool,
}
#[derive(Clone, Default, Serialize)]
pub struct Diagnostics {
    pub noise_method: crate::settings::NoiseMethod,
    pub noise_methods: Vec<super::suppression::MethodInfo>,
    pub suppression_available: bool,
    pub suppression_delay_ms: u32,
    pub running: bool,
    pub message: String,
    pub warning: String,
    pub input_name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_frames: u32,
    pub callback_us: u64,
    pub queue_ms: f32,
    pub underruns: u64,
    pub overruns: u64,
    pub monitor_rate: u32,
    pub route_rate: u32,
}
#[derive(Clone, Default, Serialize)]
pub struct Snapshot {
    pub devices: Devices,
    pub levels: Levels,
    pub diagnostics: Diagnostics,
}
struct Live {
    // Streams must outlive the callback's parameter and telemetry buffers.
    _capture: cpal::Stream,
    _outputs: Vec<cpal::Stream>,
    controls: Input<Controls>,
    meter: Output<Levels>,
    failed: Arc<AtomicBool>,
    frames: Arc<AtomicU32>,
    callback_us: Arc<AtomicU64>,
    outputs: Vec<Arc<OutputCounters>>,
    info: Diagnostics,
}
enum Command {
    Update(Box<Config>, mpsc::Sender<Result<(), String>>),
    Refresh,
    Shutdown,
}
pub struct Engine {
    tx: mpsc::SyncSender<Command>,
    pub snapshot: Arc<Mutex<Snapshot>>,
}
impl Engine {
    pub fn new(config: Config) -> Self {
        let (tx, rx) = mpsc::sync_channel(16);
        let snapshot = Arc::new(Mutex::new(Snapshot::default()));
        let shared = snapshot.clone();
        thread::Builder::new()
            .name("luna-mic-audio-control".into())
            .spawn(move || worker(rx, shared, config))
            .expect("Could not spawn audio worker");
        Self { tx, snapshot }
    }
    pub fn update(&self, config: Config) -> Result<(), String> {
        config.validate()?;
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(Command::Update(Box::new(config), tx))
            .map_err(|_| "Audio worker stopped")?;
        rx.recv_timeout(Duration::from_secs(10))
            .map_err(|_| "Audio worker did not respond".to_string())?
    }
    pub fn refresh(&self) {
        let _ = self.tx.try_send(Command::Refresh);
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.tx.try_send(Command::Shutdown);
    }
}

fn needs_restart(a: &Config, b: &Config) -> bool {
    a.noise_method != b.noise_method
        || a.input != b.input
        || a.input_channel != b.input_channel
        || a.monitor != b.monitor
        || a.monitor_output != b.monitor_output
        || a.route_output != b.route_output
}
fn monitor_shares_route(config: &Config, default_output: Option<&str>) -> bool {
    config.monitor
        && config
            .route_output
            .as_deref()
            .is_some_and(|route| config.monitor_output.as_deref().or(default_output) == Some(route))
}
fn device_changes_require_restart(old: &Devices, new: &Devices, config: &Config) -> bool {
    fn selected(
        list: &[crate::devices::DeviceInfo],
        id: Option<&str>,
    ) -> Option<(String, u32, u16, String)> {
        list.iter()
            .find(|device| id.map(|id| device.id == id).unwrap_or(device.is_default))
            .map(|device| {
                (
                    device.id.clone(),
                    device.sample_rate,
                    device.channels,
                    device.format.clone(),
                )
            })
    }
    // Default badges and unrelated device changes do not affect a specifically
    // selected stream. Reopen only if one of its endpoint identities/formats changes.
    selected(&old.inputs, config.input.as_deref()) != selected(&new.inputs, config.input.as_deref())
        || (config.monitor
            && selected(&old.outputs, config.monitor_output.as_deref())
                != selected(&new.outputs, config.monitor_output.as_deref()))
        || (config.route_output.is_some()
            && selected(&old.outputs, config.route_output.as_deref())
                != selected(&new.outputs, config.route_output.as_deref()))
}
fn worker(rx: mpsc::Receiver<Command>, snapshot: Arc<Mutex<Snapshot>>, mut config: Config) {
    let events = DeviceEvents::new();
    let mut scan_requested = true;
    let mut live: Option<Live> = None;
    let mut last_scan = Instant::now() - Duration::from_secs(10);
    let mut retry = Instant::now();
    let mut device_list = Devices::default();
    let mut error = String::new();
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Command::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(Command::Refresh) => scan_requested = true,
            Ok(Command::Update(new, reply)) => {
                let restart = needs_restart(&config, &new);
                config = *new;
                if restart {
                    live = None;
                    retry = Instant::now();
                } else if let Some(l) = &mut live {
                    l.controls.write(Controls {
                        parameters: config.parameters,
                        monitor_raw: config.monitor_raw,
                    });
                }
                let _ = reply.send(Ok(()));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        let notified = events
            .as_ref()
            .is_some_and(|e| e.dirty.swap(false, Ordering::AcqRel));
        // Windows uses endpoint events. Only a failed notification registration
        // (or another platform) falls back to a low-frequency device scan.
        if scan_requested
            || notified
            || (events.is_none() && last_scan.elapsed() >= Duration::from_secs(3))
        {
            scan_requested = false;
            last_scan = Instant::now();
            match devices::enumerate() {
                Ok(d) => {
                    let removed = config
                        .input
                        .as_ref()
                        .is_some_and(|id| !d.inputs.iter().any(|i| &i.id == id));
                    let changed = d.inputs.iter().map(|i| &i.id).collect::<Vec<_>>()
                        != device_list.inputs.iter().map(|i| &i.id).collect::<Vec<_>>()
                        || d.outputs.iter().map(|i| &i.id).collect::<Vec<_>>()
                            != device_list
                                .outputs
                                .iter()
                                .map(|i| &i.id)
                                .collect::<Vec<_>>();
                    if removed {
                        live = None;
                        error = "Microphone disconnected. Waiting for it to return.".into();
                    }
                    if changed || notified {
                        retry = Instant::now();
                        // Reopen a missing optional sink when its device returns,
                        // and release stale default endpoints after a device change.
                        if !removed && device_changes_require_restart(&device_list, &d, &config) {
                            live = None;
                        }
                    }
                    device_list = d;
                }
                Err(e) => error = format!("Could not list audio devices: {e}"),
            }
        }
        if live.as_ref().is_some_and(|l| {
            l.failed.load(Ordering::Relaxed)
                || l.outputs.iter().any(|o| o.failed.load(Ordering::Relaxed))
        }) {
            live = None;
            error="An audio stream stopped. Retrying in a few seconds; check Windows microphone privacy and device connections.".into();
            retry = Instant::now() + Duration::from_secs(3);
        }
        if live.is_none() && Instant::now() >= retry {
            retry = Instant::now() + Duration::from_secs(5);
            if config.input.is_none() && device_list.inputs.len() > 1 {
                error = "Choose your microphone below to begin.".into();
            } else {
                match open(&config) {
                    Ok(l) => {
                        error.clear();
                        live = Some(l);
                    }
                    Err(e) => error = e,
                }
            }
        }
        let mut state = Snapshot {
            devices: device_list.clone(),
            levels: Levels {
                raw_db: -120.,
                gate_input_db: -120.,
                gate_detector_db: -120.,
                output_db: -120.,
                peak_db: -120.,
                ..Levels::default()
            },
            diagnostics: Diagnostics {
                message: error.clone(),
                ..Diagnostics::default()
            },
        };
        if let Some(l) = &mut live {
            state.levels = *l.meter.read();
            state.diagnostics = l.info.clone();
            state.diagnostics.running = true;
            state.diagnostics.suppression_delay_ms = if state.diagnostics.suppression_available
                && config.parameters.suppression_on
                && !config.parameters.bypass
            {
                l.info.suppression_delay_ms
            } else {
                0
            };
            state.diagnostics.buffer_frames = l.frames.load(Ordering::Relaxed);
            state.diagnostics.callback_us = l.callback_us.load(Ordering::Relaxed);
            for output in &l.outputs {
                state.diagnostics.underruns += output.underruns.load(Ordering::Relaxed);
                state.diagnostics.overruns += output.overruns.load(Ordering::Relaxed);
                state.diagnostics.queue_ms = state.diagnostics.queue_ms.max(
                    output.queued.load(Ordering::Relaxed) as f32
                        / state.diagnostics.sample_rate as f32
                        * 1000.,
                );
            }
        }
        if let Ok(mut out) = snapshot.lock() {
            *out = state;
        }
    }
}
fn open(config: &Config) -> Result<Live, String> {
    if !devices::has_vb_cable(&devices::enumerate()?) {
        return Err("Install VB-CABLE and restart Windows before using Luna Mic.".into());
    }
    let device = devices::find(config.input.as_deref(), true)?;
    let input_name = device.name().unwrap_or_default();
    let input_lower = input_name.to_ascii_lowercase();
    if devices::is_luna_virtual_microphone(&input_name)
        || (input_lower.contains("cable output") && input_lower.contains("vb-audio"))
    {
        return Err("Select your physical microphone as Luna Mic's input. Luna Mic Virtual Microphone is the processed output for Discord / OBS.".into());
    }
    let supported = device
        .default_input_config()
        .map_err(|e| format!("Cannot use microphone format: {e}"))?;
    let rate = supported.sample_rate().0;
    let channels = supported.channels();
    if !(8000..=192000).contains(&rate) || channels == 0 || channels > 64 {
        return Err(
            "Unsupported microphone format. Set 44.1 or 48 kHz in Windows Sound settings.".into(),
        );
    }
    if config.input_channel >= channels as usize {
        return Err(format!("Choose an input channel between 1 and {channels}."));
    }
    let (controls, read_controls) = triple_buffer(&Controls {
        parameters: config.parameters,
        monitor_raw: config.monitor_raw,
    });
    let (write_meter, meter) = triple_buffer(&Levels::default());
    let failed = Arc::new(AtomicBool::new(false));
    let frames = Arc::new(AtomicU32::new(0));
    let callback_us = Arc::new(AtomicU64::new(0));
    let mut streams = vec![];
    let mut output_counters = vec![];
    let mut monitor = None;
    let mut route = None;
    let mut info = Diagnostics {
        noise_method: config.noise_method,
        noise_methods: super::suppression::methods(rate),
        input_name: device.name().unwrap_or_else(|_| "Microphone".into()),
        sample_rate: rate,
        channels,
        message: "Capturing microphone".into(),
        ..Diagnostics::default()
    };
    let chain = Chain::new_with_method(rate as f32, config.parameters, config.noise_method);
    info.suppression_available = chain.suppression_supported();
    info.suppression_delay_ms = chain.suppression_delay();
    if let Some(error) = chain.suppression_error() {
        info.warning = format!("Noise engine unavailable: {error}");
    }
    if config.monitor {
        match RenderOutput::open(config.monitor_output.as_deref(), rate) {
            Ok(o) => {
                info.monitor_rate = o.rate;
                output_counters.push(o.sink.counters.clone());
                streams.push(o.stream);
                monitor = Some(o.sink);
            }
            Err(e) => info.warning = format!("Monitoring unavailable: {e}"),
        }
    }
    let default_output =
        if config.monitor && config.monitor_output.is_none() && config.route_output.is_some() {
            devices::enumerate()
                .ok()
                .and_then(|list| list.outputs.into_iter().find(|device| device.is_default))
                .map(|device| device.id)
        } else {
            None
        };
    if monitor.is_some() && monitor_shares_route(config, default_output.as_deref()) {
        // Two WASAPI streams to one endpoint would mix dry/processed audio or
        // double its level. Monitoring wins so Original/With effects remains valid.
        info.warning.push_str("Monitoring and routing share the same output. Only the monitor is played here to avoid doubled/mixed audio. Choose another output for routing.");
    } else if let Some(id) = &config.route_output {
        match RenderOutput::open(Some(id), rate) {
            Ok(o) => {
                info.route_rate = o.rate;
                output_counters.push(o.sink.counters.clone());
                streams.push(o.stream);
                route = Some(o.sink);
            }
            Err(e) => info
                .warning
                .push_str(&format!(" Output routing unavailable: {e}")),
        }
    }
    let state = Capture {
        controls: read_controls,
        meter: write_meter,
        chain,
        accumulator: Meter::new(rate),
        monitor,
        route,
        channels: channels as usize,
        channel: config.input_channel,
        frames: frames.clone(),
        callback_us: callback_us.clone(),
    };
    let stream_config: cpal::StreamConfig = supported.clone().into();
    let capture = match supported.sample_format() {
        cpal::SampleFormat::F32 => build::<f32>(&device, &stream_config, state, failed.clone()),
        cpal::SampleFormat::I16 => build::<i16>(&device, &stream_config, state, failed.clone()),
        cpal::SampleFormat::U16 => build::<u16>(&device, &stream_config, state, failed.clone()),
        f => {
            return Err(format!(
            "Unsupported capture format {f:?}. Use float32 or 16-bit in Windows Sound settings."
        ))
        }
    }?;
    capture.play().map_err(|e| {
        format!("Microphone could not start. Check Windows microphone privacy permissions. {e}")
    })?;
    Ok(Live {
        _capture: capture,
        _outputs: streams,
        controls,
        meter,
        failed,
        frames,
        callback_us,
        outputs: output_counters,
        info,
    })
}
struct Capture {
    controls: Output<Controls>,
    meter: Input<Levels>,
    chain: Chain,
    accumulator: Meter,
    monitor: Option<AudioSink>,
    route: Option<AudioSink>,
    channels: usize,
    channel: usize,
    frames: Arc<AtomicU32>,
    callback_us: Arc<AtomicU64>,
}
impl Capture {
    fn process<T: Sample>(&mut self, data: &[T])
    where
        f32: FromSample<T>,
    {
        let start = Instant::now();
        let controls = *self.controls.read();
        self.chain.configure(controls.parameters);
        self.frames
            .store((data.len() / self.channels) as u32, Ordering::Relaxed);
        for frame in data.chunks_exact(self.channels) {
            let raw = f32::from_sample(frame[self.channel]);
            let raw = if raw.is_finite() { raw } else { 0. };
            let processed = self.chain.process(raw);
            if let Some(monitor) = &mut self.monitor {
                monitor.push(if !controls.parameters.enabled {
                    0.
                } else if controls.monitor_raw {
                    raw
                } else {
                    processed
                });
            }
            if let Some(route) = &mut self.route {
                route.push(processed);
            }
            if let Some(level) = self.accumulator.push(raw, processed, &self.chain) {
                self.meter.write(level);
            }
        }
        self.callback_us
            .store(start.elapsed().as_micros() as u64, Ordering::Relaxed);
    }
}
fn build<T: SizedSample>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut state: Capture,
    failed: Arc<AtomicBool>,
) -> Result<cpal::Stream, String>
where
    f32: FromSample<T>,
{
    device.build_input_stream(config,move|data:&[T],_|state.process(data),move|_|{failed.store(true,Ordering::Relaxed);},None).map_err(|e|format!("Cannot open microphone. Check Windows privacy permissions and other apps using exclusive mode. {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_shared_destination_gets_only_the_monitor_feed() {
        let mut config = Config {
            monitor: true,
            route_output: Some("headphones".into()),
            ..Config::default()
        };
        assert!(monitor_shares_route(&config, Some("headphones")));
        assert!(!monitor_shares_route(&config, Some("cable")));
        config.monitor_output = Some("headphones".into());
        assert!(monitor_shares_route(&config, Some("cable")));
        config.monitor = false;
        assert!(!monitor_shares_route(&config, Some("headphones")));
    }
    #[test]
    fn parameters_do_not_restart_stream() {
        let a = Config::default();
        let mut b = a.clone();
        b.parameters.gate_db = -30.;
        assert!(!needs_restart(&a, &b));
        b.monitor = true;
        assert!(needs_restart(&a, &b));
        b.monitor = false;
        b.noise_method = crate::settings::NoiseMethod::Speex;
        assert!(needs_restart(&a, &b));
    }
    fn device(id: &str, default: bool) -> crate::devices::DeviceInfo {
        crate::devices::DeviceInfo {
            id: id.into(),
            name: id.into(),
            brand: "Generic".into(),
            sample_rate: 48000,
            channels: 1,
            format: "F32".into(),
            is_default: default,
        }
    }
    #[test]
    fn changing_default_badges_keeps_an_explicit_mic_running() {
        let old = Devices {
            inputs: vec![device("mic-a", true), device("mic-b", false)],
            outputs: vec![],
        };
        let new = Devices {
            inputs: vec![device("mic-a", false), device("mic-b", true)],
            outputs: vec![],
        };
        let selected = Config {
            input: Some("mic-a".into()),
            ..Config::default()
        };
        assert!(!device_changes_require_restart(&old, &new, &selected));
        assert!(device_changes_require_restart(
            &old,
            &new,
            &Config::default()
        ));
    }
    #[test]
    fn irrelevant_outputs_do_not_restart_capture_but_selected_removal_does() {
        let old = Devices {
            inputs: vec![device("mic", true)],
            outputs: vec![],
        };
        let new = Devices {
            inputs: old.inputs.clone(),
            outputs: vec![device("headphones", true)],
        };
        let selected = Config {
            input: Some("mic".into()),
            ..Config::default()
        };
        assert!(!device_changes_require_restart(&old, &new, &selected));
        assert!(device_changes_require_restart(
            &old,
            &Devices::default(),
            &selected
        ));
    }
}
