#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# pybids 0.20.0 (fMRIPrep 25.2.6's image): the sdist from PyPI, SHA-256 checked against PyPI's JSON,
# unpacked into <workspace>/tags/pybids-0.20.0/ (the sdist kept in pybids-0.20.0-download/).
set -u
T=${LARMORX_WORKSPACE:-$HOME/VScode_projects/super_fmriprep}/tags  # scripts/orchestration/env.sh sets it
name=pybids-0.20.0
url=https://files.pythonhosted.org/packages/8b/7a/595b5608054f73c0ed09817bfb6365d0931c27d929c0c0e70f8e1018c24f/pybids-0.20.0.tar.gz
sha=a68b577426942e50e6ac37e011463361d00a378d19b75df817619450357ebbb6
if [ -d "$T/$name" ]; then echo "exists: $name"; exit 0; fi
mkdir -p "$T/$name-download"
f="$T/$name-download/$(basename "$url")"
timeout 300 curl -sSfL -o "$f" "$url" || { echo "download failed"; exit 1; }
got=$(sha256sum "$f" | cut -d' ' -f1)
[ "$got" = "$sha" ] || { echo "SHA MISMATCH $got"; exit 1; }
mkdir -p "$T/$name.tmp"
tar -xzf "$f" -C "$T/$name.tmp"
inner=$(ls "$T/$name.tmp")
mv "$T/$name.tmp/$inner" "$T/$name"
rmdir "$T/$name.tmp"
echo "ok: $name sha256 $got"
