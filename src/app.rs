use crate::command::{self, PaletteAction};
use crate::document::{Document, LineDiffKind};
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

/// Which main view is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MainView {
    Editor,
    GitReview,
    CodeDiff,
    GitConfig,
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
    main_view: MainView,
    hide_done_changes: bool,
    git_repo: Option<PathBuf>,
    git_branch: String,
    git_files: Vec<GitFile>,
    selected_git_path: Option<String>,
    diff_rows: Vec<DiffRow>,
    commit_message: String,
    status: String,
    pending_close: Option<CloseRequest>,
    show_terminal: bool,
    terminal_input: String,
    terminal_output: String,
    terminal_running: bool,
    terminal_receiver: Option<std::sync::mpsc::Receiver<String>>,
    // Git config form fields
    git_config_name: String,
    git_config_email: String,
    git_config_branch: String,
    git_config_status: String,
    // Branch management
    new_branch_name: String,
    git_branches: Vec<String>,
    // Git log
    git_log: Vec<String>,
    // Code diff panel state
    code_diff_doc_idx: Option<usize>,
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
            main_view: MainView::Editor,
            hide_done_changes: true,
            git_repo: None,
            git_branch: String::new(),
            git_files: Vec::new(),
            selected_git_path: None,
            diff_rows: Vec::new(),
            commit_message: String::new(),
            status: "Ready. Orion is an IDE not for you, but for your agents.".to_string(),
            pending_close: None,
            show_terminal: false,
            terminal_input: String::new(),
            terminal_output: String::new(),
            terminal_running: false,
            terminal_receiver: None,
            git_config_name: String::new(),
            git_config_email: String::new(),
            git_config_branch: String::new(),
            git_config_status: String::new(),
            new_branch_name: String::new(),
            git_branches: Vec::new(),
            git_log: Vec::new(),
            code_diff_doc_idx: None,
        };

        if let Some(root) = app.settings.last_workspace.clone() {
            let _ = app.workspace.open(root, app.settings.show_hidden_files);
            app.refresh_git();
        }

        // Load git config from settings
        app.git_config_name = app.settings.git_config.user_name.clone();
        app.git_config_email = app.settings.git_config.user_email.clone();
        app.git_config_branch = app.settings.git_config.default_branch.clone();

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
        self.main_view = MainView::Editor;
        self.status = "New file created".to_string();
    }

    fn open_document(&mut self, path: PathBuf) {
        if let Some(idx) = self.documents.iter().position(|doc| doc.path.as_deref() == Some(path.as_path())) {
            self.current = idx;
            self.main_view = MainView::Editor;
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
                self.main_view = MainView::Editor;
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
        if self.documents.is_empty() {
            self.documents.push(Document::untitled(self.next_doc_id));
            self.next_doc_id += 1;
            self.current = 0;
        } else {
            self.current = self.current.min(self.documents.len().saturating_sub(1));
        }
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
            self.git_branches.clear();
            self.git_log.clear();
            self.status = "Open a project folder to use Git".to_string();
            return;
        };

        match git::repo_root(&base) {
            Ok(Some(repo)) => {
                self.git_repo = Some(repo.clone());
                self.git_branch = git::branch(&repo).unwrap_or_else(|_| "detached".to_string());
                self.git_branches = git::list_branches(&repo).unwrap_or_default();
                self.git_log = git::recent_log(&repo, 15).unwrap_or_default();
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
                self.git_branches.clear();
                self.git_log.clear();
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

    fn stage_all_and_commit(&mut self) {
        let Some(repo) = self.git_repo.clone() else {
            self.status = "No Git repository open".to_string();
            return;
        };
        if let Err(err) = git::stage_all(&repo) {
            self.status = err;
            return;
        }
        match git::commit(&repo, &self.commit_message) {
            Ok(()) => {
                self.commit_message.clear();
                self.status = "All changes staged and committed".to_string();
                self.refresh_git();
            }
            Err(err) => self.status = err,
        }
    }

    fn clear_all_diffs(&mut self) {
        for doc in &mut self.documents {
            doc.clear_diff();
        }
        self.status = "All code diffs cleared".to_string();
    }

    fn clear_current_diff(&mut self) {
        if let Some(doc) = self.documents.get_mut(self.current) {
            doc.clear_diff();
            self.status = format!("Diff cleared for {}", doc.title);
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
        let terminal = ctx.input(|i| i.key_pressed(egui::Key::T) && i.modifiers.command);
        let clear_diff = ctx.input(|i| i.key_pressed(egui::Key::D) && i.modifiers.command);
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
            self.main_view = MainView::GitReview;
            self.refresh_git();
        }
        if terminal {
            self.show_terminal = !self.show_terminal;
        }
        if clear_diff {
            self.clear_current_diff();
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
                self.main_view = MainView::GitReview;
                self.refresh_git();
            }
            PaletteAction::RefreshGit => self.refresh_git(),
            PaletteAction::CodeDiff => {
                self.code_diff_doc_idx = Some(self.current);
                self.main_view = MainView::CodeDiff;
            }
            PaletteAction::ClearDiff => self.clear_current_diff(),
            PaletteAction::GitConfig => {
                self.main_view = MainView::GitConfig;
            }
            PaletteAction::Search => self.show_search = true,
            PaletteAction::RefreshWorkspace => match self.workspace.refresh(self.settings.show_hidden_files) {
                Ok(()) => self.status = "Workspace refreshed".to_string(),
                Err(err) => self.status = err,
            },
            PaletteAction::Settings => self.show_settings = true,
            PaletteAction::Help => self.show_help = true,
            PaletteAction::Terminal => {
                self.show_terminal = !self.show_terminal;
            }
        }
    }

    fn execute_freeform_palette_command(&mut self) -> bool {
        let query = self.palette_query.trim().to_string();
        if query.eq_ignore_ascii_case("git") || query.eq_ignore_ascii_case("review") {
            self.main_view = MainView::GitReview;
            self.refresh_git();
            return true;
        }
        if query.eq_ignore_ascii_case("diff") || query.eq_ignore_ascii_case("code diff") {
            self.code_diff_doc_idx = Some(self.current);
            self.main_view = MainView::CodeDiff;
            return true;
        }
        if query.eq_ignore_ascii_case("config") || query.eq_ignore_ascii_case("git config") {
            self.main_view = MainView::GitConfig;
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
                    // Code Diff button with change count indicator
                    let diff_count: usize = self.documents.iter().map(|doc| doc.diff_line_count()).sum();
                    let diff_label = if diff_count > 0 {
                        format!("Code Diff ({})", diff_count)
                    } else {
                        "Code Diff".to_string()
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(diff_label)
                                    .strong()
                                    .color(if diff_count > 0 {
                                        egui::Color32::from_rgb(255, 200, 100)
                                    } else {
                                        egui::Color32::from_rgb(216, 222, 233)
                                    }),
                            )
                            .fill(egui::Color32::from_rgb(42, 36, 20)),
                        )
                        .clicked()
                    {
                        self.code_diff_doc_idx = Some(self.current);
                        self.main_view = MainView::CodeDiff;
                    }

                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("Git Review").strong())
                                .fill(egui::Color32::from_rgb(36, 51, 82)),
                        )
                        .clicked()
                    {
                        self.main_view = MainView::GitReview;
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
                        self.main_view = MainView::GitReview;
                        self.refresh_git();
                        ui.close();
                    }
                    if ui.button("Git config").clicked() {
                        self.main_view = MainView::GitConfig;
                        ui.close();
                    }
                    if ui.button("Refresh Git").clicked() {
                        self.refresh_git();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Stage selected").clicked() {
                        self.stage_selected();
                        ui.close();
                    }
                    if ui.button("Unstage selected").clicked() {
                        self.unstage_selected();
                        ui.close();
                    }
                    if ui.button("Stage all").clicked() {
                        if let Some(repo) = self.git_repo.clone() {
                            match git::stage_all(&repo) {
                                Ok(()) => {
                                    self.status = "All files staged".to_string();
                                    self.refresh_git();
                                }
                                Err(err) => self.status = err,
                            }
                        }
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Mark selected done").clicked() {
                        self.mark_selected_done();
                        ui.close();
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui.button("Editor").clicked() {
                        self.main_view = MainView::Editor;
                        ui.close();
                    }
                    if ui.button("Code Diff    Ctrl-D").clicked() {
                        self.code_diff_doc_idx = Some(self.current);
                        self.main_view = MainView::CodeDiff;
                        ui.close();
                    }
                    if ui.button("Terminal    Ctrl-T").clicked() {
                        self.show_terminal = !self.show_terminal;
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
                let mut toggle_dir = None;
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for entry in &self.workspace.entries {
                        ui.horizontal(|ui| {
                            ui.add_space((entry.depth * 14) as f32);
                            if entry.is_dir {
                                let collapsed = self.workspace.is_collapsed(&entry.path);
                                let arrow = if collapsed { ">" } else { "v" };
                                let label = format!("{} {}/", arrow, entry.name);
                                let text =
                                    egui::RichText::new(label).color(egui::Color32::from_rgb(142, 160, 184));
                                let response = ui.selectable_label(false, text);
                                if response.clicked() {
                                    toggle_dir = Some(entry.path.clone());
                                }
                            } else {
                                // Show file icon hint based on extension
                                let icon = file_icon_hint(&entry.name);
                                let label = format!("{} {}", icon, entry.name);
                                let is_open = self
                                    .documents
                                    .iter()
                                    .any(|doc| doc.path.as_deref() == Some(entry.path.as_path()));
                                let color = if is_open {
                                    egui::Color32::from_rgb(143, 179, 255)
                                } else {
                                    egui::Color32::from_rgb(216, 222, 233)
                                };
                                let text = egui::RichText::new(label).color(color);
                                let response = ui.selectable_label(false, text);
                                if response.clicked() {
                                    open_path = Some(entry.path.clone());
                                }
                            }
                        });
                    }
                });
                if let Some(path) = toggle_dir {
                    self.workspace.toggle_collapsed(&path);
                    let _ = self.workspace.refresh(self.settings.show_hidden_files);
                }
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
        match self.main_view {
            MainView::Editor => self.show_editor(ui),
            MainView::GitReview => self.show_git_review_panel(ui),
            MainView::CodeDiff => self.show_code_diff_panel(ui),
            MainView::GitConfig => self.show_git_config_panel(ui),
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
                let selected = idx == self.current && self.main_view == MainView::Editor;
                let has_diff = doc.has_diff_changes();

                let title_text = if has_diff {
                    egui::RichText::new(doc.display_title()).color(egui::Color32::from_rgb(255, 200, 100))
                } else {
                    egui::RichText::new(doc.display_title())
                };

                if ui.selectable_label(selected, title_text).clicked() {
                    self.current = idx;
                    self.main_view = MainView::Editor;
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

    // ---- Code Diff Panel ----
    fn show_code_diff_panel(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("Code Diff")
                            .size(24.0)
                            .strong()
                            .color(egui::Color32::from_rgb(226, 237, 248)),
                    );
                    ui.label(
                        egui::RichText::new(
                            "Shows changes between the last snapshot and the current text. Use Check All or Ctrl-D to clear.",
                        )
                        .color(egui::Color32::from_rgb(142, 160, 184)),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Editor").clicked() {
                        self.main_view = MainView::Editor;
                    }
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("Check All").strong())
                                .fill(egui::Color32::from_rgb(29, 78, 64)),
                        )
                        .on_hover_text("Clear all diffs across every open file")
                        .clicked()
                    {
                        self.clear_all_diffs();
                    }
                });
            });

            ui.add_space(10.0);

            // Summary tiles
            let total_files_changed: usize = self.documents.iter().filter(|doc| doc.has_diff_changes()).count();
            let total_added: usize = self.documents.iter().map(|doc| doc.diff_added_count()).sum();
            let total_removed: usize = self.documents.iter().map(|doc| doc.diff_removed_count()).sum();
            ui.horizontal(|ui| {
                summary_tile(ui, "Files changed", &total_files_changed.to_string());
                summary_tile(ui, "Lines added", &format!("+{total_added}"));
                summary_tile(ui, "Lines removed", &format!("-{total_removed}"));
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                // Left: file list with diff indicators
                ui.vertical(|ui| {
                    ui.set_width(280.0);
                    ui.group(|ui| {
                        ui.heading("Changed files");
                        let docs_with_changes: Vec<(usize, String, usize)> = self
                            .documents
                            .iter()
                            .enumerate()
                            .filter(|(_, doc)| doc.has_diff_changes())
                            .map(|(idx, doc)| (idx, doc.display_title(), doc.diff_line_count()))
                            .collect();

                        if docs_with_changes.is_empty() {
                            ui.add_space(8.0);
                            ui.label("No local changes detected.");
                        }

                        egui::ScrollArea::vertical().max_height(430.0).show(ui, |ui| {
                            for (idx, title, count) in &docs_with_changes {
                                let selected = self.code_diff_doc_idx == Some(*idx);
                                let label = format!("{} ({} changes)", title, count);
                                let response = ui.selectable_label(
                                    selected,
                                    egui::RichText::new(label)
                                        .monospace()
                                        .color(egui::Color32::from_rgb(255, 200, 100)),
                                );
                                if response.clicked() {
                                    self.code_diff_doc_idx = Some(*idx);
                                }
                            }
                        });

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    egui::Button::new(egui::RichText::new("Check All").strong())
                                        .fill(egui::Color32::from_rgb(29, 78, 64)),
                                )
                                .clicked()
                            {
                                self.clear_all_diffs();
                            }
                            if ui.button("Clear selected").clicked() {
                                if let Some(idx) = self.code_diff_doc_idx {
                                    if let Some(doc) = self.documents.get_mut(idx) {
                                        doc.clear_diff();
                                        self.status = format!("Diff cleared for {}", doc.title);
                                    }
                                }
                            }
                        });
                    });
                });

                ui.separator();

                // Right: diff view for the selected document
                ui.vertical(|ui| {
                    ui.set_min_width(650.0);
                    ui.group(|ui| {
                        let selected_title = self
                            .code_diff_doc_idx
                            .and_then(|idx| self.documents.get(idx))
                            .map(|doc| doc.display_title())
                            .unwrap_or_else(|| "No file selected".to_string());

                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&selected_title)
                                    .size(18.0)
                                    .strong()
                                    .color(egui::Color32::from_rgb(226, 237, 248)),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Clear this diff").clicked() {
                                    if let Some(idx) = self.code_diff_doc_idx {
                                        if let Some(doc) = self.documents.get_mut(idx) {
                                            doc.clear_diff();
                                            self.status = format!("Diff cleared for {}", doc.title);
                                        }
                                    }
                                }
                            });
                        });
                        ui.separator();

                        // Render the line-by-line diff
                        let diff_lines = self
                            .code_diff_doc_idx
                            .and_then(|idx| self.documents.get(idx))
                            .map(|doc| doc.diff_lines())
                            .unwrap_or_default();

                        if diff_lines.is_empty() || !diff_lines.iter().any(|d| d.kind != LineDiffKind::Unchanged) {
                            ui.add_space(16.0);
                            ui.label("No changes in this file.");
                        } else {
                            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                                draw_code_diff_lines(ui, &diff_lines);
                            });
                        }
                    });
                });
            });
        });
    }

    // ---- Git Config Panel ----
    fn show_git_config_panel(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("Git Configuration")
                            .size(24.0)
                            .strong()
                            .color(egui::Color32::from_rgb(226, 237, 248)),
                    );
                    ui.label(
                        egui::RichText::new(
                            "Configure your Git identity and preferences. This will be applied to the current repository.",
                        )
                        .color(egui::Color32::from_rgb(142, 160, 184)),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Editor").clicked() {
                        self.main_view = MainView::Editor;
                    }
                });
            });

            ui.add_space(16.0);

            ui.horizontal(|ui| {
                // Left: Config form
                ui.vertical(|ui| {
                    ui.set_width(440.0);
                    ui.group(|ui| {
                        ui.heading("Git Identity");
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("User name").strong().color(egui::Color32::from_rgb(216, 222, 233)));
                            ui.add_space(20.0);
                            ui.add(
                                egui::TextEdit::singleline(&mut self.git_config_name)
                                    .hint_text("Your Name")
                                    .desired_width(260.0),
                            );
                        });
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("User email").strong().color(egui::Color32::from_rgb(216, 222, 233)));
                            ui.add_space(18.0);
                            ui.add(
                                egui::TextEdit::singleline(&mut self.git_config_email)
                                    .hint_text("you@example.com")
                                    .desired_width(260.0),
                            );
                        });
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Default branch")
                                    .strong()
                                    .color(egui::Color32::from_rgb(216, 222, 233)),
                            );
                            ui.add(
                                egui::TextEdit::singleline(&mut self.git_config_branch)
                                    .hint_text("main")
                                    .desired_width(260.0),
                            );
                        });

                        ui.add_space(12.0);

                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    egui::Button::new(egui::RichText::new("Apply config").strong())
                                        .fill(egui::Color32::from_rgb(29, 78, 64)),
                                )
                                .clicked()
                            {
                                // Save to settings
                                self.settings.git_config.user_name = self.git_config_name.clone();
                                self.settings.git_config.user_email = self.git_config_email.clone();
                                self.settings.git_config.default_branch = self.git_config_branch.clone();
                                self.settings.git_config.configured = true;
                                let _ = self.settings.save();

                                // Apply to repository
                                if let Some(repo) = self.git_repo.clone() {
                                    match git::apply_config(&repo, &self.git_config_name, &self.git_config_email) {
                                        Ok(()) => {
                                            self.git_config_status = "Configuration applied to repository".to_string();
                                            self.status = "Git config applied".to_string();
                                        }
                                        Err(err) => {
                                            self.git_config_status = err.clone();
                                            self.status = err;
                                        }
                                    }
                                } else {
                                    self.git_config_status =
                                        "Settings saved. Open a Git repository to apply.".to_string();
                                }
                            }

                            if ui.button("Init repository").on_hover_text("Run git init in the workspace root").clicked() {
                                if let Some(root) = self.workspace.root.clone() {
                                    match git::init_repo(&root) {
                                        Ok(()) => {
                                            self.git_config_status = "Repository initialized".to_string();
                                            self.status = "Git repository initialized".to_string();
                                            self.refresh_git();
                                            // Auto-apply config if set
                                            if !self.git_config_name.is_empty() && !self.git_config_email.is_empty() {
                                                let _ = git::apply_config(&root, &self.git_config_name, &self.git_config_email);
                                            }
                                        }
                                        Err(err) => {
                                            self.git_config_status = err.clone();
                                            self.status = err;
                                        }
                                    }
                                } else {
                                    self.git_config_status = "Open a folder first".to_string();
                                }
                            }
                        });

                        if !self.git_config_status.is_empty() {
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new(&self.git_config_status)
                                    .color(egui::Color32::from_rgb(85, 224, 212)),
                            );
                        }

                        // Show config state
                        ui.add_space(12.0);
                        ui.separator();
                        ui.label(
                            egui::RichText::new("Current state")
                                .small()
                                .color(egui::Color32::from_rgb(142, 160, 184)),
                        );
                        let configured = self.settings.git_config.configured;
                        ui.label(
                            egui::RichText::new(if configured { "Configured" } else { "Not configured" })
                                .strong()
                                .color(if configured {
                                    egui::Color32::from_rgb(85, 224, 212)
                                } else {
                                    egui::Color32::from_rgb(245, 190, 190)
                                }),
                        );
                        if let Some(repo) = &self.git_repo {
                            ui.label(format!("Repository: {}", repo.display()));
                        } else {
                            ui.label("No repository detected");
                        }
                    });
                });

                ui.separator();

                // Right: Branch management and log
                ui.vertical(|ui| {
                    ui.set_min_width(400.0);

                    // Branch management
                    ui.group(|ui| {
                        ui.heading("Branches");
                        ui.add_space(4.0);

                        if !self.git_branch.is_empty() {
                            ui.label(
                                egui::RichText::new(format!("Current: {}", self.git_branch))
                                    .strong()
                                    .color(egui::Color32::from_rgb(85, 224, 212)),
                            );
                        }

                        ui.add_space(4.0);

                        // List branches
                        if !self.git_branches.is_empty() {
                            egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                                let mut switch_to = None;
                                for branch_name in &self.git_branches {
                                    let is_current = *branch_name == self.git_branch;
                                    let color = if is_current {
                                        egui::Color32::from_rgb(85, 224, 212)
                                    } else {
                                        egui::Color32::from_rgb(216, 222, 233)
                                    };
                                    let response = ui.selectable_label(
                                        is_current,
                                        egui::RichText::new(branch_name).monospace().color(color),
                                    );
                                    if response.clicked() && !is_current {
                                        switch_to = Some(branch_name.clone());
                                    }
                                }
                                if let Some(name) = switch_to {
                                    if let Some(repo) = self.git_repo.clone() {
                                        match git::switch_branch(&repo, &name) {
                                            Ok(()) => {
                                                self.status = format!("Switched to branch {name}");
                                                self.refresh_git();
                                            }
                                            Err(err) => self.status = err,
                                        }
                                    }
                                }
                            });
                        }

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.new_branch_name)
                                    .hint_text("new-branch-name")
                                    .desired_width(200.0),
                            );
                            if ui.button("Create branch").clicked() {
                                if let Some(repo) = self.git_repo.clone() {
                                    match git::create_branch(&repo, &self.new_branch_name) {
                                        Ok(()) => {
                                            self.status = format!("Created branch {}", self.new_branch_name);
                                            self.new_branch_name.clear();
                                            self.refresh_git();
                                        }
                                        Err(err) => self.status = err,
                                    }
                                }
                            }
                        });
                    });

                    ui.add_space(8.0);

                    // Recent commits
                    ui.group(|ui| {
                        ui.heading("Recent commits");
                        ui.add_space(4.0);

                        if self.git_log.is_empty() {
                            ui.label("No commits yet, or not a Git repository.");
                        } else {
                            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                                for entry in &self.git_log {
                                    ui.label(
                                        egui::RichText::new(entry)
                                            .monospace()
                                            .color(egui::Color32::from_rgb(216, 222, 233)),
                                    );
                                }
                            });
                        }
                    });
                });
            });
        });
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
                        self.main_view = MainView::Editor;
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
                            if ui.button("Stage all").clicked() {
                                if let Some(repo) = self.git_repo.clone() {
                                    match git::stage_all(&repo) {
                                        Ok(()) => {
                                            self.status = "All files staged".to_string();
                                            self.refresh_git();
                                        }
                                        Err(err) => self.status = err,
                                    }
                                }
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
                        ui.horizontal(|ui| {
                            if ui.button("Commit").clicked() {
                                self.commit_staged();
                            }
                            if ui.button("Stage all + Commit").clicked() {
                                self.stage_all_and_commit();
                            }
                        });
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
                // Show local diff count
                let diff_count: usize = self.documents.iter().map(|doc| doc.diff_line_count()).sum();
                if diff_count > 0 {
                    ui.label(
                        egui::RichText::new(format!("diff: {diff_count}"))
                            .color(egui::Color32::from_rgb(255, 200, 100)),
                    );
                    ui.separator();
                }
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
                ui.label("Freeform: open <path>, folder <path>, git, review, diff, config.");
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
            ui.monospace("Ctrl-D          Clear current code diff");
            ui.monospace("Ctrl-T          Integrated Terminal");
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

    fn poll_terminal(&mut self) {
        if let Some(rx) = &self.terminal_receiver {
            if let Ok(output) = rx.try_recv() {
                self.terminal_output.push_str(&output);
                self.terminal_output.push_str("\n");
                self.terminal_running = false;
                self.terminal_receiver = None;
            }
        }
    }

    fn execute_terminal_command(&mut self) {
        let command_str = self.terminal_input.trim().to_string();
        if command_str.is_empty() {
            return;
        }
        self.terminal_output.push_str(&format!("$ {}\n", command_str));
        self.terminal_input.clear();
        self.terminal_running = true;

        let (tx, rx) = std::sync::mpsc::channel();
        self.terminal_receiver = Some(rx);

        let workspace_root = self.workspace.root.clone();

        std::thread::spawn(move || {
            let mut cmd = if cfg!(target_os = "windows") {
                let mut c = std::process::Command::new("cmd");
                c.args(["/C", &command_str]);
                c
            } else {
                let mut c = std::process::Command::new("sh");
                c.args(["-c", &command_str]);
                c
            };

            if let Some(root) = workspace_root {
                cmd.current_dir(root);
            }

            match cmd.output() {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let _ = tx.send(format!("{}{}", stdout, stderr));
                }
                Err(err) => {
                    let _ = tx.send(format!("Error executing command: {}\n", err));
                }
            }
        });
    }

    fn show_terminal_panel(&mut self, ui: &mut egui::Ui) {
        if !self.show_terminal {
            return;
        }

        egui::Panel::bottom("terminal_panel").resizable(true).default_size(180.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Terminal").strong().color(egui::Color32::from_rgb(226, 237, 248)));
                if self.terminal_running {
                    ui.spinner();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Clear").clicked() {
                        self.terminal_output.clear();
                    }
                    if ui.button("Close").clicked() {
                        self.show_terminal = false;
                    }
                });
            });
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Scrollable terminal output history
            egui::ScrollArea::vertical().max_height(120.0).auto_shrink([false, false]).show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.terminal_output)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .interactive(false),
                );
            });

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Interactive command input
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("$").strong().color(egui::Color32::from_rgb(99, 137, 255)));
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.terminal_input)
                        .hint_text("Enter terminal command (e.g. ls, cargo build)")
                        .desired_width(f32::INFINITY),
                );

                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.execute_terminal_command();
                    response.request_focus(); // keep focus on terminal input!
                }
            });
        });
    }
}

impl eframe::App for OrionApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_terminal();
        let ctx = ui.ctx().clone();
        self.handle_shortcuts(&ctx);
        self.show_top_bar(ui);
        self.show_status_bar(ui);
        self.show_workspace_panel(ui);
        self.show_main_area(ui);
        self.show_terminal_panel(ui);
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
    let ring_a = egui::Color32::from_rgb(203, 213, 225);
    let ring_b = egui::Color32::from_rgb(100, 116, 139);
    // Border color reserved for future ring outline
    let _border = egui::Color32::from_rgb(30, 41, 59);

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

fn draw_code_diff_lines(ui: &mut egui::Ui, lines: &[crate::document::LineDiff]) {
    egui::Grid::new("code_diff_grid").num_columns(4).spacing([8.0, 3.0]).striped(true).show(ui, |ui| {
        ui.strong("Line");
        ui.strong("Status");
        ui.strong("Old");
        ui.strong("New");
        ui.end_row();

        for line in lines {
            if line.kind == LineDiffKind::Unchanged {
                continue; // Only show changed lines
            }
            let (status_text, status_color, bg_color) = match line.kind {
                LineDiffKind::Added => ("+ added", egui::Color32::from_rgb(180, 230, 190), egui::Color32::from_rgb(20, 64, 42)),
                LineDiffKind::Removed => ("- removed", egui::Color32::from_rgb(245, 190, 190), egui::Color32::from_rgb(74, 34, 38)),
                LineDiffKind::Modified => ("~ modified", egui::Color32::from_rgb(200, 200, 130), egui::Color32::from_rgb(60, 55, 20)),
                LineDiffKind::Unchanged => continue,
            };

            ui.monospace(line.line_number.to_string());
            ui.label(egui::RichText::new(status_text).monospace().small().color(status_color));
            ui.label(
                egui::RichText::new(&line.old_text)
                    .monospace()
                    .color(egui::Color32::from_rgb(245, 190, 190))
                    .background_color(if line.kind == LineDiffKind::Removed { bg_color } else { egui::Color32::TRANSPARENT }),
            );
            ui.label(
                egui::RichText::new(&line.new_text)
                    .monospace()
                    .color(egui::Color32::from_rgb(180, 230, 190))
                    .background_color(if line.kind == LineDiffKind::Added { bg_color } else { egui::Color32::TRANSPARENT }),
            );
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

/// Return a simple text-based icon hint for a file based on its extension.
fn file_icon_hint(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "rs" => "[rs]",
        "c" | "h" => "[c]",
        "cc" | "cpp" | "cxx" | "hpp" | "hxx" => "[c++]",
        "zig" => "[zig]",
        "py" => "[py]",
        "js" | "mjs" => "[js]",
        "ts" | "tsx" => "[ts]",
        "html" | "htm" => "[html]",
        "css" => "[css]",
        "json" => "[json]",
        "toml" => "[toml]",
        "yaml" | "yml" => "[yaml]",
        "md" | "markdown" => "[md]",
        "sh" | "bash" => "[sh]",
        "txt" => "[txt]",
        "lock" => "[lock]",
        "svg" => "[svg]",
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" => "[img]",
        _ => "[ ]",
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
