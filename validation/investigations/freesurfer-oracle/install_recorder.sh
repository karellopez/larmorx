#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Put fs_time_recorder.py in place of FreeSurfer's fs_time in an unpacked FreeSurfer
# (its own kept as bin/fs_time.orig; without LARMORX_FS_RECORD the recorder only runs it).
#   install_recorder.sh <freesurfer home>
set -eu
FS=$1
D=$(cd "$(dirname "$0")" && pwd)
if [ ! -e "$FS/bin/fs_time.orig" ]; then
  mv "$FS/bin/fs_time" "$FS/bin/fs_time.orig"
fi
cp "$D/fs_time_recorder.py" "$FS/bin/fs_time"
chmod 755 "$FS/bin/fs_time"
echo "recorder installed: $(sha256sum "$FS/bin/fs_time" | cut -d' ' -f1); original: $(sha256sum "$FS/bin/fs_time.orig" | cut -d' ' -f1)"
