#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Runs one FSL BET call <n> times in fresh directories and prints each output's SHA-256, to
# see whether FSL's result is the same from run to run (memory it never wrote, address-space
# layout randomisation). Usage: bet_repeat.sh <FSLDIR> <n> <dir with inputs> <FSLOUTPUTTYPE>
# <tool> <args...>
set -u
fsldir=$1
n=$2
dir=$3
otype=$4
tool=$5
shift 5
for i in $(seq 1 "$n"); do
  w="$dir/repeat-$i"
  rm -rf "$w"
  mkdir -p "$w"
  for f in "$dir"/*; do
    [ -f "$f" ] && cp "$f" "$w/"
  done
  (
    cd "$w" || exit 9
    env -i HOME="$HOME" PATH="$fsldir/share/fsl/bin:/usr/bin:/bin" FSLDIR="$fsldir" FSLOUTPUTTYPE="$otype" \
      "$fsldir/share/fsl/bin/$tool" "$@" >stdout.txt 2>stderr.txt
    echo "run $i: exit $?"
    for f in *; do
      case "$f" in stdout.txt | stderr.txt) continue ;; esac
      if [ -f "$f" ]; then
        if [ "${f%.gz}" != "$f" ]; then
          echo "  $f $(gzip -dc "$f" | sha256sum | cut -c1-16) (data)"
        else
          echo "  $f $(sha256sum "$f" | cut -c1-16)"
        fi
      fi
    done
  )
done
