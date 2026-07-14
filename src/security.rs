use std::fs;
use std::path::Path;

pub const DEFAULT_MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;

pub fn validate_text_file(path: &Path, max_bytes: u64) -> Result<(), String> {
    let metadata = fs::metadata(path).map_err(|err| format!("Cannot read metadata for {}: {err}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("Not a regular file: {}", path.display()));
    }
    if metadata.len() > max_bytes {
        return Err(format!(
            "File is too large: {} bytes. Current limit is {} bytes.",
            metadata.len(),
            max_bytes
        ));
    }
    Ok(())
}

pub fn reject_probably_binary(bytes: &[u8]) -> Result<(), String> {
    if bytes.iter().take(8192).any(|byte| *byte == 0) {
        return Err("File looks like binary data. Orion opens text source files only.".to_string());
    }
    Ok(())
}

pub fn should_ignore_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".hg"
            | ".svn"
            | ".cache"
            | ".idea"
            | "node_modules"
            | "target"
            | "build"
            | "dist"
            | "out"
            | "coverage"
            | "__pycache__"
            | ".pytest_cache"
            | ".mypy_cache"
            | ".ruff_cache"
            | ".next"
            | ".nuxt"
            | ".svelte-kit"
            | ".turbo"
            | ".venv"
            | "venv"
    )
}

pub fn is_hidden_name(name: &str) -> bool {
    name.starts_with('.') && name != "." && name != ".."
}
