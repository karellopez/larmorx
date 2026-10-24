# SPDX-License-Identifier: Apache-2.0
"""A worked example being exported: its display grid and mask, its panels (an input, or one
program's run with its outputs), the comparisons between them, and the still posters of its
default view (for pages read without JavaScript).

**Views.** An example ships either its whole display grid (``view="volume"``: every slice of
the box around the shown region, for a tool's two or three key datasets) or three orthogonal
full-resolution slices through one voxel (``view="slices"``: the voxel where the main
comparison differs most, or the region's centre where nothing differs; for new data-panel cases
only, added beside the existing examples, never replacing a volume). Each slice is cropped only
where every array is exactly constant (the constant recorded, so the viewer draws it exactly),
and the arrays are stored as the slices one after the other (axial, coronal, sagittal; for a
series, volume by volume), each with its offset, so the viewer maps a voxel to its value
exactly. Every metric is still computed offline over every voxel of the files: a view decides
only what the reader can browse."""

from __future__ import annotations

import io
from collections.abc import Callable
from dataclasses import dataclass

import numpy as np

from larmorx_validation.evidence.core import (
    Context,
    Grid,
    blob,
    compare,
    dtype_code,
    encode,
    put,
    store,
)

#: An example's views: its whole display grid, or three slices through one voxel.
VIEWS = ("volume", "slices")
#: The planes of a slices view, in their order in the stored arrays.
PLANE_ORDER = ("axial", "coronal", "sagittal")


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
    #: output id -> {"on": panel id, "window": [lo, hi]}: the output (a mask) is drawn over
    #: that panel's image, in that window
    overlay: dict | None = None


class Example:
    """A worked example: arrays are shown on the display grid, zero outside the display mask
    (the brain, slightly dilated, so that no face can show).

    Each panel's array is stored once: identical to an earlier panel's (``same``), the
    negation of a shown one (``neg``), its raw values (the first of each output), or the
    exclusive or of its bits with a reference (``xor``): the output's first array, or what
    ``predict[output](panel id)`` returns, a closer reference (e.g. a quotient of two arrays).

    With ``view="slices"`` the arrays are encoded when :meth:`cut` names the voxel the three
    slices go through (``common.finish`` calls it with the default view's voxel)."""

    def __init__(self, ctx: Context, grid: Grid, show: np.ndarray, view: str = "volume"):
        if view not in VIEWS:
            raise ValueError(f"unknown view {view!r}; choose from {VIEWS}")
        self.ctx, self.grid, self.view = ctx, grid, view
        self.mask = grid.take(show).astype(np.uint8)
        self.m = self.mask.astype(bool)
        self.panels: list[Panel] = []
        self.specs: dict[str, dict] = {}
        self.shown: dict[str, np.ndarray] = {}  # "pid:out" -> display array, as stored
        self.predict: dict[str, Callable[[str], tuple[np.ndarray, object]]] = {}
        #: slices view: plane -> {index, box [u0, v0, nu, nv], offset}, once cut
        self.planes: dict[str, dict] | None = None
        self.voxel: list[int] | None = None
        self.per_volume = 0
        #: slices view: what is encoded at the cut (key, spec to fill, reference keys, prediction)
        self.pending: list[tuple[str, dict, list[str], tuple | None, dict]] = []

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
            elif self.view == "volume":
                spec[out] = self._encode(key, a, self._candidates(key, p.id, a), None, meta)
            else:
                spec[out] = {}
                pred = self.predict[out](p.id) if out in self.predict else None
                self.pending.append((key, spec[out], self._candidates(key, p.id, a), pred, meta))
        self.panels.append(p)
        self.specs[p.id] = spec

    def _candidates(self, key: str, pid: str, a: np.ndarray) -> list[str]:
        """The shown arrays an array may be stored against: the same type and shape, another
        panel's, shown before it."""
        return [
            k
            for k, b in self.shown.items()
            if k != key
            and b.dtype == a.dtype
            and b.shape == a.shape
            and not k.startswith(f"{pid}:")
        ]

    def _encode(self, key: str, a: np.ndarray, cands: list[str], pred, meta: dict) -> dict:
        """The smallest of: the values, the bits' difference from the candidate closest to it
        (the fewest differing bytes), the bits' difference from the predicted reference."""
        view = self.slices if self.view == "slices" else (lambda x: x)
        a = view(a)
        choices = [encode(a)]
        bytes_a = a.view(np.uint8)
        refs = [
            (int(np.count_nonzero(view(self.shown[k]).view(np.uint8) != bytes_a)), k) for k in cands
        ]
        if refs:
            _, k = min(refs)
            choices.append(encode(a, view(self.shown[k]), k))
        if pred is None and self.view == "volume":
            pid, out = key.split(":")
            if out in self.predict:
                pred = self.predict[out](pid)
        if pred is not None:
            ref, ref_spec = pred
            choices.append(encode(a, view(ref), ref_spec))
        return store(self.ctx, min(choices, key=lambda c: len(c[0])), **meta)

    # ------------------------------------------------------------------ slices
    def cut(self, voxel) -> None:
        """A slices view: the three planes through ``voxel`` (display voxel), and every array
        encoded on them. A plane is cropped only where every array is exactly constant: the box
        keeps every voxel where any array (input, output, mask) is not 0 or the display mask
        differs from its value at the plane's corner; outside the box every array is 0 and the
        mask is that corner value (``outside_mask``), which the viewer draws exactly."""
        if self.view != "slices":
            raise ValueError("cut() is for a slices view")
        voxel = [int(v) for v in voxel[:3]]
        if self.planes is not None:
            if voxel != self.voxel:
                raise ValueError(f"already cut at {self.voxel}, not {voxel}")
            return
        self.voxel, self.planes, off = voxel, {}, 0
        for name in PLANE_ORDER:
            s = PLANE_AXES[name][0]
            m2 = np.take(self.mask, voxel[s], axis=s)
            corner = int(m2[0, 0])
            keep = m2 != corner
            for a in self.shown.values():
                a2 = np.take(a, voxel[s], axis=s)
                keep |= (a2 != 0).any(axis=2) if a2.ndim == 3 else a2 != 0
            idx = np.argwhere(keep)
            if idx.size:
                lo, hi = idx.min(0), idx.max(0) + 1
            else:  # every array constant in this plane: one voxel
                lo, hi = np.zeros(2, int), np.ones(2, int)
            box = [int(lo[0]), int(lo[1]), int(hi[0] - lo[0]), int(hi[1] - lo[1])]
            self.planes[name] = {
                "index": voxel[s],
                "box": box,
                "offset": off,
                "outside_mask": corner,
            }
            off += box[2] * box[3]
        self.per_volume = off
        for key, spec, cands, pred, meta in self.pending:
            spec.update(self._encode(key, self.shown[key], cands, pred, meta))
        self.pending = []

    def slices(self, a: np.ndarray) -> np.ndarray:
        """A display array (x, y, z[, t]) as a slices view stores it: for each volume, the
        axial, coronal and sagittal slices, each cropped to its box, u fastest."""
        if self.planes is None:
            raise ValueError("the slices view is not cut yet")
        cuts = []
        for name in PLANE_ORDER:
            p = self.planes[name]
            u0, v0, nu, nv = p["box"]
            cuts.append(
                np.take(a, p["index"], axis=PLANE_AXES[name][0])[u0 : u0 + nu, v0 : v0 + nv]
            )
        nt = a.shape[3] if a.ndim == 4 else 1
        parts = [
            (c if a.ndim == 3 else c[..., t]).ravel(order="F") for t in range(nt) for c in cuts
        ]
        return np.ascontiguousarray(np.concatenate(parts))

    def unslice(self, i: int, ndim: int = 3) -> tuple[str, list[int]]:
        """The plane and display voxel (x, y, z, and t for a series) of position ``i`` in a
        slices array."""
        t, r = divmod(int(i), self.per_volume)
        for name in PLANE_ORDER:
            p = self.planes[name]
            u0, v0, nu, nv = p["box"]
            if r < p["offset"] + nu * nv:
                dv, du = divmod(r - p["offset"], nu)
                s, uu, vv, _ = PLANE_AXES[name]
                at = [0, 0, 0]
                at[s], at[uu], at[vv] = p["index"], u0 + du, v0 + dv
                return name, at + ([t] if ndim == 4 else [])
        raise AssertionError(i)

    def mask_blob(self) -> dict:
        return blob(self.ctx, self.slices(self.mask) if self.view == "slices" else self.mask)

    def describe_view(self) -> dict:
        if self.view == "volume":
            return {"kind": "volume"}
        return {
            "kind": "slices",
            "voxel": self.voxel,
            "planes": self.planes,
            "per_volume": self.per_volume,
        }

    def display_diff(self, ka: str, kb: str) -> dict:
        """Where the shown values of A and B differ most: over the whole display grid, or, in
        a cut slices view, over its three slices (with the plane)."""
        va, vb = self.values(ka), self.values(kb)
        ndim = va.ndim
        if self.planes is not None:
            va, vb = self.slices(va), self.slices(vb)
        d = np.abs(va - vb)
        k = int(np.argmax(d))
        if self.planes is not None:
            plane, at = self.unslice(k, ndim)
        else:
            plane, at = None, [int(i) for i in np.unravel_index(k, d.shape)]
        out = {
            "max_abs": float(d.flat[k]),
            "at": at,
            "a": float(va.flat[k]),
            "b": float(vb.flat[k]),
            "differ": int(np.count_nonzero(d)),
        }
        if plane:
            out["plane"] = plane
        return out

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
        return {"metrics": m, "display": self.display_diff(self.key(a, out), self.key(b, out))}

    def describe_panels(self) -> list[dict]:
        out = []
        for p in self.panels:
            d = {"id": p.id, "role": p.role, "label": p.label, "sub": p.sub}
            d["arrays"] = self.specs[p.id]
            if p.overlay:
                d["overlay"] = p.overlay
            out.append(d)
        return out


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


#: A mask over its image, as the viewer draws it on light paper: the fill (AFNI's pastel), mixed
#: half and half with the grey level, and the mask's edge in the slice (AFNI's deeper pastel).
MASK_FILL = (227, 199, 121)
MASK_EDGE = (152, 126, 46)


def poster_overlay(
    ctx: Context, base: np.ndarray, mask: np.ndarray, shown: np.ndarray, lo: float, hi: float
) -> str:
    """A still mask over the image it was made from: grey levels of ``base`` in the window, the
    voxels where ``mask`` is not zero tinted, its edge in this slice drawn in full, transparent
    outside ``shown`` (lossless WebP)."""
    g = np.clip((base.astype(np.float64) - lo) * 255.0 / ((hi - lo) or 1.0), 0, 255)[..., None]
    on = (mask != 0) & shown
    rgb = np.where(on[..., None], g + (np.array(MASK_FILL) - g) * 0.5, np.repeat(g, 3, axis=2))
    pad = np.pad(on, 1)
    inner = pad[:-2, 1:-1] & pad[2:, 1:-1] & pad[1:-1, :-2] & pad[1:-1, 2:]
    rgb = np.where((on & ~inner)[..., None], np.array(MASK_EDGE, dtype=np.float64), rgb)
    rgba = np.dstack([np.rint(rgb).astype(np.uint8), np.where(shown, 255, 0).astype(np.uint8)])
    return _webp(rgba, ctx)


def aspect(grid: Grid, plane: str) -> float:
    _, u, v, _ = PLANE_AXES[plane]
    sh, sp = grid.shape, grid.spacing
    return round(sh[u] * sp[u] / (sh[v] * sp[v]), 5)
