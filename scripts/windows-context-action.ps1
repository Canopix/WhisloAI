param(
  [string]$Text = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-BestTextExecutable {
  $candidates = @()

  if ($env:BESTTEXT_EXE) {
    $candidates += $env:BESTTEXT_EXE
  }

  $candidates += @(
    "$env:LOCALAPPDATA\Programs\BestText\BestText.exe",
    "$env:ProgramFiles\BestText\BestText.exe",
    (Join-Path $PSScriptRoot "..\src-tauri\target\release\app.exe"),
    (Join-Path $PSScriptRoot "..\src-tauri\target\debug\app.exe")
  )

  foreach ($path in $candidates) {
    if ($path -and (Test-Path $path)) {
      return (Resolve-Path $path).Path
    }
  }

  throw "BestText executable not found. Set BESTTEXT_EXE or install the app first."
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

$bestTextExe = Resolve-BestTextExecutable
$tempPath = Join-Path $env:TEMP ("besttext-selection-" + [guid]::NewGuid().ToString("N") + ".txt")
[System.IO.File]::WriteAllText($tempPath, $Text, [System.Text.UTF8Encoding]::new($false))

Start-Process -FilePath $bestTextExe -ArgumentList @("--improve-text-file", $tempPath) | Out-Null
