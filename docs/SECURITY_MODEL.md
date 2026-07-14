# Security model

Orion is designed to be safe by default while still allowing users and agents to inspect local code efficiently.

## Non-goals

Orion is not a sandbox for untrusted code. It also intentionally does not include a built-in code runner output window. If you run programs or tests outside Orion, those programs use the permissions of the process that starts them.

## Default protections

- No telemetry
- No automatic network calls
- No extension runtime
- No plugin marketplace
- No built-in code runner output panel
- No automatic project script execution
- Git commands are executed directly, not through a shell
- Workspace scanner does not follow symlinks
- Generated and dependency directories are ignored by default
- Large file limit before opening
- Binary file detection before opening
- Done markers store only paths and fingerprints, not source code

## Git execution

Orion uses the installed `git` executable with `std::process::Command`.

For example, staging a file is executed as a program plus arguments:

```text
program: git
args: -C, repo, add, --, path
```

It is not executed as:

```text
sh -c "git add path"
```

This reduces shell injection risk.

## File opening

Before opening a file, Orion checks:

- The path points to a regular file
- The file is under the configured size limit
- The first bytes do not look like binary data

## Project memory

Orion remembers the last project folder path in settings. It does not copy all project files into Orion's folder.

## Done markers

Done markers are stored as repository path, file path, and change fingerprint. They are used only to hide reviewed files from the Git Review list while Hide done is enabled.

## Reporting security issues

See `SECURITY.md` in the repository root.
