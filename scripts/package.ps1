$ErrorActionPreference = "Stop"
$Profile = if ($env:ORION_PROFILE) { $env:ORION_PROFILE } else { "release" }
if (Test-Path "Cargo.lock") {
    cargo build --profile $Profile --locked
} else {
    cargo build --profile $Profile
}
New-Item -ItemType Directory -Force -Path "release" | Out-Null
$Out = "release\orion-windows-x64.exe"
Copy-Item "target\$Profile\orion.exe" $Out -Force
Write-Host "Created $Out"
