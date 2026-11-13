# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's motion parameters and root mean square deviation: larmorx against this
environment's fMRIPrep (25.2.7 and 25.2.6, from the tags), niworkflows, nitransforms, SciPy and
numpy, bit for bit.

Sections:

- ``rotation``: SciPy's ``Rotation.from_matrix`` against ``_core.scipy_rotation_from_matrix``:
  the quaternions bit for bit (C- and F-ordered stacks: fMRIPrep 25.2.7's ``M`` and 25.2.6's
  ``M.T``) on rotations, rotations rounded to float32 (the head-motion transforms' kind, which
  SciPy orthogonalises by SVD), orthogonal matrices that pass the Gramian check, scaled, sheared
  and nearly singular matrices, each of Shepperd's four branches, stacks of 1 to 600; SciPy's
  errors (a zero or negative determinant with its message, NaN and infinite matrices).
  ``Rotation.as_euler`` against ``_core.scipy_rotation_as_euler`` on SciPy's own quaternions,
  every one of the twelve sequences, intrinsic and extrinsic, gimbal lock included: SciPy calls
  the C library's ``atan2`` and ``hypot``, larmorx the correctly rounded ones (D4), so the check
  runs a model of SciPy's ``_get_angles`` twice: with glibc's functions (``ctypes``), which must
  give SciPy's angles bit for bit (the model is exact), and with CORE-MATH's ``atan2``
  (``oracles/core-math-preload``) and an exact ``hypot`` (integer square root), which must give
  larmorx's; the angles where the two differ are counted apart by cause (``atan2``, ``hypot``).
- ``itk``: nitransforms' ``linear.load(path).matrix`` against larmorx's reading (ITK text files
  written by the check: ``%g``, ``%.9g`` and ``repr`` values, fixed parameters, blank lines,
  ``AffineTransform_float_3_3`` and ``_double_``: PIPE-73), the errors (no banner, a missing line,
  a wrong number of parameters, a non-integer index, a singular matrix, a last row not
  ``(0, 0, 0, 1)``, no transform) by type and last line; one transform (PIPE-71).
- ``fsl``: nitransforms' ``FSLLinearTransformArray.from_ras(ras, reference=img, moving=img)``
  parameters against ``_core.nitransforms_fsl_from_ras`` on oblique affines of both handedness,
  anisotropic and tiny or huge voxels, several first-axis sizes; a singular affine (numpy's
  ``LinAlgError``).
- ``iface``: fMRIPrep 25.2.7's and 25.2.6's ``FSLMotionParams`` and ``FSLRMSDeviation`` and
  niworkflows' copies (run in a temporary folder) against ``lx.confounds.fsl_motion_params`` and
  ``fsl_rms_deviation`` with ``upstream_defects="reproduce"`` and the release's
  ``fmriprep_version``: references of every NIfTI type (float32, float64, int16, uint8, a scaled
  int16, big-endian float32 above and below 8,192 voxels: numpy's buffered sums), both
  handedness, 2-D and 4-D references (numpy's ``matmul`` errors), a constant one and one with a
  NaN voxel (PIPE-72), one volume (PIPE-71), double-precision transforms (PIPE-73), gimbal
  lock, a reflection (SciPy's ``ValueError``), 2 to 600 volumes; the table's text (gzipped for a
  ``.nii.gz`` reference) where every ``trans_*`` and ``rmsd`` cell is identical and every ``rot_*``
  cell is the model's (``rotation``) with the correctly rounded functions, the cells glibc's
  functions change counted apart (D4); errors by type and last line. The default against the
  corrections (PIPE-71, PIPE-72, PIPE-73).
- ``real``: fMRIPrep's own ``motion_params`` and ``rmsd`` nodes of the 36 confounds workflows
  of 21 oracle runs (fMRIPrep 25.2.6 in its image) with their own inputs: larmorx with
  ``fmriprep_version="25.2.6"``, ``numpy_version="2.2.6"`` and ``reproduce`` against the node's
  file (D4's cells counted apart, as ``iface``); and in every environment fMRIPrep 25.2.7's and
  25.2.6's interfaces run again on the same inputs against larmorx with this numpy.

Run it in each environment with the worktree's extension::

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 PYTHONPATH=<worktree>/python \\
        python -P check_motion.py [--tier quick|full] [--sections ...]

(``LARMORX_WORKSPACE``: the workspace, for the oracle runs, the tags and the CORE-MATH library).
Exit status 1 when any case differs.
"""

from __future__ import annotations

import argparse
import contextlib
import ctypes
import gzip
import importlib.util
import logging
import math
import os
import sys
import tempfile
import time
import warnings
from collections import defaultdict
from fractions import Fraction
from pathlib import Path

import nibabel as nb
import numpy as np
import scipy
from scipy.spatial.transform import Rotation

import larmorx as lx
from larmorx import _core
from larmorx.confounds import motion as lm

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))
RUNS = WORKSPACE / "oracles" / "fmriprep-25.2.6"
TAGS = WORKSPACE / "tags"
NUMPY = np.__version__
COUNTS: dict[str, list[int]] = defaultdict(lambda: [0, 0])
APART: dict[str, int] = defaultdict(int)
FAILS: list[str] = []
SECONDS: dict[str, float] = defaultdict(float)
logging.getLogger("nipype.interface").setLevel(logging.ERROR)
logging.getLogger("nipype.workflow").setLevel(logging.ERROR)

_libm = ctypes.CDLL("libm.so.6")
_cr = ctypes.CDLL(str(WORKSPACE / "oracles" / "core-math-preload" / "libcrmath.so"))
for _lib, _names in ((_libm, ("atan2", "hypot")), (_cr, ("cr_atan2",))):
    for _n in _names:
        getattr(_lib, _n).restype = ctypes.c_double
        getattr(_lib, _n).argtypes = [ctypes.c_double, ctypes.c_double]


def count(key: str, ok: bool, what: str) -> None:
    COUNTS[key][0 if ok else 1] += 1
    if not ok and len(FAILS) < 80:
        FAILS.append(f"{key}: {what}")


def last_line(e: BaseException) -> str:
    msg = str(e).strip().splitlines()
    kind = type(e).__name__
    kind = "ValueError" if kind in ("TraitError",) else kind
    return f"{kind}: {msg[-1] if msg else ''}"


def bits(a) -> bytes:
    return np.ascontiguousarray(np.asarray(a, dtype=np.float64)).tobytes()


@contextlib.contextmanager
def cwd(path: Path):
    old = os.getcwd()
    os.chdir(path)
    try:
        yield
    finally:
        os.chdir(old)


def content(p: Path) -> bytes:
    data = p.read_bytes()
    return gzip.decompress(data) if p.name.endswith(".gz") else data


# --- the model of SciPy's as_euler -------------------------------------------------------------


def exact_hypot(x: float, y: float) -> float:
    """``sqrt(x² + y²)`` correctly rounded (exact squares, an integer square root with a sticky
    half unit: Python's ``int / int`` rounds correctly)."""
    if math.isinf(x) or math.isinf(y):
        return math.inf
    if math.isnan(x) or math.isnan(y):
        return math.nan
    s = Fraction(x) ** 2 + Fraction(y) ** 2
    if s == 0:
        return 0.0
    n, d = s.numerator, s.denominator
    k = d.bit_length() - 1  # d is a power of two
    t = max(0, 130 - n.bit_length())
    if (t + k) % 2:
        t += 1
    big = n << t
    r = math.isqrt(big)
    half = (t + k) // 2
    if r * r == big:
        return r / (1 << half) if half >= 0 else float(r << -half)
    return (2 * r + 1) / (1 << (half + 1))


GLIBC = (_libm.atan2, _libm.hypot)
CR = (_cr.cr_atan2, exact_hypot)


def model_euler(q, seq: str, atan2, hypot) -> tuple[list[float], bool]:
    """SciPy 1.18.1's ``_compute_euler_from_quat`` and ``_get_angles`` of one quaternion with
    the given C functions (the same arithmetic in SciPy 1.15)."""
    extrinsic = seq.islower()
    s = seq.lower()
    if not extrinsic:
        s = s[::-1]
    i, j, k = ("xyz".index(c) for c in s)
    symmetric = i == k
    if symmetric:
        k = 3 - i - j
    sign = (i - j) * (j - k) * (k - i) // 2
    q = [float(v) for v in q]
    if symmetric:
        a, b, c, d = q[3], q[i], q[j], q[k] * sign
    else:
        a, b, c, d = q[3] - q[j], q[i] + q[k] * sign, q[j] + q[3], q[k] * sign - q[i]
    ang = [0.0, 0.0, 0.0]
    ang[1] = 2 * atan2(hypot(c, d), hypot(a, b))
    first, third = (0, 2) if extrinsic else (2, 0)
    case = 1 if abs(ang[1]) <= 1e-7 else 2 if abs(ang[1] - math.pi) <= 1e-7 else 0
    hs, hd = atan2(b, a), atan2(d, c)
    if case == 0:
        ang[first] = hs - hd
        ang[third] = hs + hd
    else:
        ang[2] = 0.0
        ang[0] = 2 * hs if case == 1 else 2 * hd * (-1 if extrinsic else 1)
    if not symmetric:
        ang[third] *= sign
        ang[1] -= math.pi / 2
    for t in range(3):
        if ang[t] < -math.pi:
            ang[t] += 2 * math.pi
        elif ang[t] > math.pi:
            ang[t] -= 2 * math.pi
    return ang, case != 0


def euler_causes(q, seq: str) -> tuple[list[float], list[float], set[str]]:
    """The model's angles with glibc's functions and with the correctly rounded ones, and which
    function changes them (``atan2``, ``hypot``)."""
    g, _ = model_euler(q, seq, *GLIBC)
    c, _ = model_euler(q, seq, *CR)
    why = set()
    if bits(g) != bits(c):
        if bits(model_euler(q, seq, CR[0], GLIBC[1])[0]) != bits(c):
            why.add("hypot")
        if bits(model_euler(q, seq, GLIBC[0], CR[1])[0]) != bits(c):
            why.add("atan2")
        if not why:
            why.add("atan2 and hypot together")
    return g, c, why


# --- rotation ----------------------------------------------------------------------------------

SEQS = [
    "XYZ", "XZY", "YXZ", "YZX", "ZXY", "ZYX", "XYX", "XZX", "YXY", "YZY", "ZXZ", "ZYZ",
]  # fmt: skip


def rot(rng: np.random.Generator, scale: float) -> np.ndarray:
    return Rotation.from_rotvec(rng.standard_normal(3) * scale).as_matrix()


def matrices(rng: np.random.Generator, n: int) -> dict[str, np.ndarray]:
    """Stacks of 3 × 3 matrices of each kind."""
    out = {}
    out["small rotations, float32"] = np.stack(
        [rot(rng, 0.02).astype(np.float32).astype(np.float64) for _ in range(n)]
    )
    out["rotations"] = np.stack([rot(rng, 1.5) for _ in range(n)])
    out["large rotations, float32"] = np.stack(
        [rot(rng, 3.0).astype(np.float32).astype(np.float64) for _ in range(n)]
    )
    out["identity and signed zeros"] = np.stack(
        [np.eye(3), np.where(np.eye(3) == 0, -0.0, 1.0), np.diag([1.0, 1, 1])]
    )
    s = []
    for _ in range(n):
        m = rot(rng, 1.0) * rng.uniform(0.5, 2.0, 3)
        m[0, 1] += rng.uniform(-0.2, 0.2)
        s.append(m)
    out["scaled and sheared"] = np.stack(s)
    near = []
    for _ in range(max(n // 4, 2)):
        m = rot(rng, 1.0)
        m[2] = m[1] * (1 + 1e-9) + m[2] * 1e-6
        near.append(m)
    out["nearly singular"] = np.stack(near)
    # Shepperd's four branches: the largest of m00, m11, m22, the trace.
    branches = []
    for axis in range(3):
        v = np.zeros(3)
        v[axis] = 3.0
        branches += [Rotation.from_rotvec(v + rng.standard_normal(3) * 0.05).as_matrix()]
    branches.append(np.eye(3) + 1e-9)
    out["Shepperd's branches"] = np.stack(branches)
    g = []
    for sign in (1, -1):
        g.append(Rotation.from_euler("XYZ", [0.3, sign * np.pi / 2, 0.2]).as_matrix())
        g.append(Rotation.from_euler("ZXZ", [0.3, 0.0, 0.2]).as_matrix())
    out["gimbal lock"] = np.stack(g)
    return out


def scipy_quats(m: np.ndarray, order: str):
    """SciPy's quaternions of the stack (``order`` "F": ``m`` given as a transposed view, as
    fMRIPrep 25.2.6 passes it), or its error's last line."""
    arg = np.ascontiguousarray(m.transpose(0, 2, 1)).transpose(0, 2, 1) if order == "F" else m
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            return Rotation.from_matrix(arg).as_quat()
    except Exception as e:
        return last_line(e)


def lx_quats(m: np.ndarray, order: str):
    try:
        return lm._rotations(m, NUMPY, order)
    except Exception as e:
        return last_line(e)


def rotation_section(tier: str, tmp: Path) -> None:
    rng = np.random.default_rng(20261112)
    n = 60 if tier == "quick" else 600
    for name, stack in matrices(rng, n).items():
        for order in ("C", "F"):
            for part in (stack, stack[:1]):
                want, got = scipy_quats(part, order), lx_quats(part, order)
                if isinstance(want, str) or isinstance(got, str):
                    count(f"rotation from_matrix {name}", want == got, f"{want} | {got}")
                    continue
                for r in range(len(part)):
                    ok = bits(want[r]) == bits(got[r])
                    count(f"rotation from_matrix {name}", ok, f"{order} {len(part)} {r}")
    bad = {
        "a reflection": np.diag([1.0, -1.0, 1.0]),
        "a zero matrix": np.zeros((3, 3)),
        "a rank-2 matrix": np.array([[1.0, 0, 0], [0, 1, 0], [1, 1, 0]]),
        "NaN": np.full((3, 3), np.nan),
        "an infinite element": np.array([[np.inf, 0, 0], [0, 1, 0], [0, 0, 1.0]]),
        "a NaN element": np.array([[1.0, np.nan, 0], [0, 1, 0], [0, 0, 1.0]]),
    }
    hangs = tuple(int(v) for v in NUMPY.split(".")[:2]) < (2, 5)
    for name, m in bad.items():
        for stack in (np.stack([np.eye(3), m]), np.stack([np.eye(3), m, np.diag([-1.0, 1, 1])])):
            if hangs and "infinite" in name and len(stack) == 2:
                # NP-10: numpy 2.3.5's and 2.2.6's SVD of a matrix holding an infinity hangs.
                APART["rotation from_matrix: an infinite matrix not run (NP-10: numpy hangs)"] += 1
                continue
            for order in ("C", "F"):
                want, got = scipy_quats(stack, order), lx_quats(stack, order)
                ok = want == got if isinstance(want, str) else bits(want) == bits(got)
                count("rotation from_matrix errors", ok, f"{name} {order}: {want} | {got}")
    # as_euler on SciPy's own quaternions.
    quats = []
    for stack in matrices(rng, n).values():
        q = scipy_quats(stack, "C")
        if not isinstance(q, str):
            quats.append(q)
    q = np.concatenate([*quats, Rotation.random(4 * n, rng=rng).as_quat()])
    for seq in SEQS + [s.lower() for s in SEQS]:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            want = Rotation.from_quat(q).as_euler(seq)
            stored = Rotation.from_quat(q).as_quat()
        got, _ = _core.scipy_rotation_as_euler(np.ascontiguousarray(stored), seq)
        got = np.asarray(got)
        for r in range(len(stored)):
            g, c, why = euler_causes(stored[r], seq)
            count("rotation as_euler model = SciPy", bits(g) == bits(want[r]), f"{seq} {r}")
            count("rotation as_euler larmorx = model (CR)", bits(c) == bits(got[r]), f"{seq} {r}")
            for w in why:
                APART[f"rotation as_euler: glibc's {w} changes the angles (D4)"] += 1


# --- ITK files ---------------------------------------------------------------------------------

BANNER = "#Insight Transform File V1.0"


def itk_text(params: np.ndarray, fixed: np.ndarray, fmt: str, kind: str = "float") -> str:
    lines = [BANNER]
    for i, (p, c) in enumerate(zip(params, fixed, strict=True)):
        f = (lambda v: repr(float(v))) if fmt == "repr" else (lambda v: fmt % v)
        lines += [
            f"#Transform {i}",
            f"Transform: AffineTransform_{kind}_3_3",
            "Parameters: " + " ".join(f(v) for v in p),
            "FixedParameters: " + " ".join(f(v) for v in c),
            "",
        ]
    return "\n".join(lines)


def motion_series(rng: np.random.Generator, n: int, scale: float = 1.0) -> np.ndarray:
    """``n × 12`` ITK parameters of small rigid motions (float32 values)."""
    out = []
    for _ in range(n):
        m = rot(rng, 0.02 * scale)
        t = rng.standard_normal(3) * scale
        out.append(np.concatenate([m.ravel(), t]))
    return np.asarray(out, dtype=np.float32).astype(np.float64)


def nt_matrix(path: Path):
    import nitransforms as nt

    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            return np.asarray(nt.linear.load(str(path)).matrix)
    except Exception as e:
        return last_line(e)


def lx_matrix(path: Path, mode: str = "reproduce"):
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            ras, single = lm._hmc_matrices(str(path), mode)
        return ras[0] if single else ras
    except Exception as e:
        return last_line(e)


def same(want, got) -> bool:
    if isinstance(want, str) or isinstance(got, str):
        if isinstance(want, str) and isinstance(got, str) and "TransformFileError" in want:
            return "TransformFileError" in got
        return want == got
    return np.shape(want) == np.shape(got) and bits(want) == bits(got)


def itk_files(rng: np.random.Generator, n: int) -> dict[str, str]:
    p = motion_series(rng, n)
    c = np.asarray(rng.standard_normal((n, 3)) * 5, dtype=np.float32).astype(np.float64)
    z = np.zeros((n, 3))
    files = {
        "%g, no centre": itk_text(p, z, "%g"),
        "%.9g, a centre": itk_text(p, c, "%.9g"),
        "repr, a centre": itk_text(p, c, "repr"),
        "large motion": itk_text(motion_series(rng, n, 20.0), c, "%.9g"),
        "blank lines and spaces": itk_text(p, c, "%.9g").replace("\n#Transform", "\n\n  \n#Transform"),
        "double type, repr": itk_text(
            np.asarray(motion_series(rng, n), dtype=np.float64) + rng.standard_normal((n, 12)) * 1e-9,
            c,
            "repr",
            "double",
        ),
        "one transform": itk_text(p[:1], c[:1], "%.9g"),
        "two transforms": itk_text(p[:2], c[:2], "%.9g"),
        "no banner": itk_text(p, z, "%g").replace(BANNER + "\n", ""),
        "only a banner": BANNER + "\n",
        "a missing line": itk_text(p, z, "%g").replace("FixedParameters: 0 0 0\n", "", 1),
        "11 parameters": itk_text(p[:, :11], z, "%g"),
        "15 parameters": itk_text(np.hstack([p, z]), z, "%g"),
        "a non-integer index": itk_text(p, z, "%g").replace("#Transform 0", "#Transform x", 1),
        "a singular matrix": itk_text(np.vstack([p[:1] * 0, p[1:]]), z, "%g"),
        "an infinite centre": itk_text(p, np.vstack([[np.inf, 0, 0], c[1:]]), "%g"),
        "a NaN parameter": itk_text(np.vstack([p[:1], np.full((1, 12), np.nan), p[2:]]), z, "%g"),
        "an Euler3DTransform": itk_text(p[:, :6], z, "%g").replace("AffineTransform_float", "Euler3DTransform_double"),
        "not ITK at all": "1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n",
    }  # fmt: skip
    return files


def itk_section(tier: str, tmp: Path) -> None:
    rng = np.random.default_rng(7)
    for n in (3, 40) if tier == "quick" else (3, 40, 600):
        for name, text in itk_files(rng, n).items():
            path = tmp / f"{name.replace(' ', '_').replace(',', '')}-{n}.txt"
            path.write_text(text, encoding="utf-8")
            want, got = nt_matrix(path), lx_matrix(path)
            if name in ("no banner", "not ITK at all"):
                # larmorx reads ITK's text files only (nitransforms' LTA, AFNI and FSL readers
                # are not ported); nitransforms' own outcome on these is counted apart.
                ok = isinstance(got, str) and "not an ITK text transform file" in got
                count("itk a file that is not ITK text: refused", ok, f"{name} {n}: {got}")
                APART[f"itk {name}: nitransforms gives {str(want)[:70]!r}"] += 1
                continue
            count(f"itk {name}", same(want, got), f"{n}: {str(want)[:120]} | {str(got)[:120]}")
            if name == "double type, repr":
                with warnings.catch_warnings(record=True) as caught:
                    warnings.simplefilter("always")
                    try:
                        fix = lm._hmc_matrices(str(path), "fix")[0]
                    except Exception as e:
                        fix = last_line(e)
                entries = {
                    w.message.entry
                    for w in caught
                    if isinstance(w.message, lx.UpstreamDefectWarning)
                }
                count(
                    "itk PIPE-73 default (double precision)",
                    "PIPE-73" in entries and not isinstance(fix, str),
                    f"{n}: {entries}",
                )
    for suffix in (".mat", ".h5"):
        path = tmp / f"x{suffix}"
        path.write_bytes(b"")
        got = lx_matrix(path)
        count("itk .mat/.h5 refused", "TransformFileError" in str(got), got)


# --- FSL matrices ------------------------------------------------------------------------------


def affines(rng: np.random.Generator, n: int) -> list[tuple[np.ndarray, tuple[int, ...]]]:
    out = []
    for _ in range(n):
        z = rng.uniform(0.5, 4.0, 3)
        r = rot(rng, 0.3)
        a = np.eye(4)
        a[:3, :3] = r * z
        a[:3, 3] = rng.uniform(-100, 100, 3)
        if rng.uniform() < 0.5:
            a[:, 0] *= -1  # the other handedness
        out.append((a, tuple(int(s) for s in rng.integers(2, 120, 3))))
    a = np.diag([3.125, 3.125, 4.0, 1.0])
    a[:3, 3] = (-100, -100, -60)
    out += [(a, (64, 64, 34)), (np.diag([-2.0, 2.0, 2.0, 1.0]), (104, 104, 72))]
    out.append((np.diag([1e-5, 1e-5, 1e-5, 1.0]), (10, 10, 10)))
    out.append((np.diag([1e5, 2e4, 3.0, 1.0]), (5, 6, 7)))
    return out


def fsl_section(tier: str, tmp: Path) -> None:
    import nitransforms as nt

    rng = np.random.default_rng(11)
    n = 20 if tier == "quick" else 200
    ras = np.stack(
        [
            np.vstack([np.c_[rot(rng, 0.05), rng.standard_normal(3)], [0, 0, 0, 1]])
            for _ in range(30)
        ]
    )
    for aff, shape in affines(rng, n):
        img = nb.Nifti1Image(np.zeros(shape, dtype=np.float32), aff)
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            fsl = nt.io.fsl.FSLLinearTransformArray.from_ras(ras, reference=img, moving=img)
        want = np.stack([x["parameters"] for x in fsl.xforms])
        aff_nb = img.affine
        got = lm._fsl(ras, aff_nb, shape[0])
        count("fsl from_ras", bits(want) == bits(got), f"{shape} {np.round(aff_nb, 3).tolist()}")
    from types import SimpleNamespace

    img = SimpleNamespace(affine=np.diag([1.0, 1, 0, 1]), shape=(4, 4, 4))
    try:
        nt.io.fsl.FSLLinearTransformArray.from_ras(ras, reference=img, moving=img)
        want = "no error"
    except Exception as e:
        want = last_line(e)
    try:
        lm._fsl(ras, img.affine, 4)
        got = "no error"
    except Exception as e:
        got = last_line(e)
    count("fsl from_ras singular affine", want == got, f"{want} | {got}")


# --- the interfaces ----------------------------------------------------------------------------


def fmriprep_module(version: str):
    """fMRIPrep ``version``'s ``interfaces/confounds.py`` from its tag, as a module."""
    path = TAGS / f"fmriprep-{version}" / "fmriprep" / "interfaces" / "confounds.py"
    spec = importlib.util.spec_from_file_location(f"fmriprep_confounds_{version}", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def run_orig(klass, d: Path, **inputs):
    d.mkdir(parents=True)
    with cwd(d), warnings.catch_warnings():
        warnings.simplefilter("ignore")
        try:
            res = klass(**inputs).run()
        except Exception as e:
            return last_line(e)
    return content(Path(res.outputs.out_file))


def run_lx(fn, d: Path, *args, **kw):
    d.mkdir(parents=True)
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        try:
            r = fn(*args, out_dir=d, **kw)
        except Exception as e:
            return last_line(e), set()
    entries = {w.message.entry for w in caught if isinstance(w.message, lx.UpstreamDefectWarning)}
    return content(Path(r.out_file)), entries


def table(text: bytes) -> list[list[str]]:
    return [line.split("\t") for line in text.decode().splitlines()]


def motion_model(
    xfm: str, ref: str, release: str, quat_numpy: str | None = None
) -> dict[tuple[int, int], tuple[str, str, set]]:
    """For each ``rot_*`` cell: the model's text with glibc's and with the correctly rounded
    functions, and the causes, from SciPy's own quaternions of nitransforms' FSL matrices (or,
    with ``quat_numpy``, larmorx's quaternions with that numpy's LAPACK: for the oracle's files
    outside its image; the ``rotation`` section proves them SciPy's)."""
    import nitransforms as nt

    img = nb.load(ref)
    hmc = nt.linear.load(xfm)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        fsl = nt.io.fsl.FSLLinearTransformArray.from_ras(hmc.matrix, reference=img, moving=img)
    f = np.stack([x["parameters"] for x in fsl.xforms])
    m = f[:, :3, :3] if release == "25.2.7" else f[:, :3, :3].transpose(0, 2, 1)
    if quat_numpy is None:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            q = Rotation.from_matrix(m).as_quat()
    else:
        q = lm._rotations(m, quat_numpy, "C" if release == "25.2.7" else "F")
    out = {}
    for r in range(len(q)):
        g, c, why = euler_causes(q[r], "XYZ")
        if release == "25.2.7":
            g, c = [-v for v in g], [-v for v in c]
        for k in range(3):
            out[(r, 3 + k)] = (str(np.float64(g[k])), str(np.float64(c[k])), why)
    return out


@contextlib.contextmanager
def double_itk():
    """nitransforms' ITK text reading with the parameters in double precision (PIPE-73's
    model: the fixed parameters stay float32, as its structure stores them)."""
    from nitransforms.io import itk

    orig = itk.ITKLinearTransform.from_string

    def from_string(cls, string):
        tf = cls()
        sa = tf.structarr
        lines = [line.strip() for line in string.splitlines() if line.strip()]
        if not lines or not lines[0].startswith("#"):
            raise itk.TransformFileError
        if lines[1][0] == "#":
            lines = lines[1:]
        parameters = np.eye(4, dtype="f8")
        sa["index"] = int(lines[0][lines[0].index("T") :].split()[1])
        sa["offset"] = np.genfromtxt([lines[3].split(":")[-1].encode()], dtype=cls.dtype["offset"])
        vals = np.genfromtxt([lines[2].split(":")[-1].encode()], dtype="f8")
        parameters[:3, :3] = vals[:-3].reshape((3, 3))
        parameters[:3, 3] = vals[-3:]
        sa["parameters"] = parameters
        try:
            cls.from_string("\n".join(lines[4:8]))
        except itk.TransformFileError:
            return tf
        raise itk.TransformFileError("More than one linear transform found.")

    itk.ITKLinearTransform.from_string = classmethod(from_string)
    try:
        yield
    finally:
        itk.ITKLinearTransform.from_string = orig


def compare_motion(key: str, want, got, model, release: str) -> None:
    """The tables: every cell identical but the ``rot_*`` cells where glibc's functions and the
    correctly rounded ones differ, which must be the model's (D4)."""
    if isinstance(want, str) or isinstance(got, str):
        count(
            key,
            want == got,
            f"{want if isinstance(want, str) else 'table'} | {got if isinstance(got, str) else 'table'}",
        )
        return
    a, b = table(want), table(got)
    ok = len(a) == len(b) and a[0] == b[0]
    d4 = 0
    for r, (ra, rb) in enumerate(zip(a[1:], b[1:], strict=False)):
        for k, (x, y) in enumerate(zip(ra, rb, strict=True)):
            if x == y:
                if model and k >= 3 and model[(r, k)][0] != model[(r, k)][1]:
                    ok = False  # the model says they differ
                continue
            if model is None or k < 3:
                ok = False
                continue
            g, c, why = model[(r, k)]
            if not (x == g and y == c):
                ok = False
            else:
                d4 += 1
                for w in why:
                    APART[f"{key.split(':')[0]}: rot cells glibc's {w} changes (D4)"] += 1
    if d4:
        APART[f"{key.split(':')[0]}: tables with D4 cells"] += 1
    count(key, ok, f"{len(a)} rows")


def reference(
    tmp: Path, name: str, data: np.ndarray, affine: np.ndarray, dtype=None, scale=None, gz=True
) -> str:
    img = nb.Nifti1Image(data, affine)
    if dtype is not None:
        img.set_data_dtype(dtype)
    if scale is not None:
        img.header.set_slope_inter(*scale)
    path = tmp / f"{name}_boldref.nii{'.gz' if gz else ''}"
    nb.save(img, path)
    return str(path)


def big_endian(tmp: Path, name: str, data: np.ndarray, affine: np.ndarray) -> str:
    hdr = nb.Nifti1Header(endianness=">")
    img = nb.Nifti1Image(data.astype(">f4"), affine, hdr)
    path = tmp / f"{name}_boldref.nii.gz"
    nb.save(img, path)
    return str(path)


def iface_cases(rng: np.random.Generator, tmp: Path, tier: str):
    """``(name, xfm, ref, expect)`` with ``expect`` the defects the default corrects."""
    base = np.diag([3.0, 3.0, 3.3, 1.0])
    base[:3, 3] = (-90, -100, -60)
    pos = base.copy()
    pos[0, 0] = -3.0  # det > 0 after the flip? det(base) > 0; this one < 0
    obl = np.eye(4)
    obl[:3, :3] = rot(rng, 0.2) * (2.5, 2.5, 3.0)
    obl[:3, 3] = (-80, -90, -40)
    shape = (24, 26, 18)
    vol = rng.uniform(100, 1000, shape)
    cases = []
    sizes = (12, 300) if tier == "quick" else (2, 12, 300, 600)

    def xfm(n, name, fmt="%.9g", kind="float", scale=1.0):
        p = motion_series(rng, n, scale)
        c = np.zeros((n, 3))
        path = tmp / f"{name}_xfm.txt"
        if kind == "double":
            p = p + rng.standard_normal(p.shape) * 1e-9
        path.write_text(itk_text(p, c, fmt, kind), encoding="utf-8")
        return str(path)

    for n in sizes:
        cases.append(
            (
                f"float32 det>0 n{n}",
                xfm(n, f"a{n}"),
                reference(tmp, f"a{n}", vol.astype(np.float32), base),
                set(),
            )
        )
    cases += [
        ("float32 det<0", xfm(40, "b"), reference(tmp, "b", vol.astype(np.float32), pos), set()),
        ("oblique float64", xfm(40, "c"), reference(tmp, "c", vol, obl), set()),
        ("int16", xfm(40, "d"), reference(tmp, "d", vol.astype(np.int16), base), set()),
        ("uint8, .nii", xfm(40, "e"), reference(tmp, "e", (vol / 4).astype(np.uint8), obl, gz=False), set()),
        ("scaled int16", xfm(40, "f"), reference(tmp, "f", vol, base, dtype=np.int16, scale=(0.25, 3.0)), set()),
        ("big-endian float32 above 8192", xfm(40, "g"), big_endian(tmp, "g", vol, base), set()),
        ("big-endian float32 below 8192", xfm(40, "h"), big_endian(tmp, "h", vol[:12, :12, :12], base), set()),
        ("large motion", xfm(40, "i", scale=30.0), reference(tmp, "i", vol.astype(np.float32), obl), set()),
        ("double transforms", xfm(40, "j", "repr", "double"), reference(tmp, "j", vol.astype(np.float32), base), {"PIPE-73"}),
        ("one volume", xfm(1, "k"), reference(tmp, "k", vol.astype(np.float32), base), {"PIPE-71"}),
        ("constant reference", xfm(40, "l"), reference(tmp, "l", np.full(shape, 7, np.float32), base), {"PIPE-72 25.2.7"}),
        ("a sum of zero", xfm(40, "p"), reference(tmp, "p", np.where(np.arange(vol.size).reshape(shape) % 2, 1.0, -1.0).astype(np.float32), base), {"PIPE-72 25.2.6"}),
        ("a NaN voxel", xfm(40, "m"), reference(tmp, "m", np.where(vol > 990, np.nan, vol).astype(np.float32), base), {"PIPE-72"}),
        ("4-D reference", xfm(40, "n"), reference(tmp, "n", vol[..., None].astype(np.float32), base), set()),
        ("2-D reference", xfm(40, "o"), reference(tmp, "o", vol[:, :, 0].astype(np.float32), base), set()),
    ]  # fmt: skip
    # Gimbal lock: an isotropic, axis-aligned reference and a rotation of 90° about y.
    p = Rotation.from_euler("XYZ", [0.1, np.pi / 2, 0.0]).as_matrix()
    gp = np.tile(np.concatenate([p.ravel(), [1.0, 2.0, 3.0]]), (3, 1))
    gp[0] = np.concatenate([np.eye(3).ravel(), [0, 0, 0]])
    path = tmp / "gimbal_xfm.txt"
    path.write_text(itk_text(gp, np.zeros((3, 3)), "repr"), encoding="utf-8")
    iso = np.diag([2.0, 2.0, 2.0, 1.0])
    cases.append(
        ("gimbal lock", str(path), reference(tmp, "q", vol.astype(np.float32), iso), set())
    )
    # A reflection.
    rp = np.tile(np.concatenate([np.diag([1.0, -1, 1]).ravel(), [0, 0, 0]]), (3, 1))
    path = tmp / "reflection_xfm.txt"
    path.write_text(itk_text(rp, np.zeros((3, 3)), "%g"), encoding="utf-8")
    cases.append(
        ("a reflection", str(path), reference(tmp, "r", vol.astype(np.float32), base), set())
    )
    return cases


def iface_section(tier: str, tmp: Path) -> None:
    import niworkflows
    from niworkflows.interfaces import confounds as nwc

    from larmorx.niworkflows import confounds as lnc

    rng = np.random.default_rng(3)
    mods = {v: fmriprep_module(v) for v in ("25.2.6", "25.2.7")}
    nw_release = "25.2.7" if "data.min()" in Path(nwc.__file__).read_text() else "25.2.6"
    for name, xfm, ref, expected in iface_cases(rng, tmp, tier):
        every = {e for e in expected if " " not in e}
        for release, mod in mods.items():
            key = f"{name} {release}"
            expect = every | {e.split()[0] for e in expected if e.endswith(release)}
            d = tmp / "runs" / key.replace(" ", "_")
            want = run_orig(mod.FSLMotionParams, d / "orig", xfm_file=xfm, boldref_file=ref)
            if release == nw_release:
                nwant = run_orig(nwc.FSLMotionParams, d / "nw", xfm_file=xfm, boldref_file=ref)
                count("iface niworkflows FSLMotionParams = fMRIPrep's", nwant == want, key)
                nlx, _ = run_lx(
                    lnc.fsl_motion_params, d / "nwlx", xfm, ref,
                    niworkflows_version=niworkflows.__version__, numpy_version=NUMPY,
                    upstream_defects="reproduce",
                )  # fmt: skip
            got, entries = run_lx(
                lx.confounds.fsl_motion_params, d / "lx", xfm, ref,
                fmriprep_version=release, numpy_version=NUMPY, upstream_defects="reproduce",
            )  # fmt: skip
            model = None
            if not isinstance(want, str):
                model = motion_model(xfm, ref, release)
            compare_motion(f"iface FSLMotionParams: {key}", want, got, model, release)
            if release == nw_release:
                count("iface larmorx.niworkflows' copy = lx.confounds'", nlx == got, key)
            fixed, fentries = run_lx(
                lx.confounds.fsl_motion_params, d / "fix", xfm, ref,
                fmriprep_version=release, numpy_version=NUMPY,
            )  # fmt: skip
            ok = (fixed == got) if not expect else (expect <= (entries | fentries))
            count(
                "iface FSLMotionParams default", ok, f"{key}: {expect} {fentries} {str(fixed)[:80]}"
            )
            if "PIPE-72" in expect:
                count(
                    "iface PIPE-72 default: ValueError",
                    "ValueError" in str(fixed) and "PIPE-72" in str(fixed),
                    key,
                )
            if "PIPE-71" in expect:
                # The default's row: the first row of the original on a series of the transform twice.
                text = Path(xfm).read_text(encoding="utf-8")
                block = text.split("#Transform 0", 1)[1]
                two = Path(d / "two_xfm.txt")
                two.write_text(text + "#Transform 1" + block, encoding="utf-8")
                want2 = run_orig(
                    mod.FSLMotionParams, d / "orig2", xfm_file=str(two), boldref_file=ref
                )
                ok = not isinstance(want2, str) and not isinstance(fixed, str)
                ok = ok and table(fixed) == table(want2)[:2]
                count("iface PIPE-71 default = the first row of a series of two", ok, key)
            if "PIPE-73" in expect:
                with double_itk():
                    want3 = run_orig(
                        mod.FSLMotionParams, d / "orig3", xfm_file=xfm, boldref_file=ref
                    )
                    m3 = motion_model(xfm, ref, release)
                compare_motion(
                    f"iface PIPE-73 default = nitransforms in double: {key}",
                    want3,
                    fixed,
                    m3,
                    release,
                )
        want = run_orig(mods["25.2.7"].FSLRMSDeviation, d / "rorig", xfm_file=xfm, boldref_file=ref)
        nwant = run_orig(nwc.FSLRMSDeviation, d / "rnw", xfm_file=xfm, boldref_file=ref)
        count("iface niworkflows FSLRMSDeviation = fMRIPrep's", nwant == want, name)
        got, _ = run_lx(
            lx.confounds.fsl_rms_deviation, d / "rlx", xfm, ref, upstream_defects="reproduce"
        )
        count(
            f"iface FSLRMSDeviation: {name}", want == got, f"{str(want)[:100]} | {str(got)[:100]}"
        )


# --- real --------------------------------------------------------------------------------------

QUICK_REAL = ("ds005040", "ds005752-sub-ON01016-noreconall/", "ds000256", "ds005454", "ds000258")


def real_section(tier: str, tmp: Path) -> None:
    from nipype.utils.filemanip import loadpkl

    in_image = scipy.__version__ == "1.15.2"
    mods = {v: fmriprep_module(v) for v in ("25.2.6", "25.2.7")}
    wfs = sorted(RUNS.glob("*/work/fmriprep_25_2_wf/sub_*_wf/bold_*_wf/bold_confounds_wf"))
    if tier == "quick":
        wfs = [w for w in wfs if any(str(w.relative_to(RUNS)).startswith(q) for q in QUICK_REAL)]
    print(f"real: {len(wfs)} confounds workflows", flush=True)
    for wf in wfs:
        run = wf.relative_to(RUNS).parts[0]
        inputs = loadpkl(wf / "motion_params" / "_inputs.pklz")
        host = {k: str(RUNS / run / v.lstrip("/")) for k, v in inputs.items()}
        xfm, ref = host["xfm_file"], host["boldref_file"]
        tag = f"{run}/{wf.parts[-2]}"
        d = tmp / run / wf.parts[-2]
        # The node's own files (fMRIPrep 25.2.6 in its image).
        t0 = time.perf_counter()
        node = content(next((wf / "motion_params").glob("*_motion.tsv*")))
        got, _ = run_lx(
            lx.confounds.fsl_motion_params, d / "n-lx", xfm, ref,
            fmriprep_version="25.2.6", numpy_version="2.2.6", upstream_defects="reproduce",
        )  # fmt: skip
        SECONDS["motion_params"] += time.perf_counter() - t0
        model = motion_model(xfm, ref, "25.2.6", None if in_image else "2.2.6")
        compare_motion("real node motion_params", node, got, model, "25.2.6")
        if in_image:
            again = run_orig(
                mods["25.2.6"].FSLMotionParams, d / "n-orig", xfm_file=xfm, boldref_file=ref
            )
            count("real node motion_params = fMRIPrep 25.2.6 again", again == node, tag)
        t0 = time.perf_counter()
        node = content(next((wf / "rmsd").glob("*_motion.tsv*")))
        got, _ = run_lx(
            lx.confounds.fsl_rms_deviation, d / "r-lx", xfm, ref, upstream_defects="reproduce"
        )
        SECONDS["rmsd"] += time.perf_counter() - t0
        count("real node rmsd", node == got, tag)
        # Both releases again in this environment.
        for release, mod in mods.items():
            t0 = time.perf_counter()
            want = run_orig(mod.FSLMotionParams, d / f"o{release}", xfm_file=xfm, boldref_file=ref)
            SECONDS[f"fMRIPrep {release} FSLMotionParams"] += time.perf_counter() - t0
            t0 = time.perf_counter()
            run_orig(mod.FSLRMSDeviation, d / f"r{release}", xfm_file=xfm, boldref_file=ref)
            SECONDS[f"fMRIPrep {release} FSLRMSDeviation"] += time.perf_counter() - t0
            got, _ = run_lx(
                lx.confounds.fsl_motion_params, d / f"l{release}", xfm, ref,
                fmriprep_version=release, numpy_version=NUMPY, upstream_defects="reproduce",
            )  # fmt: skip
            compare_motion(
                f"real {release} motion_params (numpy {NUMPY})", want, got,
                motion_model(xfm, ref, release), release,
            )  # fmt: skip
            fixed, entries = run_lx(
                lx.confounds.fsl_motion_params, d / f"f{release}", xfm, ref,
                fmriprep_version=release, numpy_version=NUMPY,
            )  # fmt: skip
            count(f"real {release} default = reproduce", fixed == got and not entries, tag)


# -----------------------------------------------------------------------------------------------


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--tier", choices=("quick", "full"), default="quick")
    p.add_argument("--sections", default="rotation,itk,fsl,iface,real")
    a = p.parse_args()
    import nitransforms
    import niworkflows

    print(
        f"numpy {NUMPY}, SciPy {scipy.__version__}, nitransforms {nitransforms.__version__}, "
        f"niworkflows {niworkflows.__version__}, nibabel {nb.__version__}, tier {a.tier}",
        flush=True,
    )
    with tempfile.TemporaryDirectory(prefix="lx-check-motion-") as tmp:
        for s in a.sections.split(","):
            t0 = time.perf_counter()
            d = Path(tmp) / s
            d.mkdir()
            {
                "rotation": rotation_section,
                "itk": itk_section,
                "fsl": fsl_section,
                "iface": iface_section,
                "real": real_section,
            }[s](a.tier, d)
            print(f"section {s}: {time.perf_counter() - t0:.1f} s", flush=True)
    for key, (i, d) in COUNTS.items():
        print(f"  {key:60s} identical {i:6d}  differ {d:5d}")
    if SECONDS:
        print("  larmorx's seconds: " + ", ".join(f"{k} {v:.1f}" for k, v in SECONDS.items()))
    for key, n in sorted(APART.items()):
        print(f"  apart: {key} ({n})")
    for f in FAILS:
        print("  FAIL " + f)
    differ = sum(c[1] for c in COUNTS.values())
    print(f"CHECK {'PASS' if differ == 0 else 'DIFFER'}: {differ} cases differ")
    return 0 if differ == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
