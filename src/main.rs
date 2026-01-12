use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

mod clipboard;
mod history;
mod models;
mod monitor;
mod ui;
mod utils;

use clipboard::detect_clipboard_backend;
use history::ClipboardHistory;
use monitor::{
    create_trigger_script, get_trigger_script_path, remove_pid_file, start_clipboard_monitor,
    start_signal_listener, write_pid_file,
};
use ui::show_ui;

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    let args: Vec<String> = env::args().collect();
    let backend = detect_clipboard_backend();

    if args.len() > 1 && args[1] == "--ui" {
        if let Err(e) = show_ui(backend) {
            eprintln!("UI Error: {}", e);
            std::process::exit(1);
        }
        std::process::exit(0);
    }

    // Daemon mode
    println!("╔════════════════════════════════════════╗");
    println!("║     Clipboard Manager - Daemon Mode    ║");
    println!("╚════════════════════════════════════════╝\n");

    let history = Arc::new(ClipboardHistory::new());
    let data_dir = history.data_dir().clone();

    write_pid_file(&data_dir).ok();

    let binary_path = env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(String::from))
        .unwrap_or_else(|| String::from("clipboard-manager"));

    create_trigger_script(&data_dir, &binary_path).ok();

    let shutdown_trigger = Arc::new(AtomicBool::new(false));
    start_signal_listener(Arc::clone(&shutdown_trigger));
    start_clipboard_monitor(Arc::clone(&history), backend);

    println!("✓ Backend: {:?}", backend);
    println!("✓ Data dir: {}", data_dir.display());
    println!("✓ Trigger: {}\n", get_trigger_script_path(&data_dir).display());
    
    println!("ℹ Auto-configuration is active for Hyprland.");
    println!("  If the window doesn't float, add this rule to hyprland.conf:");
    println!("    windowrule = float on, match:class floating-clipboard");
    println!();
    println!(
        "  Bind key to open UI:\n    bind = SUPER, V, exec, {}",
        get_trigger_script_path(&data_dir).display()
    );
    println!();

    while !shutdown_trigger.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(100));
    }

    println!("\nShutting down...");

    remove_pid_file(&data_dir);
}
