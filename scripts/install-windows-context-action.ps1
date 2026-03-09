Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-AhkExecutable {
  $command = Get-Command AutoHotkey64.exe -ErrorAction SilentlyContinue
  if ($command) {
    return $command.Source
  }

  $command = Get-Command AutoHotkey.exe -ErrorAction SilentlyContinue
  if ($command) {
    return $command.Source
  }

  $paths = @(
    "$env:ProgramFiles\AutoHotkey\v2\AutoHotkey64.exe",
    "$env:ProgramFiles\AutoHotkey\AutoHotkey64.exe",
    "$env:ProgramFiles\AutoHotkey\v2\AutoHotkey.exe",
    "$env:ProgramFiles\AutoHotkey\AutoHotkey.exe"
  )

  foreach ($path in $paths) {
    if (Test-Path $path) {
      return $path
    }
  }

  throw "AutoHotkey v2 was not found. Install it from https://www.autohotkey.com/ first."
}

$repoRoot = Split-Path -Parent $PSScriptRoot
$sourcePs1 = Join-Path $PSScriptRoot "windows-context-action.ps1"
$sourceAhk = Join-Path $PSScriptRoot "windows-context-action.ahk"

if (!(Test-Path $sourcePs1) -or !(Test-Path $sourceAhk)) {
  throw "Required files were not found in scripts/."
}

$installDir = Join-Path $env:LOCALAPPDATA "BestText\WindowsContextAction"
New-Item -ItemType Directory -Path $installDir -Force | Out-Null

$targetPs1 = Join-Path $installDir "windows-context-action.ps1"
$targetAhk = Join-Path $installDir "windows-context-action.ahk"
Copy-Item $sourcePs1 $targetPs1 -Force
Copy-Item $sourceAhk $targetAhk -Force

$ahkExe = Resolve-AhkExecutable
$startupDir = [Environment]::GetFolderPath("Startup")
$shortcutPath = Join-Path $startupDir "BestText Context Action.lnk"

$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($shortcutPath)
$shortcut.TargetPath = $ahkExe
$shortcut.Arguments = '"' + $targetAhk + '"'
$shortcut.WorkingDirectory = $installDir
$shortcut.WindowStyle = 7
$shortcut.Description = "BestText Shift+RightClick context action"
$shortcut.Save()

Write-Host "Installed."
Write-Host "- Script folder: $installDir"
Write-Host "- Startup shortcut: $shortcutPath"
Write-Host "Use Shift + Right Click on selected text to send it to BestText."
