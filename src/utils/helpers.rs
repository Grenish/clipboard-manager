// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Format bytes into human-readable size string
#[inline]
pub fn format_size(bytes: u64) -> String {
    match bytes {
        b if b < 1024 => format!("{} B", b),
        b if b < 1024 * 1024 => format!("{:.1} KB", b as f64 / 1024.0),
        b => format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)),
    }
}

use crate::clipboard::ClipboardBackend;
use std::process::Command;
use std::{thread, time::Duration};

/// Performs paste action (intended to be run in a detached process)
pub fn perform_background_paste(backend: ClipboardBackend) {
    // Wait for the main window to largely close and focus to return
    thread::sleep(Duration::from_millis(300));

    // Try wtype (Wayland - wlroots based compositors)
    if matches!(backend, ClipboardBackend::WlClipboard) {
        if Command::new("wtype")
            .arg("-M")
            .arg("ctrl")
            .arg("-k")
            .arg("v")
            .arg("-m")
            .arg("ctrl")
            .spawn()
            .and_then(|mut c| c.wait())
            .map(|s| s.success())
            .unwrap_or(false)
        {
            println!("✓ Pasted using wtype");
            return;
        }
    }

    // Try ydotool (Wayland/X11 - uses uinput, works universally)
    if Command::new("ydotool")
        .arg("key")
        .arg("29:1")  // Ctrl press (keycode 29)
        .arg("47:1")  // V press (keycode 47)
        .arg("47:0")  // V release
        .arg("29:0")  // Ctrl release
        .spawn()
        .and_then(|mut c| c.wait())
        .map(|s| s.success())
        .unwrap_or(false)
    {
        println!("✓ Pasted using ydotool");
        return;
    }

    // Try xdotool (X11)
    if matches!(backend, ClipboardBackend::Arboard) {
        if Command::new("xdotool")
            .arg("key")
            .arg("ctrl+v")
            .spawn()
            .and_then(|mut c| c.wait())
            .map(|s| s.success())
            .unwrap_or(false)
        {
            println!("✓ Pasted using xdotool");
            return;
        }
    }

    // No tools available - show helpful message
    eprintln!("⚠ Auto-paste failed: No compatible input simulation tool found.");
    eprintln!("  Install one of the following:");
    eprintln!("    - wtype (Wayland/Hyprland): sudo pacman -S wtype");
    eprintln!("    - ydotool (universal):      sudo pacman -S ydotool");
    eprintln!("    - xdotool (X11 only):       sudo pacman -S xdotool");
}

