# Bundled DSP sources

These are upstream dependencies, not Luna Mic-owned code. Do not relicense them.

- SpeexDSP 1.2.1 from Xiph, commit `1b28a0f61bc31162979e1f26f3981fc3637095c8`.
  BSD-style COPYING and per-file notices apply. The preprocessing, echo-residual
  support, filterbank and small FFT sources compile statically with the Windows
  C++ toolchain. Source algorithms are unchanged; no external DLL is required.
- DeepFilterNet 0.5.6, commit `978576aa8400552a4ce9730838c635aa30db5e61`.
  libDF, its bundled models and upstream licenses are included. MIT / Apache-2.0.
  Only Cargo lint declarations were added for current Rust; DSP/inference source
  is unchanged. Selected inference uses its own worker, not the audio callback.

Temporary retrieval checkouts are excluded from the repository. The source used by
the build is here, exported from the tags without nested Git repositories.
Regenerate dependency notices after changes. Distribute Speex notices and both
DeepFilterNet licenses with the executable. Models are embedded for offline use.

VB-CABLE is proprietary donationware; see VB-CABLE-NOTICE.md and its original readme.
