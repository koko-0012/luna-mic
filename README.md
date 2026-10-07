# Luna Mic

A lightweight Windows microphone processor with a compact dark interface.
Clean up background noise, shape your voice, and send the result to Discord,
OBS, games and other voice apps. Audio processing runs locally on your PC.

**[Download the latest Windows release](https://github.com/koko-0012/luna-mic/releases/latest)**

## What it does

- Noise removal with RNNoise, SpeexDSP, WebRTC or DeepFilterNet 3.
- A draggable noise gate, EQ, compression and peak protection.
- Voice presets and controls for bass, warmth, treble and clarity.
- Headphone monitoring to compare your original and processed voice.
- Microphone selection, live levels and a Windows default-microphone option.
- Saved settings, system tray support and optional startup with Windows.

Luna Mic uses **VB-CABLE** to make processed audio available to other apps.
The app detects it and offers the official setup when needed. VB-CABLE is
[donationware by VB-Audio](https://vb-audio.com/Cable/); donations are welcome.
Current noise-removal methods require a 48 kHz microphone input.

No account or cloud audio processing is required.

## Source

Built with Rust, Tauri and TypeScript. Developer information is in
[CONTRIBUTING.md](CONTRIBUTING.md).

Luna Mic's own code is MIT licensed. Dependencies and VB-CABLE retain their
respective licenses; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
