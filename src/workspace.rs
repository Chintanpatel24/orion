use crate::security;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct TreeEntry {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub is_dir: bool,
}

#[derive(Debug, Clone)]
pub struct Workspace {
    pub root: Option<PathBuf>,
    pub entries: Vec<TreeEntry>,
    pub max_depth: usize,
    pub max_entries: usize,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            root: None,
            entries: Vec::new(),
            max_depth: 12,
            max_entries: 10_000,
        }
    }
}

impl Workspace {
    pub fn open(&mut self, root: PathBuf, show_hidden: bool) -> Result<(), String> {
        if !root.is_dir() {
            return Err(format!("Not a directory: {}", root.display()));
        }
        self.root = Some(root);
        self.refresh(show_hidden)
    }

    pub fn refresh(&mut self, show_hidden: bool) -> Result<(), String> {
        self.entries.clear();
        let Some(root) = self.root.clone() else {
            return Ok(());
        };
        collect_entries(&root, 0, self.max_depth, self.max_entries, show_hidden, &mut self.entries)?;
        Ok(())
    }
}

fn collect_entries(
    dir: &Path,
    depth: usize,
    max_depth: usize,
    max_entries: usize,
    show_hidden: bool,
    out: &mut Vec<TreeEntry>,
) -> Result<(), String> {
    if depth > max_depth || out.len() >= max_entries {
        return Ok(());
    }

    let mut children = Vec::new();
    for entry in fs::read_dir(dir).map_err(|err| format!("Cannot read directory {}: {err}", dir.display()))? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if !show_hidden && security::is_hidden_name(&name) {
            continue;
        }

        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if metadata.file_type().is_symlink() {
            continue;
        }

        let is_dir = metadata.is_dir();
        if is_dir && security::should_ignore_dir(&name) {
            continue;
        }

        children.push((path, name, is_dir));
    }

    children.sort_by(|a, b| match (a.2, b.2) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.1.to_ascii_lowercase().cmp(&b.1.to_ascii_lowercase()),
    });

    for (path, name, is_dir) in children {
        if out.len() >= max_entries {
            return Ok(());
        }
        out.push(TreeEntry {
            path: path.clone(),
            name: name.clone(),
            depth,
            is_dir,
        });
        if is_dir {
            collect_entries(&path, depth + 1, max_depth, max_entries, show_hidden, out)?;
        }
    }

    Ok(())
}
