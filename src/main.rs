use std::collections::VecDeque;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use arboard::Clipboard;
use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode, KeyEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use serde::{Deserialize, Serialize};
use signal_hook::consts::signal::*;
use signal_hook::iterator::Signals;

// ============================================================================
// CONSTANTS
// ============================================================================

const MAX_HISTORY: usize = 50;
const POLL_INTERVAL_MS: u64 = 150;
const HISTORY_FILE: &str = "clipboard_history.json";
const PID_FILE: &str = "clipboard_manager.pid";
const IMAGES_DIR: &str = "images";
const MAX_DISPLAY_LENGTH: usize = 75;

// ============================================================================
// DATA STRUCTURES
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum ClipboardContentType {
    Text,
    Image,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ClipboardEntry {
    content_type: ClipboardContentType,
    content: String,
    timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_info: Option<ImageInfo>,
    #[serde(skip)]
    content_hash: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct ImageInfo {
    width: u32,
    height: u32,
    size_bytes: u64,
}

impl ClipboardEntry {
    fn new_text(content: String) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        let content_hash = hasher.finish();

        Self {
            content_type: ClipboardContentType::Text,
            content,
            timestamp: chrono::Utc::now().timestamp(),
            image_info: None,
            content_hash,
        }
    }

    fn new_image(filename: String, info: ImageInfo, hash: u64) -> Self {
        Self {
            content_type: ClipboardContentType::Image,
            content: filename,
            timestamp: chrono::Utc::now().timestamp(),
            image_info: Some(info),
            content_hash: hash,
        }
    }

    fn compute_hash(&mut self) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        match self.content_type {
            ClipboardContentType::Text => {
                self.content.hash(&mut hasher);
            }
            ClipboardContentType::Image => {
                self.content.hash(&mut hasher);
                self.timestamp.hash(&mut hasher);
            }
        }
        self.content_hash = hasher.finish();
    }

    fn formatted_time(&self) -> String {
        chrono::DateTime::from_timestamp(self.timestamp, 0)
            .map(|dt| dt.format("%H:%M:%S").to_string())
            .unwrap_or_else(|| String::from("--:--:--"))
    }

    fn display_content(&self) -> String {
        match self.content_type {
            ClipboardContentType::Text => {
                let content: String = self
                    .content
                    .chars()
                    .map(|c| if c == '\n' || c == '\t' { ' ' } else { c })
                    .collect();

                let trimmed = content.trim();
                if trimmed.len() > MAX_DISPLAY_LENGTH {
                    format!("{}...", &trimmed[..MAX_DISPLAY_LENGTH])
                } else {
                    trimmed.to_string()
                }
            }
            ClipboardContentType::Image => {
                if let Some(info) = &self.image_info {
                    format!(
                        "Image {}×{} ({})",
                        info.width,
                        info.height,
                        format_size(info.size_bytes)
                    )
                } else {
                    String::from("Image")
                }
            }
        }
    }

    fn icon(&self) -> &'static str {
        match self.content_type {
            ClipboardContentType::Text => "📝",
            ClipboardContentType::Image => "🖼️",
        }
    }
}

#[inline]
fn format_size(bytes: u64) -> String {
    match bytes {
        b if b < 1024 => format!("{} B", b),
        b if b < 1024 * 1024 => format!("{:.1} KB", b as f64 / 1024.0),
        b => format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)),
    }
}

// ============================================================================
// CLIPBOARD BACKEND
// ============================================================================

#[derive(Debug, Clone, Copy)]
enum ClipboardBackend {
    WlClipboard,
    Arboard,
}

fn detect_clipboard_backend() -> ClipboardBackend {
    if (env::var("WAYLAND_DISPLAY").is_ok()
        || env::var("XDG_SESSION_TYPE").map_or(false, |v| v == "wayland"))
        && Command::new("wl-paste").arg("--version").output().is_ok()
    {
        ClipboardBackend::WlClipboard
    } else {
        ClipboardBackend::Arboard
    }
}

// ============================================================================
// CLIPBOARD OPERATIONS
// ============================================================================

fn get_clipboard_types(backend: ClipboardBackend) -> Vec<String> {
    match backend {
        ClipboardBackend::WlClipboard => Command::new("wl-paste")
            .arg("--list-types")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default(),
        ClipboardBackend::Arboard => Vec::new(),
    }
}

fn get_clipboard_text(backend: ClipboardBackend) -> Option<String> {
    match backend {
        ClipboardBackend::WlClipboard => Command::new("wl-paste")
            .arg("--no-newline")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .filter(|s| !s.trim().is_empty()),
        ClipboardBackend::Arboard => Clipboard::new()
            .ok()
            .and_then(|mut cb| cb.get_text().ok())
            .filter(|s| !s.trim().is_empty()),
    }
}

fn get_clipboard_image(backend: ClipboardBackend) -> Option<Vec<u8>> {
    match backend {
        ClipboardBackend::WlClipboard => {
            for mime_type in &["image/png", "image/jpeg", "image/jpg", "image/bmp"] {
                if let Ok(output) = Command::new("wl-paste")
                    .arg("--type")
                    .arg(mime_type)
                    .output()
                {
                    if output.status.success() && !output.stdout.is_empty() {
                        return Some(output.stdout);
                    }
                }
            }
            None
        }
        ClipboardBackend::Arboard => Clipboard::new()
            .ok()
            .and_then(|mut cb| cb.get_image().ok())
            .and_then(|img| {
                use image::{ImageBuffer, RgbaImage};
                use std::io::Cursor;

                let img_buffer: RgbaImage =
                    ImageBuffer::from_raw(img.width as u32, img.height as u32, img.bytes.to_vec())?;

                let mut png_data = Vec::new();
                img_buffer
                    .write_to(&mut Cursor::new(&mut png_data), image::ImageFormat::Png)
                    .ok()?;

                Some(png_data)
            }),
    }
}

fn set_clipboard_text(content: &str, backend: ClipboardBackend) -> Result<(), String> {
    match backend {
        ClipboardBackend::WlClipboard => Command::new("wl-copy")
            .arg("--")
            .arg(content)
            .output()
            .map_err(|e| format!("Failed to run wl-copy: {}", e))
            .and_then(|output| {
                if output.status.success() {
                    Ok(())
                } else {
                    Err(format!(
                        "wl-copy failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ))
                }
            }),
        ClipboardBackend::Arboard => Clipboard::new()
            .and_then(|mut cb| cb.set_text(content))
            .map_err(|e| format!("Failed to set text: {}", e)),
    }
}

fn set_clipboard_image(image_path: &PathBuf, backend: ClipboardBackend) -> Result<(), String> {
    match backend {
        ClipboardBackend::WlClipboard => {
            let image_data =
                fs::read(image_path).map_err(|e| format!("Failed to read image: {}", e))?;

            let mime_type = match image_path.extension().and_then(|s| s.to_str()) {
                Some("png") => "image/png",
                Some("jpg") | Some("jpeg") => "image/jpeg",
                Some("bmp") => "image/bmp",
                _ => "image/png",
            };

            let mut child = Command::new("wl-copy")
                .arg("--type")
                .arg(mime_type)
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| format!("Failed to spawn wl-copy: {}", e))?;

            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin
                    .write_all(&image_data)
                    .map_err(|e| format!("Failed to write to wl-copy: {}", e))?;
            }

            child.wait().map_err(|e| format!("wl-copy failed: {}", e))?;

            Ok(())
        }
        ClipboardBackend::Arboard => {
            use image::ImageReader;

            let img = ImageReader::open(image_path)
                .map_err(|e| format!("Failed to open image: {}", e))?
                .decode()
                .map_err(|e| format!("Failed to decode image: {}", e))?;

            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();

            let img_data = arboard::ImageData {
                width: width as usize,
                height: height as usize,
                bytes: rgba.into_raw().into(),
            };

            Clipboard::new()
                .and_then(|mut cb| cb.set_image(img_data))
                .map_err(|e| format!("Failed to set image: {}", e))
        }
    }
}

// ============================================================================
// CLIPBOARD HISTORY MANAGER
// ============================================================================

struct ClipboardHistory {
    entries: Arc<Mutex<VecDeque<ClipboardEntry>>>,
    data_dir: PathBuf,
    images_dir: PathBuf,
    last_modified: Arc<Mutex<Option<SystemTime>>>,
}

impl ClipboardHistory {
    fn new() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("clipboard-manager");

        let images_dir = data_dir.join(IMAGES_DIR);

        fs::create_dir_all(&data_dir).ok();
        fs::create_dir_all(&images_dir).ok();

        let mut history = Self {
            entries: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_HISTORY))),
            data_dir,
            images_dir,
            last_modified: Arc::new(Mutex::new(None)),
        };

        history.load();
        history
    }

    fn check_and_reload(&self) {
        let history_path = self.data_dir.join(HISTORY_FILE);

        if let Ok(metadata) = fs::metadata(&history_path) {
            if let Ok(modified) = metadata.modified() {
                let last_mod = self.last_modified.lock().unwrap();

                // If file was modified externally, reload it
                if last_mod.map_or(true, |last| modified > last) {
                    drop(last_mod); // Release lock before loading

                    if let Ok(json) = fs::read_to_string(&history_path) {
                        if let Ok(mut loaded_entries) =
                            serde_json::from_str::<VecDeque<ClipboardEntry>>(&json)
                        {
                            // Recompute hashes for loaded entries
                            for entry in loaded_entries.iter_mut() {
                                entry.compute_hash();
                            }

                            let mut entries = self.entries.lock().unwrap();
                            *entries = loaded_entries;

                            // Update last modified time
                            let mut last_mod = self.last_modified.lock().unwrap();
                            *last_mod = Some(modified);

                            println!("↻ Reloaded history from disk ({} items)", entries.len());
                        }
                    }
                }
            }
        }
    }

    fn add_text(&self, content: String) {
        if content.trim().is_empty() {
            return;
        }

        // Check if file was modified externally before adding
        self.check_and_reload();

        let entry = ClipboardEntry::new_text(content.clone());
        let mut entries = self.entries.lock().unwrap();

        // Skip duplicates using hash comparison
        if entries.iter().any(|e| e.content_hash == entry.content_hash) {
            return;
        }

        entries.push_front(entry);

        // Remove old entries
        while entries.len() > MAX_HISTORY {
            if let Some(old_entry) = entries.pop_back() {
                if old_entry.content_type == ClipboardContentType::Image {
                    let _ = fs::remove_file(self.images_dir.join(&old_entry.content));
                }
            }
        }

        drop(entries);
        println!(
            "✓ Added text ({} chars) - Total: {}",
            content.len(),
            self.entries.lock().unwrap().len()
        );
        self.save();
    }

    fn add_image(&self, image_data: Vec<u8>) -> Result<(), String> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Check if file was modified externally before adding
        self.check_and_reload();

        let mut hasher = DefaultHasher::new();
        image_data.hash(&mut hasher);
        let hash = hasher.finish();

        let mut entries = self.entries.lock().unwrap();

        // Skip duplicate images
        if entries.iter().any(|e| e.content_hash == hash) {
            return Ok(());
        }

        let timestamp = chrono::Utc::now().timestamp();
        let filename = format!("img_{}.png", timestamp);
        let image_path = self.images_dir.join(&filename);

        fs::write(&image_path, &image_data).map_err(|e| format!("Failed to save image: {}", e))?;

        let img = image::load_from_memory(&image_data)
            .map_err(|e| format!("Failed to load image: {}", e))?;

        let info = ImageInfo {
            width: img.width(),
            height: img.height(),
            size_bytes: image_data.len() as u64,
        };

        println!(
            "✓ Added image {}×{} ({}) - Total: {}",
            info.width,
            info.height,
            format_size(info.size_bytes),
            entries.len() + 1
        );

        let entry = ClipboardEntry::new_image(filename, info, hash);
        entries.push_front(entry);

        while entries.len() > MAX_HISTORY {
            if let Some(old_entry) = entries.pop_back() {
                if old_entry.content_type == ClipboardContentType::Image {
                    let _ = fs::remove_file(self.images_dir.join(&old_entry.content));
                }
            }
        }

        drop(entries);
        self.save();
        Ok(())
    }

    fn get_all(&self) -> Vec<ClipboardEntry> {
        self.entries.lock().unwrap().iter().cloned().collect()
    }

    fn clear(&self) {
        let mut entries = self.entries.lock().unwrap();

        // Remove all image files
        for entry in entries.iter() {
            if entry.content_type == ClipboardContentType::Image {
                let _ = fs::remove_file(self.images_dir.join(&entry.content));
            }
        }

        entries.clear();
        drop(entries);
        println!("✓ Cleared all history");
        self.save();
    }

    fn save(&self) {
        let entries = self.entries.lock().unwrap();
        let history_path = self.data_dir.join(HISTORY_FILE);

        if let Ok(json) = serde_json::to_string(&*entries) {
            if fs::write(&history_path, json).is_ok() {
                // Update last modified time after successful save
                if let Ok(metadata) = fs::metadata(&history_path) {
                    if let Ok(modified) = metadata.modified() {
                        let mut last_mod = self.last_modified.lock().unwrap();
                        *last_mod = Some(modified);
                    }
                }
            }
        }
    }

    fn load(&mut self) {
        let history_path = self.data_dir.join(HISTORY_FILE);

        if let Ok(json) = fs::read_to_string(&history_path) {
            if let Ok(mut loaded_entries) = serde_json::from_str::<VecDeque<ClipboardEntry>>(&json)
            {
                // Recompute hashes for loaded entries
                for entry in loaded_entries.iter_mut() {
                    entry.compute_hash();
                }
                *self.entries.lock().unwrap() = loaded_entries;

                // Set initial last modified time
                if let Ok(metadata) = fs::metadata(&history_path) {
                    if let Ok(modified) = metadata.modified() {
                        *self.last_modified.lock().unwrap() = Some(modified);
                    }
                }
            }
        }
    }

    fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    fn images_dir(&self) -> &PathBuf {
        &self.images_dir
    }
}

// ============================================================================
// PID FILE MANAGEMENT
// ============================================================================

fn write_pid_file(data_dir: &PathBuf) -> Result<(), std::io::Error> {
    let pid_path = data_dir.join(PID_FILE);
    fs::write(pid_path, std::process::id().to_string())
}

fn remove_pid_file(data_dir: &PathBuf) {
    let _ = fs::remove_file(data_dir.join(PID_FILE));
}

fn get_trigger_script_path(data_dir: &PathBuf) -> PathBuf {
    data_dir.join("trigger.sh")
}

fn create_trigger_script(data_dir: &PathBuf, binary_path: &str) -> Result<(), std::io::Error> {
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
// CLIPBOARD MONITORING
// ============================================================================

fn start_clipboard_monitor(history: Arc<ClipboardHistory>, backend: ClipboardBackend) {
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

// ============================================================================
// SIGNAL LISTENER
// ============================================================================

fn start_signal_listener(shutdown_trigger: Arc<AtomicBool>) {
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
// TERMINAL UI
// ============================================================================

struct AppState {
    list_state: ListState,
    should_quit: bool,
    selected_index: Option<usize>,
    show_clear_confirm: bool,
}

impl AppState {
    fn new() -> Self {
        let mut state = Self {
            list_state: ListState::default(),
            should_quit: false,
            selected_index: None,
            show_clear_confirm: false,
        };
        state.list_state.select(Some(0));
        state
    }

    fn next(&mut self, max: usize) {
        if max == 0 {
            return;
        }
        let i = self
            .list_state
            .selected()
            .map(|i| if i >= max - 1 { 0 } else { i + 1 })
            .unwrap_or(0);
        self.list_state.select(Some(i));
    }

    fn previous(&mut self, max: usize) {
        if max == 0 {
            return;
        }
        let i = self
            .list_state
            .selected()
            .map(|i| if i == 0 { max - 1 } else { i - 1 })
            .unwrap_or(0);
        self.list_state.select(Some(i));
    }

    fn select(&mut self) {
        self.selected_index = self.list_state.selected();
        self.should_quit = true;
    }

    fn quit(&mut self) {
        self.should_quit = true;
    }
}

fn show_ui(backend: ClipboardBackend) -> Result<(), Box<dyn std::error::Error>> {
    let history = ClipboardHistory::new();

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend_term = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend_term)?;
    terminal.clear()?;

    let mut app_state = AppState::new();

    loop {
        let entries = history.get_all();

        terminal.draw(|f| {
            if app_state.show_clear_confirm {
                // Clear confirmation dialog
                let area = f.area();
                let text = Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "⚠️  Clear All History?",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "This will permanently delete all clipboard entries and images.",
                        Style::default().fg(Color::White),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Press Y to confirm • N or Esc to cancel",
                        Style::default().fg(Color::DarkGray),
                    )),
                ])
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Red)),
                );

                let centered = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Percentage(35),
                        Constraint::Length(9),
                        Constraint::Percentage(35),
                    ])
                    .split(area);

                let h_centered = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(20),
                        Constraint::Percentage(60),
                        Constraint::Percentage(20),
                    ])
                    .split(centered[1]);

                f.render_widget(text, h_centered[1]);
            } else if entries.is_empty() {
                // Empty state
                let area = f.area();
                let text = Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "Clipboard History Empty",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Copy text or images to start",
                        Style::default().fg(Color::Gray),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Press Esc to close",
                        Style::default().fg(Color::DarkGray),
                    )),
                ])
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)),
                );

                let centered = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Percentage(40),
                        Constraint::Length(9),
                        Constraint::Percentage(40),
                    ])
                    .split(area);

                f.render_widget(text, centered[1]);
            } else {
                // Main UI
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(2)])
                    .split(f.area());

                let items: Vec<ListItem> = entries
                    .iter()
                    .map(|entry| {
                        let color = match entry.content_type {
                            ClipboardContentType::Text => Color::White,
                            ClipboardContentType::Image => Color::Cyan,
                        };

                        ListItem::new(Line::from(vec![
                            Span::styled(format!(" {} ", entry.icon()), Style::default().fg(color)),
                            Span::styled(entry.display_content(), Style::default().fg(color)),
                            Span::styled(
                                format!(" {}", entry.formatted_time()),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]))
                    })
                    .collect();

                let list = List::new(items)
                    .block(
                        Block::default()
                            .title(format!(" Clipboard ({}) ", entries.len()))
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Cyan)),
                    )
                    .highlight_style(
                        Style::default()
                            .bg(Color::Blue)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("▶ ");

                f.render_stateful_widget(list, chunks[0], &mut app_state.list_state);

                let footer =
                    Paragraph::new("↑↓: Navigate  │  Enter: Copy  │  C: Clear All  │  Esc: Close")
                        .style(Style::default().fg(Color::DarkGray))
                        .alignment(Alignment::Center);

                f.render_widget(footer, chunks[1]);
            }
        })?;

        if event::poll(Duration::from_millis(50))? {
            if let CrosstermEvent::Key(KeyEvent { code, .. }) = event::read()? {
                if app_state.show_clear_confirm {
                    match code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            history.clear();
                            app_state.show_clear_confirm = false;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            app_state.show_clear_confirm = false;
                        }
                        _ => {}
                    }
                } else {
                    let entries_len = entries.len();
                    match code {
                        KeyCode::Char('q') | KeyCode::Esc => app_state.quit(),
                        KeyCode::Char('c') | KeyCode::Char('C') if entries_len > 0 => {
                            app_state.show_clear_confirm = true;
                        }
                        KeyCode::Down | KeyCode::Char('j') => app_state.next(entries_len),
                        KeyCode::Up | KeyCode::Char('k') => app_state.previous(entries_len),
                        KeyCode::Enter if entries_len > 0 => app_state.select(),
                        _ => {}
                    }
                }
            }
        }

        if app_state.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Some(index) = app_state.selected_index {
        let entries = history.get_all();
        if let Some(entry) = entries.get(index) {
            match entry.content_type {
                ClipboardContentType::Text => {
                    if set_clipboard_text(&entry.content, backend).is_ok() {
                        println!("✓ Copied to clipboard");
                    }
                }
                ClipboardContentType::Image => {
                    let image_path = history.images_dir().join(&entry.content);
                    if set_clipboard_image(&image_path, backend).is_ok() {
                        println!("✓ Copied image to clipboard");
                    }
                }
            }
        }
    }

    Ok(())
}

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
    println!("║     Clipboard Manager - Daemon Mode   ║");
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
    println!(
        "✓ Trigger: {}\n",
        get_trigger_script_path(&data_dir).display()
    );
    println!("Hyprland Config:");
    println!(
        "  bind = SUPER, V, exec, {}",
        get_trigger_script_path(&data_dir).display()
    );
    println!("  windowrulev2 = float, class:(floating-clipboard)");
    println!("  windowrulev2 = size 900 600, class:(floating-clipboard)");
    println!("  windowrulev2 = center, class:(floating-clipboard)\n");

    while !shutdown_trigger.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(100));
    }

    println!("\nShutting down...");
    history.save();
    remove_pid_file(&data_dir);
}
