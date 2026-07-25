use crate::security;
use crate::syntax::{self, Language};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum LineDiffKind {
    Unchanged,
    Added,
    Removed,
    Modified,
}

#[derive(Debug, Clone)]
pub struct LineDiff {
    pub line_number: usize,
    pub kind: LineDiffKind,
    pub old_text: String,
    pub new_text: String,
}

#[derive(Debug, Clone)]
pub struct Document {
    pub path: Option<PathBuf>,
    pub title: String,
    pub text: String,
    pub dirty: bool,
    pub language: Language,
    pub snapshot: String,
}

impl Document {
    pub fn untitled(id: u64) -> Self {
        Self {
            path: None,
            title: format!("untitled-{id}"),
            text: String::new(),
            dirty: false,
            language: Language::Plain,
            snapshot: String::new(),
        }
    }

    pub fn open(path: PathBuf, max_bytes: u64) -> Result<Self, String> {
        security::validate_text_file(&path, max_bytes)?;
        let bytes = fs::read(&path).map_err(|err| format!("Cannot read {}: {err}", path.display()))?;
        security::reject_probably_binary(&bytes)?;

        let text = String::from_utf8(bytes).unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned());
        let title = file_title(&path);
        let language = syntax::language_for_path(&path);
        let snapshot = text.clone();

        Ok(Self { path: Some(path), title, text, dirty: false, language, snapshot })
    }

    pub fn save(&mut self) -> Result<(), String> {
        let Some(path) = self.path.clone() else {
            return Err("Document has no path. Use Save As first.".to_string());
        };
        fs::write(&path, self.text.as_bytes()).map_err(|err| format!("Cannot write {}: {err}", path.display()))?;
        self.dirty = false;
        self.title = file_title(&path);
        self.language = syntax::language_for_path(&path);
        Ok(())
    }

    pub fn save_as(&mut self, path: PathBuf) -> Result<(), String> {
        self.path = Some(path);
        self.save()
    }

    pub fn display_title(&self) -> String {
        if self.dirty {
            format!("{} *", self.title)
        } else {
            self.title.clone()
        }
    }

    pub fn line_count(&self) -> usize {
        if self.text.is_empty() {
            1
        } else {
            self.text.lines().count().max(1)
        }
    }

    pub fn byte_count(&self) -> usize {
        self.text.len()
    }

    pub fn directory(&self) -> Option<PathBuf> {
        self.path.as_ref().and_then(|path| path.parent().map(Path::to_path_buf))
    }

    pub fn has_diff_changes(&self) -> bool {
        self.text != self.snapshot
    }

    pub fn clear_diff(&mut self) {
        self.snapshot = self.text.clone();
    }

    pub fn diff_line_count(&self) -> usize {
        let old_lines: Vec<&str> = self.snapshot.lines().collect();
        let new_lines: Vec<&str> = self.text.lines().collect();
        let max_len = old_lines.len().max(new_lines.len());
        let mut changed = 0;
        for i in 0..max_len {
            let old = old_lines.get(i).copied().unwrap_or("");
            let new = new_lines.get(i).copied().unwrap_or("");
            if old != new {
                changed += 1;
            }
        }
        changed
    }

    /// Compute a structured line-by-line diff between the snapshot and the
    /// current text. This uses a simple longest-common-subsequence approach
    /// to match lines rather than a purely positional comparison.
    pub fn diff_lines(&self) -> Vec<LineDiff> {
        let old_lines: Vec<&str> = self.snapshot.lines().collect();
        let new_lines: Vec<&str> = self.text.lines().collect();

        let mut result = Vec::new();

        // Build LCS table
        let old_len = old_lines.len();
        let new_len = new_lines.len();
        let mut lcs = vec![vec![0u32; new_len + 1]; old_len + 1];
        for i in (0..old_len).rev() {
            for j in (0..new_len).rev() {
                if old_lines[i] == new_lines[j] {
                    lcs[i][j] = lcs[i + 1][j + 1] + 1;
                } else {
                    lcs[i][j] = lcs[i + 1][j].max(lcs[i][j + 1]);
                }
            }
        }

        // Walk the LCS table to build the diff
        let mut i = 0;
        let mut j = 0;
        let mut line_num = 1;

        while i < old_len || j < new_len {
            if i < old_len && j < new_len && old_lines[i] == new_lines[j] {
                // Unchanged line
                result.push(LineDiff {
                    line_number: line_num,
                    kind: LineDiffKind::Unchanged,
                    old_text: old_lines[i].to_string(),
                    new_text: new_lines[j].to_string(),
                });
                i += 1;
                j += 1;
                line_num += 1;
            } else if j < new_len && (i >= old_len || lcs[i][j + 1] >= lcs[i + 1][j]) {
                // Added line
                result.push(LineDiff {
                    line_number: line_num,
                    kind: LineDiffKind::Added,
                    old_text: String::new(),
                    new_text: new_lines[j].to_string(),
                });
                j += 1;
                line_num += 1;
            } else if i < old_len {
                // Removed line
                result.push(LineDiff {
                    line_number: line_num,
                    kind: LineDiffKind::Removed,
                    old_text: old_lines[i].to_string(),
                    new_text: String::new(),
                });
                i += 1;
                // Don't increment line_num for removed lines
            }
        }

        result
    }

    /// Count of lines added compared to the snapshot.
    pub fn diff_added_count(&self) -> usize {
        self.diff_lines().iter().filter(|d| d.kind == LineDiffKind::Added).count()
    }

    /// Count of lines removed compared to the snapshot.
    pub fn diff_removed_count(&self) -> usize {
        self.diff_lines().iter().filter(|d| d.kind == LineDiffKind::Removed).count()
    }
}

fn file_title(path: &Path) -> String {
    path.file_name().and_then(|name| name.to_str()).map(ToOwned::to_owned).unwrap_or_else(|| path.display().to_string())
}
