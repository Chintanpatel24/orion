use crate::command::{self, PaletteAction};
use crate::document::Document;
use crate::git::{self, DiffKind, DiffRow, GitFile};
use crate::settings::{Settings, ThemeMode};
use crate::syntax;
use crate::workspace::Workspace;
use eframe::egui;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
enum CloseRequest {
    Document(usize),
    App,
}

pub struct OrionApp {
    settings: Settings,
    workspace: Workspace,
    documents: Vec<Document>,
    current: usize,
    next_doc_id: u64,
    show_palette: bool,
    palette_query: String,
    show_search: bool,
    search_query: String,
    replace_query: String,
    show_settings: bool,
    show_help: bool,
    show_git_review: bool,
    hide_done_changes: bool,
    git_repo: Option<PathBuf>,
    git_branch: String,
    git_files: Vec<GitFile>,
    selected_git_path: Option<String>,
    diff_rows: Vec<DiffRow>,
    commit_message: String,
    status: String,
    pending_close: Option<CloseRequest>,
}

impl OrionApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = Settings::load();
        settings.apply_to_context(&cc.egui_ctx);

        let mut app = Self {
            settings,
            workspace: Workspace::default(),
            documents: vec![Document::untitled(1)],
            current: 0,
            next_doc_id: 2,
            show_palette: false,
            palette_query: String::new(),
            show_search: false,
            search_query: String::new(),
            replace_query: String::new(),
            show_settings: false,
            show_help: false,
            show_git_review: false,
            hide_done_changes: true,
            git_repo: None,
            git_branch: String::new(),
            git_files: Vec::new(),
            selected_git_path: None,
            diff_rows: Vec::new(),
            commit_message: String::new(),
            status: "Ready. Orion is an IDE not for you, but for your agents.".to_string(),
            pending_close: None,
        };

        if let Some(root) = app.settings.last_workspace.clone() {
            let _ = app.workspace.open(root, app.settings.show_hidden_files);
            app.refresh_git();
        }

        for arg in std::env::args_os().skip(1) {
            let path = PathBuf::from(arg);
            if path.is_dir() {
                app.open_workspace(path);
            } else {
                app.open_document(path);
            }
        }

        app
    }

    fn new_document(&mut self) {
        let id = self.next_doc_id;
        self.next_doc_id += 1;
        self.documents.push(Document::untitled(id));
        self.current = self.documents.len() - 1;
        self.show_git_review = false;
        self.status = "New file created".to_string();
    }

    fn open_document(&mut self, path: PathBuf) {
        if let Some(idx) = self.documents.iter().position(|doc| doc.path.as_deref() == Some(path.as_path())) {
            self.current = idx;
            self.show_git_review = false;
            self.status = format!("Already open: {}", path.display());
            return;
        }

        match Document::open(path.clone(), self.settings.max_file_bytes()) {
            Ok(document) => {
                if self.documents.len() == 1
                    && self.documents[0].path.is_none()
                    && self.documents[0].text.is_empty()
                    && !self.documents[0].dirty
                {
                    self.documents[0] = document;
                    self.current = 0;
                } else {
                    self.documents.push(document);
                    self.current = self.documents.len() - 1;
                }
                self.show_git_review = false;
                self.status = format!("Opened {}", path.display());
            }
            Err(err) => self.status = err,
        }
    }

    fn open_workspace(&mut self, path: PathBuf) {
        match self.workspace.open(path.clone(), self.settings.show_hidden_files) {
            Ok(()) => {
                self.settings.last_workspace = Some(path.clone());
                let _ = self.settings.save();
                self.status = format!("Workspace remembered: {}", path.display());
                self.refresh_git();
            }
            Err(err) => self.status = err,
        }
    }

    fn save_current(&mut self) {
        let Some(doc) = self.documents.get_mut(self.current) else {
            return;
        };
        if doc.path.is_none() {
            self.save_current_as();
            return;
        }
        match doc.save() {
            Ok(()) => {
                self.status = format!("Saved {}", doc.title);
                self.refresh_git();
            }
            Err(err) => self.status = err,
        }
    }

    fn save_current_as(&mut self) {
        let Some(path) = pick_save_file() else {
            self.status = "Save As cancelled or native file dialogs are disabled".to_string();
            return;
        };
        let Some(doc) = self.documents.get_mut(self.current) else {
            return;
        };
        match doc.save_as(path.clone()) {
            Ok(()) => {
                self.status = format!("Saved {}", path.display());
                self.refresh_git();
            }
            Err(err) => self.status = err,
        }
    }

    fn save_all(&mut self) {
        let mut saved = 0usize;
        let mut skipped = 0usize;
        for doc in &mut self.documents {
            if !doc.dirty {
                continue;
            }
            if doc.path.is_none() {
                skipped += 1;
                continue;
            }
            match doc.save() {
                Ok(()) => saved += 1,
                Err(err) => {
                    self.status = err;
                    return;
                }
            }
        }
        self.status = format!("Saved {saved} file(s). Skipped {skipped} untitled file(s).");
        self.refresh_git();
    }

    fn request_close_document(&mut self, idx: usize) {
        if self.documents.get(idx).is_some_and(|doc| doc.dirty) {
            self.pending_close = Some(CloseRequest::Document(idx));
        } else {
            self.close_document_now(idx);
        }
    }

    fn close_document_now(&mut self, idx: usize) {
        if idx >= self.documents.len() {
            return;
        }
        self.documents.remove(idx);
        self.current = self.current.min(self.documents.len().saturating_sub(1));
    }

    fn current_document(&self) -> Option<&Document> {
        self.documents.get(self.current)
    }

    fn current_document_mut(&mut self) -> Option<&mut Document> {
        self.documents.get_mut(self.current)
    }

    fn refresh_git(&mut self) {
        let base = self.workspace.root.clone().or_else(|| self.current_document().and_then(|doc| doc.directory()));
        let Some(base) = base else {
            self.git_repo = None;
            self.git_files.clear();
            self.diff_rows.clear();
            self.git_branch.clear();
            self.status = "Open a project folder to use Git Review".to_string();
            return;
        };

        match git::repo_root(&base) {
            Ok(Some(repo)) => {
                self.git_repo = Some(repo.clone());
                self.git_branch = git::branch(&repo).unwrap_or_else(|_| "detached".to_string());
                match git::changed_files(&repo) {
                    Ok(files) => {
                        self.git_files = files;
                        self.select_first_visible_git_file_if_needed();
                        self.load_selected_diff();
                        self.status = format!("Git refreshed on {}", self.git_branch);
                    }
                    Err(err) => self.status = err,
                }
            }
            Ok(None) => {
                self.git_repo = None;
                self.git_files.clear();
                self.diff_rows.clear();
                self.git_branch.clear();
                self.status = "The current workspace is not a Git repository".to_string();
            }
            Err(err) => self.status = err,
        }
    }

    fn select_first_visible_git_file_if_needed(&mut self) {
        let selected_still_visible = self
            .selected_git_path
            .as_ref()
            .and_then(|selected| self.git_files.iter().find(|file| &file.path == selected))
            .is_some_and(|file| !self.hide_done_changes || !self.is_git_file_done(file));

        if selected_still_visible {
            return;
        }

        self.selected_git_path = self
            .git_files
            .iter()
            .find(|file| !self.hide_done_changes || !self.is_git_file_done(file))
            .map(|file| file.path.clone());
    }

    fn load_selected_diff(&mut self) {
        let Some(repo) = self.git_repo.clone() else {
            self.diff_rows.clear();
            return;
        };
        let Some(path) = self.selected_git_path.clone() else {
            self.diff_rows.clear();
            return;
        };
        let Some(file) = self.git_files.iter().find(|file| file.path == path).cloned() else {
            self.diff_rows.clear();
            return;
        };
        match git::diff_for_file(&repo, &file.path, &file.status) {
            Ok(rows) => self.diff_rows = rows,
            Err(err) => {
                self.diff_rows = vec![DiffRow {
                    old_line: None,
                    new_line: None,
                    old_text: err,
                    new_text: String::new(),
                    kind: DiffKind::Header,
                }]
            }
        }
    }

    fn is_git_file_done(&self, file: &GitFile) -> bool {
        self.git_repo.as_ref().is_some_and(|repo| self.settings.is_reviewed(repo, &file.path, &file.fingerprint))
    }

    fn mark_selected_done(&mut self) {
        let Some(repo) = self.git_repo.clone() else {
            return;
        };
        let Some(path) = self.selected_git_path.clone() else {
            return;
        };
        let Some(file) = self.git_files.iter().find(|file| file.path == path).cloned() else {
            return;
        };
        self.settings.mark_reviewed(&repo, &file.path, &file.fingerprint);
        let _ = self.settings.save();
        self.status = format!("Marked done: {}", file.path);
        self.refresh_git();
    }

    fn unmark_selected_done(&mut self) {
        let Some(repo) = self.git_repo.clone() else {
            return;
        };
        let Some(path) = self.selected_git_path.clone() else {
            return;
        };
        self.settings.forget_reviewed(&repo, &path);
        let _ = self.settings.save();
        self.status = format!("Marked not done: {path}");
        self.refresh_git();
    }

    fn stage_selected(&mut self) {
        let Some(repo) = self.git_repo.clone() else {
            return;
        };
        let Some(path) = self.selected_git_path.clone() else {
            return;
        };
        match git::stage_file(&repo, &path) {
            Ok(()) => {
                self.status = format!("Staged {path}");
                self.refresh_git();
            }
            Err(err) => self.status = err,
        }
    }

    fn unstage_selected(&mut self) {
        let Some(repo) = self.git_repo.clone() else {
            return;
        };
        let Some(path) = self.selected_git_path.clone() else {
            return;
        };
        match git::unstage_file(&repo, &path) {
            Ok(()) => {
                self.status = format!("Unstaged {path}");
                self.refresh_git();
            }
            Err(err) => self.status = err,
        }
    }

    fn commit_staged(&mut self) {
        let Some(repo) = self.git_repo.clone() else {
            self.status = "No Git repository open".to_string();
            return;
        };
        match git::commit(&repo, &self.commit_message) {
            Ok(()) => {
                self.commit_message.clear();
                self.status = "Commit created".to_string();
                self.refresh_git();
            }
            Err(err) => self.status = err,
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let new_file = ctx.input(|i| i.key_pressed(egui::Key::N) && i.modifiers.command);
        let open_file = ctx.input(|i| i.key_pressed(egui::Key::O) && i.modifiers.command && !i.modifiers.shift);
        let open_folder = ctx.input(|i| i.key_pressed(egui::Key::O) && i.modifiers.command && i.modifiers.shift);
        let save = ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.command && !i.modifiers.shift);
        let save_as = ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.command && i.modifiers.shift);
        let palette = ctx.input(|i| i.key_pressed(egui::Key::P) && i.modifiers.command);
        let search = ctx.input(|i| i.key_pressed(egui::Key::F) && i.modifiers.command);
        let git_review = ctx.input(|i| i.key_pressed(egui::Key::G) && i.modifiers.command);
        let quit = ctx.input(|i| i.key_pressed(egui::Key::Q) && i.modifiers.command);

        if new_file {
            self.new_document();
        }
        if open_file {
            self.pick_and_open_file();
        }
        if open_folder {
            self.pick_and_open_workspace();
        }
        if save {
            self.save_current();
        }
        if save_as {
            self.save_current_as();
        }
        if palette {
            self.show_palette = true;
        }
        if search {
            self.show_search = true;
        }
        if git_review {
            self.show_git_review = true;
            self.refresh_git();
        }
        if quit {
            self.request_quit(ctx);
        }
    }

    fn pick_and_open_file(&mut self) {
        if let Some(path) = pick_file() {
            self.open_document(path);
        } else {
            self.status = "Open file cancelled or native file dialogs are disabled".to_string();
        }
    }

    fn pick_and_open_workspace(&mut self) {
        if let Some(path) = pick_folder() {
            self.open_workspace(path);
        } else {
            self.status = "Open folder cancelled or native file dialogs are disabled".to_string();
        }
    }

    fn request_quit(&mut self, ctx: &egui::Context) {
        if self.documents.iter().any(|doc| doc.dirty) {
            self.pending_close = Some(CloseRequest::App);
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn execute_palette_action(&mut self, action: PaletteAction) {
        match action {
            PaletteAction::NewFile => self.new_document(),
            PaletteAction::OpenFile => self.pick_and_open_file(),
            PaletteAction::OpenFolder => self.pick_and_open_workspace(),
            PaletteAction::Save => self.save_current(),
            PaletteAction::SaveAs => self.save_current_as(),
            PaletteAction::SaveAll => self.save_all(),
            PaletteAction::GitReview => {
                self.show_git_review = true;
                self.refresh_git();
            }
            PaletteAction::RefreshGit => self.refresh_git(),
            PaletteAction::Search => self.show_search = true,
            PaletteAction::RefreshWorkspace => match self.workspace.refresh(self.settings.show_hidden_files) {
                Ok(()) => self.status = "Workspace refreshed".to_string(),
                Err(err) => self.status = err,
            },
            PaletteAction::Settings => self.show_settings = true,
            PaletteAction::Help => self.show_help = true,
        }
    }

    fn execute_freeform_palette_command(&mut self) -> bool {
        let query = self.palette_query.trim().to_string();
        if query.eq_ignore_ascii_case("git") || query.eq_ignore_ascii_case("review") {
            self.show_git_review = true;
            self.refresh_git();
            return true;
        }
        if let Some(path) = query.strip_prefix("open ").map(str::trim).filter(|s| !s.is_empty()) {
            self.open_document(PathBuf::from(path));
            return true;
        }
        if let Some(path) = query.strip_prefix("folder ").map(str::trim).filter(|s| !s.is_empty()) {
            self.open_workspace(PathBuf::from(path));
            return true;
        }
        false
    }

    fn show_top_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        egui::Panel::top("top_bar").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                draw_orion_mark(ui);
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("Orion").size(20.0).strong().color(egui::Color32::from_rgb(226, 237, 248)),
                    );
                    ui.label(
                        egui::RichText::new("IDE not for you, but for your agents")
                            .size(12.0)
                            .color(egui::Color32::from_rgb(142, 160, 184)),
                    );
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("Git Review").strong())
                                .fill(egui::Color32::from_rgb(36, 51, 82)),
                        )
                        .clicked()
                    {
                        self.show_git_review = true;
                        self.refresh_git();
                    }
                });
            });
            ui.add_space(4.0);
            ui.separator();
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New file    Ctrl-N").clicked() {
                        self.new_document();
                        ui.close();
                    }
                    if ui.button("Open file    Ctrl-O").clicked() {
                        self.pick_and_open_file();
                        ui.close();
                    }
                    if ui.button("Open folder    Ctrl-Shift-O").clicked() {
                        self.pick_and_open_workspace();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Save    Ctrl-S").clicked() {
                        self.save_current();
                        ui.close();
                    }
                    if ui.button("Save as    Ctrl-Shift-S").clicked() {
                        self.save_current_as();
                        ui.close();
                    }
                    if ui.button("Save all").clicked() {
                        self.save_all();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Quit    Ctrl-Q").clicked() {
                        self.request_quit(&ctx);
                        ui.close();
                    }
                });

                ui.menu_button("Git", |ui| {
                    if ui.button("Review changes    Ctrl-G").clicked() {
                        self.show_git_review = true;
                        self.refresh_git();
                        ui.close();
                    }
                    if ui.button("Refresh Git").clicked() {
                        self.refresh_git();
                        ui.close();
                    }
                    if ui.button("Stage selected").clicked() {
                        self.stage_selected();
                        ui.close();
                    }
                    if ui.button("Unstage selected").clicked() {
                        self.unstage_selected();
                        ui.close();
                    }
                    if ui.button("Mark selected done").clicked() {
                        self.mark_selected_done();
                        ui.close();
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui.button("Editor").clicked() {
                        self.show_git_review = false;
                        ui.close();
                    }
                    if ui.button("Command palette    Ctrl-P").clicked() {
                        self.show_palette = true;
                        ui.close();
                    }
                    if ui.button("Search    Ctrl-F").clicked() {
                        self.show_search = true;
                        ui.close();
                    }
                    if ui.button("Settings").clicked() {
                        self.show_settings = true;
                        ui.close();
                    }
                    if ui.button("Refresh workspace").clicked() {
                        let _ = self.workspace.refresh(self.settings.show_hidden_files);
                        ui.close();
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("Shortcuts").clicked() {
                        self.show_help = true;
                        ui.close();
                    }
                });

                ui.separator();
                if !self.git_branch.is_empty() {
                    ui.label(
                        egui::RichText::new(format!("git: {}", self.git_branch))
                            .color(egui::Color32::from_rgb(85, 224, 212)),
                    );
                    ui.separator();
                }
                ui.label(
                    egui::RichText::new(format!("changes: {}", self.git_files.len()))
                        .color(egui::Color32::from_rgb(142, 160, 184)),
                );
            });
        });
    }

    fn show_workspace_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("workspace_panel").resizable(true).default_size(286.0).show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading("Project");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Refresh").clicked() {
                        match self.workspace.refresh(self.settings.show_hidden_files) {
                            Ok(()) => self.status = "Workspace refreshed".to_string(),
                            Err(err) => self.status = err,
                        }
                    }
                });
            });
            ui.add_space(4.0);

            if let Some(root) = &self.workspace.root {
                ui.group(|ui| {
                    ui.label(
                        egui::RichText::new("Remembered project").small().color(egui::Color32::from_rgb(142, 160, 184)),
                    );
                    ui.label(
                        egui::RichText::new(root.display().to_string())
                            .monospace()
                            .color(egui::Color32::from_rgb(226, 237, 248)),
                    );
                    if !self.git_branch.is_empty() {
                        ui.label(
                            egui::RichText::new(format!("branch: {}", self.git_branch))
                                .color(egui::Color32::from_rgb(85, 224, 212)),
                        );
                    }
                });
                ui.add_space(8.0);

                let mut open_path = None;
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for entry in &self.workspace.entries {
                        ui.horizontal(|ui| {
                            ui.add_space((entry.depth * 14) as f32);
                            let label = if entry.is_dir { format!("{}/", entry.name) } else { entry.name.clone() };
                            let text = if entry.is_dir {
                                egui::RichText::new(label).color(egui::Color32::from_rgb(142, 160, 184))
                            } else {
                                egui::RichText::new(label).color(egui::Color32::from_rgb(216, 222, 233))
                            };
                            let response = ui.selectable_label(false, text);
                            if response.clicked() && !entry.is_dir {
                                open_path = Some(entry.path.clone());
                            }
                        });
                    }
                });
                if let Some(path) = open_path {
                    self.open_document(path);
                }
            } else {
                ui.group(|ui| {
                    ui.label(egui::RichText::new("No project folder open").strong());
                    ui.label("Orion remembers the last folder until you choose another one.");
                    if ui.button("Open folder").clicked() {
                        self.pick_and_open_workspace();
                    }
                });
            }
        });
    }

    fn show_main_area(&mut self, ui: &mut egui::Ui) {
        if self.show_git_review {
            self.show_git_review_panel(ui);
        } else {
            self.show_editor(ui);
        }
    }

    fn show_editor(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            self.show_tabs(ui);
            ui.separator();

            let Some(doc) = self.documents.get(self.current) else {
                return;
            };
            let highlight_limit = self.settings.highlight_limit_mb.saturating_mul(1024).saturating_mul(1024) as usize;
            let language = if self.settings.syntax_highlighting
                && !self.settings.low_power_mode
                && doc.byte_count() <= highlight_limit
            {
                doc.language
            } else {
                syntax::Language::Plain
            };
            let font_size = self.settings.font_size;

            let Some(doc) = self.documents.get_mut(self.current) else {
                return;
            };

            let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
                let job = syntax::highlighted_job(ui, text.as_str(), language, wrap_width, font_size);
                ui.fonts_mut(|fonts| fonts.layout_job(job))
            };

            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                let response = ui.add(
                    egui::TextEdit::multiline(&mut doc.text)
                        .font(egui::TextStyle::Monospace)
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .desired_rows(32)
                        .lock_focus(true)
                        .layouter(&mut layouter),
                );

                if response.changed() {
                    doc.dirty = true;
                }
            });
        });
    }

    fn show_tabs(&mut self, ui: &mut egui::Ui) {
        let mut close_idx = None;
        ui.horizontal_wrapped(|ui| {
            for idx in 0..self.documents.len() {
                let doc = &self.documents[idx];
                let selected = idx == self.current;
                if ui.selectable_label(selected, doc.display_title()).clicked() {
                    self.current = idx;
                    self.show_git_review = false;
                }
                if ui.small_button("x").on_hover_text("Close tab").clicked() {
                    close_idx = Some(idx);
                }
                ui.separator();
            }
        });
        if let Some(idx) = close_idx {
            self.request_close_document(idx);
        }
    }

    fn show_git_review_panel(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("Agent Git Review")
                            .size(24.0)
                            .strong()
                            .color(egui::Color32::from_rgb(226, 237, 248)),
                    );
                    ui.label(
                        egui::RichText::new(
                            "Review changes side by side, then mark files Done until they change again.",
                        )
                        .color(egui::Color32::from_rgb(142, 160, 184)),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Editor").clicked() {
                        self.show_git_review = false;
                    }
                    if ui.button("Refresh").clicked() {
                        self.refresh_git();
                    }
                    if ui.checkbox(&mut self.hide_done_changes, "Hide done").changed() {
                        self.select_first_visible_git_file_if_needed();
                        self.load_selected_diff();
                    }
                });
            });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let repo_text = self
                    .git_repo
                    .as_ref()
                    .map(|repo| repo.display().to_string())
                    .unwrap_or_else(|| "No Git repository".to_string());
                summary_tile(ui, "Repository", &repo_text);
                summary_tile(ui, "Branch", if self.git_branch.is_empty() { "none" } else { &self.git_branch });
                summary_tile(ui, "Changed files", &self.git_files.len().to_string());
                let done_count = self.git_files.iter().filter(|file| self.is_git_file_done(file)).count();
                summary_tile(ui, "Done", &done_count.to_string());
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(320.0);
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.heading("Changed files");
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(
                                    egui::RichText::new("review queue")
                                        .small()
                                        .color(egui::Color32::from_rgb(142, 160, 184)),
                                );
                            });
                        });

                        let visible_files: Vec<GitFile> = self
                            .git_files
                            .iter()
                            .filter(|file| !self.hide_done_changes || !self.is_git_file_done(file))
                            .cloned()
                            .collect();

                        if visible_files.is_empty() {
                            ui.add_space(8.0);
                            ui.label("No visible changes. Disable Hide done to see reviewed files.");
                        }

                        egui::ScrollArea::vertical().max_height(430.0).show(ui, |ui| {
                            for file in visible_files {
                                let done = self.is_git_file_done(&file);
                                let selected = self.selected_git_path.as_deref() == Some(file.path.as_str());
                                let status_color = if done {
                                    egui::Color32::from_rgb(142, 160, 184)
                                } else if file.staged {
                                    egui::Color32::from_rgb(85, 224, 212)
                                } else {
                                    egui::Color32::from_rgb(143, 179, 255)
                                };
                                let label = if done {
                                    format!("{}  {}  done", file.status, file.path)
                                } else {
                                    format!("{}  {}", file.status, file.path)
                                };
                                let response = ui.selectable_label(
                                    selected,
                                    egui::RichText::new(label).monospace().color(status_color),
                                );
                                if response.clicked() {
                                    self.selected_git_path = Some(file.path.clone());
                                    self.load_selected_diff();
                                }
                            }
                        });

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("Stage").clicked() {
                                self.stage_selected();
                            }
                            if ui.button("Unstage").clicked() {
                                self.unstage_selected();
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    egui::Button::new(egui::RichText::new("Done").strong())
                                        .fill(egui::Color32::from_rgb(29, 78, 64)),
                                )
                                .clicked()
                            {
                                self.mark_selected_done();
                            }
                            if ui.button("Not done").clicked() {
                                self.unmark_selected_done();
                            }
                        });

                        ui.separator();
                        ui.label(egui::RichText::new("Commit staged changes").strong());
                        ui.text_edit_singleline(&mut self.commit_message);
                        if ui.button("Commit").clicked() {
                            self.commit_staged();
                        }
                    });
                });

                ui.separator();

                ui.vertical(|ui| {
                    ui.set_min_width(650.0);
                    ui.group(|ui| {
                        let selected = self.selected_git_path.clone().unwrap_or_else(|| "No file selected".to_string());
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(selected)
                                    .size(18.0)
                                    .strong()
                                    .color(egui::Color32::from_rgb(226, 237, 248)),
                            );
                            if let Some(file) = self.selected_git_file() {
                                let done = self.is_git_file_done(&file);
                                review_badge(ui, &file.status, egui::Color32::from_rgb(143, 179, 255));
                                review_badge(
                                    ui,
                                    if file.staged { "staged" } else { "unstaged" },
                                    egui::Color32::from_rgb(85, 224, 212),
                                );
                                if done {
                                    review_badge(ui, "done", egui::Color32::from_rgb(167, 139, 250));
                                }
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Not done").clicked() {
                                    self.unmark_selected_done();
                                }
                                if ui
                                    .add(
                                        egui::Button::new(egui::RichText::new("Done").strong())
                                            .fill(egui::Color32::from_rgb(29, 78, 64)),
                                    )
                                    .clicked()
                                {
                                    self.mark_selected_done();
                                }
                            });
                        });
                        ui.separator();
                        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                            draw_diff_rows(ui, &self.diff_rows);
                        });
                    });
                });
            });
        });
    }

    fn selected_git_file(&self) -> Option<GitFile> {
        let path = self.selected_git_path.as_ref()?;
        self.git_files.iter().find(|file| &file.path == path).cloned()
    }

    fn show_status_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status_bar").exact_size(28.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                ui.separator();
                if let Some(root) = &self.workspace.root {
                    ui.label(format!("project: {}", root.display()));
                }
                ui.separator();
                ui.label(format!("changes: {}", self.git_files.len()));
                ui.separator();
                if let Some(doc) = self.current_document() {
                    ui.label(format!("{} lines", doc.line_count()));
                    ui.separator();
                    ui.label(doc.language.name());
                }
            });
        });
    }

    fn show_palette_window(&mut self, ctx: &egui::Context) {
        if !self.show_palette {
            return;
        }

        let mut open = self.show_palette;
        egui::Window::new("Command palette")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(560.0)
            .show(ctx, |ui| {
                ui.label("Type a command or select an action.");
                ui.label("Freeform commands: open <path>, folder <path>, git, review.");
                let enter =
                    ui.add(egui::TextEdit::singleline(&mut self.palette_query).hint_text("Command")).lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter));

                if enter && self.execute_freeform_palette_command() {
                    self.palette_query.clear();
                    self.show_palette = false;
                    return;
                }

                ui.separator();
                egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                    let mut clicked_action = None;
                    for item in
                        command::palette_items().iter().filter(|item| command::matches_query(item, &self.palette_query))
                    {
                        if ui.selectable_label(false, item.name).on_hover_text(item.detail).clicked() {
                            clicked_action = Some(item.action);
                        }
                    }
                    if let Some(action) = clicked_action {
                        self.execute_palette_action(action);
                        self.palette_query.clear();
                        self.show_palette = false;
                    }
                });
            });
        self.show_palette = open && self.show_palette;
    }

    fn show_search_window(&mut self, ctx: &egui::Context) {
        if !self.show_search {
            return;
        }

        let mut open = self.show_search;
        egui::Window::new("Search").open(&mut open).collapsible(false).default_width(420.0).show(ctx, |ui| {
            ui.label("Find text in the current file.");
            ui.text_edit_singleline(&mut self.search_query);
            ui.horizontal(|ui| {
                ui.label("Replace with");
                ui.text_edit_singleline(&mut self.replace_query);
            });
            let count = self.current_document().map(|doc| count_matches(&doc.text, &self.search_query)).unwrap_or(0);
            ui.label(format!("Matches: {count}"));
            ui.horizontal(|ui| {
                if ui.button("Replace all").clicked() {
                    let search = self.search_query.clone();
                    let replace = self.replace_query.clone();
                    if !search.is_empty() {
                        if let Some(doc) = self.current_document_mut() {
                            doc.text = doc.text.replace(&search, &replace);
                            doc.dirty = true;
                        }
                    }
                }
                if ui.button("Close").clicked() {
                    self.show_search = false;
                }
            });
        });
        self.show_search = open && self.show_search;
    }

    fn show_settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }

        let mut changed = false;
        let mut open = self.show_settings;
        egui::Window::new("Settings")
            .open(&mut open)
            .default_width(540.0)
            .show(ctx, |ui| {
                ui.heading("Editor");
                egui::ComboBox::from_label("Theme")
                    .selected_text(theme_name(self.settings.theme))
                    .show_ui(ui, |ui| {
                        changed |= ui.selectable_value(&mut self.settings.theme, ThemeMode::System, "System").changed();
                        changed |= ui.selectable_value(&mut self.settings.theme, ThemeMode::Light, "Light").changed();
                        changed |= ui.selectable_value(&mut self.settings.theme, ThemeMode::Dark, "Dark").changed();
                    });
                changed |= ui.add(egui::Slider::new(&mut self.settings.font_size, 10.0..=24.0).text("Font size")).changed();
                changed |= ui.add(egui::Slider::new(&mut self.settings.tab_size, 2..=8).text("Tab size")).changed();
                changed |= ui.checkbox(&mut self.settings.syntax_highlighting, "Syntax highlighting").changed();
                changed |= ui.add(egui::Slider::new(&mut self.settings.highlight_limit_mb, 1..=16).text("Highlight limit MB")).changed();
                changed |= ui.checkbox(&mut self.settings.low_power_mode, "Low-power mode for very old hardware").changed();
                changed |= ui.checkbox(&mut self.settings.show_hidden_files, "Show hidden files").changed();
                changed |= ui.add(egui::Slider::new(&mut self.settings.max_file_size_mb, 1..=256).text("Max file size MB")).changed();

                ui.separator();
                ui.heading("Agent Git Review");
                ui.label("Done markers are stored in Orion settings as path and change fingerprints only. Project files are never copied into Orion's config folder.");
                changed |= ui.checkbox(&mut self.hide_done_changes, "Hide files marked Done").changed();

                if ui.button("Save settings").clicked() {
                    changed = true;
                }
            });
        self.show_settings = open;

        if changed {
            self.settings.apply_to_context(ctx);
            if let Err(err) = self.settings.save() {
                self.status = err;
            } else {
                self.status = "Settings saved".to_string();
            }
            let _ = self.workspace.refresh(self.settings.show_hidden_files);
            self.select_first_visible_git_file_if_needed();
            self.load_selected_diff();
        }
    }

    fn show_help_window(&mut self, ctx: &egui::Context) {
        if !self.show_help {
            return;
        }
        let mut open = self.show_help;
        egui::Window::new("Shortcuts").open(&mut open).default_width(440.0).show(ctx, |ui| {
            ui.monospace("Ctrl-N          New file");
            ui.monospace("Ctrl-O          Open file");
            ui.monospace("Ctrl-Shift-O    Open project folder");
            ui.monospace("Ctrl-S          Save");
            ui.monospace("Ctrl-Shift-S    Save as");
            ui.monospace("Ctrl-P          Command palette");
            ui.monospace("Ctrl-F          Search");
            ui.monospace("Ctrl-G          Agent Git Review");
            ui.monospace("Ctrl-Q          Quit");
        });
        self.show_help = open;
    }

    fn show_confirm_close_window(&mut self, ctx: &egui::Context) {
        let Some(request) = self.pending_close else {
            return;
        };

        egui::Window::new("Unsaved changes").collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label("There are unsaved changes.");
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    match request {
                        CloseRequest::Document(idx) => {
                            self.current = idx;
                            self.save_current();
                            if !self.documents.get(idx).is_some_and(|doc| doc.dirty) {
                                self.close_document_now(idx);
                                self.pending_close = None;
                            }
                        }
                        CloseRequest::App => {
                            self.save_all();
                            if !self.documents.iter().any(|doc| doc.dirty) {
                                self.pending_close = None;
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                    }
                }
                if ui.button("Discard").clicked() {
                    match request {
                        CloseRequest::Document(idx) => self.close_document_now(idx),
                        CloseRequest::App => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    }
                    self.pending_close = None;
                }
                if ui.button("Cancel").clicked() {
                    self.pending_close = None;
                }
            });
        });
    }
}

impl eframe::App for OrionApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_shortcuts(&ctx);
        self.show_top_bar(ui);
        self.show_status_bar(ui);
        self.show_workspace_panel(ui);
        self.show_main_area(ui);
        self.show_palette_window(&ctx);
        self.show_search_window(&ctx);
        self.show_settings_window(&ctx);
        self.show_help_window(&ctx);
        self.show_confirm_close_window(&ctx);
    }
}

fn draw_orion_mark(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::hover());
    let center = rect.center();
    let painter = ui.painter();
    let bg = egui::Color32::from_rgb(15, 23, 42);
    let border = egui::Color32::from_rgb(30, 41, 59);
    let ring_a = egui::Color32::from_rgb(203, 213, 225);
    let ring_b = egui::Color32::from_rgb(100, 116, 139);

    let _border = border;
    painter.rect_filled(rect.shrink(1.0), 8.0, bg);
    painter.add(egui::Shape::line(
        ellipse_points(center, 10.3, 4.4, -30.0_f32.to_radians()),
        egui::Stroke::new(1.7, ring_a),
    ));
    painter.add(egui::Shape::line(
        ellipse_points(center, 10.3, 4.4, 35.0_f32.to_radians()),
        egui::Stroke::new(1.7, ring_b),
    ));
}

fn ellipse_points(center: egui::Pos2, rx: f32, ry: f32, angle: f32) -> Vec<egui::Pos2> {
    let mut points = Vec::with_capacity(49);
    let cos = angle.cos();
    let sin = angle.sin();
    for step in 0..=48 {
        let t = step as f32 / 48.0 * std::f32::consts::TAU;
        let x = rx * t.cos();
        let y = ry * t.sin();
        points.push(egui::pos2(center.x + x * cos - y * sin, center.y + x * sin + y * cos));
    }
    points
}

fn summary_tile(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.group(|ui| {
        ui.set_min_width(150.0);
        ui.label(egui::RichText::new(label).small().color(egui::Color32::from_rgb(142, 160, 184)));
        ui.label(egui::RichText::new(value).strong().color(egui::Color32::from_rgb(226, 237, 248)));
    });
}

fn review_badge(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    ui.label(egui::RichText::new(text).small().strong().color(color));
}

fn draw_diff_rows(ui: &mut egui::Ui, rows: &[DiffRow]) {
    egui::Grid::new("agent_diff_grid").num_columns(4).spacing([8.0, 3.0]).striped(true).show(ui, |ui| {
        ui.strong("Old");
        ui.strong("Before");
        ui.strong("New");
        ui.strong("After");
        ui.end_row();

        for row in rows {
            let (left_color, right_color, bg) = diff_colors(row.kind);
            ui.monospace(row.old_line.map(|line| line.to_string()).unwrap_or_default());
            ui.label(egui::RichText::new(&row.old_text).monospace().color(left_color).background_color(bg));
            ui.monospace(row.new_line.map(|line| line.to_string()).unwrap_or_default());
            ui.label(egui::RichText::new(&row.new_text).monospace().color(right_color).background_color(bg));
            ui.end_row();
        }
    });
}

fn diff_colors(kind: DiffKind) -> (egui::Color32, egui::Color32, egui::Color32) {
    match kind {
        DiffKind::Added => (
            egui::Color32::from_rgb(180, 230, 190),
            egui::Color32::from_rgb(180, 230, 190),
            egui::Color32::from_rgb(20, 64, 42),
        ),
        DiffKind::Removed => (
            egui::Color32::from_rgb(245, 190, 190),
            egui::Color32::from_rgb(245, 190, 190),
            egui::Color32::from_rgb(74, 34, 38),
        ),
        DiffKind::Header => (
            egui::Color32::from_rgb(147, 164, 188),
            egui::Color32::from_rgb(147, 164, 188),
            egui::Color32::from_rgb(16, 24, 38),
        ),
        DiffKind::Context => {
            (egui::Color32::from_rgb(216, 222, 233), egui::Color32::from_rgb(216, 222, 233), egui::Color32::TRANSPARENT)
        }
    }
}

fn count_matches(text: &str, query: &str) -> usize {
    if query.is_empty() {
        0
    } else {
        text.match_indices(query).count()
    }
}

fn theme_name(theme: ThemeMode) -> &'static str {
    match theme {
        ThemeMode::System => "System",
        ThemeMode::Light => "Light",
        ThemeMode::Dark => "Dark",
    }
}

#[cfg(feature = "native-dialogs")]
fn pick_file() -> Option<PathBuf> {
    rfd::FileDialog::new().pick_file()
}

#[cfg(not(feature = "native-dialogs"))]
fn pick_file() -> Option<PathBuf> {
    None
}

#[cfg(feature = "native-dialogs")]
fn pick_folder() -> Option<PathBuf> {
    rfd::FileDialog::new().pick_folder()
}

#[cfg(not(feature = "native-dialogs"))]
fn pick_folder() -> Option<PathBuf> {
    None
}

#[cfg(feature = "native-dialogs")]
fn pick_save_file() -> Option<PathBuf> {
    rfd::FileDialog::new().save_file()
}

#[cfg(not(feature = "native-dialogs"))]
fn pick_save_file() -> Option<PathBuf> {
    None
}
