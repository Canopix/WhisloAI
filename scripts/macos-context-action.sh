#!/bin/sh
set -eu

SELECTED_TEXT="${1-}"

if [ -z "${SELECTED_TEXT}" ]; then
  exit 0
fi

TMP_FILE="$(mktemp /tmp/whisloai-input.XXXXXX.txt)"
printf "%s" "${SELECTED_TEXT}" > "${TMP_FILE}"

open -na "WhisloAI" --args --improve-text-file "${TMP_FILE}"

# Clean temp file after the app has had time to consume it.
(
  sleep 30
  rm -f "${TMP_FILE}"
) >/dev/null 2>&1 &
