## Summary

Describe the change.

## Testing

List commands you ran.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## Security impact

Does this change add command execution, network access, file-system access, plugins, or dependency changes? If yes, explain the impact.
