use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use signal_hook::consts::signal::*;
use signal_hook::iterator::Signals;

use crate::clipboard::{
    ClipboardBackend, get_clipboard_image, get_clipboard_text, get_clipboard_types,
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
    thread::spawn(move || {
        println!("📋 Clipboard monitor started");

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
            let types = get_clipboard_types(backend);
            let has_image = types.iter().any(|t| t.starts_with("image/"));

            if has_image {
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
                }
            } else if let Some(content) = get_clipboard_text(backend) {
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
