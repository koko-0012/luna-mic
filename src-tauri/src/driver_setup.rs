//! Driver setup runs only on the control/UI worker, never an audio callback.
//! The package is fixed beside the executable; IPC accepts no paths or commands.
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct DriverPackageStatus {
    pub install_available: bool,
    pub message: String,
}
fn package_dir() -> Result<PathBuf, String> {
    std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .map(|parent| parent.join("vb-cable"))
        .ok_or_else(|| "Application folder unavailable.".into())
}
fn run_installer(package: &Path, check_only: bool) -> Result<String, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let system = std::env::var_os("SystemRoot").ok_or("Windows folder unavailable")?;
        let powershell =
            PathBuf::from(system).join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let output = std::process::Command::new(powershell)
            // A VS Code/PowerShell 7 parent can supply incompatible module paths.
            // Let Windows PowerShell initialize its own built-in modules.
            .env_remove("PSModulePath")
            // Process-only script policy for our bundled installer; never alters
            // system policy, kernel signing, Secure Boot or certificate stores.
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(package.join("install-vb-cable.ps1"))
            .arg("-Package")
            .arg(package)
            .arg(if check_only { "-CheckOnly" } else { "-Elevate" })
            .creation_flags(0x08000000)
            .output()
            .map_err(|e| format!("Could not start driver setup: {e}"))?;
        if !output.status.success() {
            #[cfg(test)]
            eprintln!(
                "Setup preflight failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return Err(if check_only {
                "The official VB-CABLE installer could not be verified. Re-extract the complete release ZIP."
                    .into()
            } else {
                "Installation did not finish. Administrator permission may have been canceled, or Windows rejected the package.".into()
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().into())
    }
    #[cfg(not(windows))]
    {
        let _ = (package, check_only);
        Err("Driver installation requires Windows.".into())
    }
}
fn check_at(package: &Path) -> DriverPackageStatus {
    let complete = [
        "vbMmeCable64_win10.inf",
        "vbaudio_cable64_win10.sys",
        "vbaudio_cable64_win10.cat",
        "VBCABLE_Setup_x64.exe",
        "install-vb-cable.ps1",
    ]
    .iter()
    .all(|name| package.join(name).is_file());
    if !complete {
        return DriverPackageStatus {
            install_available: false,
            message: "Driver package is not included. Use a complete Luna Mic release ZIP.".into(),
        };
    }
    match run_installer(package, true) {
        Ok(_) => DriverPackageStatus {
            install_available: true,
            message: "Ready to install. Windows will ask for administrator permission.".into(),
        },
        Err(message) => DriverPackageStatus {
            install_available: false,
            message,
        },
    }
}
pub fn status() -> Result<DriverPackageStatus, String> {
    Ok(check_at(&package_dir()?))
}
pub fn install() -> Result<String, String> {
    let package = package_dir()?;
    let checked = check_at(&package);
    if !checked.install_available {
        return Err(checked.message);
    }
    // The elevated script revalidates signature and membership immediately before
    // installation, rather than trusting the earlier status displayed in the UI.
    run_installer(&package, false)?;
    Ok("VB-CABLE setup closed. If installed, restart Windows and reopen Luna Mic. Then choose CABLE Output in Discord.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_package_cannot_install() {
        let absent = Path::new(env!("CARGO_MANIFEST_DIR")).join("__no_driver_package__");
        assert!(!check_at(&absent).install_available);
    }
    #[test]
    #[cfg(windows)]
    fn bundled_vendor_installer_verifies_without_installing() {
        let package = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/release/vb-cable");
        if package.exists() {
            assert!(run_installer(&package, true).is_ok());
        }
    }
}
