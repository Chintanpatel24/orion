# Install guide

## Windows

Install Rust from `https://rustup.rs`, make sure Git is installed, then run:

```powershell
iwr -useb https://raw.githubusercontent.com/orion-ide/orion/main/scripts/install.ps1 | iex
```

Local source install:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\install.ps1
```

The installer places `orion.exe` in:

```text
%LOCALAPPDATA%\Orion
```

It also tries to add Orion to the user PATH and create a Start Menu shortcut.

## Linux

Install Rust from `https://rustup.rs`, make sure Git is installed, then run:

```sh
curl -fsSL https://raw.githubusercontent.com/orion-ide/orion/main/scripts/install.sh | sh
```

Local source install:

```sh
sh ./install.sh
```

The installer places the binary in:

```text
$HOME/.local/bin/orion
```

It also creates a desktop entry at:

```text
$HOME/.local/share/applications/orion.desktop
```

## macOS

Install Rust from `https://rustup.rs`, make sure Git is installed, then run:

```sh
curl -fsSL https://raw.githubusercontent.com/orion-ide/orion/main/scripts/install.sh | sh
```

Local source install:

```sh
sh ./install.sh
```

The installer places the binary in:

```text
$HOME/.local/bin/orion
```

It also creates a simple application bundle at:

```text
$HOME/Applications/Orion.app
```

## Custom repository URL

If you fork or publish Orion under another GitHub account, use:

Linux and macOS:

```sh
export ORION_REPO_URL=https://github.com/your-name/orion.git
curl -fsSL https://raw.githubusercontent.com/your-name/orion/main/scripts/install.sh | sh
```

Windows PowerShell:

```powershell
$env:ORION_REPO_URL="https://github.com/your-name/orion.git"; iwr -useb https://raw.githubusercontent.com/your-name/orion/main/scripts/install.ps1 | iex
```
