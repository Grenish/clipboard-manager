use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::models::{ClipboardContentType, ClipboardEntry, ImageInfo};
use crate::utils::{HISTORY_FILE, IMAGES_DIR, MAX_HISTORY, format_size};
use chrono::Utc;

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

        let history = Self {
            entries: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_HISTORY))),
            data_dir,
            images_dir,
        };

        history.reload();
        history
    }

    /// Reload entries from disk to pick up changes made by other processes (e.g., TUI pinning an entry while daemon is running).
    pub fn reload(&self) {
        let history_path = self.data_dir.join(HISTORY_FILE);
        let mut loaded_entries: VecDeque<ClipboardEntry> = VecDeque::new();

        if let Ok(file) = fs::File::open(&history_path) {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if let Ok(mut entry) = serde_json::from_str::<ClipboardEntry>(&line) {
                        entry.compute_hash();

                        if let Some(pos) = loaded_entries
                            .iter()
                            .position(|e| e.content_hash == entry.content_hash)
                        {
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

        // Remove any expired secrets
        self.cleanup_expired();
    }

    pub fn add_text(&self, content: String) {
        let trimmed_content = content.trim().to_string();
        if trimmed_content.is_empty() {
            return;
        }

        // Reload from disk to pick up any changes made by TUI (e.g., pins)
        self.reload();

        let entry = ClipboardEntry::new_text(trimmed_content.clone());
        let mut entries = self.entries.lock().unwrap();

        // Check for duplicate and remove if exists (move to top behavior)
        let mut rewrite = false;
        if let Some(pos) = entries
            .iter()
            .position(|e| e.content_hash == entry.content_hash)
        {
            entries.remove(pos);
            rewrite = true;
            // println!("  ↻ Moving duplicate text to top");
        }

        entries.push_front(entry.clone());

        // Remove old entries from memory
        rewrite |= self.cleanup_old_entries(&mut entries);

        drop(entries); // unlock before I/O

        let flag_path = self.data_dir.join(".pasting");
        let is_pasting = if flag_path.exists() {
            let _ = std::fs::remove_file(&flag_path);
            true
        } else {
            false
        };
        let action = if is_pasting { "Pasted" } else { "Copied" };
        println!("✓ {} text ({} chars)", action, trimmed_content.len());

        if rewrite {
            self.rewrite_history();
        } else {
            self.append_entry(&entry);
        }
    }

    pub fn add_image(&self, image_data: Vec<u8>) -> Result<(), String> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        image_data.hash(&mut hasher);
        let hash = hasher.finish();

        // Reload from disk to pick up any changes made by TUI (e.g., pins)
        self.reload();

        let mut entries = self.entries.lock().unwrap();

        let mut removed_existing = false;
        // Check for duplicate images (move to top)
        if let Some(pos) = entries.iter().position(|e| e.content_hash == hash) {
            let existing_entry = entries.remove(pos).unwrap();

            // Update timestamp to now so it appears as new
            // Note: We don't change the filename/content, just the metadata timestamp if possible.
            // Since we append to file, we should probably append this 'renewed' entry.
            // Ideally we'd update the timestamp in the struct, assuming it has one.
            // If not, we just move it.

            entries.push_front(existing_entry.clone());
            removed_existing = true;

            println!("✓ Moved existing image to top");
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

        let flag_path = self.data_dir.join(".pasting");
        let is_pasting = if flag_path.exists() {
            let _ = std::fs::remove_file(&flag_path);
            true
        } else {
            false
        };
        let action = if is_pasting { "Pasted" } else { "Copied" };

        println!(
            "✓ {} image {}×{} ({})",
            action,
            entry.image_info.as_ref().unwrap().width,
            entry.image_info.as_ref().unwrap().height,
            format_size(entry.image_info.as_ref().unwrap().size_bytes)
        );

        if !removed_existing {
            entries.push_front(entry.clone());
        }

        let rewrite = removed_existing || self.cleanup_old_entries(&mut entries);

        drop(entries);

        if rewrite {
            self.rewrite_history();
        } else {
            self.append_entry(&entry);
        }
        Ok(())
    }

    fn cleanup_old_entries(&self, entries: &mut VecDeque<ClipboardEntry>) -> bool {
        let mut cleaned = false;
        // Count only unpinned entries against MAX_HISTORY
        let unpinned_count = entries.iter().filter(|e| !e.pinned).count();
        if unpinned_count <= MAX_HISTORY {
            return false;
        }
        let mut to_remove = unpinned_count - MAX_HISTORY;
        // Remove oldest unpinned entries (from the back)
        while to_remove > 0 {
            // Find the last unpinned entry
            if let Some(pos) = entries.iter().rposition(|e| !e.pinned) {
                let old_entry = entries.remove(pos).unwrap();
                cleaned = true;
                if old_entry.content_type == ClipboardContentType::Image {
                    let _ = fs::remove_file(self.images_dir.join(&old_entry.content));
                }
                to_remove -= 1;
            } else {
                break;
            }
        }
        cleaned
    }

    pub fn get_all(&self) -> Vec<ClipboardEntry> {
        let entries = self.entries.lock().unwrap();
        let mut result: Vec<ClipboardEntry> = entries.iter().cloned().collect();
        // Stable sort: pinned items float to the top, preserving relative order within each group
        result.sort_by(|a, b| b.pinned.cmp(&a.pinned));
        result
    }

    /// Remove entries whose secret expiry has passed.
    /// Called automatically during reload() and can be called periodically.
    pub fn cleanup_expired(&self) {
        let mut entries = self.entries.lock().unwrap();
        let now = Utc::now().timestamp();

        let had_expired = entries.iter().any(|e| {
            if let Some(ref info) = e.secret_info {
                if let Some(expires_at) = info.expires_at {
                    return now >= expires_at;
                }
            }
            false
        });

        if !had_expired {
            return;
        }

        // Collect image filenames of expired entries before removing
        let expired_images: Vec<String> = entries
            .iter()
            .filter(|e| {
                if let Some(ref info) = e.secret_info {
                    if let Some(expires_at) = info.expires_at {
                        return now >= expires_at;
                    }
                }
                false
            })
            .filter(|e| e.content_type == ClipboardContentType::Image)
            .map(|e| e.content.clone())
            .collect();

        // Remove expired entries
        entries.retain(|e| {
            if let Some(ref info) = e.secret_info {
                if let Some(expires_at) = info.expires_at {
                    return now < expires_at;
                }
            }
            true
        });

        // Clean up image files
        for filename in &expired_images {
            let _ = std::fs::remove_file(self.images_dir.join(filename));
        }

        drop(entries);

        if had_expired {
            self.rewrite_history();
            println!("✓ Cleaned up expired secrets");
        }
    }

    /// Stop the auto-expiry timer on a secret entry (makes it permanent).
    /// `index` is the position in the sorted (pinned-first) view returned by get_all().
    pub fn stop_expiry(&self, index: usize) {
        self.reload();
        let sorted = self.get_all();
        if index >= sorted.len() {
            return;
        }
        let target_hash = sorted[index].content_hash;

        let mut entries = self.entries.lock().unwrap();
        if let Some(entry) = entries.iter_mut().find(|e| e.content_hash == target_hash) {
            if let Some(ref mut info) = entry.secret_info {
                info.expires_at = None;
            }
        }
        drop(entries);
        self.rewrite_history();
    }

    pub fn toggle_pin(&self, index: usize) {
        // Reload from disk to ensure we have the latest state
        self.reload();
        // `index` is the position in the sorted (pinned-first) view returned by get_all().
        // We need to find the matching entry in the internal deque by content_hash.
        let sorted = self.get_all();
        if index >= sorted.len() {
            return;
        }
        let target_hash = sorted[index].content_hash;

        let mut entries = self.entries.lock().unwrap();
        if let Some(entry) = entries.iter_mut().find(|e| e.content_hash == target_hash) {
            entry.pinned = !entry.pinned;
        }
        drop(entries);
        self.rewrite_history();
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
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(history_path)
        {
            if let Ok(json) = serde_json::to_string(entry) {
                let _ = writeln!(file, "{}", json);
            }
        }
    }

    // Helper to delete specific entry (used by UI)
    // `index` is the position in the sorted (pinned-first) view returned by get_all().
    pub fn delete_entry(&self, index: usize) {
        // Reload from disk to ensure we have the latest state
        self.reload();
        let sorted = self.get_all();
        if index >= sorted.len() {
            return;
        }
        let target_hash = sorted[index].content_hash;

        let mut entries = self.entries.lock().unwrap();
        if let Some(pos) = entries.iter().position(|e| e.content_hash == target_hash) {
            if let Some(removed) = entries.remove(pos) {
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
