# Architecture

Orion is a native desktop IDE written in Rust.

Theme:

```text
IDE not for you, but for your agents.
```

## Design priorities

1. Low startup time
2. Low idle CPU usage
3. Low memory usage
4. No web runtime
5. No telemetry
6. Persistent project memory without copying project files
7. Agent-first Git review workflow
8. Simple editing for low-level languages

## Main modules

| Module | Purpose |
| --- | --- |
| `main.rs` | Starts the native eframe application |
| `app.rs` | Main IDE state, UI layout, tabs, panels, Git Review, settings, actions |
| `document.rs` | Open, save, and track editor documents |
| `workspace.rs` | Workspace folder scanning and file explorer entries |
| `syntax.rs` | Lightweight syntax highlighting |
| `git.rs` | Git status, branch, stage, unstage, commit, and diff parsing |
| `settings.rs` | Config loading, project memory, Done markers, theme settings |
| `security.rs` | File validation, ignored directories, and safe path helpers |
| `command.rs` | Command palette actions |

## UI layout

Orion uses egui panels:

- Top menu bar
- Left Project explorer
- Central editor or Git Review screen
- Bottom status bar
- Floating windows for command palette, search, settings, confirmations, and help

There is no output window for running code. Orion intentionally avoids acting as a build console or terminal multiplexer.

## Git support

Orion uses the installed `git` binary through `std::process::Command`. Commands are executed directly with arguments, not through a shell.

Supported GUI Git actions:

- Detect repository root
- Show current branch
- Show changed files
- Show staged or unstaged status
- Stage selected file
- Unstage selected file
- Commit staged changes
- Parse unified diffs
- Render side-by-side diffs
- Mark files Done for agent review

## Done markers

A Done marker stores:

- Repository path
- File path
- Change fingerprint

It does not store or copy source code. If a file changes again, the fingerprint changes and the file can appear again for review.

## Workspace scanning

Workspace scanning uses the standard library and avoids following symlinks. Heavy and generated directories are ignored by default, including `.git`, `target`, `node_modules`, `build`, and `dist`.

## Syntax highlighting

Syntax highlighting is intentionally lightweight. It is a small lexical highlighter for common low-level languages. It does not require a language server or background indexer, and it avoids allocating keyword sets during each editor layout pass.

## Persistence

Settings are stored as TOML in the user's platform-specific config directory through the `directories` crate. Orion remembers only settings and paths, not project contents.
