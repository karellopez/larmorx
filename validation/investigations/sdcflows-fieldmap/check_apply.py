# SPDX-License-Identifier: Apache-2.0
"""Bit-compare lx.sdcflows' B0FieldTransform.apply, apply_coeffs_field and the displacement
conversions with sdcflows'.

Runs, in this interpreter's environment (each kept one: ``oracles/python-newest`` and the
shared ``.venv``):

    OPENBLAS_CORETYPE=Haswell PYTHONPATH=<repo>/python LARMORX_WORKSPACE=<workspace> \\
        python -P check_apply.py

1. ``B0FieldTransform(coeffs).apply(moving, pe_dir, ro_time, ...)`` against sdcflows': the
   unwarped data (float32 bytes), affine, header, the file nibabel writes from it
   (``to_filename`` against ``save_nibabel``, decompressed), the warnings, or the same
   exception. Synthetic coefficients and images: RAS and LAS, 3D, 4D and 5D, float32,
   float64, int16 and scaled int16 (written back scaled into int16), every option (orders 0,
   1, 3, 5; the boundary modes; ``cval`` and ``allow_negative``; ``prefilter``; ``jacobian``;
   ``output_dtype``; one or per-volume ``pe_dir``/``ro_time``, numpy float64 readout times; a
   rigid ``xfm_data2fmap`` with and without ``approx``; two control grids; NIfTI-2; an
   already fitted field; head-motion transforms), and the failures (an unknown direction, too
   few directions, a one-voxel phase-encoding axis with the Jacobian, an integer readout time:
   SDC-05, reproduced and corrected). SciPy 1.15's ``reflect``-type prefilter boundary
   (SCI-03, fixed in SciPy 1.18) is reported as EXPECTED where this environment has it.
2. Real data: fMRIPrep 25.2.6 oracle runs' coefficients on their BOLD series (ds001600's
   phase-difference and TOPUP coefficients on acq-v1 and acq-PA, 5 volumes; ds005040's TOPUP
   coefficients on its scaled int16 series of 63 volumes with its field-map-to-BOLD
   registration, both directions; ds005454's coefficients on the first 10 volumes of its
   multiband series).
3. ``apply_coeffs_field`` against the ``ApplyCoeffsField`` interface: the ``_sdc`` and
   ``_field`` files, byte-identical once decompressed.
4. ``fmap_to_disp`` / ``to_displacements``, ``disp_to_fmap`` and
   ``displacements_field_to_fieldmap`` (``DisplacementsField2Fieldmap``, ``demean``) against
   sdcflows': arrays, headers and files, every direction, ITK and not, one voxel, float64
   readout times.

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
import scipy
from sdcflows.interfaces.bspline import ApplyCoeffsField
from sdcflows.interfaces.fmap import DisplacementsField2Fieldmap
from sdcflows.transform import B0FieldTransform as SdcB0
from sdcflows.transform import disp_to_fmap as sdc_disp_to_fmap
from sdcflows.transform import fmap_to_disp as sdc_fmap_to_disp

from larmorx import sdcflows as lxs
from larmorx.sdcflows import save_nibabel

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE") or Path(__file__).resolve().parents[4])
R = WORKSPACE / "oracles" / "fmriprep-25.2.6"
BIDS = Path.home() / ".cache" / "larmorx-testdata" / "data" / "openneuro"
FAILS: list[str] = []
OLD_SCIPY = tuple(int(x) for x in scipy.__version__.split(".")[:2]) < (1, 18)
SCI03_MODES = ("reflect", "grid-mirror", "nearest")
REPRODUCE = {"upstream_defects": "reproduce"}


def report(name: str, ok: bool, detail: str = "", expected: bool = False) -> None:
    if not ok and expected:
        print(f"EXPECTED {name}: {detail} (SCI-03: SciPy {scipy.__version__})")
        return
    print(f"{'OK  ' if ok else 'FAIL'} {name}{': ' + detail if detail else ''}")
    if not ok:
        FAILS.append(name)


def raw(path) -> bytes:
    b = Path(path).read_bytes()
    return gzip.decompress(b) if b[:2] == b"\x1f\x8b" else b


def header_same(nb_hdr, lx_hdr) -> list[str]:
    klass = nb.Nifti2Header if lx_hdr.version == 2 else nb.Nifti1Header
    h = klass(lx_hdr.to_bytes())
    return [
        k
        for k in nb_hdr.keys()  # noqa: SIM118 - a nibabel header
        if k not in ("vox_offset", "scl_slope", "scl_inter")
        and nb_hdr[k].tobytes() != h[k].tobytes()
    ]


def rigid(angles, shift):
    m = np.eye(4)
    m[:3, :3] = nb.eulerangles.euler2mat(*angles)
    m[:3, 3] = shift
    return m


def image(path, data, affine, dtype=np.float32, slope=None, inter=None, version=1):
    klass = nb.Nifti2Image if version == 2 else nb.Nifti1Image
    img = klass(np.asarray(data).astype(dtype), affine, dtype=np.dtype(dtype))
    img.header.set_qform(affine, 1)
    img.header.set_sform(affine, 1)
    img.header.set_xyzt_units("mm", "sec")
    if slope is not None:
        img.header.set_slope_inter(slope, inter)
    img.to_filename(path)
    return path


def exc(e):
    return None if e is None else (type(e).__name__, str(e))


def in_memory(path):
    img = nb.load(path)
    return img.__class__(np.asanyarray(img.dataobj), img.affine, img.header)


def run_both(coeffs, moving, args, kw, mine_kw=None, refit=False, ref_in_memory=False):
    """Results of sdcflows and larmorx: (image or None, warnings, exception)."""
    out = []
    for impl in ("sdcflows", "larmorx"):
        with warnings.catch_warnings(record=True) as w:
            warnings.simplefilter("always")
            err = res = None
            try:
                if impl == "sdcflows":
                    t = SdcB0(coeffs=[nb.load(c) for c in coeffs])
                    src = in_memory(moving) if ref_in_memory else str(moving)
                    if refit:
                        t.apply(src, *args, **kw)
                    res = t.apply(src, *args, **kw)
                else:
                    t = lxs.B0FieldTransform(coeffs=[str(c) for c in coeffs])
                    k = dict(kw, n_threads=8, **(mine_kw or {}))
                    if refit:
                        t.apply(str(moving), *args, **k)
                    res = t.apply(str(moving), *args, **k)
            except Exception as e:  # the exception is the result
                err = e
            msgs = [
                f"{x.category.__name__}: {x.message}"
                for x in w
                if "aligned" in str(x.message)
                or "already fit" in str(x.message)
                or "Head-motion" in str(x.message)
                or "SDC-05" in str(x.message)
                or "SDC-08" in str(x.message)
            ]
        out.append((res, msgs, err))
    return out


def apply_case(
    name,
    coeffs,
    moving,
    *args,
    expected=False,
    mine_kw=None,
    refit=False,
    ref_in_memory=False,
    **kw,
):
    (ref, w1, e1), (mine, w2, e2) = run_both(
        coeffs, moving, args, kw, mine_kw, refit, ref_in_memory
    )
    if e1 is not None or e2 is not None:
        ok = exc(e1) == exc(e2)
        report(f"apply {name}", ok, f"{exc(e1)} vs {exc(e2)}")
        return
    with tempfile.TemporaryDirectory() as t:
        a, b = Path(t) / "a.nii.gz", Path(t) / "b.nii.gz"
        with np.errstate(all="ignore"):
            ref.to_filename(a)
        save_nibabel(mine, b, n_threads=8)
        wd = np.asanyarray(ref.dataobj)
        hd = header_same(ref.header, mine.header)
        checks = {
            "data": wd.dtype == mine.data.dtype
            and wd.shape == mine.data.shape
            and np.ascontiguousarray(wd).tobytes() == np.ascontiguousarray(mine.data).tobytes(),
            "affine": ref.affine.tobytes() == np.asarray(mine.affine).tobytes(),
            "header": not hd,
            "file": raw(a) == raw(b),
            "warnings": w1 == [x for x in w2 if "SDC-05" not in x and "SDC-08" not in x],
        }
    bad = [k for k, v in checks.items() if not v]
    detail = f"{wd.shape} {ref.header.get_data_dtype()}"
    if bad:
        detail += f"; differ: {bad}"
        if "data" in bad and wd.shape == mine.data.shape:
            d = np.abs(wd.astype(np.float64) - mine.data)
            detail += f" (max |diff| {np.nanmax(d):.3g}, {int(np.sum(d > 0))} voxels)"
        if hd:
            detail += f" header {hd}"
        if "warnings" in bad:
            detail += f" {w1} vs {w2}"
    report(f"apply {name}", not bad, detail, expected=expected)


def synthetic(tmp: Path):
    rng = np.random.default_rng(11)
    ras = np.diag([3.0, 3.0, 3.5, 1.0])
    ras[:3, 3] = (-60, -66, -40)
    shape = (24, 26, 14)
    coeffs = []
    for i, sp in enumerate([(40.0, 40.0, 20.0), (16.0, 16.0, 10.0)]):
        g = lxs.bspline_grid((np.zeros(shape, np.float32), ras), sp)
        coeffs.append(
            image(tmp / f"c{i}_coeff000.nii.gz", rng.normal(0, 30, g.shape), np.asarray(g.affine))
        )
    ii, jj, kk = np.meshgrid(*[np.linspace(-1, 1, n) for n in shape], indexing="ij")
    brain = 400 * np.exp(-(ii**2 + jj**2 + kk**2) * 2) + rng.normal(0, 4, shape)
    return rng, ras, shape, coeffs, brain


def check_apply(tmp: Path) -> None:
    _, ras, _, coeffs, brain = synthetic(tmp)
    c1 = coeffs[1:]
    las = ras.copy()
    las[0, 0] = -3.0
    las[0, 3] = 60.0
    f3 = image(tmp / "ras3d.nii.gz", brain, ras)
    series = np.stack([brain * (1 + 0.01 * t) for t in range(3)], -1)
    f4 = image(tmp / "las4d.nii.gz", series, las)
    f64 = image(tmp / "f64.nii.gz", series, ras, dtype=np.float64)
    i16 = image(tmp / "i16.nii.gz", np.round(series * 20), las, dtype=np.int16)
    s16 = image(
        tmp / "s16.nii.gz", np.round(series * 20), las, dtype=np.int16, slope=0.05, inter=-3.0
    )
    five = image(tmp / "five.nii.gz", series[:, :, :, None, :], ras)
    single = image(tmp / "single4.nii.gz", brain[..., None], las)
    n2 = image(tmp / "n2.nii", series, las, version=2)
    slab = image(tmp / "slab.nii.gz", brain[:, :, 7:8], ras)
    neg = image(tmp / "neg.nii.gz", brain - 200, ras)
    x = rigid((0.04, -0.03, 0.06), (2.0, -1.5, 1.0))

    apply_case("RAS 3D, j", c1, f3, "j", 0.05)
    apply_case("RAS 3D, two grids, j-", coeffs, f3, "j-", 0.05)
    apply_case("LAS 4D, i (flipped axis)", c1, f4, "i", 0.04)
    apply_case("LAS 4D, per-volume pe and ro", c1, f4, ["j", "j-", "k"], [0.03, 0.05, 0.02])
    apply_case("LAS 4D, no Jacobian", c1, f4, "j", 0.05, jacobian=False)
    # SDC-08: a RAS series read from a file fails in sdcflows (an ArrayProxy given to
    # rollaxis); corrected, it gives sdcflows' result for the same data in memory
    apply_case("float64 4D RAS (SDC-08 reproduced)", c1, f64, "j", 0.05, mine_kw=REPRODUCE)
    apply_case("float64 4D RAS (SDC-08 corrected)", c1, f64, "j", 0.05, ref_in_memory=True)
    apply_case("int16 4D (written scaled)", c1, i16, "j-", 0.05)
    apply_case("scaled int16 4D", c1, s16, "i-", 0.05)
    apply_case("scaled int16, output float32", c1, s16, "j", 0.05, output_dtype="float32")
    apply_case("float32, output uint8", c1, f4, "j", 0.05, output_dtype="uint8")
    apply_case("5D (X, Y, Z, 1, T) RAS (SDC-08 reproduced)", c1, five, "j", 0.05, mine_kw=REPRODUCE)
    apply_case("5D (X, Y, Z, 1, T) RAS (SDC-08 corrected)", c1, five, "j", 0.05, ref_in_memory=True)
    five_las = image(tmp / "five_las.nii.gz", series[:, :, :, None, :], las)
    apply_case("5D (X, Y, Z, 1, T) LAS", c1, five_las, "j", 0.05)
    two_vec = image(tmp / "five2.nii.gz", np.stack([series, series], 3), las)
    apply_case("5D (X, Y, Z, 2, T) LAS", c1, two_vec, ["j"] * 3, [0.05] * 3)
    apply_case("5D (X, Y, Z, 2, T) LAS, one direction", c1, two_vec, "j", 0.05)
    apply_case("4D with one volume", c1, single, "j", 0.05)
    apply_case("NIfTI-2", c1, n2, "j", 0.05)
    apply_case("one slice, j", c1, slab, "j", 0.05)
    apply_case("one slice, k with the Jacobian (numpy's error)", c1, slab, "k", 0.05)
    apply_case("negative values to cval 7", c1, neg, "j", 0.05, cval=7.0)
    apply_case("negative values kept", c1, neg, "j", 0.05, allow_negative=True)
    apply_case("no prefilter", c1, f4, "j", 0.05, prefilter=False)
    for order in (0, 1, 5):
        apply_case(f"order {order}", c1, f4, "j", 0.05, order=order)
    for mode in ("nearest", "reflect", "mirror", "wrap", "grid-wrap", "grid-constant"):
        apply_case(
            f"mode {mode}", c1, f4, "j", 0.08, mode=mode, expected=OLD_SCIPY and mode in SCI03_MODES
        )
    apply_case("mode invalid", c1, f4, "j", 0.05, mode="edge")
    apply_case("order 6", c1, f4, "j", 0.05, order=6)
    apply_case("numpy float64 readout time", c1, f4, "j", np.float64(0.05))
    apply_case("numpy float64 readout times", c1, f4, "i-", [np.float64(0.05)] * 3)
    apply_case("rigid xfm, approx", c1, f4, "j", 0.05, xfm_data2fmap=x)
    apply_case("rigid xfm, no approx", c1, f4, "j", 0.05, xfm_data2fmap=x, approx=False)
    apply_case("head-motion transforms ignored", c1, f4, "j", 0.05, xfms=[np.eye(4)] * 3)
    apply_case("already fit (applied twice)", c1, f4, "j", 0.05, refit=True)
    apply_case("num_threads 2", c1, f4, "j", 0.05, num_threads=2)
    apply_case("pe x (an FSL letter)", c1, f4, "x", 0.05)
    apply_case("too few directions", c1, f4, ["j", "j"], [0.05, 0.05])
    apply_case(
        "integer readout time, 4D (SDC-05 reproduced)",
        c1,
        f4,
        "j",
        1,
        mine_kw={"upstream_defects": "reproduce"},
    )
    apply_case(
        "integer readout time, 3D (SDC-05 reproduced)",
        c1,
        f3,
        "j",
        1,
        mine_kw={"upstream_defects": "reproduce"},
    )
    # SDC-05 corrected: an integer or float32 readout time gives sdcflows' result for the float
    ref = run_both(c1, f4, ("j", 1.0), {})[0][0]
    for ro in (1, np.float32(0.05)):
        (_, _, _), (mine, w, e) = run_both(c1, f4, ("j", ro), {})
        want = ref if ro == 1 else run_both(c1, f4, ("j", float(ro)), {})[0][0]
        ok = (
            e is None
            and np.ascontiguousarray(np.asanyarray(want.dataobj)).tobytes()
            == np.ascontiguousarray(mine.data).tobytes()
            and any("SDC-05" in m for m in w)
        )
        report(f"apply readout time {type(ro).__name__} (SDC-05 corrected, warned)", ok)


def bold_head(src: Path, dst: Path, n: int) -> Path:
    img = nb.load(src)
    sub = img.slicer[..., :n]
    nb.Nifti1Image(np.asanyarray(sub.dataobj), img.affine, img.header).to_filename(dst)
    return dst


def check_real(tmp: Path) -> None:
    r1600 = R / "ds001600-sub-1-noreconall" / "out" / "sub-1" / "fmap"
    b1600 = BIDS / "ds001600" / "sub-1" / "func"
    w5040 = R / "ds005040-sub-002-noreconall" / "work" / "fmriprep_25_2_wf" / "sub_002_wf"
    b5040 = BIDS / "ds005040" / "sub-002" / "func" / "sub-002_task-clip_run-1_bold.nii.gz"
    w5454 = R / "ds005454-sub-16-noreconall" / "work" / "fmriprep_25_2_wf" / "sub_16_wf"
    b5454 = BIDS / "ds005454" / "sub-16" / "func" / "sub-16_task-rest_bold.nii.gz"
    if r1600.exists() and b1600.exists():
        v1 = r1600 / "sub-1_acq-v1_fmapid-auto00000_desc-coeff_fieldmap.nii.gz"
        pepolar = r1600 / "sub-1_fmapid-auto00003_desc-coeff_fieldmap.nii.gz"
        apply_case(
            "ds001600 acq-v1 (phase1/phase2 field)",
            [v1],
            b1600 / "sub-1_task-rest_acq-v1_bold.nii.gz",
            "j-",
            0.0346501,
        )
        apply_case(
            "ds001600 acq-PA (TOPUP field)",
            [pepolar],
            b1600 / "sub-1_task-rest_acq-PA_bold.nii.gz",
            "j",
            0.0426603,
        )
    else:
        print("skipped (no oracle run here): ds001600")
    fixed = sorted(w5040.glob("fmap_preproc_wf/wf_auto_00000/fix_coeff/*_fixed.nii.gz"))
    xfm = (
        w5040
        / "bold_task_clip_run_1_wf/bold_fit_wf/fmapreg_wf/coregister/transform0GenericAffine.mat"
    )
    if fixed and b5040.exists() and xfm.exists():
        apply_case("ds005040 scaled int16 series, TOPUP", fixed, b5040, "j", 0.0137951)
        coeff_case("ds005040, fmap2data_xfm", fixed, b5040, "j", 0.0137951, fmap2data_xfm=xfm)
        coeff_case("ds005040, data2fmap_xfm", fixed, b5040, "j", 0.0137951, data2fmap_xfm=[xfm])
    else:
        print("skipped (no oracle run here): ds005040")
    c5454 = sorted(w5454.glob("fmap_preproc_wf/wf_gre16/bs_filter/*_coeff000.nii.gz"))
    x5454 = (
        w5454 / "bold_task_rest_wf/bold_fit_wf/fmapreg_wf/coregister/transform0GenericAffine.mat"
    )
    if c5454 and b5454.exists():
        head = bold_head(b5454, tmp / "sub-16_task-rest_bold.nii.gz", 10)
        coeff_case("ds005454 first 10 volumes", c5454, head, "j-", 0.0429303, fmap2data_xfm=x5454)
    else:
        print("skipped (no oracle run here): ds005454")


def coeff_case(name, coeffs, data, pe, ro, **xfm):
    with tempfile.TemporaryDirectory() as t1, tempfile.TemporaryDirectory() as t2:
        cwd = os.getcwd()
        os.chdir(t1)
        try:
            node = ApplyCoeffsField(pe_dir=pe, ro_time=ro, num_threads=8)
            node.inputs.in_data = str(data)
            node.inputs.in_coeff = [str(c) for c in coeffs]
            for k, v in xfm.items():
                setattr(node.inputs, k, [str(p) for p in v] if isinstance(v, list) else str(v))
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                outs = node.run().outputs
        finally:
            os.chdir(cwd)
        mine = lxs.apply_coeffs_field(
            str(data),
            [str(c) for c in coeffs],
            ro,
            pe,
            out_dir=t2,
            n_threads=8,
            **{k: ([str(p) for p in v] if isinstance(v, list) else str(v)) for k, v in xfm.items()},
        )
        checks = {
            "names": Path(outs.out_corrected).name == mine.out_corrected.name
            and Path(outs.out_field).name == mine.out_field.name,
            "corrected": raw(outs.out_corrected) == raw(mine.out_corrected),
            "field": raw(outs.out_field) == raw(mine.out_field),
        }
    bad = [k for k, v in checks.items() if not v]
    report(f"ApplyCoeffsField {name}", not bad, f"differ: {bad}" if bad else "")


def check_coeffs_field(tmp: Path) -> None:
    _, ras, _, coeffs, brain = synthetic(tmp)
    las = ras.copy()
    las[0, 0], las[0, 3] = -3.0, 60.0
    series = np.stack([brain * (1 + 0.01 * t) for t in range(3)], -1)
    i16 = image(tmp / "bold_i16.nii.gz", np.round(series * 20), las, dtype=np.int16)
    xf = tmp / "xfm.txt"
    m = rigid((0.03, 0.02, -0.04), (1.0, 2.0, -1.0))
    lps = np.diag([-1.0, -1.0, 1.0, 1.0])
    itk = lps @ m @ lps
    params = " ".join(repr(float(v)) for v in [*itk[:3, :3].ravel(), *itk[:3, 3]])
    xf.write_text(
        "#Insight Transform File V1.0\n#Transform 0\nTransform: AffineTransform_float_3_3\n"
        f"Parameters: {params}\nFixedParameters: 0 0 0\n",
        encoding="utf-8",
    )
    coeff_case("synthetic int16, no transform", coeffs[1:], i16, ["j"], [0.05])
    coeff_case("synthetic int16, data2fmap_xfm", coeffs[1:], i16, "i-", 0.05, data2fmap_xfm=xf)
    coeff_case("synthetic int16, fmap2data_xfm", coeffs, i16, "j", 0.04, fmap2data_xfm=xf)


def disp_case(name, fmap, epi, ro, pe, itk=True):
    errs = []
    res = []
    for impl in ("sdcflows", "larmorx"):
        try:
            if impl == "sdcflows":
                d = sdc_fmap_to_disp(nb.load(fmap), ro, pe, itk_format=itk)
                f = sdc_disp_to_fmap(d, nb.load(epi), ro, pe, itk_format=itk)
            else:
                d = lxs.fmap_to_disp(str(fmap), ro, pe, itk_format=itk)
                f = lxs.disp_to_fmap(d, str(epi), ro, pe, itk_format=itk)
            res.append((d, f))
            errs.append(None)
        except Exception as e:  # the exception is the result
            res.append(None)
            errs.append(exc(e))
    if errs[0] or errs[1]:
        report(f"displacements {name}", errs[0] == errs[1], f"{errs[0]} vs {errs[1]}")
        return
    checks = {}
    with tempfile.TemporaryDirectory() as t:
        for label, (a, b) in zip(("disp", "fmap"), zip(res[0], res[1], strict=True), strict=True):
            wa = np.asanyarray(a.dataobj)
            checks[f"{label}-data"] = (
                wa.dtype == b.data.dtype and wa.tobytes() == np.ascontiguousarray(b.data).tobytes()
            )
            checks[f"{label}-header"] = not header_same(a.header, b.header)
            pa, pb = Path(t) / f"{label}a.nii.gz", Path(t) / f"{label}b.nii.gz"
            a.to_filename(pa)
            save_nibabel(b, pb)
            checks[f"{label}-file"] = raw(pa) == raw(pb)
    bad = [k for k, v in checks.items() if not v]
    report(f"displacements {name}", not bad, f"differ: {bad}" if bad else "")


def check_displacements(tmp: Path) -> None:
    rng = np.random.default_rng(5)
    ras = np.diag([2.0, 2.5, 3.0, 1.0])
    ras[:3, 3] = (-30, -40, -20)
    obl = rigid((0.1, -0.05, 0.2), (3, 1, -2)) @ ras
    las = np.diag([-2.0, 2.5, 3.0, 1.0])
    las[:3, 3] = (30, -40, -20)
    fm = image(tmp / "fm.nii.gz", rng.normal(0, 40, (20, 18, 12)), ras)
    fmo = image(tmp / "fmo.nii.gz", rng.normal(0, 40, (20, 18, 12)), obl)
    fml = image(tmp / "fml.nii.gz", rng.normal(0, 40, (20, 18, 12)), las, dtype=np.float64)
    one = image(tmp / "one.nii.gz", np.full((1, 1, 1), 33.3), ras)
    for pe in ("i", "i-", "j", "j-", "k", "k-"):
        disp_case(f"RAS {pe}", fm, fm, 0.05, pe)
    disp_case("oblique j", fmo, fmo, 0.05, "j")
    disp_case("oblique j, not ITK", fmo, fmo, 0.05, "j", itk=False)
    disp_case("LAS float64 i- on a RAS EPI", fml, fm, 0.031, "i-")
    disp_case("one voxel", one, one, 0.05, "k")
    disp_case("numpy float64 readout time", fm, fm, np.float64(0.05), "j-")
    disp_case("readout time 0", fm, fm, 0.0, "j")
    disp_case("pe y", fm, fm, 0.05, "y")
    # the transform as an object and to_displacements
    t1 = SdcB0(coeffs=[nb.load(c) for c in synthetic(tmp)[3][1:]])
    t1.fit(nb.load(fm))
    t2 = lxs.B0FieldTransform(coeffs=[str(c) for c in synthetic(tmp)[3][1:]])
    t2.fit(str(fm))
    a, b = t1.to_displacements(0.05, "j-"), t2.to_displacements(0.05, "j-")
    report(
        "to_displacements",
        np.asanyarray(a.dataobj).tobytes() == np.ascontiguousarray(b.data).tobytes()
        and not header_same(a.header, b.header),
    )
    # DisplacementsField2Fieldmap (files, demean)
    dpath = tmp / "sub-01_xfm.nii.gz"
    sdc_fmap_to_disp(nb.load(fm), 0.05, "j").to_filename(dpath)
    for demean in (False, True):
        for itk in (True, False):
            with tempfile.TemporaryDirectory() as t1d, tempfile.TemporaryDirectory() as t2d:
                cwd = os.getcwd()
                os.chdir(t1d)
                try:
                    node = DisplacementsField2Fieldmap(
                        transform=str(dpath),
                        epi=str(fm),
                        ro_time=0.05,
                        pe_dir="j",
                        demean=demean,
                        itk_transform=itk,
                    )
                    outs = node.run().outputs
                finally:
                    os.chdir(cwd)
                mine = lxs.displacements_field_to_fieldmap(
                    str(dpath), str(fm), 0.05, "j", demean=demean, itk_transform=itk, out_dir=t2d
                )
                ok = Path(outs.out_file).name == mine.out_file.name and raw(outs.out_file) == raw(
                    mine.out_file
                )
            report(f"DisplacementsField2Fieldmap demean={demean} itk={itk}", ok)


def main() -> int:
    print(f"numpy {np.__version__}, scipy {scipy.__version__}")
    with tempfile.TemporaryDirectory() as tmp:
        check_apply(Path(tmp))
        check_coeffs_field(Path(tmp))
        check_displacements(Path(tmp))
        check_real(Path(tmp))
    print("ALL OK" if not FAILS else f"FAILED: {len(FAILS)}: {', '.join(FAILS)}")
    return 0 if not FAILS else 1


if __name__ == "__main__":
    sys.exit(main())
