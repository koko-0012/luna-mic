# Third-party dependencies and distribution notices

The portable release also bundles the **standard VB-CABLE Pack45** by VB-Audio.
This is proprietary donationware, excluded from Luna Mic's MIT license. Original
upstream files and end-user license/readme are preserved in `vb-cable/`.
VB-Audio's current distribution permission requires attribution and maintaining
the donationware model; the app's setup popup identifies VB-Audio and links the
official website/donation page. Paid A+B/C+D cables are not bundled. See
`VB-CABLE-NOTICE.md`, https://vb-audio.com/Services/licensing.htm and the original
`vb-cable/readme.txt`. The vendor installer runs interactively with user consent.

Luna Mic's MIT license covers only its original source, documentation and SVG /
derived app icons. Manufacturer names identify devices; no manufacturer photos,
logos or copyrighted product art are included. The generic illustration is
original to this project.

Exact Cargo/npm versions and upstream license declarations are recorded in
`docs/dependencies.json`, generated from local installed package metadata and the
committed lockfiles. Inventory includes development and other-platform packages;
it is broader than the contents of a Windows executable. It does not assign
Luna Mic copyright to any upstream material.

| Direct dependency | License | Purpose / Windows status |
|---|---|---|
| Tauri / tauri-build / JS API / CLI | MIT OR Apache-2.0 | Native WebView2 desktop, commands, tray and build tooling; compiled on Windows |
| tauri-plugin-single-instance | MIT OR Apache-2.0 | Prevent duplicate desktop capture engines; Windows supported |
| CPAL 0.16.0 | Apache-2.0 | WASAPI input/output; tested on Windows float32 shared-mode devices |
| ringbuf 0.4.8 | MIT OR Apache-2.0 | Bounded lock-free SPSC audio queues |
| triple_buffer 9.0.0 | MPL-2.0 | Nonblocking fixed-size DSP parameter / telemetry exchange |
| serde / serde_json | MIT OR Apache-2.0 | Local configuration/preset serialization, outside callbacks |
| windows 0.54 | MIT OR Apache-2.0 | Native endpoint notification COM interfaces |
| winreg 0.55 | MIT | Optional current-user Windows login startup value |
| TypeScript | Apache-2.0 | Frontend type checking; build tool |
| Vite | MIT | Frontend development and production asset bundling; build tool |
| Prettier | MIT | Readable frontend formatting and VS Code workflow; build tool |
| nnnoiseless 0.5.1 / rustfft | BSD-3-Clause / MIT OR Apache-2.0 | Optional local RNNoise suppression; preallocated model and FFT buffers; 48 kHz |
| Sonora / Sonora NS 0.2.0 | BSD-3-Clause | Pure Rust port of WebRTC; only noise suppression enabled |
| SpeexDSP 1.2.1 | BSD-style, per-file notices | Statically compiled preprocessing and FFT source; COPYING and source notices distributed |
| DeepFilterNet 0.5.6 / Tract | MIT OR Apache-2.0 | Embedded DeepFilterNet 3 model with isolated inference worker |

CPAL and the buffer dependencies were inspected for callback suitability. Callback
buffers/state are preallocated; file operations and settings validation are not
performed by the audio callback. Actual Rust sources and official CPAL/Tauri
examples were consulted instead of assuming APIs. CPAL upstream remains maintained
with newer releases; 0.16 is a deliberate tested API selection.

## License compatibility and obligations

The locked dependency license expressions offer permissive licenses or MPL-2.0;
none requires relicensing Luna Mic's own files as GPL. Some other-platform packages
offer LGPL as an alternative to MIT / Apache; that alternative need not be chosen.
Unicode data and other BSD/ISC/Zlib notices must be retained where included.

MPL is a file-level copyleft license. Unmodified MPL dependency sources remain
MPL; modifications to those files must remain available under MPL when distributed.
For executable distribution, make their corresponding source available to recipients
and tell recipients where to find it. A portable archive should include
`third-party/mpl-source/` and these notices. Our own separate Rust files remain MIT.
The attribution script copies MPL sources for the exact lockfile versions used.

Run `pwsh -File scripts/dependency-notices.ps1` after dependency changes. It writes
the version/license inventory, copies original upstream license/notice files,
and includes source of MPL dependencies. Ship the resulting `third-party/`
directory, this file, and LICENSE alongside the executable. Review the inventory
and any new upstream obligations before publishing. This is a practical project
license review, not a warranty covering future dependency updates or modifications.

Microsoft Edge WebView2 / Windows are external, separately licensed Microsoft
runtimes. WebView2 binaries are not bundled here. Installer terms and redistribution
requirements need separate review if an installer is introduced later.
