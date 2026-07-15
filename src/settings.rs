use crate::security::DEFAULT_MAX_FILE_BYTES;
use directories::ProjectDirs;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    System,
    Light,
    #[default]
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewedChange {
    pub repo: PathBuf,
    pub path: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemeMode,
    pub font_size: f32,
    pub tab_size: usize,
    pub show_hidden_files: bool,
    pub max_file_size_mb: u64,
    pub syntax_highlighting: bool,
    pub highlight_limit_mb: u64,
    pub low_power_mode: bool,
    pub last_workspace: Option<PathBuf>,
    pub reviewed_changes: Vec<ReviewedChange>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::Dark,
            font_size: 14.0,
            tab_size: 4,
            show_hidden_files: false,
            max_file_size_mb: DEFAULT_MAX_FILE_BYTES / 1024 / 1024,
            syntax_highlighting: true,
            highlight_limit_mb: 2,
            low_power_mode: false,
            last_workspace: None,
            reviewed_changes: Vec::new(),
        }
    }
}

impl Settings {
    pub fn config_path() -> Option<PathBuf> {
        ProjectDirs::from("dev", "Orion", "Orion").map(|dirs| dirs.config_dir().join("settings.toml"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };

        match fs::read_to_string(&path) {
            Ok(contents) => toml::from_str::<Self>(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let Some(path) = Self::config_path() else {
            return Err("Cannot locate a config directory on this system".to_string());
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("Cannot create config directory {}: {err}", parent.display()))?;
        }

        let contents = toml::to_string_pretty(self).map_err(|err| format!("Cannot serialize settings: {err}"))?;
        fs::write(&path, contents).map_err(|err| format!("Cannot write settings {}: {err}", path.display()))
    }

    pub fn max_file_bytes(&self) -> u64 {
        self.max_file_size_mb.saturating_mul(1024).saturating_mul(1024).max(1024 * 1024)
    }

    pub fn is_reviewed(&self, repo: &Path, path: &str, fingerprint: &str) -> bool {
        self.reviewed_changes
            .iter()
            .any(|change| change.repo == repo && change.path == path && change.fingerprint == fingerprint)
    }

    pub fn mark_reviewed(&mut self, repo: &Path, path: &str, fingerprint: &str) {
        if self.is_reviewed(repo, path, fingerprint) {
            return;
        }
        self.reviewed_changes.retain(|change| !(change.repo == repo && change.path == path));
        self.reviewed_changes.push(ReviewedChange {
            repo: repo.to_path_buf(),
            path: path.to_string(),
            fingerprint: fingerprint.to_string(),
        });
        if self.reviewed_changes.len() > 4096 {
            let extra = self.reviewed_changes.len() - 4096;
            self.reviewed_changes.drain(0..extra);
        }
    }

    pub fn forget_reviewed(&mut self, repo: &Path, path: &str) {
        self.reviewed_changes.retain(|change| !(change.repo == repo && change.path == path));
    }

    pub fn apply_to_context(&self, ctx: &egui::Context) {
        let mut visuals = match self.theme {
            ThemeMode::System => egui::Visuals::dark(),
            ThemeMode::Light => egui::Visuals::light(),
            ThemeMode::Dark => egui::Visuals::dark(),
        };

        if visuals.dark_mode {
            visuals.panel_fill = egui::Color32::from_rgb(13, 17, 23);
            visuals.window_fill = egui::Color32::from_rgb(18, 24, 33);
            visuals.extreme_bg_color = egui::Color32::from_rgb(7, 10, 15);
            visuals.faint_bg_color = egui::Color32::from_rgb(22, 29, 40);
            visuals.hyperlink_color = egui::Color32::from_rgb(124, 156, 255);
            visuals.selection.bg_fill = egui::Color32::from_rgb(58, 91, 172);
        }
        ctx.set_visuals(visuals);

        let themes = match self.theme {
            ThemeMode::System => vec![egui::Theme::Light, egui::Theme::Dark],
            ThemeMode::Light => vec![egui::Theme::Light],
            ThemeMode::Dark => vec![egui::Theme::Dark],
        };

        for theme in themes {
            let mut style = (*ctx.style_of(theme)).clone();
            style.spacing.item_spacing = egui::vec2(8.0, 6.0);
            style.spacing.button_padding = egui::vec2(12.0, 6.0);
            style.text_styles.insert(egui::TextStyle::Monospace, egui::FontId::monospace(self.font_size));
            style.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(self.font_size));
            style.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(self.font_size));
            ctx.set_style_of(theme, style);
        }
    }
}
