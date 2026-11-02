#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# profile.sh <out> <program args...>: a poor man's profiler where perf is not allowed (gdb
# backtraces of every thread every
# 20 ms, triggered by SIGUSR1; the busy threads' stacks summed): the leaf and inclusive frames.
S=$(cd "$(dirname "$0")" && pwd)
out=$1; shift
gdb -batch -x $S/profile.gdb --args "$@" > "$out.raw" 2>&1 &
g=$!
sleep 0.3
for i in 1 2 3 4 5 6 7 8 9 10; do pid=$(pgrep -n -f "^$1 "); [ -n "$pid" ] && break; sleep 0.1; done; echo "pid $pid" >&2
while kill -0 $pid 2>/dev/null; do kill -USR1 $pid 2>/dev/null; sleep 0.02; done
wait $g
python3 - "$out.raw" > "$out" <<'PY'
import re, sys, collections
txt = open(sys.argv[1], errors="replace").read()
stacks, cur = [], []
for ln in txt.splitlines():
    m = re.match(r"#(\d+)\s+(?:0x[0-9a-f]+ in )?(\S+)", ln)
    if m:
        if m.group(1) == "0" and cur:
            stacks.append(cur); cur = []
        name = re.sub(r"::h[0-9a-f]{16}$", "", m.group(2))
        cur.append(name)
if cur: stacks.append(cur)
# busy threads only: stacks in the program's code, not waiting
stacks = [s for s in stacks if any("larmorx" in f for f in s) and not any(w in s[0] for w in ("syscall", "futex", "wait", "sched_yield", "nanosleep"))]
n = max(len(stacks), 1)
leaf = collections.Counter(s[0] for s in stacks)
incl = collections.Counter()
for s in stacks:
    for f in set(s): incl[f] += 1
print("SAMPLES", n)
for k, v in leaf.most_common(25): print(f"LEAF {100*v/n:5.1f}% {k[:160]}")
for k, v in incl.most_common(50): print(f"INCL {100*v/n:5.1f}% {k[:160]}")
PY
