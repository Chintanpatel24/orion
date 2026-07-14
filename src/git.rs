use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct GitFile {
    pub path: String,
    pub status: String,
    pub staged: bool,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    Context,
    Added,
    Removed,
    Header,
}

#[derive(Debug, Clone)]
pub struct DiffRow {
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub old_text: String,
    pub new_text: String,
    pub kind: DiffKind,
}

pub fn repo_root(start: &Path) -> Result<Option<PathBuf>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(start)
        .args(["rev-parse", "--show-toplevel"])
        .stdin(Stdio::null())
        .output()
        .map_err(|err| format!("Cannot run git: {err}"))?;

    if !output.status.success() {
        return Ok(None);
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if root.is_empty() {
        Ok(None)
    } else {
        Ok(Some(PathBuf::from(root)))
    }
}

pub fn branch(root: &Path) -> Result<String, String> {
    let output = run_git(root, &["branch", "--show-current"])?;
    let branch = output.trim();
    if !branch.is_empty() {
        return Ok(branch.to_string());
    }
    let head = run_git(root, &["rev-parse", "--short", "HEAD"])?;
    Ok(head.trim().to_string())
}

pub fn changed_files(root: &Path) -> Result<Vec<GitFile>, String> {
    let output = run_git(root, &["status", "--porcelain"])?;
    let mut files = Vec::new();

    for line in output.lines() {
        if line.len() < 4 {
            continue;
        }
        let xy = line.get(0..2).unwrap_or("  ").to_string();
        let mut path = line.get(3..).unwrap_or_default().to_string();
        if let Some((_, new_path)) = path.split_once(" -> ") {
            path = new_path.to_string();
        }
        let staged = xy.as_bytes().first().is_some_and(|status| *status != b' ' && *status != b'?');
        let fingerprint = change_fingerprint(root, &path, &xy);
        files.push(GitFile {
            path,
            status: xy.trim().to_string(),
            staged,
            fingerprint,
        });
    }

    files.sort_by(|a, b| a.path.to_ascii_lowercase().cmp(&b.path.to_ascii_lowercase()));
    Ok(files)
}

pub fn diff_for_file(root: &Path, path: &str, status: &str) -> Result<Vec<DiffRow>, String> {
    if status.contains("??") {
        return diff_for_untracked(root, path);
    }

    let mut diff = run_git_path(root, &["diff", "--unified=3", "--"], path)?;
    if diff.trim().is_empty() {
        diff = run_git_path(root, &["diff", "--cached", "--unified=3", "--"], path)?;
    }

    if diff.trim().is_empty() {
        return Ok(vec![DiffRow {
            old_line: None,
            new_line: None,
            old_text: "No textual diff available".to_string(),
            new_text: String::new(),
            kind: DiffKind::Header,
        }]);
    }

    Ok(parse_unified_diff(&diff))
}

pub fn stage_file(root: &Path, path: &str) -> Result<(), String> {
    run_git_path(root, &["add", "--"], path).map(|_| ())
}

pub fn unstage_file(root: &Path, path: &str) -> Result<(), String> {
    run_git_path(root, &["restore", "--staged", "--"], path).map(|_| ())
}

pub fn commit(root: &Path, message: &str) -> Result<(), String> {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return Err("Commit message is empty".to_string());
    }
    run_git(root, &["commit", "-m", trimmed]).map(|_| ())
}

fn run_git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|err| format!("Cannot run git: {err}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() { "Git command failed".to_string() } else { stderr })
    }
}

fn run_git_path(root: &Path, args: &[&str], path: &str) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .arg(path)
        .stdin(Stdio::null())
        .output()
        .map_err(|err| format!("Cannot run git: {err}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() { "Git command failed".to_string() } else { stderr })
    }
}

fn diff_for_untracked(root: &Path, path: &str) -> Result<Vec<DiffRow>, String> {
    let full_path = root.join(path);
    let text = fs::read_to_string(&full_path)
        .map_err(|err| format!("Cannot read untracked file {}: {err}", full_path.display()))?;
    let mut rows = vec![DiffRow {
        old_line: None,
        new_line: None,
        old_text: format!("new file: {path}"),
        new_text: String::new(),
        kind: DiffKind::Header,
    }];

    for (idx, line) in text.lines().enumerate() {
        rows.push(DiffRow {
            old_line: None,
            new_line: Some(idx + 1),
            old_text: String::new(),
            new_text: line.to_string(),
            kind: DiffKind::Added,
        });
    }

    Ok(rows)
}

fn change_fingerprint(root: &Path, path: &str, status: &str) -> String {
    let mut data = String::new();
    data.push_str(status);
    data.push('\n');
    data.push_str(path);
    data.push('\n');

    if status.contains("??") {
        let full_path = root.join(path);
        match fs::metadata(&full_path) {
            Ok(metadata) => {
                data.push_str(&metadata.len().to_string());
                if let Ok(modified) = metadata.modified() {
                    if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                        data.push_str(&duration.as_nanos().to_string());
                    }
                }
            }
            Err(err) => data.push_str(&err.to_string()),
        }
    } else if let Ok(diff) = run_git_path(root, &["diff", "--unified=0", "--"], path) {
        data.push_str(&diff);
        if diff.trim().is_empty() {
            if let Ok(cached) = run_git_path(root, &["diff", "--cached", "--unified=0", "--"], path) {
                data.push_str(&cached);
            }
        }
    }

    format!("{:016x}", fnv1a64(data.as_bytes()))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn parse_unified_diff(diff: &str) -> Vec<DiffRow> {
    let mut rows = Vec::new();
    let mut old_line = 0usize;
    let mut new_line = 0usize;

    for line in diff.lines() {
        if line.starts_with("diff --git") || line.starts_with("index ") || line.starts_with("--- ") || line.starts_with("+++ ") {
            rows.push(DiffRow {
                old_line: None,
                new_line: None,
                old_text: line.to_string(),
                new_text: String::new(),
                kind: DiffKind::Header,
            });
            continue;
        }

        if line.starts_with("@@") {
            if let Some((old_start, new_start)) = parse_hunk_header(line) {
                old_line = old_start;
                new_line = new_start;
            }
            rows.push(DiffRow {
                old_line: None,
                new_line: None,
                old_text: line.to_string(),
                new_text: String::new(),
                kind: DiffKind::Header,
            });
            continue;
        }

        if let Some(text) = line.strip_prefix('-') {
            rows.push(DiffRow {
                old_line: Some(old_line),
                new_line: None,
                old_text: text.to_string(),
                new_text: String::new(),
                kind: DiffKind::Removed,
            });
            old_line += 1;
            continue;
        }

        if let Some(text) = line.strip_prefix('+') {
            rows.push(DiffRow {
                old_line: None,
                new_line: Some(new_line),
                old_text: String::new(),
                new_text: text.to_string(),
                kind: DiffKind::Added,
            });
            new_line += 1;
            continue;
        }

        let text = line.strip_prefix(' ').unwrap_or(line).to_string();
        rows.push(DiffRow {
            old_line: Some(old_line),
            new_line: Some(new_line),
            old_text: text.clone(),
            new_text: text,
            kind: DiffKind::Context,
        });
        old_line += 1;
        new_line += 1;
    }

    rows
}

fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    let mut parts = line.split_whitespace();
    parts.next()?;
    let old = parts.next()?;
    let new = parts.next()?;
    Some((parse_hunk_start(old)?, parse_hunk_start(new)?))
}

fn parse_hunk_start(part: &str) -> Option<usize> {
    let trimmed = part.trim_start_matches(['-', '+']);
    let start = trimmed.split(',').next()?;
    start.parse::<usize>().ok()
}
