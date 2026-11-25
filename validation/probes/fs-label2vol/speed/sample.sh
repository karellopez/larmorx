#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# sample.sh <work folder> <program> [arguments...]: a sampling profile without perf (not allowed on the main
# machine): gdb runs the program and a loop outside it interrupts it every 0.03 s; each stop prints the stack.
# Prints how often each function was the innermost frame and among the next three. The program's own output
# goes to <work folder>/prog.out. Run it inside a memory box; the environment is the caller's.
set -u
D=$1; P=$2; shift 2
mkdir -p "$D"; cd "$D"
{ echo "set pagination off"; echo "set print thread-events off"; echo "handle SIGINT stop print nopass"
  echo "set args $* > $D/prog.out 2>&1"; echo "run"
  for _ in $(seq 2000); do echo "bt 8"; echo "continue"; done; } > "$D/cmds.gdb"
gdb -batch -x "$D/cmds.gdb" "$P" > "$D/gdb.out" 2>&1 &
GP=$!
sleep 1
for _ in $(seq 20000); do
  sleep 0.03
  kill -0 $GP 2>/dev/null || break
  C=$(pgrep -P $GP | head -1)
  [ -n "$C" ] && kill -INT "$C" 2>/dev/null
done
wait $GP
grep -E "^#0 " "$D/gdb.out" | sed -E 's/^#0 +(0x[0-9a-f]+ in )?//; s/ \(.*//' | sort | uniq -c | sort -rn | head -15
echo "--- frames #1..#3:"
grep -E "^#[1-3] " "$D/gdb.out" | sed -E 's/^#[0-9]+ +(0x[0-9a-f]+ in )?//; s/ \(.*//' | sort | uniq -c | sort -rn | head -15
