use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::utils::format_size;

// ============================================================================
// DATA STRUCTURES
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipboardContentType {
    Text,
    Image,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub content_type: ClipboardContentType,
    pub content: String,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_info: Option<ImageInfo>,
    #[serde(skip)]
    pub content_hash: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
}

impl ClipboardEntry {
    pub fn new_text(content: String) -> Self {
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

    pub fn new_image(filename: String, info: ImageInfo, hash: u64) -> Self {
        Self {
            content_type: ClipboardContentType::Image,
            content: filename,
            timestamp: chrono::Utc::now().timestamp(),
            image_info: Some(info),
            content_hash: hash,
        }
    }

    pub fn compute_hash(&mut self) {
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





    pub fn metadata_label(&self) -> String {
        match self.content_type {
            ClipboardContentType::Text => format!("{} char", self.content.len()),
            ClipboardContentType::Image => {
                if let Some(info) = &self.image_info {
                    format_size(info.size_bytes)
                } else {
                    String::from("Unknown size")
                }
            }
        }
    }

    pub fn preview_lines(&self) -> Vec<String> {
        match self.content_type {
            ClipboardContentType::Text => {
                // Normalize text: replace newlines/tabs with spaces to treat as continuous flow
                let clean_text = self.content.replace(['\n', '\t'], " ");
                let words: Vec<&str> = clean_text.split_whitespace().collect();
                
                let mut lines: Vec<String> = Vec::new();
                let mut current_line = String::new();
                let max_width = 85; // Approximate width to fit comfortably in the box
                
                for word in words {
                    if lines.len() >= 2 {
                        // If we are about to start a 3rd line, stick "..." at end of 2nd and stop
                        let last_idx = lines.len() - 1;
                        if lines[last_idx].len() + 3 <= max_width {
                           lines[last_idx].push_str("...");
                        }
                        break;
                    }
                    
                    if current_line.len() + word.len() + 1 > max_width {
                        if !current_line.is_empty() {
                            lines.push(current_line);
                            current_line = String::new();
                        }
                    }
                    
                    if !current_line.is_empty() {
                        current_line.push(' ');
                    }
                    current_line.push_str(word);
                }
                
                if lines.len() < 2 && !current_line.is_empty() {
                    lines.push(current_line);
                }

                lines
            }
            ClipboardContentType::Image => {
                if let Some(info) = &self.image_info {
                    vec![format!("Image {}×{}", info.width, info.height)]
                } else {
                    vec![String::from("Image")]
                }
            }
        }
    }
}
