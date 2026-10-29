#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Runs one BET call three ways in fresh copies of a directory: FSL 6.0.7.23, FSL 6.0.7.17 and the
# replica (larmorx-nc, `reproduce`), each in <dir>/fsl23, <dir>/fsl17, <dir>/lx, printing exit
# status and messages, then cmp of every output. Used to triage the version trial's BET
# differences (README.md).
# Usage: bet_triage_run.sh <dir with the inputs> <FSLOUTPUTTYPE> <tool> <args...>
set -u
dir=$1
otype=$2
tool=$3
shift 3
here=$(cd "$(dirname "$0")/../../.." && pwd)
nc=${LARMORX_NC_BIN:-$here/crates-nc/target/release/larmorx-nc}
for who in fsl23 fsl17 lx; do
  rm -rf "${dir:?}/$who"
  mkdir -p "$dir/$who"
  for f in "$dir"/*; do
    [ -f "$f" ] && cp "$f" "$dir/$who/"
  done
done
run() {
  local who=$1 fsldir=$2
  shift 2
  (
    cd "$dir/$who" || exit 9
    if [ "$who" = lx ]; then
      env -i HOME="$HOME" PATH=/usr/bin:/bin FSLOUTPUTTYPE="$otype" LARMORX_UPSTREAM_DEFECTS=reproduce \
        "$nc" fsl "$tool" "$@" >stdout.txt 2>stderr.txt
    else
      env -i HOME="$HOME" PATH="$fsldir/share/fsl/bin:/usr/bin:/bin" FSLDIR="$fsldir" FSLOUTPUTTYPE="$otype" \
        "$fsldir/share/fsl/bin/$tool" "$@" >stdout.txt 2>stderr.txt
    fi
    echo "$who: exit $?"
  )
}
run fsl23 "$HOME/fsl-6.0.7.23" "$@"
run fsl17 "$HOME/fsl" "$@"
run lx "" "$@"
for who in fsl23 fsl17 lx; do
  echo "--- $who stdout"; cat "$dir/$who/stdout.txt"
  echo "--- $who stderr"; cat "$dir/$who/stderr.txt"
  echo "--- $who files"; ls "$dir/$who"
done
for f in "$dir"/fsl23/*; do
  b=$(basename "$f")
  case "$b" in stdout.txt | stderr.txt) continue ;; esac
  for who in fsl17 lx; do
    if [ -f "$dir/$who/$b" ]; then
      if [ "${b%.gz}" != "$b" ]; then
        # compressed: the content (the gzip stream's own bytes may differ)
        if cmp -s <(gzip -dc "$f") <(gzip -dc "$dir/$who/$b"); then
          echo "same   fsl23 $who $b (content)"
        else
          echo "DIFFER fsl23 $who $b (content)"
        fi
      else
        cmp -s "$f" "$dir/$who/$b" && echo "same   fsl23 $who $b" || echo "DIFFER fsl23 $who $b"
      fi
    else
      echo "MISSING in $who: $b"
    fi
  done
done
