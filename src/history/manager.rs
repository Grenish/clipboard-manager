use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::models::{ClipboardContentType, ClipboardEntry, ImageInfo};
use crate::utils::{HISTORY_FILE, IMAGES_DIR, MAX_HISTORY, format_size};

// ============================================================================
// CLIPBOARD HISTORY MANAGER
// ============================================================================

pub struct ClipboardHistory {
    entries: Arc<Mutex<VecDeque<ClipboardEntry>>>,
    data_dir: PathBuf,
    images_dir: PathBuf,
    last_modified: Arc<Mutex<Option<SystemTime>>>,
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
            last_modified: Arc::new(Mutex::new(None)),
        };

        history.load();
        history
    }

    pub fn check_and_reload(&self) {
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

    pub fn add_text(&self, content: String) {
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

    pub fn add_image(&self, image_data: Vec<u8>) -> Result<(), String> {
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
        println!("✓ Cleared all history");
        self.save();
    }

    pub fn save(&self) {
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

    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    pub fn images_dir(&self) -> &PathBuf {
        &self.images_dir
    }
}
