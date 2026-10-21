# SPDX-License-Identifier: Apache-2.0
"""A worked example being exported: its display grid and mask, its panels (an input, or one
program's run with its outputs), the comparisons between them, and the still posters of its
default view (for pages read without JavaScript)."""

from __future__ import annotations

import io
from collections.abc import Callable
from dataclasses import dataclass

import numpy as np

from larmorx_validation.evidence.core import Context, Grid, compare, dtype_code, encode, put, store


@dataclass
class Panel:
    """One image of a worked example: an input, or one program's run (its outputs)."""

    id: str
    role: str  # input, original, fmriprep, reference, larmorx
    label: str
    sub: str
    full: dict[str, np.ndarray]  # output id ("all" for an input) -> the file's array, as stored
    scale: float | None = None  # an integer image's scale factor (scl_slope)
    inter: float = 0.0


class Example:
    """A worked example: arrays are shown on the display grid, zero outside the display mask
    (the brain, slightly dilated, so that no face can show).

    Each panel's array is stored once: identical to an earlier panel's (``same``), the
    negation of a shown one (``neg``), its raw values (the first of each output), or the
    exclusive or of its bits with a reference (``xor``): the output's first array, or what
    ``predict[output](panel id)`` returns, a closer reference (e.g. a quotient of two arrays)."""

    def __init__(self, ctx: Context, grid: Grid, show: np.ndarray):
        self.ctx, self.grid = ctx, grid
        self.mask = grid.take(show).astype(np.uint8)
        self.m = self.mask.astype(bool)
        self.panels: list[Panel] = []
        self.specs: dict[str, dict] = {}
        self.shown: dict[str, np.ndarray] = {}  # "pid:out" -> display array, as stored
        self.predict: dict[str, Callable[[str], tuple[np.ndarray, object]]] = {}

    def panel(self, pid: str) -> Panel:
        return next(p for p in self.panels if p.id == pid)

    def key(self, pid: str, out: str) -> str:
        return f"{pid}:{out}" if f"{pid}:{out}" in self.shown else f"{pid}:all"

    def disp(self, a: np.ndarray) -> np.ndarray:
        t = self.grid.take(a)
        m = self.m if t.ndim == 3 else self.m[..., None]
        return np.where(m, t, np.zeros((), a.dtype)).astype(a.dtype)

    def values(self, key: str) -> np.ndarray:
        """A shown array in the image's units (float64): stored × scale + intercept."""
        p = self.panel(key.split(":")[0])
        a = self.shown[key].astype(np.float64)
        return a * p.scale + p.inter if p.scale is not None else a

    def add(self, p: Panel) -> None:
        spec: dict[str, dict] = {}
        for out, full in p.full.items():
            key = f"{p.id}:{out}"
            meta = {} if p.scale is None else {"scale": float(p.scale), "inter": float(p.inter)}
            same = next(
                (
                    f"{q.id}:{o2}"
                    for q in self.panels
                    for o2, f2 in q.full.items()
                    if o2 in (out, "all")
                    and q.scale == p.scale
                    and q.inter == p.inter
                    and f2.dtype == full.dtype
                    and f2.shape == full.shape
                    and f2.tobytes() == full.tobytes()
                ),
                None,
            )
            if same:
                spec[out] = {"same": same, "dtype": dtype_code(full), **meta}
                self.shown[key] = self.shown[same]
                continue
            a = self.disp(full)
            self.shown[key] = a
            neg = None
            if a.dtype.kind == "i":
                neg = next(
                    (
                        k
                        for k, b in self.shown.items()
                        if k != key
                        and b.shape == a.shape
                        and b.dtype == a.dtype
                        and np.array_equal(-b, a)
                    ),
                    None,
                )
            if neg:
                spec[out] = {"neg": neg, "dtype": dtype_code(full), **meta}
            else:
                # the smallest of: the values, the bits' difference from the shown array of the
                # same type closest to it (the fewest differing bytes), the bits' difference
                # from the predicted reference
                cands = [encode(a)]
                bytes_a = a.view(np.uint8)
                refs = [
                    (int(np.count_nonzero(b.view(np.uint8) != bytes_a)), k)
                    for k, b in self.shown.items()
                    if k != key
                    and b.dtype == a.dtype
                    and b.shape == a.shape
                    and not k.startswith(f"{p.id}:")
                ]
                if refs:
                    _, k = min(refs)
                    cands.append(encode(a, self.shown[k], k))
                if out in self.predict:
                    ref, ref_spec = self.predict[out](p.id)
                    cands.append(encode(a, ref, ref_spec))
                spec[out] = store(self.ctx, min(cands, key=lambda c: len(c[0])), **meta)
        self.panels.append(p)
        self.specs[p.id] = spec

    def compare(self, a: str, b: str, out: str) -> dict:
        """A − B: the metrics over the whole image (every voxel of the files), and where the
        displayed brain differs most (display voxel, both values there)."""
        pa, pb = self.panel(a), self.panel(b)
        fa = pa.full.get(out, pa.full.get("all"))
        fb = pb.full.get(out, pb.full.get("all"))
        if pa.scale is not None and pa.scale == pb.scale and pa.inter == pb.inter:
            m = compare(fa, fb, scale=pa.scale, inter=pa.inter)
        elif pa.scale is not None or pb.scale is not None:
            va = fa.astype(np.float64) * (pa.scale or 1.0) + pa.inter
            vb = fb.astype(np.float64) * (pb.scale or 1.0) + pb.inter
            m = compare(va, vb)
            m["identical"] = bool(np.array_equal(va, vb))
        else:
            m = compare(fa, fb)
        m["at_display"] = self.grid.display_index(tuple(m["at"]))
        va, vb = self.values(self.key(a, out)), self.values(self.key(b, out))
        d = np.abs(va - vb)
        k = int(np.argmax(d))
        at = [int(i) for i in np.unravel_index(k, d.shape)]
        shown = {
            "max_abs": float(d.flat[k]),
            "at": at,
            "a": float(va.flat[k]),
            "b": float(vb.flat[k]),
            "differ": int(np.count_nonzero(d)),
        }
        return {"metrics": m, "display": shown}

    def describe_panels(self) -> list[dict]:
        return [
            {"id": p.id, "role": p.role, "label": p.label, "sub": p.sub, "arrays": self.specs[p.id]}
            for p in self.panels
        ]


def own_windows(ex: Example, panels: list[dict], outputs: list[dict]) -> None:
    """Each output image's own window (0.5 to 99.5 % of its shown brain), for the viewer's
    *each image's own* preset: an image far off the others' range stays readable."""
    for p in panels:
        if p["role"] == "input":
            continue
        own = {}
        for o in outputs:
            key = ex.key(p["id"], o["id"])
            v = ex.values(key)
            m = ex.m if v.ndim == 3 else np.broadcast_to(ex.m[..., None], v.shape)
            lo, hi = np.percentile(v[m], [0.5, 99.5])
            own[o["id"]] = [float(lo), float(hi)]
        p["own"] = own
    for o in outputs:
        o["windows"].append({"id": "own", "label": "each image's own 0.5–99.5 %", "lo": 0, "hi": 1})


def windows(values: np.ndarray, label: str = "brain") -> list[dict]:
    """An output's intensity windows: 0.5 to 99.5 % of the shown brain, and its full range."""
    v = values.astype(np.float64)
    lo, hi = np.percentile(v, [0.5, 99.5])
    return [
        {"id": "brain", "label": f"{label}, 0.5–99.5 %", "lo": float(lo), "hi": float(hi)},
        {"id": "full", "label": "full range", "lo": float(v.min()), "hi": float(v.max())},
    ]


# ------------------------------------------------------------------------------- posters

#: plane -> (slice axis, across, up, across flipped), as the site's viewer draws it
PLANE_AXES = {"axial": (2, 0, 1, False), "coronal": (1, 0, 2, False), "sagittal": (0, 1, 2, True)}


def plane_slice(a: np.ndarray, plane: str, index: int) -> np.ndarray:
    """The slice of a display array (x, y, z) as an image (rows from the top, columns from the
    left), as the viewer draws it: the subject's left on the left, superior or anterior up."""
    s, u, v, flip = PLANE_AXES[plane]
    sl = np.take(a, index, axis=s)
    img = (sl if u < v else sl.T).T[::-1]  # rows: v from high to low; columns: u
    return img[:, ::-1] if flip else img


def _webp(rgba: np.ndarray, ctx: Context) -> str:
    from PIL import Image

    buf = io.BytesIO()
    Image.fromarray(rgba, "RGBA").save(buf, format="WEBP", lossless=True, method=6, exact=True)
    return put(ctx, buf.getvalue(), "webp")


def poster_gray(ctx: Context, a: np.ndarray, mask: np.ndarray, lo: float, hi: float) -> str:
    """A still panel: grey levels in the window, transparent outside the mask (lossless WebP)."""
    g = np.clip((a.astype(np.float64) - lo) * 255.0 / ((hi - lo) or 1.0), 0, 255).astype(np.uint8)
    return _webp(np.dstack([g, g, g, np.where(mask, 255, 0).astype(np.uint8)]), ctx)


def poster_diff(
    ctx: Context, d: np.ndarray, mask: np.ndarray, rng: float, log: bool = False
) -> str:
    """A still difference: blue below zero, apricot above, opaque in proportion to |Δ| / rng
    (or, on a log scale, to its place in the six decades below rng), any non-zero difference
    visible, a faint grey brain; the page's paper shows through. As the viewer draws it."""
    a = np.abs(d.astype(np.float64))
    rng = rng or 1.0
    if log:
        lo = rng * 1e-6
        with np.errstate(divide="ignore"):
            t = np.where(a > lo, np.log(np.maximum(a, lo) / lo) / np.log(rng / lo), 0.0)
    else:
        t = a / rng
    t = np.where(d != 0, np.maximum(0.12, np.minimum(1.0, t)), 0.0)
    col = np.where((d < 0)[..., None], np.array([70, 125, 234]), np.array([199, 103, 23]))
    col = np.where((t > 0)[..., None], col, np.array([128, 128, 128])).astype(np.uint8)
    alpha = np.where(mask, np.where(t > 0, 60 + 195 * t, 28), 0).astype(np.uint8)
    return _webp(np.dstack([col, alpha]), ctx)


def aspect(grid: Grid, plane: str) -> float:
    _, u, v, _ = PLANE_AXES[plane]
    sh, sp = grid.shape, grid.spacing
    return round(sh[u] * sp[u] / (sh[v] * sp[v]), 5)
