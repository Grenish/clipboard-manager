use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use signal_hook::consts::signal::*;
use signal_hook::iterator::Signals;

use crate::clipboard::{
    ClipboardBackend, get_clipboard_image, get_clipboard_text,
};
use crate::history::ClipboardHistory;
use crate::utils::POLL_INTERVAL_MS;

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
    if matches!(backend, ClipboardBackend::WlClipboard) {
        // Use event-driven watcher for Wayland
        crate::monitor::wayland::monitor_wayland(history);
    } else {
        // Fallback to polling for other backends (e.g. Arboard/X11)
        thread::spawn(move || {
            println!("📋 Clipboard monitor started (Polling Mode)");

            let mut last_text_hash: Option<u64> = None;
            let mut last_image_hash: Option<u64> = None;
            let mut poll_count = 0u64;

            loop {
                thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
                poll_count += 1;

                // Heartbeat every ~10 seconds
                if poll_count % 67 == 0 {
                    let count = history.get_all().len();
                    println!("💓 Monitor active - {} items in history", count);
                }

                // Check for images first (higher priority)
                // Note: process::get_clipboard_types might not exist or be needed if we assume Arboard doesn't support it well?
                // Actually `get_clipboard_types` in backend.rs handles Arboard by returning empty vec.
                
                // For Arboard, we just try to get image, then text.
                if let Some(image_data) = get_clipboard_image(backend) {
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};

                    let mut hasher = DefaultHasher::new();
                    image_data.hash(&mut hasher);
                    let hash = hasher.finish();

                    if Some(hash) != last_image_hash {
                        if let Err(e) = history.add_image(image_data) {
                            eprintln!("Failed to add image: {}", e);
                        }
                        last_image_hash = Some(hash);
                        last_text_hash = None;
                    }
                } else if let Some(content) = get_clipboard_text(backend) {
                    // Only check text if no image found (or if image failed?)
                    // Logic from before:
                    // if has_image { check image } else if text { check text }
                    // Arboard doesn't support `get_clipboard_types`, so we just try both?
                    // But if we have text, we might not have image.
                    // Let's stick to the previous logic but slightly adapted for "no types check"
                    
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};

                    let mut hasher = DefaultHasher::new();
                    content.hash(&mut hasher);
                    let hash = hasher.finish();

                    if Some(hash) != last_text_hash {
                        history.add_text(content);
                        last_text_hash = Some(hash);
                        last_image_hash = None;
                    }
                }
            }
        });
    }
}
