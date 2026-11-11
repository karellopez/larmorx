#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Runs the README's and the documentation site's PowerShell examples ("An example, start to end")
# on the Windows helper `win-laptop` (docs/helper-machines.md: key-based SSH, cmd.exe as the
# login shell), in Windows PowerShell 5.1, as a user types them (win_examples.py writes the
# script): a new folder %USERPROFILE%\<folder> (it must not exist yet) with the script and this
# branch's Windows wheel in files\, the README's example with that wheel for `pip install
# larmorx`, the site's as it is (0.1.0rc1 from PyPI), each in its own subfolder; then the folder
# removed and checked gone. Nothing outside the folder is written (pip's cache, the temporary
# folder and Python's bytecode are kept inside it).
#
# Usage: win_examples.sh <folder> <branch wheel (win_amd64)> <site checkout> [<log>]
# The transcript goes to <log> (default: standard output); the last line:
#   WIN-EXAMPLES <runs> runs, <lines> lines, <failed> failed; %USERPROFILE%\<folder> gone|left
set -u
[ $# -ge 3 ] || { sed -n '3,15p' "$0"; exit 2; }
D=$1
WHL=$2
SITE=$3
LOG=${4:-/dev/stdout}
HERE=$(cd "$(dirname "$0")" && pwd)
W=$(cd "$HERE/../../.." && pwd)
S=$(mktemp -d)
SSH=(ssh -o BatchMode=yes -o ConnectTimeout=10 -o ServerAliveInterval=30 win-laptop)
python3 -P "$HERE/win_examples.py" --out "$S/examples.ps1" \
  --python-dir '$env:APPDATA\uv\python\cpython-3.12-windows-x86_64-none' \
  --run readme "$W/README.md" "pip install larmorx=pip install ..\\files\\$(basename "$WHL")" \
  --run site "$SITE/content/installation.md" || exit 2
there=$("${SSH[@]}" "if exist $D (echo exists) else (mkdir $D && mkdir $D\\files && echo made)") || { echo "win-laptop: no answer"; exit 2; }
[ "$(echo "$there" | tr -d '\r')" = made ] || { echo "win-laptop: %USERPROFILE%\\$D exists already"; exit 2; }
scp -q -o BatchMode=yes "$S/examples.ps1" "win-laptop:$D/" && scp -q -o BatchMode=yes "$WHL" "win-laptop:$D/files/" || exit 2
timeout 3600 "${SSH[@]}" "powershell -NoProfile -NonInteractive -File $D\\examples.ps1" > "$LOG" 2>&1
left=$("${SSH[@]}" "rmdir /s /q $D & if exist $D (echo left) else (echo gone)" | tr -d '\r')
runs=$(grep -c "^@@RUN" "$LOG")
lines=$(grep -c "^@@STATUS" "$LOG")
expected=$(grep "^@@RUN" "$LOG" | awk '{s += $3} END {print s + 0}')
failed=$(grep "^@@STATUS" "$LOG" | grep -vc " True :: ")
[ "$lines" = "$expected" ] || failed=$((failed + expected - lines))
grep -q "^@@END" "$LOG" || failed=$((failed + 1))
grep "^@@STATUS" "$LOG" | grep -v " True :: "
echo "WIN-EXAMPLES $runs runs, $lines lines, $failed failed; %USERPROFILE%\\$D $left"
[ "$failed" = 0 ] && [ "$left" = gone ]
