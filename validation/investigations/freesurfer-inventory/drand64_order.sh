#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (sub-task 5, built from ITK 4.13.2's vnl_random.cxx).
# The order in which the official binaries evaluate vnl_random::drand64's two lrand32() calls
# (operands of '+', so unspecified in C++): for each binary, find the code that divides by
# 4294967295.0^2 (the low part's constant, find_consts.py), and print the two calls before it and
# the register each division converts. The first call's result is kept in %rbx across the second.
#   drand64_order.sh <binary> [<binary> ...]
for B in "$@"; do
  C2=$(python3 -P "$(dirname "$0")/find_consts.py" "$B" | sed -n "s/^c2.*'.rodata', '0x\([0-9a-f]*\)'.*/\1/p")
  if [ -z "$C2" ]; then echo "$(basename "$B"): constant not found"; continue; fi
  objdump -d --no-show-raw-insn "$B" | grep -B 18 "# $C2 " | grep -E "call|cvtsi2sd|divsd" \
    | sed "s#^#$(basename "$B"): #" | head -8
done
