#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# PIPE-80's path handling and the golden case cli.nipype.run's text on a Windows machine reached by
# key-based SSH (docs/helper-machines.md), without building a Windows wheel: a Python 3.12 venv (uv)
# under the machine's %TEMP%\larmorx-paths with larmorx 0.1.0rc1 from PyPI (its extension's
# ants_file_program has the signature _nipype.run calls), the given commit's python/larmorx/_nipype.py
# laid over it, and its tests/python/test_nipype_paths.py run; then probe_golden_text_windows.py.
# PowerShell's own exit status is ignored (it reports a native program's stderr as an error).
# Usage: windows_paths.sh <commit> [<commit> ...] [--host win-laptop]
set -euo pipefail
host=win-laptop; commits=()
while [ $# -gt 0 ]; do
  case "$1" in --host) host=$2; shift 2 ;; *) commits+=("$1"); shift ;; esac
done
repo=$(cd "$(dirname "$0")/../../.." && pwd)
winps() { { ssh -o BatchMode=yes -o ConnectTimeout=10 "$host" "powershell -NoProfile -NonInteractive -EncodedCommand $(printf '%s' "$1" | iconv -t UTF-16LE | base64 -w0)" 2>&1 || true; } | { grep -v 'CLIXML\|<Objs\|^#<' || true; }; }
remote=$(winps '$d = Join-Path $env:TEMP "larmorx-paths"; New-Item -ItemType Directory -Force -Path $d | Out-Null; Write-Output $d' | tr -d '\r' | tail -1)
winps '$ProgressPreference="SilentlyContinue"; Set-Location (Join-Path $env:TEMP "larmorx-paths"); if (-not (Test-Path venv)) { uv venv --python 3.12 venv 2>&1 | Out-Null }; uv pip install --python venv\Scripts\python.exe "larmorx==0.1.0rc1" nibabel pytest 2>&1 | Select-Object -Last 1'
tmp=$(mktemp -d)
for c in "${commits[@]}"; do
  mkdir -p "$tmp/$c"
  git -C "$repo" show "$c:python/larmorx/_nipype.py" > "$tmp/$c/_nipype.py"
  git -C "$repo" show "$c:tests/python/test_nipype_paths.py" > "$tmp/$c/test_nipype_paths.py"
done
cp "$(dirname "$0")/probe_golden_text_windows.py" "$tmp/"
scp -q -o BatchMode=yes -r "$tmp"/* "$host:${remote//\\//}/"
for c in "${commits[@]}"; do
  echo "===== $c"
  winps "Set-Location (Join-Path \$env:TEMP 'larmorx-paths'); Copy-Item '$c\\_nipype.py' 'venv\\Lib\\site-packages\\larmorx\\_nipype.py' -Force; & venv\\Scripts\\python.exe -m pytest -q -p no:cacheprovider --basetemp \"\$PWD\\bt-$c\" '$c\\test_nipype_paths.py' 2>&1 | Select-String -Pattern 'passed|failed|^FAILED'"
done
echo "===== the golden case's text"
winps "Set-Location (Join-Path \$env:TEMP 'larmorx-paths'); & venv\\Scripts\\python.exe -I probe_golden_text_windows.py \"\$PWD\\sim\" 2>&1 | Select-String -Pattern '^(old|new) '"
rm -rf "$tmp"
