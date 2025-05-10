param (
    [string]$mode = "dev"
)

$tauriPath = "src-tauri\tauri.conf.json"

if ($mode -eq "dev") {
    Write-Host "`nLaunching in DEV mode (hot reload)...`n"
    Copy-Item "src-tauri\tauri.dev.json" $tauriPath -Force
    cargo tauri dev
}
elseif ($mode -eq "static") {
    Write-Host "`nLaunching in STATIC mode (prebuilt frontend)...`n"
    Push-Location frontend
    npm run build
    Pop-Location
    Copy-Item "src-tauri\tauri.static.json" $tauriPath -Force
    cargo tauri build
}
else {
    Write-Host "`nUnknown mode: $mode"
    Write-Host "Usage: .\run.ps1 [dev|static]"
    exit 1
}
