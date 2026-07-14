# Performance notes

Orion is designed to stay light on modest and very old hardware.

## Choices that keep it fast

- Native Rust binary
- No Electron, Chromium, Node.js, or webview runtime
- No background indexer by default
- No telemetry service
- No extension host by default
- No built-in code runner output panel
- Workspace scanner skips generated and dependency directories
- Syntax highlighting is lightweight lexical highlighting with no per-frame keyword allocation
- Low-power mode can disable costly visual work on very old hardware
- Git commands run only when the user opens or refreshes Git Review
- Done markers are tiny path and fingerprint records, not copied source files
- Release profile enables optimization, thin LTO, single codegen unit, panic abort, and symbol stripping

## Suggested release profile

The default release profile is tuned for speed:

```sh
cargo build --release
```

The small release profile is tuned for binary size:

```sh
cargo build --profile release-small
```

The lightest dependency profile disables native file dialogs:

```sh
cargo build --release --no-default-features
```

## Practical target

Orion should feel responsive on old laptops, small virtual machines, low-memory systems, and remote development machines where heavier IDEs are uncomfortable.

## What Orion avoids

Orion intentionally avoids features that commonly make IDEs heavy unless they become optional later:

- Always-on project indexing
- Built-in web browser runtime
- Background extension processes
- Built-in run terminal or output window
- Automatic cloud sync
- Telemetry uploaders
- Auto-running project scripts
