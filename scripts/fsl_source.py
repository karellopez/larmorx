# SPDX-License-Identifier: Apache-2.0
"""Copy FSL's original source files of the replicated parts into ``crates-nc/fsl-source/``.

The FSL Licence lets larmorx-nc pass its translations on only if "all original and amended
source code is included in any transmitted product" (``crates-nc/LICENSE``): the amended
source is the Rust translation in ``crates-nc/``, the original source the FSL files it was
translated from, copied here unchanged from an FSL installation's ``$FSLDIR/src`` (FSL 6.0.7.23
since 2026-10-29, the packages pinned below; FSL 6.0.7.17's before, in git history and in its
own installation). ``fsl-source/MANIFEST.tsv`` lists every file with its package, version and
SHA-256; ``--check`` verifies the copies against it (no FSL needed). A copy replaces the
directory's previous files: files no longer translated from (FSL 6.0.7.17's ``znzlib.c``) leave
it with the version they belonged to.

Usage:
    python scripts/fsl_source.py --fsldir ~/fsl-6.0.7.23   # copy the files, write the manifest
    python scripts/fsl_source.py --check                   # verify the copies

Each replica adds the files it translates to ``FILES`` (package, file).
"""

from __future__ import annotations

import argparse
import hashlib
import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEST = ROOT / "crates-nc" / "fsl-source"

#: The FSL packages the replicas are translated from (FSL 6.0.7.23's conda packages; FSL
#: 6.0.7.17's were fsl-newimage 2203.12, fsl-miscmaths 2203.2, fsl-newnifti 4.1.0, fsl-znzlib
#: 2111.0, fsl-bet2 2111.8, fsl-utils 2203.5, fsl-avwutils 2209.3, the others the same).
VERSIONS = {
    "fsl-mcflirt": "2111.0",
    "fsl-newimage": "2601.0",
    "fsl-miscmaths": "2412.6",
    "fsl-newnifti": "5.0.0",
    "fsl-znzlib": "2511.3",
    "fsl-bet2": "2111.9",
    "fsl-meshclass": "2111.0",
    "fsl-utils": "2412.6",
    "fsl-avwutils": "2209.6",
}

#: The original files: every file a translation in crates-nc/ comes from (see each crate's
#: PROVENANCE.md).
FILES = [
    ("fsl-mcflirt", "mcflirt.cc"),
    ("fsl-mcflirt", "Globaloptions.h"),
    ("fsl-mcflirt", "Globaloptions.cc"),
    ("fsl-mcflirt", "Log.h"),
    ("fsl-mcflirt", "Log.cc"),
    ("fsl-newimage", "newimage.h"),
    ("fsl-newimage", "newimage.cc"),
    ("fsl-newimage", "newimagefns.h"),
    ("fsl-newimage", "newimagefns.cc"),
    ("fsl-newimage", "newimageio.h"),
    ("fsl-newimage", "generalio.cc"),
    ("fsl-newimage", "costfns.h"),
    ("fsl-newimage", "costfns.cc"),
    ("fsl-miscmaths", "miscmaths.h"),
    ("fsl-miscmaths", "miscmaths.cc"),
    ("fsl-miscmaths", "optimise.h"),
    ("fsl-miscmaths", "optimise.cc"),
    ("fsl-miscmaths", "kernel.h"),
    ("fsl-miscmaths", "kernel.cc"),
    ("fsl-miscmaths", "splinterpolator.h"),
    ("fsl-newnifti", "NewNifti.h"),
    ("fsl-newnifti", "NewNifti.cc"),
    ("fsl-newnifti", "legacyFunctions.h"),
    ("fsl-newnifti", "legacyFunctions.cc"),
    ("fsl-newnifti", "nifti1.h"),
    ("fsl-newnifti", "nifti2.h"),
    # znzlib 2511.3 (FSL 6.0.7.23) is C++ with one plugin per format; 2111.0's znzlib.c is gone
    ("fsl-znzlib", "znzlib.h"),
    ("fsl-znzlib", "znzlib.cpp"),
    ("fsl-znzlib", "raw_plugin.h"),
    ("fsl-znzlib", "raw_plugin.cpp"),
    ("fsl-znzlib", "gzip_plugin.h"),
    ("fsl-znzlib", "gzip_plugin.cpp"),
    ("fsl-znzlib", "zlib_wrapper.h"),
    ("fsl-znzlib", "zstd_reader.h"),
    ("fsl-znzlib", "zstd_reader.cpp"),
    ("fsl-znzlib", "zstd_writer.h"),
    ("fsl-znzlib", "zstd_writer.cpp"),
    ("fsl-znzlib", "bzip2_plugin.h"),
    ("fsl-znzlib", "bzip2_plugin.cpp"),
    # BET (larmorx-nc-bet; its libraries in larmorx-nc-core)
    ("fsl-bet2", "bet"),
    ("fsl-bet2", "bet2.cpp"),
    ("fsl-bet2", "betsurf.cpp"),
    ("fsl-bet2", "betsurf.h"),
    ("fsl-meshclass", "meshclass.h"),
    ("fsl-meshclass", "point.h"),
    ("fsl-meshclass", "point.cpp"),
    ("fsl-meshclass", "mpoint.h"),
    ("fsl-meshclass", "mpoint.cpp"),
    ("fsl-meshclass", "triangle.h"),
    ("fsl-meshclass", "triangle.cpp"),
    ("fsl-meshclass", "mesh.h"),
    ("fsl-meshclass", "mesh.cpp"),
    ("fsl-meshclass", "pt_special.h"),
    ("fsl-meshclass", "pt_special.cpp"),
    ("fsl-meshclass", "selfintersection.cpp"),
    ("fsl-meshclass", "profile.h"),
    ("fsl-meshclass", "profile.cpp"),
    ("fsl-utils", "options.h"),
    ("fsl-utils", "options.cc"),
    ("fsl-utils", "parse.cc"),
    ("fsl-utils", "usage.cc"),
    ("fsl-utils", "functions.cc"),
    ("fsl-utils", "matches.cc"),
    ("fsl-avwutils", "fslstats.cc"),
    ("fsl-avwutils", "fslroi.cc"),
    ("fsl-avwutils", "fslmerge.cc"),
    ("fsl-avwutils", "fslmaths.cc"),
    ("fsl-avwutils", "fslval"),
]

MANIFEST = DEST / "MANIFEST.tsv"
HEADER = "package\tversion\tfile\tsha256\n"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def installed_version(fsldir: Path, package: str) -> str | None:
    """The version of ``package`` recorded in ``$FSLDIR/src/<package>``'s conda metadata, else
    (a package built without it: fsl-znzlib 2511.3) in ``$FSLDIR/conda-meta``."""
    meta = fsldir / "src" / package / "metadata_conda_debug.yaml"
    if meta.is_file():
        text = meta.read_text(encoding="utf-8")
        m = re.search(r"^\s*version:\s*'?([^'\s]+)'?", text, re.M)
        return m.group(1) if m else None
    for record in sorted((fsldir / "conda-meta").glob(f"{package}-[0-9]*.json")):
        m = re.match(re.escape(package) + r"-([0-9][^-]*)-", record.name)
        if m:
            return m.group(1)
    return None


def copy(fsldir: Path) -> int:
    for package, want in VERSIONS.items():
        got = installed_version(fsldir, package)
        if got != want:
            print(f"error: {fsldir}/src/{package} is version {got}, expected {want}")
            return 1
    # the previous version's files leave the directory (git keeps them with their version)
    for old in DEST.glob("*/*"):
        if old.is_file() and (old.parent.name, old.name) not in FILES:
            old.unlink()
    rows = []
    for package, name in FILES:
        src = fsldir / "src" / package / name
        dst = DEST / package / name
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(src, dst)
        rows.append(f"{package}\t{VERSIONS[package]}\t{name}\t{sha256(dst)}\n")
    MANIFEST.write_text(HEADER + "".join(rows), encoding="utf-8", newline="\n")
    print(f"copied {len(rows)} files into {DEST.relative_to(ROOT)}")
    return 0


def check() -> int:
    if not MANIFEST.is_file():
        print(f"error: {MANIFEST.relative_to(ROOT)} is missing")
        return 1
    lines = MANIFEST.read_text(encoding="utf-8").splitlines()[1:]
    listed = set()
    bad = 0
    for line in lines:
        package, version, name, digest = line.split("\t")
        listed.add((package, name))
        path = DEST / package / name
        if VERSIONS.get(package) != version:
            print(f"error: {package} {version} in the manifest, {VERSIONS.get(package)} pinned")
            bad += 1
        if not path.is_file() or sha256(path) != digest:
            print(f"error: {path.relative_to(ROOT)} is missing or changed")
            bad += 1
    for entry in FILES:
        if entry not in listed:
            print(f"error: {entry[0]}/{entry[1]} is not in the manifest (run with --fsldir)")
            bad += 1
    if bad == 0:
        print(f"{len(lines)} FSL source files match {MANIFEST.relative_to(ROOT)}")
    return 1 if bad else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    g = ap.add_mutually_exclusive_group(required=True)
    g.add_argument("--fsldir", type=Path, help="an FSL 6.0.7.23 installation with its src/")
    g.add_argument("--check", action="store_true", help="verify the copies against the manifest")
    a = ap.parse_args()
    return check() if a.check else copy(a.fsldir.expanduser())


if __name__ == "__main__":
    sys.exit(main())
