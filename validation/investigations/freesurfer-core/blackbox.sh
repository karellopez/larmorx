#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Black-box checks of FreeSurfer 8.2.0's core services on its official mri_info (FS-01, FS-02,
# DiagInit's lines): docs/findings/freesurfer-core.md, specs/freesurfer-core.md.
# Usage: blackbox.sh <FREESURFER_HOME>   (the unpacked official package: its bin/mri_info)
# Prints each case's standard output, standard error (bytes made visible with od -c) and status.
set -u
FS=$1
M=$FS/bin/mri_info
T=$(mktemp -d)
run() {
  local name=$1; shift
  env FREESURFER_HOME="$FS" "$@" "$M" /nonexistent.mgz > "$T/out" 2> "$T/err"
  local status=$?
  echo "== $name: exit $status"
  echo "-- stdout"; cat "$T/out"
  echo "-- stderr (od -c)"; od -c "$T/err"
}
run "missing file (FS-01: a NUL byte before the newline)"
run "PROFILE=7.9 (truncated)" PROFILE=7.9
run "PROFILE=1e10 (FS-02)" PROFILE=1e10
run "PROFILE=nan (FS-02)" PROFILE=nan
run "DIAG_VERBOSE empty" DIAG_VERBOSE=
run "DIAG=0x10000000" DIAG=0x10000000
rm -rf "$T"
