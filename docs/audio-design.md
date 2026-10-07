# Audio design and future optional modules

The Rust control worker owns CPAL streams, handles validated configurations,
enumerates endpoints and retries errors. It receives UI commands through a bounded
channel. Stream and file operations never occur in the capture callback.

Audio callbacks communicate using fixed-size triple buffers and preallocated
ring buffers. Their closures own DSP state. Rust mutexes protect only settings
and snapshots read/written outside callbacks. Error callbacks set atomic flags;
the control worker reports a readable error and retries. There is no continuous
log file. Endpoint notifications only set a dirty flag; enumeration runs later
on the control worker. Optional output failures leave capture available and
surface a warning. A disappearing active sink triggers stream reconstruction.

## Current DSP behavior

Gain changes smooth over roughly 5 ms. The gate uses a decaying peak detector,
3 dB closing hysteresis, hold time, and exponential attack/release gain smoothing.
Metering shows post-gain/pre-gate RMS, so calibration and the threshold marker
remain useful when the gate is closed. The meter does not use the exact peak
detector envelope; calibration suggests a starting value and asks users to refine
it. It samples only advancing audio frame counters, requires enough samples,
and refuses calibration when voice and noise are too similar.

EQ uses eight RBJ peaking biquads at 60/120/250/500/1000/2000/4000/8000 Hz, Q=1,
with gains bounded to ±12 dB and frequencies clamped below Nyquist. Coefficients
smooth toward their targets; transient filter state is not discarded when a
slider moves. Basic Bass/Warmth/Treble/Clarity directly edit 120/250/8000/4000 Hz,
respectively. The remaining bands stay available in Advanced.

Compression uses a 6 dB soft knee and a smoothed gain-reduction envelope. Ratio,
threshold, attack/release and makeup are bounded and can be edited directly.
The basic amount maps into ratio/threshold/makeup, leaving Advanced controls
available. This is a simple peak-oriented voice compressor, not an emulation
of a commercial processor.

The limiter applies immediate peak gain reduction with an 80 ms release and
an explicit final ceiling. It is not a lookahead limiter, true-peak limiter, or
automatic loudness normalizer. At extreme boosts it can reshape transients.
Bypass crossfades over roughly 5 ms toward raw input. Muting output is immediate.
Individual processors retain state while off and settle toward unity; disabling
a module does not destroy or allocate its state. Input metering remains active
when processed output is muted.

Current routing duplicates mono to every channel of a render endpoint. Linear
interpolation supports different endpoint rates; bounded ratio correction
handles modest clock drift. Initial queue prefill is approximately 20 ms of
source audio; queue capacity is 200 ms. Capacity is a bound, not intended latency.
Underruns rebuffer and produce silence. Overflow drops newest samples and is
counted. For best fidelity use matching 48 kHz devices. A future polyphase
resampler should improve anti-alias filtering during downsampling.

## Dependency investigation

[CPAL](https://github.com/RustAudio/cpal) is maintained, Apache-2.0, and supports
Windows WASAPI shared-mode streams. Version 0.16.0 is pinned to the inspected,
tested API; newer versions exist. Its official
[feedback example](https://github.com/RustAudio/cpal/blob/v0.16.0/examples/feedback.rs)
demonstrates independently clocked streams and ring buffers. Luna Mic replaces
example callback logging with atomic counters and adds sample-rate conversion.
There is no ASIO dependency.

[RNNoise](https://github.com/xiph/rnnoise) was the preferred next suppression
candidate. Upstream code is BSD-3-Clause; preserve its notices if integrating.
It is designed for real-time speech suppression, but its normal frame contract
is 480 samples at 48 kHz and a float amplitude convention that must be explicitly
adapted. Upstream's build path uses native C/autotools, so a reproducible Windows
MSVC integration or a thoroughly tested Rust port must be selected first.
Luna Mic 0.3 now implements a verified pure-Rust port at 48 kHz, via nnnoiseless
0.5.1. See [noise-removal.md](noise-removal.md) for the current implementation,
latency, feature flags and tests. A future C integration would need model
state created before capture, fixed buffers, and tests for scale/latency/alignment.
Do not mark RNNoise available just because a DLL was found.

Add frame buffering before the gate; align dry and denoised taps before
implementing suppression-strength blending or removed-audio monitoring. The
removed signal must be dry minus **time-aligned** denoised audio. Otherwise users
would hear delay artifacts rather than removed noise. Missing optional libraries
must yield a visibly unavailable module and leave the baseline pipeline usable.

DeepFilterNet and pitch/formant processing are deferred. Select and verify their
exact versions, licenses, Windows builds, allocation behavior and latency before
adding dependencies. Pitch shifting must keep stream duration/sample rate and
must not slow playback. Formant control needs a separate validated algorithm;
do not imply that bass EQ provides it.

## Priority follow-ups

1. Multi-hour physical mic/headphones/cable tests under gaming CPU load; actual
   unplug/replug and Windows default-device/format changes.
2. Stable Windows endpoint IDs and manufacturer/interface metadata; single-instance
   protection; preserve capture when only the monitoring sink changes.
3. Higher quality asynchronous resampling and stronger click-free transition tests.
4. Perceptual RNNoise tests, more sample rates, and aligned removed-audio monitoring.
5. Natural pitch/formants, then a separately developed/signed virtual driver.
