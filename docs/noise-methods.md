# Noise-removal methods

Cleanup → **Removal method** chooses RNNoise, SpeexDSP, WebRTC or DeepFilterNet 3.
Noise removal's switch turns the selected method off/on. Only one method is
constructed and active for a stream. This choice is saved separately from sound
presets, so choosing a tone preset does not silently change the engine.

All four initial integrations use mono 48 kHz input. An unsupported rate or a build
missing an engine disables that selection. Initialization errors leave the rest of
the processing chain available and display a warning. Switching methods restarts
the audio streams briefly so heavy model initialization stays outside callbacks.

| Method | Implementation | Trade-off |
|---|---|---|
| RNNoise | nnnoiseless 0.5.1, BSD-3-Clause | Lightweight neural cleanup; roughly 20 ms frame delay |
| SpeexDSP | Xiph SpeexDSP 1.2.1, statically compiled C, BSD notices | Traditional suppression, AGC/VAD disabled, -30 dB suppression setting |
| WebRTC | Sonora 0.2.0 Rust port, BSD-3-Clause | Only noise suppression enabled; echo cancellation and automatic gain remain off |
| DeepFilterNet 3 | DeepFilterNet 0.5.6 / Tract, MIT or Apache-2.0 | Larger embedded model, heavier CPU/memory and additional buffering |

Strength blends the filtered signal with delayed original audio. The adapter aligns
the dry history to its engine delay. WebRTC delay is measured with an isolated
initialization probe followed by a fresh state. DeepFilterNet delay comes from the
model hop/FFT/lookahead plus bounded worker buffering; the UI reports an estimate.
Headphone/driver latency is additional. Different engines will not necessarily have
identical loudness or preserve every voice/keyboard equally well; compare by ear.

DeepFilterNet's inference may allocate and its state is not transferred between
threads. It is created and kept on its own worker. Capture sends preallocated,
fixed-size frames through bounded SPSC queues and never waits for results. Late
frames are counted and become silence rather than unbounded latency. Diagnostics
shows missed/error frames; if these accumulate, choose a lighter method.

Models are embedded. There is no runtime download, Python dependency, cloud service,
account requirement or recording-to-disk. SpeexDSP needs the existing Visual C++
build tools during compilation, not an extra driver or runtime DLL. Rust 1.91+ is
required for the WebRTC port. Dependencies and notices retain their own licenses;
see `vendor/README.md` for exact source revisions and the one lint-only adjustment.
