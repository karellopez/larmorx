#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# The clean-room audit at a merge (docs/orchestration.md, the merge checklist; the audit:
# docs/validation/clean-room-audit.md): the part's transcripts archived with their SHA-256, the
# scan and the verdicts over every transcript, then the gate, which stops the merge when a
# clean-room original was edited by an agent after it had seen restricted material of that tool's
# family (an implementer's finding, or another role's edit) and validation/cleanroom-audit/
# judgements.toml does not judge it, or when a commit of clean-room code has no maker.
#
# Usage: cleanroom-audit-merge.sh <checkout> <part> [--agent ID]... [--notes FILE]...
#            [--worktree NAME]... [--scratch NAME]... [--summary] [--similarity] [--gib N] [--jobs N]
#   <checkout>    the merged checkout: its history (every branch) and its judgements are read
#   <part>        the part's name, as its docs/parts folder (e.g. 2026-11-07-prelude-replica)
#   --agent, --notes, --worktree, --scratch   the part's agents (archive_transcripts.py): by id, by
#                 the ids a notes file names, by their worktree, by the scratch folder their brief names
#   --summary     regenerate docs/validation/clean-room-audit.{json,md} in <checkout> from this
#                 dossier and the newest similarity record (then commit them with the merge)
#   --similarity  re-measure the code similarity first (about 15 minutes with --wide, boxed)
#   --gib N, --jobs N   the scan's memory box and readers (16 GiB and 8 by default: the orchestrator's
#                 verification beside two agents stays within 32 GiB and 8 threads)
# Writes <workspace>/archive/<date>-session-transcripts-<part>/ (the copies, SHA256SUMS, README.txt,
# part-<part>.json) and <workspace>/archive/clean-room-audit/<date>-<part>/ (scan/, verdicts/,
# gate.txt, main.log: one line per step, ALLDONE at the end). Private: never publish either.
# Exit status: 0 the gate passed; 1 the gate stopped the merge (its STOP lines: read each call in
# the dossier's verdicts.md, then judge it in judgements.toml or undo the edit; re-run); 2 a step
# failed (an agent of the part without a transcript included).
set -u
[ $# -ge 2 ] || { sed -n '3,24p' "$0"; exit 2; }
R=$(cd "$1" && pwd); PART=$2; shift 2
D=$(cd "$(dirname "$0")" && pwd); source "$D/env.sh"
SEL=(); SUMMARY=0; SIM=0; GIB=16; JOBS=8
while [ $# -gt 0 ]; do
  case $1 in
    --agent|--notes|--worktree|--scratch) SEL+=("$1" "$2"); shift 2;;
    --summary) SUMMARY=1; shift;;
    --similarity) SIM=1; SUMMARY=1; shift;;
    --gib) GIB=$2; shift 2;;
    --jobs) JOBS=$2; shift 2;;
    *) echo "cleanroom-audit-merge.sh: unknown option $1" >&2; exit 2;;
  esac
done
DATE=$(date +%F)
OUT=$LARMORX_WORKSPACE/archive/clean-room-audit/$DATE-$PART
mkdir -p "$OUT"; L=$OUT/main.log; : > "$L"
step() { echo "$(date -Is) $*" >> "$L"; echo "$*"; }
fail() { step "FAILED $1 (exit $2; log $OUT/$1.log)"; step ALLDONE; exit 2; }
LXV=("$D/lxv.sh" "$R")

step "ARCHIVE the part's transcripts (archive_transcripts.py ${SEL[*]})"
python -P "$D/archive_transcripts.py" --part "$PART" --workspace "$LARMORX_WORKSPACE" "${SEL[@]}" \
  > "$OUT/archive.log" 2>&1 || fail archive $?
tail -n 1 "$OUT/archive.log" | tee -a "$L"

step "SCAN every transcript (boxed $GIB GiB, $JOBS readers)"
"$D/boxed.sh" "$GIB" "${LXV[@]}" cleanroom-audit scan --out "$OUT/scan" --jobs "$JOBS" \
  > "$OUT/scan.log" 2>&1 || fail scan $?
tail -n 1 "$OUT/scan.log" | tee -a "$L"

step "VERDICTS from the scan and $R's history"
"${LXV[@]}" cleanroom-audit verdicts --scan "$OUT/scan" --out "$OUT/verdicts" --repo "$R" \
  > "$OUT/verdicts.log" 2>&1 || fail verdicts $?
tail -n 1 "$OUT/verdicts.log" | tee -a "$L"

if [ "$SIM" = 1 ]; then
  step "SIMILARITY (boxed 24 GiB, --wide)"
  "$D/boxed.sh" 24 "${LXV[@]}" cleanroom-audit similarity --out "$OUT/similarity" --repo "$R" \
    --wide --jobs "$JOBS" > "$OUT/similarity.log" 2>&1 || fail similarity $?
fi

step "GATE (judgements: $R/validation/cleanroom-audit/judgements.toml)"
"${LXV[@]}" cleanroom-audit gate --verdicts "$OUT/verdicts" > "$OUT/gate.txt" 2>&1
GATE=$?
grep -E '^(STOP|JUDGE) ' "$OUT/gate.txt" | tee -a "$L"
tail -n 1 "$OUT/gate.txt" | tee -a "$L"
[ "$GATE" -le 1 ] || fail gate "$GATE"

if [ "$SUMMARY" = 1 ] && [ "$GATE" = 0 ]; then
  step "SUMMARY docs/validation/clean-room-audit.{json,md} in $R"
  "${LXV[@]}" cleanroom-audit summary --dossier "$OUT" --verbatim "$OUT/verbatim.md" \
    > "$OUT/summary.log" 2>&1 || fail summary $?
  tail -n 1 "$OUT/summary.log" | tee -a "$L"
fi
if [ "$GATE" = 0 ]; then step "PASSED the clean-room audit"; else step "STOPPED: the gate's STOP lines above"; fi
step ALLDONE
exit "$GATE"
