# SPDX-License-Identifier: Apache-2.0
"""Parity of the 3dvolreg replica (``larmorx afni 3dvolreg`` with
``LARMORX_IMPLEMENTATION=replica``, in-process; ``lx.afni.volreg(..., implementation="replica")``)
with AFNI's ``3dvolreg`` (26.3.05; each case also runs 25.2.09, AFNI's own step): exit status,
messages and every output byte for byte: the registered series (data and NIfTI header), the
parameters (``-1Dfile``), the RMS file (``-dfile``), the matrices (``-1Dmatrix_save``), the
maximum displacements (``-maxdisp1D``), the displacement datasets (``-savedisp``,
``-SAVEDISP``) and ``-rotcom``'s commands (standard output)
(:mod:`larmorx_validation.parity.afni_replica`).

Cases: every option and its paths on synthetic moving series (an ellipsoid with texture,
rotated and shifted per volume, with noise: :func:`_make_series`) and their variants (int16,
scaled, bytes, oblique with odd sizes, sagittal, slice timing, grids for ``-gridparent``, a
shifted base, one, two and four slices, a constant series, one volume, no motion, large
rotations, NaN), ``-rotparent`` datasets made by AFNI's own 3dvolreg (its ``VOLREG_*``
attributes); the errors and crashes; the defects (D13: AFNI-86 to AFNI-96); AFNI's global
options and the variables 3dvolreg reads; and real data: fMRIPrep's own call (niworkflows'
``RobustAverage``: ``3dvolreg -Fourier -twopass -1Dfile -1Dmatrix_save -prefix -zpad 4
-maxdisp1D``) on the volumes its BOLD reference averaged, re-made from the oracle runs'
``gen_avg`` nodes (:func:`_sliced`), and sdcflows' PEPOLAR reference averages
(``ref_average``, ds005040 and ds001600), each compared with fMRIPrep's own output of the node
too (the median of the registered volumes, :func:`_average`) and run through fMRIPrep 25.2.6's
own AFNI (its image).

3dvolreg has no OpenMP loop (its binary does not link libgomp): AFNI runs with
``OMP_NUM_THREADS=1``, its runs cached at once. The replica's default (``fix``) uses correctly
rounded maths where ``reproduce`` uses glibc 2.35's (GLIBC-07, which an iterative fit does not
name): the fix pass allows changes with no defect named as GLIBC-07's.

The derived inputs are made once in ``<workspace>/oracles/inputs/afni-volreg/`` (seeded, with
SciPy's interpolation for the synthetic motion; Linux x86-64 only, as every parity input).
"""

from __future__ import annotations

import ast
import contextlib
import functools
import re
import shutil
from pathlib import Path

import numpy as np

from larmorx_validation.parity import afni_replica as ar
from larmorx_validation.parity import afni_versions
from larmorx_validation.parity import defects as d13
from larmorx_validation.parity.harness import Case, CheckList, Suite

PROGRAM = "3dvolreg"
SUITE = "afni-volreg"
#: The derived inputs' versions (each a stamp in their directory): a new version adds inputs,
#: never changes the ones made.
INPUTS_VERSION = 2
OUT = "o.nii"
OGZ = "o.nii.gz"
ONE = (("OMP_NUM_THREADS", "1"),)
#: AFNI-92's notice names a condition (``-nomaxdisp -twopass``: the coarse rotation search's
#: automask from another clip fraction) whose effect the outputs may not show.
NOMAXDISP = ("AFNI-92",)


def S(args, inputs, outputs, **kw) -> ar.Scenario:
    """A case (:class:`afni_replica.Scenario`), with one OpenMP thread (3dvolreg has none)."""
    env = (*ONE, *kw.pop("env", ()))
    return ar.Scenario(tuple(args), tuple(inputs), tuple(outputs), env=env, **kw)


def _py(outputs: dict[str, str] | None = None, **kw) -> dict:
    return {"python": kw, "python_outputs": outputs or {"registered": OUT}}


# ------------------------------------------------------------------------------------------------
# Inputs


def workspace() -> Path:
    ws = ar.workspace()
    if ws is None:
        raise FileNotFoundError("the workspace (oracles/) was not found")
    return ws


def _save(path: Path, arr, affine, *, tr=None, slice_code=0, slope=None, qform=1, sform=1):
    import nibabel as nib

    im = nib.Nifti1Image(arr, affine)
    im.set_qform(affine if qform else None, qform)
    im.set_sform(affine if sform else None, sform)
    h = im.header
    h.set_xyzt_units("mm", "sec")
    if tr is not None:
        h["pixdim"][4] = tr
    if slice_code:
        h.set_dim_info(slice=2)
        h["slice_code"] = slice_code
        h["slice_start"] = 0
        h["slice_end"] = arr.shape[2] - 1
        h["slice_duration"] = tr / arr.shape[2]
    if slope is not None:
        h.set_slope_inter(slope, 0)
    im.to_filename(path)


def _make_series(out: Path, shape=(40, 44, 24, 6), *, degrees=0.6, shift=(0.3, -0.2, 0.15),
                 seed=7, axis=2) -> None:  # fmt: skip
    """A synthetic moving series (``validation/investigations/afni-volreg/make_series.py``):
    an ellipsoid with texture, volume ``t`` rotated ``degrees * t`` about ``axis`` and shifted
    ``shift * t`` voxels, with noise; float32, 3 x 3 x 3.5 mm, RAI, TR 2 s."""
    import nibabel as nib
    from scipy import ndimage

    nx, ny, nz, nt = shape
    rng = np.random.default_rng(seed)
    x, y, z = np.meshgrid(
        np.arange(nx) - nx / 2, np.arange(ny) - ny / 2, np.arange(nz) - nz / 2, indexing="ij"
    )
    r = (x / (nx * 0.35)) ** 2 + (y / (ny * 0.38)) ** 2 + (z / (nz * 0.4)) ** 2
    base = np.where(r < 1, 800 + 300 * np.cos(x / 3.0) * np.sin(y / 4.0) + 50 * z, 0.0)
    base += rng.normal(0, 10, base.shape) * (r < 1.2)
    base = ndimage.gaussian_filter(base, 0.7)
    vols = []
    for t in range(nt):
        ang = np.deg2rad(degrees * t) if t else 0.0
        c, s = np.cos(ang), np.sin(ang)
        a, b = [k for k in range(3) if k != axis]
        m = np.eye(3)
        m[a, a], m[a, b], m[b, a], m[b, b] = c, -s, s, c
        centre = np.array([nx, ny, nz]) / 2
        off = centre - m @ centre + np.asarray(shift) * t
        v = ndimage.affine_transform(base, m, offset=off, order=3)
        vols.append(v + rng.normal(0, 5, v.shape))
    data = np.stack(vols, axis=-1).astype(np.float32)
    aff = np.diag([-3.0, -3.0, 3.5, 1.0])
    aff[:3, 3] = [60, 66, -40]
    img = nib.Nifti1Image(data, aff)
    img.header.set_xyzt_units("mm", "sec")
    img.header["pixdim"][4] = 2.0
    img.set_qform(aff, 1)
    img.set_sform(aff, 1)
    img.to_filename(out)


def _variants(d: Path) -> None:
    """``variants.py``'s and ``variants2.py``'s inputs (relays 1 and 2), from ``s1.nii.gz``."""
    import nibabel as nib

    img = nib.load(d / "s1.nii.gz")
    data = np.asarray(img.dataobj, dtype=np.float32)
    aff = img.affine
    # LPI (a positive diagonal), int16; int16 with a slope.
    a2 = np.diag([3.0, 3.0, 3.5, 1.0])
    a2[:3, 3] = [-60, -66, -40]
    i2 = nib.Nifti1Image(np.round(data).astype(np.int16), a2)
    i2.header.set_xyzt_units("mm", "sec")
    i2.set_qform(a2, 1)
    i2.set_sform(a2, 1)
    i2.to_filename(d / "s2_short.nii.gz")
    i2b = nib.Nifti1Image(np.round(data * 4).astype(np.int16), a2)
    i2b.header.set_slope_inter(0.25, 0)
    i2b.set_qform(a2, 1)
    i2b.set_sform(a2, 1)
    i2b.to_filename(d / "s2_scaled.nii.gz")
    # Oblique (8 degrees), odd sizes (39 x 43 x 23).
    c = data[:39, :43, :23, :]
    th = np.deg2rad(8)
    r = np.array([[np.cos(th), -np.sin(th), 0], [np.sin(th), np.cos(th), 0], [0, 0, 1]])
    a3 = np.eye(4)
    a3[:3, :3] = r @ np.diag([2.5, -3.0, 3.5])
    a3[:3, 3] = [-50, 60, -30]
    i3 = nib.Nifti1Image(c, a3)
    i3.set_qform(a3, 1)
    i3.set_sform(a3, 1)
    i3.to_filename(d / "s3_oblique.nii.gz")
    # Sagittal: i = A-P, j = S-I, k = L-R.
    p = np.transpose(data, (1, 2, 0, 3))
    a4 = np.zeros((4, 4))
    a4[1, 0], a4[2, 1], a4[0, 2], a4[3, 3] = -3.0, 3.5, 3.0, 1
    a4[:3, 3] = [-60, 66, -40]
    i4 = nib.Nifti1Image(np.ascontiguousarray(p), a4)
    i4.set_qform(a4, 1)
    i4.set_sform(a4, 1)
    i4.to_filename(d / "s4_sagittal.nii.gz")
    nib.Nifti1Image(data[..., 2], aff).to_filename(d / "base3d.nii.gz")
    nib.Nifti1Image(np.abs(data[..., :2]).astype(np.float32), aff).to_filename(d / "weight.nii.gz")
    nib.Nifti1Image(np.clip(data / 6, 0, 255).astype(np.uint8), aff).to_filename(
        d / "s5_byte.nii.gz"
    )
    # Slice timing (TR 2 s, sequential and alternating), -gridparent grids, a moved base,
    # a constant series, one and four slices.
    _save(d / "s6_st.nii.gz", data, aff, tr=2.0, slice_code=1)
    _save(d / "s6_stneg.nii.gz", np.round(-data * 4).astype(np.int16), aff, tr=2.0,
          slice_code=3, slope=-0.25)  # fmt: skip
    _save(d / "s6_st4.nii.gz", data[..., :4], aff, tr=2.0, slice_code=1)
    zero = np.zeros((40, 44, 27), np.float32)
    _save(d / "gp27.nii.gz", zero, aff)
    _save(d / "gp24.nii.gz", np.zeros((40, 44, 24), np.float32), aff)
    _save(d / "gp22.nii.gz", np.zeros((40, 44, 22), np.float32), aff)
    _save(d / "gpx.nii.gz", np.zeros((41, 44, 27), np.float32), aff)
    a_sp = aff.copy()
    a_sp[2, 2] = 4.0
    _save(d / "gpsp.nii.gz", zero, a_sp)
    a_gm = aff.copy()
    a_gm[:3, 3] += [1.5, -2.0, -3.5]
    _save(d / "gp27m.nii.gz", np.zeros((40, 44, 27, 2), np.float32), a_gm)
    a_b = aff.copy()
    a_b[:3, 3] += [3.0, -2.0, 1.75]
    _save(d / "base_shift.nii.gz", data[..., 2], a_b)
    _save(d / "s7_const.nii.gz", np.full((40, 44, 24, 3), 100.0, np.float32), aff)
    _save(d / "s7_nz1.nii.gz", np.ascontiguousarray(data[:, :, 12:13, :4]), aff)
    _save(d / "s7_nz4.nii.gz", np.ascontiguousarray(data[:, :, 10:14, :4]), aff)


def _edge_inputs(d: Path) -> None:
    """Relay 3's edge cases: one volume, no motion, large rotations (12 degrees per volume
    about each axis), NaN, two slices, a NIfTI-2 file, an uncompressed file, a qform only,
    int16 big-endian, a 3D series of one volume with an external base."""
    import nibabel as nib

    img = nib.load(d / "s1.nii.gz")
    data = np.asarray(img.dataobj, dtype=np.float32)
    aff = img.affine
    _save(d / "e_one.nii.gz", data[..., :1], aff, tr=2.0)
    _save(d / "e_still.nii.gz", np.repeat(data[..., :1], 4, axis=3), aff, tr=2.0)
    _make_series(d / "e_large_z.nii.gz", (40, 44, 24, 4), degrees=12.0, shift=(1.0, -1.5, 0.5),
                 seed=11, axis=2)  # fmt: skip
    _make_series(d / "e_large_x.nii.gz", (40, 44, 24, 4), degrees=9.0, shift=(0.5, 0.5, -0.5),
                 seed=12, axis=0)  # fmt: skip
    nan = data.copy()
    nan[10:14, 20:24, 8:10, 2] = np.nan
    _save(d / "e_nan.nii.gz", nan, aff, tr=2.0)
    _save(d / "e_nz2.nii.gz", np.ascontiguousarray(data[:, :, 11:13, :4]), aff, tr=2.0)
    n2 = nib.Nifti2Image(data, aff)
    n2.set_qform(aff, 1)
    n2.set_sform(aff, 1)
    n2.header.set_xyzt_units("mm", "sec")
    n2.to_filename(d / "e_nifti2.nii")
    _save(d / "e_plain.nii", data, aff, tr=2.0)
    _save(d / "e_qform.nii.gz", data, aff, tr=2.0, sform=0)
    be = nib.Nifti1Image(np.round(data).astype(">i2"), aff)
    be.set_qform(aff, 1)
    be.set_sform(aff, 1)
    be.to_filename(d / "e_bigendian.nii")
    _save(d / "e_vol3d.nii.gz", data[..., 4], aff)


def _rotparents(d: Path) -> None:
    """``-rotparent`` datasets: AFNI's own 3dvolreg outputs (``-base 3``), which keep their
    ``VOLREG_*`` attributes in AFNI's NIfTI extension (made once, through the oracle cache)."""
    env = ar.environment((*ONE, ("AFNI_NIFTI_NOEXT", None)))
    for src, out in (
        ("s1.nii.gz", "rp.nii.gz"),
        ("s6_st.nii.gz", "rp_st.nii.gz"),
        ("s3_oblique.nii.gz", "rp_obl.nii.gz"),
    ):
        work = d / f"make-{out}"
        shutil.rmtree(work, ignore_errors=True)
        work.mkdir()
        r = ar.run_afni(PROGRAM, ["-base", "3", "-prefix", out, str(d / src)], env, work,
                        version=afni_versions.NEWEST)  # fmt: skip
        if r.code != 0 or not (work / out).is_file():
            raise RuntimeError(f"AFNI's 3dvolreg did not make {out}: {r.err[-400:]}")
        (work / out).replace(d / out)
        shutil.rmtree(work)


@contextlib.contextmanager
def _locked(d: Path):
    import fcntl

    d.mkdir(parents=True, exist_ok=True)
    with (d / ".lock").open("w") as f:
        fcntl.flock(f, fcntl.LOCK_EX)
        try:
            yield
        finally:
            fcntl.flock(f, fcntl.LOCK_UN)


def _v1(d: Path) -> None:
    _make_series(d / "s1.nii.gz")
    _variants(d)
    _edge_inputs(d)
    _rotparents(d)


def _v2(d: Path) -> None:
    """A series of 120 x 120 x 120 voxels (3 volumes): -twopass's coarse search blurs and
    subsamples a volume of 120 voxels or more along every axis (and AFNI-94's case)."""
    _make_series(d / "e_big120.nii.gz", (120, 120, 120, 3), degrees=2.0, shift=(0.6, -0.4, 0.3),
                 seed=13)  # fmt: skip


@functools.cache
def inputs_dir() -> Path:
    """The derived inputs (``oracles/inputs/afni-volreg/``), made once, version by version
    (under a lock: the suite's workers share them)."""
    d = workspace() / "oracles" / "inputs" / SUITE
    if (d / f"DONE-v{INPUTS_VERSION}").is_file():
        return d
    with _locked(d):
        for version, make in ((1, _v1), (2, _v2)):
            stamp = d / f"DONE-v{version}"
            if not stamp.is_file():
                make(d)
                stamp.write_text(f"derived inputs of the {SUITE} suite, version {version}\n")
    return d


# ------------------------------------------------------------------------------------------------
# fMRIPrep's calls


FMRIPREP_RUNS = "oracles/fmriprep-25.2.6"
#: The oracle runs' RobustAverage nodes in the suite: label -> (run, the node's directory
#: below ``work/fmriprep_25_2_wf/<subject>_wf/``, tier, what it adds).
CALLS = {
    "ds000005-run-01": ("ds000005-sub-01-noreconall",
                        "bold_task_mixedgamblestask_run_01_wf/bold_fit_wf/hmc_boldref_wf/gen_avg",
                        "smoke", "single-band, int16, 64 x 64 x 34, 20 volumes"),
    "ds000005-run-03": ("ds000005-sub-01-noreconall",
                        "bold_task_mixedgamblestask_run_03_wf/bold_fit_wf/hmc_boldref_wf/gen_avg",
                        "standard", "two volumes"),
    "ds005454": ("ds005454-sub-16-noreconall", "bold_task_rest_wf/bold_fit_wf/hmc_boldref_wf/gen_avg",
                 "standard", "multiband, 160 x 160 x 96 (the subsampled coarse search), oblique"),
    "ds003592-echo-1": ("ds003592-sub-280-noreconall",
                        "bold_ses_1_task_rest_echo_1_wf/bold_fit_wf/hmc_boldref_wf/gen_avg",
                        "standard", "multi-echo (echo 1), oblique, 64 x 64 x 43"),
    "ds004958": ("ds004958-sub-fsm64ps-noreconall", "bold_task_rest_wf/bold_fit_wf/hmc_boldref_wf/gen_avg",
                 "standard", "multiband 8, uint16 stored, 104 x 104 x 72, 3 volumes, 24 degrees oblique"),
    "ds002603-mb4": ("ds002603-sub-01-noreconall",
                     "bold_task_wm_acq_Mb4Mm3Tr550_wf/bold_fit_wf/hmc_boldref_wf/gen_avg",
                     "standard", "Philips multiband 4, float32 stored"),
    "ds000228": ("ds000228-sub-pixar008-noreconall", "bold_task_pixar_wf/bold_fit_wf/hmc_boldref_wf/gen_avg",
                 "full", "a 3.5-year-old, 21 degrees oblique"),
    "ds000256": ("ds000256-sub-CTS203-noreconall",
                 "bold_task_restbaseline_wf/bold_fit_wf/hmc_boldref_wf/gen_avg", "full", "a child, motion"),
    "ds002422": ("ds002422-sub-02-noreconall", "bold_task_rest_wf/bold_fit_wf/hmc_boldref_wf/gen_avg",
                 "full", "1.5 T"),
    "ds004884": ("ds004884-sub-M2079-noreconall",
                 "bold_ses_217_task_naming40_acq_epfid2_dir_AP_run_2_wf/bold_fit_wf/hmc_boldref_wf/gen_avg",
                 "full", "a stroke"),
    "ds005003": ("ds005003-sub-RS001-noreconall",
                 "bold_ses_1_task_rest_wf/bold_fit_wf/hmc_boldref_wf/gen_avg", "full", "a glioma"),
    "ds005040": ("ds005040-sub-002-noreconall", "bold_task_clip_run_1_wf/bold_fit_wf/hmc_boldref_wf/gen_avg",
                 "full", "PEPOLAR run's BOLD"),
    "ds005752-reverse": ("ds005752-sub-ON01016-noreconall",
                         "bold_ses_01_task_rest_dir_reverse_wf/bold_fit_wf/hmc_boldref_wf/gen_avg",
                         "full", "10 volumes"),
    "pepolar-ds005040": ("ds005040-sub-002-noreconall", "fmap_preproc_wf/wf_auto_00000/ref_average",
                         "standard", "sdcflows' PEPOLAR reference average: 2 topup-corrected volumes, float32"),
    "pepolar-ds001600": ("ds001600-sub-1-noreconall", "fmap_preproc_wf/wf_auto_00003/ref_average",
                         "standard", "sdcflows' PEPOLAR reference average: 2 volumes, 80 x 80 x 60"),
}  # fmt: skip


def _node(label: str) -> Path:
    run, rel, _, _ = CALLS[label]
    found = sorted(
        (workspace() / FMRIPREP_RUNS / run / "work" / "fmriprep_25_2_wf").glob(f"*_wf/{rel}")
    )
    if not found:
        raise FileNotFoundError(f"{run}: {rel}")
    return found[0]


def _node_inputs(node: Path) -> tuple[Path, list[bool] | None, bool]:
    """A RobustAverage node's input file, ``t_mask`` and ``nonnegative`` (its report)."""
    rep = (node / "_report" / "report.rst").read_text(encoding="utf-8")
    exe = rep.split("Execution Inputs")[1].split("Execution Outputs")[0]
    in_file = re.search(r"\* in_file : (\S+)", exe).group(1)
    tm = re.search(r"\* t_mask : (\[.*?\]|<undefined>)", exe).group(1)
    t_mask = None if tm == "<undefined>" else ast.literal_eval(tm)
    nonneg = re.search(r"\* nonnegative : (\S+)", exe).group(1) == "True"
    if in_file.startswith("/work/"):
        work = Path(*node.parts[: node.parts.index("work") + 1])
        src = work / Path(in_file).relative_to("/work")
    else:
        import larmorx_testdata as td

        dataset = node.parts[node.parts.index("work") - 1].split("-")[0]
        src = Path(td.get(f"openneuro/{dataset}/{Path(in_file).relative_to('/data')}"))
    return src, t_mask, nonneg


#: niworkflows' ``RobustAverage`` (``interfaces/images.py``) up to the ``_sliced`` file it gives
#: 3dvolreg, its own lines, run with fMRIPrep 25.2.6's Python in its image (numpy 2.2.6, nibabel
#: 5.3.2): the drift's mean over the volumes sums float32 values in an order numpy 2.3 changed, so
#: the shared environment's numpy re-makes another input (2,002 of 2,785,280 stored values one
#: unit apart on ds000005's run 1). Arguments: the input, a file naming the volumes kept (``0``/``1``
#: per volume, or ``all``), ``nonnegative`` (``1``/``0``), the output.
SLICED_SCRIPT = """\
import sys

import nibabel as nb
import numpy as np

in_file, nonneg, out = sys.argv[1], sys.argv[3] == "1", sys.argv[4]
with open(sys.argv[2], encoding="utf-8") as f:
    mask = f.read().strip()
img = nb.load(in_file)
img.header.extensions.clear()
img = nb.squeeze_image(img)
t_mask = [True] * img.shape[3] if mask == "all" else [c == "1" for c in mask]
sliced = nb.concat_images(i for i, t in zip(nb.four_to_three(img), t_mask, strict=False) if t)
data = sliced.get_fdata(dtype="float32")
data = np.clip(
    data,
    a_min=0.0 if nonneg else np.percentile(data, 0.2),
    a_max=np.percentile(data, 99.8),
)
gs_drift = np.mean(data, axis=(0, 1, 2))
gs_drift /= gs_drift.max()
data /= gs_drift
data = np.clip(data, a_min=0.0 if nonneg else data.min(), a_max=data.max())
sliced.__class__(data, sliced.affine, sliced.header).to_filename(out)
"""


def _sliced(label: str) -> Path:
    """RobustAverage's ``_sliced.nii.gz`` (niworkflows' ``interfaces/images.py``), re-made from
    the oracle run's node by fMRIPrep's own Python (:data:`SLICED_SCRIPT`, in its image, through
    the oracle cache): the volumes of ``t_mask``, clipped to the 0.2 (or 0) and 99.8
    percentiles, divided by the global signal's drift, clipped again, written with the input's
    header (its extensions cleared): the input fMRIPrep gave 3dvolreg."""
    from larmorx_validation import oracle_cache

    d = inputs_dir() / "fmriprep"
    out = d / f"{label}_sliced.nii.gz"
    if out.is_file():
        return out
    image = ar.fmriprep_image()
    if image is None:
        raise FileNotFoundError("fMRIPrep 25.2.6's image (or apptainer)")
    with _locked(d):
        if out.is_file():
            return out
        src, t_mask, nonneg = _node_inputs(_node(label))
        script = d / "robust_average_sliced.py"
        if not script.is_file() or script.read_text(encoding="utf-8") != SLICED_SCRIPT:
            script.write_text(SLICED_SCRIPT, encoding="utf-8")
        work = d / f"make-{label}"
        shutil.rmtree(work, ignore_errors=True)
        work.mkdir()
        mask = d / f"{label}_t_mask.txt"
        mask.write_text(
            ("all" if t_mask is None else "".join("1" if t else "0" for t in t_mask)) + "\n",
            encoding="utf-8",
        )
        args = [str(script), str(src), str(mask), "1" if nonneg else "0", "sliced.nii.gz"]
        cmd = ["apptainer", "exec", "--cleanenv", "--no-home", "--pwd", str(work)]
        for b in sorted({str(work), str(d), str(src.parent)}):
            cmd += ["--bind", f"{b}:{b}"]
        p = oracle_cache.run_apptainer(
            image, "python", args, {}, [*cmd, str(image), "python", *args], cwd=work,
            policy=oracle_cache.Policy(threads=1), runner="fmriprep-python",
        )  # fmt: skip
        if p.returncode != 0 or not (work / "sliced.nii.gz").is_file():
            raise RuntimeError(f"{label}: RobustAverage's slicing failed: {p.stderr[-400:]}")
        (work / "sliced.nii.gz").replace(out)
        shutil.rmtree(work)
    return out


#: The file :func:`_average` writes: RobustAverage's ``out_file``, from the registered series.
AVERAGE = "average.nii.gz"


def _average(s: ar.Scenario, d: Path) -> None:
    """RobustAverage's last step on a run's registered series (``OGZ``): its values clipped
    (to 0 below: ``nonnegative``) and their median over the volumes, written with the sliced
    input's header (``sliced.__class__(np.median(data, axis=3), sliced.affine,
    sliced.header)``), as ``AVERAGE``; only for the fMRIPrep cases."""
    import nibabel as nib

    name = s.inputs[0]
    if not name.startswith("fmriprep:") or not (d / OGZ).is_file():
        return
    label = name.removeprefix("fmriprep:")
    _, _, nonneg = _node_inputs(_node(label))
    sliced = nib.load(_sliced(label))
    data = nib.load(d / OGZ).get_fdata(dtype="float32")
    data = np.clip(data, a_min=0.0 if nonneg else data.min(), a_max=data.max())
    sliced.__class__(np.median(data, axis=3), sliced.affine, sliced.header).to_filename(d / AVERAGE)


def _fmriprep_average(label: str) -> Path:
    found = sorted(_node(label).glob("*_average.nii.gz"))
    if not found:
        raise FileNotFoundError(f"{label}: the node's average")
    return found[0]


def _input(name: str) -> Path:
    """An input: a derived file's name, ``fmriprep:<label>`` (RobustAverage's sliced
    volumes), ``fmriprep-average:<label>`` (fMRIPrep's own output of the node), ``blank:<name>``
    (a copy in a directory whose name holds a blank) or ``dt:<key>`` (random testing's)."""
    if name.startswith("fmriprep:"):
        return _sliced(name.removeprefix("fmriprep:"))
    if name.startswith("fmriprep-average:"):
        return _fmriprep_average(name.removeprefix("fmriprep-average:"))
    if name.startswith("blank:"):
        src = inputs_dir() / name.removeprefix("blank:")
        d = inputs_dir() / "with blank"
        d.mkdir(exist_ok=True)
        target = d / src.name
        if not target.exists():
            target.write_bytes(src.read_bytes())
        return target
    path = inputs_dir() / name
    if not path.is_file():
        raise FileNotFoundError(path)
    return path


# ------------------------------------------------------------------------------------------------
# Cases


def _scenarios() -> list[tuple[str, str, str, ar.Scenario]]:
    # fmt: off
    out: list[tuple[str, str, str, ar.Scenario]] = []

    def add(cid: str, category: str, desc: str, s: ar.Scenario) -> None:
        out.append((f"{category}/{cid}", category, desc, s))

    s1 = ("s1.nii.gz",)
    p, pg = ("-prefix", OUT), ("-prefix", OGZ)
    P1, M1, D1, MD = "p.1D", "m.aff12.1D", "d.txt", "md.1D"
    one = (*p, "-1Dfile", P1)

    # --- Interpolation methods and the final resampling ------------------------------------
    add("default", "interpolation", "the defaults (heptic), every text output",
        S((*p, "-1Dfile", P1, "-dfile", D1, "-1Dmatrix_save", M1, "-maxdisp1D", MD, "{0}"), s1,
          (OUT, P1, D1, M1, MD, "md_delt.1D"),
          **_py({"registered": OUT, "parameters": P1, "matrices": M1, "max_displacement": MD})))
    for method, tier in (("Fourier", "smoke"), ("cubic", "smoke"), ("quintic", "standard"), ("linear", "standard"),
                         ("heptic", "standard")):
        add(method, "interpolation", f"-{method}",
            S((f"-{method}", *one, "-dfile", D1, "{0}"), s1, (OUT, P1, D1), tier=tier,
              **(_py({"registered": OUT, "parameters": P1}, interpolation=method) if method in ("Fourier", "cubic") else {})))
    add("fourier-lowercase", "interpolation", "-fourier: AFNI compares four letters", S(("-fourier", *one, "{0}"), s1, (OUT, P1), tier="standard"))
    for final, tier in (("NN", "smoke"), ("Fourier", "standard"), ("cubic", "standard"), ("linear", "standard"),
                        ("quintic", "standard"), ("heptic", "standard")):
        add(f"final-{final}", "interpolation", f"-final {final}",
            S(("-final", final, *one, "{0}"), s1, (OUT, P1), tier=tier,
              **(_py({"registered": OUT}, final=final) if final == "NN" else {})))
    add("final-cubic-noclip", "interpolation", "-final cubic -noclip", S(("-final", "cubic", "-noclip", *p, "{0}"), s1, (OUT,),
                                                                         **_py(final="cubic", clip=False)))
    add("Fourier-noclip", "interpolation", "-Fourier -noclip", S(("-Fourier", "-noclip", *p, "{0}"), s1, (OUT,), tier="standard"))
    add("clipit", "interpolation", "-clipit (the default, a notice), -wtrim, -coarserot", S(("-clipit", "-wtrim", "-coarserot", *p, "{0}"), s1, (OUT,), tier="standard"))
    for z, tier in (("0", "smoke"), ("2", "standard"), ("8", "standard")):
        add(f"zpad-{z}", "interpolation", f"-zpad {z}",
            S(("-zpad", z, *one, "{0}"), s1, (OUT, P1), tier=tier, **(_py({"registered": OUT}, zpad=0) if z == "0" else {})))
    add("zpad-twice", "interpolation", "-zpad 3 -zpad 2: the last one", S(("-zpad", "3", "-zpad", "2", *p, "{0}"), s1, (OUT,), tier="standard"))
    add("Fourier-zpad-0", "interpolation", "-Fourier -zpad 0", S(("-Fourier", "-zpad", "0", *one, "{0}"), s1, (OUT, P1), tier="standard"))

    # --- The base -------------------------------------------------------------------------------
    add("index", "base", "-base 3", S(("-base", "3", *p, "-1Dmatrix_save", M1, "{0}"), s1, (OUT, M1), **_py({"registered": OUT, "matrices": M1}, base=3)))
    add("last", "base", "-base 5 (the last volume)", S(("-base", "5", *one, "{0}"), s1, (OUT, P1), tier="standard"))
    add("dataset", "base", "-base <3D dataset>", S(("-base", "{1}", *p, "-1Dmatrix_save", M1, "-dfile", D1, "{0}"), ("s1.nii.gz", "base3d.nii.gz"), (OUT, M1, D1)))
    add("dataset-twopass", "base", "-twopass -base <3D dataset>", S(("-twopass", "-base", "{1}", *p, "-1Dmatrix_save", M1, "{0}"), ("s1.nii.gz", "base3d.nii.gz"), (OUT, M1), tier="standard"))
    add("dataset-4d", "base", "-base <4D dataset>: its first volume", S(("-base", "{1}", *one, "{0}"), ("s1.nii.gz", "s2_short.nii.gz"), (OUT, P1), tier="standard"))
    add("3d-input", "base", "a 3D input with an external base", S(("-tshift", "0", "-base", "{1}", *p, "{0}"), ("base3d.nii.gz", "s1.nii.gz"), (OUT,), tier="standard"))
    add("vol3d-input", "base", "a 3D input, the base a moving series", S(("-base", "{1}", *one, "{0}"), ("e_vol3d.nii.gz", "s1.nii.gz"), (OUT, P1), tier="standard"))

    # --- -twopass ---------------------------------------------------------------------------------
    add("twopass", "twopass", "-twopass", S(("-twopass", *one, "{0}"), s1, (OUT, P1), **_py({"registered": OUT, "parameters": P1}, twopass=True)))
    add("twoblur-3", "twopass", "-twopass -twoblur 3", S(("-twopass", "-twoblur", "3", *one, "{0}"), s1, (OUT, P1), tier="standard",
                                                        **_py({"parameters": P1}, twopass=True, twoblur=3.0)))
    add("coarse-1-1", "twopass", "-twopass -coarse 1 1", S(("-twopass", "-coarse", "1", "1", *one, "{0}"), s1, (OUT, P1), tier="standard"))
    add("coarse-0-0", "twopass", "-twopass -coarse 0 0", S(("-twopass", "-coarse", "0", "0", *one, "{0}"), s1, (OUT, P1), tier="standard"))
    add("sinit", "twopass", "-twopass -sinit", S(("-twopass", "-sinit", *one, "{0}"), s1, (OUT, P1), tier="standard",
                                                 **_py({"parameters": P1}, twopass=True, sinit=False)))
    add("nomaxdisp", "twopass", "-twopass -nomaxdisp: AFNI-92's clip fraction", S(("-twopass", "-nomaxdisp", *one, "{0}"), s1, (OUT, P1),
                                                                                   silent_ok=NOMAXDISP))
    add("nocoarserot", "twopass", "-nocoarserot -twopass: AFNI reads -noclip (AFNI-86)", S(("-nocoarserot", "-twopass", *one, "{0}"), s1, (OUT, P1)))
    add("twopass-weight", "twopass", "-twopass -weight", S(("-twopass", "-weight", "{1}[0]", *one, "{0}"), ("s1.nii.gz", "weight.nii.gz"), (OUT, P1), tier="standard"))
    add("twopass-wtinp", "twopass", "-twopass -wtinp", S(("-twopass", "-wtinp", *one, "{0}"), s1, (OUT, P1), tier="standard"))
    add("twopass-maxite-2", "twopass", "-twopass -maxite 2: raised to 66", S(("-twopass", "-maxite", "2", *one, "{0}"), s1, (OUT, P1), tier="standard"))
    add("fmriprep-call", "twopass", "fMRIPrep's call on the synthetic series",
        S(("-Fourier", "-twopass", "-1Dfile", "o.1D", "-1Dmatrix_save", "o.aff12.1D", *pg, "-zpad", "4", "-maxdisp1D", "o_md.1D", "{0}"), s1,
          (OGZ, "o.1D", "o.aff12.1D", "o_md.1D", "o_md_delt.1D"),
          **_py({"registered": OGZ, "parameters": "o.1D", "matrices": "o.aff12.1D", "max_displacement": "o_md.1D"},
                interpolation="Fourier", twopass=True, zpad=4)))

    # --- Weights, edging, iteration controls --------------------------------------------------------
    add("wtinp", "weights", "-wtinp", S(("-wtinp", *one, "{0}"), s1, (OUT, P1), **_py({"parameters": P1}, weight_input=True)))
    add("weight", "weights", "-weight 'wset[1]'", S(("-weight", "{1}[1]", *one, "{0}"), ("s1.nii.gz", "weight.nii.gz"), (OUT, P1),
                                                    **_py({"parameters": P1}, weight="{1}", weight_index=1)))
    add("edging-3", "weights", "-edging 3", S(("-edging", "3", *one, "{0}"), s1, (OUT, P1), tier="standard",
                                              **_py({"parameters": P1}, edging=3.0)))
    add("edging-10pc", "weights", "-edging 10%", S(("-edging", "10%", *one, "{0}"), s1, (OUT, P1), tier="standard"))
    add("edging-0", "weights", "-edging 0", S(("-edging", "0", *one, "{0}"), s1, (OUT, P1), tier="standard"))
    add("controls", "weights", "-maxite 2 -x_thresh 0.1 -rot_thresh 0.1 -delta 1",
        S(("-maxite", "2", "-x_thresh", "0.1", "-rot_thresh", "0.1", "-delta", "1", *one, "{0}"), s1, (OUT, P1),
          **_py({"parameters": P1}, max_iterations=2, x_threshold=0.1, rot_threshold=0.1, delta=1.0)))
    add("params", "weights", "-params 3 0.05 0.05 0.5", S(("-params", "3", "0.05", "0.05", "0.5", *one, "{0}"), s1, (OUT, P1), tier="standard"))
    add("controls-nonpositive", "weights", "-maxite 0 -delta -1: ignored", S(("-maxite", "0", "-delta", "-1", *one, "{0}"), s1, (OUT, P1), tier="standard"))
    add("edging-env", "environment", "AFNI_VOLREG_EDGING=10", S((*one, "{0}"), s1, (OUT, P1), env=(("AFNI_VOLREG_EDGING", "10"),), tier="standard"))
    add("edging-env-pc", "environment", "AFNI_VOLREG_EDGING=12%", S((*one, "{0}"), s1, (OUT, P1), env=(("AFNI_VOLREG_EDGING", "12%"),), tier="standard"))
    add("blur-firfac", "environment", "-twopass with AFNI_BLUR_FIRFAC=1.5", S(("-twopass", *one, "{0}"), s1, (OUT, P1), env=(("AFNI_BLUR_FIRFAC", "1.5"),), tier="standard"))
    add("blur-firold", "environment", "-twopass with AFNI_BLUR_FIROLD=YES", S(("-twopass", *one, "{0}"), s1, (OUT, P1), env=(("AFNI_BLUR_FIROLD", "YES"),), tier="standard"))
    add("rota-zpad", "environment", "AFNI_ROTA_ZPAD=7: not read", S((*one, "{0}"), s1, (OUT, P1), env=(("AFNI_ROTA_ZPAD", "7"),), tier="standard"))

    # --- Outputs --------------------------------------------------------------------------------------
    add("rotcom", "outputs", "-rotcom -prefix NULL", S(("-rotcom", "-prefix", "NULL", "-1Dfile", P1, "{0}"), s1, (P1,)))
    add("prefix-null", "outputs", "-prefix NULL with every text output",
        S(("-prefix", "NULL", "-dfile", D1, "-1Dmatrix_save", M1, "-maxdisp1D", MD, "{0}"), s1, (D1, M1, MD, "md_delt.1D"), tier="standard"))
    add("matrix-no-suffix", "outputs", "-1Dmatrix_save m: .aff12.1D added", S(("-1Dmatrix_save", "m", *p, "{0}"), s1, (OUT, M1), tier="standard"))
    add("maxdisp1D-stdout", "outputs", "-maxdisp1D -: standard output, and the file -_delt", S(("-maxdisp1D", "-", *p, "{0}"), s1, (OUT, "-_delt"), tier="standard"))
    add("float", "outputs", "-float on int16", S(("-float", *p, "{0}"), ("s2_short.nii.gz",), (OUT,), **_py(float_output=True)))
    add("nii-gz", "outputs", "a compressed output", S((*pg, "{0}"), s1, (OGZ,), tier="standard"))
    add("savedisp", "outputs", "-savedisp", S(("-savedisp", "sd.nii.gz", *p, "{0}"), s1, (OUT, "sd_DX.nii.gz", "sd_DY.nii.gz", "sd_DZ.nii.gz"),
                                              **_py({"registered": OUT}, save_displacements=True)))
    sd19 = tuple(f"SD{x}.nii" for x in ("_DX", "_DY", "_DZ", "_DXX", "_DYY", "_DZZ", "_DXY", "_DXZ", "_DYZ", "_DXXX", "_DYYY",
                                       "_DZZZ", "_DXXY", "_DXXZ", "_DXYY", "_DXZZ", "_DYYZ", "_DYZZ", "_DXYZ"))
    add("SAVEDISP", "outputs", "-SAVEDISP -verbose: 19 datasets", S(("-SAVEDISP", "SD.nii", *p, "-verbose", "{0}"), s1, (OUT, *sd19)))
    add("savedisp-oblique", "outputs", "-savedisp -prefix NULL on an oblique series",
        S(("-savedisp", "sd.nii.gz", "-prefix", "NULL", "{0}"), ("s3_oblique.nii.gz",), ("sd_DX.nii.gz", "sd_DY.nii.gz", "sd_DZ.nii.gz"), tier="standard"))
    add("savedisp-bare", "outputs", "-savedisp .nii", S(("-savedisp", ".nii", *p, "{0}"), s1, (OUT, "_DX.nii", "_DY.nii", "_DZ.nii"), tier="standard"))

    # --- -tshift ----------------------------------------------------------------------------------------
    add("tshift-0", "tshift", "-tshift 0 on sequential slices", S(("-tshift", "0", *one, "{0}"), ("s6_st.nii.gz",), (OUT, P1), **_py({"registered": OUT}, tshift=0)))
    add("tshift-1", "tshift", "-tshift 1", S(("-tshift", "1", *one, "{0}"), ("s6_st.nii.gz",), (OUT, P1), tier="standard"))
    add("tshift-verbose", "tshift", "-verbose -tshift 2 -base 1", S(("-verbose", "-tshift", "2", "-base", "1", *one, "{0}"), ("s6_st.nii.gz",), (OUT, P1), tier="standard"))
    add("tshift-twopass", "tshift", "-tshift 0 -twopass -Fourier", S(("-tshift", "0", "-twopass", "-Fourier", *p, "-dfile", D1, "{0}"), ("s6_st.nii.gz",), (OUT, D1), tier="standard"))
    add("tshift-negative-slope", "tshift", "-tshift 0 on int16 with a negative slope (AFNI-01) and alternating slices",
        S(("-tshift", "0", *one, "{0}"), ("s6_stneg.nii.gz",), (OUT, P1)))
    add("tshift-no-timing", "tshift", "-tshift 0 without slice times", S(("-tshift", "0", *p, "{0}"), s1, (OUT,), tier="standard"))
    add("tshift-ignore-large", "tshift", "-tshift 0 on 4 volumes: ignore too large", S(("-tshift", "0", *p, "{0}"), ("s6_st4.nii.gz",), (OUT,), tier="standard"))
    add("tshift-savedisp", "tshift", "-tshift 0 -savedisp", S(("-tshift", "0", "-savedisp", "sd.nii", *p, "{0}"), ("s6_st.nii.gz",), (OUT, "sd_DX.nii", "sd_DY.nii", "sd_DZ.nii"), tier="standard"))

    # --- -rotparent, -gridparent ----------------------------------------------------------------------
    rp = ("s1.nii.gz", "rp.nii.gz")
    add("rotparent", "rotparent", "-rotparent", S(("-rotparent", "{1}", *one, "-dfile", D1, "{0}"), rp, (OUT, P1, D1),
                                                  **_py({"registered": OUT}, rotparent="{1}")))
    add("rotparent-gridparent", "rotparent", "-verbose -rotparent -gridparent (27 slices)",
        S(("-verbose", "-rotparent", "{1}", "-gridparent", "{2}", *p, "{0}"), (*rp, "gp27.nii.gz"), (OUT,)))
    add("gridparent-24", "rotparent", "-rotparent -gridparent (24 slices) -Fourier -final cubic",
        S(("-rotparent", "{1}", "-gridparent", "{2}", *p, "-Fourier", "-final", "cubic", "{0}"), (*rp, "gp24.nii.gz"), (OUT,), tier="standard"))
    add("gridparent-moved", "rotparent", "-rotparent -gridparent (moved origin) -noclip -final linear",
        S(("-rotparent", "{1}", "-gridparent", "{2}", *p, "-noclip", "-final", "linear", "{0}"), (*rp, "gp27m.nii.gz"), (OUT,), tier="standard"))
    add("rotparent-slice-times", "rotparent", "-verbose -rotparent -gridparent on slice-timed data",
        S(("-verbose", "-rotparent", "{1}", "-gridparent", "{2}", *p, "{0}"), ("s6_st.nii.gz", "rp_st.nii.gz", "gp27m.nii.gz"), (OUT,), tier="standard"))
    add("rotparent-tshift", "rotparent", "-tshift 0 -rotparent", S(("-tshift", "0", "-rotparent", "{1}", *p, "{0}"), ("s6_st.nii.gz", "rp_st.nii.gz"), (OUT,), tier="standard"))
    for g, desc in (("gp22.nii.gz", "too few slices"), ("gpx.nii.gz", "another x size"), ("gpsp.nii.gz", "another spacing")):
        add(f"gridparent-{g.split('.')[0]}", "rotparent", f"-rotparent -gridparent with {desc}",
            S(("-rotparent", "{1}", "-gridparent", "{2}", *p, "{0}"), (*rp, g), (OUT,), tier="standard"))
    add("gridparent-alone", "rotparent", "-gridparent without -rotparent", S(("-gridparent", "{1}", *one, "{0}"), ("s1.nii.gz", "gp27.nii.gz"), (OUT, P1), tier="standard"))
    add("rotparent-twice", "rotparent", "-rotparent twice", S(("-rotparent", "{1}", "-rotparent", "{1}", *p, "{0}"), rp, (OUT,), tier="standard"))
    add("gridparent-twice", "rotparent", "-gridparent twice", S(("-gridparent", "{1}", "-gridparent", "{1}", *p, "{0}"), ("s1.nii.gz", "gp27.nii.gz"), (OUT,), tier="standard"))
    add("rotparent-missing", "rotparent", "-rotparent of a missing file", S(("-rotparent", "nothere.nii.gz", *p, "{0}"), s1, (OUT,), tier="standard"))
    add("rotparent-twopass", "rotparent", "-rotparent -twopass", S(("-rotparent", "{1}", "-twopass", *one, "{0}"), rp, (OUT, P1), tier="standard"))
    add("rotparent-oblique", "rotparent", "-rotparent on an oblique series", S(("-rotparent", "{1}", *p, "-1Dmatrix_save", M1, "{0}"),
                                                                              ("s3_oblique.nii.gz", "rp_obl.nii.gz"), (OUT, M1), tier="standard"))
    add("rotparent-twodup-grid", "rotparent", "-rotparent -twopass -twodup -base <shifted> -gridparent",
        S(("-rotparent", "{1}", "-twopass", "-twodup", "-base", "{2}", "-gridparent", "{3}", *p, "{0}"), (*rp, "base_shift.nii.gz", "gp27.nii.gz"), (OUT,), tier="standard"))
    add("rotparent-twodup", "rotparent", "-rotparent -twopass -twodup -base <shifted>",
        S(("-rotparent", "{1}", "-twopass", "-twodup", "-base", "{2}", *p, "{0}"), (*rp, "base_shift.nii.gz"), (OUT,), tier="standard"))
    add("rotparent-savedisp", "rotparent", "-rotparent -savedisp -gridparent",
        S(("-rotparent", "{1}", "-savedisp", "sd.nii.gz", "-gridparent", "{2}", *p, "{0}"), (*rp, "gp27.nii.gz"), (OUT, "sd_DX.nii.gz", "sd_DY.nii.gz", "sd_DZ.nii.gz"), tier="standard"))
    add("rotparent-int16", "rotparent", "-rotparent on int16 (LPI)", S(("-rotparent", "{1}", *p, "{0}"), ("s2_short.nii.gz", "rp.nii.gz"), (OUT,), tier="standard"))
    add("rotparent-without-attributes", "rotparent", "-rotparent of a dataset without VOLREG attributes: AFNI's error",
        S(("-rotparent", "{1}", *p, "{0}"), ("s1.nii.gz", "s1.nii.gz"), (OUT,), expect="error", tier="standard"))

    # --- -twodup --------------------------------------------------------------------------------------------
    add("twodup", "twodup", "-twopass -twodup -base <shifted>", S(("-twopass", "-twodup", "-base", "{1}", *p, "-1Dmatrix_save", M1, "{0}"),
                                                                  ("s1.nii.gz", "base_shift.nii.gz"), (OUT, M1), **_py({"registered": OUT}, twopass=True, twodup=True, base="{1}")))
    add("twodup-onepass", "twodup", "-twodup without -twopass", S(("-twodup", "-base", "{1}", *p, "{0}"), ("s1.nii.gz", "base_shift.nii.gz"), (OUT,), tier="standard"))
    add("twodup-internal", "twodup", "-twopass -twodup, an internal base", S(("-twopass", "-twodup", *one, "{0}"), s1, (OUT, P1), tier="standard"))
    add("twodup-savedisp", "twodup", "-twopass -twodup -base <shifted> -savedisp on slice-timed data",
        S(("-twopass", "-twodup", "-base", "{1}", "-savedisp", "sd.nii.gz", *p, "{0}"), ("s6_st.nii.gz", "base_shift.nii.gz"),
          (OUT, "sd_DX.nii.gz", "sd_DY.nii.gz", "sd_DZ.nii.gz"), tier="standard"))

    # --- Messages -------------------------------------------------------------------------------------------
    add("verbose", "messages", "-verbose", S(("-verbose", *p, "{0}"), s1, (OUT,)))
    add("verbose-verbose", "messages", "-verbose -verbose -twopass -Fourier", S(("-verbose", "-verbose", "-twopass", "-Fourier", *p, "{0}"), s1, (OUT,)))
    add("base-before-verbose", "messages", "-base <dset> -verbose: messages in the scan's order", S(("-base", "{1}", "-verbose", *p, "{0}"), ("s1.nii.gz", "base3d.nii.gz"), (OUT,), tier="standard"))
    add("verbose-base-mismatch", "messages", "-verbose -base <another grid>", S(("-verbose", "-base", "{1}", *p, "{0}"), ("s1.nii.gz", "s2_short.nii.gz"), (OUT,), tier="standard"))
    add("verbose-weight", "messages", "-verbose -weight -base 2", S(("-verbose", "-weight", "{1}[1]", "-base", "2", *p, "{0}"), ("s1.nii.gz", "weight.nii.gz"), (OUT,), tier="standard"))
    add("verbose-names", "messages", "-verbose with a base and a weight: their names", S(("-verbose", "-base", "{1}", "-weight", "{2}[0]", *p, "{0}"),
                                                                                         ("s1.nii.gz", "base3d.nii.gz", "weight.nii.gz"), (OUT,), tier="standard"))
    add("verbose-byte", "messages", "-verbose on bytes", S(("-verbose", *one, "{0}"), ("s5_byte.nii.gz",), (OUT, P1), tier="standard"))
    add("verbose-tshift-base", "messages", "-verbose -tshift 0 -base <a 4D weight>", S(("-verbose", "-tshift", "0", "-base", "{1}", *p, "{0}"),
                                                                                    ("s6_st.nii.gz", "weight.nii.gz"), (OUT,), tier="standard"))
    add("verbose-3d-input", "messages", "-verbose, a 3D input and a 4D base", S(("-verbose", "-base", "{1}", *p, "{0}"), ("base3d.nii.gz", "s1.nii.gz"), (OUT,), tier="standard"))
    add("weight-index-beyond", "messages", "-weight 'wset[5]': beyond its volumes", S(("-weight", "{1}[5]", *p, "{0}"), ("s1.nii.gz", "weight.nii.gz"), (OUT,), tier="standard"))

    # --- Storage types and geometry --------------------------------------------------------------------
    for img, desc, tier in (
        ("s2_short.nii.gz", "int16, LPI", "smoke"),
        ("s2_scaled.nii.gz", "int16 with a slope: written as float", "standard"),
        ("s3_oblique.nii.gz", "oblique, odd sizes", "smoke"),
        ("s4_sagittal.nii.gz", "sagittal", "standard"),
        ("e_nifti2.nii", "NIfTI-2", "standard"),
        ("e_plain.nii", "uncompressed NIfTI-1", "standard"),
        ("e_qform.nii.gz", "a qform only", "standard"),
        ("e_bigendian.nii", "int16, big-endian", "standard"),
    ):
        args = ("-Fourier", "-twopass", "-1Dmatrix_save", M1, "-maxdisp1D", MD) if img in ("s3_oblique.nii.gz", "s4_sagittal.nii.gz") else ()
        outs = (M1, MD, "md_delt.1D") if args else ()
        add(img.split(".")[0], "datatypes", desc, S((*args, *one, "-dfile", D1, "{0}"), (img,), (OUT, P1, D1, *outs), tier=tier))
    add("byte", "datatypes", "bytes (AFNI-87)", S((*one, "{0}"), ("s5_byte.nii.gz",), (OUT, P1)))
    add("input-option", "datatypes", "-input, and a dataset name left over", S(("-input", "{0}", *p, "extra", "{0}"), s1, (OUT,), tier="standard"))

    # --- Edge cases -------------------------------------------------------------------------------------
    add("one-volume", "edge cases", "a 4D file of one volume", S((*one, "-dfile", D1, "{0}"), ("e_one.nii.gz",), (OUT, P1, D1)))
    add("one-volume-twopass", "edge cases", "one volume, -twopass", S(("-twopass", *one, "{0}"), ("e_one.nii.gz",), (OUT, P1), tier="standard"))
    add("no-motion", "edge cases", "four identical volumes", S((*one, "-maxdisp1D", MD, "{0}"), ("e_still.nii.gz",), (OUT, P1, MD, "md_delt.1D")))
    add("no-motion-fmriprep", "edge cases", "four identical volumes, fMRIPrep's call",
        S(("-Fourier", "-twopass", "-zpad", "4", *one, "-1Dmatrix_save", M1, "{0}"), ("e_still.nii.gz",), (OUT, P1, M1), tier="standard"))
    for img, tier in (("e_large_z.nii.gz", "smoke"), ("e_large_x.nii.gz", "standard")):
        n = img.split(".")[0].removeprefix("e_")
        add(f"{n}", "edge cases", f"rotations of {'12' if n.endswith('z') else '9'} degrees per volume about {n[-1]}, -twopass -Fourier",
            S(("-twopass", "-Fourier", *one, "-1Dmatrix_save", M1, "{0}"), (img,), (OUT, P1, M1), tier=tier))
        add(f"{n}-onepass", "edge cases", "the same, one pass", S((*one, "{0}"), (img,), (OUT, P1), tier="standard"))
        add(f"{n}-nocoarserot", "edge cases", "the same, -twopass -nocoarserot (AFNI-86)", S(("-twopass", "-nocoarserot", *one, "{0}"), (img,), (OUT, P1), tier="standard"))
    add("nan", "edge cases", "NaN in volume 2", S((*one, "-dfile", D1, "{0}"), ("e_nan.nii.gz",), (OUT, P1, D1)))
    add("nan-twopass", "edge cases", "NaN in volume 2, fMRIPrep's call", S(("-Fourier", "-twopass", "-zpad", "4", *one, "{0}"), ("e_nan.nii.gz",), (OUT, P1), tier="standard"))
    add("nan-base", "edge cases", "NaN in the base (-base 2)", S(("-base", "2", *one, "{0}"), ("e_nan.nii.gz",), (OUT, P1), tier="standard"))
    add("two-slices", "edge cases", "two slices", S((*one, "{0}"), ("e_nz2.nii.gz",), (OUT, P1), tier="standard"))
    add("one-slice", "edge cases", "one slice", S((*p, "{0}"), ("s7_nz1.nii.gz",), (OUT,), tier="standard"))
    add("one-slice-twopass", "edge cases", "one slice, -twopass", S(("-twopass", *p, "{0}"), ("s7_nz1.nii.gz",), (OUT,), tier="standard"))
    add("subsampled", "edge cases", "120 x 120 x 120 voxels, -twopass: the coarse search blurred and subsampled",
        S(("-twopass", *one, "-1Dmatrix_save", M1, "{0}"), ("e_big120.nii.gz",), (OUT, P1, M1), tier="standard"))
    add("subsampled-fmriprep", "edge cases", "120 x 120 x 120 voxels, fMRIPrep's call",
        S(("-Fourier", "-twopass", "-zpad", "4", *one, "{0}"), ("e_big120.nii.gz",), (OUT, P1), tier="standard"))
    add("subsampled-coarse-2", "edge cases", "120 x 120 x 120 voxels, -twopass -coarse 2 1: the subsampled step 1",
        S(("-twopass", "-coarse", "2", "1", *one, "{0}"), ("e_big120.nii.gz",), (OUT, P1), tier="standard"))
    add("four-slices-twopass", "edge cases", "four slices, -twopass: an empty coarse region (AFNI-93)", S(("-twopass", *one, "{0}"), ("s7_nz4.nii.gz",), (OUT, P1)))
    add("four-slices-nocoarserot", "edge cases", "four slices, -twopass -nocoarserot", S(("-twopass", "-nocoarserot", *one, "{0}"), ("s7_nz4.nii.gz",), (OUT, P1), tier="standard"))
    add("constant", "edge cases", "a constant series: one pass registers it", S((*p, "{0}"), ("s7_const.nii.gz",), (OUT,)))
    add("constant-twopass", "edge cases", "a constant series, -twopass (AFNI-89)", S(("-twopass", *p, "{0}"), ("s7_const.nii.gz",), (OUT,), expect="crash", tier="standard"))

    # --- Errors ------------------------------------------------------------------------------------------
    for cid, args, expect in (
        ("unknown-option", ("-bogus", "{0}"), "error"),
        ("final-bogus", ("-final", "bogus", "{0}"), "error"),
        ("twoblur-low", ("-twoblur", "1", "{0}"), "error"),
        ("zpad-negative", ("-zpad", "-2", "{0}"), "error"),
        ("base-beyond", ("-base", "99", "{0}"), "error"),
        ("base-negative", ("-base", "-1", "{0}"), "error"),
        ("edging-negative", ("-edging", "-1", "{0}"), "error"),
        ("edging-30pc", ("-edging", "30%", "{0}"), "error"),
        ("weight-no-selector", ("-weight", "{1}", "{0}"), "error"),
        ("weight-missing", ("-weight", "nothere.nii.gz[0]", *p, "{0}"), "error"),
        ("base-missing", ("-base", "nothere.nii.gz", *p, "{0}"), "error"),
        ("tshift-negative", ("-tshift", "-1", "{0}"), "error"),
        ("no-dataset", (*p,), "error"),
        ("missing-input", (*p, "/nonexistent/x.nii"), "error"),
        ("base-twice", ("-base", "0", "-base", "3", *p, "{0}"), "pass"),
        ("prefix-after-dataset", (*p, "{0}", "-prefix"), "error"),
        ("maxite-last", ("-maxite",), "crash"),
        ("input-last", ("-input",), "error"),
        ("rotparent-last", ("-rotparent",), "error"),
        ("gridparent-last", ("-gridparent",), "error"),
        ("matrix-save-last", ("-1Dmatrix_save",), "error"),
        ("one-volume-no-base", ("-base", "1", *p, "{3}"), "error"),
    ):
        kw: dict = {}
        if cid == "base-twice":
            kw["fix_refuses"] = "AFNI-95"
        ins = ("s1.nii.gz", "weight.nii.gz", "s1.nii.gz", "e_one.nii.gz") if cid in ("weight-no-selector", "one-volume-no-base") else s1
        add(cid, "errors", " ".join(args).replace("{0}", "<input>").replace("{1}", "<weight>").replace("{3}", "<one volume>"),
            S(args, ins, (OUT,), expect=expect, tier="smoke" if cid in ("unknown-option", "maxite-last") else "standard", **kw))
    add("default-prefix", "divergences", "no -prefix: AFNI writes its own format (volreg+orig)",
        S(("{0}",), s1, ("volreg+orig.HEAD",), expect="divergence", tier="standard",
          reason="AFNI's default prefix 'volreg' is its own dataset format, which larmorx does not write: it refuses (NIfTI only)."))
    add("savedisp-afni-format", "divergences", "-savedisp sd: AFNI's own format",
        S(("-savedisp", "sd", *p, "{0}"), s1, (OUT,), expect="divergence", tier="standard",
          reason="-savedisp without .nii/.nii.gz writes AFNI's own dataset format, which larmorx does not write: it refuses (NIfTI only)."))

    # --- AFNI's global options ---------------------------------------------------------------------------
    taken = ((OUT, "taken"),)
    add("exists", "global", "an existing output (AFNI-11)", S((*p, "{0}"), s1, (OUT,), files=taken, fix_refuses="AFNI-11"))
    add("overwrite", "global", "-overwrite", S(("-overwrite", *p, "{0}"), s1, (OUT,), files=taken, tier="standard"))
    add("deconflict-yes", "global", "AFNI_DECONFLICT=YES: renamed", S((*p, "{0}"), s1, (OUT, "o_AA1.nii"), files=taken,
                                                                     env=(("AFNI_DECONFLICT", "YES"),), tier="standard"))
    add("D-write-type-2", "global", "-DAFNI_NIFTI_WRITE_TYPE=2", S(("-DAFNI_NIFTI_WRITE_TYPE=2", *p, "{0}"), s1, (OUT,), tier="standard"))
    add("quiet-startup", "environment", "AFNI_QUIET_STARTUP=YES", S((*p, "{0}"), s1, (OUT,), env=(("AFNI_QUIET_STARTUP", "YES"),), tier="standard"))
    add("prefix-with-blank", "global", "a -prefix with a blank: AFNI writes nothing, with exit status 0 (AFNI-12)",
        S(("-prefix", "a b.nii", "{0}"), s1, ("a b.nii",), tier="standard"))
    add("savedisp-with-blank", "global", "-savedisp with a blank (AFNI-12)",
        S(("-savedisp", "s d.nii", *p, "{0}"), s1, (OUT, "s d_DX.nii", "s d_DY.nii", "s d_DZ.nii"), tier="standard"))
    add("input-dir-with-blank", "global", "an input in a directory with a blank (AFNI-18)",
        S((*p, "{0}"), ("blank:s1.nii.gz",), (OUT,), tier="standard", fix_writes="AFNI-18"))
    add("missing-directory", "global", "an output in a missing directory (AFNI-26)",
        S(("-prefix", "nodir/o.nii", "{0}"), s1, ("nodir/o.nii",), tier="standard", fix_refuses="AFNI-26"))
    # AFNI-90: a -prefix of 255 bytes fits AFNI's buffer, 256 overflow it (a directory of 100
    # bytes and a name: no path component beyond the system's 255).
    folder = "D" * 100
    made = ((f"{folder}/.keep", ""),)
    long = f"{folder}/" + "L" * 150 + ".nii"
    add("prefix-255", "defects", "a -prefix of 255 bytes", S(("-prefix", long, "{0}"), s1, (long,), files=made, tier="standard"))
    longer = f"{folder}/" + "L" * 151 + ".nii"
    add("prefix-256", "defects", "a -prefix of 256 bytes: AFNI aborts (AFNI-90)",
        S(("-prefix", longer, "{0}"), s1, (longer,), files=made, expect="crash", fix_writes="AFNI-90", tier="standard"))

    # --- fMRIPrep's calls ---------------------------------------------------------------------------------
    for label, (_, _, tier, what) in CALLS.items():
        pepolar = label.startswith("pepolar-")
        call = ("-Fourier", "-twopass", "-zpad", "4", *pg, "-1Dfile", "o.1D", "-1Dmatrix_save", "o.aff12.1D", "-maxdisp1D", "o_md.1D", "{0}")
        if pepolar:
            # sdcflows' ref_average: RobustAverage with its own default outputs (the same call).
            pass
        add(label, "fmriprep", f"fMRIPrep's RobustAverage call ({what})",
            S(call, (f"fmriprep:{label}",), (OGZ, "o.1D", "o.aff12.1D", "o_md.1D", "o_md_delt.1D", AVERAGE), tier=tier,
              fmriprep_build=True, fmriprep=((AVERAGE, f"fmriprep-average:{label}"),),
              **(_py({"registered": OGZ, "parameters": "o.1D", "matrices": "o.aff12.1D"}, interpolation="Fourier", twopass=True, zpad=4)
                 if label == "ds000005-run-01" else {})))
    # fmt: on
    return out


@functools.cache
def _scenario_list() -> list[tuple[str, str, str, ar.Scenario]]:
    return _scenarios()


#: The implementations this suite validates (no clean-room original yet).
IMPLEMENTATIONS = ("replica",)


def spec(implementation: str = "replica") -> ar.ToolSpec:
    if implementation not in IMPLEMENTATIONS:
        raise ValueError(
            f"3dvolreg has no clean-room original yet: implementation must be one of "
            f"{IMPLEMENTATIONS}, not {implementation!r}"
        )
    return ar.ToolSpec(
        PROGRAM,
        "volreg",
        _scenario_list,
        _input,
        implementation,
        derive=_average,
        every_file=True,
        unnamed_ok=d13.UNNAMED_GLIBC,
    )


def cases(tier: str) -> list[Case]:
    return ar.cases_of(spec(), tier)


def _guard(case: Case, checks: CheckList) -> None:
    """AFNI-94's case (``-coarse 1`` on a volume of 120³ voxels or more never ends in AFNI):
    larmorx refuses in both modes, naming the defect, and writes nothing; AFNI is not run."""
    import tempfile

    s: ar.Scenario = case.payload
    with tempfile.TemporaryDirectory(prefix="lx-volreg-guard_") as t:
        args = ar._arguments(s, [_input(i) for i in s.inputs])
        env = ar.environment(s.env)
        for mode in (d13.REPRODUCE, d13.FIX):
            d = Path(t) / mode
            ar._setup(d, s)
            r = ar.run_larmorx(PROGRAM, args, mode, env, d, "replica")
            text = r.err.replace(str(t), "<tmp>")
            checks(
                f"larmorx refuses ({mode}), naming AFNI-94",
                r.code == 1 and "AFNI-94" in text,
                detail=f"exit {r.code}: {ar.last_error(text)}",
            )
            written = ar._produced(d, s)
            checks(f"nothing written ({mode})", not written, detail=f"{written}")


def _guard_cases() -> list[Case]:
    s = S(("-twopass", "-coarse", "1", "1", "-prefix", OUT, "{0}"), ("e_big120.nii.gz",), (OUT,))
    return [
        Case(
            "defects/coarse-1-large",
            "defects",
            "-twopass -coarse 1 1 on 120 x 120 x 120 voxels: AFNI never ends (AFNI-94); larmorx "
            "refuses in both modes (AFNI is not run)",
            s,
        )
    ]


def suite(implementation: str = "replica") -> Suite:
    """The 3dvolreg suite for the bit-exact replica (``larmorx afni 3dvolreg``, in process),
    through larmorx's command line and its Python API."""
    tool_spec = spec(implementation)

    def run_case(case: Case, checks: CheckList) -> None:
        if case.id == "defects/coarse-1-large":
            _guard(case, checks)
            return
        ar.run_case(tool_spec, case, checks)

    def suite_cases(tier: str) -> list[Case]:
        rank = ar.TIERS.index(tier)
        guard = _guard_cases() if rank >= ar.TIERS.index("standard") else []
        return [*cases(tier), *guard]

    return Suite(
        name="afni-volreg-replica",
        title="3dvolreg (replica)",
        tool=(
            f"the bit-exact replica ({ar.replica_version()}: the translation of AFNI 26.3.05's "
            "source, Apache-2.0 with CC BY 4.0's attribution), in-process: `larmorx afni "
            "3dvolreg` with `LARMORX_IMPLEMENTATION=replica` and "
            '`lx.afni.volreg(..., implementation="replica")`'
        ),
        reference=(
            f"3dvolreg from AFNI ({ar.afni_build(PROGRAM, implementation)}; no OpenMP loop), "
            "the binary built by `scripts/build_afni_oracle.sh`"
        ),
        thresholds=[
            (
                "Exit status",
                "the same (both succeed, or both reject the arguments); where AFNI crashes, "
                "larmorx exits 1",
            ),
            (
                "Messages",
                "standard output and error byte for byte (AFNI's banner and compile date, "
                "larmorx's own lines and the elapsed time set aside)",
            ),
            (
                "Every output (the registered series, -1Dfile, -dfile, -1Dmatrix_save, "
                "-maxdisp1D, -savedisp, -SAVEDISP)",
                "the same bytes: NIfTI values bit for bit and header bytes; text files byte "
                "for byte; every file written compared",
            ),
        ],
        cases=suite_cases,
        run_case=run_case,
        packages=("nibabel", "scipy"),
        highlights=lambda results: ar.highlights(
            results,
            PROGRAM,
            "volreg",
            "an option given last without its value, AFNI-88; a base whose least squares "
            "cannot be set up, AFNI-89; a name of 256 bytes or more, AFNI-90; a rotparent "
            "without its centres, AFNI-96",
            "replica",
        ),
        notes=(
            "Both programs run in directories of their own on the same input files (made once "
            "in `oracles/inputs/afni-volreg/`), in the same environment (no AFNI variables of "
            "this process, `HOME` an empty directory, `AFNI_DONT_LOGFILE=YES`, "
            "`AFNI_NIFTI_NOEXT=YES`, `OMP_NUM_THREADS=1`: 3dvolreg has no OpenMP loop, plus "
            "each case's variables); outputs are compared as stored (data bytes, header bytes "
            "decompressed, text byte for byte). The replica is compared in `reproduce` mode; "
            "the fix pass runs its default (correctly rounded maths, which an iterative fit "
            "does not name: GLIBC-07). fMRIPrep's calls run on RobustAverage's input re-made "
            "from each oracle run's node, and the median of the registered volumes is compared "
            "with the node's own output; they also run fMRIPrep 25.2.6's own AFNI (its image). "
            + afni_versions.record_line(implementation)
        ),
    )
