Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$installDir = Join-Path $env:LOCALAPPDATA "BestText\WindowsContextAction"
$startupDir = [Environment]::GetFolderPath("Startup")
$shortcutPath = Join-Path $startupDir "BestText Context Action.lnk"

if (Test-Path $shortcutPath) {
  Remove-Item $shortcutPath -Force
}

if (Test-Path $installDir) {
  Remove-Item $installDir -Recurse -Force
}

Write-Host "BestText Windows context action removed."
