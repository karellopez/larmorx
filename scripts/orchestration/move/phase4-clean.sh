#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# The move's phase 4 (docs/machine-move.md): the old laptop cleaned to a helper, only the list the
# user approved on 2026-10-27. Runs on the old laptop (from the main machine:
# `ssh old-laptop 'bash -s' < phase4-clean.sh` for the dry run, `... 'bash -s -- --delete' < ...`
# to delete). The dry run prints every path, its size and the checks; nothing outside the list is
# touched. Refuses to run on any other machine, and refuses to delete while a check fails.
set -euo pipefail
export LC_ALL=C  # the old laptop's locale writes decimal commas, which printf then refuses
[[ $(hostname) == karelo-GE75-Raider-8SF ]] || { echo "not the old laptop: $(hostname)" >&2; exit 1; }
WS=$HOME/VScode_projects/super_fmriprep
LOG=$WS/oracles/logs/phase4-clean.log
DELETE=0
[[ ${1:-} == --delete ]] && DELETE=1
PATHS=(
  "$WS/larmorx/.claude/worktrees"
  "/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep"
  "$WS/archive"
  "$WS/oracles/compare"
  "$WS/oracles/fsl-6.0.7/mcflirt/band/real"
  "$WS/oracles/fsl-6.0.7/mcflirt/band/syn"
  "$WS/oracles/ants-2.6.5-gcc11-O3"
  "$WS/oracles/figures-cache"
)

fail=0
# The agents' worktrees: each clean and its commit in main (so the work is in git).
for w in "$WS"/larmorx/.claude/worktrees/*/; do
  [[ -d $w ]] || continue
  if [[ -n $(git -C "$w" status --porcelain 2>&1) ]]; then echo "CHECK FAIL: $w has changes"; fail=1; fi
  if ! git -C "$WS/larmorx" merge-base --is-ancestor "$(git -C "$w" rev-parse HEAD)" main; then
    echo "CHECK FAIL: $w is not merged into main"; fail=1
  fi
done
# mcflirt's band summary, which the records read, stays.
[[ -f $WS/oracles/fsl-6.0.7/mcflirt/band/band.json ]] || { echo "CHECK FAIL: band.json missing"; fail=1; }
echo "checks: $([[ $fail == 0 ]] && echo passed || echo FAILED)"

total=0
for p in "${PATHS[@]}"; do
  if [[ -e $p ]]; then
    k=$(du -sk "$p" 2>/dev/null | cut -f1)
    total=$((total + k))
    printf '%8.1f GB  %s\n' "$(echo "$k / 1048576" | bc -l)" "$p"
  else
    printf '%8s     %s (absent)\n' "-" "$p"
  fi
done
printf '%8.1f GB  in all\n' "$(echo "$total / 1048576" | bc -l)"
df -h "$WS" | tail -1

if [[ $DELETE == 1 ]]; then
  [[ $fail == 0 ]] || { echo "a check failed: nothing deleted" >&2; exit 1; }
  {
    echo "== phase 4, $(date -Is)"
    df -h "$WS" | tail -1
    for p in "${PATHS[@]}"; do
      [[ -e $p ]] || continue
      rm -rf --one-file-system "$p"
      echo "deleted $p"
    done
    # Locked entries survive `prune`; unlock those whose folder is gone (their branches stay).
    git -C "$WS/larmorx" worktree list --porcelain | awk '/^worktree /{print $2}' | while read -r w; do
      [[ -d $w ]] || git -C "$WS/larmorx" worktree unlock "$w" 2>/dev/null || true
    done
    git -C "$WS/larmorx" worktree prune
    echo "git worktree prune: $(git -C "$WS/larmorx" worktree list | wc -l) entries left"
    df -h "$WS" | tail -1
    echo "PHASE4-DONE"
  } 2>&1 | tee -a "$LOG"
else
  echo "dry run: nothing deleted (--delete to delete)"
fi
