#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# run_remote.sh <ssh host> <exe> [args...]: copies <exe> into %TEMP%\larmorx-cross on the Windows
# host (key-based SSH only) and runs it there with the arguments; its output comes back here.
set -eu
[ $# -ge 2 ] || { echo "usage: run_remote.sh <ssh host> <exe> [args...]" >&2; exit 2; }
H=$1; EXE=$2; shift 2
ssh -o BatchMode=yes "$H" "if not exist %TEMP%\\larmorx-cross mkdir %TEMP%\\larmorx-cross"
scp -q -o BatchMode=yes "$EXE" "$H:AppData/Local/Temp/larmorx-cross/"
ssh -o BatchMode=yes "$H" "cd %TEMP%\\larmorx-cross && $(basename "$EXE") $*"
