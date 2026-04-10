param(
  [string]$KeyPath = "$env:USERPROFILE\.tauri\taskReminder-updater.key",
  [string]$ReleaseNotes = "",
  [string]$KeyPassword = ""
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $KeyPath)) {
  throw "Updater key not found: $KeyPath"
}

# Set signing key and password so child processes (tauri build) can read them
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content -LiteralPath $KeyPath -Raw).Trim()
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $KeyPassword

if ([string]::IsNullOrWhiteSpace($ReleaseNotes)) {
  Remove-Item Env:RELEASE_NOTES -ErrorAction SilentlyContinue
} else {
  $env:RELEASE_NOTES = $ReleaseNotes
}

Write-Host "[updater] TAURI_SIGNING_PRIVATE_KEY is set ($($env:TAURI_SIGNING_PRIVATE_KEY.Length) chars)"
Write-Host "[updater] Starting tauri build..."

# Use Start-Process to ensure env vars propagate correctly, or invoke directly
# pnpm.cmd is a batch wrapper; call it via cmd /c to guarantee env inheritance
cmd /c "pnpm.cmd tauri build --bundles msi"

if ($LASTEXITCODE -ne 0) {
  throw "tauri build failed with exit code $LASTEXITCODE"
}

node scripts/write-updater-manifest.mjs

if ($LASTEXITCODE -ne 0) {
  throw "write-updater-manifest failed with exit code $LASTEXITCODE"
}

# Summary
$msiDir = "src-tauri\target\release\bundle\msi"
Write-Host ""
Write-Host "[updater] Artifacts ready in $msiDir :"
Get-ChildItem -Path $msiDir | ForEach-Object { Write-Host "  $_  ($([math]::Round($_.Length / 1KB, 1)) KB)" }
