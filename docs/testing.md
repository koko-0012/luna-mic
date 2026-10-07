# Validation

## Automated

Run `npm run build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`,
and `cargo fmt --check` using `--manifest-path src-tauri/Cargo.toml`, or run `npm run check`.

Tests cover preset serialization, settings validation and corrupt files, brand
matching, gate open/hold/close behavior, compressor reduction, EQ unity when off,
limiter output bounds under high boosts, finite silent/invalid sample processing,
chain bypass/mute, output underrun/resampling, and parameter changes that do not
require a stream rebuild. An instrumented allocator test processes 96,000
generated samples, live EQ changes, triple-buffer updates and ring-buffer routing
with zero heap allocations on the processing thread.

`audio-smoke` enumerates real devices and, only with `--capture`, checks advancing
capture frame counters, a live bypass update, missing-device handling and recovery.
It never writes microphone audio to disk. Optional `--monitor` requires an
explicit endpoint ID; use headphones. This is a smoke test, not an acoustic
quality assessment or substitute for physical USB disconnect testing.

## Manual checks before distributing

- Start with one mic, multiple mics, no mic, inaccessible mic and changed format.
- Unplug/replug the selected physical microphone; ensure readable errors and
  automatic recovery, including a device name that changes after reconnect.
- Disconnect monitoring headphones or a cable, then restore them; capture should
  remain recoverable. Test default playback device changes and different rates.
- Speak softly/loudly across the gate threshold; check hold/release and clipping.
  Compare Raw / Processed on headphones. Try ±12 dB EQ, compressor makeup and
  limiter off/on. Avoid high monitoring volume.
- Calibrate with real quiet/speech phases, weak speech, constant noise, gain
  changes and disconnected devices. Ensure manual threshold editing still works.
- Save a preset, restart, import/export it, and test malformed/out-of-range JSON.
- Close to tray, reopen, use enable and bypass from tray, then Exit. Start minimized.
  Test Windows login startup from a fixed release path, and disable it afterward.
- Run for hours under CPU/GPU gaming load. Inspect underrun/overflow counters,
  microphone-to-headphones latency and all related WebView2 process memory.
  Callback time alone is not end-to-end latency.
- Test signed cable routing into a separate application's recording endpoint.
  Do not install or test unsigned drivers as part of these checks.

Windows 10 compatibility, login startup after a real reboot, physical unplug/replug,
actual speech calibration, cable delivery into Discord/OBS, and extended gaming-load
performance are not established by unit tests and must be reported separately.
