#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Generates crates/larmorx-io/src/dicom/dict_table.rs from CTN 3.0.6's dcmdict.c (gen_dict.c) and
# checks that FreeSurfer 8.2.0's copy gives the same table.
# Usage: gen_dict.sh <scratch-dir> [<ctn-3.0.6>] [<freesurfer-8.2.0>]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
ws=$repo/..
case "$repo" in */.claude/worktrees*) ws=${repo%%/larmorx/.claude/worktrees*} ;; esac
out=${1:?usage: gen_dict.sh <scratch-dir> [<ctn-3.0.6>] [<freesurfer-8.2.0>]}
ctn=$(cd "${2:-$ws/tags/ctn-3.0.6}/facilities" && pwd)
fs=$(cd "${3:-$ws/tags/freesurfer-8.2.0}/packages/dicom" && pwd)
mkdir -p "$out"
export PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)
CC=${CC:-gcc}
D=(-DLITTLE_ENDIAN_ARCHITECTURE -DLinux -w)
"$CC" "${D[@]}" -DSHORTSIZE=16 -DINTSIZE=32 -DLONGSIZE=64 -I"$here/up-include" -I"$ctn/dicom" \
  -I"$ctn/objects" -I"$ctn/condition" -I"$ctn/lst" -I"$ctn/uid" -I"$ctn/thread" \
  "$here/gen_dict.c" "$ctn/condition/condition.c" "$ctn/objects/dcmcond.c" -o "$out/gen_dict_up"
"$CC" "${D[@]}" -I"$fs" "$here/gen_dict.c" "$fs/condition.c" "$fs/dcmcond.c" -o "$out/gen_dict_fs"
"$out/gen_dict_up" > "$out/dict_table_up.rs"
"$out/gen_dict_fs" > "$out/dict_table_fs.rs"
cmp "$out/dict_table_up.rs" "$out/dict_table_fs.rs"
cp "$out/dict_table_up.rs" "$repo/crates/larmorx-io/src/dicom/dict_table.rs"
echo "dict_table.rs: $(grep -c 'Entry {' "$out/dict_table_up.rs") entries in $(grep -c '^    (0x' "$out/dict_table_up.rs") groups; FreeSurfer's copy identical"
