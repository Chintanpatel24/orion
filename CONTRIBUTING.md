# Contributing

Thank you for helping improve Orion.

## Development setup

Install Rust from `https://rustup.rs`, then run:

```sh
cargo check --all-features
cargo test --all-features
cargo run
```

## Quality checks

Before opening a pull request, run:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## Pull request guidelines

- Keep changes focused
- Include tests for parsing, task execution helpers, and security-sensitive code when practical
- Update documentation when behavior changes
- Do not add telemetry
- Do not add network calls without an explicit security review
- Do not introduce shell execution for Git actions or future agent tasks

## Commit style

Use clear commit messages:

```text
area: short explanation
```

Examples:

```text
editor: improve tab closing behavior
git: improve side-by-side diff parsing
docs: update install guide
```
