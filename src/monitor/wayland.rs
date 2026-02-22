use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;

use crate::clipboard::{ClipboardBackend, get_clipboard_image, get_clipboard_text};
use crate::history::ClipboardHistory;

pub fn monitor_wayland(history: Arc<ClipboardHistory>) {
    thread::spawn(move || {
        println!("Displaying Wayland watcher...");
        
        // We use wl-paste --watch to output a delimiter "CHANGED" whenever clipboard content changes.
        // This avoids polling and uses Wayland's native change notification.
        let mut cmd = Command::new("wl-paste")
            .arg("--watch")
            .arg("echo")
            .arg("CHANGED")
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to start wl-paste watcher");

        let stdout = cmd.stdout.take().expect("Failed to open stdout");
        let reader = BufReader::new(stdout);

        let mut last_hash: Option<u64> = None;

        for line in reader.lines() {
            if let Ok(l) = line {
                if l.trim() == "CHANGED" {
                     handle_clipboard_change(&history, &mut last_hash);
                }
            }
        }
        
        let _ = cmd.wait();
    });
}

fn handle_clipboard_change(history: &Arc<ClipboardHistory>, last_hash: &mut Option<u64>) {
    // We assume Wayland backend since this is the specific Wayland monitor
    let backend = ClipboardBackend::WlClipboard;
    
    // Check for images first
    if let Some(image_data) = get_clipboard_image(backend) {
         use std::collections::hash_map::DefaultHasher;
         use std::hash::{Hash, Hasher};

         let mut hasher = DefaultHasher::new();
         image_data.hash(&mut hasher);
         let hash = hasher.finish();
         
         if Some(hash) != *last_hash {
             if let Err(e) = history.add_image(image_data) {
                 eprintln!("Error adding image: {}", e);
             }
             *last_hash = Some(hash);
         }
         return;
    }
    
    // Check for text
    if let Some(text) = get_clipboard_text(backend) {
         use std::collections::hash_map::DefaultHasher;
         use std::hash::{Hash, Hasher};

         let mut hasher = DefaultHasher::new();
         text.hash(&mut hasher);
         let hash = hasher.finish();
         
         if Some(hash) != *last_hash {
             history.add_text(text);
             *last_hash = Some(hash);
         }
    }
}
