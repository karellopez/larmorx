# SPDX-License-Identifier: Apache-2.0
"""Synthetic inputs for the 3dAutomask and 3dUnifize parity suites: phantoms that make each
step of the two programs visible, in every storage type, scaling and geometry AFNI reads.

A head phantom: background noise, a bright skull, CSF, grey and white matter, a dark ventricle
(a hole the automask fills), a thin neck (which peeling cuts) and a separate blob outside the
head (which clustering drops), under a smooth multiplicative shading (what 3dUnifize removes),
with bright vessels in the white matter (what its squash compresses). Everything is generated
from seeded random numbers (``numpy.random.default_rng``), written once per process to a
temporary directory; nothing is committed (CLAUDE.md: no image data in git).

Real data (the fMRIPrep oracle runs' node inputs, BIDS T1w and BOLD files) come from the
workspace's ``oracles/`` and ``larmorx-testdata``; see :func:`real`.
"""

from __future__ import annotations

import functools
from pathlib import Path

import numpy as np

from larmorx_validation.parity.afni_replica import FMRIPREP_RUNS, generated_dir, workspace

GRID = (64, 64, 40)
ZOOM = 3.0


def _coords(shape: tuple[int, int, int]) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    nx, ny, nz = shape
    x, y, z = np.meshgrid(
        (np.arange(nx) - (nx - 1) / 2) / (nx / 2),
        (np.arange(ny) - (ny - 1) / 2) / (ny / 2),
        (np.arange(nz) - (nz - 1) / 2) / (nz / 2),
        indexing="ij",
    )
    return x, y, z


@functools.cache
def head(contrast: str = "t1", shape: tuple[int, int, int] = GRID, seed: int = 7) -> np.ndarray:
    """The head phantom (float64): ``t1`` (white 1000 > grey 650 > CSF 250, bright skull) or
    ``t2`` (CSF 1400 > grey 700 > white 420, dark skull)."""
    rng = np.random.default_rng(seed)
    x, y, z = _coords(shape)
    r = np.sqrt(x**2 + y**2 + (1.15 * z) ** 2)
    levels = {"t1": (1000.0, 650.0, 250.0, 900.0), "t2": (420.0, 700.0, 1400.0, 150.0)}[contrast]
    wm, gm, csf, skull = levels
    v = np.full(shape, 15.0)
    v[r < 0.92] = skull
    v[r < 0.82] = csf
    v[r < 0.76] = gm
    v[r < 0.55] = wm
    # A ventricle: a dark hole in the white matter.
    v[(np.abs(x) < 0.12) & (np.abs(y + 0.05) < 0.18) & (np.abs(z) < 0.2)] = csf
    # A thin neck below the head, one voxel wide, and a separate blob outside it.
    neck = (np.abs(x) < 1.5 / shape[0]) & (np.abs(y) < 0.25) & (z < -0.75) & (r > 0.9)
    v[neck] = gm
    blob = ((x - 0.85) ** 2 + (y - 0.85) ** 2 + (z - 0.6) ** 2) < 0.012
    v[blob] = wm
    # Bright vessels in the white matter (3dUnifize squashes values above 1300).
    if contrast == "t1":
        vessels = (np.abs(x - 0.2) < 0.03) & (np.abs(y) < 0.4) & (r < 0.5)
        v[vessels] = 1800.0
    # Shading: a smooth bias field, 0.6 to 1.4.
    shade = 1.0 + 0.3 * x - 0.15 * y + 0.1 * z * z
    v = v * shade
    v = v + rng.normal(0.0, 0.03, shape) * v + np.abs(rng.normal(0.0, 8.0, shape))
    return np.clip(v, 0.0, None)


def _affine(shape: tuple[int, ...], zoom: float = ZOOM) -> np.ndarray:
    nx, ny, nz = shape[:3]
    a = np.diag([zoom, zoom, zoom, 1.0])
    a[:3, 3] = [-(nx - 1) * zoom / 2, -(ny - 1) * zoom / 2, -(nz - 1) * zoom / 2]
    return a


def _save(name: str, data: np.ndarray, affine: np.ndarray, dtype, **header) -> Path:
    import nibabel as nib

    path = generated_dir() / name
    img = nib.Nifti1Image(np.asarray(data).astype(dtype), affine)
    h = img.header
    h.set_data_dtype(dtype)
    slope, inter = header.pop("slope", None), header.pop("inter", None)
    if slope is not None:
        h.set_slope_inter(slope, inter if inter is not None else 0.0)
    for k, v in header.items():
        if k == "sform_code":
            img.set_sform(affine if v else None, v)
        elif k == "qform_code":
            img.set_qform(affine if v else None, v)
        elif k == "xyzt_units":
            h["xyzt_units"] = v
        else:
            h[k] = v
    if len(np.shape(data)) == 4:
        h["pixdim"][4] = 2.0
        h.set_xyzt_units("mm", "sec")
    nib.save(img, path)
    return path


def _series(n: int = 6, contrast: str = "t2") -> np.ndarray:
    rng = np.random.default_rng(11)
    base = head(contrast)
    return np.stack(
        [base * (1.0 + 0.01 * t) + rng.normal(0.0, 6.0, base.shape) for t in range(n)], axis=-1
    )


@functools.cache
def synthetic(name: str) -> Path:
    """The generated input ``name`` (see the code for the list)."""
    t1, t2 = head("t1"), head("t2")
    a = _affine(GRID)
    if name == "head-f32":
        return _save("head-f32.nii", t1, a, np.float32)
    if name == "head-t2-f32":
        return _save("head-t2-f32.nii", t2, a, np.float32)
    if name == "head-i16":
        return _save("head-i16.nii.gz", np.round(t1), a, np.int16)
    if name == "head-u8":
        return _save("head-u8.nii", np.round(t1 * 255 / t1.max()), a, np.uint8)
    if name == "head-i16-slope2":
        return _save("head-i16-slope2.nii", np.round(t1 / 2), a, np.int16, slope=2.0)
    if name == "head-i16-slope1":
        return _save("head-i16-slope1.nii", np.round(t1), a, np.int16, slope=1.0)
    if name == "head-i16-intercept":
        return _save(
            "head-i16-intercept.nii", np.round((t1 - 100) * 2), a, np.int16, slope=0.5, inter=100.0
        )
    if name == "head-f32-slope-inter":
        return _save("head-f32-slope-inter.nii", (t1 - 5) / 3, a, np.float32, slope=3.0, inter=5.0)
    if name == "head-f64":
        return _save("head-f64.nii.gz", t1, a, np.float64)
    if name == "head-i8":
        return _save("head-i8.nii", np.round(t1 * 127 / t1.max()), a, np.int8)
    if name == "head-u16":
        return _save("head-u16.nii", np.round(t1 * 4), a, np.uint16)
    if name == "head-i32":
        return _save("head-i32.nii", np.round(t1 * 100), a, np.int32)
    if name == "head-nan":
        v = t1.astype(np.float32).copy()
        v[10, 10, 10] = np.nan
        v[30, 30, 20] = np.inf
        v[31, 30, 20] = -np.inf
        return _save("head-nan.nii", v, a, np.float32)
    if name == "head-oblique":
        c, s = np.cos(0.2), np.sin(0.2)
        r = np.array([[c, 0, s, 0], [0, 1, 0, 0], [-s, 0, c, 0], [0, 0, 0, 1]])
        return _save("head-oblique.nii", t1, r @ a, np.float32)
    if name == "head-nearly-plumb":
        r = np.eye(4)
        r[0, 2], r[2, 0] = -1.5e-4, 1.5e-4
        return _save("head-nearly-plumb.nii", t1, r @ a, np.float32)
    if name == "head-qform-only":
        return _save("head-qform-only.nii", t1, a, np.float32, sform_code=0, qform_code=1)
    if name == "head-sform-only":
        return _save("head-sform-only.nii", t1, a, np.float32, sform_code=1, qform_code=0)
    if name == "head-las":
        f = a.copy()
        f[0, 0], f[0, 3] = -ZOOM, (GRID[0] - 1) * ZOOM / 2
        return _save("head-las.nii", t1[::-1], f, np.float32)
    if name == "head-superior-first":
        # The z axis runs from superior to inferior: the first z planes are the top.
        f = a.copy()
        f[2, 2], f[2, 3] = -ZOOM, (GRID[2] - 1) * ZOOM / 2
        return _save("head-superior-first.nii", t1[:, :, ::-1], f, np.float32)
    if name == "head-coronal":
        # Superior along the second axis (a coronal grid): -SI clips along y.
        f = np.array([[ZOOM, 0, 0, 0], [0, 0, ZOOM, 0], [0, ZOOM, 0, 0], [0, 0, 0, 1.0]])
        f[:3, 3] = [-(GRID[0] - 1) * ZOOM / 2, -(GRID[2] - 1) * ZOOM / 2, -(GRID[1] - 1) * ZOOM / 2]
        return _save("head-coronal.nii", np.transpose(t1, (0, 2, 1)), f, np.float32)
    if name == "head-metre":
        m = a.copy()
        m[:3] /= 1000.0
        return _save("head-metre.nii", t1, m, np.float32, xyzt_units=1)
    if name == "series-i16":
        return _save("series-i16.nii.gz", np.round(_series()), _affine(GRID), np.int16)
    if name == "series-f32":
        return _save("series-f32.nii", _series(), _affine(GRID), np.float32)
    if name == "series-negative":
        return _save(
            "series-negative.nii", -np.round(_series() / 2), _affine(GRID), np.int16, slope=-2.0
        )
    if name == "series-slope":
        return _save(
            "series-slope.nii", np.round(_series() / 2), _affine(GRID), np.int16, slope=2.0
        )
    if name == "series-one":
        return _save("series-one.nii", _series(1), _affine(GRID), np.float32)
    if name == "slice":
        return _save("slice.nii", t1[:, :, 20:21], _affine(GRID), np.float32)
    if name == "slice-small":
        return _save("slice-small.nii", t1[26:38, 26:38, 20:21], _affine(GRID), np.float32)
    if name == "row":
        return _save("row.nii", t1[:, 32:33, 20:21], _affine(GRID), np.float32)
    if name == "two-slices":
        return _save("two-slices.nii", t1[:, :, 19:21], _affine(GRID), np.float32)
    if name == "tiny":
        return _save("tiny.nii", t1[30:32, 30:32, 19:21], _affine(GRID), np.float32)
    if name == "zeros":
        return _save("zeros.nii", np.zeros(GRID), a, np.float32)
    if name == "constant":
        return _save("constant.nii", np.full(GRID, 100.0), a, np.float32)
    if name == "big":
        shape = (128, 128, 72)
        return _save("big.nii.gz", head("t1", shape, 13), _affine(shape, 1.5), np.float32)
    if name == "big-t2":
        shape = (128, 128, 72)
        return _save("big-t2.nii.gz", head("t2", shape, 14), _affine(shape, 1.5), np.float32)
    if name == "slab":
        # More than a million voxels with an axis of 2: the half-size grid keeps 1 (AFNI-47).
        shape = (720, 720, 2)
        v = head("t1", (720, 720, 3), 15)[:, :, :2]
        return _save("slab.nii.gz", v, _affine(shape, 0.3), np.float32)
    raise FileNotFoundError(f"no synthetic input {name!r}")


@functools.cache
def fmriprep_runs() -> list[tuple[str, Path]]:
    """The BOLD runs of the fMRIPrep oracle runs: ``(label, enhance_and_skullstrip_bold_wf
    directory)``, for every run whose BOLD reference fMRIPrep enhanced."""
    ws = workspace()
    if ws is None:
        return []
    out = []
    for wf in sorted(
        (ws / FMRIPREP_RUNS).glob(
            "*/work/fmriprep_25_2_wf/*/*/bold_fit_wf/enhance_and_skullstrip_bold_wf"
        )
    ):
        run = wf.parts[-7].split("-")[0]
        bold = wf.parts[-3].removeprefix("bold_").removesuffix("_wf")
        out.append((f"{run}-{bold}", wf))
    return out


def real(name: str) -> Path:
    """A real input: ``fmriprep:<label>:unifize`` (the input fMRIPrep's 3dUnifize node read),
    ``fmriprep:<label>:automask`` (its 3dAutomask node's), ``fmriprep:<label>:uni`` and
    ``...:mask`` (fMRIPrep's own outputs), or ``td:<catalog path>`` (larmorx-testdata)."""
    kind, _, rest = name.partition(":")
    if kind == "td":
        import larmorx_testdata as td

        return Path(td.get(rest))
    if kind == "fmriprep":
        label, _, what = rest.rpartition(":")
        wf = dict(fmriprep_runs()).get(label)
        if wf is None:
            raise FileNotFoundError(name)
        if what == "unifize":
            found = sorted((wf / "first_mask").glob("*_masked.nii.gz"))
        elif what == "automask":
            found = [wf / "fixhdr_unifize" / "uni_xform.nii.gz"]
        elif what == "uni":
            found = [wf / "unifize" / "uni.nii.gz"]
        elif what == "mask":
            found = [wf / "skullstrip_second_pass" / "uni_xform_mask.nii.gz"]
        else:
            found = []
        if not found or not found[0].is_file():
            raise FileNotFoundError(name)
        return found[0]
    raise FileNotFoundError(name)


def input_path(name: str) -> Path:
    return real(name) if ":" in name else synthetic(name)
