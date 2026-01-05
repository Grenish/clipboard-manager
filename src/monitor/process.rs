use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::clipboard::{
    ClipboardBackend, get_clipboard_image, get_clipboard_text, get_clipboard_types,
};
use crate::history::ClipboardHistory;
use crate::utils::{PID_FILE, POLL_INTERVAL_MS};

// ============================================================================
// PID FILE MANAGEMENT
// ============================================================================

pub fn write_pid_file(data_dir: &PathBuf) -> Result<(), std::io::Error> {
    let pid_path = data_dir.join(PID_FILE);
    fs::write(pid_path, std::process::id().to_string())
}

pub fn remove_pid_file(data_dir: &PathBuf) {
    let _ = fs::remove_file(data_dir.join(PID_FILE));
}

pub fn get_trigger_script_path(data_dir: &PathBuf) -> PathBuf {
    data_dir.join("trigger.sh")
}

pub fn create_trigger_script(data_dir: &PathBuf, binary_path: &str) -> Result<(), std::io::Error> {
    let script_path = get_trigger_script_path(data_dir);

    let script_content = format!(
        r#"#!/bin/bash
BINARY="{}"

if command -v kitty &> /dev/null; then
    kitty --class floating-clipboard \
          --title "Clipboard Manager" \
          -o initial_window_width=900 \
          -o initial_window_height=600 \
          -o remember_window_size=no \
          "$BINARY" --ui &
elif command -v alacritty &> /dev/null; then
    alacritty --class floating-clipboard \
              --title "Clipboard Manager" \
              -o window.dimensions.columns=100 \
              -o window.dimensions.lines=30 \
              -e "$BINARY" --ui &
elif command -v foot &> /dev/null; then
    foot --app-id=floating-clipboard \
         --title="Clipboard Manager" \
         --window-size-chars=100x30 \
         "$BINARY" --ui &
else
    notify-send "Clipboard Manager" "No suitable terminal found"
fi
"#,
        binary_path
    );

    fs::write(&script_path, script_content)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms)?;
    }

    Ok(())
}

// ============================================================================
// POLLING MONITOR (FALLBACK)
// ============================================================================

pub fn monitor_loop(history: Arc<ClipboardHistory>, backend: ClipboardBackend) {
    println!("📋 Clipboard monitor started (Polling Fallback)");

    let mut last_text_hash: Option<u64> = None;
    let mut last_image_hash: Option<u64> = None;
    let mut poll_count = 0u64;

    loop {
        thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
        poll_count += 1;

        // Heartbeat every ~10 seconds
        if poll_count % 67 == 0 {

            // println!("💓 Monitor active - {} items in history", count);
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
}
