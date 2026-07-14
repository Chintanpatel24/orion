#!/usr/bin/env sh
set -eu

REPO_URL=${ORION_REPO_URL:-https://github.com/orion-ide/orion.git}
PREFIX=${PREFIX:-$HOME/.local}
BINDIR=${BINDIR:-$PREFIX/bin}
PROFILE=${ORION_PROFILE:-release}
CACHE_DIR=${XDG_CACHE_HOME:-$HOME/.cache}/orion-src

need() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1"
    echo "$2"
    exit 1
  fi
}

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" 2>/dev/null && pwd || pwd)

if [ -f "$SCRIPT_DIR/../Cargo.toml" ]; then
  SRC_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
elif [ -f "./Cargo.toml" ]; then
  SRC_DIR=$(pwd)
else
  need git "Install git or download the Orion source archive manually."
  rm -rf "$CACHE_DIR"
  mkdir -p "$(dirname "$CACHE_DIR")"
  git clone --depth 1 "$REPO_URL" "$CACHE_DIR"
  SRC_DIR="$CACHE_DIR"
fi

need cargo "Install Rust from https://rustup.rs, then run this installer again."

cd "$SRC_DIR"
if [ -f Cargo.lock ]; then
  cargo build --profile "$PROFILE" --locked
else
  cargo build --profile "$PROFILE"
fi

mkdir -p "$BINDIR"
cp "target/$PROFILE/orion" "$BINDIR/orion"
chmod +x "$BINDIR/orion"

OS=$(uname -s 2>/dev/null || echo unknown)
if [ "$OS" = "Linux" ]; then
  APP_DIR=${XDG_DATA_HOME:-$HOME/.local/share}/applications
  ICON_DIR=${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps
  mkdir -p "$APP_DIR" "$ICON_DIR"
  if [ -f "$SRC_DIR/assets/dev.orion.Orion.svg" ]; then
    cp "$SRC_DIR/assets/dev.orion.Orion.svg" "$ICON_DIR/dev.orion.Orion.svg"
  fi
  cat > "$APP_DIR/orion.desktop" <<EOF_DESKTOP
[Desktop Entry]
Type=Application
Name=Orion IDE
Comment=Fast lightweight IDE for low-level languages
Exec=$BINDIR/orion
Icon=dev.orion.Orion
Terminal=false
Categories=Development;IDE;TextEditor;
EOF_DESKTOP
fi

if [ "$OS" = "Darwin" ]; then
  MAC_APP_DIR="$HOME/Applications/Orion.app"
  mkdir -p "$MAC_APP_DIR/Contents/MacOS"
  cp "$BINDIR/orion" "$MAC_APP_DIR/Contents/MacOS/orion"
  cat > "$MAC_APP_DIR/Contents/Info.plist" <<EOF_PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Orion IDE</string>
  <key>CFBundleIdentifier</key><string>dev.orion.ide</string>
  <key>CFBundleExecutable</key><string>orion</string>
  <key>CFBundlePackageType</key><string>APPL</string>
</dict>
</plist>
EOF_PLIST
fi

echo "Installed Orion IDE to $BINDIR/orion"
case ":$PATH:" in
  *":$BINDIR:"*) ;;
  *)
    echo "Add this to your shell profile if orion is not found:"
    echo "export PATH=\"$BINDIR:\$PATH\""
    ;;
esac
