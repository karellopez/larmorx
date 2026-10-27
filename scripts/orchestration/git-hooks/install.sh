#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Installs commit-msg (no AI attribution in commit messages) into the repositories given, or by
# default into the workspace's three: larmorx, larmorx_documentation, larmorx-testdata. Also run it
# on any new clone (an agent's clone of the site, a fresh clone on another machine).
#   bash install.sh [repository ...]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ws=$(cd "$here/../../../.." && pwd)
repos=("$@")
[ ${#repos[@]} -gt 0 ] || repos=("$ws/larmorx" "$ws/larmorx_documentation" "$ws/larmorx-testdata")
for r in "${repos[@]}"; do
  hooks=$(git -C "$r" rev-parse --git-path hooks)
  case $hooks in /*) ;; *) hooks="$r/$hooks" ;; esac
  mkdir -p "$hooks"
  install -m 755 "$here/commit-msg" "$hooks/commit-msg"
  echo "installed: $hooks/commit-msg"
done
