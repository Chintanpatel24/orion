# User guide

Orion is an agent-first IDE. It is built around editing and Git review instead of a built-in run-output panel.

## Open Orion

Run:

```sh
orion
```

Open a project directly:

```sh
orion /path/to/project
```

## Persistent project memory

When you open a project folder, Orion stores that folder path in the user settings file. The same project opens again next time you start Orion. It remains the remembered project until you choose another project folder.

Orion does not copy your project files into the Orion folder. It only stores settings such as the remembered project path and Done review fingerprints.

## Open a project folder

Use `Ctrl-Shift-O` or choose `File > Open folder`.

The Project panel shows the files. Generated folders such as `.git`, `target`, `node_modules`, `build`, and `dist` are hidden by default.

## Open a file

Use `Ctrl-O`, `File > Open file`, or click a file in the Project panel.

## Save files

Use `Ctrl-S` for Save and `Ctrl-Shift-S` for Save As.

## Agent Git Review

Use `Ctrl-G` or choose `Git > Review changes`.

The Git Review screen shows:

- Repository path
- Branch name
- Changed files
- Staged or unstaged state
- Side-by-side diff
- Stage selected
- Unstage selected
- Done
- Not done
- Commit staged changes

## Done button

Click `Done` when a file has been reviewed. With `Hide done` enabled, Orion hides that file from the changed-file list.

A Done marker stores only:

- Repository path
- File path
- Change fingerprint

If the file changes again, the fingerprint changes and the file can appear again for review.

## Search and replace

Use `Ctrl-F` to open the search window. Enter text to count matches, then use Replace All if needed.

## Command palette

Use `Ctrl-P`. You can select built-in actions or type commands:

```text
open src/main.c
folder /path/to/project
git
review
```

## Settings

Open `View > Settings` or use the command palette. Settings include:

- Theme
- Font size
- Tab size
- Syntax highlighting
- Highlighting size limit
- Low-power mode for very old hardware
- Hidden file visibility
- Maximum file size
- Hide files marked Done
