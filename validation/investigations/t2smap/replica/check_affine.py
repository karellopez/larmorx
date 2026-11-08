# SPDX-License-Identifier: Apache-2.0
"""The affine the t2smap replica reads (nibabel's ``img.affine``: ``get_best_affine``, its qform
in numpy's ``longdouble``) against nibabel's own, bit for bit, on generated headers.

Random testing (``difftest --tools tedana-t2smap``, 2026-11-08) found the replica's outputs'
``srow`` fields holding ``-0.0`` where tedana's hold ``+0.0`` for images with only a qform
(``sform_code`` 0): larmorx-io's plain float64 qform computes ``R[i][j] * vox[j]``, nibabel's
``np.dot(R, diag(vox))`` sums from ``+0`` in x87 extended precision (and rounds oblique rotations'
entries differently). This script writes NIfTI-1 and NIfTI-2 headers (rational and random float32
quaternions, the threshold of ``w``, qfac -1 and 1, voxel sizes, sform and qform codes, an sform
of its own), reads each with the replica's ``nibabel_affine`` example and with nibabel, and counts
the files whose 16 values differ in any bit. Usage:

    cargo build --release --manifest-path crates-gpl/Cargo.toml -p larmorx-gpl-tedana \\
        --example nibabel_affine
    <env>/bin/python -P validation/investigations/t2smap/replica/check_affine.py \\
        --example <target>/release/examples/nibabel_affine --work <empty dir> [--n 4000]
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

import nibabel as nb
import numpy as np


def headers(n: int, seed: int = 20261108):
    rng = np.random.default_rng(seed)
    axis = [(1, 0, 0, 0), (0, 1, 0, 0), (0, 0, 1, 0), (0, 0, 0, 1), (1, 1, 0, 0), (1, 0, -1, 0)]
    for k in range(n):
        kind = k % 6
        if kind == 0:  # an axis permutation or flip, maybe combined with a small rotation
            q = np.array(axis[rng.integers(0, len(axis))], dtype=np.float64)
        elif kind == 1:  # a rational small rotation (gen's oblique ones)
            q = np.array([rng.integers(4, 40), *rng.integers(-3, 4, 3)], dtype=np.float64)
        elif kind == 2:  # random unit quaternion
            q = rng.standard_normal(4)
        elif kind == 3:  # w near the threshold: b^2 + c^2 + d^2 close to 1
            v = rng.standard_normal(3)
            q = np.array([rng.choice([0.0, 1e-4, 3e-4, 6e-4]), *(v / np.linalg.norm(v))])
        elif kind == 4:  # tiny rotations
            q = np.array([1.0, *(rng.standard_normal(3) * 1e-6)])
        else:  # a scanner-like oblique
            q = np.array([1.0, *(rng.standard_normal(3) * 0.1)])
        q = q / np.linalg.norm(q)
        if q[0] < 0:
            q = -q
        nifti2 = k % 7 == 3
        cls = nb.Nifti2Image if nifti2 else nb.Nifti1Image
        img = cls(np.zeros((3, 4, 5), np.float32), np.eye(4))
        h = img.header
        h["quatern_b"], h["quatern_c"], h["quatern_d"] = q[1:]
        h["qoffset_x"], h["qoffset_y"], h["qoffset_z"] = rng.integers(-800, 800, 3) / 8
        pix = rng.choice([0.5, 0.8, 1.0, 1.2, 1.5, 2.0, 2.4, 3.0, 3.3, 4.0], 3)
        if k % 5 == 0:
            pix = rng.uniform(0.3, 4.0, 3)
        h["pixdim"][1:4] = pix
        h["pixdim"][0] = rng.choice([-1.0, 1.0])
        h["qform_code"] = rng.choice([1, 2, 3, 4])
        h["sform_code"] = 0 if k % 9 else rng.choice([1, 2])
        if k % 13 == 0:
            h["qform_code"] = 0
            h["sform_code"] = 0
        if h["sform_code"]:
            h["srow_x"] = rng.uniform(-3, 3, 4)
            h["srow_y"] = rng.uniform(-3, 3, 4)
            h["srow_z"] = rng.uniform(-3, 3, 4)
        yield k, img


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--example", required=True)
    ap.add_argument("--work", required=True, type=Path)
    ap.add_argument("--n", type=int, default=4000)
    a = ap.parse_args()
    a.work.mkdir(parents=True, exist_ok=True)
    paths, expected = [], []
    for k, img in headers(a.n):
        p = a.work / f"h{k:05d}.nii"
        # The header as set (to_filename would recompute nothing for a NIfTI header written
        # from its own fields, but img.update_header() would: write the raw block).
        img.header.set_data_shape((3, 4, 5))
        with open(p, "wb") as f:
            img.header.write_to(f)
            f.write(b"\x00" * (int(img.header["vox_offset"]) - f.tell()))
            f.write(np.zeros((3, 4, 5), np.float32).tobytes())
        paths.append(p)
        try:
            aff = nb.load(p).affine
            expected.append(" ".join(f"{np.float64(v).view(np.uint64):016x}" for v in aff.ravel()))
        except Exception as e:  # nibabel refuses the header
            expected.append(f"ERROR {e}")
    out = []
    for i in range(0, len(paths), 500):
        chunk = [str(p) for p in paths[i : i + 500]]
        out += subprocess.run(
            [a.example, *chunk], capture_output=True, text=True, check=True
        ).stdout.splitlines()
    differ = [
        (p.name, e, o)
        for p, e, o in zip(paths, expected, out, strict=True)
        if e != o and not (e.startswith("ERROR") and o.startswith("ERROR"))
    ]
    errors = sum(e.startswith("ERROR") for e in expected)
    print(f"{len(paths)} headers, {errors} refused by nibabel, {len(differ)} differ")
    for name, e, o in differ[:10]:
        print(name, "\n  nibabel", e, "\n  replica", o)
    return 1 if differ else 0


if __name__ == "__main__":
    sys.exit(main())
