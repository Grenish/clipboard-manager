use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;


use signal_hook::consts::signal::*;
use signal_hook::iterator::Signals;
use crate::clipboard::ClipboardBackend;
use crate::history::ClipboardHistory;



// ============================================================================
// SIGNAL LISTENER
// ============================================================================

pub fn start_signal_listener(shutdown_trigger: Arc<AtomicBool>) {
    thread::spawn(move || {
        if let Ok(mut signals) = Signals::new(&[SIGTERM, SIGINT]) {
            for signal in signals.forever() {
                if signal == SIGTERM || signal == SIGINT {
                    shutdown_trigger.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }
    });
}

// ============================================================================
// CLIPBOARD MONITORING
// ============================================================================

pub fn start_clipboard_monitor(history: Arc<ClipboardHistory>, backend: ClipboardBackend) {
    // Attempt to configure Hyprland window rules automatically
    crate::monitor::hyprland::apply_hyprland_rules();

    if matches!(backend, ClipboardBackend::WlClipboard) {
        // Use event-driven watcher for Wayland
        crate::monitor::wayland::monitor_wayland(history);
    } else {
        // Fallback to polling for other backends (e.g. Arboard/X11)
        thread::spawn(move || {
            crate::monitor::process::monitor_loop(history, backend);
        });
    }
}
