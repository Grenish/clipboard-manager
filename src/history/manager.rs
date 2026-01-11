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
        let trimmed_content = content.trim().to_string();
        if trimmed_content.is_empty() {
            return;
        }

        let entry = ClipboardEntry::new_text(trimmed_content.clone());
        let mut entries = self.entries.lock().unwrap();

        // Check for duplicate and remove if exists (move to top behavior)
        if let Some(pos) = entries.iter().position(|e| e.content_hash == entry.content_hash) {
            entries.remove(pos);
            // println!("  ↻ Moving duplicate text to top");
        }

        entries.push_front(entry.clone());

        // Remove old entries from memory
        self.cleanup_old_entries(&mut entries);

        drop(entries); // unlock before I/O
        
        println!("✓ Added text ({} chars)", trimmed_content.len());
        self.append_entry(&entry);
    }

    pub fn add_image(&self, image_data: Vec<u8>) -> Result<(), String> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        image_data.hash(&mut hasher);
        let hash = hasher.finish();

        let mut entries = self.entries.lock().unwrap();

        // Check for duplicate images (move to top)
        if let Some(pos) = entries.iter().position(|e| e.content_hash == hash) {
            let existing_entry = entries.remove(pos).unwrap();
            
            // Update timestamp to now so it appears as new
            // Note: We don't change the filename/content, just the metadata timestamp if possible.
            // Since we append to file, we should probably append this 'renewed' entry.
            // Ideally we'd update the timestamp in the struct, assuming it has one.
            // If not, we just move it.
            
            entries.push_front(existing_entry.clone());
            drop(entries);
            
            println!("✓ Moved existing image to top");
            self.append_entry(&existing_entry);
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

    fn load(&mut self) {
        let history_path = self.data_dir.join(HISTORY_FILE);
        let mut loaded_entries: VecDeque<ClipboardEntry> = VecDeque::new();

        if let Ok(file) = fs::File::open(&history_path) {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if let Ok(mut entry) = serde_json::from_str::<ClipboardEntry>(&line) {
                         entry.compute_hash();
                         
                         // Dedup during load: if exists, remove old one (because we read oldest -> newest)
                         // This ensures the list corresponds to "latest wins" logic
                         if let Some(pos) = loaded_entries.iter().position(|e| e.content_hash == entry.content_hash) {
                             loaded_entries.remove(pos);
                         }
                         
                         loaded_entries.push_front(entry);
                    }
                }
            }
        }
        
        while loaded_entries.len() > MAX_HISTORY {
             loaded_entries.pop_back();
        }

        *self.entries.lock().unwrap() = loaded_entries;
    }
    
    // Helper to delete specific entry (used by UI)
    pub fn delete_entry(&self, index: usize) {
        let mut entries = self.entries.lock().unwrap();
        
        if index < entries.len() {
            if let Some(removed) = entries.remove(index) {
                if removed.content_type == ClipboardContentType::Image {
                    let _ = fs::remove_file(self.images_dir.join(&removed.content));
                }
            }
        }
        
        drop(entries);
        // Rewriting the file is necessary when deleting from middle, sadly.
        // But deletes are rare compared to appends.
        self.rewrite_history();
    }
    
    fn rewrite_history(&self) {
        let entries = self.entries.lock().unwrap();
        let history_path = self.data_dir.join(HISTORY_FILE);
        if let Ok(mut file) = fs::File::create(&history_path) {
            // Write in reverse order (oldest to newest) or keep order?
            // load() reads line by line and pushes front... wait.
            // If we write current deque (newest first) to file, then load() reads first line (newest) and pushes front.
            // So deque becomes [newest, 2nd newest..].
            // If we append:
            // File: [Old1, Old2, New3]
            // Load: reads Old1 -> Entry is [Old1]. reads Old2 -> Entry is [Old2, Old1]. reads New3 -> Entry is [New3, Old2, Old1].
            // Correct.
            // So when rewriting, we should write from Oldest to Newest (back to front).
            for entry in entries.iter().rev() {
                 if let Ok(json) = serde_json::to_string(entry) {
                     let _ = writeln!(file, "{}", json);
                 }
            }
        }
    }

    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    pub fn images_dir(&self) -> &PathBuf {
        &self.images_dir
    }
}
