#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# bet2 of FSL 6.0.7.17 and 6.0.7.23 and the replica (both D13 modes) on the same arguments, each
# in a directory of its own under <scratch>, with their messages and exit statuses.
# Usage: bet2_compare.sh <scratch dir> <input image> [bet2 options...]
set -u
out=$1
img=$2
shift 2
here=$(cd "$(dirname "$0")/../../.." && pwd)
nc=${LARMORX_NC_BIN:-$here/crates-nc/target/release/larmorx-nc}
mkdir -p "$out"
for who in fsl-6.0.7.17 fsl-6.0.7.23 replica-fix replica-reproduce; do
  d=$out/$who
  rm -rf "$d"
  mkdir -p "$d"
  case $who in
    fsl-6.0.7.17) cmd=("$HOME/fsl/bin/bet2") ;;
    fsl-6.0.7.23) cmd=("$HOME/fsl-6.0.7.23/bin/bet2") ;;
    replica-*) cmd=("$nc" fsl bet2) ;;
  esac
  mode=${who#replica-}
  (cd "$d" && env -i HOME="$HOME" PATH=/usr/bin:/bin FSLOUTPUTTYPE=NIFTI_GZ \
    LARMORX_UPSTREAM_DEFECTS="$mode" "${cmd[@]}" "$img" out "$@" >stdout.txt 2>stderr.txt)
  echo "== $who: exit $? ($(cd "$d" && ls | grep -v std | tr '\n' ' '))"
  head -c 600 "$d/stderr.txt"
done
