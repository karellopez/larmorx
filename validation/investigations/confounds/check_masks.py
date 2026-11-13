# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's and niworkflows' masks and signals for the confounds: larmorx against this
environment's niworkflows, nibabel, SciPy, scikit-image and numpy, and fMRIPrep 25.2.7's
functions from its tag (the same in 25.2.6), bit for bit.

Sections:

- ``union``: fMRIPrep's ``_binary_union`` against ``lx.confounds.binary_union`` (``reproduce``):
  masks of every NIfTI type (``uint8``, ``int16``, ``int32``, ``uint64`` about ``2**63``,
  ``float32`` and ``float64`` with fractions, negatives, NaN and infinities, a scaled ``int16``),
  NIfTI-2 and ``.nii``, equal shapes, broadcastable shapes (PIPE-75) and others, a missing file;
  the written file's bytes (decompressed) or the error's type and last line. The default against
  fMRIPrep on the masks with their undefined values clamped (PIPE-74) and against PIPE-75's
  ``ValueError``.
- ``morph``: niworkflows' ``BinaryDilation`` and ``BinarySubtraction`` and
  ``image_binary_dilation`` against ``larmorx.niworkflows.morphology`` (``reproduce``): random
  masks of several types (NaN and fractions are inside: ``np.bool_``), radii 0 to 3, negative
  radii (PIPE-77), float radii of the function, a ``bool`` radius (``traits.Int`` takes it), 2-D
  and 4-D masks (SciPy's ``RuntimeError``), different shapes (numpy's ``IndexError``); the
  default against PIPE-77's ``ValueError``.
- ``acc``: fMRIPrep's ``acompcor_masks``, ``mask2vf`` and ``aCompCorMasks`` (from the tag)
  against ``lx.confounds.acompcor_masks``, ``mask2vf`` and ``acompcor_masks_node`` with this
  numpy (``numpy_version``: ``percentile``'s weak ``q`` changed in numpy 2.5): FAST-like
  partial-volume maps (``float32``; GM stored as ``float32``, ``uint8`` with nibabel's scaling,
  ``int16``; NIfTI-2), the segmentation path ``is_aseg=True`` with discrete and partial-volume
  maps and GM maps whose ``uint8`` cast wraps or is undefined (PIPE-76), the BOLD zooms given or
  not, anisotropic voxels, ``mask2vf``'s array bit for bit and its file, an image without a
  positive value (numpy's ``IndexError``), files of other shapes; the default against fMRIPrep
  on the GM map clamped to ``[0, 255]``.
- ``signals``: niworkflows' ``SignalExtraction`` against
  ``larmorx.niworkflows.images.signal_extraction``: several probability maps and thresholds,
  ``uint8`` masks, one label image (integer and float labels), one 4-D file of maps
  (``four_to_three``) or of one volume, series of ``float32``, big-endian ``float32``,
  ``float64``, ``int16`` and scaled ``int16``, one volume (numpy's pairwise sum, chunks of 8192
  in numpy 2.2), no volume, an empty region (NaN), NaN and infinite values, more than 8,192
  voxels, 5-D series, 2-D and 4-D masks, every error niworkflows raises (the first mask's shape,
  the labels' number, a 3-D series, another mask's shape), labels of every length, non-latin-1
  labels (``UnicodeEncodeError``) and labels with a tab (PIPE-78), ``out_file``; the table's
  bytes.
- ``zooms``: fMRIPrep's ``_get_zooms`` against ``lx.confounds.get_zooms`` (2-D to 5-D files,
  negative voxel sizes nibabel fixes).
- ``real``: the ``union_mask``, ``dilated_mask``, ``subtract_mask``, ``acc_masks`` and
  ``signals`` nodes of the 36 confounds workflows of 21 oracle runs (fMRIPrep 25.2.6 in its
  image) with their own inputs: larmorx (``reproduce``) against the node's files, the default
  against ``reproduce``, ``get_zooms`` of the series against the node's ``bold_zooms``; and the
  originals run again here (niworkflows' interfaces, fMRIPrep's functions) against larmorx.

Run it in each environment with the worktree's extension::

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 PYTHONPATH=<worktree>/python \\
        python -P check_masks.py [--tier quick|full] [--sections ...]

(``LARMORX_WORKSPACE``: the workspace, for the oracle runs and the tags). Exit status 1 when
any case differs.
"""

from __future__ import annotations

import argparse
import ast
import contextlib
import gzip
import logging
import os
import sys
import tempfile
import time
import types
import warnings
from collections import defaultdict
from pathlib import Path

import nibabel as nb
import numpy as np
import scipy

import larmorx as lx
from larmorx.confounds import masks as lm
from larmorx.niworkflows import images as li
from larmorx.niworkflows import morphology as lmo

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))
RUNS = WORKSPACE / "oracles" / "fmriprep-25.2.6"
TAG = WORKSPACE / "tags" / "fmriprep-25.2.7" / "fmriprep"
NUMPY = np.__version__
COUNTS: dict[str, list[int]] = defaultdict(lambda: [0, 0])
FAILS: list[str] = []
SECONDS: dict[str, float] = defaultdict(float)
logging.getLogger("nipype.interface").setLevel(logging.ERROR)
logging.getLogger("nipype.workflow").setLevel(logging.ERROR)


def count(key: str, ok: bool, what: str) -> None:
    COUNTS[key][0 if ok else 1] += 1
    if not ok and len(FAILS) < 80:
        FAILS.append(f"{key}: {what}")


def last_line(e: BaseException) -> str:
    msg = str(e).strip().splitlines()
    kind = type(e).__name__
    kind = "ValueError" if kind in ("TraitError",) else kind
    return f"{kind}: {msg[-1] if msg else ''}"


@contextlib.contextmanager
def cwd(path: Path):
    old = os.getcwd()
    os.chdir(path)
    try:
        yield
    finally:
        os.chdir(old)


def content(p) -> bytes:
    data = Path(p).read_bytes()
    return gzip.decompress(data) if str(p).endswith(".gz") else data


# --- the originals -----------------------------------------------------------------------------


def tag_functions(relpath: str, names: tuple[str, ...]) -> dict:
    """The module-level functions ``names`` of the tag's file ``relpath``, executed from their
    source (they import what they use inside)."""
    path = TAG / relpath
    tree = ast.parse(path.read_text())
    ns: dict = {}
    for node in tree.body:
        if isinstance(node, ast.FunctionDef) and node.name in names:
            exec(compile(ast.Module([node], []), str(path), "exec"), ns)
    return {n: ns[n] for n in names}


def tag_module(relpath: str, name: str, replace: dict[str, str] | None = None):
    path = TAG / relpath
    text = path.read_text()
    for a, b in (replace or {}).items():
        assert a in text, a
        text = text.replace(a, b)
    mod = types.ModuleType(name)
    mod.__file__ = str(path)
    sys.modules[name] = mod
    exec(compile(text, str(path), "exec"), mod.__dict__)
    return mod


UNION = tag_functions("workflows/bold/confounds.py", ("_binary_union", "_get_zooms"))
UTILS = tag_module("utils/confounds.py", "_lx_tag_fmriprep_utils_confounds")
IFACES = tag_module(
    "interfaces/confounds.py",
    "_lx_tag_fmriprep_interfaces_confounds",
    {
        "from ..utils.confounds import acompcor_masks": (
            "from _lx_tag_fmriprep_utils_confounds import acompcor_masks"
        )
    },
)


def run_orig(fn, d: Path, *args, **kw):
    """``fn(*args, **kw)`` in the folder ``d``: its result, or the error's last line."""
    d.mkdir(parents=True, exist_ok=True)
    with cwd(d), warnings.catch_warnings():
        warnings.simplefilter("ignore")
        try:
            return fn(*args, **kw)
        except Exception as e:
            return last_line(e)


def run_iface(klass, d: Path, out: str, **inputs):
    """A nipype interface run in ``d``: its output ``out``, or the error's last line."""
    d.mkdir(parents=True, exist_ok=True)
    with cwd(d), warnings.catch_warnings():
        warnings.simplefilter("ignore")
        try:
            res = klass(**inputs).run()
        except Exception as e:
            return last_line(e)
    return getattr(res.outputs, out)


def run_lx(fn, *args, **kw):
    """``fn(*args, **kw)``: ``(result, notice IDs)``, or ``(error's last line, set())``."""
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        try:
            r = fn(*args, **kw)
        except Exception as e:
            return last_line(e), set()
    return r, {w.message.entry for w in caught if isinstance(w.message, lx.UpstreamDefectWarning)}


def same_file(want, got) -> bool:
    """Both an error with the same last line, or both a file with the same bytes."""
    if isinstance(want, str) and isinstance(got, str) and not os.path.exists(want):
        return want == got
    if not (isinstance(want, (str, os.PathLike)) and isinstance(got, (str, os.PathLike))):
        return False
    if not (os.path.exists(want) and os.path.exists(got)):
        return want == got
    return content(want) == content(got)


def describe(want, got) -> str:
    def one(x):
        return x if isinstance(x, str) and not os.path.exists(x) else f"file {x}"

    return f"want {one(want)!s:.160} got {one(got)!s:.160}"


# --- inputs ------------------------------------------------------------------------------------


AFF = np.array(
    [[-2.5, 0.1, 0.0, 90.0], [0.05, 2.5, 0.2, -120.0], [0.0, -0.1, 2.6, -70.0], [0, 0, 0, 1]]
)


def write(path: Path, data, dtype=None, *, version: int = 1, affine=AFF, zooms=None) -> str:
    """``data`` written by nibabel (``dtype``: the stored type; floats into integers scaled)."""
    klass = nb.Nifti2Image if version == 2 else nb.Nifti1Image
    arr = np.asarray(data)
    wide = arr.dtype in (np.dtype(np.int64), np.dtype(np.uint64))
    img = klass(arr, affine, dtype=arr.dtype if wide else None)
    if dtype is not None:
        img.set_data_dtype(dtype)
    if zooms is not None:
        img.header.set_zooms(zooms)
    path.parent.mkdir(parents=True, exist_ok=True)
    img.to_filename(str(path))
    return str(path)


def blobs(rng, shape, frac: float = 0.4) -> np.ndarray:
    """A smooth random mask (seeded noise smoothed by SciPy, thresholded)."""
    from scipy.ndimage import gaussian_filter

    noise = gaussian_filter(rng.standard_normal(shape), 1.5)
    return noise > np.quantile(noise, 1 - frac)


def pv_maps(rng, shape) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """FAST-like partial-volume maps (GM, WM, CSF; ``float32`` summing to 1, many exact 0 and 1)."""
    from scipy.ndimage import gaussian_filter

    raw = np.stack([gaussian_filter(rng.standard_normal(shape), 2.0) for _ in range(3)])
    raw = np.clip(raw * 4 + rng.uniform(-0.3, 0.3, raw.shape), 0, None)
    raw[:, raw.sum(0) == 0] = 0
    tot = raw.sum(0)
    with np.errstate(invalid="ignore", divide="ignore"):
        pv = np.where(tot > 0, raw / tot, 0).astype(np.float32)
    pv[pv > 0.97] = 1.0
    pv[pv < 0.02] = 0.0
    return pv[0], pv[1], pv[2]


# --- union -------------------------------------------------------------------------------------


def union_cases(rng, tmp: Path, tier: str):
    shape = (11, 12, 10)
    n = 3 if tier == "quick" else 10
    for i in range(n):
        m = blobs(rng, shape)
        f = rng.uniform(-1.5, 3.0, shape)
        yield (
            f"uint8 x float64 {i}",
            write(tmp / f"a{i}.nii.gz", m.astype(np.uint8)),
            write(tmp / f"b{i}.nii.gz", f, np.float64),
        )
        yield (
            f"float32 x int16 {i}",
            write(tmp / f"c{i}.nii.gz", f.astype(np.float32)),
            write(tmp / f"d{i}.nii.gz", np.round(f * 3).astype(np.int16)),
        )
    big = rng.uniform(-1, 2, shape)
    big.flat[:6] = [np.nan, np.inf, -np.inf, 1e19, 2.0**63, -1e19]
    big.flat[6:9] = [2.0**63 - 1024, 0.999999, 1.0]
    yield (
        "float64 undefined",
        write(tmp / "u1.nii.gz", big, np.float64),
        write(tmp / "u2.nii.gz", blobs(rng, shape).astype(np.uint8)),
    )
    yield (
        "float32 undefined",
        write(tmp / "u3.nii.gz", big.astype(np.float32)),
        write(tmp / "u4.nii.gz", big[::-1].astype(np.float32)),
    )
    u64 = rng.integers(0, 3, shape).astype(np.uint64)
    u64.flat[:4] = [2**63, 2**63 - 1, 2**64 - 1, 2**63 - 512]
    yield (
        "uint64 about 2**63",
        write(tmp / "u5.nii.gz", u64),
        write(tmp / "u6.nii.gz", np.zeros(shape, np.uint8)),
    )
    yield (
        "int32 x uint8",
        write(tmp / "i1.nii.gz", rng.integers(-3, 3, shape).astype(np.int32)),
        write(tmp / "i2.nii.gz", blobs(rng, shape).astype(np.uint8)),
    )
    yield (
        "scaled int16",
        write(tmp / "s1.nii.gz", rng.uniform(-2, 3, shape), np.int16),
        write(tmp / "s2.nii.gz", rng.uniform(0, 1.2, shape), np.int16),
    )
    yield (
        "nifti-2 and .nii",
        write(tmp / "n1.nii", blobs(rng, shape).astype(np.uint8), version=2),
        write(tmp / "n2.nii.gz", blobs(rng, shape).astype(np.int16), version=2),
    )
    yield (
        "another affine",
        write(tmp / "f1.nii.gz", blobs(rng, shape).astype(np.uint8)),
        write(
            tmp / "f2.nii.gz", blobs(rng, shape).astype(np.uint8), affine=np.diag([3.0, 3, 3, 1])
        ),
    )
    yield (
        "broadcast slice (PIPE-75)",
        write(tmp / "b1.nii.gz", blobs(rng, shape).astype(np.uint8)),
        write(tmp / "b2.nii.gz", blobs(rng, (*shape[:2], 1)).astype(np.uint8)),
    )
    yield (
        "broadcast 4-D (PIPE-75)",
        write(tmp / "b3.nii.gz", blobs(rng, shape).astype(np.uint8)),
        write(tmp / "b4.nii.gz", blobs(rng, (1, 12, 10)).astype(np.uint8)),
    )
    yield (
        "shapes differ (PIPE-75)",
        write(tmp / "b5.nii.gz", blobs(rng, shape).astype(np.uint8)),
        write(tmp / "b6.nii.gz", blobs(rng, (11, 12, 9)).astype(np.uint8)),
    )
    yield (
        "missing file",
        write(tmp / "m1.nii.gz", blobs(rng, shape).astype(np.uint8)),
        str(tmp / "absent.nii.gz"),
    )


def clamped_copy(path: str, out: Path) -> str:
    """The file with its values from ``2**63`` on (``+inf`` included) set to ``2**62``, written as
    nibabel writes the original's (the default's model, PIPE-74)."""
    img = nb.load(path)
    data = np.asanyarray(img.dataobj)
    if data.dtype.kind == "f":
        data = np.where(data >= 2.0**63, data.dtype.type(2.0**62), data)
    elif data.dtype == np.uint64:
        data = np.where(data.astype(np.float64) >= 2.0**63, np.uint64(2**62), data)
    new = img.__class__(data, img.affine, img.header)
    new.to_filename(str(out))
    return str(out)


def union_section(tier: str, tmp: Path) -> None:
    rng = np.random.default_rng(74)
    for k, (tag, m1, m2) in enumerate(union_cases(rng, tmp / "in", tier)):
        want = run_orig(UNION["_binary_union"], tmp / f"o{k}", m1, m2)
        got, entries = run_lx(
            lm.binary_union, m1, m2, out_dir=tmp / f"l{k}", upstream_defects="reproduce"
        )
        count("union reproduce = fMRIPrep", same_file(want, got), f"{tag}: {describe(want, got)}")
        fixed, fe = run_lx(lm.binary_union, m1, m2, out_dir=tmp / f"f{k}")
        if "PIPE-75" in tag:
            count(
                "union default: PIPE-75's ValueError",
                "PIPE-75" in str(fixed) and str(fixed).startswith("ValueError"),
                f"{tag}: {fixed}",
            )
            continue
        if "PIPE-74" in entries or "PIPE-74" in fe:
            c1 = clamped_copy(m1, tmp / f"c{k}a.nii.gz")
            c2 = clamped_copy(m2, tmp / f"c{k}b.nii.gz")
            model = run_orig(UNION["_binary_union"], tmp / f"m{k}", c1, c2)
            count(
                "union default = fMRIPrep on clamped masks (PIPE-74)",
                same_file(model, fixed),
                f"{tag}: {describe(model, fixed)}",
            )
        else:
            count("union default = reproduce (no notice)", same_file(got, fixed) and not fe, tag)


# --- morph -------------------------------------------------------------------------------------


def morph_section(tier: str, tmp: Path) -> None:
    from niworkflows.interfaces.morphology import (
        BinaryDilation,
        BinarySubtraction,
        image_binary_dilation,
    )

    rng = np.random.default_rng(77)
    shapes = (
        [(17, 15, 13), (9, 9, 9)]
        if tier == "quick"
        else [(17, 15, 13), (9, 9, 9), (30, 26, 22), (5, 40, 3)]
    )
    radii = [0, 1, 2, 3, -1, -2, -3, True]
    k = 0
    for shape in shapes:
        kinds = {
            "uint8": blobs(rng, shape, 0.2).astype(np.uint8),
            "float32 fractions NaN": np.where(
                blobs(rng, shape, 0.2), rng.uniform(-1, 1, shape), 0
            ).astype(np.float32),
            "int16 negative": (blobs(rng, shape, 0.15) * -3).astype(np.int16),
        }
        kinds["float32 fractions NaN"].flat[3] = np.nan
        for kind, arr in kinds.items():
            src = write(tmp / "in" / f"m{k}.nii.gz", arr)
            other = write(tmp / "in" / f"s{k}.nii.gz", blobs(rng, shape, 0.3).astype(np.uint8))
            for r in radii:
                tag = f"{shape} {kind} radius {r!r}"
                want = run_iface(
                    BinaryDilation, tmp / f"o{k}-{r}", "out_mask", in_mask=src, radius=r
                )
                got, _ = run_lx(
                    lmo.binary_dilation,
                    src,
                    radius=r,
                    out_dir=tmp / f"l{k}-{r}",
                    upstream_defects="reproduce",
                )
                count(
                    "BinaryDilation reproduce = niworkflows",
                    same_file(want, got),
                    f"{tag}: {describe(want, got)}",
                )
                fixed, fe = run_lx(lmo.binary_dilation, src, radius=r, out_dir=tmp / f"f{k}-{r}")
                if r < 0:
                    count(
                        "BinaryDilation default: PIPE-77's ValueError", "PIPE-77" in str(fixed), tag
                    )
                else:
                    count(
                        "BinaryDilation default = reproduce", same_file(got, fixed) and not fe, tag
                    )
            want = run_iface(
                BinarySubtraction, tmp / f"so{k}", "out_mask", in_base=src, in_subtract=other
            )
            got, _ = run_lx(lmo.binary_subtraction, src, other, out_dir=tmp / f"sl{k}")
            count(
                "BinarySubtraction = niworkflows",
                same_file(want, got),
                f"{shape} {kind}: {describe(want, got)}",
            )
            k += 1
        for r in (2.5, 1.5, 0.5, 2, 4):
            m = blobs(rng, shape, 0.1)
            want = run_orig(image_binary_dilation, tmp / "x", m, radius=r)
            got, _ = run_lx(lmo.image_binary_dilation, m, radius=r, upstream_defects="reproduce")
            ok = (
                isinstance(want, np.ndarray)
                and isinstance(got, np.ndarray)
                and want.dtype == got.dtype
                and np.array_equal(want, got)
            )
            count(
                "image_binary_dilation = niworkflows",
                ok or (isinstance(want, str) and want == got),
                f"{shape} r {r}",
            )
    # errors: 2-D and 4-D masks, a float radius, different shapes, a missing file
    m2 = write(tmp / "in" / "two.nii.gz", blobs(rng, (12, 13, 1))[..., 0].astype(np.uint8))
    m4 = write(tmp / "in" / "four.nii.gz", blobs(rng, (12, 13, 9, 2)).astype(np.uint8))
    m3 = write(tmp / "in" / "three.nii.gz", blobs(rng, (12, 13, 9)).astype(np.uint8))
    m3b = write(tmp / "in" / "threeb.nii.gz", blobs(rng, (12, 13, 8)).astype(np.uint8))
    for tag, kw in (
        ("2-D mask", {"in_mask": m2}),
        ("4-D mask", {"in_mask": m4}),
        ("float radius", {"in_mask": m3, "radius": 2.0}),
        ("missing", {"in_mask": str(tmp / "absent.nii.gz")}),
    ):
        want = run_iface(BinaryDilation, tmp / f"e-{tag}", "out_mask", **kw)
        mask = kw.pop("in_mask")
        got, _ = run_lx(lmo.binary_dilation, mask, out_dir=tmp / f"el-{tag}", **kw)
        count(
            "BinaryDilation errors = niworkflows",
            same_file(want, got),
            f"{tag}: {describe(want, got)}",
        )
    for tag, (a, b) in (
        ("shapes differ", (m3, m3b)),
        ("4-D base", (m4, m3)),
        ("missing", (m3, str(tmp / "absent.nii.gz"))),
    ):
        want = run_iface(BinarySubtraction, tmp / f"se-{tag}", "out_mask", in_base=a, in_subtract=b)
        got, _ = run_lx(lmo.binary_subtraction, a, b, out_dir=tmp / f"sel-{tag}")
        count(
            "BinarySubtraction errors = niworkflows",
            same_file(want, got),
            f"{tag}: {describe(want, got)}",
        )


# --- acc ---------------------------------------------------------------------------------------


def gm_clamped(path: str, out: Path) -> str:
    """The GM map with its values clamped to ``[0, 255]`` (NaN 0) in its own type, written as
    nibabel writes the original (the default's model, PIPE-76)."""
    img = nb.load(path)
    data = np.asanyarray(img.dataobj)
    if data.dtype.kind == "f":
        data = np.clip(np.nan_to_num(data, nan=0.0, posinf=255.0, neginf=0.0), 0, 255).astype(
            data.dtype
        )
    else:
        data = np.clip(data, 0, 255).astype(data.dtype)
    img.__class__(data, img.affine, img.header).to_filename(str(out))
    return str(out)


def acc_cases(rng, tmp: Path, tier: str):
    shapes = [(24, 26, 20)] if tier == "quick" else [(24, 26, 20), (31, 17, 23), (40, 44, 36)]
    k = 0
    for shape in shapes:
        gm, wm, csf = pv_maps(rng, shape)
        zooms = (1.0, 1.2, 0.9)
        for store in ("float32", "uint8", "int16", "nifti-2"):
            d = tmp / f"in{k}"
            dt = None if store in ("float32", "nifti-2") else store
            v = 2 if store == "nifti-2" else 1
            files = [
                write(d / "gm.nii.gz", gm, dt, version=v, zooms=zooms),
                write(d / "wm.nii.gz", wm, zooms=zooms),
                write(d / "csf.nii.gz", csf, zooms=zooms),
            ]
            yield f"{shape} pv GM {store}", files, False, (2.0, 2.0, 2.4)
            k += 1
        d = tmp / f"in{k}"
        seg = rng.integers(0, 3, shape)
        files = [
            write(d / "gm.nii.gz", (seg == 0).astype(np.uint8), zooms=zooms),
            write(d / "wm.nii.gz", (seg == 1).astype(np.uint8), zooms=zooms),
            write(d / "csf.nii.gz", (seg == 2).astype(np.uint8), zooms=zooms),
        ]
        yield f"{shape} aseg discrete", files, True, (2.0, 2.0, 2.4)
        yield f"{shape} aseg discrete, zooms None", files, True, None
        k += 1
        d = tmp / f"in{k}"
        pfiles = [
            write(d / f"{n}.nii.gz", a, zooms=zooms)
            for n, a in (("gm", gm), ("wm", wm), ("csf", csf))
        ]
        yield (
            f"{shape} aseg on partial-volume maps (fMRIPrep with FreeSurfer)",
            pfiles,
            True,
            (2.5, 2.5, 2.5),
        )
        k += 1
        d = tmp / f"in{k}"
        bad = (seg == 0).astype(np.int16)
        bad.flat[:5] = [-1, 256, 300, 511, -255]
        yield (
            f"{shape} aseg int16 GM wraps (PIPE-76)",
            [write(d / "gm.nii.gz", bad, zooms=zooms), *files[1:]],
            True,
            (2.0, 2.0, 2.4),
        )
        k += 1
        d = tmp / f"in{k}"
        badf = gm.copy()
        badf.flat[:7] = [np.nan, -1.5, 300.7, 1e10, np.inf, 256.0, 257.9]
        yield (
            f"{shape} aseg float32 GM undefined (PIPE-76)",
            [write(d / "gm.nii.gz", badf, zooms=zooms), *pfiles[1:]],
            True,
            (2.0, 2.0, 2.4),
        )
        k += 1
    d = tmp / f"in{k}"
    gm, wm, csf = pv_maps(rng, (12, 13, 11))
    g = write(d / "gm.nii.gz", gm)
    yield "two files", [g, write(d / "wm.nii.gz", wm)], False, (2.0, 2.0, 2.0)
    yield (
        "WM of another shape",
        [g, write(d / "wm2.nii.gz", wm[:, :, :10]), write(d / "csf.nii.gz", csf)],
        False,
        (2.0, 2.0, 2.0),
    )
    yield (
        "aseg CSF without a positive value",
        [g, write(d / "wm3.nii.gz", wm), write(d / "csf0.nii.gz", np.zeros_like(csf))],
        True,
        (2.0, 2.0, 2.0),
    )
    yield (
        "missing CSF",
        [g, write(d / "wm4.nii.gz", wm), str(d / "absent.nii.gz")],
        False,
        (2.0, 2.0, 2.0),
    )


def acc_section(tier: str, tmp: Path) -> None:
    rng = np.random.default_rng(76)
    for k, (tag, files, aseg, zooms) in enumerate(acc_cases(rng, tmp, tier), start=1):
        want = run_orig(UTILS.acompcor_masks, tmp / f"o{k}", files, aseg, zooms)
        got, entries = run_lx(
            lm.acompcor_masks,
            files,
            aseg,
            zooms,
            out_dir=tmp / f"l{k}",
            numpy_version=NUMPY,
            upstream_defects="reproduce",
        )
        if isinstance(want, list) and isinstance(got, list):
            ok = len(want) == len(got) == 3 and all(
                (a == b) if not aseg and i == 0 else content(a) == content(b)
                for i, (a, b) in enumerate(zip(want, got, strict=True))
            )
        else:
            ok = want == got
        count("acompcor_masks reproduce = fMRIPrep", ok, f"{tag}: {want!s:.150} | {got!s:.150}")
        fixed, fe = run_lx(
            lm.acompcor_masks, files, aseg, zooms, out_dir=tmp / f"f{k}", numpy_version=NUMPY
        )
        if "PIPE-76" in entries or "PIPE-76" in fe:
            c = gm_clamped(files[0], tmp / f"gm{k}.nii.gz")
            model = run_orig(UTILS.acompcor_masks, tmp / f"m{k}", [c, *files[1:]], aseg, zooms)
            ok = (
                isinstance(model, list)
                and isinstance(fixed, list)
                and all(content(a) == content(b) for a, b in zip(model[1:], fixed[1:], strict=True))
            )
            count("acompcor_masks default = fMRIPrep on the clamped GM (PIPE-76)", ok, tag)
        elif isinstance(got, list):
            ok = isinstance(fixed, list) and all(
                content(a) == content(b) for a, b in zip(got[1:], fixed[1:], strict=True)
            )
            count("acompcor_masks default = reproduce (no notice)", ok and not fe, tag)
        # the interface
        if k % 3 == 1 and isinstance(zooms, tuple):
            want = run_iface(
                IFACES.aCompCorMasks,
                tmp / f"io{k}",
                "out_masks",
                in_vfs=files,
                is_aseg=aseg,
                bold_zooms=zooms,
            )
            got, _ = run_lx(
                lm.acompcor_masks_node,
                files,
                zooms,
                is_aseg=aseg,
                out_dir=tmp / f"il{k}",
                numpy_version=NUMPY,
                upstream_defects="reproduce",
            )
            got = got.out_masks if hasattr(got, "out_masks") else got
            if isinstance(want, list) and isinstance(got, list):
                ok = all(
                    (a == b) if not aseg and i == 0 else content(a) == content(b)
                    for i, (a, b) in enumerate(zip(want, got, strict=True))
                )
            else:
                ok = want == got
            count("aCompCorMasks = fMRIPrep", ok, f"{tag}: {want!s:.150} | {got!s:.150}")
        # mask2vf alone, its array and its file
        if aseg and len(files) == 3 and os.path.exists(files[1]):
            for z in (zooms, None):
                want = run_orig(UTILS.mask2vf, tmp / "v", files[1], z)
                got, _ = run_lx(lm.mask2vf, files[1], z, numpy_version=NUMPY)
                ok = (
                    isinstance(want, np.ndarray)
                    and isinstance(got, np.ndarray)
                    and want.dtype == got.dtype
                    and want.shape == got.shape
                    and want.tobytes() == np.ascontiguousarray(got).tobytes(order="C")
                    if isinstance(got, np.ndarray) and isinstance(want, np.ndarray)
                    else want == got
                )
                count("mask2vf array = fMRIPrep", bool(ok), f"{tag} zooms {z}")
            want = run_orig(UTILS.mask2vf, tmp / "v", files[2], zooms, str(tmp / f"vo{k}.nii.gz"))
            got, _ = run_lx(
                lm.mask2vf, files[2], zooms, str(tmp / f"vl{k}.nii.gz"), numpy_version=NUMPY
            )
            count("mask2vf file = fMRIPrep", same_file(want, got), f"{tag}: {describe(want, got)}")
    # aCompCorMasks' traits
    gm, wm, csf = pv_maps(rng, (9, 9, 9))
    files = [write(tmp / "t" / f"{n}.nii.gz", a) for n, a in (("gm", gm), ("wm", wm), ("csf", csf))]
    for tag, z in (("two zooms", (2.0, 2.0)), ("text zoom", (2.0, "a", 2.0))):
        want = run_iface(
            IFACES.aCompCorMasks, tmp / f"t-{tag}", "out_masks", in_vfs=files, bold_zooms=z
        )
        got, _ = run_lx(lm.acompcor_masks_node, files, z, out_dir=tmp / f"tl-{tag}")
        count(
            "aCompCorMasks errors refused by both",
            isinstance(want, str)
            and isinstance(got, str)
            and want.split(":")[0] == got.split(":")[0],
            f"{tag}: {want} | {got}",
        )


# --- signals -----------------------------------------------------------------------------------


def signal_cases(rng, tmp: Path, tier: str):
    """``(tag, in_file, label_files, class_labels, keywords)``."""
    shape = (13, 12, 11)
    t = 7
    n = 2 if tier == "quick" else 6
    for i in range(n):
        series = rng.normal(1000, 50, (*shape, t))
        series[..., ::2] *= 1 + 1e-9 * rng.standard_normal((*shape, 1))
        maps = [rng.uniform(0, 1, shape) * blobs(rng, shape, 0.5) for _ in range(3)]
        bold = write(tmp / f"b{i}.nii.gz", series.astype(np.float32))
        lf = [write(tmp / f"p{i}{j}.nii.gz", m) for j, m in enumerate(maps)]
        lab = ["csf", "white_matter", "csf_wm"]
        yield f"float32 maps {i}", bold, lf, lab, {}
        for pt in (0.0, 0.3, 0.7, 1.0, True):
            yield f"float32 maps {i} prob_thres {pt}", bold, lf, lab, {"prob_thres": pt}
        um = [
            write(tmp / f"m{i}{j}.nii.gz", blobs(rng, shape, 0.3).astype(np.uint8))
            for j in range(2)
        ]
        yield f"uint8 masks {i}", bold, um, ["global_signal", "tcompcor"], {}
        labels = rng.integers(0, 5, shape).astype(np.int16)
        yield (
            f"label image int16 {i}",
            bold,
            [write(tmp / f"l{i}.nii.gz", labels)],
            list("abcd"),
            {},
        )
        fl = np.where(labels > 0, labels * 0.5 + 0.25, 0.0)
        fl.flat[0] = np.nan
        yield (
            f"label image float with NaN {i}",
            bold,
            [write(tmp / f"lf{i}.nii.gz", fl)],
            list("abcde"),
            {},
        )
        four = np.stack(maps, axis=-1)
        yield f"one 4-D file of maps {i}", bold, [write(tmp / f"q{i}.nii.gz", four)], lab, {}
        yield (
            f"one 4-D file of one volume {i}",
            bold,
            [write(tmp / f"q1{i}.nii.gz", labels[..., None])],
            list("abcd"),
            {},
        )
        for dt, name in (
            (np.float64, "float64"),
            (np.int16, "int16 scaled"),
            (">f4", "big-endian float32"),
        ):
            p = tmp / f"s{i}{name.replace(' ', '')}.nii.gz"
            if dt == ">f4":
                img = nb.Nifti1Image(series.astype(">f4"), AFF)
                img.to_filename(str(p))
                path = str(p)
            else:
                path = write(p, series, dt)
            yield f"{name} series {i}", path, um, ["global_signal", "tcompcor"], {}
    v1 = write(tmp / "one.nii.gz", rng.normal(5, 1, (*shape, 1)).astype(np.float32))
    big = (40, 30, 20)
    bigser = write(tmp / "bigone.nii.gz", rng.normal(5, 1, (*big, 1)).astype(np.float32))
    bigm = write(tmp / "bigm.nii.gz", blobs(rng, big, 0.7).astype(np.uint8))
    bigm2 = write(tmp / "bigm2.nii.gz", np.ones(big, np.uint8))
    um = [write(tmp / f"mm{j}.nii.gz", blobs(rng, shape, 0.3).astype(np.uint8)) for j in range(2)]
    yield "one volume", v1, um, ["a", "b"], {}
    yield (
        "one volume, 8,400 and 24,000 voxels (numpy 2.2's chunks)",
        bigser,
        [bigm, bigm2],
        ["a", "b"],
        {},
    )
    yield (
        "no volume",
        write(tmp / "zero.nii.gz", np.zeros((*shape, 0), np.float32)),
        um,
        ["a", "b"],
        {},
    )
    empty = write(tmp / "empty.nii.gz", np.zeros(shape, np.uint8))
    bold = write(tmp / "bold.nii.gz", rng.normal(100, 5, (*shape, 5)).astype(np.float32))
    yield "an empty region", bold, [um[0], empty], ["a", "b"], {}
    nanser = rng.normal(100, 5, (*shape, 5))
    nanser.flat[::97] = np.nan
    nanser.flat[5::101] = np.inf
    nanser.flat[7::103] = -np.inf
    yield "NaN and infinities", write(tmp / "nan.nii.gz", nanser), um, ["a", "b"], {}
    yield (
        "5-D series",
        write(tmp / "five.nii.gz", rng.normal(1, 1, (*shape, 4, 1)).astype(np.float32)),
        um,
        ["a", "b"],
        {},
    )
    yield (
        "5-D series of 2",
        write(tmp / "five2.nii.gz", rng.normal(1, 1, (*shape, 4, 2)).astype(np.float32)),
        um,
        ["a", "b"],
        {},
    )
    yield (
        "2-D second mask",
        bold,
        [um[0], write(tmp / "m2d.nii.gz", blobs(rng, (*shape[:2], 1))[..., 0].astype(np.uint8))],
        ["a", "b"],
        {},
    )
    flat = (13, 12, 1)
    fb = write(tmp / "flatb.nii.gz", rng.normal(1, 1, (*flat, 3)).astype(np.float32))
    yield (
        "2-D second mask on one slice",
        fb,
        [
            write(tmp / "fm.nii.gz", blobs(rng, flat).astype(np.uint8)),
            write(tmp / "m2d1.nii.gz", blobs(rng, flat)[..., 0].astype(np.uint8)),
        ],
        ["a", "b"],
        {},
    )
    yield (
        "4-D second mask",
        bold,
        [um[0], write(tmp / "m4d.nii.gz", blobs(rng, (*shape, 1)).astype(np.uint8))],
        ["a", "b"],
        {},
    )
    yield (
        "first mask's shape",
        bold,
        [write(tmp / "sm.nii.gz", blobs(rng, (13, 12, 10)).astype(np.uint8))],
        ["a"],
        {},
    )
    yield (
        "second mask's shape",
        bold,
        [um[0], write(tmp / "sm2.nii.gz", blobs(rng, (13, 12, 10)).astype(np.uint8))],
        ["a", "b"],
        {},
    )
    yield "labels' number", bold, um, ["a"], {}
    yield "3-D series", write(tmp / "three.nii.gz", np.ones(shape, np.float32)), um, ["a", "b"], {}
    yield "long and odd labels", bold, um, ["a" * 70, "é ü %s {}"], {}
    yield "non-latin-1 label (UTF-8)", bold, um, ["a", "→"], {}
    yield "label with a tab (PIPE-78)", bold, um, ["a\tb", "c"], {}
    yield "label with a line break (PIPE-78)", bold, um, ["a", "c\nd"], {}
    yield "out_file", bold, um, ["a", "b"], {"out_file": "sig.tsv"}
    yield "one label file as a path", bold, um[0], ["a"], {}
    yield "prob_thres above 1", bold, um, ["a", "b"], {"prob_thres": 1.5}
    yield "missing label file", bold, [um[0], str(tmp / "absent.nii.gz")], ["a", "b"], {}


def signals_section(tier: str, tmp: Path) -> None:
    from niworkflows.interfaces.images import SignalExtraction

    rng = np.random.default_rng(78)
    for k, (tag, bold, lf, lab, kw) in enumerate(signal_cases(rng, tmp / "in", tier)):
        want = run_iface(
            SignalExtraction,
            tmp / f"o{k}",
            "out_file",
            in_file=bold,
            label_files=lf,
            class_labels=lab,
            **kw,
        )
        got, _ = run_lx(
            li.signal_extraction,
            bold,
            lf,
            lab,
            out_dir=tmp / f"l{k}",
            numpy_version=NUMPY,
            n_threads=3,
            upstream_defects="reproduce",
            **kw,
        )
        count(
            "SignalExtraction reproduce = niworkflows",
            same_file(want, got),
            f"{tag}: {describe(want, got)}",
        )
        fixed, fe = run_lx(
            li.signal_extraction, bold, lf, lab, out_dir=tmp / f"f{k}", numpy_version=NUMPY, **kw
        )
        if "PIPE-78" in tag:
            count("SignalExtraction default: PIPE-78's ValueError", "PIPE-78" in str(fixed), tag)
        else:
            count(
                "SignalExtraction default = reproduce (no notice, 1 thread)",
                same_file(got, fixed) and not fe,
                tag,
            )


# --- zooms -------------------------------------------------------------------------------------


def zooms_section(tier: str, tmp: Path) -> None:
    rng = np.random.default_rng(3)
    cases = {
        "2-D": ((5, 6), (1.5, 2.5)),
        "3-D": ((5, 6, 7), (1.5, 2.5, 3.3)),
        "4-D": ((5, 6, 7, 3), (1.4, 1.3999999, 1.4, 2.0)),
        "5-D": ((3, 4, 5, 2, 2), (1.0, 2.0, 3.0, 0.8, 1.0)),
    }
    for tag, (shape, z) in cases.items():
        p = write(tmp / f"{tag}.nii.gz", rng.normal(0, 1, shape).astype(np.float32), zooms=z)
        want = run_orig(UNION["_get_zooms"], tmp, p)
        got, _ = run_lx(lm.get_zooms, p)
        count(
            "get_zooms = fMRIPrep", isinstance(want, tuple) and want == got, f"{tag}: {want} {got}"
        )
    img = nb.Nifti1Image(np.zeros((4, 4, 4), np.float32), np.eye(4))
    img.header["pixdim"][1:4] = [-2.0, 0.0, 3.0]
    p = tmp / "neg.nii.gz"
    img.to_filename(str(p))
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        want = run_orig(UNION["_get_zooms"], tmp, str(p))
        got, _ = run_lx(lm.get_zooms, str(p))
    count("get_zooms = fMRIPrep", want == got, f"negative and zero sizes: {want} {got}")


# --- real --------------------------------------------------------------------------------------

QUICK_REAL = ("ds005040", "ds005752-sub-ON01016-noreconall/", "ds000256", "ds005454", "ds000258")


def real_section(tier: str, tmp: Path) -> None:
    from nipype.utils.filemanip import loadpkl
    from niworkflows.interfaces.images import SignalExtraction
    from niworkflows.interfaces.morphology import BinaryDilation, BinarySubtraction

    wfs = sorted(RUNS.glob("*/work/fmriprep_25_2_wf/sub_*_wf/bold_*_wf/bold_confounds_wf"))
    if tier == "quick":
        wfs = [w for w in wfs if any(str(w.relative_to(RUNS)).startswith(q) for q in QUICK_REAL)]
    print(f"real: {len(wfs)} confounds workflows", flush=True)

    def timed(key, fn, *a, **k):
        t0 = time.perf_counter()
        r = fn(*a, **k)
        SECONDS[key] += time.perf_counter() - t0
        return r

    for wf in wfs:
        run = wf.relative_to(RUNS).parts[0]

        def host(v: str, run: str = run) -> str:
            return str(RUNS / run / v.lstrip("/"))

        tag = f"{run}/{wf.parts[-2]}"
        d = tmp / run / wf.parts[-2]
        node = {}
        # union_mask
        i = loadpkl(wf / "union_mask" / "_inputs.pklz")
        a, b = host(i["mask1"]), host(i["mask2"])
        node["union"] = wf / "union_mask" / "mask_union.nii.gz"
        got, _ = run_lx(
            timed,
            "larmorx binary_union",
            lm.binary_union,
            a,
            b,
            out_dir=d / "u",
            upstream_defects="reproduce",
        )
        count("real union_mask = node", same_file(node["union"], got), tag)
        fixed, fe = run_lx(lm.binary_union, a, b, out_dir=d / "uf")
        count("real union_mask default = reproduce", same_file(got, fixed) and not fe, tag)
        want = timed("fMRIPrep _binary_union", run_orig, UNION["_binary_union"], d / "uo", a, b)
        count("real union_mask fMRIPrep again = larmorx", same_file(want, got), tag)
        # dilated_mask, subtract_mask
        i = loadpkl(wf / "dilated_mask" / "_inputs.pklz")
        m, r = host(i["in_mask"]), i["radius"]
        got, _ = run_lx(
            timed,
            "larmorx BinaryDilation",
            lmo.binary_dilation,
            m,
            radius=r,
            out_dir=d / "d",
            n_threads=8,
            upstream_defects="reproduce",
        )
        count(
            "real dilated_mask = node",
            same_file(wf / "dilated_mask" / "dilated_mask.nii.gz", got),
            tag,
        )
        fixed, fe = run_lx(lmo.binary_dilation, m, radius=r, out_dir=d / "df")
        count(
            "real dilated_mask default (1 thread) = reproduce",
            same_file(got, fixed) and not fe,
            tag,
        )
        want = timed(
            "niworkflows BinaryDilation",
            run_iface,
            BinaryDilation,
            d / "do",
            "out_mask",
            in_mask=m,
            radius=r,
        )
        count("real dilated_mask niworkflows again = larmorx", same_file(want, got), tag)
        i = loadpkl(wf / "subtract_mask" / "_inputs.pklz")
        a, b = host(i["in_base"]), host(i["in_subtract"])
        got, _ = run_lx(
            timed, "larmorx BinarySubtraction", lmo.binary_subtraction, a, b, out_dir=d / "s"
        )
        count(
            "real subtract_mask = node",
            same_file(wf / "subtract_mask" / "subtracted_mask.nii.gz", got),
            tag,
        )
        want = timed(
            "niworkflows BinarySubtraction",
            run_iface,
            BinarySubtraction,
            d / "so",
            "out_mask",
            in_base=a,
            in_subtract=b,
        )
        count("real subtract_mask niworkflows again = larmorx", same_file(want, got), tag)
        # acc_masks
        i = loadpkl(wf / "acc_masks" / "_inputs.pklz")
        vfs = [host(v) for v in i["in_vfs"]]
        got, _ = run_lx(
            timed, "larmorx aCompCorMasks", lm.acompcor_masks_node, vfs, i["bold_zooms"], is_aseg=i["is_aseg"],
            out_dir=d / "a", n_threads=8, numpy_version=NUMPY, upstream_defects="reproduce",
        )  # fmt: skip
        masks = got.out_masks if hasattr(got, "out_masks") else [got] * 3
        ok = (
            masks[0] == vfs[2]
            and same_file(wf / "acc_masks" / "acompcor_wm.nii.gz", masks[1])
            and same_file(wf / "acc_masks" / "acompcor_wmcsf.nii.gz", masks[2])
        )
        count("real acc_masks = node", ok, tag)
        fixed, fe = run_lx(
            lm.acompcor_masks_node,
            vfs,
            i["bold_zooms"],
            is_aseg=i["is_aseg"],
            out_dir=d / "af",
            numpy_version=NUMPY,
        )
        fm = fixed.out_masks if hasattr(fixed, "out_masks") else [fixed] * 3
        count(
            "real acc_masks default (1 thread) = reproduce",
            fe == set() and all(same_file(x, y) for x, y in zip(masks[1:], fm[1:], strict=True)),
            tag,
        )
        want = timed(
            "fMRIPrep aCompCorMasks",
            run_iface,
            IFACES.aCompCorMasks,
            d / "ao",
            "out_masks",
            in_vfs=vfs,
            is_aseg=i["is_aseg"],
            bold_zooms=i["bold_zooms"],
        )
        ok = (
            isinstance(want, list)
            and want[0] == masks[0]
            and all(same_file(x, y) for x, y in zip(want[1:], masks[1:], strict=True))
        )
        count("real acc_masks fMRIPrep again = larmorx", ok, tag)
        # signals
        i = loadpkl(wf / "signals" / "_inputs.pklz")
        bold, lf = host(i["in_file"]), [host(v) for v in i["label_files"]]
        count(
            "real get_zooms(bold) = node's bold_zooms",
            lm.get_zooms(bold) == tuple(loadpkl(wf / "acc_masks" / "_inputs.pklz")["bold_zooms"]),
            tag,
        )
        got, _ = run_lx(
            timed, "larmorx SignalExtraction", li.signal_extraction, bold, lf, i["class_labels"], prob_thres=i["prob_thres"],
            out_dir=d / "g", n_threads=8, numpy_version=NUMPY, upstream_defects="reproduce",
        )  # fmt: skip
        count("real signals = node", same_file(wf / "signals" / "signals.tsv", got), tag)
        fixed, fe = run_lx(
            li.signal_extraction,
            bold,
            lf,
            i["class_labels"],
            prob_thres=i["prob_thres"],
            out_dir=d / "gf",
            numpy_version=NUMPY,
        )
        count("real signals default (1 thread) = reproduce", same_file(got, fixed) and not fe, tag)
        want = timed(
            "niworkflows SignalExtraction", run_iface, SignalExtraction, d / "go", "out_file",
            in_file=bold, label_files=lf, class_labels=i["class_labels"], prob_thres=i["prob_thres"],
        )  # fmt: skip
        count("real signals niworkflows again = larmorx", same_file(want, got), tag)


# -----------------------------------------------------------------------------------------------


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--tier", choices=("quick", "full"), default="quick")
    p.add_argument("--sections", default="union,morph,acc,signals,zooms,real")
    a = p.parse_args()
    import niworkflows
    import skimage

    from larmorx.io.nibabel_header import use_lapack

    use_lapack("3.12.1" if tuple(int(x) for x in NUMPY.split(".")[:2]) >= (2, 4) else "3.12.0")
    print(
        f"numpy {NUMPY}, SciPy {scipy.__version__}, scikit-image {skimage.__version__}, "
        f"niworkflows {niworkflows.__version__}, nibabel {nb.__version__}, tier {a.tier}",
        flush=True,
    )
    with tempfile.TemporaryDirectory(prefix="lx-check-masks-") as tmp:
        for s in a.sections.split(","):
            t0 = time.perf_counter()
            d = Path(tmp) / s
            d.mkdir()
            {
                "union": union_section,
                "morph": morph_section,
                "acc": acc_section,
                "signals": signals_section,
                "zooms": zooms_section,
                "real": real_section,
            }[s](a.tier, d)
            print(f"section {s}: {time.perf_counter() - t0:.1f} s", flush=True)
    for key, (i, d) in COUNTS.items():
        print(f"  {key:66s} identical {i:6d}  differ {d:5d}")
    if SECONDS:
        print("  seconds: " + ", ".join(f"{k} {v:.1f}" for k, v in SECONDS.items()))
    for f in FAILS:
        print("  FAIL " + f)
    differ = sum(c[1] for c in COUNTS.values())
    print(f"CHECK {'PASS' if differ == 0 else 'DIFFER'}: {differ} cases differ")
    return 0 if differ == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
