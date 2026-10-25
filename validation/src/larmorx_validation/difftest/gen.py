# SPDX-License-Identifier: Apache-2.0
"""Random valid inputs for differential testing: seeded, plain arithmetic, written explicitly.

Every random choice of a case is drawn here, from one seeded stream per case
(:class:`Draw`, keyed by the text ``"<tool>:<seed>:<case number>"``), and written into the
case's recipe as explicit values: a recipe alone rebuilds its inputs, on any machine, byte for
byte. The golden tests' rule holds (CLAUDE.md rule 10): integers and IEEE arithmetic only
(``+ - * /`` and the correctly rounded ``sqrt``), no ``sin``, ``exp`` or other transcendental
functions, whose last bits differ between platforms; rotations are rational quaternions; noise
is a sum of uniform draws (Irwin-Hall).

An **image spec** (a JSON object) names everything a NIfTI file holds:

- ``shape``: 1 to 32 voxels per axis, 2D to 4D, at most 32³ voxels in a volume, 4 to 8 volumes
  in a series (some axes 1 voxel thick, some odd);
- ``geometry``: the qform as a rational quaternion with ``qfac`` (flips and axis permutations
  are quaternions too), the voxel spacing (anisotropic), the origin, the qform and sform codes,
  an sform equal to the qform, absent, or another (sheared) matrix; the spatial units;
- ``content``: what the values look like (a head-like phantom of nested ellipsoids with a
  polynomial bias field, polynomial blobs, labels, a ramp, a checkerboard, noise, a constant,
  zeros, sparse points, a crop of a real image from ``larmorx-testdata``), evaluated in world
  coordinates, so a moved copy of the same phantom is a registration problem with a known
  answer;
- ``values``: the range (scale and offset: tiny, huge, negative), integer steps, special
  values (NaN, infinities, signed zeros, outliers);
- ``dtype`` and ``scaling`` (``scl_slope``/``scl_inter``, a negative slope included);
- ``timing`` (4D): the TR and its unit, the slice code, duration and range;
- ``header``: other fields (``descrip``, ``intent_code``, ``cal_min``/``cal_max``,
  ``toffset``) and the file form (``.nii``, ``.nii.gz``, ``.hdr``/``.img``, byte order,
  NIfTI-2).
"""

from __future__ import annotations

import gzip
import hashlib
import json
from collections.abc import Sequence
from fractions import Fraction
from pathlib import Path
from typing import Any

import numpy as np

#: The largest volume an input may have (the golden tests' limit, rule 10).
MAX_VOXELS = 32**3
#: Data types NIfTI stores and the programs read (NIfTI code, numpy dtype).
DTYPES = {
    "uint8": 2,
    "int16": 4,
    "int32": 8,
    "float32": 16,
    "float64": 64,
    "int8": 256,
    "uint16": 512,
    "uint32": 768,
    "int64": 1024,
    "uint64": 1280,
}
_UNITS = {"unknown": 0, "m": 1, "mm": 2, "um": 3}
_TUNITS = {"unknown": 0, "sec": 8, "msec": 16, "usec": 24}

#: Rational quaternions (a, b, c, d) of the 24 axis-aligned rotations: every permutation and
#: flip of the axes with ``qfac`` (NIfTI's sign of the third axis).
AXIS_QUATERNIONS: tuple[tuple[int, int, int, int], ...] = (
    (1, 0, 0, 0),
    (0, 1, 0, 0),
    (0, 0, 1, 0),
    (0, 0, 0, 1),
    (1, 1, 0, 0),
    (1, -1, 0, 0),
    (1, 0, 1, 0),
    (1, 0, -1, 0),
    (1, 0, 0, 1),
    (1, 0, 0, -1),
    (0, 1, 1, 0),
    (0, 1, -1, 0),
    (0, 1, 0, 1),
    (0, 1, 0, -1),
    (0, 0, 1, 1),
    (0, 0, 1, -1),
    (1, 1, 1, 1),
    (1, 1, 1, -1),
    (1, 1, -1, 1),
    (1, -1, 1, 1),
    (1, -1, -1, 1),
    (1, -1, 1, -1),
    (1, 1, -1, -1),
    (1, -1, -1, -1),
)


# ------------------------------------------------------------------------------------------------
# The stream of choices


#: The share of cases drawn as *extreme* (user, 2026-10-25: "reasonable size tests"): header
#: units of metres or micrometres and voxel sizes far outside an MRI's. The others are
#: realistic: millimetres (or no units), voxels of 0.5 to 4 mm, at most 32 voxels per axis.
EXTREME_SHARE = 0.1
#: Realistic voxel sizes, mm (within 0.2 to 10 mm), and the extreme ones.
REALISTIC_SPACINGS = (0.5, 0.8, 1.0, 1.0, 1.2, 1.5, 2.0, 2.5, 3.0, 4.0)
EXTREME_SPACINGS = (0.01, 0.05, 0.1, 15.0, 25.0, 50.0)


class Draw:
    """The seeded choices of one case. Each choice is named; the names and values chosen are
    kept (:attr:`chosen`), so the night's report can say which options and input features
    were covered. :attr:`extreme`: the case is one of the night's extreme share (the generators
    may then draw odd units and sizes)."""

    def __init__(self, key: str):
        self.key = key
        digest = hashlib.sha256(key.encode("utf-8")).digest()
        self.rng = np.random.default_rng(int.from_bytes(digest[:16], "little"))
        self.chosen: dict[str, Any] = {}
        self.extreme = False

    def child(self, name: str) -> Draw:
        """An independent stream for a part of the case (an input, an option group)."""
        d = Draw(f"{self.key}/{name}")
        d.chosen = self.chosen
        d.extreme = self.extreme
        return d

    def seed(self) -> int:
        """A seed for a content generator (kept in the recipe)."""
        return int(self.rng.integers(0, 2**31 - 1))

    def note(self, name: str, value: Any) -> Any:
        self.chosen[name] = value
        return value

    def chance(self, name: str, p: float) -> bool:
        return self.note(name, bool(self.rng.random() < p))

    def integer(self, name: str, lo: int, hi: int) -> int:
        """An integer in [lo, hi]."""
        return self.note(name, int(self.rng.integers(lo, hi + 1)))

    def choice(self, name: str, options: Sequence[Any], weights: Sequence[float] | None = None):
        """One of ``options`` (with relative ``weights``)."""
        if weights is None:
            i = int(self.rng.integers(0, len(options)))
        else:
            w = np.asarray(weights, dtype=np.float64)
            cum = np.cumsum(w / w.sum())
            i = min(int(np.searchsorted(cum, self.rng.random(), side="right")), len(options) - 1)
        value = options[i]
        self.note(name, value if isinstance(value, str | int | float | bool) else i)
        return value

    def subset(self, name: str, options: Sequence[Any], p: float = 0.5) -> list:
        out = [o for o in options if self.rng.random() < p]
        self.note(name, len(out))
        return out

    def step(self, name: str, lo: float, hi: float, step: float) -> float:
        """A value in [lo, hi] on a grid of ``step`` (short decimal numbers on command lines)."""
        n = round((hi - lo) / step)
        v = lo + step * int(self.rng.integers(0, n + 1))
        return self.note(name, float(round(v, 10)))


# ------------------------------------------------------------------------------------------------
# Image specs


def draw_shape(
    d: Draw,
    ndim: int,
    *,
    lo: int = 1,
    hi: int = 32,
    volumes: tuple[int, int] = (4, 8),
    thin: float = 0.15,
    max_voxels: int = MAX_VOXELS,
) -> list[int]:
    """A shape of ``ndim`` axes: each spatial axis ``lo``..``hi`` voxels, some one voxel thick
    (``thin``), the volume at most ``max_voxels``; a fourth axis of ``volumes`` volumes."""
    spatial = min(ndim, 3)
    shape = []
    for _ in range(spatial):
        if lo <= 1 <= hi and d.rng.random() < thin:
            shape.append(1)
        else:
            shape.append(int(d.rng.integers(max(lo, 2), hi + 1)))
    while int(np.prod(shape)) > max_voxels:
        k = int(np.argmax(shape))
        shape[k] = max(lo, shape[k] * 3 // 4)
    if ndim == 4:
        shape.append(int(d.rng.integers(volumes[0], volumes[1] + 1)))
    d.note("shape", "x".join(map(str, shape)))
    return shape


def draw_geometry(d: Draw, *, oblique: float = 0.3, axis: float = 0.4) -> dict:
    """Spacing, orientation (a rational quaternion), origin, the qform and sform codes. A
    realistic case has voxels of 0.5 to 4 mm in millimetres (or no units); an extreme one
    (:attr:`Draw.extreme`) may have metres or micrometres and voxels from 0.01 to 50 units."""
    choices = REALISTIC_SPACINGS + (EXTREME_SPACINGS if d.extreme else ())
    spacing = [d.choice(f"spacing{k}", choices) for k in range(3)]
    if d.rng.random() < 0.3:  # isotropic
        spacing = [spacing[0]] * 3
    kind = d.choice(
        "orientation",
        ("plumb", "axes", "oblique", "axes+oblique"),
        (1 - axis - oblique, axis * 0.7, oblique * 0.7, 0.3 * (axis + oblique)),
    )
    q: tuple[int, ...] = (1, 0, 0, 0)
    if kind in ("axes", "axes+oblique"):
        q = AXIS_QUATERNIONS[int(d.rng.integers(0, len(AXIS_QUATERNIONS)))]
    if kind in ("oblique", "axes+oblique"):
        # A small rotation: (n, b, c, d) with |b|, |c|, |d| small against n.
        n = int(d.rng.integers(4, 40))
        small = tuple(int(d.rng.integers(-3, 4)) for _ in range(3))
        q = quaternion_product(q, (n, *small))
    qfac = d.choice("qfac", (1, -1), (0.8, 0.2))
    origin = [d.step(f"origin{k}", -60.0, 60.0, 0.25) for k in range(3)]
    qcode = d.choice("qform_code", (0, 1, 2, 3, 4), (0.15, 0.55, 0.1, 0.1, 0.1))
    scode = d.choice("sform_code", (0, 1, 2, 3, 4), (0.25, 0.45, 0.1, 0.1, 0.1))
    sform = d.choice("sform", ("same", "other"), (0.85, 0.15))
    geometry = {
        "spacing": spacing,
        "quaternion": list(q),
        "qfac": qfac,
        "origin": origin,
        "qform_code": qcode,
        "sform_code": scode,
        "sform": sform,
        "units": d.choice(
            "units", ("m", "um", "mm") if d.extreme else ("mm", "mm", "mm", "mm", "unknown")
        ),
    }
    if sform == "other":
        geometry["shear"] = [d.step(f"shear{k}", -0.1, 0.1, 0.025) for k in range(3)]
    return geometry


def plumb_geometry(spacing: Sequence[float] = (1.0, 1.0, 1.0), origin=(0.0, 0.0, 0.0)) -> dict:
    return {
        "spacing": list(spacing),
        "quaternion": [1, 0, 0, 0],
        "qfac": 1,
        "origin": list(origin),
        "qform_code": 1,
        "sform_code": 1,
        "sform": "same",
        "units": "mm",
    }


def quaternion_product(p: Sequence[int], q: Sequence[int]) -> tuple[int, int, int, int]:
    a1, b1, c1, d1 = p
    a2, b2, c2, d2 = q
    return (
        a1 * a2 - b1 * b2 - c1 * c2 - d1 * d2,
        a1 * b2 + b1 * a2 + c1 * d2 - d1 * c2,
        a1 * c2 - b1 * d2 + c1 * a2 + d1 * b2,
        a1 * d2 + b1 * c2 - c1 * b2 + d1 * a2,
    )


def rotation(q: Sequence[int]) -> np.ndarray:
    """The rotation matrix of the (unnormalised, integer) quaternion ``q``: exact rationals,
    rounded once to float64."""
    a, b, c, d = (Fraction(int(v)) for v in q)
    n = a * a + b * b + c * c + d * d
    if n == 0:
        raise ValueError("zero quaternion")
    m = [
        [a * a + b * b - c * c - d * d, 2 * (b * c - a * d), 2 * (b * d + a * c)],
        [2 * (b * c + a * d), a * a + c * c - b * b - d * d, 2 * (c * d - a * b)],
        [2 * (b * d - a * c), 2 * (c * d + a * b), a * a + d * d - c * c - b * b],
    ]
    return np.array([[float(v / n) for v in row] for row in m], dtype=np.float64)


def _unit_quaternion(q: Sequence[int]) -> tuple[float, float, float, float]:
    """``q`` normalised with ``a >= 0`` (NIfTI stores b, c, d and derives a)."""
    a, b, c, d = (int(v) for v in q)
    if a < 0 or (a == 0 and (b, c, d) < (0, 0, 0)):
        a, b, c, d = -a, -b, -c, -d
    n = float(np.sqrt(float(a * a + b * b + c * c + d * d)))
    return a / n, b / n, c / n, d / n


def affine_of(geometry: dict) -> np.ndarray:
    """The voxel-to-world matrix the geometry's qform defines (float64)."""
    r = rotation(geometry["quaternion"])
    s = np.diag([*geometry["spacing"][:2], geometry["spacing"][2] * geometry["qfac"]])
    a = np.eye(4)
    a[:3, :3] = r @ s
    a[:3, 3] = geometry["origin"]
    return a


def sform_of(geometry: dict) -> np.ndarray:
    a = affine_of(geometry)
    if geometry.get("sform") == "other":
        sh = geometry.get("shear", [0.0, 0.0, 0.0])
        m = np.array([[1.0, sh[0], sh[1]], [0.0, 1.0, sh[2]], [0.0, 0.0, 1.0]])
        a[:3, :3] = a[:3, :3] @ m
        a[:3, 3] += [0.5, -0.25, 0.75]
    return a


# ------------------------------------------------------------------------------------------------
# Content: a function of world coordinates


def draw_content(d: Draw, kinds: Sequence[str], weights: Sequence[float] | None = None) -> dict:
    kind = d.choice("content", tuple(kinds), weights)
    c: dict[str, Any] = {"type": kind, "seed": d.seed()}
    if kind in ("head", "blobs", "labels"):
        c["centre"] = [d.step(f"centre{k}", -6.0, 6.0, 0.5) for k in range(3)]
        c["size"] = d.step("size", 8.0, 30.0, 0.5)
        c["axes"] = [d.step(f"axis{k}", 0.75, 1.25, 0.05) for k in range(3)]
    if kind == "head":
        c["bias"] = [d.step(f"bias{k}", -0.01, 0.01, 0.0025) for k in range(4)]
        c["noise"] = d.choice("noise", (0.0, 1.0, 3.0, 6.0))
        c["tissues"] = d.choice("tissues", ("t1", "t2", "epi"))
    if kind == "blobs":
        c["n"] = d.integer("blobs", 1, 6)
    if kind == "labels":
        c["n"] = d.integer("labels", 1, 8)
    if kind == "ramp":
        c["direction"] = [d.integer(f"ramp{k}", -2, 2) for k in range(3)]
    if kind == "checker":
        c["period"] = d.integer("period", 1, 6)
    if kind == "sparse":
        c["fraction"] = d.choice("sparse", (0.001, 0.01, 0.05))
    if kind == "crop":
        c["source"] = d.choice(
            "crop source",
            (
                "templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-02_T1w.nii.gz",
                "templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-02_desc-fMRIPrep_boldref.nii.gz",
            ),
        )
        c["corner"] = [d.integer(f"corner{k}", 20, 60) for k in range(3)]
        c["stride"] = d.choice("crop stride", (1, 1, 2))
    return c


def _irwin_hall(rng: np.random.Generator, shape) -> np.ndarray:
    """Noise with mean 0 and variance 1/4: the sum of three uniform draws, centred."""
    return rng.random(shape) + rng.random(shape) + rng.random(shape) - 1.5


def world_points(shape: Sequence[int], affine: np.ndarray) -> np.ndarray:
    """The world coordinates (3, *shape[:3]) of every voxel centre."""
    sp = list(shape[:3]) + [1] * (3 - len(shape[:3]))
    idx = np.indices(sp, dtype=np.float64).reshape(3, -1)
    pts = affine[:3, :3] @ idx + affine[:3, 3:4]
    return pts.reshape(3, *sp)


def render(content: dict, points: np.ndarray, *, volume: int = 0) -> np.ndarray:
    """The content's values at world ``points`` (3, ...): float64, before ``values``."""
    kind = content["type"]
    rng = np.random.default_rng([content["seed"], volume])
    shape = points.shape[1:]
    if kind == "zeros":
        return np.zeros(shape)
    if kind == "constant":
        return np.full(shape, 100.0)
    if kind == "noise":
        return rng.random(shape) * 100.0
    if kind == "ramp":
        dirn = np.asarray(content["direction"], dtype=np.float64)
        if not dirn.any():
            dirn = np.array([1.0, 0.0, 0.0])
        return 50.0 + np.tensordot(dirn, points, axes=1)
    if kind == "checker":
        p = content["period"]
        cells = sum(np.floor(points[k] / p) for k in range(3))
        return np.where(cells % 2 == 0, 100.0, 20.0)
    if kind == "sparse":
        out = np.zeros(shape)
        hit = rng.random(shape) < content["fraction"]
        out[hit] = 1.0 + np.floor(rng.random(int(hit.sum())) * 4)
        return out
    if kind == "crop":
        return _crop(content, shape)
    c = np.asarray(content["centre"], dtype=np.float64).reshape(3, *([1] * len(shape)))
    size = float(content["size"])
    axes = np.asarray(content["axes"], dtype=np.float64).reshape(3, *([1] * len(shape)))
    u = (points - c) / (size * axes)
    r2 = (u * u).sum(axis=0)
    if kind == "head":
        return _head(content, u, r2, rng, shape)
    if kind == "blobs":
        out = np.zeros(shape)
        brng = np.random.default_rng(content["seed"])
        for _ in range(content["n"]):
            centre = (brng.random(3) - 0.5) * 1.2
            radius = 0.2 + 0.4 * brng.random()
            amp = 20.0 + 80.0 * brng.random()
            dd = ((u - centre.reshape(3, *([1] * len(shape)))) ** 2).sum(axis=0) / radius**2
            out += np.where(dd < 1.0, amp * (1.0 - dd) ** 2, 0.0)
        return out
    if kind == "labels":
        lrng = np.random.default_rng(content["seed"])
        seeds = (lrng.random((content["n"], 3)) - 0.5) * 1.4
        best = np.full(shape, np.inf)
        lab = np.zeros(shape)
        for k, s in enumerate(seeds):
            dd = ((u - s.reshape(3, *([1] * len(shape)))) ** 2).sum(axis=0)
            better = dd < best
            best = np.where(better, dd, best)
            lab = np.where(better, float(k + 1), lab)
        return np.where(r2 < 1.0, lab, 0.0)
    raise ValueError(f"unknown content {kind!r}")


def _head(content: dict, u: np.ndarray, r2: np.ndarray, rng, shape) -> np.ndarray:
    """Nested ellipsoids: skull, CSF, grey and white matter, ventricles; a polynomial bias
    field; Irwin-Hall noise."""
    t = content.get("tissues", "t1")
    level = {
        "t1": (160, 30, 70, 110, 25),
        "t2": (40, 160, 100, 70, 170),
        "epi": (60, 120, 90, 80, 130),
    }[t]
    skull, csf, gm, wm, vent = level
    wavy = 0.7 + 0.08 * (u[0] * u[0] - u[1] * u[1]) + 0.05 * u[2]
    out = np.where(r2 < 1.12**2, float(skull), 0.0)
    out = np.where(r2 < 1.0, float(csf), out)
    out = np.where(r2 < 0.92**2, float(gm), out)
    out = np.where(r2 < wavy * wavy, float(wm), out)
    v = (u[0] / 0.25) ** 2 + (u[1] / 0.12) ** 2 + (u[2] / 0.18) ** 2
    out = np.where(v < 1.0, float(vent), out)
    b = content.get("bias", [0.0] * 4)
    size = 1.0
    bias = 1.0 + size * (
        b[0] * u[0] * 30 + b[1] * u[1] * 30 + b[2] * u[2] * 30 + b[3] * u[0] * u[1] * 900
    )
    out = out * bias
    if content.get("noise", 0.0):
        out = np.abs(out + content["noise"] * 2.0 * _irwin_hall(rng, shape))
    return out


def _crop(content: dict, shape: Sequence[int]) -> np.ndarray:
    import larmorx_testdata as td
    import nibabel as nib

    img = np.asarray(nib.load(td.get(content["source"])).dataobj, dtype=np.float64)
    s = content["stride"]
    c = content["corner"]
    sl = tuple(slice(c[k], c[k] + s * shape[k], s) for k in range(len(shape)))
    block = img[sl]
    out = np.zeros(shape)
    out[tuple(slice(0, n) for n in block.shape)] = block
    return out


# ------------------------------------------------------------------------------------------------
# Values, types, scaling


def draw_values(d: Draw, *, specials: float = 0.1, extreme: float = 0.15) -> dict:
    v: dict[str, Any] = {"scale": 1.0, "offset": 0.0}
    r = d.choice(
        "range",
        ("plain", "tiny", "huge", "negative", "steps", "unit"),
        (1 - extreme, extreme * 0.2, extreme * 0.2, extreme * 0.3, extreme * 0.2, extreme * 0.1),
    )
    if r == "tiny":
        v["scale"] = 1e-30
    elif r == "huge":
        v["scale"] = 1e30
    elif r == "negative":
        v["offset"] = -d.step("offset", 10.0, 200.0, 10.0)
    elif r == "steps":
        v["quantize"] = d.choice("quantize", (1.0, 10.0, 25.0))
    elif r == "unit":
        v["scale"] = 0.01
    if d.rng.random() < specials:
        v["specials"] = d.subset("specials", ("nan", "inf", "-inf", "-0", "outlier"), 0.4) or [
            "outlier"
        ]
    return v


def draw_dtype(
    d: Draw,
    options: Sequence[str] = (
        "float32",
        "float32",
        "float32",
        "int16",
        "uint8",
        "float64",
        "int32",
        "uint16",
        "int8",
    ),
) -> str:
    return d.choice("dtype", tuple(options))


def draw_scaling(d: Draw, dtype: str, p: float = 0.2) -> dict | None:
    if dtype.startswith("float") or d.rng.random() >= p:
        return None
    slope = d.choice("slope", (0.5, 2.0, 0.01, -1.0, 1.5))
    inter = d.choice("inter", (0.0, 0.0, 10.0, -5.0))
    return {"slope": slope, "inter": inter}


def apply_values(x: np.ndarray, values: dict, rng: np.random.Generator) -> np.ndarray:
    if values.get("binarize"):
        return (x > 0).astype(np.float64)
    x = x * values.get("scale", 1.0) + values.get("offset", 0.0)
    if q := values.get("quantize"):
        x = np.floor(x / q) * q
    for s in values.get("specials", ()):
        if x.size == 0:
            break
        i = int(rng.integers(0, x.size))
        flat = x.reshape(-1)
        flat[i] = {"nan": np.nan, "inf": np.inf, "-inf": -np.inf, "-0": -0.0}.get(s, 0.0)
        if s == "outlier":
            flat[i] = (
                np.nanmax(np.abs(flat[np.isfinite(flat)])) if np.isfinite(flat).any() else 1.0
            ) * 50.0
    return x


def to_dtype(x: np.ndarray, dtype: str, scaling: dict | None) -> np.ndarray:
    """``x`` stored as ``dtype``: integers rounded and clipped to the type's range (NaN to 0),
    after ``scaling`` is undone (``stored = (x - inter) / slope``)."""
    if scaling:
        x = (x - scaling.get("inter", 0.0)) / scaling["slope"]
    dt = np.dtype(dtype)
    if dt.kind == "f":
        with np.errstate(over="ignore", invalid="ignore"):
            return x.astype(dt)
    info = np.iinfo(dt)
    x = np.where(np.isfinite(x), x, 0.0)
    x = np.clip(np.floor(x + 0.5), float(info.min), float(info.max))
    return x.astype(dt)


# ------------------------------------------------------------------------------------------------
# Headers and files


def draw_timing(d: Draw, nz: int) -> dict:
    t = {
        "tr": d.choice("tr", (0.5, 1.0, 2.0, 2.5, 3.0, 0.8)),
        "tunits": d.choice("tunits", ("sec", "sec", "sec", "msec", "unknown")),
        "slice_code": 0,
    }
    if t["tunits"] == "msec":
        t["tr"] = t["tr"] * 1000.0
    if nz > 1 and d.rng.random() < 0.5:
        t["slice_code"] = d.choice("slice_code", (1, 2, 3, 4, 5, 6))
        t["slice_start"] = 0
        t["slice_end"] = nz - 1
        t["slice_duration"] = round(float(t["tr"]) / nz, 4)
    return t


def draw_image(
    d: Draw,
    ndim: int = 3,
    *,
    contents: Sequence[str] = (
        "head",
        "blobs",
        "noise",
        "ramp",
        "checker",
        "constant",
        "zeros",
        "sparse",
        "labels",
    ),
    content_weights: Sequence[float] | None = None,
    dtypes: Sequence[str] | None = None,
    shape: Sequence[int] | None = None,
    geometry: dict | None = None,
    suffix: str | None = None,
    specials: float = 0.1,
    extreme: float = 0.15,
    scaling: float = 0.2,
    **shape_kw,
) -> dict:
    """A random image spec (see the module's documentation)."""
    spec: dict[str, Any] = {}
    spec["shape"] = list(shape) if shape is not None else draw_shape(d, ndim, **shape_kw)
    spec["geometry"] = geometry if geometry is not None else draw_geometry(d)
    spec["content"] = draw_content(d, contents, content_weights)
    spec["values"] = draw_values(d, specials=specials, extreme=extreme)
    spec["dtype"] = draw_dtype(d) if dtypes is None else d.choice("dtype", tuple(dtypes))
    spec["scaling"] = draw_scaling(d, spec["dtype"], scaling)
    if len(spec["shape"]) == 4:
        spec["timing"] = draw_timing(d, spec["shape"][2])
    spec["suffix"] = suffix or d.choice("suffix", (".nii.gz", ".nii.gz", ".nii"))
    return spec


def field_data(spec: dict) -> np.ndarray:
    """A smooth displacement field (``spec["field"]``: amplitude in mm, a seed): each component
    a random quadratic polynomial of the normalised voxel coordinates; shape (x, y, z, 1, 3)."""
    shape = list(spec["shape"])
    f = spec["field"]
    rng = np.random.default_rng(f["seed"])
    spatial = shape[:3]
    idx = np.indices(spatial, dtype=np.float64)
    u = [(idx[k] - (spatial[k] - 1) / 2) / max(1.0, spatial[k] / 2) for k in range(3)]
    comps = []
    for _ in range(shape[4]):
        c = (rng.random(10) - 0.5) * 2 * f["amplitude"]
        v = (
            c[0]
            + c[1] * u[0]
            + c[2] * u[1]
            + c[3] * u[2]
            + c[4] * u[0] * u[1]
            + c[5] * u[1] * u[2]
            + c[6] * u[0] * u[2]
            + c[7] * u[0] * u[0]
            + c[8] * u[1] * u[1]
            + c[9] * u[2] * u[2]
        )
        comps.append(v)
    data = np.stack(comps, axis=-1).reshape(shape)
    return to_dtype(data, spec.get("dtype", "float32"), None)


def image_data(spec: dict) -> np.ndarray:
    """The stored values of ``spec`` (its dtype; the volumes on the last axis)."""
    if spec.get("field"):
        return field_data(spec)
    shape = list(spec["shape"])
    if len(shape) > 4:  # 5 to 7 axes: noise (the NIfTI tests' layouts)
        rng = np.random.default_rng(spec["content"]["seed"])
        data = rng.random(int(np.prod(shape))).reshape(shape) * 100.0
        data = apply_values(data, spec.get("values", {}), rng)
        return to_dtype(data, spec.get("dtype", "float32"), spec.get("scaling"))
    affine = affine_of(spec["geometry"])
    points = world_points(shape, affine)
    vols = shape[3] if len(shape) == 4 else 1
    rng = np.random.default_rng([spec["content"]["seed"], 99])
    out = []
    for t in range(vols):
        p = points
        if (m := spec.get("motion")) and t < len(m):
            p = move_points(points, m[t])
        x = render(spec["content"], p, volume=t)
        if (sig := spec.get("signal")) is not None:
            x = x * (1.0 + _triangle(t, sig))
        out.append(x)
    data = np.stack(out, axis=-1) if len(shape) == 4 else out[0]
    data = data.reshape(shape)
    data = apply_values(data, spec.get("values", {}), rng)
    return to_dtype(data, spec.get("dtype", "float32"), spec.get("scaling"))


def _triangle(t: int, signal: dict) -> float:
    """A triangle wave in time (plain arithmetic): amplitude and period in volumes."""
    period = signal.get("period", 4)
    phase = (t % period) / period
    return signal.get("amplitude", 0.05) * (4.0 * abs(phase - 0.5) - 1.0)


def move_points(points: np.ndarray, motion: dict) -> np.ndarray:
    """``points`` moved by the inverse of a rigid motion (rotation quaternion and translation
    in mm about ``centre``): sampling the phantom there gives the phantom moved."""
    r = rotation(motion.get("quaternion", (1, 0, 0, 0)))
    c = np.asarray(motion.get("centre", (0.0, 0.0, 0.0)), dtype=np.float64).reshape(3, 1)
    t = np.asarray(motion.get("translation", (0.0, 0.0, 0.0)), dtype=np.float64).reshape(3, 1)
    flat = points.reshape(3, -1)
    moved = r.T @ (flat - c - t) + c
    return moved.reshape(points.shape)


def header_bytes(spec: dict, data: np.ndarray, nifti2: bool = False) -> bytes:
    """The NIfTI-1 (or -2) header of ``spec`` for ``data`` (single file), every field written
    explicitly."""
    import nibabel as nib

    hdtype = nib.nifti2.header_dtype if nifti2 else nib.nifti1.header_dtype
    h = np.zeros((), dtype=hdtype)
    big = spec.get("header", {}).get("big_endian", False)
    shape = list(data.shape)
    h["sizeof_hdr"] = 540 if nifti2 else 348
    h["magic"] = (
        b"n+2\x00\r\n\x1a\n"
        if nifti2
        else (b"n+1" if spec.get("suffix", ".nii") != ".hdr" else b"ni1")
    )
    dim = [len(shape), *shape] + [1] * (7 - len(shape))
    h["dim"] = dim
    h["datatype"] = DTYPES[data.dtype.name]
    h["bitpix"] = data.dtype.itemsize * 8
    g = spec["geometry"]
    sp = list(g["spacing"])
    timing = spec.get("timing", {})
    pixdim = [float(g.get("qfac", 1)), *sp, float(timing.get("tr", 0.0)), 0.0, 0.0, 0.0]
    if nifti2:
        h["pixdim"] = pixdim
    else:
        h["pixdim"] = np.asarray(pixdim, dtype=np.float32)
    h["vox_offset"] = (
        0
        if spec.get("suffix") == ".hdr"
        else (544 if nifti2 else 352)
        + _extension_size(spec)
        + spec.get("header", {}).get("vox_pad", 0)
    )
    sc = spec.get("scaling") or {}
    h["scl_slope"] = sc.get("slope", 0.0) if sc else 0.0
    h["scl_inter"] = sc.get("inter", 0.0) if sc else 0.0
    h["xyzt_units"] = (
        _UNITS[g.get("units", "mm")]
        | _TUNITS[timing.get("tunits", "sec" if len(shape) == 4 else "unknown")]
    )
    if timing.get("slice_code"):
        h["slice_code"] = timing["slice_code"]
        h["slice_start"] = timing.get("slice_start", 0)
        h["slice_end"] = timing.get("slice_end", 0)
        h["slice_duration"] = timing.get("slice_duration", 0.0)
        h["dim_info"] = 3 << 4 | 2 << 2 | 1  # slice axis 3 (k)
    other = spec.get("header", {})
    if spec.get("field"):
        h["intent_code"] = 1007  # NIFTI_INTENT_VECTOR: a displacement field
    special = spec.get("header", {}).get("slope_special")
    if special:
        h["scl_slope"] = {"nan": np.nan, "inf": np.inf, "0": 0.0, "-0": -0.0}[special]
    for name in ("cal_min", "cal_max", "toffset", "intent_code", "intent_p1"):
        if name in other:
            h[name] = other[name]
    h["descrip"] = other.get("descrip", "larmorx difftest").encode("ascii")[:79]
    h["qform_code"] = g.get("qform_code", 1)
    h["sform_code"] = g.get("sform_code", 1)
    _, b, c, dq = _unit_quaternion(g["quaternion"])
    h["quatern_b"], h["quatern_c"], h["quatern_d"] = b, c, dq
    h["qoffset_x"], h["qoffset_y"], h["qoffset_z"] = g["origin"]
    s = sform_of(g)
    h["srow_x"], h["srow_y"], h["srow_z"] = s[0], s[1], s[2]
    raw = h.astype(h.dtype.newbyteorder(">")).tobytes() if big else h.tobytes()
    return raw


def _extension_size(spec: dict) -> int:
    return len(_extension_bytes(spec, False)) - 4


def _extension_bytes(spec: dict, big: bool) -> bytes:
    """The extender and the extensions (``header.extensions``: (code, text) pairs, each padded
    to a multiple of 16 bytes)."""
    exts = spec.get("header", {}).get("extensions", [])
    if not exts:
        return b"\x00\x00\x00\x00"
    order = ">" if big else "<"
    out = b"\x01\x00\x00\x00"
    for code, text in exts:
        body = text.encode("utf-8")
        size = 8 + len(body)
        size += (-size) % 16
        out += (
            np.array([size, code], dtype=order + "i4").tobytes()
            + body
            + b"\x00" * (size - 8 - len(body))
        )
    return out


def write_nifti(spec: dict, target: Path) -> Path:
    """Writes ``spec`` at ``target`` (its suffix decides the form: ``.nii``, ``.nii.gz``,
    ``.hdr`` with ``.img`` beside it). Returns the path to name on command lines."""
    data = image_data(spec)
    nifti2 = bool(spec.get("header", {}).get("nifti2"))
    big = spec.get("header", {}).get("big_endian", False)
    hdr = header_bytes(spec, data, nifti2)
    order = ">" if big else "<"
    raw = np.asarray(data, dtype=data.dtype.newbyteorder(order)).tobytes(order="F")
    target.parent.mkdir(parents=True, exist_ok=True)
    if spec.get("suffix") == ".hdr":
        target.write_bytes(hdr + _extension_bytes(spec, big))
        target.with_suffix(".img").write_bytes(raw)
        return target
    body = (
        hdr + _extension_bytes(spec, big) + b"\x00" * spec.get("header", {}).get("vox_pad", 0) + raw
    )
    if str(target).endswith(".gz"):
        target.write_bytes(gzip.compress(body, compresslevel=6, mtime=0))
    else:
        target.write_bytes(body)
    return target


# ------------------------------------------------------------------------------------------------
# Transforms


def draw_rigid(d: Draw, *, degrees: str = "small", shift: float = 4.0) -> dict:
    """A rigid motion: a rational rotation quaternion, a translation (mm), a centre."""
    n = {"small": d.integer("rot scale", 12, 60), "large": d.integer("rot scale", 2, 6)}[degrees]
    q = (n, d.integer("rot b", -2, 2), d.integer("rot c", -2, 2), d.integer("rot d", -2, 2))
    return {
        "quaternion": list(q),
        "translation": [d.step(f"shift{k}", -shift, shift, 0.25) for k in range(3)],
        "centre": [0.0, 0.0, 0.0],
    }


def itk_text(transforms: Sequence[dict]) -> str:
    """An ITK text transform file (``#Insight Transform File V1.0``) of ``transforms``: each
    ``{"type": name, "parameters": [...], "fixed": [...]}``."""
    lines = ["#Insight Transform File V1.0"]
    for i, t in enumerate(transforms):
        lines += [
            f"#Transform {i}",
            f"Transform: {t['type']}",
            "Parameters: " + " ".join(_num(v) for v in t["parameters"]),
            "FixedParameters: " + " ".join(_num(v) for v in t["fixed"]),
        ]
    return "\n".join(lines) + "\n"


def _num(v: float) -> str:
    return (
        repr(float(v))
        if not float(v).is_integer()
        else str(int(v))
        if abs(v) < 1e15
        else repr(float(v))
    )


def write_itk_mat(transform: dict, target: Path) -> None:
    """An ITK MATLAB transform file (MATLAB v4: the parameters and ``fixed``), as ITK writes."""
    import io

    import scipy.io as sio

    b = io.BytesIO()
    sio.savemat(
        b,
        {
            transform["type"]: np.asarray(transform["parameters"], dtype="<f8").reshape(-1, 1),
            "fixed": np.asarray(transform["fixed"], dtype="<f8").reshape(-1, 1),
        },
        format="4",
    )
    target.write_bytes(b.getvalue())


def affine_parameters(matrix: np.ndarray, translation: Sequence[float]) -> list[float]:
    return [float(v) for v in np.asarray(matrix).reshape(-1)] + [float(v) for v in translation]


# ------------------------------------------------------------------------------------------------
# Recipes


def digest(obj: Any) -> str:
    """A short, stable name for a JSON-able object (a recipe, an input spec)."""
    text = json.dumps(obj, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(text.encode("utf-8")).hexdigest()[:16]


def pack_floats(values: Sequence[float]) -> str:
    """Floats as text, exactly (``float.hex``), for recipes that must keep every bit."""
    return " ".join(float(v).hex() for v in values)


def unpack_floats(text: str) -> list[float]:
    return [float.fromhex(v) for v in text.split()]


__all__ = [
    "AXIS_QUATERNIONS",
    "DTYPES",
    "MAX_VOXELS",
    "Draw",
    "affine_of",
    "affine_parameters",
    "digest",
    "draw_content",
    "draw_dtype",
    "draw_geometry",
    "draw_image",
    "draw_rigid",
    "draw_scaling",
    "draw_shape",
    "draw_timing",
    "draw_values",
    "header_bytes",
    "image_data",
    "itk_text",
    "move_points",
    "plumb_geometry",
    "quaternion_product",
    "render",
    "rotation",
    "sform_of",
    "world_points",
    "write_itk_mat",
    "write_nifti",
]
