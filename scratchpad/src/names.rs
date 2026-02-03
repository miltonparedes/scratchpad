//! Session name generation module
//!
//! Generates unique session names using:
//! 1. LLM (claude or codex) if available
//! 2. Static adjective-noun combinations as fallback

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use rand::prelude::*;

use crate::models::Config;

const ADJECTIVES: &[&str] = &[
    "atomic", "quantum", "orbital", "galactic", "nuclear", "binary", "cryo",
    "turbo", "nano", "stealth", "hyper", "cosmic", "neon", "plasma", "cyber",
    "chrome", "vector", "rogue", "phantom", "shadow", "blazing", "frozen",
    "silent", "swift", "dark", "bright", "wild", "calm", "fierce", "gentle",
];

const NOUNS: &[&str] = &[
    "comet", "reactor", "pulsar", "quasar", "drone", "nexus", "vortex",
    "titan", "phoenix", "cipher", "matrix", "daemon", "kernel", "codec",
    "payload", "vertex", "axiom", "proxy", "mantis", "falcon", "spark",
    "storm", "wave", "pulse", "flare", "orbit", "prism", "beacon", "echo",
];

const MODIFIERS: &[&str] = &[
    "mk2", "prime", "zero", "alpha", "omega", "x9", "pro", "max", "ultra", "lite",
];

const CACHE_SIZE: usize = 10;

fn cache_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "scratchpad")
        .map(|d| d.config_dir().join("name-cache.txt"))
        .unwrap_or_else(|| PathBuf::from("~/.config/scratchpad/name-cache.txt"))
}

fn load_name_cache() -> Vec<String> {
    let path = cache_path();
    if !path.exists() {
        return Vec::new();
    }

    fs::read_to_string(&path)
        .ok()
        .map(|content| {
            content
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn save_to_cache(name: &str) {
    let path = cache_path();

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let mut cache = load_name_cache();
    cache.push(name.to_string());

    // Keep only last CACHE_SIZE entries
    if cache.len() > CACHE_SIZE {
        let skip = cache.len() - CACHE_SIZE;
        cache = cache.into_iter().skip(skip).collect();
    }

    let content = cache.join("\n") + "\n";
    let _ = fs::write(&path, content);
}

/// Generate a random static name (adjective-noun or noun-modifier)
fn generate_static_name() -> String {
    let mut rng = rand::rng();

    // 80% adjective-noun, 20% noun-modifier
    if rng.random_bool(0.8) {
        let adj = ADJECTIVES[rng.random_range(0..ADJECTIVES.len())];
        let noun = NOUNS[rng.random_range(0..NOUNS.len())];
        format!("{}-{}", adj, noun)
    } else {
        let noun = NOUNS[rng.random_range(0..NOUNS.len())];
        let modifier = MODIFIERS[rng.random_range(0..MODIFIERS.len())];
        format!("{}-{}", noun, modifier)
    }
}

/// Try to generate a name using Claude
fn try_claude_generate() -> Option<String> {
    if which::which("claude").is_err() {
        return None;
    }

    let prompt = "Generate a single creative two-word project codename in the format 'adjective-noun' (lowercase, hyphenated). Examples: quantum-phoenix, stealth-matrix. Output ONLY the name, nothing else.";

    let output = Command::new("claude")
        .args(["--print", "-p", prompt])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let name = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_lowercase()
        .replace(' ', "-");

    // Validate it looks like a reasonable name
    if name.contains('-') && name.len() >= 5 && name.len() <= 30 && name.chars().all(|c| c.is_alphanumeric() || c == '-') {
        Some(name)
    } else {
        None
    }
}

/// Try to generate a name using Codex
fn try_codex_generate() -> Option<String> {
    if which::which("codex").is_err() {
        return None;
    }

    let prompt = "Generate a single creative two-word project codename in the format 'adjective-noun' (lowercase, hyphenated). Examples: quantum-phoenix, stealth-matrix. Output ONLY the name, nothing else.";

    // Try codex with quiet mode
    let output = Command::new("codex")
        .args(["--quiet", "-p", prompt])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let name = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_lowercase()
        .replace(' ', "-");

    // Validate it looks like a reasonable name
    if name.contains('-') && name.len() >= 5 && name.len() <= 30 && name.chars().all(|c| c.is_alphanumeric() || c == '-') {
        Some(name)
    } else {
        None
    }
}

/// Try to generate a name using LLM based on config
fn generate_llm_name(config: &Config) -> Option<String> {
    match config.name_generator.as_str() {
        "auto" => {
            // Try claude first, then codex
            try_claude_generate().or_else(try_codex_generate)
        }
        "claude" => try_claude_generate(),
        "codex" => try_codex_generate(),
        "static" | _ => None,
    }
}

/// Generate a unique session name, avoiding collisions and recently used names
pub fn generate_session_name(existing: &[String], config: &Config) -> String {
    let cache = load_name_cache();

    for _ in 0..10 {
        let name = generate_llm_name(config).unwrap_or_else(generate_static_name);

        // Skip if in cache or already exists
        if !cache.contains(&name) && !existing.contains(&name) {
            save_to_cache(&name);
            return name;
        }
    }

    // Fallback: add numeric suffix
    let base = generate_static_name();
    for i in 2..100 {
        let name = format!("{}-{}", base, i);
        if !existing.contains(&name) {
            save_to_cache(&name);
            return name;
        }
    }

    // Ultimate fallback
    let name = format!("{}-{}", base, rand::rng().random_range(100..1000));
    save_to_cache(&name);
    name
}

/// Convert a title/text to a valid slug.
/// Returns None if the input contains no alphanumeric characters.
pub fn slugify(title: &str) -> Option<String> {
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    if slug.is_empty() {
        None
    } else {
        Some(slug)
    }
}

/// Convert a title to a slug, falling back to a generated name if empty.
pub fn slugify_or_generate(title: &str, existing: &[String], config: &Config) -> String {
    slugify(title).unwrap_or_else(|| generate_session_name(existing, config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), Some("hello-world".to_string()));
        assert_eq!(slugify("My Project 2024"), Some("my-project-2024".to_string()));
        assert_eq!(slugify("  multiple   spaces  "), Some("multiple-spaces".to_string()));
        assert_eq!(slugify("special!@#chars"), Some("special-chars".to_string()));
        // Edge cases that should return None
        assert_eq!(slugify("!!!"), None);
        assert_eq!(slugify("   "), None);
        assert_eq!(slugify(""), None);
    }

    #[test]
    fn test_static_name_generation() {
        for _ in 0..10 {
            let name = generate_static_name();
            assert!(name.contains('-'));
            assert!(name.len() >= 5);
        }
    }
}
