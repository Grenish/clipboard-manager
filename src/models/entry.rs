use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::utils::{SECRET_EXPIRY_SECS, format_size};

// DATA STRUCTURES
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipboardContentType {
    Text,
    Image,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SecretInfo {
    /// The detected provider name (e.g., "OpenAI", "GitHub", "AWS")
    pub provider: String,
    /// Unix timestamp when this secret expires and should be auto-deleted.
    /// None means expiry has been stopped by the user.
    pub expires_at: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub content_type: ClipboardContentType,
    pub content: String,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_info: Option<ImageInfo>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_info: Option<SecretInfo>,
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

        let secret_info = Self::detect_secret(&content);

        Self {
            content_type: ClipboardContentType::Text,
            content,
            timestamp: chrono::Utc::now().timestamp(),
            image_info: None,
            pinned: false,
            secret_info,
            content_hash,
        }
    }

    pub fn new_image(filename: String, info: ImageInfo, hash: u64) -> Self {
        Self {
            content_type: ClipboardContentType::Image,
            content: filename,
            timestamp: chrono::Utc::now().timestamp(),
            image_info: Some(info),
            pinned: false,
            secret_info: None,
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

    /// Returns true if this secret has expired.
    #[allow(dead_code)]
    pub fn is_expired(&self) -> bool {
        if let Some(ref info) = self.secret_info {
            if let Some(expires_at) = info.expires_at {
                return chrono::Utc::now().timestamp() >= expires_at;
            }
        }
        false
    }

    /// Returns true if this entry is a detected secret.
    pub fn is_secret(&self) -> bool {
        self.secret_info.is_some()
    }

    /// Returns the remaining seconds until expiry, or None if no expiry is set.
    pub fn expiry_remaining_secs(&self) -> Option<i64> {
        if let Some(ref info) = self.secret_info {
            if let Some(expires_at) = info.expires_at {
                let remaining = expires_at - chrono::Utc::now().timestamp();
                return Some(if remaining > 0 { remaining } else { 0 });
            }
        }
        None
    }

    /// Format the expiry countdown as a human-readable string.
    pub fn expiry_label(&self) -> Option<String> {
        if let Some(remaining) = self.expiry_remaining_secs() {
            let mins = remaining / 60;
            let secs = remaining % 60;
            if mins > 0 {
                Some(format!("Expires in {}m {:02}s", mins, secs))
            } else {
                Some(format!("Expires in {}s", secs))
            }
        } else if self.is_secret() {
            Some("No expiry".to_string())
        } else {
            None
        }
    }

    /// Detect if the content is a secret/sensitive value.
    /// Returns Some(SecretInfo) with the provider name and an expiry timestamp if detected.
    fn detect_secret(content: &str) -> Option<SecretInfo> {
        let trimmed = content.trim();

        // Skip empty or very short strings
        if trimmed.len() < 8 {
            return None;
        }

        // Skip multi-line content that looks like code/prose (secrets are typically single values)
        let line_count = trimmed.lines().count();

        // Provider-specific prefix detection
        if let Some(provider) = Self::detect_secret_provider(trimmed) {
            return Some(SecretInfo {
                provider,
                expires_at: Some(chrono::Utc::now().timestamp() + SECRET_EXPIRY_SECS),
            });
        }

        // Private key block detection (multi-line is OK for keys)
        if trimmed.starts_with("-----BEGIN") && trimmed.contains("PRIVATE KEY") {
            return Some(SecretInfo {
                provider: "Private Key".to_string(),
                expires_at: Some(chrono::Utc::now().timestamp() + SECRET_EXPIRY_SECS),
            });
        }

        // Google Cloud service account JSON detection
        if trimmed.contains("\"type\"")
            && trimmed.contains("service_account")
            && trimmed.contains("\"private_key\"")
        {
            return Some(SecretInfo {
                provider: "Google Cloud".to_string(),
                expires_at: Some(chrono::Utc::now().timestamp() + SECRET_EXPIRY_SECS),
            });
        }

        // Generic high-entropy secret detection (single line only)
        if line_count == 1 && Self::looks_like_generic_secret(trimmed) {
            return Some(SecretInfo {
                provider: "Secret".to_string(),
                expires_at: Some(chrono::Utc::now().timestamp() + SECRET_EXPIRY_SECS),
            });
        }

        None
    }

    /// Check for known provider-specific API key prefixes.
    fn detect_secret_provider(text: &str) -> Option<String> {
        // Each tuple: (prefix, provider_name, minimum_total_length)
        let providers: [(&str, &str, usize); 39] = [
            // OpenAI
            ("sk-proj-", "OpenAI", 20),
            ("sk-ant-", "Anthropic", 20),
            // Generic OpenAI (sk- followed by long alphanumeric)
            ("sk-", "API Key", 20),
            // GitHub
            ("ghp_", "GitHub", 20),
            ("gho_", "GitHub", 20),
            ("ghs_", "GitHub", 20),
            ("ghu_", "GitHub", 20),
            ("github_pat_", "GitHub", 30),
            // AWS
            ("AKIA", "AWS", 16),
            ("ASIA", "AWS", 16),
            // Slack
            ("xoxb-", "Slack", 20),
            ("xoxp-", "Slack", 20),
            ("xoxe-", "Slack", 20),
            ("xoxa-", "Slack", 20),
            // Stripe
            ("sk_live_", "Stripe", 20),
            ("sk_test_", "Stripe", 20),
            ("pk_live_", "Stripe", 20),
            ("pk_test_", "Stripe", 20),
            ("rk_live_", "Stripe", 20),
            ("rk_test_", "Stripe", 20),
            // Twilio
            ("SK", "Twilio", 30),
            // Discord
            ("mfa.", "Discord", 60),
            // Vercel
            ("vercel_", "Vercel", 20),
            // Supabase
            ("sbp_", "Supabase", 20),
            // Mailgun
            ("key-", "Mailgun", 20),
            // SendGrid
            ("SG.", "SendGrid", 30),
            // Google / GCP / Gemini / Firebase (all share AIza prefix)
            ("AIza", "Google/Gemini", 30),
            // Google OAuth client secret
            ("GOCSPX-", "Google OAuth", 20),
            // Google Cloud service account JSON key
            ("gcp_", "Google Cloud", 20),
            // Firebase (web API key alternative format)
            ("firebase-", "Firebase", 20),
            // Vertex AI (service agent key)
            ("ya29.", "Google Cloud", 30),
            // Npm
            ("npm_", "npm", 20),
            // PyPI
            ("pypi-", "PyPI", 20),
            // Heroku
            ("heroku_", "Heroku", 20),
            // DigitalOcean
            ("dop_v1_", "DigitalOcean", 30),
            ("doo_v1_", "DigitalOcean", 30),
            // Databricks
            ("dapi", "Databricks", 20),
            // Hugging Face
            ("hf_", "HuggingFace", 20),
            // Telegram
            ("bot", "Telegram", 30),
        ];

        // Only check single-line, no-space content for prefix matching
        if text.contains('\n') || text.contains(' ') {
            // Exception: allow "Bearer" tokens
            if text.starts_with("Bearer ") || text.starts_with("bearer ") {
                let token = text.splitn(2, ' ').nth(1).unwrap_or("");
                if token.len() >= 20 && !token.contains(' ') {
                    // JWT detection
                    if token.starts_with("eyJ") {
                        return Some("JWT".to_string());
                    }
                    return Some("Bearer Token".to_string());
                }
            }
            return None;
        }

        // JWT detection (standalone, no Bearer prefix)
        if text.starts_with("eyJ")
            && text.chars().filter(|&c| c == '.').count() == 2
            && text.len() >= 30
        {
            return Some("JWT".to_string());
        }

        for (prefix, provider, min_len) in providers {
            if text.starts_with(prefix) && text.len() >= min_len {
                // Twilio special case: "SK" prefix is too generic, require all hex after it
                if prefix == "SK" {
                    let rest = &text[2..];
                    if rest.len() >= 30 && rest.chars().all(|c| c.is_ascii_hexdigit()) {
                        return Some(provider.to_string());
                    }
                    continue;
                }
                // Telegram special case: "bot" prefix needs format like "bot123456:ABC..."
                if prefix == "bot" {
                    if text.contains(':') {
                        let parts: Vec<&str> = text.splitn(2, ':').collect();
                        if parts.len() == 2 {
                            let num_part = &parts[0][3..];
                            if num_part.chars().all(|c| c.is_ascii_digit()) && num_part.len() >= 5 {
                                return Some(provider.to_string());
                            }
                        }
                    }
                    continue;
                }
                return Some(provider.to_string());
            }
        }

        // Credit card detection (Luhn algorithm)
        let digits_only: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
        let all_valid = text
            .chars()
            .all(|c| c.is_ascii_digit() || c == ' ' || c == '-');
        if all_valid
            && digits_only.len() >= 13
            && digits_only.len() <= 19
            && Self::luhn_check(&digits_only)
        {
            return Some("Credit Card".to_string());
        }

        None
    }

    /// Luhn algorithm to validate credit card numbers.
    fn luhn_check(digits: &str) -> bool {
        let mut sum = 0u32;
        let mut double = false;
        for ch in digits.chars().rev() {
            if let Some(d) = ch.to_digit(10) {
                let val = if double {
                    let doubled = d * 2;
                    if doubled > 9 { doubled - 9 } else { doubled }
                } else {
                    d
                };
                sum += val;
                double = !double;
            } else {
                return false;
            }
        }
        sum % 10 == 0
    }

    /// Heuristic: does this single-line string look like a generic secret?
    /// (High entropy, mixed character types, no spaces, long enough)
    fn looks_like_generic_secret(text: &str) -> bool {
        // Must be single token (no spaces), reasonable length for a secret
        if text.contains(' ') || text.len() < 20 || text.len() > 200 {
            return false;
        }

        // Skip things that look like URLs, paths, emails
        if text.starts_with("http://")
            || text.starts_with("https://")
            || text.starts_with("ftp://")
            || text.starts_with('/')
            || text.starts_with("~/")
            || text.starts_with("./")
            || text.contains('@')
        {
            return false;
        }

        // Skip things that look like hex colors
        if text.starts_with('#') && text.len() <= 9 {
            return false;
        }

        let has_upper = text.chars().any(|c| c.is_ascii_uppercase());
        let has_lower = text.chars().any(|c| c.is_ascii_lowercase());
        let has_digit = text.chars().any(|c| c.is_ascii_digit());
        let has_special = text
            .chars()
            .any(|c| !c.is_alphanumeric() && c != '-' && c != '_');

        let alpha_numeric_count = text.chars().filter(|c| c.is_alphanumeric()).count();
        let total = text.len();

        // Must be mostly alphanumeric (not prose/sentences)
        if (alpha_numeric_count as f64 / total as f64) < 0.7 {
            return false;
        }

        // Typical password: mixed case + digits + special, 8-40 chars
        let char_type_count =
            has_upper as u8 + has_lower as u8 + has_digit as u8 + has_special as u8;

        // Need at least 3 different character types for high entropy
        if char_type_count >= 3 && text.len() >= 12 {
            return true;
        }

        // Long random-looking alphanumeric strings (API keys without special chars)
        if char_type_count >= 2 && text.len() >= 30 {
            // Check it's not just a simple word repeated or a readable string
            // Simple heuristic: count unique characters — high diversity = likely random
            let unique_chars: std::collections::HashSet<char> = text.chars().collect();
            if unique_chars.len() >= 10 {
                return true;
            }
        }

        false
    }

    /// Detect the content category and return (icon, label) for display.
    pub fn detect_category(&self) -> (&str, &str) {
        // If it's a detected secret, return the secret category
        if self.is_secret() {
            return ("🔒", "Secret");
        }

        match self.content_type {
            ClipboardContentType::Image => ("🖼️", "Image"),
            ClipboardContentType::Text => {
                let trimmed = self.content.trim();

                // URL detection
                if trimmed.starts_with("http://")
                    || trimmed.starts_with("https://")
                    || trimmed.starts_with("ftp://")
                {
                    return ("🔗", "Link");
                }

                // Email detection: contains @ with text before and after, has a dot after @
                if !trimmed.contains(' ') && trimmed.contains('@') {
                    if let Some(at_pos) = trimmed.find('@') {
                        let before = &trimmed[..at_pos];
                        let after = &trimmed[at_pos + 1..];
                        if !before.is_empty() && after.contains('.') && after.len() > 2 {
                            return ("📧", "Email");
                        }
                    }
                }

                // Hex color detection: #RGB, #RRGGBB, #RRGGBBAA
                if trimmed.starts_with('#') && trimmed.len() >= 4 && trimmed.len() <= 9 {
                    let hex_part = &trimmed[1..];
                    if matches!(hex_part.len(), 3 | 4 | 6 | 8)
                        && hex_part.chars().all(|c| c.is_ascii_hexdigit())
                    {
                        return ("🎨", "Color");
                    }
                }

                // RGB/HSL color detection
                if (trimmed.starts_with("rgb(")
                    || trimmed.starts_with("rgba(")
                    || trimmed.starts_with("hsl(")
                    || trimmed.starts_with("hsla("))
                    && trimmed.ends_with(')')
                {
                    return ("🎨", "Color");
                }

                // File path detection
                if trimmed.starts_with('/')
                    || trimmed.starts_with("~/")
                    || trimmed.starts_with("./")
                    || trimmed.starts_with("../")
                {
                    // Make sure it looks like a path (has separators, no spaces at start)
                    if trimmed.contains('/') && !trimmed.contains("  ") {
                        return ("📁", "Path");
                    }
                }

                // Phone number detection: starts with + or digits, mostly digits/spaces/dashes/parens
                if trimmed.len() >= 7 && trimmed.len() <= 20 {
                    let first = trimmed.chars().next().unwrap_or(' ');
                    if first == '+' || first.is_ascii_digit() {
                        let digit_count = trimmed.chars().filter(|c| c.is_ascii_digit()).count();
                        let valid_chars = trimmed
                            .chars()
                            .all(|c| c.is_ascii_digit() || " -+()".contains(c));
                        if valid_chars && digit_count >= 7 {
                            return ("📞", "Phone");
                        }
                    }
                }

                // Code snippet detection: look for common programming patterns
                if Self::looks_like_code(trimmed) {
                    return ("💻", "Code");
                }

                // Default: plain text
                ("📝", "Text")
            }
        }
    }

    /// Heuristic check for whether content looks like a code snippet.
    fn looks_like_code(text: &str) -> bool {
        // Single-line or multi-line code indicators
        let code_starters = [
            "fn ",
            "pub fn ",
            "pub struct ",
            "pub enum ",
            "impl ",
            "let ",
            "const ",
            "mut ",
            "use ",
            "mod ", // Rust
            "function ",
            "const ",
            "var ",
            "let ",
            "=>", // JS/TS
            "import ",
            "export ",
            "require(", // JS/TS modules
            "def ",
            "class ",
            "from ",
            "elif ", // Python
            "if (",
            "if(",
            "for (",
            "for(",
            "while (",
            "while(", // C-style
            "return ",
            "return;",
            "#include",
            "#define",
            "#ifdef", // C/C++
            "package ",
            "func ", // Go
        ];

        let first_line = text.lines().next().unwrap_or("");

        for starter in &code_starters {
            if first_line.trim_start().starts_with(starter) {
                return true;
            }
        }

        // Check for structural code patterns
        let brace_count = text.chars().filter(|&c| c == '{' || c == '}').count();
        let semicolons = text.chars().filter(|&c| c == ';').count();
        let arrows = text.matches("->").count() + text.matches("=>").count();
        let parens = text.chars().filter(|&c| c == '(' || c == ')').count();

        // If it has a significant amount of code-like punctuation, it's probably code
        let total_signals = brace_count + semicolons + arrows;
        if total_signals >= 3 {
            return true;
        }

        // Function-call-heavy text: multiple parentheses pairs with semicolons
        if parens >= 4 && semicolons >= 2 {
            return true;
        }

        false
    }

    pub fn metadata_label(&self) -> String {
        let pin_prefix = if self.pinned { "📌 " } else { "" };

        // Special handling for secrets
        if let Some(ref secret) = self.secret_info {
            let expiry = self.expiry_label().unwrap_or_default();
            return format!("{}🔒 Secret · {} · {}", pin_prefix, secret.provider, expiry);
        }

        let (icon, label) = self.detect_category();
        match self.content_type {
            ClipboardContentType::Text => {
                format!(
                    "{}{} {} · {} char",
                    pin_prefix,
                    icon,
                    label,
                    self.content.len()
                )
            }
            ClipboardContentType::Image => {
                if let Some(info) = &self.image_info {
                    format!(
                        "{}{} {} · {}",
                        pin_prefix,
                        icon,
                        label,
                        format_size(info.size_bytes)
                    )
                } else {
                    format!("{}{} {} · Unknown size", pin_prefix, icon, label)
                }
            }
        }
    }

    /// Generate preview lines for display in the TUI.
    /// If `reveal` is true, show the actual content even for secrets.
    pub fn preview_lines_with_reveal(&self, reveal: bool) -> Vec<String> {
        // Mask secret content unless revealed
        if self.is_secret() && !reveal {
            let provider = self
                .secret_info
                .as_ref()
                .map(|s| s.provider.as_str())
                .unwrap_or("Secret");

            // Show masked content with a hint of the beginning
            let trimmed = self.content.trim();
            let mask = if trimmed.len() > 6 {
                let visible = &trimmed[..4];
                format!("{}{}", visible, "•".repeat(20))
            } else {
                "•".repeat(20)
            };

            return vec![format!("{} — {}", provider, mask)];
        }

        self.preview_lines()
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
