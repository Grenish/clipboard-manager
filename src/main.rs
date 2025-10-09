use std::collections::VecDeque;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

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

// ============================================================================
// DATA STRUCTURES
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum ClipboardContentType {
    Text,
    Image,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct ClipboardEntry {
    content_type: ClipboardContentType,
    content: String, // For text: the actual text. For images: filename
    timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_info: Option<ImageInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct ImageInfo {
    width: u32,
    height: u32,
    size_bytes: u64,
    format: String,
}

impl ClipboardEntry {
    fn new_text(content: String) -> Self {
        Self {
            content_type: ClipboardContentType::Text,
            content,
            timestamp: chrono::Utc::now().timestamp(),
            image_info: None,
        }
    }

    fn new_image(filename: String, info: ImageInfo) -> Self {
        Self {
            content_type: ClipboardContentType::Image,
            content: filename,
            timestamp: chrono::Utc::now().timestamp(),
            image_info: Some(info),
        }
    }

    fn formatted_time(&self) -> String {
        chrono::DateTime::from_timestamp(self.timestamp, 0)
            .map(|dt| dt.format("%H:%M:%S").to_string())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    fn display_content(&self, max_len: usize) -> String {
        match self.content_type {
            ClipboardContentType::Text => {
                let content = self.content.replace('\n', " ").replace('\t', " ");
                let content = content.trim();
                if content.len() > max_len {
                    format!("{}...", &content[..max_len])
                } else {
                    content.to_string()
                }
            }
            ClipboardContentType::Image => {
                if let Some(info) = &self.image_info {
                    format!(
                        "🖼️  Image {}x{} ({}) - {}",
                        info.width,
                        info.height,
                        info.format,
                        format_size(info.size_bytes)
                    )
                } else {
                    "🖼️  Image".to_string()
                }
            }
        }
    }

    fn icon(&self) -> &'static str {
        match self.content_type {
            ClipboardContentType::Text => "📝",
            ClipboardContentType::Image => "🖼️ ",
        }
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
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
    if env::var("WAYLAND_DISPLAY").is_ok()
        || env::var("XDG_SESSION_TYPE")
            .map(|v| v == "wayland")
            .unwrap_or(false)
    {
        if Command::new("wl-paste").arg("--version").output().is_ok() {
            println!("✓ Detected Wayland with wl-clipboard tools");
            return ClipboardBackend::WlClipboard;
        }
    }

    println!("✓ Using arboard clipboard backend");
    ClipboardBackend::Arboard
}

// ============================================================================
// CLIPBOARD OPERATIONS
// ============================================================================

fn get_clipboard_types(backend: ClipboardBackend) -> Vec<String> {
    match backend {
        ClipboardBackend::WlClipboard => {
            match Command::new("wl-paste").arg("--list-types").output() {
                Ok(output) => {
                    if output.status.success() {
                        String::from_utf8_lossy(&output.stdout)
                            .lines()
                            .map(|s| s.to_string())
                            .collect()
                    } else {
                        Vec::new()
                    }
                }
                Err(_) => Vec::new(),
            }
        }
        ClipboardBackend::Arboard => Vec::new(),
    }
}

fn get_clipboard_text(backend: ClipboardBackend) -> Result<String, String> {
    match backend {
        ClipboardBackend::WlClipboard => {
            match Command::new("wl-paste").arg("--no-newline").output() {
                Ok(output) => {
                    if output.status.success() {
                        String::from_utf8(output.stdout)
                            .map_err(|e| format!("Invalid UTF-8: {}", e))
                    } else {
                        Err("Clipboard empty or unavailable".to_string())
                    }
                }
                Err(e) => Err(format!("Failed to run wl-paste: {}", e)),
            }
        }
        ClipboardBackend::Arboard => Clipboard::new()
            .map_err(|e| format!("Failed to create clipboard: {}", e))
            .and_then(|mut cb| {
                cb.get_text()
                    .map_err(|e| format!("Failed to get text: {}", e))
            }),
    }
}

fn get_clipboard_image(backend: ClipboardBackend) -> Result<Vec<u8>, String> {
    match backend {
        ClipboardBackend::WlClipboard => {
            // Try different image formats
            for mime_type in &["image/png", "image/jpeg", "image/jpg", "image/bmp"] {
                match Command::new("wl-paste")
                    .arg("--type")
                    .arg(mime_type)
                    .output()
                {
                    Ok(output) => {
                        if output.status.success() && !output.stdout.is_empty() {
                            return Ok(output.stdout);
                        }
                    }
                    Err(_) => continue,
                }
            }
            Err("No image in clipboard".to_string())
        }
        ClipboardBackend::Arboard => {
            Clipboard::new()
                .map_err(|e| format!("Failed to create clipboard: {}", e))
                .and_then(|mut cb| {
                    cb.get_image()
                        .map_err(|_| "No image in clipboard".to_string())
                        .and_then(|img| {
                            // Convert arboard ImageData to PNG bytes
                            use image::{ImageBuffer, RgbaImage};

                            let rgba = img.bytes.to_vec();
                            let width = img.width;
                            let height = img.height;

                            let img_buffer: RgbaImage =
                                ImageBuffer::from_raw(width as u32, height as u32, rgba)
                                    .ok_or_else(|| "Failed to create image buffer".to_string())?;

                            // Encode to PNG using the new API
                            let mut png_data = Vec::new();
                            use std::io::Cursor;
                            img_buffer
                                .write_to(&mut Cursor::new(&mut png_data), image::ImageFormat::Png)
                                .map_err(|e| format!("Failed to encode PNG: {}", e))?;

                            Ok(png_data)
                        })
                })
        }
    }
}

fn set_clipboard_text(content: &str, backend: ClipboardBackend) -> Result<(), String> {
    match backend {
        ClipboardBackend::WlClipboard => {
            match Command::new("wl-copy").arg("--").arg(content).output() {
                Ok(output) => {
                    if output.status.success() {
                        Ok(())
                    } else {
                        Err(format!(
                            "wl-copy failed: {}",
                            String::from_utf8_lossy(&output.stderr)
                        ))
                    }
                }
                Err(e) => Err(format!("Failed to run wl-copy: {}", e)),
            }
        }
        ClipboardBackend::Arboard => Clipboard::new()
            .map_err(|e| format!("Failed to create clipboard: {}", e))
            .and_then(|mut cb| {
                cb.set_text(content)
                    .map_err(|e| format!("Failed to set text: {}", e))
            }),
    }
}

fn set_clipboard_image(image_path: &PathBuf, backend: ClipboardBackend) -> Result<(), String> {
    match backend {
        ClipboardBackend::WlClipboard => {
            let image_data =
                fs::read(image_path).map_err(|e| format!("Failed to read image file: {}", e))?;

            let mime_type = match image_path.extension().and_then(|s| s.to_str()) {
                Some("png") => "image/png",
                Some("jpg") | Some("jpeg") => "image/jpeg",
                Some("bmp") => "image/bmp",
                _ => "image/png",
            };

            match Command::new("wl-copy")
                .arg("--type")
                .arg(mime_type)
                .stdin(std::process::Stdio::piped())
                .spawn()
            {
                Ok(mut child) => {
                    use std::io::Write;
                    if let Some(mut stdin) = child.stdin.take() {
                        stdin
                            .write_all(&image_data)
                            .map_err(|e| format!("Failed to write to wl-copy: {}", e))?;
                    }
                    child
                        .wait()
                        .map_err(|e| format!("wl-copy process failed: {}", e))?;
                    Ok(())
                }
                Err(e) => Err(format!("Failed to spawn wl-copy: {}", e)),
            }
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
                .map_err(|e| format!("Failed to create clipboard: {}", e))
                .and_then(|mut cb| {
                    cb.set_image(img_data)
                        .map_err(|e| format!("Failed to set image: {}", e))
                })
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
}

impl ClipboardHistory {
    fn new() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("clipboard-manager");

        let images_dir = data_dir.join(IMAGES_DIR);

        if let Err(e) = fs::create_dir_all(&data_dir) {
            eprintln!("Warning: Could not create data directory: {}", e);
        }

        if let Err(e) = fs::create_dir_all(&images_dir) {
            eprintln!("Warning: Could not create images directory: {}", e);
        }

        let mut history = Self {
            entries: Arc::new(Mutex::new(VecDeque::new())),
            data_dir,
            images_dir,
        };

        history.load();
        history
    }

    fn add_text(&self, content: String) {
        if content.trim().is_empty() {
            return;
        }

        let entry = ClipboardEntry::new_text(content.clone());
        let mut entries = self.entries.lock().unwrap();

        if let Some(last) = entries.front() {
            if last.content_type == ClipboardContentType::Text && last.content == entry.content {
                return;
            }
        }

        entries.push_front(entry);
        let count = entries.len();

        while entries.len() > MAX_HISTORY {
            if let Some(old_entry) = entries.pop_back() {
                if old_entry.content_type == ClipboardContentType::Image {
                    let img_path = self.images_dir.join(&old_entry.content);
                    let _ = fs::remove_file(img_path);
                }
            }
        }

        drop(entries);

        println!(
            "✅ Added text to history (#{}) - {}",
            count,
            content.chars().take(60).collect::<String>()
        );

        self.save();
    }

    fn add_image(&self, image_data: Vec<u8>) -> Result<(), String> {
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
            format: "PNG".to_string(),
        };

        let entry = ClipboardEntry::new_image(filename.clone(), info.clone());
        let mut entries = self.entries.lock().unwrap();

        if let Some(last) = entries.front() {
            if last.content_type == ClipboardContentType::Image && last.content == entry.content {
                return Ok(());
            }
        }

        entries.push_front(entry);
        let count = entries.len();

        while entries.len() > MAX_HISTORY {
            if let Some(old_entry) = entries.pop_back() {
                if old_entry.content_type == ClipboardContentType::Image {
                    let img_path = self.images_dir.join(&old_entry.content);
                    let _ = fs::remove_file(img_path);
                }
            }
        }

        drop(entries);

        println!(
            "✅ Added image to history (#{}) - {}x{} ({})",
            count, info.width, info.height, info.format
        );

        self.save();
        Ok(())
    }

    fn get_all(&self) -> Vec<ClipboardEntry> {
        self.entries.lock().unwrap().iter().cloned().collect()
    }

    fn len(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    fn save(&self) {
        let entries = self.entries.lock().unwrap();
        let history_path = self.data_dir.join(HISTORY_FILE);

        match serde_json::to_string_pretty(&*entries) {
            Ok(json) => {
                if let Err(e) = fs::write(&history_path, json) {
                    eprintln!("Failed to save history: {}", e);
                }
            }
            Err(e) => eprintln!("Failed to serialize history: {}", e),
        }
    }

    fn load(&mut self) {
        let history_path = self.data_dir.join(HISTORY_FILE);

        match fs::read_to_string(&history_path) {
            Ok(json) => match serde_json::from_str::<VecDeque<ClipboardEntry>>(&json) {
                Ok(loaded_entries) => {
                    let count = loaded_entries.len();
                    *self.entries.lock().unwrap() = loaded_entries;
                    if count > 0 {
                        println!("✓ Loaded {} clipboard entries from history", count);
                    }
                }
                Err(e) => eprintln!("Failed to parse history file: {}", e),
            },
            Err(_) => {}
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
    let pid = std::process::id();
    fs::write(pid_path, pid.to_string())?;
    Ok(())
}

fn remove_pid_file(data_dir: &PathBuf) {
    let pid_path = data_dir.join(PID_FILE);
    let _ = fs::remove_file(pid_path);
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
elif command -v wezterm &> /dev/null; then
    wezterm start --class floating-clipboard \
                  -- "$BINARY" --ui &
else
    notify-send "Clipboard Manager" "No suitable terminal emulator found"
    exit 1
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
        println!("🔍 Initializing clipboard monitor...");
        println!("   Backend: {:?}", backend);

        thread::sleep(Duration::from_millis(100));

        let mut last_text_content = String::new();
        let mut last_image_hash: Option<u64> = None;
        let mut poll_count: u64 = 0;

        println!("📋 Clipboard monitor is now active!");
        println!("   Monitoring for text, emojis, and images...\n");

        loop {
            thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            poll_count += 1;

            if poll_count % 67 == 0 {
                println!("💓 Monitor alive - {} items in history", history.len());
            }

            let types = get_clipboard_types(backend);
            let has_image = types.iter().any(|t| t.starts_with("image/"));

            if has_image {
                match get_clipboard_image(backend) {
                    Ok(image_data) => {
                        use std::collections::hash_map::DefaultHasher;
                        use std::hash::{Hash, Hasher};
                        let mut hasher = DefaultHasher::new();
                        image_data.hash(&mut hasher);
                        let hash = hasher.finish();

                        if Some(hash) != last_image_hash {
                            println!("\n🎉 NEW IMAGE DETECTED!");
                            println!("   Size: {} bytes\n", image_data.len());

                            if let Err(e) = history.add_image(image_data) {
                                eprintln!("Failed to save image: {}", e);
                            }

                            last_image_hash = Some(hash);
                            last_text_content.clear();
                        }
                    }
                    Err(_) => {}
                }
            } else {
                match get_clipboard_text(backend) {
                    Ok(content) => {
                        if !content.is_empty() && content != last_text_content {
                            println!("\n🎉 NEW TEXT/EMOJI DETECTED!");
                            println!("   Length: {} chars", content.len());
                            println!(
                                "   Preview: {}\n",
                                content.chars().take(100).collect::<String>()
                            );

                            history.add_text(content.clone());
                            last_text_content = content;
                            last_image_hash = None;
                        }
                    }
                    Err(_) => {
                        if !last_text_content.is_empty() {
                            last_text_content.clear();
                        }
                    }
                }
            }
        }
    });
}

// ============================================================================
// SIGNAL LISTENER
// ============================================================================

fn start_signal_listener(ui_trigger: Arc<AtomicBool>, shutdown_trigger: Arc<AtomicBool>) {
    thread::spawn(move || {
        let mut signals = match Signals::new(&[SIGUSR1, SIGTERM, SIGINT]) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to register signal handlers: {}", e);
                return;
            }
        };

        for signal in signals.forever() {
            match signal {
                SIGUSR1 => {
                    ui_trigger.store(true, Ordering::Relaxed);
                }
                SIGTERM | SIGINT => {
                    shutdown_trigger.store(true, Ordering::Relaxed);
                    break;
                }
                _ => {}
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
}

impl AppState {
    fn new() -> Self {
        let mut state = Self {
            list_state: ListState::default(),
            should_quit: false,
            selected_index: None,
        };
        state.list_state.select(Some(0));
        state
    }

    fn next(&mut self, max: usize) {
        if max == 0 {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= max - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn previous(&mut self, max: usize) {
        if max == 0 {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    max - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
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

fn show_ui(backend: ClipboardBackend) -> Result<Option<usize>, Box<dyn std::error::Error>> {
    let history = ClipboardHistory::new();
    let entries = history.get_all();

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend_term = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend_term)?;
    terminal.clear()?;

    let mut app_state = AppState::new();

    if entries.is_empty() {
        terminal.draw(|f| {
            let area = f.area();
            let text = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "📋 Clipboard History is Empty!",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Copy some text, emojis, or images to get started.",
                    Style::default().fg(Color::White),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Press any key to close...",
                    Style::default().fg(Color::DarkGray),
                )),
            ])
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .title(" Clipboard Manager ")
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
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
        })?;

        event::read()?;
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        return Ok(None);
    }

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(3),
                ])
                .split(f.area());

            let header = Paragraph::new(format!(
                "📋 Clipboard Manager ({} items) - Text, Emojis & Images",
                entries.len()
            ))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            );
            f.render_widget(header, chunks[0]);

            let items: Vec<ListItem> = entries
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    let display_content = entry.display_content(75);
                    let timestamp = entry.formatted_time();
                    let icon = entry.icon();

                    let color = match entry.content_type {
                        ClipboardContentType::Text => Color::White,
                        ClipboardContentType::Image => Color::LightCyan,
                    };

                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("{:2}. ", i + 1),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(icon, Style::default().fg(color)),
                        Span::raw(" "),
                        Span::styled(display_content, Style::default().fg(color)),
                        Span::styled(
                            format!(" [{}]", timestamp),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]))
                })
                .collect();

            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::White)),
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::Blue)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("► ");

            f.render_stateful_widget(list, chunks[1], &mut app_state.list_state);

            let footer =
                Paragraph::new("↑/↓: Navigate  │  Enter: Copy to Clipboard  │  Esc/q: Cancel")
                    .alignment(Alignment::Center)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Green)),
                    )
                    .style(Style::default().fg(Color::Gray));
            f.render_widget(footer, chunks[2]);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let CrosstermEvent::Key(KeyEvent { code, .. }) = event::read()? {
                match code {
                    KeyCode::Char('q') | KeyCode::Esc => app_state.quit(),
                    KeyCode::Down | KeyCode::Char('j') => app_state.next(entries.len()),
                    KeyCode::Up | KeyCode::Char('k') => app_state.previous(entries.len()),
                    KeyCode::Enter => app_state.select(),
                    _ => {}
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
        if let Some(entry) = entries.get(index) {
            match entry.content_type {
                ClipboardContentType::Text => {
                    if let Err(e) = set_clipboard_text(&entry.content, backend) {
                        eprintln!("Failed to copy text: {}", e);
                    } else {
                        println!("✅ Copied text to clipboard: {}", entry.display_content(50));
                    }
                }
                ClipboardContentType::Image => {
                    let image_path = history.images_dir().join(&entry.content);
                    if let Err(e) = set_clipboard_image(&image_path, backend) {
                        eprintln!("Failed to copy image: {}", e);
                    } else {
                        println!("✅ Copied image to clipboard: {}", entry.content);
                    }
                }
            }
        }
    }

    Ok(app_state.selected_index)
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
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║      Clipboard Manager - Text, Emojis & Images           ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!();

    if matches!(backend, ClipboardBackend::WlClipboard) {
        println!("ℹ️  Using wl-clipboard backend");
    } else {
        println!("⚠️  Using arboard backend");
        println!("   For best results on Wayland: sudo pacman -S wl-clipboard");
        println!();
    }

    let history = Arc::new(ClipboardHistory::new());
    let data_dir = history.data_dir().clone();

    if let Err(e) = write_pid_file(&data_dir) {
        eprintln!("⚠️  Warning: Could not write PID file: {}", e);
    } else {
        println!("✓ PID file created");
    }

    let binary_path = env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "clipboard-manager".to_string());

    if let Err(e) = create_trigger_script(&data_dir, &binary_path) {
        eprintln!("⚠️  Warning: Could not create trigger script: {}", e);
    } else {
        println!("✓ Trigger script created");
    }

    let ui_trigger = Arc::new(AtomicBool::new(false));
    let shutdown_trigger = Arc::new(AtomicBool::new(false));

    start_signal_listener(Arc::clone(&ui_trigger), Arc::clone(&shutdown_trigger));
    start_clipboard_monitor(Arc::clone(&history), backend);

    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  Hyprland Config:");
    println!("═══════════════════════════════════════════════════════════");
    println!(
        "  bind = SUPER, comma, exec, {}",
        get_trigger_script_path(&data_dir).display()
    );
    println!("  windowrulev2 = float, class:(floating-clipboard)");
    println!("  windowrulev2 = size 900 600, class:(floating-clipboard)");
    println!("  windowrulev2 = center, class:(floating-clipboard)");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    println!("✨ Features: Text, Emojis (😀🎉🚀), and Images!");
    println!();

    loop {
        thread::sleep(Duration::from_millis(100));

        if shutdown_trigger.load(Ordering::Relaxed) {
            break;
        }
    }

    println!("\n\nShutting down...");
    history.save();
    remove_pid_file(&data_dir);
    println!("✅ Goodbye!");
}
