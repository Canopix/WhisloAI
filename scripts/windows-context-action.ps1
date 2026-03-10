param(
  [string]$Text = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-WhisloAIExecutable {
  $candidates = @()

  if ($env:WHISLOAI_EXE) {
    $candidates += $env:WHISLOAI_EXE
  }

  $candidates += @(
    "$env:LOCALAPPDATA\Programs\WhisloAI\WhisloAI.exe",
    "$env:ProgramFiles\WhisloAI\WhisloAI.exe",
    (Join-Path $PSScriptRoot "..\src-tauri\target\release\app.exe"),
    (Join-Path $PSScriptRoot "..\src-tauri\target\debug\app.exe")
  )

  foreach ($path in $candidates) {
    if ($path -and (Test-Path $path)) {
      return (Resolve-Path $path).Path
    }
  }

  throw "WhisloAI executable not found. Set WHISLOAI_EXE or install the app first."
}

if ([string]::IsNullOrWhiteSpace($Text)) {
  try {
    $Text = Get-Clipboard -Raw
  } catch {
    $Text = ""
  }
}

$Text = ($Text ?? "").Trim()
if ([string]::IsNullOrWhiteSpace($Text)) {
  Write-Host "No text found. Select text, copy it, and run again."
  exit 1
}

$whisloaiExe = Resolve-WhisloAIExecutable
$tempPath = Join-Path $env:TEMP ("whisloai-selection-" + [guid]::NewGuid().ToString("N") + ".txt")
[System.IO.File]::WriteAllText($tempPath, $Text, [System.Text.UTF8Encoding]::new($false))

Start-Process -FilePath $whisloaiExe -ArgumentList @("--improve-text-file", $tempPath) | Out-Null
