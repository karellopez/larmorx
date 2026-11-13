#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# The official FreeSurfer 8.2.0 mri_info as a black box, for specs/freesurfer-core.md §20.4: a file
# that does not exist (the C-style error's bytes and the exit status), the diagnostic variables, the
# version options. Prints each case's exit status, standard output and standard error (bytes made
# visible by od -c), with the time, the user, the machine and the program's folder masked.
# Usage: official.sh <FREESURFER_HOME>   (FreeSurfer 8.2.0's unpacked package: its bin/mri_info)
set -u
FS=${1:?FREESURFER_HOME}
M=$FS/bin/mri_info
T=$(mktemp -d)
mask() {
  sed -e "s/User: [^ ]*/User: <user>/" -e "s/Machine: [^ ]*/Machine: <machine>/" \
    -e "s/TimeStamp: [^ ]*/TimeStamp: <now>/" -e "s/PlatformVersion: [^ ]*/PlatformVersion: <release>/" \
    -e "s#$M#<mri_info>#g"
}
run() {
  local name=$1
  shift
  env -i PATH=/usr/bin:/bin LC_ALL=C FREESURFER_HOME="$FS" "$@" > "$T/out" 2> "$T/err"
  local status=$?
  echo "== $name: exit $status"
  echo "-- stdout"
  mask < "$T/out"
  echo "-- stderr (od -c)"
  mask < "$T/err" | od -c
}
run "missing file" "$M" /nonexistent.mgz
run "PROFILE=7.9" PROFILE=7.9 "$M" /nonexistent.mgz
run "PROFILE=1e10" PROFILE=1e10 "$M" /nonexistent.mgz
run "PROFILE=nan" PROFILE=nan "$M" /nonexistent.mgz
run "DIAG_VERBOSE empty" DIAG_VERBOSE= "$M" /nonexistent.mgz
run "DIAG=0x10000000" DIAG=0x10000000 "$M" /nonexistent.mgz
run "--version" "$M" --version
run "--all-info" "$M" --all-info
run "--versionx -version --all-info" "$M" --versionx -version --all-info
run "--version --version x" "$M" --version --version x
rm -rf "$T"
