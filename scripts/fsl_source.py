# SPDX-License-Identifier: Apache-2.0
"""Copy FSL's original source files of the replicated parts into ``crates-nc/fsl-source/``.

The FSL Licence lets larmorx-nc pass its translations on only if "all original and amended
source code is included in any transmitted product" (``crates-nc/LICENSE``): the amended
source is the Rust translation in ``crates-nc/``, the original source the FSL files it was
translated from, copied here unchanged from an FSL installation's ``$FSLDIR/src`` (FSL 6.0.7.17,
the packages pinned below). ``fsl-source/MANIFEST.tsv`` lists every file with its package,
version and SHA-256; ``--check`` verifies the copies against it (no FSL needed).

Usage:
    python scripts/fsl_source.py --fsldir ~/fsl      # copy the files and write the manifest
    python scripts/fsl_source.py --check             # verify the copies against the manifest

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

#: The FSL packages the replicas are translated from (FSL 6.0.7.17's conda packages).
VERSIONS = {
    "fsl-mcflirt": "2111.0",
    "fsl-newimage": "2203.12",
    "fsl-miscmaths": "2203.2",
    "fsl-newnifti": "4.1.0",
    "fsl-znzlib": "2111.0",
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
    ("fsl-znzlib", "znzlib.h"),
    ("fsl-znzlib", "znzlib.c"),
]

MANIFEST = DEST / "MANIFEST.tsv"
HEADER = "package\tversion\tfile\tsha256\n"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def installed_version(fsldir: Path, package: str) -> str | None:
    """The version of ``package`` recorded in ``$FSLDIR/src/<package>``'s conda metadata."""
    meta = fsldir / "src" / package / "metadata_conda_debug.yaml"
    if not meta.is_file():
        return None
    m = re.search(r"^\s*version:\s*'?([^'\s]+)'?", meta.read_text(encoding="utf-8"), re.M)
    return m.group(1) if m else None


def copy(fsldir: Path) -> int:
    for package, want in VERSIONS.items():
        got = installed_version(fsldir, package)
        if got != want:
            print(f"error: {fsldir}/src/{package} is version {got}, expected {want}")
            return 1
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
    g.add_argument("--fsldir", type=Path, help="an FSL 6.0.7.17 installation with its src/")
    g.add_argument("--check", action="store_true", help="verify the copies against the manifest")
    a = ap.parse_args()
    return check() if a.check else copy(a.fsldir.expanduser())


if __name__ == "__main__":
    sys.exit(main())
