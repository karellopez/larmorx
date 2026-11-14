#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# FS-680's reproducer: FreeSurfer 8.2.0's own mri_convert (the official Ubuntu 22.04 package, unpacked
# in <workspace>/oracles/freesurfer-8.2.0/root) on make_series.py's two GE series. FreeSurfer's
# licence file is read in place
# (FS_LICENSE, default /usr/local/freesurfer/license.txt).
#
#   validation/investigations/freesurfer-ge-genesis/run.sh <workspace> <out>
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ws=$1
out=$2
fs=$ws/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0
python3 -P "$here/make_series.py" "$out"
# each series by name alone, then with its input type given (-it genesis, -it gelx)
for c in "genesis/I.002" "-it genesis genesis/I.002" "lx/e1s1i2" "-it gelx lx/e1s1i2"; do
  echo "== FreeSurfer 8.2.0: mri_convert $c out.mgz"
  # shellcheck disable=SC2086
  (cd "$out" && env -i PATH=/usr/bin:/bin FREESURFER_HOME="$fs" \
    FS_LICENSE="${FS_LICENSE:-/usr/local/freesurfer/license.txt}" "$fs/bin/mri_convert" $c out.mgz 2>&1)
  echo "exit status $?"
  rm -f "$out/out.mgz"
done
