#Requires AutoHotkey v2.0

; Phase 1 Windows equivalent:
; Shift + Right Click on selected text -> copy selection -> send to BestText.
+RButton::
{
  A_Clipboard := ""
  Send "^c"
  if !ClipWait(0.35) {
    return
  }

  scriptPath := A_ScriptDir "\windows-context-action.ps1"
  Run('powershell.exe -NoProfile -ExecutionPolicy Bypass -File "' scriptPath '"')
}
