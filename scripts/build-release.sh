#!/usr/bin/env sh
set -eu
PROFILE=${ORION_PROFILE:-release}
if [ -f Cargo.lock ]; then
  cargo build --profile "$PROFILE" --locked
else
  cargo build --profile "$PROFILE"
fi
mkdir -p release
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)
BIN="target/$PROFILE/orion"
cp "$BIN" "release/orion-$OS-$ARCH"
chmod +x "release/orion-$OS-$ARCH"
echo "Created release/orion-$OS-$ARCH"
