#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# The merge checklist's checks (CLAUDE.md, "Merging an agent's work"), run on a committed merge.
# Usage: verify-merge.sh <checkout> <outdir> [options]
#   --suite "<parity args>"   re-run a standard-tier record (repeatable), e.g. --suite "ants-ai --jobs 6"
#                             or --suite "afni-tshift --implementation replica"
#   --site <site checkout>    a clean checkout of the agent's site branch, for the export check
#   --evidence <tool>         re-export that tool's evidence into --site (repeatable); git must see no change
#   --golden                  re-record the golden checksums from the merge (after parallel merges);
#                             its parity smoke tier is step 4. Without it, step 4 runs pytest tests/parity.
#   --threads N               cap cargo and rayon threads (when another agent shares the machine)
#   --oracle-cache MODE       how the parity runs use the oracle-output cache: use (the default:
#                             the agent's runs of the originals are served, only larmorx runs),
#                             refresh (run them all again and compare: a difference fails) or off
#                             (docs/validation/oracle-cache.md); LARMORX_ORACLE_CACHE names the cache
# Writes <outdir>/main.log (one line per step, ALLDONE at the end) and a log per step.
# Afterwards: compare_records.py <outdir>/records <suite>...; check the site's git status.
R=$1; OUT=$2; shift 2
D=$(cd "$(dirname "$0")" && pwd); source "$D/env.sh"
SUITES=(); EVID=(); SITE=""; GOLDEN=0; export LARMORX_ORACLE_CACHE_MODE=${LARMORX_ORACLE_CACHE_MODE:-use}
while [ $# -gt 0 ]; do
  case $1 in
    --suite) SUITES+=("$2"); shift 2;;
    --site) SITE=$2; shift 2;;
    --evidence) EVID+=("$2"); shift 2;;
    --golden) GOLDEN=1; shift;;
    --threads) export CARGO_BUILD_JOBS=$2 RAYON_NUM_THREADS=$2; shift 2;;
    --oracle-cache) export LARMORX_ORACLE_CACHE_MODE=$2; shift 2;;
    *) echo "unknown option $1"; exit 2;;
  esac
done
mkdir -p "$OUT/records"; L=$OUT/main.log; : > "$L"
log() { echo "$*" | tee -a "$L"; }
log "commit $(git -C "$R" rev-parse --short HEAD) dirty=[$(git -C "$R" status --porcelain | wc -l)] start $(date -Is) oracle-cache=$LARMORX_ORACLE_CACHE_MODE"
bash "$D/gates-rust.sh" "$R" all > "$OUT/rust.log" 2>&1; log "STEP1 rust gates exit $?"
bash "$D/build-gpl.sh" "$R" > "$OUT/gpl-build.log" 2>&1; log "GPL build exit $?"
bash "$D/build-nc.sh" "$R" > "$OUT/nc-build.log" 2>&1; log "NC build exit $?"
bash "$D/gates-python.sh" "$R" > "$OUT/python.log" 2>&1; log "STEP2+3 python gates exit $?"
if [ $GOLDEN = 0 ]; then
  (export PYTHONPATH=$R/python:$R/validation/src; cd "$R" && python -m pytest -q tests/parity) > "$OUT/parity-tests.log" 2>&1; log "STEP4 parity smoke exit $?"
fi
log "dirty after gates=[$(git -C "$R" status --porcelain | wc -l)]"
for a in "${SUITES[@]}"; do
  n=$(echo "$a" | tr ' ' '_')
  timeout 21600 bash "$D/lxv.sh" "$R" parity $a --tier standard --out "$OUT/records" > "$OUT/$n.log" 2>&1; log "STEP5 $a exit $?"
done
for t in "${EVID[@]}"; do
  (export PYTHONPATH=$R/python:$R/validation/src; cd "$R" && ulimit -v 16000000 && python -m larmorx_validation evidence "$t" --no-compute --site "$SITE") >> "$OUT/evidence.log" 2>&1; log "STEP5 evidence $t exit $?"
done
[ -n "$SITE" ] && log "site changes after re-export: [$(git -C "$SITE" status --porcelain | tr '\n' ' ')]"
if [ $GOLDEN = 1 ]; then
  log "dirty before golden=[$(git -C "$R" status --porcelain | wc -l)]"
  (export PYTHONPATH=$R/python:$R/validation/src; cd "$R" && python -P scripts/record_golden.py) > "$OUT/golden.log" 2>&1; log "STEP4+golden exit $?"
  log "golden diff: $(git -C "$R" diff --stat | tail -1)"
fi
log "ALLDONE $(date -Is)"
