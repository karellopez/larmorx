#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# The move's final pass (2026-10-27), on the NEW laptop, in tmux session "move2": waits until the
# first copy (pull.sh, tmux session "move") has ended, copies again with --verify (every file's
# content compared; brings what changed meanwhile), sets up the shell, then runs check.py.
# Results: ~/move-check.log (check.py's full output) and ~/MOVE-READY.txt (a summary).
WS=/home/karelo/VScode_projects/super_fmriprep
while pgrep -f '^bash /tmp/pull.sh' >/dev/null; do sleep 120; done

ok=""
for attempt in 1 2 3; do
  if bash /tmp/pull.sh old-laptop --verify; then ok=yes; break; fi
  sleep 60
done

# Shell settings as on the old laptop: Rust on PATH, Node 22 as nvm's default.
for f in "$HOME/.profile" "$HOME/.bashrc"; do
  grep -qF '. "$HOME/.cargo/env"' "$f" ||
    printf '\n# Rust, as on the old laptop (the move, 2026-10-27)\n. "$HOME/.cargo/env"\n' >> "$f"
done
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh" && nvm alias default 22.23.2 >/dev/null

python3 "$WS/larmorx/scripts/orchestration/move/check.py" > "$HOME/move-check.log" 2>&1
rc=$?
echo "CHECK-EXIT $rc" >> "$HOME/move-check.log"
{
  echo "The move's final pass ended $(date '+%Y-%m-%d %H:%M')."
  if [ -n "$ok" ]; then echo "Copy: done."; else echo "Copy: NOT COMPLETE: run  bash /tmp/pull.sh old-laptop  again."; fi
  echo "Check (check.py, full output in ~/move-check.log):"
  grep -E '^(FAIL|WARN)' "$HOME/move-check.log" | sed 's/^/  /'
  grep -E 'checks:' "$HOME/move-check.log" | sed 's/^/  /'
  echo "Next: open VS Code in $WS, start Claude Code (any first message): it follows docs/machine-move.md, phase 3."
} > "$HOME/MOVE-READY.txt"
cat "$HOME/MOVE-READY.txt"
