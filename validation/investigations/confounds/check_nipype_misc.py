# SPDX-License-Identifier: Apache-2.0
"""nipype's TSNR, NonSteadyStateDetector and FramewiseDisplacement, and niworkflows'
RobustACompCor/RobustTCompCor: larmorx against this environment's nipype, niworkflows, numpy and
nibabel, bit for bit.

Sections:

- ``tsnr``: nipype's ``TSNR`` interface (run in a temporary folder) against
  ``lx.confounds.tsnr(..., upstream_defects="reproduce")``: every file (header and data as nibabel
  reads them), over the stored types (float32, float64, int16, uint8, int32, scaled int16 and
  uint8, big-endian float32 and int16), NaN, infinite, constant and near-threshold voxels, 1 to
  130 volumes, ``regress_poly`` None and 1 to 4, a 3-D file, lists of 3-D and 4-D files, a 5-D
  file, NIfTI-2, ``.nii``, a TR in milliseconds, the output names given; the errors (a 2-D file,
  mismatched shapes, ``regress_poly`` 0). The default (``"fix"``) is checked to equal
  ``reproduce`` for float series and, for integer series (PIPE-51), to equal nipype's code with
  its float32 header switch made effective (``nipype_tsnr_float32``).
- ``nssd``: nipype's ``NonSteadyStateDetector`` against ``lx.confounds.nonsteady_state_detector``
  (``n_volumes_to_discard``; the global signal against nibabel's ``dataobj[:, :, :, :50]`` and
  numpy's three ``mean(axis=0)``s, bit for bit with its type; nipype's ``is_outlier`` of it): the
  stored types, 1 to 120 volumes, first axes of 2 to 9,000 values (numpy's pairwise blocks and
  numpy 2.2's chunks), leading bright volumes, constant series (PIPE-23), a 5-D file, ``.nii``, a
  3-D file (nibabel's ``IndexError``). The default equals ``reproduce`` unless PIPE-23 is noticed.
- ``fd``: nipype's ``FramewiseDisplacement`` against ``lx.confounds.framewise_displacement``
  (the file byte for byte, ``fd_average`` bit for bit): every source, 3 to 9 columns, 2 to 9,001
  volumes, five radii, NaN and infinite values, several text formats; the errors (one row, one
  column, an empty file, too few columns, a lower-case source); ``save_plot`` counted apart.
  ``normalize_mc_params`` on single rows of every length and source, in any case.
- ``nipy``: the ``NIPY`` source (nipy 0.6.1's ``to_matrix44`` and ``aff2euler``, which nipype
  imports from nipy: ``nipy_inject.py`` compiles them from the tag's own source into
  ``sys.modules``). ``normalize_mc_params(row, "NIPY")`` and ``FramewiseDisplacement`` against
  nipype running nipy's functions in three forms: with glibc's ``exp`` and a correctly rounded
  ``atan2`` (what ``upstream_defects="reproduce"`` follows), with every C function correctly
  rounded (CORE-MATH's: the default), and as this machine runs them (``plain``: SVML's ``exp``
  with AVX-512, glibc's ``atan2``), whose differences from ``reproduce`` are counted apart as
  expected, each with its cause (``exp``: NP-04; ``atan2``: D4) told exactly by a fourth form
  (numpy's own ``exp``, a correctly rounded ``atan2``). Rows of 0 to 15 values over eight scales
  (the Taylor form below ``SMALL_ANGLE``, the identity above ``MAX_ANGLE``, both boundaries),
  scales and log-scales (negative, zero, tiny, huge, NaN, infinite; clamped), gimbal lock, NaN
  and infinite values everywhere; float32, float16, integer and boolean rows (the integer rows
  whose squares wrap: PIPE-60), 2-D and 0-D arrays; GLIBC-07's notice exactly where the two
  modes differ. ``FramewiseDisplacement`` files of 2 to 15 columns (the errors of 3 to 5 and 8
  to 11, the valid 10 and 11 with a second vector above ``MAX_ANGLE``), 2 to 9,001 rows, five
  radii, NaN rows.
- ``robust``: niworkflows' ``RobustACompCor`` and ``RobustTCompCor`` against
  ``lx.niworkflows.robust_acompcor``/``robust_tcompcor`` (every file), and PIPE-48's retries
  measured with niworkflows' ``sleep`` replaced (the attempts, the sleeps asked for).
- ``real``: the oracle runs' raw BOLD series (``--tier quick``: five runs; ``full``: one per
  dataset) through ``TSNR`` (``regress_poly`` 1, none for the largest) and
  ``NonSteadyStateDetector``; real motion parameters through ``FramewiseDisplacement``:
  fMRIPrep's own motion columns (trans/rot) as SPM and FSL tables, AFNI ``3dvolreg``'s and FSL
  ``mcflirt``'s files of the figures cache, also read as nipy's parameters (``NIPY``; the
  ``3dvolreg`` file with its index column gives seven columns: a scale, often negative); with
  the times.

``--control`` tells larmorx the other numpy release (2.2.6 for numpy >= 2.3, else 2.5.4) and
counts the cases whose bits change. Run it in each environment with the worktree's extension
(numpy 2.5.4 with ``OPENBLAS_L2_SIZE=512``)::

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 PYTHONPATH=<worktree>/python \\
        python -P check_nipype_misc.py [--tier quick|full] [--sections ...] [--control]

Exit status 1 when any case differs (``--control``: never).
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import logging
import os
import sys
import tempfile
import time
import warnings
from collections import defaultdict
from pathlib import Path

import nibabel as nb
import numpy as np
from nipype.algorithms import confounds as nac
from niworkflows.interfaces import patches as nwp

import larmorx as lx

sys.path.insert(0, str(Path(__file__).resolve().parent))
import nipy_inject as ni
from check_compcor import cwd, iface_inputs, same

NUMPY = np.__version__
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))
RUNS = WORKSPACE / "oracles" / "fmriprep-25.2.6"
FIGCACHE = WORKSPACE / "oracles" / "figures-cache"
COUNTS: dict[str, list[int]] = defaultdict(lambda: [0, 0, 0])  # identical, nan-bits, differ
NOTICES: dict[str, int] = defaultdict(int)
APART: dict[str, int] = defaultdict(int)
FAILS: list[str] = []
CONTROL = False
NP = NUMPY
logging.getLogger("nipype.interface").setLevel(logging.ERROR)
logging.getLogger("nipype.workflow").setLevel(logging.ERROR)


def count(key: str, verdict: str, what: str) -> None:
    if verdict == "differ" and os.environ.get("CHECK_VERBOSE"):
        print(f"    differ {key}: {what}", flush=True)
    COUNTS[key][["identical", "nan-bits", "differ"].index(verdict)] += 1
    if verdict == "differ" and not CONTROL and len(FAILS) < 60:
        FAILS.append(f"{key}: {what}")


def digest(p: Path) -> str:
    """A NIfTI file's header and data as nibabel reads them (type, shape and bytes), or a text
    file's bytes, as a SHA-256."""
    h = hashlib.sha256()
    if p.name.endswith((".nii.gz", ".nii")):
        img = nb.load(p)
        data = np.asarray(img.dataobj)
        h.update(img.header.binaryblock)
        h.update(f"{data.dtype.str}{data.shape}".encode())
        h.update(np.ascontiguousarray(data).tobytes())
    else:
        h.update(p.read_bytes())
    return h.hexdigest()


def files_of(d: Path) -> dict[str, str]:
    return {
        p.name: digest(p)
        for p in sorted(d.iterdir())
        if p.is_file() and not p.name.startswith(("_", "result"))
    }


def last_line(e: BaseException) -> str:
    msg = str(e).strip().splitlines()
    return f"{type(e).__name__}: {msg[-1] if msg else ''}"


def run_nipype(klass, d: Path, **inputs):
    """The interface's files and outputs, or its error's last line."""
    d.mkdir(parents=True)
    with cwd(d), warnings.catch_warnings():
        warnings.simplefilter("ignore")
        try:
            res = klass(**{k: v for k, v in inputs.items() if v is not None}).run()
        except Exception as e:  # traits' and nipype's errors: compared by type and last line
            return last_line(e)
    return files_of(d), res.outputs


def run_lx(fn, d: Path, *args, notices: list | None = None, **kw):
    d.mkdir(parents=True)
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        try:
            out = fn(*args, out_dir=d, **kw) if "out_dir" not in kw else fn(*args, **kw)
        except Exception as e:
            out = last_line(e)
    if notices is not None:
        notices += [
            w.message.entry for w in caught if isinstance(w.message, lx.UpstreamDefectWarning)
        ]
    if isinstance(out, str):
        return out
    return files_of(d), out


def error_kind(text: str) -> str:
    """nipype's ``TraitError`` and larmorx's ``ValueError`` for the same refused input."""
    kind, _, msg = text.partition(": ")
    return f"{'ValueError' if kind == 'TraitError' else kind}: {msg}"


def compare_files(key: str, want, got, what: str) -> None:
    if isinstance(want, str) or isinstance(got, str):
        v = (
            "identical"
            if isinstance(want, str)
            and isinstance(got, str)
            and (error_kind(want) == error_kind(got))
            else "differ"
        )
        count(
            key,
            v,
            f"{what}: {want if isinstance(want, str) else 'ok'} | "
            f"{got if isinstance(got, str) else 'ok'}",
        )
        return
    count(key, "identical" if want[0] == got[0] else "differ", what)


# --- tsnr ----------------------------------------------------------------------------------------


def nipype_tsnr_float32(in_file, regress_poly, d: Path) -> dict[str, str]:
    """nipype 1.12.0's ``TSNR._run_interface`` (Apache-2.0, the nipype developers) with its
    float32 header switch for integer data made effective (the check PIPE-51 corrects): the files
    larmorx's default writes for an integer-typed series."""
    d.mkdir(parents=True)
    files = [in_file] if isinstance(in_file, (str, Path)) else list(in_file)
    img = nb.load(files[0])
    header = img.header.copy()
    vollist = [nb.load(f) for f in files]
    data = np.concatenate(
        [v.get_fdata(dtype=np.float32).reshape((*v.shape[:3], -1)) for v in vollist], axis=3
    )
    data = np.nan_to_num(data)
    if header.get_data_dtype().kind in "iu":
        header.set_data_dtype(np.float32)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        if regress_poly is not None:
            data = nac.regress_poly(regress_poly, data, remove_mean=False)[0]
            nb.save(nb.Nifti1Image(data, img.affine, header), d / "detrend.nii.gz")
        meanimg = np.mean(data, axis=3)
        stddevimg = np.std(data, axis=3)
        tsnr = np.zeros_like(meanimg)
        nz = stddevimg > 1.0e-3
        tsnr[nz] = meanimg[nz] / stddevimg[nz]
    for name, arr in (("tsnr", tsnr), ("mean", meanimg), ("stdev", stddevimg)):
        nb.save(nb.Nifti1Image(arr, img.affine, header), d / f"{name}.nii.gz")
    return files_of(d)


def tsnr_inputs(tmp: Path, tier: str) -> list[tuple[str, object]]:
    rng = np.random.default_rng(11)
    aff = np.diag([2.0, 2.0, 3.0, 1.0])
    out: list[tuple[str, object]] = []

    def save(name, arr, dtype=None, slope=None, tr=2.0, unit="sec", cls=nb.Nifti1Image):
        img = cls(arr, aff)
        if dtype is not None:
            img.set_data_dtype(dtype)
        if slope is not None:
            img.header.set_slope_inter(*slope)
        if arr.ndim >= 4:
            img.header.set_zooms((2.0, 2.0, 3.0, tr, *([1.0] * (arr.ndim - 4))))
            img.header.set_xyzt_units("mm", unit)
        p = tmp / name
        nb.save(img, p)
        return str(p)

    for t in (1, 2, 3, 20, 130) if tier == "full" else (1, 3, 20):
        base = rng.normal(800, 40, (7, 6, 5, t)) + np.linspace(0, 30, t)
        base = base.astype(np.float32)
        base[0, 0, 0] = 0.0  # a zero voxel
        base[1, 0, 0] = 555.0  # a constant voxel
        if t > 1:
            base[2, 0, 0] = 700.0 + 1e-3 * (np.arange(t) % 2)  # a deviation near 1e-3
        out.append((f"f32 T={t}", save(f"f32_{t}.nii.gz", base)))
        out.append((f"i16 T={t}", save(f"i16_{t}.nii.gz", base, dtype=np.int16)))
        if tier == "full" or t == 20:
            out.append((f"f64 T={t}", save(f"f64_{t}.nii.gz", base.astype(np.float64))))
            out.append((f"i16 scaled T={t}", save(f"i16s_{t}.nii.gz", base, np.int16, (0.37, 5.0))))
            out.append((f"u8 T={t}", save(f"u8_{t}.nii.gz", base / 5, dtype=np.uint8)))
            out.append((f"u8 scaled T={t}", save(f"u8s_{t}.nii", base, np.uint8, (4.0, -2.0))))
            out.append((f"i32 T={t}", save(f"i32_{t}.nii.gz", base * 1000, dtype=np.int32)))
            be = nb.Nifti1Image(base.astype(">f4"), aff)
            nb.save(be, tmp / f"be_{t}.nii.gz")
            out.append((f">f4 T={t}", str(tmp / f"be_{t}.nii.gz")))
            be = nb.Nifti1Image(base.astype(">i2"), aff)
            nb.save(be, tmp / f"bei_{t}.nii.gz")
            out.append((f">i2 T={t}", str(tmp / f"bei_{t}.nii.gz")))
    base = rng.normal(800, 40, (7, 6, 5, 20)).astype(np.float32)
    special = base.copy()
    special[0, 0, 0, 3] = np.nan
    special[1, 0, 0, 5] = np.inf
    special[2, 0, 0, 7] = -np.inf
    special[3, 0, 0] = 3.0e38
    out.append(("f32 NaN/inf/huge", save("special.nii.gz", special)))
    out.append(("f32 msec TR", save("msec.nii.gz", base, tr=2000.0, unit="msec")))
    out.append(("f32 NIfTI-2", save("n2.nii.gz", base, cls=nb.Nifti2Image)))
    out.append(("f32 3-D", save("v3.nii.gz", base[..., 0])))
    vols = [save(f"vol{i}.nii.gz", base[..., i]) for i in range(5)]
    out.append(("list of 3-D", vols))
    out.append(
        ("list of 4-D", [save("a4.nii.gz", base[..., :8]), save("b4.nii.gz", base[..., 8:])])
    )
    out.append(("3-D + 4-D", [vols[0], save("c4.nii.gz", base[..., 4:9])]))
    out.append(("i16 list", [save("ia.nii.gz", base[..., :6], np.int16), vols[1]]))
    out.append(("5-D", save("f5.nii.gz", rng.normal(800, 40, (4, 3, 2, 6, 2)).astype(np.float32))))
    out.append(("2-D", save("f2.nii.gz", base[:, :, 0, 0])))
    out.append(("mismatch", [vols[0], save("small.nii.gz", base[:5, ..., 1])]))
    return out


def tsnr_section(tier: str, tmp: Path) -> None:
    (tmp / "in").mkdir()
    inputs = tsnr_inputs(tmp / "in", tier)
    degrees = (None, 1, 2, 3, 4) if tier == "full" else (None, 1, 3)
    i = 0
    for name, f in inputs:
        for rp in degrees:
            i += 1
            kw = {} if rp is None else {"regress_poly": rp}
            want = run_nipype(nac.TSNR, tmp / f"n{i}", in_file=f, **kw)
            got = run_lx(
                lx.confounds.tsnr,
                tmp / f"l{i}",
                f,
                **kw,
                numpy_version=NP,
                upstream_defects="reproduce",
            )
            compare_files("tsnr", want, got, f"{name} regress_poly={rp}")
            if CONTROL or isinstance(want, str):
                continue
            notes: list = []
            fix = run_lx(lx.confounds.tsnr, tmp / f"x{i}", f, **kw, numpy_version=NP, notices=notes)
            for n in notes:
                NOTICES[n] += 1
            first = f if isinstance(f, str) else f[0]
            if nb.load(first).get_data_dtype().kind in "iu":
                expect = nipype_tsnr_float32(f, rp, tmp / f"e{i}")
                count(
                    "tsnr: default (PIPE-51) = nipype's float32 switch",
                    "identical"
                    if not isinstance(fix, str) and fix[0] == expect and notes == ["PIPE-51"]
                    else "differ",
                    f"{name} regress_poly={rp}",
                )
            else:
                count(
                    "tsnr: default = reproduce (float series)",
                    "identical"
                    if not isinstance(fix, str) and fix[0] == got[0] and not notes
                    else "differ",
                    f"{name} regress_poly={rp}",
                )
    # traits' Range(low=1): 0 and 2.5 refused, True accepted as 1.
    for kw in ({"regress_poly": 0}, {"regress_poly": True}, {"regress_poly": 2.5}):
        f = inputs[0][1]
        want = run_nipype(nac.TSNR, tmp / f"err{kw}", in_file=f, **kw)
        got = run_lx(
            lx.confounds.tsnr,
            tmp / f"lerr{kw}",
            f,
            **kw,
            numpy_version=NP,
            upstream_defects="reproduce",
        )
        compare_files("tsnr: regress_poly's trait", want, got, str(kw))
    # The output names given.
    f = inputs[0][1]
    names = dict(
        tsnr_file="t.nii", mean_file="m.nii.gz", stddev_file="s.nii", detrended_file="d.nii"
    )
    want = run_nipype(nac.TSNR, tmp / "names_n", in_file=f, regress_poly=2, **names)
    got = run_lx(
        lx.confounds.tsnr,
        tmp / "names_l",
        f,
        regress_poly=2,
        numpy_version=NP,
        upstream_defects="reproduce",
        **names,
    )
    compare_files("tsnr: output names", want, got, str(names))


# --- nssd ----------------------------------------------------------------------------------------


def nipype_signal(f: str) -> np.ndarray:
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        return nb.load(f).dataobj[:, :, :, :50].mean(axis=0).mean(axis=0).mean(axis=0)


def check_nssd(key: str, f: str, what: str) -> None:
    want = run_nipype(nac.NonSteadyStateDetector, Path(tempfile.mkdtemp(dir=TMP)) / "n", in_file=f)
    notes: list = []
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        try:
            got = lx.confounds.nonsteady_state_detector(
                f, numpy_version=NP, upstream_defects="reproduce"
            )
        except Exception as e:
            got = last_line(e)
    if isinstance(want, str) or isinstance(got, str):
        ok = isinstance(want, str) and isinstance(got, str) and want == got
        count(
            key,
            "identical" if ok else "differ",
            f"{what}: {want if isinstance(want, str) else 'ok'} | {got if isinstance(got, str) else 'ok'}",
        )
        return
    gs = nipype_signal(f)
    v = same((int(want[1].n_volumes_to_discard), gs), (got.n_volumes_to_discard, got.global_signal))
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        v2 = nac.is_outlier(gs) == got.n_volumes_to_discard
    count(key, v if v2 else "differ", what)
    if CONTROL:
        return
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        fix = lx.confounds.nonsteady_state_detector(f, numpy_version=NP)
    notes = [w.message.entry for w in caught if isinstance(w.message, lx.UpstreamDefectWarning)]
    for n in notes:
        NOTICES[n] += 1
    if not notes:
        count(
            "nssd: default = reproduce without a notice",
            "identical" if fix.n_volumes_to_discard == got.n_volumes_to_discard else "differ",
            what,
        )
    else:
        APART[
            f"nssd: PIPE-23 corrected ({got.n_volumes_to_discard} -> {fix.n_volumes_to_discard})"
        ] += 1


def nssd_section(tier: str, tmp: Path) -> None:
    rng = np.random.default_rng(12)
    aff = np.eye(4)
    i = 0

    def save(arr, dtype=None, slope=None, ext=".nii.gz"):
        nonlocal i
        i += 1
        img = nb.Nifti1Image(arr, aff)
        if dtype is not None:
            img.set_data_dtype(dtype)
        if slope is not None:
            img.header.set_slope_inter(*slope)
        p = tmp / f"s{i}{ext}"
        nb.save(img, p)
        return str(p)

    shapes = [(7, 6, 5), (9, 2, 2), (200, 2, 2), (130, 9, 3)]
    if tier == "full":
        shapes += [(2, 2, 2), (9000, 1, 1), (8193, 2, 1), (64, 64, 3)]
    lengths = (1, 2, 3, 10, 49, 50, 51, 120) if tier == "full" else (1, 3, 50, 120)
    kinds = ("random", "bright", "constant", "half")
    types = [
        (np.float32, None),
        (np.float64, None),
        (np.int16, None),
        (np.uint8, None),
        (np.int16, (0.5, 3.0)),
    ]
    if tier == "full":
        types += [
            (np.int32, None),
            (np.int8, None),
            (np.uint16, None),
            (np.int64, None),
            (">f4", None),
            (">i2", None),
        ]
    for shape in shapes:
        for t in lengths:
            for kind in kinds:
                x = rng.normal(600, 30, (*shape, t))
                if kind == "bright":
                    x[..., :3] += np.array([400.0, 200.0, 90.0])[: min(3, t)]
                elif kind == "constant":
                    x = np.full((*shape, t), 321.0)
                elif kind == "half":
                    x[..., : (t + 1) // 2 + 1] = 500.0
                for dtype, slope in types if kind == "random" else types[:3]:
                    arr = x / 6 if np.dtype(dtype) in (np.uint8, np.int8) else x
                    arr = arr.astype(np.float32 if str(dtype)[0] in ">" else np.float64)
                    if str(dtype).startswith(">"):
                        arr = arr.astype(dtype)
                        f = save(arr)
                    else:
                        f = save(
                            arr, dtype=dtype, slope=slope, ext=".nii" if i % 5 == 0 else ".nii.gz"
                        )
                    check_nssd("nssd", f, f"{shape} T={t} {kind} {dtype} {slope}")
    check_nssd(
        "nssd: 5-D", save(rng.normal(600, 30, (4, 3, 2, 60, 2)).astype(np.float32)), "5-D float32"
    )
    check_nssd(
        "nssd: 5-D", save(rng.normal(600, 30, (4, 3, 2, 60, 3)), dtype=np.int16), "5-D int16"
    )
    check_nssd(
        "nssd: 3-D (IndexError)", save(rng.normal(600, 30, (4, 3, 2)).astype(np.float32)), "3-D"
    )


# --- fd ------------------------------------------------------------------------------------------


def fd_case(key: str, f: str, src: str, what: str, **kw) -> None:
    d = Path(tempfile.mkdtemp(dir=TMP))
    want = run_nipype(nac.FramewiseDisplacement, d / "n", in_file=f, parameter_source=src, **kw)
    got = run_lx(lx.confounds.framewise_displacement, d / "l", f, src, numpy_version=NP, **kw)
    if isinstance(want, str) or isinstance(got, str):
        compare_files(key, want, got, f"{what} {src}")
        return
    name = kw.get("out_file", "fd_power_2012.txt")
    v = (
        "identical"
        if (
            want[0].get(name) is not None
            and want[0] == got[0]
            and float(want[1].fd_average).hex() == got[1].fd_average.hex()
        )
        else "differ"
    )
    count(key, v, f"{what} {src} {kw}")


def fd_section(tier: str, tmp: Path) -> None:
    rng = np.random.default_rng(13)
    cols = {"FSL": (6, 7), "AFNI": (6, 7, 9), "SPM": (3, 5, 6, 8), "FSFAST": (6, 7)}
    lengths = (2, 3, 10, 300, 9001) if tier == "full" else (2, 10, 300, 9001)
    radii = (50, 50.0, 0.0, 35.5, -10.0) if tier == "full" else (50, 35.5)
    fmts = ("%.18e", "%.6f", "%g")
    i = 0
    for src, ncols in cols.items():
        for nc in ncols:
            for t in lengths:
                for scale in ((0.01, 0.5), (1e-6, 1e3)) if tier == "full" else ((0.01, 0.5),):
                    i += 1
                    x = np.cumsum(rng.normal(0, scale[0], (t, nc)), axis=0) * scale[1]
                    fmt = fmts[i % len(fmts)]
                    f = tmp / f"m{i}.txt"
                    np.savetxt(f, x, fmt=fmt, delimiter="\t" if i % 2 else " ")
                    for r in radii:
                        fd_case("fd", str(f), src, f"{nc} columns T={t} {fmt}", radius=r)
    x = rng.normal(0, 0.01, (12, 6))
    x[3, 2], x[7, 4] = np.nan, np.inf
    np.savetxt(tmp / "nan.txt", x)
    for src in cols:
        fd_case("fd: NaN/inf", str(tmp / "nan.txt"), src, "NaN/inf")
    fd_case("fd: output name", str(tmp / "m1.txt"), "FSL", "", out_file="fd.tsv")
    errors = {"one row": (1, 6), "one column": (10, 1), "five columns": (10, 5), "three": (10, 3)}
    for name, shape in errors.items():
        np.savetxt(tmp / f"{name}.txt", rng.normal(0, 0.01, shape))
        for src in cols:
            fd_case("fd: errors", str(tmp / f"{name}.txt"), src, name)
    (tmp / "empty.txt").write_text("")
    fd_case("fd: errors", str(tmp / "empty.txt"), "FSL", "empty")
    fd_case("fd: errors", str(tmp / "m1.txt"), "fsl", "lower-case source")
    # Counted apart: the plot (the NIPY source: section nipy).
    d = Path(tempfile.mkdtemp(dir=TMP))
    got = run_lx(
        lx.confounds.framewise_displacement, d / "p", str(tmp / "m1.txt"), "FSL", save_plot=True
    )
    APART[f"fd save_plot: larmorx {got}"] += 1
    # normalize_mc_params on single rows.
    for n in range(3, 13):
        row = rng.normal(0, 2, n)
        for src in ("FSL", "fsl", "AFNI", "Afni", "FSFAST", "SPM", "spm", "other"):
            for dtype in (np.float64, np.float32, np.int64):
                r = row.astype(dtype)
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore")
                    try:
                        want = nac.normalize_mc_params(r.copy(), src)
                    except Exception as e:
                        want = last_line(e)
                    try:
                        got = lx.confounds.normalize_mc_params(r.copy(), src)
                    except Exception as e:
                        got = last_line(e)
                count("normalize_mc_params", same(want, got), f"{n} {src} {dtype}")


# --- nipy ---------------------------------------------------------------------------------------

NIPY_FORMS = ("reproduce", "fix", "plain", "reproduce-npexp")
NIPY_MODULES: dict[str, object] = {}


def nipy_form(form: str):
    """nipype's ``normalize_mc_params`` will import nipy's functions in ``form``."""
    if form not in NIPY_MODULES:
        NIPY_MODULES[form] = ni.build(form)
    for parent in ("nipy", "nipy.algorithms"):
        sys.modules.setdefault(parent, type(sys)(parent))
    sys.modules["nipy.algorithms.registration"] = NIPY_MODULES[form]


def nipy_off() -> None:
    for name in ("nipy.algorithms.registration", "nipy.algorithms", "nipy"):
        sys.modules.pop(name, None)


def outcome_of(fn):
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        try:
            return fn()
        except Exception as e:
            return last_line(e)


def nipype_row(row, form: str):
    nipy_form(form)
    return outcome_of(lambda: np.asarray(nac.normalize_mc_params(np.array(row, copy=True), "NIPY")))


def lx_row(row, mode: str, notices: list):
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        try:
            out = np.asarray(
                lx.confounds.normalize_mc_params(
                    np.array(row, copy=True), "NIPY", numpy_version=NP, upstream_defects=mode
                )
            )
        except Exception as e:
            out = last_line(e)
    notices += [w.message.entry for w in caught if isinstance(w.message, lx.UpstreamDefectWarning)]
    return out


def takes_rust(row) -> bool:
    """Whether larmorx computes this row with its own arithmetic (``framewise._nipy_row``)."""
    a = np.asarray(row)
    if a.ndim != 1:
        return False
    if a.dtype == np.float64:
        return True
    return a.dtype.kind in "iu" and all(
        abs(int(v)) <= 2**25 for r in lx.confounds._nipy.squared(a) for v in r.tolist()
    )


def causes(refs: dict) -> str:
    """Why nipype as this machine runs it differs from ``reproduce``'s form: ``exp`` (numpy's
    own, NP-04) and/or ``atan2`` (glibc's, D4), each told by the form with numpy's own ``exp``
    and a correctly rounded ``atan2``."""
    why = []
    if same(refs["reproduce-npexp"], refs["reproduce"]) != "identical":
        why.append("exp (NP-04)")
    if same(refs["reproduce-npexp"], refs["plain"]) != "identical":
        why.append("atan2 (D4)")
    return " and ".join(why) or "unexplained"


def nipy_row_case(key: str, row, what: str) -> None:
    a = np.asarray(row)
    wraps = a.ndim == 1 and a.dtype.kind in "iu" and lx.confounds._nipy.squares_overflow(a)
    rust = takes_rust(row)  # a boolean row is nipy's (its Taylor form negates booleans)
    refs = {f: nipype_row(row, f) for f in NIPY_FORMS}
    notes = {"reproduce": [], "fix": []}
    got = {m: lx_row(row, m, notes[m]) for m in notes}
    for mode in ("reproduce", "fix"):
        if wraps and mode == "fix":
            # PIPE-60 corrected: the angle from the exact sum of squares. A small row is the
            # float64 row's (larmorx's own arithmetic: nipy's form "fix"); a large one follows
            # nipy's numpy code with exact squares, here checked against nipy's code on the
            # float64 row, whose squares and sums are exact for the values chosen.
            f64 = np.asarray(row, dtype=np.float64)
            want = nipype_row(f64, "fix") if rust else nipype_row(f64, "plain")
        elif rust and not wraps:
            want = refs[mode]
        else:
            want = refs["plain"]
        count(f"{key} ({mode})", same(want, got[mode]), f"{what} {mode}: {want} | {got[mode]}")
        pipe60 = "PIPE-60" in notes[mode]
        count(
            "nipy: PIPE-60 notice where the squares wrap",
            "identical" if pipe60 == wraps else "differ",
            f"{what} {mode}",
        )
        if pipe60:
            NOTICES[f"PIPE-60 ({mode})"] += 1
    if rust and not wraps:
        v = same(refs["plain"], got["reproduce"])
        if v != "identical":
            why = causes(refs)
            APART[f"nipy rows: nipype as run here differs from reproduce: {why}"] += 1
            if why == "unexplained":
                FAILS.append(f"{key}: {what}: {v}: {refs['plain']} | {got['reproduce']}")
        differs = same(got["fix"], got["reproduce"]) != "identical"
        for mode in ("reproduce", "fix"):
            named = "GLIBC-07" in notes[mode]
            count(
                "nipy: GLIBC-07 notice exactly where the modes differ",
                "identical" if named == differs else "differ",
                f"{what} {mode}: notice {named}, modes differ {differs}",
            )
            if named:
                NOTICES[f"GLIBC-07 nipy rows ({mode})"] += 1


def nipy_rows(tier: str) -> list[tuple[str, object, str]]:
    """``(key, row, what)`` for ``normalize_mc_params(row, "NIPY")``."""
    rng = np.random.default_rng(29)
    out = []
    scales = (1e-31, 1e-4, 1e-2, 0.3, 3.0, 50.0, 1e9, 1e11)
    n = 6 if tier == "full" else 2
    for k in (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 15):
        for sc in scales:
            for _ in range(n if k in (6, 7, 12, 13) else 1):
                row = rng.normal(0, sc, k)
                if k >= 7:
                    row[6:9] = rng.normal(0, 3, min(3, k - 6))
                if k > 9:
                    row[9:12] = rng.normal(0, sc, min(3, k - 9))
                out.append(("nipy rows: random", row, f"k={k} scale={sc}"))
    # More random rigid and scaled rows (the bulk: every path's last bits).
    m = 3000 if tier == "full" else 400
    for i in range(m):
        k = (6, 7, 12)[i % 3]
        row = np.concatenate([rng.normal(0, 2, 3), rng.normal(0, rng.choice((0.02, 0.5, 2.5)), 3)])
        if k == 7:
            row = np.append(row, rng.choice((-1, 1)) * rng.uniform(0.5, 2))
        if k == 12:
            row = np.concatenate([row, rng.normal(0, 1, 3), rng.normal(0, 0.5, 3)])
        out.append(("nipy rows: random", row, f"bulk k={k}"))
    # Boundaries.
    ma, sa = 1e10 * 2 * np.pi, 1e-30
    for v in (
        ma,
        np.nextafter(ma, 0),
        np.nextafter(ma, np.inf),
        sa,
        np.nextafter(sa, 0),
        np.nextafter(sa, 1),
        0.0,
        -0.0,
        5e-324,
    ):
        out.append(("nipy rows: boundaries", np.array([1, 2, 3, v, 0, 0], float), f"r0={v!r}"))
        out.append(("nipy rows: boundaries", np.array([1, 2, 3, 0, 0, -v], float), f"r2=-{v!r}"))
        out.append(
            (
                "nipy rows: boundaries",
                np.array([0, 0, 0, 0.1, 0.2, 0.3, 1, 1, 1, v], float),
                f"q0={v!r} (10)",
            )
        )
        out.append(
            (
                "nipy rows: boundaries",
                np.array([0, 0, 0, 0.1, 0.2, 0.3, 1, 1, 1, v, -v], float),
                f"q={v!r} (11)",
            )
        )
    for v in (np.pi / 2, -np.pi / 2, np.nextafter(np.pi / 2, 0), np.pi, 2 * np.pi):
        out.append(
            ("nipy rows: gimbal lock", np.array([0, 0, 0, 0, v, 0], float), f"y rotation {v!r}")
        )
        out.append(
            ("nipy rows: gimbal lock", np.array([0, 0, 0, 1e-9, v, 1e-9], float), f"near y {v!r}")
        )
    # Scales and log-scales.
    for sc in (-2.0, -1.0, -1e-300, 1e-300, 1e300, -1e300, 0.0, -0.0, np.nan, np.inf, -np.inf, 3.0):
        out.append(("nipy rows: scales", np.array([1, 2, 3, 0.1, -0.2, 0.3, sc]), f"scale {sc!r}"))
    for ls in (30.0, -30.0, 23.0258509299, -23.0258509299, 700.0, np.nan, np.inf, -np.inf, 0.0):
        out.append(
            (
                "nipy rows: scales",
                np.array([1, 2, 3, 0.1, -0.2, 0.3, ls, -ls, 1.0, 0.2, 0.1, -0.3]),
                f"log-scales ±{ls!r}",
            )
        )
    # NaN and infinite values in every position of 6, 7 and 12 values.
    for k in (6, 7, 12):
        for pos in range(k):
            for v in (np.nan, np.inf, -np.inf):
                row = np.linspace(0.05, 0.6, k)
                row[pos] = v
                out.append(("nipy rows: NaN/inf", row, f"k={k} [{pos}]={v}"))
    # Other types and shapes.
    base = np.array([1.5, -2.0, 0.25, 0.01, -0.02, 0.03])
    for dt in (np.float32, np.float16):
        for k in (6, 7, 12, 4):
            row = (np.resize(base, k) * (1 + np.arange(k) % 3)).astype(dt)
            out.append(("nipy rows: other types", row, f"{np.dtype(dt).name} k={k}"))
    for dt in (np.int8, np.int16, np.int32, np.int64, np.uint8, np.uint16, np.uint64, np.bool_):
        for vals in (
            [1, -2, 3, 0, 1, 0],
            [0, 0, 0, 0, 0, 0],
            [1, 2, 3, 2, 3, 1, 1],
            [5, 4, 3, 1, 0, 2, 1, 1, 1, 0, 1, 1],
        ):
            row = np.array(vals).astype(dt)
            out.append(("nipy rows: integer types", row, f"{np.dtype(dt).name} {vals}"))
    for dt, vals in (
        (np.int8, [0, 0, 0, 12, 0, 0]),
        (np.int8, [0, 0, 0, 16, 0, 0]),
        (np.int8, [0, 0, 0, 11, 11, 11]),
        (np.uint8, [0, 0, 0, 16, 1, 0]),
        (np.int16, [0, 0, 0, 200, 3, 1]),
        (np.int32, [0, 0, 0, 50000, 0, 1]),
        (np.int64, [0, 0, 0, 2_000_000_000, 2_000_000_000, 2_000_000_000]),
        (np.int64, [0, 0, 0, 40_000_000, 0, 7]),
        (np.int64, [0, 0, 0, 33_554_432, 1, 0]),
        (np.int64, [0, 0, 0, 33_554_433, 1, 0]),
        (np.uint64, [0, 0, 0, 5_000_000_000, 0, 0]),
        (np.int8, [0, 0, 0, 1, 1, 1, 1, 1, 1, 12, 0, 0]),
    ):
        out.append(
            ("nipy rows: integer types", np.array(vals, dtype=dt), f"{np.dtype(dt).name} {vals}")
        )
    out.append(("nipy rows: shapes", np.zeros((2, 6)), "2-D"))
    out.append(("nipy rows: shapes", np.array(0.5), "0-D"))
    out.append(("nipy rows: shapes", [1.0, 2.0, 3.0, 0.1, 0.2, 0.3], "list"))
    return out


def fd_nipy_case(key: str, f: str, what: str, **kw) -> None:
    d = Path(tempfile.mkdtemp(dir=TMP))
    refs = {}
    for form in NIPY_FORMS:
        nipy_form(form)
        refs[form] = run_nipype(
            nac.FramewiseDisplacement, d / f"n-{form}", in_file=f, parameter_source="NIPY", **kw
        )
    notes = {"reproduce": [], "fix": []}
    got = {
        m: run_lx(
            lx.confounds.framewise_displacement,
            d / f"l-{m}",
            f,
            "NIPY",
            numpy_version=NP,
            upstream_defects=m,
            notices=notes[m],
            **kw,
        )
        for m in notes
    }

    def verdict(want, have) -> str:
        if isinstance(want, str) or isinstance(have, str):
            return (
                "identical"
                if isinstance(want, str) and isinstance(have, str) and want == have
                else "differ"
            )
        return (
            "identical"
            if want[0] == have[0] and float(want[1].fd_average).hex() == have[1].fd_average.hex()
            else "differ"
        )

    for m in notes:
        count(
            f"{key} ({m})",
            verdict(refs[m], got[m]),
            f"{what} {m}: {refs[m] if isinstance(refs[m], str) else 'ok'} | {got[m] if isinstance(got[m], str) else 'ok'}",
        )
    if verdict(refs["plain"], got["reproduce"]) != "identical":
        why = []
        if verdict(refs["reproduce-npexp"], refs["reproduce"]) != "identical":
            why.append("exp (NP-04)")
        if verdict(refs["reproduce-npexp"], refs["plain"]) != "identical":
            why.append("atan2 (D4)")
        APART[
            f"{key}: nipype as run here differs from reproduce: {' and '.join(why) or 'unexplained'}"
        ] += 1
    differs = verdict(got["fix"], got["reproduce"]) != "identical"
    for m in notes:
        named = "GLIBC-07" in notes[m]
        count(
            "nipy fd: GLIBC-07 notice exactly where the modes differ",
            "identical" if named == differs else "differ",
            f"{what} {m}: notice {named}, modes differ {differs}",
        )
        if named:
            NOTICES[f"GLIBC-07 nipy fd ({m})"] += 1


def nipy_section(tier: str, tmp: Path) -> None:
    print(f"nipy: numpy's exp is glibc's here: {ni.numpy_exp_is_glibc()}", flush=True)
    for key, row, what in nipy_rows(tier):
        nipy_row_case(key, row, what)
    rng = np.random.default_rng(31)
    lengths = (2, 3, 10, 300, 9001) if tier == "full" else (2, 10, 300)
    radii = (50, 50.0, 0.0, 35.5, -10.0) if tier == "full" else (50, 35.5)
    i = 0
    for nc in (6, 7, 12, 13, 15):
        for t in lengths:
            i += 1
            x = np.cumsum(rng.normal(0, 0.01, (t, nc)), axis=0)
            x[:, 3:6] *= 5
            if nc == 7:
                x[:, 6] = (-1) ** i * (1 + 0.05 * rng.normal(size=t))
            f = tmp / f"nipy{i}.txt"
            np.savetxt(f, x, fmt=("%.18e", "%.6f", "%g")[i % 3], delimiter="\t" if i % 2 else " ")
            for r in radii if t < 9001 else radii[:2]:
                fd_nipy_case("nipy fd", str(f), f"{nc} columns T={t}", radius=r)
    for nc in (3, 4, 5, 8, 9, 10, 11):
        x = rng.normal(0, 0.1, (12, nc))
        f = tmp / f"nipy-err{nc}.txt"
        np.savetxt(f, x)
        fd_nipy_case("nipy fd: errors", str(f), f"{nc} columns")
    for nc in (10, 11):
        x = rng.normal(0, 0.1, (12, nc))
        x[:, 9:] = 1e11
        f = tmp / f"nipy-big{nc}.txt"
        np.savetxt(f, x)
        fd_nipy_case("nipy fd: a second vector above MAX_ANGLE", str(f), f"{nc} columns")
    x = rng.normal(0, 0.1, (12, 6))
    x[5, 4] = np.nan
    np.savetxt(tmp / "nipy-nan.txt", x)
    fd_nipy_case("nipy fd: errors", str(tmp / "nipy-nan.txt"), "a NaN rotation")
    x[5, 4] = 0.1
    x[7, 1] = np.nan
    np.savetxt(tmp / "nipy-nant.txt", x)
    fd_nipy_case("nipy fd: NaN translation", str(tmp / "nipy-nant.txt"), "a NaN translation")
    fd_nipy_case("nipy fd: output name", str(tmp / "nipy1.txt"), "out_file", out_file="fd.tsv")
    nipy_off()
    fd_case("nipy fd: errors", str(tmp / "nipy1.txt"), "nipy", "lower-case source")


# --- robust --------------------------------------------------------------------------------------


def robust_section(tier: str, tmp: Path) -> None:
    f = iface_inputs(tmp)
    cases = [
        dict(
            mask_files=[f["m1"]],
            pre_filter="cosine",
            repetition_time=2.0,
            variance_threshold=0.5,
            save_pre_filter=True,
            save_metadata=True,
            failure_mode="NaN",
        ),
        dict(
            mask_files=[f["m1"], f["m2"]],
            merge_method="none",
            mask_names=["a", "b"],
            num_components=3,
            save_metadata=True,
            ignore_initial_volumes=2,
        ),
        dict(pre_filter="polynomial", regress_poly_degree=2, num_components="all"),
    ]
    for i, case in enumerate(cases):
        for kname, klass, fn in (
            ("RobustACompCor", nwp.RobustACompCor, lx.niworkflows.robust_acompcor),
            ("RobustTCompCor", nwp.RobustTCompCor, lx.niworkflows.robust_tcompcor),
        ):
            want = run_nipype(klass, tmp / f"n{kname}{i}", realigned_file=f["bold"], **case)
            got = run_lx(
                fn,
                tmp / f"l{kname}{i}",
                f["bold"],
                **case,
                numpy_version=NP,
                upstream_defects="reproduce",
            )
            compare_files(f"robust: {kname}", want, got, str(case))
    # PIPE-48 measured: niworkflows' retries when ACompCor raises LinAlgError.
    attempts, sleeps = [], []
    real_run, real_sleep = nac.CompCor._run_interface, nwp.sleep

    def failing(self, runtime):
        attempts.append(1)
        raise np.linalg.LinAlgError("SVD did not converge")

    nac.CompCor._run_interface, nwp.sleep = failing, sleeps.append
    try:
        res = run_nipype(nwp.RobustACompCor, tmp / "retry", realigned_file=f["bold"], **cases[0])
    finally:
        nac.CompCor._run_interface, nwp.sleep = real_run, real_sleep
    ok = len(attempts) == 11 and all(10 * k - 6 <= s <= 10 * k for k, s in enumerate(sleeps, 1))
    APART[
        f"robust: PIPE-48 niworkflows' attempts {len(attempts)}, sleeps {sum(sleeps)} s ({'as documented' if ok else 'NOT as documented'}), then {res}"
    ] += 1


# --- real ----------------------------------------------------------------------------------------


QUICK_REAL = (
    "ds000005",
    "ds005040",
    "ds002603-sub-01-noreconall/",
    "ds000258",
    "ds004958-sub-fsm64ps-noreconall/",
)


def raw_bolds(tier: str) -> list[tuple[str, str]]:
    out, seen = [], set()
    for info in sorted(RUNS.glob("*/RUN_INFO.txt")):
        run = info.parent.name
        ds = run.split("-")[0]
        if ds in seen or (
            not any(f"{run}/".startswith(q) or run.startswith(q) for q in QUICK_REAL)
            and tier == "quick"
        ):
            continue
        line = [
            x for x in info.read_text(encoding="utf-8").splitlines() if x.startswith("BIDS input")
        ]
        bids = Path(line[0].split(": ", 1)[1].strip())
        bolds = sorted(bids.glob("sub-*/func/*_bold.nii.gz")) + sorted(
            bids.glob("sub-*/ses-*/func/*_bold.nii.gz")
        )
        bolds = [b for b in bolds if nb.load(b).ndim == 4 and nb.load(b).shape[3] > 5]
        if bolds:
            seen.add(ds)
            out.append((run, str(bolds[0])))
    return out


def real_motion(tier: str) -> list[tuple[str, str, str]]:
    out = []
    tsvs = sorted(RUNS.glob("*/out/sub-*/**/func/*desc-confounds_timeseries.tsv"))
    for tsv in tsvs if tier == "full" else tsvs[:6]:
        with open(tsv, encoding="utf-8") as fh:
            rows = list(csv.DictReader(fh, delimiter="\t"))
        trans = [[float(r[f"trans_{a}"]) for a in "xyz"] for r in rows]
        rot = [[float(r[f"rot_{a}"]) for a in "xyz"] for r in rows]
        spm = TMP / f"{tsv.stem}.spm.txt"
        fsl = TMP / f"{tsv.stem}.fsl.par"
        np.savetxt(spm, np.hstack([trans, rot]))
        np.savetxt(fsl, np.hstack([rot, trans]), fmt="%.6f")
        out += [
            (f"fMRIPrep {tsv.parent.parent.name} {tsv.stem[:40]}", str(spm), "SPM"),
            (f"fMRIPrep {tsv.stem[:40]} as FSL", str(fsl), "FSL"),
        ]
    for par in sorted(FIGCACHE.glob("mri-hmc/*/*/*.par"))[:2]:
        out.append((f"mcflirt {par.parent.parent.name}/{par.parent.name}", str(par), "FSL"))
    for oned in sorted(FIGCACHE.glob("afni-volreg/*/*/o.1D"))[:3]:
        out.append((f"3dvolreg {oned.parent.parent.name}/{oned.parent.name}", str(oned), "AFNI"))
        x = np.loadtxt(oned)
        seven = TMP / f"{oned.parent.name}.dfile.1D"
        np.savetxt(seven, np.hstack([np.arange(len(x))[:, None], x]), fmt="%g")
        out.append((f"3dvolreg {oned.parent.name} with an index column", str(seven), "AFNI"))
    return out


def real_section(tier: str, tmp: Path) -> None:
    print("real: case, nipype s, larmorx s, verdict")
    for run, bold in raw_bolds(tier):
        shape = nb.load(bold).shape
        big = np.prod(shape) > 3e8
        for rp in (None,) if big else (None, 1):
            kw = {} if rp is None else {"regress_poly": rp}
            t0 = time.perf_counter()
            want = run_nipype(nac.TSNR, tmp / f"tn{run}{rp}", in_file=bold, **kw)
            t1 = time.perf_counter()
            got = run_lx(
                lx.confounds.tsnr,
                tmp / f"tl{run}{rp}",
                bold,
                **kw,
                numpy_version=NP,
                upstream_defects="reproduce",
                n_threads=8,
            )
            t2 = time.perf_counter()
            compare_files("real: TSNR", want, got, f"{run} regress_poly={rp}")
            print(
                f"  TSNR {run} {shape} regress_poly={rp}: {t1 - t0:.2f} s, {t2 - t1:.2f} s, "
                f"{'identical' if not isinstance(want, str) and not isinstance(got, str) and want[0] == got[0] else 'DIFFER'}",
                flush=True,
            )
            for d in (tmp / f"tn{run}{rp}", tmp / f"tl{run}{rp}"):
                for p in d.iterdir():
                    p.unlink()
        t0 = time.perf_counter()
        check_nssd("real: NonSteadyStateDetector", bold, run)
        print(
            f"  NonSteadyStateDetector {run}: {time.perf_counter() - t0:.2f} s (both)", flush=True
        )
    for name, f, src in real_motion(tier):
        fd_case("real: FramewiseDisplacement", f, src, name)
    t0 = time.perf_counter()
    for name, f, _src in real_motion(tier):
        fd_nipy_case("real: FramewiseDisplacement NIPY", f, name)
    nipy_off()
    print(
        f"  FramewiseDisplacement NIPY of the real tables: {time.perf_counter() - t0:.1f} s (all forms)"
    )


def main() -> int:
    global CONTROL, NP, TMP
    p = argparse.ArgumentParser()
    p.add_argument("--tier", choices=("quick", "full"), default="quick")
    p.add_argument("--sections", default="tsnr,nssd,fd,nipy,robust,real")
    p.add_argument("--control", action="store_true")
    a = p.parse_args()
    CONTROL = a.control
    if CONTROL:
        NP = "2.2.6" if tuple(int(v) for v in NUMPY.split(".")[:2]) >= (2, 3) else "2.5.4"
    import nipype
    import niworkflows

    print(
        f"numpy {NUMPY}, nipype {nipype.__version__}, niworkflows {niworkflows.__version__}, "
        f"nibabel {nb.__version__}, larmorx told numpy {NP}, tier {a.tier}",
        flush=True,
    )
    with tempfile.TemporaryDirectory(prefix="lx-check-misc-") as tmp:
        TMP = Path(tmp)
        for s in a.sections.split(","):
            t0 = time.perf_counter()
            d = TMP / s
            d.mkdir()
            {
                "tsnr": tsnr_section,
                "nssd": nssd_section,
                "fd": fd_section,
                "nipy": nipy_section,
                "robust": robust_section,
                "real": real_section,
            }[s](a.tier, d)
            print(f"section {s}: {time.perf_counter() - t0:.1f} s", flush=True)
    for key, (i, nb_, d) in COUNTS.items():
        print(f"  {key:52s} identical {i:6d}  nan-bits {nb_:4d}  differ {d:5d}")
    for key, n in APART.items():
        print(f"  apart: {key} ({n})")
    if NOTICES:
        print(
            "  default's notices (cases): "
            + ", ".join(f"{k} {v}" for k, v in sorted(NOTICES.items()))
        )
    for f in FAILS:
        print("  FAIL " + f)
    differ = sum(c[2] for c in COUNTS.values())
    print(
        f"{'CONTROL' if CONTROL else 'CHECK'} {'PASS' if differ == 0 and not FAILS else 'DIFFER'}: {differ} cases differ"
    )
    return 0 if CONTROL or (differ == 0 and not FAILS) else 1


TMP = Path(tempfile.gettempdir())

if __name__ == "__main__":
    sys.exit(main())
