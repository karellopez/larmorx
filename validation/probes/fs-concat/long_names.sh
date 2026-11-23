#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it measures the official build for the replica's
# FS-1197 and FS-1198).
#
# The official mri_concat (FreeSurfer 8.2.0) with --f list file names of 1,998 to 2,013 bytes that
# open (nested folders of 250-byte names in <folder>; a name's last part past 255 bytes does not
# open) and with --f lists of 400,000 to 401,000 words. Measured 2026-11-23 (relay 55 of track A):
# names of up to 2,008 bytes run normally, 2,009 to 2,012 bytes abort with "*** stack smashing
# detected ***: terminated" (status 134) once the command line has been read; a list of 400,000
# words reads the first input's header, 400,001 and more crash (status 139). Run it in a memory box:
#
#     scripts/orchestration/boxed.sh 4 validation/probes/fs-concat/long_names.sh <empty folder>
set -u
M=$1
REPO=$(cd "$(dirname "$0")/../../.." && pwd)
source "$REPO/scripts/orchestration/env.sh" > /dev/null 2>&1
FSH=$LARMORX_WORKSPACE/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0
LIC=${FS_LICENSE:-/usr/local/freesurfer/license.txt}
cd "$M"
python3 -c "
import struct
h=struct.pack('>7i',1,2,2,1,1,3,0)+struct.pack('>h',0)
h+=b'\0'*(284-len(h))
open('a.mgh','wb').write(h+struct.pack('>4f',1,2,3,4))"
run() {
  timeout 60 env -i PATH="$FSH/bin:/usr/bin:/bin" FREESURFER_HOME="$FSH" FS_LICENSE="$LIC" \
    FS_DISABLE_LANG=1 LC_ALL=C mri_concat "$@" > out.txt 2> err.txt
  echo "status $? out: $(head -c 100 out.txt | tr '\n' '|') err: $(head -c 100 err.txt | tr '\n' '|')"
}
d=$(python3 -c "print('/'.join(['d'*250]*7))")
mkdir -p "$d"
for n in 1998 1999 2000 2001 2002 2008 2009 2010 2011 2012 2013; do
  p=$(python3 -c "d='/'.join(['d'*250]*7); print(d + '/' + 'l'*($n - len(d) - 5) + '.txt')")
  (echo "a.mgh" > "$p") 2> /dev/null
  echo -n "name of ${#p} bytes: "
  run --f "$p" --o o.mgh
done
for n in 400000 400001 400100 401000; do
  python3 -c "open('l$n.txt','w').write('nofile.mgz\n'*$n)"
  echo -n "list of $n words: "
  run --f "l$n.txt" --o o.mgh
done
