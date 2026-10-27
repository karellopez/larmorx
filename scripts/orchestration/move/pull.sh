#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copy the whole larmorx workspace from the old laptop: run on the NEW machine, as the same user,
# with the same paths (docs/machine-move.md). Resumable: run it again after an interruption, and
# once more at the end (the second pass only checks and copies what changed).
#
#   bash pull.sh karelo@<old-laptop-address> [--dry-run]
#
# It needs an SSH key from this machine to the old one (no passwords; docs/machine-move.md,
# step 2). Build caches (target/) are not copied: the new machine rebuilds them. Nothing is
# deleted on either side.
set -euo pipefail
OLD=${1:?usage: pull.sh user@old-host [--dry-run]}
DRY=${2:-}
WS=/home/karelo/VScode_projects/super_fmriprep
H=/home/karelo
LOG=$H/move-pull-$(date +%Y%m%d-%H%M%S).log
[ "$HOME" = "$H" ] || { echo "run as karelo (HOME=$H), not $HOME"; exit 1; }
command -v rsync >/dev/null || { echo "rsync missing: sudo apt install rsync"; exit 1; }
ssh -o BatchMode=yes "$OLD" true || { echo "no key-based SSH to $OLD yet (docs/machine-move.md, step 2)"; exit 1; }

R=(rsync -aH --partial --info=progress2,stats1 --human-readable)
[ "$DRY" = "--dry-run" ] && R+=(--dry-run)

FAILED=()
copy() {  # copy <source path on OLD> <destination> [rsync options...]
  local src=$1 dst=$2; shift 2
  echo "== $src -> $dst" | tee -a "$LOG"
  mkdir -p "$(dirname "$dst")"
  set +e
  "${R[@]}" "$@" "$OLD:$src" "$dst" 2>&1 | tee -a "$LOG"
  local rc=${PIPESTATUS[0]}
  set -e
  [ "$rc" = 0 ] || { echo "   FAILED (rsync exit $rc)" | tee -a "$LOG"; FAILED+=("$src"); }
}

# 1. The workspace: repositories (with their git state and worktrees), upstream clones, tags,
#    oracles (fMRIPrep runs, builds, toolchains, the oracle cache), archive, the venv, tools.
copy "$WS/" "$WS/" \
  --exclude='/larmorx/target/' --exclude='/larmorx/crates-gpl/target/' --exclude='/larmorx/crates-nc/target/' \
  --exclude='/larmorx/.claude/worktrees/*/target/' \
  --exclude='/larmorx/.claude/worktrees/*/crates-gpl/target/' \
  --exclude='/larmorx/.claude/worktrees/*/crates-nc/target/' \
  --exclude='__pycache__/'
# 2. FSL 6.0.7.17 as installed (the FSL replicas' oracle), the test data, Rust as pinned.
copy "$H/fsl/" "$H/fsl/"
copy "$H/.cache/larmorx-testdata/" "$H/.cache/larmorx-testdata/"
copy "$H/.cargo/" "$H/.cargo/"
copy "$H/.rustup/" "$H/.rustup/"
# 3. Claude Code: this project's memory notes and transcripts (the same path gives the same
#    project folder); the old settings.json beside the new one.
copy "$H/.claude/projects/-home-karelo-VScode-projects-super-fmriprep/" \
     "$H/.claude/projects/-home-karelo-VScode-projects-super-fmriprep/"
copy "$H/.claude/settings.json" "$H/.claude/settings.json.from-old-laptop"

if [ ${#FAILED[@]} -gt 0 ]; then echo "NOT COMPLETE: ${FAILED[*]} (run pull.sh again); log: $LOG"; exit 1; fi
echo "done; log: $LOG"
echo "next: python3 $WS/larmorx/scripts/orchestration/move/check.py   (docs/machine-move.md, step 5)"
