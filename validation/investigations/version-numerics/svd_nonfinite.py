# SPDX-License-Identifier: Apache-2.0
"""numpy 2.5's SVD of a matrix holding a NaN or an infinity (numpy/linalg/umath_linalg.cpp: the
input is checked first, ``all_finite``, and the LAPACK driver is no longer called), against
numpy 2.3's, and what reaches fMRIPrep's resampler through nibabel's ``io_orientation``
(sdcflows' ``ensure_positive_cosines``) and larmorx's ``lx.transforms`` on such a grid.

Usage: ``python -P svd_nonfinite.py`` under each environment (the shared .venv, then
``oracles/python-newest``)."""

from __future__ import annotations

import sys
import warnings

import numpy as np


def main() -> int:
    print("numpy", np.__version__)
    for bad in (np.nan, np.inf):
        rs = np.array([[2.0, 0.1, 0.0], [0.0, 2.0, bad], [0.0, 0.0, 3.0]])
        try:
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                _u, s, _vt = np.linalg.svd(rs)
            print(f"svd with {bad}: singular values {s}")
        except np.linalg.LinAlgError as e:
            print(f"svd with {bad}: LinAlgError: {e}")
        aff = np.eye(4)
        aff[:3, :3] = rs
        try:
            import nibabel as nib

            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                print(f"  io_orientation: {nib.orientations.io_orientation(aff).tolist()}")
        except Exception as e:
            print(f"  io_orientation: {type(e).__name__}: {e}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
