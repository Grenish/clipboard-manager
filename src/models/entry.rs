use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::utils::{MAX_DISPLAY_LENGTH, format_size};

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

    pub fn formatted_time(&self) -> String {
        chrono::DateTime::from_timestamp(self.timestamp, 0)
            .map(|dt| dt.format("%H:%M:%S").to_string())
            .unwrap_or_else(|| String::from("--:--:--"))
    }

    pub fn display_content(&self) -> String {
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

    pub fn icon(&self) -> &'static str {
        match self.content_type {
            ClipboardContentType::Text => "📝",
            ClipboardContentType::Image => "🖼️",
        }
    }
}
