// IPC data contracts mirror Rust settings.rs and audio/engine.rs.
// Update both sides when adding a persisted parameter or telemetry field.
export type NoiseMethod = 'rnnoise' | 'speex' | 'webrtc' | 'deepfilter';
export type NoiseMethodInfo = {
  id: NoiseMethod;
  name: string;
  available: boolean;
  description: string;
};
export type Parameters = {
  enabled: boolean;
  bypass: boolean;
  gain_on: boolean;
  gain_db: number;
  suppression_on: boolean;
  suppression_strength: number;
  gate_on: boolean;
  gate_db: number;
  gate_attack: number;
  gate_hold: number;
  gate_release: number;
  eq_on: boolean;
  eq: number[];
  compressor_on: boolean;
  compressor_db: number;
  ratio: number;
  compressor_attack: number;
  compressor_release: number;
  makeup_db: number;
  limiter_on: boolean;
  ceiling_db: number;
};
export type Preset = { name: string; parameters: Parameters };
export type Config = {
  noise_method: NoiseMethod;
  version: number;
  input: string | null;
  input_channel: number;
  monitor: boolean;
  monitor_raw: boolean;
  monitor_output: string | null;
  route_output: string | null;
  minimize_to_tray: boolean;
  start_minimized: boolean;
  start_with_windows: boolean;
  remember_device: boolean;
  remember_sound: boolean;
  preset: string;
  parameters: Parameters;
  presets: Preset[];
};
export type Device = {
  id: string;
  name: string;
  brand: string;
  sample_rate: number;
  channels: number;
  format: string;
  is_default: boolean;
};
export type Levels = {
  suppression_problems: number;
  raw_db: number;
  gate_input_db: number;
  gate_detector_db: number;
  output_db: number;
  peak_db: number;
  clipped: boolean;
  gate_open: boolean;
  reduction_db: number;
  limiter_active: boolean;
  frames: number;
};
export type Diagnostics = {
  noise_method: NoiseMethod;
  noise_methods: NoiseMethodInfo[];
  suppression_available: boolean;
  suppression_delay_ms: number;
  running: boolean;
  message: string;
  warning: string;
  input_name: string;
  sample_rate: number;
  channels: number;
  buffer_frames: number;
  callback_us: number;
  queue_ms: number;
  underruns: number;
  overruns: number;
  monitor_rate: number;
  route_rate: number;
};
export type Snapshot = {
  devices: { inputs: Device[]; outputs: Device[] };
  levels: Levels;
  diagnostics: Diagnostics;
};
export type Initial = {
  config: Config;
  builtins: Preset[];
  warning: string | null;
  settings_path: string;
};
