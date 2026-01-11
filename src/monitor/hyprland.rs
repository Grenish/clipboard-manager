use std::env;
use std::process::Command;

pub fn apply_hyprland_rules() {
    // Check if we are running in Hyprland
    let has_env = env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok();
    
    // Even if env var is missing, check if hyprctl works (handling possible env stripping)
    let version = get_hyprland_version();

    if !has_env && version.is_none() {
        println!("i Hyprland not detected (no env var or hyprctl response). Skipping auto-config.");
        return;
    }

    println!("⚡ Detected Hyprland session, attempting to apply window rules...");
    
    // Default to older version logic if version check fails, or check semantic version
    // Hyprland versions are typically like "v0.39.1" or "0.39.1"
    
    let is_v0_53_plus = if let Some(ver) = version {
        println!("  ✓ Hyprland version detected: {}", ver);
        is_version_ge(&ver, 0, 53)
    } else {
        println!("  ! Could not detect Hyprland version, assuming older syntax.");
        false
    };

    if is_v0_53_plus {
        apply_rules_v53();
    } else {
        apply_rules_legacy();
    }
}

fn get_hyprland_version() -> Option<String> {
    let output = Command::new("hyprctl")
        .arg("version")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Output format usually: "Hyprland, built from branch ... Tag: v0.53.1 ..."
    // Or JSON: hyprctl -j version
    
    // Let's try JSON for reliability if possible, but hyprctl version output is simple enough.
    // Example: "Tag: v0.53.1"
    
    for line in stdout.lines() {
        if line.trim().starts_with("Tag:") {
             let tag = line.split(':').nth(1)?.trim();
             return Some(tag.trim_start_matches('v').to_string());
        }
    }
    
    None
}

fn is_version_ge(version_str: &str, target_major: u32, target_minor: u32) -> bool {
    // Simple parser: major.minor.patch
    // Handle things like "0.53.1-40-g..."
    let clean_ver = version_str.split('-').next().unwrap_or(version_str);
    let parts: Vec<&str> = clean_ver.split('.').collect();
    
    if parts.len() < 2 {
        return false;
    }

    let major: u32 = parts[0].parse().unwrap_or(0);
    let minor: u32 = parts[1].parse().unwrap_or(0);

    if major > target_major {
        return true;
    }
    if major == target_major && minor >= target_minor {
        return true;
    }
    
    false
}

fn apply_rules_legacy() {
    let rules = [
        "windowrulev2 float, class:(floating-clipboard)",
        "windowrulev2 size 900 600, class:(floating-clipboard)",
        "windowrulev2 center, class:(floating-clipboard)",
        "windowrulev2 animation popin, class:(floating-clipboard)",
        "windowrulev2 stayfocused, class:(floating-clipboard)",
    ];

    for rule in rules {
        let _ = Command::new("hyprctl")
            .arg("keyword")
            .arg("windowrulev2")
            .arg(rule.strip_prefix("windowrulev2 ").unwrap_or(rule))
            .status();
    }
    println!("Applied legacy window rules (windowrulev2)");
}

fn apply_rules_v53() {
    // New rules using windowrule=RULE,WINDOW syntax suitable for single line commands
    // We utilize 'on' for booleans and space for matchers.
    let rules = [
        "windowrule float on, match:class floating-clipboard",
        "windowrule size 900 600, match:class floating-clipboard",
        "windowrule center on, match:class floating-clipboard",
        "windowrule animation popin, match:class floating-clipboard",
    ];

    for rule in rules {
         // rule string is like "windowrule float on, match:class..."
         // hyprctl keyword expects: hyprctl keyword windowrule "float on, match:class..."
         let arg = rule.strip_prefix("windowrule ").unwrap_or(rule);
         
         let _ = Command::new("hyprctl")
            .arg("keyword")
            .arg("windowrule")
            .arg(arg)
            .status();
    }
    println!("Applied v0.53+ window rules (windowrule)");
}
