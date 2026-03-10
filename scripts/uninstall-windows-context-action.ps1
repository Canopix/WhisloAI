Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$installDir = Join-Path $env:LOCALAPPDATA "WhisloAI\WindowsContextAction"
$startupDir = [Environment]::GetFolderPath("Startup")
$shortcutPath = Join-Path $startupDir "WhisloAI Context Action.lnk"

if (Test-Path $shortcutPath) {
  Remove-Item $shortcutPath -Force
}

if (Test-Path $installDir) {
  Remove-Item $installDir -Recurse -Force
}

Write-Host "WhisloAI Windows context action removed."
