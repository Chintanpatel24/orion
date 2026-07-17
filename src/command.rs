#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteAction {
    NewFile,
    OpenFile,
    OpenFolder,
    Save,
    SaveAs,
    SaveAll,
    GitReview,
    RefreshGit,
    Search,
    RefreshWorkspace,
    Settings,
    Help,
    Terminal,
}

#[derive(Debug, Clone, Copy)]
pub struct PaletteItem {
    pub name: &'static str,
    pub detail: &'static str,
    pub action: PaletteAction,
}

pub fn palette_items() -> &'static [PaletteItem] {
    &[
        PaletteItem { name: "New file", detail: "Create a new empty editor tab", action: PaletteAction::NewFile },
        PaletteItem { name: "Open file", detail: "Open a source file", action: PaletteAction::OpenFile },
        PaletteItem {
            name: "Open folder",
            detail: "Open a persistent workspace folder",
            action: PaletteAction::OpenFolder,
        },
        PaletteItem { name: "Save", detail: "Save the current file", action: PaletteAction::Save },
        PaletteItem { name: "Save as", detail: "Save the current file to a new path", action: PaletteAction::SaveAs },
        PaletteItem { name: "Save all", detail: "Save every open file", action: PaletteAction::SaveAll },
        PaletteItem {
            name: "Git review",
            detail: "Open the agent-first Git diff review window",
            action: PaletteAction::GitReview,
        },
        PaletteItem {
            name: "Refresh Git",
            detail: "Reload branch and changed files",
            action: PaletteAction::RefreshGit,
        },
        PaletteItem { name: "Search", detail: "Find text in the current file", action: PaletteAction::Search },
        PaletteItem {
            name: "Refresh workspace",
            detail: "Reload the file explorer",
            action: PaletteAction::RefreshWorkspace,
        },
        PaletteItem {
            name: "Terminal",
            detail: "Toggle the integrated system terminal",
            action: PaletteAction::Terminal,
        },
        PaletteItem { name: "Settings", detail: "Open editor settings", action: PaletteAction::Settings },
        PaletteItem { name: "Help", detail: "Show important shortcuts", action: PaletteAction::Help },
    ]
}

pub fn matches_query(item: &PaletteItem, query: &str) -> bool {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return true;
    }
    item.name.to_ascii_lowercase().contains(&query) || item.detail.to_ascii_lowercase().contains(&query)
}
