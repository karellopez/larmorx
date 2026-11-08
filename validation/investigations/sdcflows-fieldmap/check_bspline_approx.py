# SPDX-License-Identifier: Apache-2.0
"""Bit-compare lx.sdcflows.bspline_approx, resample_to_zooms and rescale_affine with sdcflows.

Runs, in this interpreter's environment (run it in each kept one: ``oracles/python-newest`` and
the shared ``.venv``):

    OPENBLAS_CORETYPE=Haswell PYTHONPATH=<repo>/python LARMORX_WORKSPACE=<workspace> \\
        python -P check_bspline_approx.py [--quick]

1. ``rescale_affine`` against nibabel's on 300 random affines (oblique, flipped, permuted),
   shapes and zooms (float32 and float64): every affine bit-identical.
2. ``resample_to_zooms`` against sdcflows' ``sdcflows.utils.tools.resample_to_zooms``: the data
   (values and type), the affine and the 348-byte header, on field maps and masks of every
   stored type (float32, float64, uint8, int16 with and without scaling), oblique and flipped
   grids, sform/qform codes 0 and 1, a 4D image with one volume, interpolation orders 0, 1, 3
   and 5, no prefilter, zooms as a number, float32 or float64.
3. ``bspline_approx`` against sdcflows' ``BSplineApprox`` (nipype interface, in a temporary
   directory) with its files written (``out_dir``): every output file byte-identical once
   decompressed (header, extensions, data), the file names, ``out_extrapolated`` naming the
   field's file when the mask holds every voxel, the intercept, every log record (INFO and
   DEBUG), and the exception and records of failures; on every option (recentring by median,
   mode, mean, none or 0; no mask, a full mask, scaled masks; one and two grids; alpha; no
   extrapolation; resizes with ``zooms_min`` as a number and a tuple, with and without a mask;
   a single slice; odd and oblique grids; a NIfTI-2, an uncompressed and a float64 input; a
   name containing ``_field.``; a failed fit; NaN in the field; a 4D header; a mask on another
   grid) and on the 7 ``bs_filter`` calls of the fMRIPrep oracle runs, whose own files
   (fMRIPrep 25.2.6, ``*_coeff000``, ``*_extra``) are compared too.

One line per case; exit status 1 if any differs.
"""

import gzip
import logging
import os
import sys
import tempfile
import warnings
from pathlib import Path

import nibabel as nb
import numpy as np
from nibabel.affines import rescale_affine as nb_rescale
from sdcflows.interfaces.bspline import BSplineApprox
from sdcflows.utils.tools import resample_to_zooms as sdc_resample

import larmorx as lx
from larmorx import sdcflows as lxs

sys.path.insert(0, str(Path(__file__).parent))  # stepdigest (python -P)
from stepdigest import oracle, orthogonal

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE") or Path(__file__).resolve().parents[4])
R = WORKSPACE / "oracles" / "fmriprep-25.2.6"
QUICK = "--quick" in sys.argv
FAILS: list[str] = []


def report(name: str, ok: bool, detail: str = "") -> None:
    print(f"{'OK  ' if ok else 'FAIL'} {name}{': ' + detail if detail else ''}")
    if not ok:
        FAILS.append(name)


def raw(path) -> bytes:
    b = Path(path).read_bytes()
    return gzip.decompress(b) if b[:2] == b"\x1f\x8b" else b


class Capture(logging.Handler):
    def __init__(self) -> None:
        super().__init__(logging.DEBUG)
        self.records: list[tuple[str, str]] = []

    def emit(self, record: logging.LogRecord) -> None:
        if record.levelno in (logging.INFO, logging.DEBUG):
            self.records.append((record.levelname.lower(), record.getMessage()))


def captured(logger_name, fn):
    log = logging.getLogger(logger_name)
    cap = Capture()
    old = log.level
    log.setLevel(logging.DEBUG)
    log.addHandler(cap)
    try:
        try:
            return fn(), None, cap.records
        except Exception as e:
            return None, e, cap.records
    finally:
        log.removeHandler(cap)
        log.setLevel(old)


# ------------------------------------------------------------------------------------------------
# 1. rescale_affine


def random_affine(rng, oblique=True):
    zooms = rng.uniform(0.5, 4.0, 3)
    if oblique:
        q = orthogonal(rng)  # never numpy.linalg (its bits follow LAPACK's release)
    else:
        q = np.eye(3)[rng.permutation(3)] * rng.choice([-1, 1], 3)[:, None]
    a = np.eye(4)
    a[:3, :3] = q * zooms
    if oblique and rng.random() < 0.3:
        a[:3, :3] += rng.normal(0, 0.05, (3, 3))  # shears
    a[:3, 3] = rng.uniform(-120, 120, 3)
    return a


def check_rescale() -> None:
    rng = np.random.default_rng(20261108)
    bad = 0
    for i in range(300):
        a = random_affine(rng, oblique=i % 2 == 0)
        shape = tuple(int(n) for n in rng.integers(1, 200, 3))
        zooms = rng.uniform(0.3, 6.0, 3)
        if i % 3 == 0:
            zooms = zooms.astype(np.float32)
        new_shape = np.rint(np.array(shape) * rng.uniform(0.2, 3, 3))
        want = oracle(nb_rescale(a, shape, zooms, new_shape))
        got = lxs.rescale_affine(a, shape, zooms, new_shape)
        if want.tobytes() != got.tobytes():
            bad += 1
    report("rescale_affine: 300 random affines", bad == 0, f"{bad} differ" if bad else "")


# ------------------------------------------------------------------------------------------------
# 2. resample_to_zooms


def nb_image(data, affine, *, scode=1, qcode=1, slope=None, inter=0.0, version=1):
    klass = nb.Nifti2Image if version == 2 else nb.Nifti1Image
    img = klass(data, affine)
    img.header.set_sform(affine, scode)
    img.header.set_qform(affine, qcode)
    img.header.set_xyzt_units("mm", "sec")
    if slope is not None:
        img.header.set_slope_inter(slope, inter)
    return img


def field(shape, seed, scale=1.0):
    rng = np.random.default_rng(seed)
    ii, jj, kk = np.meshgrid(*[np.linspace(-1, 1, n) for n in shape], indexing="ij")
    return (80 * ii * jj - 40 * kk**2 + 25 * ii + 12 + rng.normal(0, 2, shape)) * scale


def check_resample(tmp: Path) -> None:
    rng = np.random.default_rng(7)
    lps = np.array([[-1.6, 0, 0, 40], [0, 1.6, 0, -50], [0, 0, 2.2, -20], [0, 0, 0, 1.0]])
    obl = random_affine(rng)
    obl[:3, :3] = obl[:3, :3] / nb.affines.voxel_sizes(obl) * np.array([1.4, 1.7, 0.9])
    f = field((30, 26, 14), 1)
    m = ((np.indices((30, 26, 14)) - 12.0) ** 2).sum(0) < 90
    cases = [
        ("float32 LAS", nb_image(f.astype(np.float32), lps), 3.0, {}),
        ("float32 oblique", nb_image(f.astype(np.float32), obl), np.float32(2.0), {}),
        ("float64", nb_image(f, lps), (2.0, 2.5, 3.0), {}),
        ("uint8 mask", nb_image(m.astype(np.uint8), lps), np.float32(4.0), {}),
        ("int16 scaled", nb_image((f * 20).astype(np.int16), lps, slope=0.05, inter=-1.0), 3.0, {}),
        ("int16 unscaled", nb_image((f * 20).astype(np.int16), obl), 2.5, {}),
        ("sform 0, qform 1", nb_image(f.astype(np.float32), obl, scode=0), 2.0, {}),
        ("sform 0, qform 0", nb_image(f.astype(np.float32), lps, scode=0, qcode=0), 2.0, {}),
        ("NIfTI-2", nb_image(f.astype(np.float32), lps, version=2), 2.0, {}),
        ("4D, one volume", nb_image(f.astype(np.float32)[..., None], lps), (2.0, 2.0, 3.0), {}),
        ("order 1", nb_image(f.astype(np.float32), obl), 2.0, {"order": 1}),
        ("order 0", nb_image(f.astype(np.float32), lps), 2.0, {"order": 0}),
        ("order 5", nb_image(f.astype(np.float32), lps), 2.0, {"order": 5}),
        ("no prefilter", nb_image(f.astype(np.float32), obl), 2.0, {"prefilter": False}),
        ("single slice", nb_image(f[:, :, :1].astype(np.float32), lps), 1.0, {}),
    ]
    for name, img, zooms, kw in cases:
        p = tmp / f"rz_{len(name)}_{abs(hash(name)) % 10**6}.nii.gz"
        img.to_filename(p)
        loaded = nb.load(p)
        want = oracle(sdc_resample(loaded, zooms, **kw))
        got = lxs.resample_to_zooms(p, zooms, n_threads=4, **kw)
        wdata = np.asanyarray(want.dataobj)
        mine_hdr = nb.Nifti2Header if got.header.version == 2 else nb.Nifti1Header
        hl = mine_hdr(got.header.to_bytes())
        checks = {
            "data": wdata.dtype == got.data.dtype
            and wdata.shape == got.data.shape
            and np.asarray(wdata, order="F").tobytes() == np.asarray(got.data, order="F").tobytes(),
            "affine": want.affine.tobytes() == np.asarray(got.affine).tobytes(),
            # every field of the header in memory but those nibabel sets when writing
            "header": all(
                want.header[k].tobytes() == hl[k].tobytes()
                for k in want.header
                if k not in ("vox_offset", "scl_slope", "scl_inter")
            ),
        }
        if wdata.dtype == want.header.get_data_dtype():
            # the files nibabel and larmorx write: identical once decompressed
            a, b = p.with_name("w_" + p.name), p.with_name("g_" + p.name)
            want.to_filename(a)
            got.save(b)
            checks["file"] = raw(a) == raw(b)
        bad = [k for k, v in checks.items() if not v]
        report(f"resample_to_zooms {name}", not bad, f"differ: {bad}" if bad else "")


# ------------------------------------------------------------------------------------------------
# 3. bspline_approx


def run_sdcflows(tmp, in_data, in_mask, params):
    def go():
        node = BSplineApprox(in_data=str(in_data), **params)
        if in_mask is not None:
            node.inputs.in_mask = str(in_mask)
        return node.run(cwd=tmp).outputs

    return oracle(*captured("nipype.interface", go))


def run_larmorx(out_dir, in_data, in_mask, params, mine_kw=None):
    return captured(
        "larmorx.sdcflows",
        lambda: lxs.bspline_approx(
            str(in_data),
            None if in_mask is None else str(in_mask),
            **params,
            out_dir=out_dir,
            n_threads=14,
            **(mine_kw or {}),
        ),
    )


def exc_text(e):
    if e is None:
        return None
    # nipype wraps nothing here (SimpleInterface re-raises); compare class and first line
    return f"{type(e).__name__}: {str(e).splitlines()[0] if str(e) else ''}"


def case(name, in_data, in_mask=None, oracle_dir=None, mine_kw=None, **kw):
    params = {
        "bs_spacing": [(16.0, 16.0, 10.0)],
        "ridge_alpha": 1e-4,
        "recenter": "median",
        "extrapolate": True,
        "zooms_min": 1.0,
    }
    params.update(kw)
    with tempfile.TemporaryDirectory() as t1, tempfile.TemporaryDirectory() as t2:
        outs, err_ref, log_ref = run_sdcflows(t1, in_data, in_mask, params)
        mine, err, log = run_larmorx(t2, in_data, in_mask, params, mine_kw)
        if err_ref is not None or err is not None:
            ok = exc_text(err_ref) == exc_text(err) and log_ref == log
            detail = f"{exc_text(err_ref)!r} vs {exc_text(err)!r}"
            if log_ref != log:
                detail += f"; logs {log_ref} vs {log}"
            report(name, ok, detail)
            return
        checks = {}
        ref_coeff = outs.out_coeff if isinstance(outs.out_coeff, list) else [outs.out_coeff]
        pairs = [
            ("field", outs.out_field, mine.out_field),
            ("error", outs.out_error, mine.out_error),
        ]
        pairs += [
            (f"coeff{i}", a, b)
            for i, (a, b) in enumerate(zip(ref_coeff, mine.out_coeff, strict=False))
        ]
        checks["n_coeff"] = len(ref_coeff) == len(mine.out_coeff)
        extra_ref = outs.out_extrapolated if params["extrapolate"] else None
        if params["extrapolate"]:
            pairs.append(("extra", extra_ref, mine.out_extrapolated))
            checks["extra-is-field"] = (extra_ref == outs.out_field) == (
                mine.out_extrapolated == mine.out_field
            )
        else:
            checks["no-extrapolation"] = mine.out_extrapolated is None and mine.extrapolated is None
        for label, a, b in pairs:
            checks[f"{label}-name"] = b is not None and Path(a).name == Path(b).name
            checks[f"{label}-bytes"] = b is not None and raw(a) == raw(b)
        # the in-memory results are the files' contents
        checks["in-memory"] = all(
            np.array_equal(np.asanyarray(nb.load(p).dataobj), img.data, equal_nan=True)
            for p, img in [
                (mine.out_field, mine.field),
                *(
                    [(mine.out_error, mine.error)]
                    if np.dtype(mine.error.header.data_dtype).kind == "f"
                    else []  # an integer input type: the file holds nibabel's scaled values
                ),
                *list(zip(mine.out_coeff, mine.coefficients, strict=False)),
            ]
        )
        checks["intercept"] = outs.out_intercept == mine.intercept == 0.0
        checks["log"] = log_ref == log
        if oracle_dir is not None:
            for f in sorted(Path(oracle_dir).glob("*_coeff*.nii.gz")) + sorted(
                Path(oracle_dir).glob("*_extra.nii.gz")
            ):
                ours = Path(mine.out_coeff[0]).parent / f.name
                checks[f"fmriprep-{f.name.rsplit('_', 1)[-1]}"] = ours.exists() and raw(f) == raw(
                    ours
                )
        bad = [k for k, v in checks.items() if not v]
        report(
            name,
            not bad,
            f"{mine.n_inside} inside, {mine.n_iter} iterations"
            + (f"; differ: {bad}" if bad else ""),
        )
        if not checks["log"]:
            print("   sdcflows:", log_ref)
            print("   larmorx: ", log)


def write(path, data, affine, **kw):
    img = nb_image(data, affine, **kw)
    img.to_filename(path)
    return path


def synthetic(
    tmp,
    name,
    shape,
    zooms,
    seed,
    *,
    scale=1.0,
    quantise=None,
    flips=(1, 1, 1),
    affine=None,
    dtype=np.float32,
    version=1,
):
    if affine is None:
        affine = np.diag([zooms[0] * flips[0], zooms[1] * flips[1], zooms[2] * flips[2], 1.0])
        affine[:3, 3] = -affine[:3, :3] @ (0.5 * (np.array(shape) - 1))
    f = field(shape, seed, scale)
    if quantise:
        f = np.round(f / quantise) * quantise
    ii, jj, kk = np.meshgrid(*[np.linspace(-1, 1, n) for n in shape], indexing="ij")
    mask = ((ii**2 + jj**2 + kk**2) < 0.8).astype(np.uint8)
    if not mask.any():
        mask[...] = 1
    ext = ".nii.gz" if version == 1 else ".nii"
    fp = write(Path(tmp) / f"{name}_fmap{ext}", f.astype(dtype), affine, version=version)
    mp = write(Path(tmp) / f"{name}_mask.nii.gz", mask, affine)
    return fp, mp, affine


BS = "work/fmriprep_25_2_wf"
ORACLE = [
    ("ds005454-sub-16", "sub_16_wf/fmap_preproc_wf/wf_gre16"),
    ("ds002422-sub-02", "sub_02_wf/fmap_preproc_wf/wf_auto_00000"),
    ("ds002422-sub-02", "sub_02_wf/fmap_preproc_wf/wf_auto_00001"),
    ("ds001600-sub-1", "sub_1_wf/fmap_preproc_wf/wf_auto_00000"),
    ("ds001600-sub-1", "sub_1_wf/fmap_preproc_wf/wf_auto_00001"),
    ("ds001600-sub-1", "sub_1_wf/fmap_preproc_wf/wf_auto_00002"),
    ("ds002603-sub-01", "sub_01_wf/fmap_preproc_wf/wf_auto_00000"),
]


def check_approx(tmp: Path) -> None:
    f, m, aff = synthetic(tmp, "base", (40, 36, 20), (3.0, 3.0, 3.5), 1)
    case("approx median", f, m)
    case("approx mean", f, m, recenter="mean")
    case("approx none", f, m, recenter=False)
    case("approx recenter 0", f, m, recenter=0)
    case("approx no mask", f, None)
    case("approx no extrapolation", f, m, extrapolate=False)
    case("approx alpha 1e-2", f, m, ridge_alpha=1e-2)
    case("approx two grids", f, m, bs_spacing=[(40.0, 40.0, 20.0), (16.0, 16.0, 10.0)])
    q, qm, _ = synthetic(tmp, "quant", (30, 28, 12), (3.0, 3.0, 3.0), 2, quantise=2.5)
    case("approx mode", q, qm, recenter="mode")
    full = write(tmp / "full_mask.nii.gz", np.ones((40, 36, 20), np.uint8), aff)
    case("approx full mask", f, full)
    sm = (np.asanyarray(nb.load(m).dataobj) * 200).astype(np.int16)
    scaled = write(tmp / "scaled_mask.nii.gz", sm, aff, slope=1e-6)
    case("approx scaled int16 mask", f, scaled)
    fm = write(tmp / "float_mask.nii.gz", np.asanyarray(nb.load(m).dataobj) * np.float32(0.6), aff)
    case("approx float32 mask", f, fm)
    o, om, _ = synthetic(tmp, "odd", (23, 31, 9), (2.7, 3.1, 4.4), 3, flips=(-1, 1, 1))
    case("approx odd LAS", o, om)
    rng = np.random.default_rng(3)
    obl = random_affine(rng)
    ob, obm, _ = synthetic(tmp, "obl", (26, 24, 14), None, 4, affine=obl)
    case("approx oblique", ob, obm)
    k, km, _ = synthetic(tmp, "kernel", (3, 3, 2), (2.0, 2.0, 2.0), 4)
    case("approx kernel system", k, km)
    s, sm_, _ = synthetic(tmp, "slice", (32, 30, 1), (3.0, 3.0, 4.0), 5)
    case("approx single slice", s, sm_)
    z, zm, _ = synthetic(tmp, "fine", (36, 30, 20), (1.5, 1.5, 2.0), 6)
    case("approx resize zooms_min 4", z, zm, zooms_min=4.0)
    case("approx resize zooms_min tuple, no mask", z, None, zooms_min=(2.0, 2.0, 3.0))
    zs, zsm, _ = synthetic(tmp, "fineslice", (40, 36, 1), (0.8, 0.8, 3.0), 9)
    case("approx resize single slice", zs, zsm, zooms_min=2.0)
    zc, zcm, _ = synthetic(tmp, "collapse", (40, 36, 1), (0.9, 0.9, 1.0), 12)
    case("approx resize collapses an axis", zc, zcm, zooms_min=4.0)
    sm2 = write(
        tmp / "fine_scaled_mask.nii.gz",
        np.asanyarray(nb.load(zm).dataobj).astype(np.int16) * 300,
        nb.load(zm).affine,
        slope=1e-6,
    )
    case("approx resize, scaled int16 mask", z, sm2, zooms_min=3.0)
    n2, n2m, _ = synthetic(tmp, "nii2", (20, 22, 12), (3.0, 3.0, 3.0), 10, version=2)
    case("approx NIfTI-2 uncompressed", n2, n2m)
    d64, d64m, _ = synthetic(tmp, "f64", (20, 22, 12), (3.0, 3.0, 3.0), 11, dtype=np.float64)
    case("approx float64 input", d64, d64m)
    i16, i16m, i16a = synthetic(tmp, "i16", (20, 22, 12), (3.0, 3.0, 3.0), 13, dtype=np.int16)
    case("approx int16 input (error map scaled into int16)", i16, i16m)
    si16 = write(
        tmp / "si16_fmap.nii.gz",
        np.round(field((20, 22, 12), 14) * 50).astype(np.int16),
        i16a,
        slope=0.02,
        inter=0.5,
    )
    case("approx scaled int16 input", si16, i16m)
    u8, u8m, _ = synthetic(tmp, "u8", (18, 16, 10), (3.0, 3.0, 3.0), 15, dtype=np.uint8)
    case("approx uint8 input (error map scaled into uint8)", u8, u8m)
    nm = tmp / "sub-01_field.v2_fmap.nii.gz"
    nm.write_bytes(Path(f).read_bytes())
    case("approx name with _field.", nm, m)
    big, bm, _ = synthetic(tmp, "huge", (20, 18, 10), (3.0, 3.0, 3.0), 7, scale=1e5)
    case("approx failed fit", big, bm)
    nanf = field((20, 18, 10), 8).astype(np.float32)
    nanf[10, 9, 5] = np.nan
    nanp = write(tmp / "nan_fmap.nii.gz", nanf, aff)
    case("approx NaN in the field", nanp, bm)
    four = write(
        tmp / "four_fmap.nii.gz", field((20, 18, 10), 8).astype(np.float32)[..., None], aff
    )
    case(
        "approx 4D header (SDC-06 reproduced)",
        four,
        None,
        mine_kw={"upstream_defects": "reproduce"},
    )
    # SDC-06 corrected: the 4D map of one volume gives sdcflows' result of the 3D map
    three = write(tmp / "three_fmap.nii.gz", field((20, 18, 10), 8).astype(np.float32), aff)
    with tempfile.TemporaryDirectory() as t1, tempfile.TemporaryDirectory() as t2:
        outs, err_ref, _ = run_sdcflows(t1, three, None, {"bs_spacing": [(16.0, 16.0, 10.0)]})
        with warnings.catch_warnings(record=True) as w:
            warnings.simplefilter("always")
            mine, err, _ = run_larmorx(t2, four, None, {"bs_spacing": [(16.0, 16.0, 10.0)]})
        named = any(getattr(x.message, "entry", "") == "SDC-06" for x in w)
        same = err_ref is None and err is None
        if same:
            for a, b in [
                (outs.out_field, mine.out_field),
                (outs.out_error, mine.out_error),
                (outs.out_coeff, mine.out_coeff[0]),
            ]:
                da = np.asanyarray(nb.load(a).dataobj)
                db = np.asanyarray(nb.load(b).dataobj)
                same &= da.tobytes() == np.squeeze(db, axis=-1 if db.ndim == 4 else None).tobytes()
        report("approx 4D header (SDC-06 corrected: the 3D map's result)", same and named)
    other = write(tmp / "other_mask.nii.gz", np.ones((20, 18, 9), np.uint8), aff)
    case("approx mask on another grid", big, other)


def check_oracle() -> None:
    for run, node in ORACLE:
        if QUICK and run == "ds002603-sub-01":
            continue
        base = R / f"{run}-noreconall" / BS / node
        datas = sorted((base / "phdiff_wf" / "compfmap").glob("*_fmap.nii.gz"))
        maskf = base / "magnitude_wf" / "brainextraction_wf" / "masker" / "clipped_mask.nii.gz"
        if not datas or not maskf.exists():
            print(f"skipped (no oracle run here): {run} {node}")
            continue
        case(f"{run}/{node.rsplit('/', 1)[-1]}", datas[0], maskf, oracle_dir=base / "bs_filter")


def main() -> int:
    warnings.simplefilter("ignore")
    print(f"numpy {np.__version__}, larmorx {lx.__file__}")
    check_rescale()
    with tempfile.TemporaryDirectory() as tmp:
        check_resample(Path(tmp))
        check_approx(Path(tmp))
    check_oracle()
    print("ALL OK" if not FAILS else f"FAILED: {len(FAILS)}: {', '.join(FAILS)}")
    return 0 if not FAILS else 1


if __name__ == "__main__":
    sys.exit(main())
