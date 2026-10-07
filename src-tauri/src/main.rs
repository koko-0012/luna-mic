#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod driver_setup;
#[tauri::command]
fn close_setup(app: tauri::AppHandle) {
    app.exit(0);
}
#[tauri::command]
fn vb_cable_website() -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", "https://vb-audio.com/Cable/"])
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
use luna_mic_core::{
    audio::engine::{Engine, Snapshot},
    settings::{self, Config, Preset},
};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};

struct Inner {
    config: Mutex<Config>,
    engine: Engine,
    path: PathBuf,
    load_warning: Option<String>,
}
#[derive(Clone)]
struct AppState(Arc<Inner>);
#[derive(Serialize)]
struct Initial {
    config: Config,
    builtins: Vec<Preset>,
    warning: Option<String>,
    settings_path: String,
}
#[tauri::command]
async fn driver_package_status() -> Result<driver_setup::DriverPackageStatus, String> {
    tauri::async_runtime::spawn_blocking(driver_setup::status)
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn install_virtual_driver(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let state = state.inner().clone();
    let result = tauri::async_runtime::spawn_blocking(driver_setup::install)
        .await
        .map_err(|e| e.to_string())?;
    state.0.engine.refresh();
    result
}
#[tauri::command]
fn initial(state: tauri::State<AppState>) -> Result<Initial, String> {
    let config = state
        .0
        .config
        .lock()
        .map_err(|_| "Settings unavailable")?
        .clone();
    Ok(Initial {
        config,
        builtins: settings::builtins(),
        warning: state.0.load_warning.clone(),
        settings_path: state.0.path.display().to_string(),
    })
}
#[tauri::command]
fn snapshot(state: tauri::State<AppState>) -> Result<Snapshot, String> {
    state
        .0
        .engine
        .snapshot
        .lock()
        .map(|s| s.clone())
        .map_err(|_| "Audio status unavailable".into())
}
#[tauri::command]
fn refresh_devices(state: tauri::State<AppState>) {
    state.0.engine.refresh();
}
#[tauri::command]
async fn set_default_microphone(
    state: tauri::State<'_, AppState>,
    selected: String,
) -> Result<String, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = luna_mic_core::windows_default::set_default_microphone(&selected);
        state.0.engine.refresh();
        result
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn open_sound_settings() -> Result<(), String> {
    // Fixed executable and arguments; never interpolate a device name into a command.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("control.exe")
            .arg("mmsys.cpl,,1")
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("Windows Sound settings are only available on Windows.".into())
    }
}
fn apply(state: &AppState, config: Config) -> Result<(), String> {
    config.validate()?;
    let mut previous = state.0.config.lock().map_err(|_| "Settings unavailable")?;
    if config.start_with_windows != previous.start_with_windows {
        autostart(config.start_with_windows)?;
    }
    // Serialize control changes outside callbacks. Do not persist a configuration
    // the audio worker could not accept; roll back the worker if disk saving fails.
    if let Err(e) = state.0.engine.update(config.clone()) {
        if config.start_with_windows != previous.start_with_windows {
            let _ = autostart(previous.start_with_windows);
        }
        return Err(e);
    }
    if let Err(e) = settings::save(&state.0.path, &config) {
        let _ = state.0.engine.update(previous.clone());
        if config.start_with_windows != previous.start_with_windows {
            let _ = autostart(previous.start_with_windows);
        }
        return Err(format!("Could not save settings: {e}"));
    }
    *previous = config;
    Ok(())
}
#[tauri::command]
async fn set_config(state: tauri::State<'_, AppState>, config: Config) -> Result<(), String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || apply(&state, config))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
fn export_preset(state: tauri::State<AppState>, preset: Preset) -> Result<String, String> {
    preset.parameters.validate()?;
    let dir = state
        .0
        .path
        .parent()
        .ok_or("Invalid settings location")?
        .join("exports");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("luna-mic-preset.json");
    let text = serde_json::to_string_pretty(&preset).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}
#[cfg(windows)]
fn autostart(enabled: bool) -> Result<(), String> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        .map_err(|e| e.to_string())?;
    // Remove only our recognizable pre-rename startup entry, avoiding two copies.
    if let Ok(old) = key.get_value::<String, _>("MicLab") {
        if PathBuf::from(old.trim_matches('"'))
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("miclab.exe"))
        {
            key.delete_value("MicLab").map_err(|e| e.to_string())?;
        }
    }
    if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        key.set_value("Luna Mic", &format!("\"{}\"", exe.display()))
            .map_err(|e| e.to_string())
    } else {
        match key.delete_value("Luna Mic") {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}
#[cfg(not(windows))]
fn autostart(_: bool) -> Result<(), String> {
    Err("Start with Windows is only supported on Windows.".into())
}

fn main() {
    tauri::Builder::default()
        // Register first: a second launch restores the existing window rather than
        // starting a second capture engine or creating another tray icon.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let path = app.path().app_config_dir()?.join("settings.json");
            let legacy = path
                .parent()
                .and_then(|p| p.parent())
                .map(|root| root.join("org.miclab.desktop/settings.json"));
            let (mut config, mut warning) = match legacy {
                Some(legacy) => settings::load_with_legacy(&path, &legacy),
                None => settings::load(&path),
            };
            // An enabled startup preference follows the renamed executable. No
            // startup entry is created unless the user had already enabled it.
            if config.start_with_windows {
                if let Err(error) = autostart(true) {
                    config.start_with_windows = false;
                    warning = Some(format!(
                        "Could not update Windows startup for Luna Mic: {error}"
                    ));
                }
            }
            let minimized = config.start_minimized;
            let state = AppState(Arc::new(Inner {
                engine: Engine::new(config.clone()),
                config: Mutex::new(config),
                path,
                load_warning: warning,
            }));
            app.manage(state);
            let open = MenuItem::with_id(app, "open", "Open Luna Mic", true, None::<&str>)?;
            let enabled =
                MenuItem::with_id(app, "enable", "Mute / Unmute Output", true, None::<&str>)?;
            let bypass = MenuItem::with_id(
                app,
                "bypass",
                "Effects On / Off (Raw Audio)",
                true,
                None::<&str>,
            )?;
            let exit = MenuItem::with_id(app, "exit", "Exit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &enabled, &bypass, &exit])?;
            let icon = tauri::image::Image::new_owned(tray_pixels(), 32, 32);
            TrayIconBuilder::new()
                .icon(icon)
                .tooltip("Luna Mic • microphone processing")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.unminimize();
                            let _ = w.set_focus();
                        }
                    }
                    "exit" => app.exit(0),
                    "enable" | "bypass" => {
                        let app = app.clone();
                        let state = app.state::<AppState>().inner().clone();
                        let bypass = event.id.as_ref() == "bypass";
                        std::thread::spawn(move || {
                            let config = state.0.config.lock().ok().map(|c| c.clone());
                            if let Some(mut c) = config {
                                if bypass {
                                    c.parameters.bypass = !c.parameters.bypass;
                                } else {
                                    c.parameters.enabled = !c.parameters.enabled;
                                }
                                if let Err(error) = apply(&state, c) {
                                    let _ = app.emit("settings-error", error);
                                }
                            }
                        });
                    }
                    _ => {}
                })
                .build(app)?;
            if !minimized {
                if let Some(w) = app.get_webview_window("main") {
                    w.show()?;
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                if state
                    .0
                    .config
                    .lock()
                    .map(|c| c.minimize_to_tray)
                    .unwrap_or(false)
                {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            initial,
            snapshot,
            set_config,
            refresh_devices,
            export_preset,
            set_default_microphone,
            open_sound_settings,
            driver_package_status,
            install_virtual_driver,
            close_setup,
            vb_cable_website
        ])
        .run(tauri::generate_context!())
        .expect("Could not start Luna Mic desktop runtime");
}
fn tray_pixels() -> Vec<u8> {
    let mut pixels = vec![0; 32 * 32 * 4];
    for y in 0..32 {
        for x in 0..32 {
            let microphone = (12..20).contains(&x) && (4..21).contains(&y);
            let stand = (15..17).contains(&x) && (20..28).contains(&y)
                || (9..23).contains(&x) && (27..29).contains(&y);
            if microphone || stand {
                let n = (y * 32 + x) * 4;
                pixels[n..n + 4].copy_from_slice(&[184, 162, 230, 255]);
            }
        }
    }
    pixels
}
