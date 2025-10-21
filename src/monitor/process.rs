use std::fs;
use std::path::PathBuf;

use crate::utils::PID_FILE;

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
