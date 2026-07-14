#!/usr/bin/env sh
set -eu
cargo build --release --no-default-features
mkdir -p release
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)
cp target/release/orion "release/orion-lite-$OS-$ARCH"
chmod +x "release/orion-lite-$OS-$ARCH"
echo "Created release/orion-lite-$OS-$ARCH"
