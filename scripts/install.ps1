$ErrorActionPreference = "Stop"

$RepoUrl = if ($env:ORION_REPO_URL) { $env:ORION_REPO_URL } else { "https://github.com/orion-ide/orion.git" }
$Profile = if ($env:ORION_PROFILE) { $env:ORION_PROFILE } else { "release" }
$InstallDir = if ($env:ORION_INSTALL_DIR) { $env:ORION_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "Orion" }
$CacheDir = Join-Path $env:TEMP "orion-src"

function Need-Command($Name, $Message) {
    if (!(Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Missing required command: $Name. $Message"
    }
}

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$CandidateRoot = Resolve-Path (Join-Path $ScriptDir "..") -ErrorAction SilentlyContinue

if ($CandidateRoot -and (Test-Path (Join-Path $CandidateRoot "Cargo.toml"))) {
    $SourceDir = $CandidateRoot.Path
} elseif (Test-Path ".\Cargo.toml") {
    $SourceDir = (Resolve-Path ".").Path
} else {
    Need-Command git "Install git or download the Orion source archive manually."
    if (Test-Path $CacheDir) { Remove-Item -Recurse -Force $CacheDir }
    git clone --depth 1 $RepoUrl $CacheDir
    $SourceDir = $CacheDir
}

Need-Command cargo "Install Rust from https://rustup.rs, then run this installer again."

Push-Location $SourceDir
try {
    if (Test-Path "Cargo.lock") {
        cargo build --profile $Profile --locked
    } else {
        cargo build --profile $Profile
    }
} finally {
    Pop-Location
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$ExeSource = Join-Path $SourceDir "target\$Profile\orion.exe"
$ExeDest = Join-Path $InstallDir "orion.exe"
Copy-Item $ExeSource $ExeDest -Force

$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($null -eq $UserPath) { $UserPath = "" }
$PathParts = $UserPath.Split(';', [System.StringSplitOptions]::RemoveEmptyEntries)
if ($PathParts -notcontains $InstallDir) {
    $NewPath = if ([string]::IsNullOrWhiteSpace($UserPath)) { $InstallDir } else { "$UserPath;$InstallDir" }
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    Write-Host "Added $InstallDir to the user PATH. Open a new terminal before running orion."
}

$StartMenu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
$ShortcutPath = Join-Path $StartMenu "Orion IDE.lnk"
try {
    $Shell = New-Object -ComObject WScript.Shell
    $Shortcut = $Shell.CreateShortcut($ShortcutPath)
    $Shortcut.TargetPath = $ExeDest
    $Shortcut.WorkingDirectory = $InstallDir
    $Shortcut.Description = "Orion IDE"
    $Shortcut.Save()
} catch {
    Write-Host "Could not create Start Menu shortcut: $_"
}

Write-Host "Installed Orion IDE to $ExeDest"
