use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;

use crate::clipboard::{ClipboardBackend, get_clipboard_image, get_clipboard_text};
use crate::history::ClipboardHistory;

pub fn monitor_wayland(history: Arc<ClipboardHistory>) {
    thread::spawn(move || {
        println!("Displaying Wayland watcher...");
        
        // We need to watch both text and images. 
        // Unfortunately `wl-paste --watch` only accepts one command.
        // So we will run two watchers? Or just one generic one?
        // Actually `wl-paste --watch command` runs `command` whenever clipboard changes.
        // But it doesn't tell us WHAT changed (type wise) easily without checking.
        
        // A common pattern is to just run a no-op or simple trigger.
        // Let's use `wl-paste --watch echo` as a trigger.
        // OR better: use `wl-paste --watch` and spawn our own handler.
        
        // Command::new("wl-paste")
        //     .arg("--watch")
        //     .arg("echo") // Generic trigger
        //     ...
        
        // However, `wl-paste --watch` keeps running and executes the command.
        // We can't easily capture the output of the *command* nicely in a single stream if it spawns separate processes.
        // But wait, `wl-paste --watch <command>` runs `<command>` with the content on stdin? No.
        // Man page: "Run command ... whenever the clipboard content changes."
        
        // If we want to stay within Rust, we can't easily "watch" without polling if we don't use the tool's watch mode.
        // But `wl-paste --watch` is a blocking command that execs the callback.
        
        // Standard approach:
        // Spawn `wl-paste --watch` which is a long running process? 
        // No, `wl-paste --watch` *is* the watcher.
        
        // Let's implement a loop that spawns `wl-paste --watch` which blocks until change?
        // No, `wl-paste --watch` *runs* and *executes* the command on change.
        
        // Alternative: Use `wl-paste --watch` to write a specific byte to a FIFO or similar?
        // Or simpler: The rust standard library doesn't easily support "monitor this fd".
        
        // Let's try this:
        // Check `wl-clipboard-rs` crate? No, we want to use the binary if possible as per existing pattern.
        
        // We can run `wl-paste --watch` and have it print a delimiter to stdout.
        // `wl-paste --watch echo "CHANGED"`
        
        let mut cmd = Command::new("wl-paste")
            .arg("--watch")
            .arg("echo")
            .arg("CHANGED")
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to start wl-paste watcher");

        let stdout = cmd.stdout.take().expect("Failed to open stdout");
        let reader = BufReader::new(stdout);

        for line in reader.lines() {
            if let Ok(l) = line {
                if l.trim() == "CHANGED" {
                    // Clipboard changed!
                     handle_clipboard_change(&history);
                }
            }
        }
        
        let _ = cmd.wait();
    });
}

fn handle_clipboard_change(history: &Arc<ClipboardHistory>) {
    // We don't know if it's text or image, and wl-clipboard prefers one.
    // We try image first, then text.
    
    // We need to pass the backend. Since we are in wayland monitor, it's definitely Wayland.
    let backend = ClipboardBackend::WlClipboard;
    
    // Check for images
    if let Some(image_data) = get_clipboard_image(backend) {
         if let Err(e) = history.add_image(image_data) {
             eprintln!("Error adding image: {}", e);
         }
         return;
    }
    
    // Check for text
    if let Some(text) = get_clipboard_text(backend) {
        history.add_text(text);
    }
}
