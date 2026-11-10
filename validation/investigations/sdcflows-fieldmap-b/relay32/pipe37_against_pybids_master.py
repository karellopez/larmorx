# SPDX-License-Identifier: Apache-2.0
"""PIPE-37: larmorx's index (``lx.bids.BIDSLayout``, its default: the patterns matched below the
root) against pybids' master, whose #1273 (2026-08-24, closing #1270, not in a release yet)
matches them below the root too, on the reproducer's dataset below four folders: every file's
entities, raw and derivatives. Run with pybids' master first on the path (a copy of its ``src/``)
and larmorx's ``python/``:

    PYTHONPATH=<pybids master src>:<repo>/python python -P pipe37_against_pybids_master.py

Prints one line per folder and ``ALL OK`` when every file's entities agree.
"""

import json
import sys
import tempfile
import warnings
from pathlib import Path

import bids
import nibabel as nb
import numpy as np

import larmorx as lx


def nifti(path):
    path.parent.mkdir(parents=True, exist_ok=True)
    nb.Nifti1Image(np.zeros((4, 4, 4), np.int16), np.eye(4)).to_filename(path)


def entities(layout, scope):
    return {
        Path(f.path).name: dict(sorted(f.get_entities().items()))
        for f in layout.get(scope=scope, extension=".nii.gz")
    }


def main() -> int:
    assert "relative_fn" in Path(bids.layout.index.__file__).read_text(), "not pybids' master"
    raw = {"Name": "t", "BIDSVersion": "1.8.0"}
    deriv = {**raw, "DatasetType": "derivative", "GeneratedBy": [{"Name": "fmriprep"}]}
    base = Path(tempfile.mkdtemp())
    ok = True
    for parent in ("plain", "sub-pilot", "func", "andres-lab"):
        root = base / parent / "ds"
        for sub in ("01", "02"):
            nifti(root / f"sub-{sub}/anat/sub-{sub}_T1w.nii.gz")
        (root / "dataset_description.json").write_text(json.dumps(raw))
        out = root / "derivatives/fmriprep"
        nifti(out / "sub-01/anat/sub-01_space-MNI152NLin2009cAsym_res-2_desc-preproc_T1w.nii.gz")
        (out / "dataset_description.json").write_text(json.dumps(deriv))
        theirs = bids.BIDSLayout(root, validate=False, derivatives=str(out))
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            mine = lx.bids.BIDSLayout(root, validate=False, derivatives=str(out))
        for scope in ("raw", "derivatives"):
            a, b = entities(theirs, scope), entities(mine, scope)
            same = a == b and len(a) > 0
            ok &= same
            print(f"{'OK' if same else 'FAIL'} {parent} {scope}: {len(a)} files"
                  + ("" if same else f"\n  pybids {a}\n  larmorx {b}"))  # fmt: skip
    print("ALL OK" if ok else "FAILED")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
