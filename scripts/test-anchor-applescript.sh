#!/bin/bash
# Test script: run this from Terminal while focusing a text field (e.g. in Notes).
# If it prints coordinates, AppleScript works. If it prints NONE or errors, there's a permission issue.

osascript -e '
set textRoles to {"AXTextField", "AXTextArea", "AXTextView"}
try
  tell application "System Events"
    set frontProcess to first application process whose frontmost is true
    tell frontProcess
      set focusedElement to value of attribute "AXFocusedUIElement"
      if focusedElement is missing value then return "NONE"
      set roleName to value of attribute "AXRole" of focusedElement
      if textRoles does not contain roleName then return "NONE"
      set p to value of attribute "AXPosition" of focusedElement
      set s to value of attribute "AXSize" of focusedElement
      set px to item 1 of p as integer
      set py to item 2 of p as integer
      set pw to item 1 of s as integer
      set ph to item 2 of s as integer
      return (px as string) & "," & (py as string) & "," & (pw as string) & "," & (ph as string)
    end tell
  end tell
on error errMsg
  return "ERROR: " & errMsg
end try
'
