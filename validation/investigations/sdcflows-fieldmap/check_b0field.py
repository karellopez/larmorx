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
   ds005040's TOPUP coefficients placed by ``TOPUPCoeffReorient``) on their BOLD references,
   with ``upstream_defects="reproduce"``.
3. The corrected fit (``upstream_defects="fix"``: SDC-01, SDC-02, SDC-10) against sdcflows'
   own fit with the three corrections made in its pieces (the weights of
   ``measure_sdc01_02.py``: the B-spline beyond the grid, the exact weights of a rotated grid;
   the approximation's grid built and sampled at the projected target): the field's values,
   on oblique targets with and without a transform, synthetic and ds002603's oblique BOLD
   references with fMRIPrep's own registration to the field map.

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

sys.path.insert(0, str(Path(__file__).parent))

from stepdigest import oracle

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
            want = oracle(
                sdc_deoblique(nb.load(cpath), nb.load(tpath), factor=factor, padding=padding)
            )
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
    oracle(ref.mapped, [str(x.message) for x in w1])
    with warnings.catch_warnings(record=True) as w2:
        warnings.simplefilter("always")
        mine = B0FieldTransform(coeffs=[str(c) for c in coeffs])
        mine.fit(
            str(target),
            xfm_data2fmap=xfm,
            approx=approx,
            n_threads=8,
            upstream_defects="reproduce",
        )
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
            # sdcflows' own warnings (larmorx's notices of the defects reproduced aside)
            "warnings": [str(x.message) for x in w1 if "aligned" in str(x.message)]
            == [
                str(x.message)
                for x in w2
                if "aligned" in str(x.message) and not hasattr(x.message, "entry")
            ],
        }
    bad = [k for k, v in checks.items() if not v]
    report(f"fit {name}", not bad, f"{wd.shape}" + (f"; differ: {bad}" if bad else ""))


def _aligned(a, b) -> bool:
    return np.allclose(np.linalg.norm(np.cross(a[:-1, :-1].T, b[:-1, :-1].T), axis=1), 0, atol=1e-3)


def fixed_weights(target, ctrl, dtype="float32"):
    """The corrected weights of one level (SDC-01 and SDC-02), built with numpy and SciPy."""
    from measure_sdc01_02 import fixed_weights as fw

    return fw(target, ctrl, dtype)


def fixed_case(name, coeffs, target, xfm=None, approx=True, notices=()):
    """larmorx's corrected fit against sdcflows' fit with the corrections made in its pieces."""
    import sdcflows.transform as sdct

    t = nb.load(target)
    original = sdct.grid_bspline_weights
    sdct.grid_bspline_weights = fixed_weights
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            ref = SdcB0(coeffs=[nb.load(c) for c in coeffs])
            moved = xfm is not None and not np.array_equal(xfm, np.eye(4))
            if approx and moved and not _aligned(nb.load(coeffs[-1]).affine, t.affine):
                # SDC-10 corrected: the target where the transform puts it, grid and samples
                proj = nb.Nifti1Image(np.zeros(t.shape[:3], np.float32), xfm @ t.affine)
                ref.fit(proj, xfm_data2fmap=np.eye(4), approx=True)
            else:
                ref.fit(t, xfm_data2fmap=xfm, approx=approx)
    finally:
        sdct.grid_bspline_weights = original
    oracle(ref.mapped)
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        mine = B0FieldTransform(coeffs=[str(c) for c in coeffs])
        mine.fit(str(target), xfm_data2fmap=xfm, approx=approx, n_threads=8)
    named = sorted({getattr(x.message, "entry", "") for x in w} - {""})
    wd = np.asarray(np.asanyarray(ref.mapped.dataobj), dtype=np.float32, order="C")
    md = np.asarray(mine.mapped.data, dtype=np.float32, order="C")
    ok = wd.shape == md.shape and wd.tobytes() == md.tobytes() and named == sorted(notices)
    detail = f"{wd.shape}; notices {named}"
    if not ok and wd.shape == md.shape:
        detail += f"; max |diff| {np.abs(wd.astype(np.float64) - md).max():.3g}"
    report(f"fit corrected: {name}", ok, detail)


def check_fixed(tmp: Path) -> None:
    rng = np.random.default_rng(11)
    ras = np.diag([3.0, 3.0, 3.5, 1.0])
    ras[:3, 3] = (-60, -66, -40)
    shape = (40, 44, 24)
    g = lxs.bspline_grid((np.zeros(shape, np.float32), ras), (16.0, 16.0, 10.0))
    coeffs = [
        image(tmp / "f_coeff000.nii.gz", g.shape, np.asarray(g.affine), rng.normal(0, 40, g.shape))
    ]
    obl = matmul(rigid((0.0, 0.0, 0.12), (0.0, 0.0, 0.0)), ras)
    oblique = image(tmp / "oblique.nii.gz", shape, obl)
    larger = np.diag([3.0, 3.0, 3.5, 1.0])
    larger[:3, 3] = (-75, -80, -50)
    big = image(tmp / "big.nii.gz", (50, 54, 30), larger)
    x = rigid((0.05, -0.03, 0.08), (3.0, -2.0, 1.5))
    fixed_case("aligned, inside", coeffs, image(tmp / "own_f.nii.gz", shape, ras))
    fixed_case("aligned, larger than the grid (SDC-01)", coeffs, big, notices=("SDC-01",))
    fixed_case("oblique, no transform (SDC-02)", coeffs, oblique, notices=("SDC-02",))
    fixed_case(
        "aligned, a rotating transform (SDC-02 on the projected grid)",
        coeffs,
        image(tmp / "own_g.nii.gz", shape, ras),
        x,
        True,
        notices=("SDC-02",),
    )
    fixed_case(
        "oblique, approx=False with a transform (SDC-02)",
        coeffs,
        oblique,
        x,
        False,
        notices=("SDC-01", "SDC-02"),
    )
    fixed_case(
        "oblique, a transform, approx (SDC-10)",
        coeffs,
        oblique,
        x,
        True,
        notices=("SDC-01", "SDC-10"),
    )
    # ds002603: fMRIPrep's coefficients, its oblique BOLD references and its registration
    import nitransforms as nt

    base = R / "ds002603-sub-01-noreconall" / "out" / "sub-01"
    cs = sorted((base / "fmap").glob("*desc-coeff_fieldmap.nii.gz"))
    for bref in sorted((base / "func").glob("*desc-hmc_boldref.nii.gz"))[:2]:
        prefix = bref.name.split("_desc-")[0]
        xp = sorted((base / "func").glob(f"{prefix}_from-boldref_to-*_desc-fmap_xfm.txt"))
        if not cs or not xp:
            print("skipped (no oracle run here): ds002603")
            return
        m = np.linalg.inv(nt.linear.Affine.from_filename(str(xp[0]), fmt="itk").matrix)
        fit_case(
            f"ds002603 {prefix} oblique, its registration (SDC-10 reproduced)", cs, bref, m, True
        )
        fixed_case(
            f"ds002603 {prefix} oblique, its registration (SDC-10 corrected)",
            cs,
            bref,
            m,
            True,
            ("SDC-01", "SDC-10"),
        )


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
        check_fixed(Path(tmp))
    print("ALL OK" if not FAILS else f"FAILED: {len(FAILS)}: {', '.join(FAILS)}")
    return 0 if not FAILS else 1


if __name__ == "__main__":
    sys.exit(main())
