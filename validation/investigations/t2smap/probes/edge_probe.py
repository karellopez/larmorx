# SPDX-License-Identifier: Apache-2.0
"""Probe (relay 1): a few failure cases of tedana 26.0.3's t2smap on tiny synthetic echoes.

Plain arithmetic inputs (seeded), 3 echoes, 8x8x4 voxels, 6 volumes. Runs the CLI entry point
in-process (tedana.workflows.t2smap._main) per case and prints the exit status and the last line
of the error, so the inventory can name each failure.
"""

import contextlib
import io
import os
import tempfile

import nibabel as nb
import numpy as np
from tedana.workflows import t2smap as t2

rng = np.random.default_rng(0)
shape = (8, 8, 4, 6)
tes = [14.0, 30.0, 46.0]
t2s = 30.0 + rng.integers(0, 20, size=shape[:3])
s0 = 1000.0 + rng.integers(0, 500, size=shape[:3])
aff = np.diag([3.0, 3.0, 3.0, 1.0])


def echoes(d, mutate=None):
    paths = []
    for i, te in enumerate(tes):
        # exp is used only to make realistic decays for this probe (not a golden input)
        arr = (s0 * np.exp(-te / t2s))[..., None] * (1 + 0.01 * rng.standard_normal(shape))
        arr = arr.astype(np.float32)
        if mutate:
            mutate(i, arr)
        p = os.path.join(d, f"echo{i + 1}.nii.gz")
        nb.Nifti1Image(arr, aff).to_filename(p)
        paths.append(p)
    m = os.path.join(d, "mask.nii.gz")
    nb.Nifti1Image(np.ones(shape[:3], np.uint8), aff).to_filename(m)
    return paths, m


def run(name, argv_extra, mutate=None, with_mask=True):
    with tempfile.TemporaryDirectory() as d:
        paths, m = echoes(d, mutate)
        argv = ["-d", *paths, "-e", *map(str, tes), "--out-dir", os.path.join(d, "out")]
        if with_mask:
            argv += ["--mask", m]
        argv += argv_extra
        err = io.StringIO()
        code = 0
        with contextlib.redirect_stderr(err):
            try:
                t2._main(argv)
            except SystemExit as e:
                code = e.code
            except Exception as e:
                code = f"exception {type(e).__name__}: {e}"
        last = [ln for ln in err.getvalue().splitlines() if ln.strip()][-1:] or [""]
        print(f"{name}: {code} | {last[0][:200]}")


def nan_first_echo(i, a):
    if i == 0:
        a[1, 1, 1, 2] = np.nan


def zeros_vox(i, a):
    a[2, 2, 2, :] = 0


run("baseline loglin", ["--fittype", "loglin"])
run("baseline curvefit", ["--fittype", "curvefit"])
run("nan in echo 1", ["--fittype", "loglin"], nan_first_echo)
run("zeros voxel", ["--fittype", "loglin"], zeros_vox)
run("exclude == n_vols", ["--exclude", "6"])
run("exclude > n_vols", ["--exclude", "7"])
run("exclude ts", ["--exclude", "1", "--fitmode", "ts"])
run("fitmode ts curvefit", ["--fitmode", "ts", "--fittype", "curvefit"])
run("paid", ["--combmode", "paid"])
run("masktype decay", ["--masktype", "decay"])
run("masktype none", ["--masktype", "none"])
run("masktype dropout decay", ["--masktype", "dropout", "decay"])
run("no mask", [], with_mask=False)
run("dummy 2 + exclude 0:3", ["--dummy-scans", "2", "--exclude", "0:3"])
run("n-independent-echos 2", ["--n-independent-echos", "2"])
run("tes in seconds", [])  # tes given in ms above; seconds case below
