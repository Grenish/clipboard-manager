use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};


use crate::models::{ClipboardContentType, ClipboardEntry, ImageInfo};
use crate::utils::{HISTORY_FILE, IMAGES_DIR, MAX_HISTORY, format_size};

// ============================================================================
// CLIPBOARD HISTORY MANAGER
// ============================================================================

pub struct ClipboardHistory {
    entries: Arc<Mutex<VecDeque<ClipboardEntry>>>,
    data_dir: PathBuf,
    images_dir: PathBuf,
    // We don't cache last_modified for reloading in this optimized version yet
    // because JSONL tailing is complex. We assume singleton daemon.
}

impl ClipboardHistory {
    pub fn new() -> Self {
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
        };

        history.load();
        history
    }

    pub fn add_text(&self, content: String) {
        if content.trim().is_empty() {
            return;
        }

        let entry = ClipboardEntry::new_text(content.clone());
        let mut entries = self.entries.lock().unwrap();

        // Skip duplicates using hash comparison
        if entries.iter().any(|e| e.content_hash == entry.content_hash) {
            return;
        }

        entries.push_front(entry.clone());

        // Remove old entries from memory
        self.cleanup_old_entries(&mut entries);

        drop(entries); // unlock before I/O
        
        println!("✓ Added text ({} chars)", content.len());
        self.append_entry(&entry);
    }

    pub fn add_image(&self, image_data: Vec<u8>) -> Result<(), String> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

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

        let entry = ClipboardEntry::new_image(filename, info, hash);
        
        println!(
            "✓ Added image {}×{} ({})",
            entry.image_info.as_ref().unwrap().width,
            entry.image_info.as_ref().unwrap().height,
            format_size(entry.image_info.as_ref().unwrap().size_bytes)
        );

        entries.push_front(entry.clone());
        self.cleanup_old_entries(&mut entries);
        
        drop(entries);
        self.append_entry(&entry);
        Ok(())
    }
    
    fn cleanup_old_entries(&self, entries: &mut VecDeque<ClipboardEntry>) {
        while entries.len() > MAX_HISTORY {
            if let Some(old_entry) = entries.pop_back() {
                if old_entry.content_type == ClipboardContentType::Image {
                    let _ = fs::remove_file(self.images_dir.join(&old_entry.content));
                }
            }
        }
    }

    pub fn get_all(&self) -> Vec<ClipboardEntry> {
        self.entries.lock().unwrap().iter().cloned().collect()
    }

    pub fn clear(&self) {
        let mut entries = self.entries.lock().unwrap();

        // Remove all image files
        for entry in entries.iter() {
            if entry.content_type == ClipboardContentType::Image {
                let _ = fs::remove_file(self.images_dir.join(&entry.content));
            }
        }

        entries.clear();
        drop(entries);
        
        // Truncate file
        let history_path = self.data_dir.join(HISTORY_FILE);
        let _ = fs::File::create(history_path); // Create truncates
        
        println!("✓ Cleared all history");
    }

    fn append_entry(&self, entry: &ClipboardEntry) {
        let history_path = self.data_dir.join(HISTORY_FILE);
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(history_path) {
             if let Ok(json) = serde_json::to_string(entry) {
                 let _ = writeln!(file, "{}", json);
             }
        }
    }
    
    // Legacy save method is removed in favor of append_entry, but we might need 'compact' later.
    // For now we just load and keep memory consistent. 
    // Note: If we restart, we load all lines. If file is huge, this is slow. 
    // TODO: Implement compaction on startup.

    fn load(&mut self) {
        let history_path = self.data_dir.join(HISTORY_FILE);
        let mut loaded_entries = VecDeque::new();

        if let Ok(file) = fs::File::open(&history_path) {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if let Ok(mut entry) = serde_json::from_str::<ClipboardEntry>(&line) {
                         entry.compute_hash();
                         loaded_entries.push_front(entry);
                    }
                }
            }
        }
        
        // Truncate memory if too many (and maybe file? For now just memory)
        while loaded_entries.len() > MAX_HISTORY {
             loaded_entries.pop_back();
        }

        *self.entries.lock().unwrap() = loaded_entries;
        println!("↻ Loaded history from disk");
    }

    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    pub fn images_dir(&self) -> &PathBuf {
        &self.images_dir
    }
}

