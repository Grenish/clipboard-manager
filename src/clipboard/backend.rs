use std::env;
use std::process::Command;

use arboard::Clipboard;

// ============================================================================
// CLIPBOARD BACKEND
// ============================================================================

#[derive(Debug, Clone, Copy)]
pub enum ClipboardBackend {
    WlClipboard,
    Arboard,
}

pub fn detect_clipboard_backend() -> ClipboardBackend {
    if (env::var("WAYLAND_DISPLAY").is_ok()
        || env::var("XDG_SESSION_TYPE").map_or(false, |v| v == "wayland"))
        && Command::new("wl-paste").arg("--version").output().is_ok()
    {
        ClipboardBackend::WlClipboard
    } else {
        ClipboardBackend::Arboard
    }
}

pub fn get_clipboard_types(backend: ClipboardBackend) -> Vec<String> {
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

pub fn get_clipboard_text(backend: ClipboardBackend) -> Option<String> {
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

pub fn get_clipboard_image(backend: ClipboardBackend) -> Option<Vec<u8>> {
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

pub fn set_clipboard_text(content: &str, backend: ClipboardBackend) -> Result<(), String> {
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

pub fn set_clipboard_image(
    image_path: &std::path::PathBuf,
    backend: ClipboardBackend,
) -> Result<(), String> {
    use std::fs;

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
