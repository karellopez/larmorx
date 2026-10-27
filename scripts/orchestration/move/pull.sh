#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copy the larmorx workspace from the old laptop: run on the NEW machine, as the same user, with
# the same paths (docs/machine-move.md, "The copy"). Resumable: run it again after an
# interruption. The last run uses --verify: every file's content is compared (checksums), not
# only its size and date, so the copy is proven identical to the source.
#
#   bash pull.sh <old-host> [--verify] [--dry-run]
#
# It needs an SSH key from this machine to the old one (no passwords). Nothing is deleted on
# either side. What is not copied, and why, is listed in docs/machine-move.md; the same list is
# manifest.py's LEFT_ON_OLD, which check.py reads.
set -euo pipefail
OLD=${1:?usage: pull.sh <old-host> [--verify] [--dry-run]}
shift
WS=/home/karelo/VScode_projects/super_fmriprep
H=/home/karelo
LOG=$H/move-pull-$(date +%Y%m%d-%H%M%S).log
[ "$HOME" = "$H" ] || { echo "run as karelo (HOME=$H), not $HOME"; exit 1; }
command -v rsync >/dev/null || { echo "rsync missing: sudo apt install rsync"; exit 1; }
ssh -o BatchMode=yes "$OLD" true || { echo "no key-based SSH to $OLD yet"; exit 1; }

R=(rsync -aH --partial --info=progress2,stats1 --human-readable)
for a in "$@"; do
  case $a in
    --verify) R+=(--checksum) ;;
    --dry-run) R+=(--dry-run) ;;
    *) echo "unknown option $a"; exit 1 ;;
  esac
done

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

# 1. The tools: FSL 6.0.7.17 as installed (the current FSL records' original; the newest FSL is
#    installed beside it on the new machine by the version update), Rust as pinned, Node 22.
copy "$H/fsl/" "$H/fsl/"
copy "$H/.cargo/" "$H/.cargo/"
copy "$H/.rustup/" "$H/.rustup/"
if ssh -o BatchMode=yes "$OLD" test -d "$H/.nvm/versions/node"; then
  copy "$H/.nvm/versions/node/" "$H/.nvm/versions/node/"
fi

# 2. The workspace, without what the work does not need (docs/machine-move.md):
#    - build caches (target/: rebuilt in minutes) and __pycache__;
#    - the agents' worktrees (.claude/worktrees: all 46 clean and merged into main on 2026-10-27,
#      so their work is in git; `git worktree prune` there removes their entries);
#    - archive/ (its documents follow in 3; its bulky temporary data stays behind);
#    - oracles/compare/ (the 2026-10-12 fMRIPrep stage comparisons; redone at the end with the
#      newest fMRIPrep);
#    - the GCC 11 ANTs build's build folders (its programs, BUILD_INFO, logs and thread band are
#      copied: the evidence of the compiler bug);
#    - mcflirt's raw band runs (the records read only band/band.json, which is copied).
copy "$WS/" "$WS/" \
  --exclude='/larmorx/target/' --exclude='/larmorx/crates-gpl/target/' --exclude='/larmorx/crates-nc/target/' \
  --exclude='__pycache__/' \
  --exclude='/larmorx/.claude/worktrees/' \
  --exclude='/archive/' \
  --exclude='/oracles/compare/' \
  --exclude='/oracles/ants-2.6.5-gcc11-O3/ants-build/' --exclude='/oracles/ants-2.6.5-gcc11-O3/itk-build/' \
  --exclude='/oracles/fsl-6.0.7/mcflirt/band/real/' --exclude='/oracles/fsl-6.0.7/mcflirt/band/syn/'

# 3. The archive's documents only: notes, reports, READMEs, scripts, source drafts, logs, tables,
#    the agents' conversation records (*.output): text files under 1 MB, about 1 GB. Its bulky
#    temporary data (test outputs, cache copies, clones, large reference tables) stays behind.
copy "$WS/archive/" "$WS/archive/" --prune-empty-dirs --max-size=1024k --exclude='.git/' \
  --include='*/' \
  --include='*.md' --include='*.txt' --include='README*' --include='NOTES*' --include='DONE*' \
  --include='BUILD_INFO*' --include='*.output' --include='*.py' --include='*.sh' --include='*.rs' \
  --include='*.c' --include='*.h' --include='*.cpp' --include='*.cxx' --include='*.toml' \
  --include='*.yml' --include='*.yaml' --include='*.json' --include='*.jsonl' --include='*.log' \
  --include='*.patch' --include='*.diff' --include='*.csv' --include='*.tsv' \
  --exclude='*'

# 4. The test data, Claude Code's notes for this project (memory and transcripts; the old
#    settings.json beside the new one), the usage tool's records.
copy "$H/.cache/larmorx-testdata/" "$H/.cache/larmorx-testdata/"
copy "$H/.claude/projects/-home-karelo-VScode-projects-super-fmriprep/" \
     "$H/.claude/projects/-home-karelo-VScode-projects-super-fmriprep/"
copy "$H/.claude/settings.json" "$H/.claude/settings.json.from-old-laptop"
copy "$H/.cache/claude-usage/" "$H/.cache/claude-usage/"
if ssh -o BatchMode=yes "$OLD" test -e "$H/.config/claude-usage.json"; then
  copy "$H/.config/claude-usage.json" "$H/.config/claude-usage.json"
fi

if [ ${#FAILED[@]} -gt 0 ]; then echo "NOT COMPLETE: ${FAILED[*]} (run pull.sh again); log: $LOG" | tee -a "$LOG"; exit 1; fi
echo "done; log: $LOG" | tee -a "$LOG"
