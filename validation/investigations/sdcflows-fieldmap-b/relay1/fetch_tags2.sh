#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# The second batch of part B's pins (sdists from PyPI, SHA-256 checked against PyPI's JSON),
# unpacked into <workspace>/tags/<name>/ (the sdist kept in <name>-download/).
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
fetch nipype-1.12.0 https://files.pythonhosted.org/packages/e2/b0/430436448e7f30f0cb992f9b0a14c7e38ab16267f49494d4fb549c6a1601/nipype-1.12.0.tar.gz 784098a8a54839836fa0eba2fa1c2040a365a272d0b28692578b5b15b475aeeb
fetch pybids-0.22.0 https://files.pythonhosted.org/packages/ba/61/48e48fef36afbab3635bc4f7341641c4ed4f83bcdc3dfb484cc90a63b750/pybids-0.22.0.tar.gz 98e7957e47c08d3c131de6f6cef629d009abce888849102b33820ca6a9930152
fetch scikit-image-0.25.2 https://files.pythonhosted.org/packages/c7/a8/3c0f256012b93dd2cb6fda9245e9f4bff7dc0486880b248005f15ea2255e/scikit_image-0.25.2.tar.gz e5a37e6cd4d0c018a7a55b9d601357e3382826d3888c10d0213fc63bff977dde
