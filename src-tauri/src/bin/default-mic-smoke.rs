//! Explicit Windows integration check. Default mode only reads system choices.
//! --set-and-restore exercises the setter and then restores all previous roles.
#[cfg(windows)]
fn main() -> Result<(), String> {
    use luna_mic_core::windows_default::{
        default_capture_ids, restore_capture_ids, set_default_microphone,
    };
    let previous = default_capture_ids()?;
    println!("Current console/media/call recording defaults: {previous:?}");
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("--set-and-restore") {
        println!(
            "To test the setter explicitly: --set-and-restore <microphone ID from audio-smoke>"
        );
        return Ok(());
    }
    let selected = args.get(1).ok_or("Missing microphone ID")?;
    let result = set_default_microphone(selected);
    let tested = default_capture_ids();
    // Restore even if setting or verification failed. Tests must not leave another
    // microphone selected for ongoing calls or apps on the developer's machine.
    restore_capture_ids(&previous)?;
    if default_capture_ids()? != previous {
        return Err("Windows microphone defaults did not restore".into());
    }
    println!("{}", result?);
    let tested = tested?;
    if !(tested[0] == tested[1] && tested[1] == tested[2]) {
        return Err("Recording roles did not agree after setting".into());
    }
    println!("PASS: selected mic applied to all recording roles; original choices restored.");
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("This test is only available on Windows.");
}
