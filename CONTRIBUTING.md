# Working on Luna Mic

Open the repository root in VS Code. Recommended extensions and tasks are in
`.vscode/`. The app uses Rust, Tauri 2, TypeScript and Vite on Windows 10/11 x64.

For development, install Node.js 24+, Rust 1.91+ with the MSVC toolchain, Visual
Studio C++ build tools and the Windows SDK. WebView2 is required by Tauri.

```powershell
npm ci
npm run app
npm run check
npm run release
pwsh -File scripts/prepare-vb-cable.ps1
pwsh -File scripts/dependency-notices.ps1
pwsh -File scripts/package-release.ps1
```

`check` runs type checking, formatting, Clippy, unit tests and the DSP
allocation test. The app requires the standard VB-CABLE endpoints; the original
vendor setup is offered in the app. For development, copy `vendor/vb-cable` to
`src-tauri/target/debug/vb-cable` and copy `virtual-device/install-vb-cable.ps1`
into that folder. VB-CABLE installation remains an explicit user action.

## Source map

| Area | Location |
|---|---|
| Layout and styles | `ui/view.ts`, `ui/style.css` |
| UI behavior and setup popup | `ui/main.ts` |
| IPC contracts | `ui/types.ts` |
| Audio pipeline and processors | `src-tauri/src/audio/` |
| Microphone enumeration | `src-tauri/src/devices.rs` |
| Settings and presets | `src-tauri/src/settings.rs` |
| Desktop commands and tray | `src-tauri/src/main.rs` |
| Verified VB-CABLE launcher | `src-tauri/src/driver_setup.rs`, `virtual-device/` |
| Microphone artwork | `assets/microphones/` |
| Required upstream source and models | `vendor/` |

The audio callback must not allocate, lock mutexes, perform file/network access,
or call the UI. Initialization and model work belong to the control thread or
isolated worker. Existing comments explain ownership and timing constraints.

Keep both Rust and TypeScript contracts in sync when adding settings. Run
`npm run check` after code changes and follow `docs/testing.md` for audio checks.
Hosted CI cannot verify physical microphone quality or driver installation.

## Releases

The Windows check workflow validates pushes and pull requests. A version-tag
workflow builds and packages the portable Windows ZIP as a GitHub Release.
Release files include upstream notices, the VB-CABLE vendor license and required
MPL dependency sources. Keep those files when repackaging the app.

Update versions together in `package.json`, `package-lock.json`,
`src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json` and the UI version label.
Regenerate dependency notices after changing dependencies:

```powershell
pwsh -File scripts/dependency-notices.ps1
```
