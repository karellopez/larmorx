# SPDX-License-Identifier: Apache-2.0
"""Bit-compare lx.sdcflows.B0FieldTransform.fit and deoblique_and_zooms with sdcflows'.

Runs, in this interpreter's environment (each kept one: ``oracles/python-newest`` and the
shared ``.venv``):

    OPENBLAS_CORETYPE=Haswell PYTHONPATH=<repo>/python LARMORX_WORKSPACE=<workspace> \\
        python -P check_b0field.py

1. ``deoblique_and_zooms`` against sdcflows' (``sdcflows.utils.tools``): shape, affine, data
   type and every header field nibabel keeps in memory, for control grids and rotated, shifted,
   finer and coarser targets, ``factor`` 1, 2 and 4, ``padding`` 0 and 3.
2. ``B0FieldTransform(coeffs).fit(target, xfm_data2fmap, approx)`` against sdcflows': the
   field (``mapped``: values, affine) and the file it writes (``mapped.to_filename``, as
   ``ApplyCoeffsField`` writes ``out_field``) byte-identical once decompressed, and the
   warnings; on the target's own grid, a LAS and a 4D target, one and two control grids, a
   rigid transform with and without ``approx``, an identity transform; with synthetic
   coefficients and with the fMRIPrep oracle runs' coefficients (ds005454's B-spline fit,
   ds005040's TOPUP coefficients placed by ``TOPUPCoeffReorient``) on their BOLD references.

One line per case; exit status 1 if any differs.
"""

import gzip
import os
import sys
import tempfile
import warnings
from pathlib import Path

import nibabel as nb
import numpy as np
from sdcflows.transform import B0FieldTransform as SdcB0
from sdcflows.utils.tools import deoblique_and_zooms as sdc_deoblique

from larmorx import sdcflows as lxs
from larmorx.sdcflows.transform import B0FieldTransform, deoblique_and_zooms

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE") or Path(__file__).resolve().parents[4])
R = WORKSPACE / "oracles" / "fmriprep-25.2.6"
FAILS: list[str] = []


def report(name: str, ok: bool, detail: str = "") -> None:
    print(f"{'OK  ' if ok else 'FAIL'} {name}{': ' + detail if detail else ''}")
    if not ok:
        FAILS.append(name)


def raw(path) -> bytes:
    b = Path(path).read_bytes()
    return gzip.decompress(b) if b[:2] == b"\x1f\x8b" else b


def header_same(nb_hdr, lx_hdr) -> bool:
    klass = nb.Nifti2Header if lx_hdr.version == 2 else nb.Nifti1Header
    h = klass(lx_hdr.to_bytes())
    return all(
        nb_hdr[k].tobytes() == h[k].tobytes()
        for k in nb_hdr.keys()  # noqa: SIM118 - a nibabel header
        if k not in ("vox_offset", "scl_slope", "scl_inter")
    )


def rigid(angles, shift):
    m = np.eye(4)
    m[:3, :3] = nb.eulerangles.euler2mat(*angles)
    m[:3, 3] = shift
    return m


def image(path, shape, affine, data=None, dtype=np.float32):
    if data is None:
        data = np.random.default_rng(len(str(path))).normal(100, 5, shape)
    img = nb.Nifti1Image(np.asarray(data).astype(dtype), affine)
    img.header.set_qform(affine, 1)
    img.header.set_sform(affine, 1)
    img.header.set_xyzt_units("mm", "sec")
    img.to_filename(path)
    return path


def check_deoblique(tmp: Path) -> None:
    rng = np.random.default_rng(3)
    ras = np.diag([2.5, 2.5, 3.0, 1.0])
    ras[:3, 3] = (-60, -70, -40)
    ctrl = lxs.bspline_grid((np.zeros((48, 56, 40), np.float32), ras), (16.0, 16.0, 10.0))
    cpath = image(tmp / "ctrl.nii.gz", ctrl.shape, np.asarray(ctrl.affine), np.zeros(ctrl.shape))
    for i, (name, shape, aff) in enumerate(
        [
            ("rotated", (64, 64, 36), matmul(rigid(rng.normal(0, 0.1, 3), (2, -3, 1)), ras)),
            ("shifted beyond", (50, 60, 44), matmul(rigid((0, 0, 0), (40, -30, 25)), ras)),
            (
                "finer LAS",
                (90, 100, 70),
                np.diag([-1.2, 1.2, 1.5, 1.0]) + np.c_[np.zeros((4, 3)), [54, -60, -50, 0]],
            ),
        ]
    ):
        tpath = image(tmp / f"target{i}.nii.gz", shape, aff)
        for factor, padding in ((4, 1), (1, 1), (2, 3), (4, 0)):
            want = sdc_deoblique(nb.load(cpath), nb.load(tpath), factor=factor, padding=padding)
            got = deoblique_and_zooms(str(cpath), str(tpath), factor=factor, padding=padding)
            checks = {
                "shape": tuple(want.shape) == tuple(got.shape),
                "dtype": np.asanyarray(want.dataobj).dtype == got.data.dtype,
                "affine": want.affine.tobytes() == np.asarray(got.affine).tobytes(),
                "header": header_same(want.header, got.header),
            }
            bad = [k for k, v in checks.items() if not v]
            report(
                f"deoblique_and_zooms {name} factor {factor} padding {padding}",
                not bad,
                f"differ: {bad}" if bad else "",
            )


def matmul(a, b):
    from larmorx.transforms._linalg import matmul as mm

    return mm(a, b)


def fit_case(name, coeffs, target, xfm=None, approx=True):
    with warnings.catch_warnings(record=True) as w1:
        warnings.simplefilter("always")
        ref = SdcB0(coeffs=[nb.load(c) for c in coeffs])
        ref.fit(nb.load(target), xfm_data2fmap=xfm, approx=approx)
    with warnings.catch_warnings(record=True) as w2:
        warnings.simplefilter("always")
        mine = B0FieldTransform(coeffs=[str(c) for c in coeffs])
        mine.fit(str(target), xfm_data2fmap=xfm, approx=approx, n_threads=8)
    with tempfile.TemporaryDirectory() as t:
        a, b = Path(t) / "a.nii.gz", Path(t) / "b.nii.gz"
        ref.mapped.to_filename(a)
        mine.mapped.save(b)
        wd = np.asanyarray(ref.mapped.dataobj)
        checks = {
            "data": wd.dtype == mine.mapped.data.dtype
            and np.asarray(wd, order="C").tobytes()
            == np.asarray(mine.mapped.data, order="C").tobytes(),
            "affine": ref.mapped.affine.tobytes() == np.asarray(mine.mapped.affine).tobytes(),
            "file": raw(a) == raw(b),
            "warnings": [str(x.message) for x in w1 if "aligned" in str(x.message)]
            == [str(x.message) for x in w2 if "aligned" in str(x.message)],
        }
    bad = [k for k, v in checks.items() if not v]
    report(f"fit {name}", not bad, f"{wd.shape}" + (f"; differ: {bad}" if bad else ""))


def check_fit(tmp: Path) -> None:
    rng = np.random.default_rng(9)
    ras = np.diag([3.0, 3.0, 3.5, 1.0])
    ras[:3, 3] = (-60, -66, -40)
    shape = (40, 44, 24)
    coeffs = []
    for i, sp in enumerate([(40.0, 40.0, 20.0), (16.0, 16.0, 10.0)]):
        g = lxs.bspline_grid((np.zeros(shape, np.float32), ras), sp)
        coeffs.append(
            image(
                tmp / f"c{i}_coeff000.nii.gz",
                g.shape,
                np.asarray(g.affine),
                rng.normal(0, 40, g.shape),
            )
        )
    own = image(tmp / "own.nii.gz", shape, ras)
    las = np.diag([-2.0, 2.0, 2.5, 1.0])
    las[:3, 3] = (64, -70, -42)
    bold = image(tmp / "bold.nii.gz", (60, 66, 34, 3), las)
    fit_case("own grid, one level", coeffs[1:], own)
    fit_case("own grid, two levels", coeffs, own)
    fit_case("LAS 4D target", coeffs[1:], bold)
    x = rigid((0.05, -0.03, 0.08), (3.0, -2.0, 1.5))
    fit_case("rigid xfm, approx", coeffs[1:], bold, x, True)
    fit_case("rigid xfm, no approx", coeffs[1:], bold, x, False)
    fit_case("rigid xfm, approx, two levels", coeffs, own, x, True)
    fit_case("identity xfm", coeffs[1:], bold, np.eye(4), True)
    fit_case("translation xfm", coeffs[1:], own, rigid((0, 0, 0), (5.0, 0, -2)), True)
    real = [
        (
            "ds005454-sub-16",
            "sub_16_wf/fmap_preproc_wf/wf_gre16/bs_filter/*_coeff000.nii.gz",
            "sub-16/func/*task-*_boldref.nii.gz",
        ),
        (
            "ds005040-sub-002",
            "sub_002_wf/fmap_preproc_wf/wf_auto_00000/fix_coeff/*_fixed.nii.gz",
            "sub-002/func/*_boldref.nii.gz",
        ),
    ]
    for run, cglob, bglob in real:
        base = R / f"{run}-noreconall"
        cs = sorted((base / "work" / "fmriprep_25_2_wf").glob(cglob))
        bs = sorted((base / "out").glob(bglob))
        if not cs or not bs:
            print(f"skipped (no oracle run here): {run}")
            continue
        fit_case(f"{run} on its BOLD reference", cs[:1], bs[0])
        fit_case(f"{run}, rigid xfm, approx", cs[:1], bs[0], x, True)


def main() -> int:
    warnings.simplefilter("ignore")
    print(f"numpy {np.__version__}")
    with tempfile.TemporaryDirectory() as tmp:
        check_deoblique(Path(tmp))
        check_fit(Path(tmp))
    print("ALL OK" if not FAILS else f"FAILED: {len(FAILS)}: {', '.join(FAILS)}")
    return 0 if not FAILS else 1


if __name__ == "__main__":
    sys.exit(main())
