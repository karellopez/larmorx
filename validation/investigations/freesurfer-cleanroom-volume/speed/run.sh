#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# run.sh <probe> <candidate>: each b<i>.txt of the current folder through the probe and the candidate
# (reproduce), 3 times each; the best wall seconds, and whether the result lines agree (the candidate's
# defect notices taken out of its standard error first, as compare.py does). Run it inside a memory box.
P=$1; C=$2
for f in b*.txt; do
  b=${f%.txt}
  for who in probe cand; do
    best=999
    for k in 1 2 3; do
      s=$(date +%s.%N)
      if [ $who = probe ]; then env -i PATH=/usr/bin:/bin LC_ALL=C "$P" batch $f > $b.$who.out
      else env -i PATH=/usr/bin:/bin LC_ALL=C LARMORX_UPSTREAM_DEFECTS=reproduce "$C" batch $f > $b.$who.out; fi
      e=$(date +%s.%N); t=$(echo "$e - $s" | bc); best=$(echo "if ($t < $best) $t else $best" | bc)
    done
    echo "$b $who $best"
  done
  python3 -P - "$b.probe.out" "$b.cand.out" <<'PY'
import re, sys
def dec(p):
    res, _, st = open(p, "rb").read().rstrip(b"\n").rpartition(b" | ")
    o, _, e = st.partition(b" ")
    e = bytes.fromhex(e[1:].decode()) if e[:1] == b"x" else b""
    return res, o, re.sub(rb"^upstream defect .*\n?", b"", e, flags=re.M)
print(sys.argv[1].split(".")[0], "identical" if dec(sys.argv[1]) == dec(sys.argv[2]) else "DIFFER")
PY
done
