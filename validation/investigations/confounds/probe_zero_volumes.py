# SPDX-License-Identifier: Apache-2.0
"""nipype's ``compute_noise_components`` against larmorx's (``reproduce``) on a series with no
volume left (CompCor's ``ignore_initial_volumes`` at least the series' length): every pre-filter
(with and without a repetition time for the cosines), both failure modes, full, empty and
other-shaped masks in several orders, four criteria. nipype's ``regress_poly`` and
``cosine_filter`` reshape a mask's rows to ``(-1, 0)`` (numpy's ``ValueError: cannot reshape array
of size 0 into shape (0)``) unless the mask is empty and ``failure_mode`` is not ``"error"``, and
its loop stops at a mask that gives no component (PIPE-44). Prints ``K of N identical``.

    OPENBLAS_CORETYPE=Haswell PYTHONPATH=<repo>/python python -P probe_zero_volumes.py
"""

import itertools
import os
import tempfile
import warnings

import nibabel as nb
import numpy as np
from nipype.algorithms.confounds import compute_noise_components as ncc

import larmorx.confounds as lc


def outcome(f):
    try:
        r = f()
        return ("ok", [np.asarray(x).shape for x in r[:2]], {k: len(v) for k, v in r[2].items()})
    except Exception as e:
        return (type(e).__name__, str(e).splitlines()[-1][:120])


def main() -> int:
    warnings.simplefilter("ignore")
    aff = np.eye(4)
    series = np.zeros((3, 4, 2, 0), np.float32)
    images = {
        "full": nb.Nifti1Image(np.ones((3, 4, 2), np.uint8), aff),
        "empty": nb.Nifti1Image(np.zeros((3, 4, 2), np.uint8), aff),
        "other": nb.Nifti1Image(np.ones((2, 2, 2), np.uint8), aff),
    }
    d = tempfile.mkdtemp()
    paths = {}
    for name, img in images.items():
        paths[name] = os.path.join(d, name + ".nii.gz")
        img.to_filename(paths[name])
    orders = (
        ["full"], ["empty"], ["empty", "full"], ["full", "empty"], ["empty", "other"],
        ["empty", "empty"], ["empty", "empty", "full"],
    )  # fmt: skip
    n = bad = 0
    for ftype, mode, names, crit in itertools.product(
        ("polynomial", False, "cosine"), ("error", "NaN"), orders, (0.5, 2, "all", 1.5)
    ):
        for rt in (None, 2.0) if ftype == "cosine" else (None,):
            args = (crit, ftype, 1, 128, rt, mode)
            want = outcome(lambda a=args, m=names: ncc(series, [images[x] for x in m], *a))
            got = outcome(
                lambda a=args, m=names: lc.compute_noise_components(
                    series, [paths[x] for x in m], *a,
                    numpy_version=np.__version__, upstream_defects="reproduce",
                )
            )  # fmt: skip
            n += 1
            if want != got:
                bad += 1
                print(ftype, mode, names, crit, rt, "\n   ", want, "\n   ", got)
    print(f"{n - bad} of {n} identical")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
