#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Relay 4: scripts/release/check_install_windows.ps1 run on the Windows helper `win-laptop`
# (docs/helper-machines.md: key-based SSH, cmd.exe as the login shell, no administrator rights
# asked for) from this machine: the folder %USERPROFILE%\larmorx-check-<version> made there (it
# must not exist yet), the two check scripts copied into it (and, with --files, a local folder
# as its files\: wheels, archives), the check run, the folder's removal checked afterwards.
# After rc1's upload: win_check.sh 0.1.0rc1 (PyPI, the GitHub Release, the tag's tests).
#
# Usage: win_check.sh <version> [--files <dir>] [-- <check_install_windows.ps1 arguments>]
#   e.g. win_check.sh 0.1.0rc1 -- -SourceRef <commit> -NoArchives   (before the release)
#        win_check.sh 0.1.0rc1 --files <dir> -- -FindLinks files\wheels -SourceRef <commit>
# The report goes to standard output; the exit status is the check's.
set -u
[ $# -ge 1 ] || { sed -n '3,15p' "$0"; exit 2; }
V=$1
shift
FILES=
if [ "${1:-}" = --files ]; then FILES=$(cd "$2" && pwd); shift 2; fi
[ "${1:-}" = -- ] && shift
HERE=$(cd "$(dirname "$0")" && pwd)
W=$(cd "$HERE/../../.." && pwd)
D=larmorx-check-$V  # relative to the profile, where the SSH session starts
SSH=(ssh -o BatchMode=yes -o ConnectTimeout=10 -o ServerAliveInterval=30 win-laptop)
there=$("${SSH[@]}" "if exist $D (echo exists) else (mkdir $D && echo made)") || { echo "win-laptop: no answer"; exit 2; }
[ "$(echo "$there" | tr -d '\r')" = made ] || { echo "win-laptop: %USERPROFILE%\\$D exists already: remove it first"; exit 2; }
scp -q -o BatchMode=yes "$W/scripts/release/check_install.py" "$W/scripts/release/check_install_windows.ps1" "win-laptop:$D/" || exit 2
if [ -n "$FILES" ]; then scp -q -r -o BatchMode=yes "$FILES" "win-laptop:$D/files" || exit 2; fi
timeout 3600 "${SSH[@]}" "powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $D\\check_install_windows.ps1 -Version $V $*"
status=$?
left=$("${SSH[@]}" "if exist $D (echo left) else (echo gone)" | tr -d '\r')
echo "win_check: the check's exit status $status; %USERPROFILE%\\$D $left"
[ "$left" = gone ] || status=1
exit $status
