#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Download the newest releases part B ports from (sdists from PyPI), check SHA-256 against PyPI's
# JSON, and unpack each into <workspace>/tags/<name>/ (sdist kept in <name>-download/).
set -u
T=${LARMORX_WORKSPACE:?}/tags
fetch() {
  name=$1; url=$2; sha=$3
  if [ -d "$T/$name" ]; then echo "exists: $name"; return; fi
  mkdir -p "$T/$name-download"
  f="$T/$name-download/$(basename "$url")"
  timeout 300 curl -sSfL -o "$f" "$url" || { echo "download failed: $name"; return; }
  got=$(sha256sum "$f" | cut -d' ' -f1)
  if [ "$got" != "$sha" ]; then echo "SHA MISMATCH $name: $got"; return; fi
  mkdir -p "$T/$name.tmp"
  tar -xzf "$f" -C "$T/$name.tmp"
  inner=$(ls "$T/$name.tmp")
  mv "$T/$name.tmp/$inner" "$T/$name"
  rmdir "$T/$name.tmp"
  echo "ok: $name sha256 $got"
}
fetch niworkflows-1.15.4 https://files.pythonhosted.org/packages/34/70/5d9c503727336498aab22c238f9a3d9af549ca039c82c1f3e6054d251faf/niworkflows-1.15.4.tar.gz 58e514502c4ccacd25557bca305e427e1b137defbd0de3974e9407bd9be07dd3
fetch fmriprep-25.2.7 https://files.pythonhosted.org/packages/81/92/8be5e572eb354b587c901a5f17ea6dc57360a29338cb9afc4475c08247ca/fmriprep-25.2.7.tar.gz 739fd88af47ff62c2677e789fd85764f44538b7210797ce52ebf95737c189c3e
fetch scikit-image-0.26.0 https://files.pythonhosted.org/packages/a1/b4/2528bb43c67d48053a7a649a9666432dc307d66ba02e3a6d5c40f46655df/scikit_image-0.26.0.tar.gz f5f970ab04efad85c24714321fcc91613fcb64ef2a892a13167df2f3e59199fa
fetch sdcflows-2.15.2 https://files.pythonhosted.org/packages/0d/14/1822d7a825db1afaa02ec27dc44181204b282a2b5330f9cdb9bb6ede85d0/sdcflows-2.15.2.tar.gz 7f7c6c4abd01dce37a34dff1217820e07b32df34630c5e3331e8da2d8b1775b3
ls "$T"
