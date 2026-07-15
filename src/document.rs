use crate::security;
use crate::syntax::{self, Language};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Document {
    pub path: Option<PathBuf>,
    pub title: String,
    pub text: String,
    pub dirty: bool,
    pub language: Language,
}

impl Document {
    pub fn untitled(id: u64) -> Self {
        Self {
            path: None,
            title: format!("untitled-{id}"),
            text: String::new(),
            dirty: false,
            language: Language::Plain,
        }
    }

    pub fn open(path: PathBuf, max_bytes: u64) -> Result<Self, String> {
        security::validate_text_file(&path, max_bytes)?;
        let bytes = fs::read(&path).map_err(|err| format!("Cannot read {}: {err}", path.display()))?;
        security::reject_probably_binary(&bytes)?;

        let text = String::from_utf8(bytes).unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned());
        let title = file_title(&path);
        let language = syntax::language_for_path(&path);

        Ok(Self { path: Some(path), title, text, dirty: false, language })
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
}

fn file_title(path: &Path) -> String {
    path.file_name().and_then(|name| name.to_str()).map(ToOwned::to_owned).unwrap_or_else(|| path.display().to_string())
}
